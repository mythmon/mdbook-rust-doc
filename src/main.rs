mod domain;

use crate::domain::{CrateRoots, RustPath};
use anyhow::{anyhow, bail, ensure, Context, Result};
use clap::Clap;
use proc_macro2::TokenTree;
use std::{convert::TryFrom, path::Path, string::ToString};
use syn::{Attribute, Fields, FieldsNamed, Item, ItemMod, ItemStruct};

#[derive(Clap, Debug)]
struct Opts {
    path: RustPath,

    #[clap(short, long = "crate")]
    crates: Vec<String>,
}

fn main() -> Result<()> {
    let opts = Opts::parse();
    let crate_roots =
        CrateRoots::try_from(opts.crates.clone()).context("Converting crate roots")?;
    let doc = find_item(&opts.path, &crate_roots)
        .context("Finding documentation")?
        .ok_or_else(|| anyhow!("Item {} not found", opts.path))?;
    println!(
        "{} doc:\n\n/// {}\n",
        opts.path,
        doc.replace("\n", "\n/// ")
    );
    Ok(())
}

fn find_item(path: &RustPath, crates: &CrateRoots) -> Result<Option<String>> {
    let (crate_name, item_path) = path.head_tail();
    let crate_path = crates
        .get(crate_name)
        .ok_or_else(|| anyhow!("Crate {} not found", crate_name))?;
    let crate_src_dir = crate_path.join("src");
    let attrs = find_attrs_in_crate(&crate_src_dir, &item_path)?;
    Ok(attrs.map(attrs_to_string))
}

fn find_attrs_in_crate(
    crate_src: &Path,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    let lib_path = crate_src.join("lib.rs");
    find_item_in_file(&lib_path, remaining_path)
}

fn find_attrs_in_struct(
    the_struct: &ItemStruct,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    if let Some(remaining_path) = remaining_path {
        let (head, tail) = remaining_path.head_tail();
        ensure!(
            tail.is_none(),
            "Expected tail to be none when scanning struct. Found {:?}",
            tail
        );

        let rv = match &the_struct.fields {
            Fields::Named(FieldsNamed { named, .. }) => named
                .iter()
                .find(|f| f.ident.as_ref().map(ToString::to_string) == Some(head.to_string()))
                .map(|field| field.attrs.clone()),

            Fields::Unnamed(_) | Fields::Unit => None,
        };

        Ok(rv)
    } else {
        Ok(Some(the_struct.attrs.clone()))
    }
}

fn find_attrs_in_mod(
    parent_path: &Path,
    the_mod: &ItemMod,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    if let Some((_, items)) = &the_mod.content {
        if let Some(remaining_path) = remaining_path {
            let rv = items
                .iter()
                .map(|i| find_attrs_in_item(parent_path, i, remaining_path))
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .next()
                .ok_or_else(|| {
                    anyhow!(
                        "Could not find expected item {} in {}",
                        remaining_path,
                        the_mod.ident
                    )
                })?;
            Ok(Some(rv))
        } else {
            Ok(Some(the_mod.attrs.clone()))
        }
    } else {
        let mod_path = match parent_path.file_stem() {
            Some(n) if n == "lib" => parent_path.with_file_name(format!("{}.rs", the_mod.ident)),
            _ => bail!(
                "Don't understand `parent_path` to find mod {}: {}",
                the_mod.ident,
                parent_path.to_string_lossy()
            ),
        };
        find_item_in_file(&mod_path, remaining_path)
    }
}

fn find_item_in_file(
    file_path: &Path,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    let file_text = std::fs::read_to_string(&file_path)
        .context(format!("Reading lib.rs at {}", file_path.to_string_lossy()))?;

    let ast =
        syn::parse_file(&file_text).context(format!("parsing {}", &file_path.to_string_lossy()))?;

    if let Some(remaining_path) = remaining_path {
        let attrs = ast
            .items
            .into_iter()
            .map(|i| find_attrs_in_item(file_path, &i, remaining_path))
            .collect::<Result<Vec<Option<Vec<Attribute>>>>>()
            .context("Error processing file")?
            .into_iter()
            .flatten()
            .next()
            .ok_or_else(|| {
                anyhow!(
                    "Could not find expected item {} in {}",
                    remaining_path,
                    file_path.to_string_lossy()
                )
            })?;
        Ok(Some(attrs))
    } else {
        Ok(Some(ast.attrs))
    }
}

fn find_attrs_in_item(
    parent_path: &Path,
    item: &Item,
    remaining_path: &RustPath,
) -> Result<Option<Vec<Attribute>>> {
    let (head, tail) = remaining_path.head_tail();

    match item {
        Item::Struct(s) => {
            if s.ident == head {
                find_attrs_in_struct(s, &tail).context(format!("Looking inside struct {}", s.ident))
            } else {
                Ok(None)
            }
        }
        Item::Mod(m) => {
            if m.ident == head {
                find_attrs_in_mod(parent_path, m, &tail)
                    .context(format!("Looking inside mod {}", m.ident))
            } else {
                Ok(None)
            }
        }
        Item::Impl(_) | Item::Enum(_) | Item::Use(_) => Ok(None),
        _ => bail!("Unexpected AST item {:?}", item),
    }
}

fn attrs_to_string(attrs: Vec<Attribute>) -> String {
    attrs
        .iter()
        .filter(|attr| attr.path.get_ident().map(ToString::to_string) == Some("doc".to_string()))
        .map(|attr| {
            let tokens = &attr.tokens.clone().into_iter().collect::<Vec<_>>();
            match (tokens.len(), tokens.get(0), tokens.get(1)) {
                (2, Some(TokenTree::Punct(c)), Some(TokenTree::Literal(l)))
                    if c.as_char() == '=' =>
                {
                    l.to_string()
                        .trim_matches('b') // byte strings/chars
                        .trim_matches('"') // strings
                        .trim_matches('\'') // chars
                        .trim() // any whitespace
                        .to_string()
                }
                _ => {
                    panic!("Unexpected format for docstring attribute {:?}", tokens)
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

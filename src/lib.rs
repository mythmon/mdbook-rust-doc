mod domain;

use anyhow::{anyhow, bail, ensure, Context, Result};
use std::{path::Path, string::ToString};
use syn::{
    Attribute, Fields, FieldsNamed, FieldsUnnamed, Item, ItemEnum, ItemImpl, ItemMod, ItemStruct,
    Type, Variant,
};

pub use crate::domain::{CrateRoots, RustPath};

/// Load the docstring for an item given by `path`, with crate information from `crates`.
///
/// # Errors
/// If the path cannot be found, a descriptive [`anyhow`] will be returned.
pub fn find_doc_for_item(path: &RustPath, crates: &CrateRoots) -> Result<Option<String>> {
    let (crate_name, item_path) = path.head_tail();
    let crate_path = crates
        .get(crate_name)
        .ok_or_else(|| anyhow!("Crate {} not found", crate_name))?;
    let crate_src_dir = crate_path.join("src");
    let attrs = find_attrs_in_crate(&crate_src_dir, &item_path)?;
    Ok(attrs.map(|attrs| attrs_to_string(&attrs)))
}

fn find_attrs_in_crate(
    crate_src: &Path,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    let lib_path = crate_src.join("lib.rs");
    find_item_in_file(&lib_path, remaining_path)
}

fn find_item_in_file(
    file_path: &Path,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    let file_text = std::fs::read_to_string(file_path)
        .context(format!("Reading lib.rs at {}", file_path.to_string_lossy()))?;

    let ast =
        syn::parse_file(&file_text).context(format!("parsing {}", &file_path.to_string_lossy()))?;

    if let Some(remaining_path) = remaining_path {
        let attrs = ast
            .items
            .into_iter()
            .map(|i| {
                find_attrs_in_item(file_path, &i, remaining_path)
                    .context(format!("Looking for {} in {:?}", remaining_path, i))
            })
            .collect::<Result<Vec<Option<Vec<Attribute>>>>>()
            .context(format!(
                "Error finding {} in file {}",
                remaining_path,
                file_path.to_string_lossy()
            ))?
            .into_iter()
            .flatten()
            .next();
        Ok(attrs)
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
        Item::Enum(e) => {
            if e.ident == head {
                find_attrs_in_enum(e, &tail).context(format!("Looking inside enum {}", e.ident))
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
        Item::Impl(i) => {
            if type_has_name(&i.self_ty, head) {
                Ok(find_attrs_in_impl(i, &tail))
            } else {
                Ok(None)
            }
        }

        Item::Use(_) | Item::ForeignMod(_) | Item::ExternCrate(_) => Ok(None),

        Item::Const(_) => bail!("Todo item type: Const"),
        Item::Fn(_) => bail!("Todo item type: Fn"),
        Item::Macro(_) => bail!("Todo item type: Macro"),
        Item::Static(_) => bail!("Todo item type: Static"),
        Item::Trait(_) => bail!("Todo item type: Trait"),
        Item::TraitAlias(_) => bail!("Todo item type: TraitAlias"),
        Item::Type(_) => bail!("Todo item type: Type"),
        Item::Union(_) => bail!("Todo item type: Union"),

        _ => bail!("Unexpected AST item {:?}", item),
    }
}

fn find_attrs_in_mod(
    parent_path: &Path,
    the_mod: &ItemMod,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    if let Some((_, items)) = &the_mod.content {
        if let Some(remaining_path) = &remaining_path {
            let rv = items
                .iter()
                .map(|i| {
                    find_attrs_in_item(parent_path, i, remaining_path)
                        .context(format!("Looking for {} in item {:?}", remaining_path, i))
                })
                .collect::<Result<Vec<_>>>()?
                .into_iter()
                .flatten()
                .next();
            Ok(rv)
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

fn find_attrs_in_impl(
    the_impl: &ItemImpl,
    remaining_path: &Option<RustPath>,
) -> Option<Vec<Attribute>> {
    remaining_path.as_ref().map_or_else(
        || Some(the_impl.attrs.clone()),
        |remaining_path| {
            if let (head, None) = remaining_path.head_tail() {
                the_impl
                    .items
                    .iter()
                    .flat_map(|item| match item {
                        syn::ImplItem::Const(c) if c.ident == head => vec![c.attrs.clone()],
                        syn::ImplItem::Fn(m) if m.sig.ident == head => vec![m.attrs.clone()],
                        syn::ImplItem::Type(t) if t.ident == head => vec![t.attrs.clone()],
                        _ => vec![],
                    })
                    .next()
            } else {
                // Impl items don't have subitems, so don't bother looking
                None
            }
        },
    )
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
        find_attrs_in_fields(&the_struct.fields, head)
    } else {
        Ok(Some(the_struct.attrs.clone()))
    }
}

fn find_attrs_in_enum(
    the_enum: &ItemEnum,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    remaining_path.as_ref().map_or_else(
        || Ok(Some(the_enum.attrs.clone())),
        |remaining_path| {
            let (head, tail) = remaining_path.head_tail();
            let rv = the_enum
                .variants
                .iter()
                .find(|v| v.ident == head)
                .map(|v| find_attrs_in_enum_variant(v, &tail));
            match rv {
                Some(Ok(Some(v))) => Ok(Some(v)),
                Some(Err(err)) => Err(err),
                Some(Ok(None)) | None => Ok(None),
            }
        },
    )
}

fn find_attrs_in_enum_variant(
    the_variant: &Variant,
    remaining_path: &Option<RustPath>,
) -> Result<Option<Vec<Attribute>>> {
    if let Some(remaining_path) = remaining_path {
        let (head, tail) = remaining_path.head_tail();
        ensure!(tail.is_none(), "Can't look deeper in enum variant fields");
        find_attrs_in_fields(&the_variant.fields, head)
    } else {
        Ok(Some(the_variant.attrs.clone()))
    }
}

fn find_attrs_in_fields(the_fields: &Fields, name: &str) -> Result<Option<Vec<Attribute>>> {
    let rv = match the_fields {
        Fields::Named(FieldsNamed { named, .. }) => named
            .iter()
            .find(|f| f.ident.as_ref().map(ToString::to_string) == Some(name.to_string()))
            .map(|field| field.attrs.clone()),

        Fields::Unnamed(FieldsUnnamed { unnamed, .. }) => {
            let index: usize = name.parse().map_err(|err| {
                anyhow!(
                    "Invalid field name for tuple {}, expected number: {}",
                    name,
                    err
                )
            })?;
            unnamed.iter().nth(index).map(|field| field.attrs.clone())
        }
        Fields::Unit => None,
    };
    Ok(rv)
}

fn attrs_to_string(attrs: &[Attribute]) -> String {
    attrs
        .iter()
        .filter(|attr| attr.path().get_ident().map(ToString::to_string) == Some("doc".to_string()))
        .filter_map(|attr| {
            attr.meta.require_name_value().ok().and_then(|nv| {
                match &nv.value {
                    syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(lit_str), .. }) => {
                        Some(lit_str.value().trim_start().to_string())
                    }
                    _ => None
                }
            })
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn type_has_name(the_type: &Type, name: &str) -> bool {
    match the_type {
        Type::Path(p) => p
            .path
            .segments
            .last()
            .is_some_and(|segment| segment.ident == name),
        Type::Reference(reference) => type_has_name(&reference.elem, name),
        _ => false,
    }
}

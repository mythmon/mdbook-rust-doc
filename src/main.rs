use std::{convert::TryFrom, io, process, str::FromStr};

use anyhow::{Context, Result};
use clap::Clap;
use lazy_static::lazy_static;
use mdbook::{
    book::Book,
    preprocess::{CmdPreprocessor, Preprocessor, PreprocessorContext},
    BookItem,
};
use mdbook_rust_doc::{find_doc_for_item, CrateRoots, RustPath};
use pulldown_cmark::Event;
use regex::{Captures, Regex};
use semver::{Version, VersionReq};
use serde::Deserialize;

#[derive(Clap, Debug)]
struct Opts {
    #[clap(subcommand)]
    cmd: Option<SubCommand>,
}

#[derive(Clap, Debug)]
enum SubCommand {
    Supports { renderer: String },
}

fn main() -> Result<()> {
    // let doc = find_doc_for_item(&opts.path, &crate_roots)
    //     .context("Finding documentation")?
    //     .ok_or_else(|| anyhow!("Item {} not found", opts.path))?;
    // println!(
    //     "{} doc:\n\n/// {}\n",
    //     opts.path,
    //     doc.replace("\n", "\n/// ")
    // );
    // Ok(())

    let opts = Opts::parse();
    // let crate_roots =
    //     CrateRoots::try_from(opts.crates.clone()).context("Converting crate roots")?;

    let preprocessor = RustDocPreprocessor;

    match opts.cmd {
        Some(SubCommand::Supports { renderer }) => handle_supports(&preprocessor, &renderer),
        None => handle_preprocessing(&preprocessor)?,
    }

    Ok(())
}

fn handle_supports(pre: &dyn Preprocessor, renderer: &str) -> ! {
    let supported = pre.supports_renderer(renderer);
    // let crate_roots =
    //     CrateRoots::try_from(opts.crates.clone()).context("Converting crate roots")?;
    // Signal whether the renderer is supported by exiting with 1 or 0.
    process::exit(if supported { 0 } else { 1 });
}

fn handle_preprocessing(pre: &dyn Preprocessor) -> Result<()> {
    let (ctx, book) = CmdPreprocessor::parse_input(io::stdin())?;

    let book_version = Version::parse(&ctx.mdbook_version)?;
    let version_req = VersionReq::parse(mdbook::MDBOOK_VERSION)?;

    if !version_req.matches(&book_version) {
        eprintln!(
            "Warning: The {} plugin was built against version {} of mdbook, \
             but we're being called from version {}",
            pre.name(),
            mdbook::MDBOOK_VERSION,
            ctx.mdbook_version
        );
    }

    let processed_book = pre.run(&ctx, book)?;
    serde_json::to_writer(io::stdout(), &processed_book)?;

    Ok(())
}

struct RustDocPreprocessor;

impl RustDocPreprocessor {
    fn process_item(crate_roots: &CrateRoots, item: &mut BookItem) -> Result<()> {
        if let BookItem::Chapter(chapter) = item {
            let mut new_content = String::with_capacity(chapter.content.len());

            let parser =
                pulldown_cmark::Parser::new_ext(&chapter.content, pulldown_cmark::Options::all());

            let modified_events = parser
                .map(|ev| match ev {
                    Event::Text(text) => {
                        lazy_static! {
                            static ref DIRECTIVE_REGEX: Regex =
                                Regex::new(r#"\{\{\s*#rustdoc\s+([\w:]+)\s*\}\}"#).unwrap();
                        }

                        let text = text.to_string();
                        let text = DIRECTIVE_REGEX.replace(&text, |captures: &Captures| {
                            let path_match = captures
                                .get(1)
                                .expect("Bug: capture group not in directive regex");
                            let item_path =
                                RustPath::from_str(path_match.as_str()).expect("invalid item path");
                            find_doc_for_item(&item_path, crate_roots)
                                .expect("Item not found")
                                .expect("Bug no doc returned")
                        });
                        Ok(Event::Text(text.to_string().into()))
                    }
                    ev => Ok(ev),
                })
                .collect::<Result<Vec<Event>>>()?
                .into_iter();
            pulldown_cmark_to_cmark::cmark(modified_events, &mut new_content, None)?;
            chapter.content = new_content;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
struct BookMeta {
    preprocessor: BookMetaPreprocessor,
}

#[derive(Debug, Clone, Deserialize)]
struct BookMetaPreprocessor {
    rustdoc: BookMetaPreprocessorRustDoc,
}

#[derive(Debug, Clone, Deserialize)]
struct BookMetaPreprocessorRustDoc {
    crates: Vec<String>,
}

impl mdbook::preprocess::Preprocessor for RustDocPreprocessor {
    fn name(&self) -> &str {
        "rust-doc"
    }

    fn run(&self, ctx: &PreprocessorContext, mut book: Book) -> Result<Book> {
        let book_meta_toml =
            std::fs::read_to_string(ctx.root.join("book.toml")).context("Opening book.toml")?;
        let book_meta: BookMeta = toml::from_str(&book_meta_toml).context("parsing book.toml")?;
        let crate_roots = CrateRoots::try_from(book_meta.preprocessor.rustdoc.crates)
            .context("Reading rustdoc crates config")?;

        book.for_each_mut(|item| Self::process_item(&crate_roots, item).unwrap());
        Ok(book)
    }
}

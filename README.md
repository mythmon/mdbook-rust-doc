# mdbook-rust-doc

Rust Doc is great for documenting code, but sometimes I found my self needing
more free-form documentation. The tool [mdBook][] works great for this, but you
have to give up the strong links between docs and source, and have to duplicate
documentation. This can be fine for some projects, but in other places it is a
recipe for stale docs and confusing code.

Inspired by the [Sphinx `autodoc`][autodoc] feature, this is an mdBook
preprocessor that can parse Rust documentation comments out of nearby crates and
embed them in the book output.

[autodoc]: https://www.sphinx-doc.org/en/master/usage/extensions/autodoc.html
[mdbook]: https://crates.io/crates/mdbook

## Usage

Given a source file like

`~/src/my-great-crate/src/some_mod.rs`

```rust
struct Crab {
  /// The number of legs this crab has. Probably 8, but there are some weird
  /// crabs out there!
  num_legs: u8,
}
```

You can configure mdBook by adding the following to your `book.toml`:

```toml
[preprocessor.rustdoc]
command = "path/to/mdbook-rust-doc"
crates = ["my_great_crate=~/src/my-great-crate"]
```

And then in your Markdown files for that book you can add a directive to include
the doc string.

```markdown
## Crab fields

- `num_legs` - {{ #rustdoc my_great_crate::some_mod::Crab::num_legs }}
```

This would be the same as if you had copied the doc string into the file, except
it will stay up to date as your code changes. It will also cause your docs to
fail to build if they become out of date relative to the structure of your
crate.

```markdown
## Crab fields

- `num_legs` - The number of legs this crab has. Probably 8, but there are some
  weird crabs out there!
```

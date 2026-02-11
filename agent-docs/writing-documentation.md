# Documentation

## Links
Use inline link syntax: [`MyStruct`] or [`MyStruct`](crate::path::to::MyStruct). Do not use reference-style links collected at the bottom of the doc comment.

## Examples
Hide setup/boilerplate lines in examples with `#` prefix so they compile but don't appear in rendered docs.

All doc examples must be testable. Run `cargo doc` and fix any warnings before finishing.

## README Sync
The first example in lib.rs must stay in sync with README.md (ignoring hidden `#` lines).

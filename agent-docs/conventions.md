# Coding Conventions

## Iterator and Collection Handling
- Prefer `.map_scan()` from `MapScanTrait` for stateful iterations over manual loops or `fold`.
- Use `.merge()` and `.merge_all()` from `Merge` for immutable vector extensions instead of `push` on mutable references.
- Leverage `pipe_trait` for fluent method chaining to promote composition over intermediate variables.

## Error Handling
- Prohibit `unwrap()` and `expect()` - use graceful error handling everywhere possible, propagating with `?` and structured types from `thiserror`.
- Do not use `anyhow` (despite its presence in dependencies due to `wit_parser`).
- Define custom error types using `thiserror` for structured error reporting. These should be specific to the operation you're making. If don't use global error types for the whole module. If a function can't produce some variant of the error type, then the error type should not be reused for that function. Either use a separate error or nesting if it makes sense.
- Use the helper `PartialSuccess` and `PartialResult` types from @src/utils/partial_success.rs when applicable

## Immutability Rule of Thumb
- Use `mut` only for performance-critical mutable borrows or when abstractions (e.g., `RwLock` for shared state) require it.
- Default to immutable bindings and ownership moves.

## Imports and Modules
- Group imports by external crates, then internal modules ordered by source crate, then super then submodules
- Avoid wildcard imports even when importing many related items except for preludes 
- Keep module declarations at the top of files
- Do not use mod.rs files; put module files one directory higher with the same name as the module
- prefer re-exporting imports in super rather than long import chains

## Naming Conventions
- do not ever use single letter names or non-standard shorthands

### Unsafe Code
- Minimize unsafe code usage; only use when instructed to
- Document safety invariants with comments


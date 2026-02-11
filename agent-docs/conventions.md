# Conventions

## Style
- Prefer iterators over explicit for-loops
- Prefer immutable bindings; use `mut` only when necessary for performance or API requirements

## Error Handling
- Use `thiserror` with operation-specific error types. Never use `anyhow`.
- No `unwrap()` or `expect()`. Propagate errors with `?`.
- `panic!` is allowed only when the invariant is documented. `unreachable!` requires review before use.

## Unsafe
- Generally forbidden. Ask for explicit permission before writing unsafe code.

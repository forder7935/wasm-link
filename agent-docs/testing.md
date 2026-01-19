# Testing

- Tests are in `./tests/`, with a single .rs file per test suite.
- Fixtures in WAT, WIT, and TOML formats.
- Test suites: dispatching_tests, resource_tests, error_handling_tests, cardinality_tests.
- Each suite has subdirectories for test scenarios, containing plugins and interfaces.
- Plugins include manifest.toml (metadata) and root.wat (WASM text code).
- Interfaces include manifest.toml and root.wit (interface definitions).
- Tests validate plugin loading, dispatching, error handling, and cardinality constraints.
- Run all with `cargo test --features test -- --nocapture`; specific suite with `--test <suite_name>`.
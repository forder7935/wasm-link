# Testing

This project uses integration tests (in `./tests/`) and doc tests.

Test fixtures use WAT, WIT, and TOML formats. Never hard-code fixture data directly in Rust test files - always use external fixture files loaded via the `fixtures!` macro.

Prefer integration tests over unit tests. Use unit tests only for small, pure logic that does not justify a full integration test and is unlikely to benefit from fixture-based coverage.

Run tests with `cargo test -- --nocapture` to see the output of new tests.

Doc examples must compile and pass. Use doc examples liberally as they serve as both documentation and tests.

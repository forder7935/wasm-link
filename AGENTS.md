# Agent Instructions

wasm-link is a runtime for loading and linking WebAssembly plugins. Plugins connect through abstract bindings, forming a DAG where each plugin can expose functionality to others and call into its dependencies.

## Essentials

- Commands: `nix develop --command <cmd>` or `nix-shell -p {package} --run` for single-use packages. Use these for all commands except standard utilities (ls, cat, grep, etc.).
- Always strive for concise, simple solutions
- If a problem can be solved in a simpler way, propose it
- If asked to do too much work at once, stop and state that clearly

## Plan Mode

- Make the plan extremely concise. Sacrifice grammar for the sake of concision.
- At the end of each plan, give me a list of unresolved questions to answer, if any.

## Build Mode

- ALWAYS run `cargo clippy` and `cargo test` after changes
- make sure to update documentation and the README.md once the api is established

## Detailed Guidelines

Make sure to read these files when performing related tasks:

- [Conventions](agent-docs/conventions.md) - read before writing or modifying any code
- [Testing](agent-docs/testing.md) - read before writing or modifying tests
- [Documentation](agent-docs/writing-documentation.md) - read before writing or modifying rustdoc comments or updating the README.md

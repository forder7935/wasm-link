# Agents Instructions

Omni is a platform for building fully modular applications based around WebAssembly plugins.

## Essentials
- Package manager: `nix develop`
- Non-standard commands: `cargo test --features test`, `clippy`
- Apply functional programming style: Avoid explicit for-loops; use iterators instead. Avoid `mut` unless it incurs a performance penalty; prefer mut abstractions from `@src/utils/`.
- Important: Never create commits. After completing a task (including adding and running tests), inform the user that the work is ready for review and manual commit.
- Always strive for concise, simple solutions
- If a problem can be solved in a simpler way, propose it
- If asked to do too much work at once, stop and state that clearly

## Plan Mode
- Make the plan extremely concise. Sacrifice grammar for the sake of concision.
- At the end of each plan, give me a list of unresolved questions to answer, if any.

## Build Mode
- Make sure to run clippy after changes
- Run tests after changes using `cargo test --features test`
- Execute all commands via `nix develop --command` or `nix run` to verify flake.nix completeness
- Never install dependencies except by modifying flake.nix

## Detailed Guidelines
- [Philosophy & Architecture](agent-docs/philosophy.md)
- [Coding Conventions](agent-docs/conventions.md)
- [Testing](agent-docs/testing.md)
- [Project Structure](agent-docs/structure.md)

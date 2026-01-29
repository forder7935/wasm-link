# Omni Desktop Host

## Overview

Wasm Compose is a framework for building fully modular applications based around [WebAssembly](https://webassembly.org/) plugins. These plugins are meant to be very simplistic building blocks that can be easily switched out, built by either the app developers or 3rd parties. These may link together in a tree-like structure defining their own interfaces. It offers fine-grained permission management only allowing each plugin to do the bare minimum it needs to function.

**Note:** This project is in early alpha. Expect breaking changes, incomplete features, and potential instability.

## Project Philosophy

- **Single tool, single task:** Apps should be broken up into small chunks that are meant to be composed together to create a whole.
- **Build around your workflow, not services:** Everything **You** use for a single task should be working together instead of you trying to duct-tape it together yourself.
- **The client belongs to the user:** Any part should be able to be easily added, removed or switched out for something else.
- **Zero-trust by default:** Don't just use something and expect it behaves, assume malice and constraint it to the minimum capabilities required.

## Quick Start

### Using Nix (Recommended)

If you have Nix installed, please use the provided `flake.nix` to set up everything you need:

```bash
nix develop
```

### Manual Setup

Running this project will require installing the following:

- [Rust toolchain](https://www.rust-lang.org/learn/get-started/)

## Technical Details

### Design Rationale

- **WebAssembly**: Easy language-agnostic low-overhead sandboxing.
- **WIT**: Standardized [IDL](https://en.wikipedia.org/wiki/Interface_description_language) designed for the WebAssembly Component Model.

### Plugin System

Plugins connect via abstract interfaces defined in the [WIT](https://github.com/WebAssembly/component-model/blob/main/design/mvp/WIT.md) format. These are not tied to any specific plugin, instead, each plugin defines a 'plug' pointing to an interface it implements, and optionally, a list of 'sockets', pointing to interfaces it expects to call into.

## Running Tests

For maintainability purposes, tests use TOML files for manifest declarations and WAT files for the plugin code where viable. To run tests:

```bash
cargo test
```

or when running specific suites use:

```bash
cargo test --test <suite>
```

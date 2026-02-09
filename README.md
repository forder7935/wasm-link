<div align="center">
  <h1><code>wasm-link</code></h1>

  <p>
    <strong>A
    <a href="https://webassembly.org/">WebAssembly</a>
    plugin runtime based around
    <a href="https://wasmtime.dev/">Wasmtime</a>
    <br>intended for building fully modular applications</strong>
  </p>

  <p>
    <a href="https://crates.io/crates/wasm-link"><img src="https://img.shields.io/crates/v/wasm-link" alt="crates.io version" /></a>
    <a href="https://docs.rs/wasm-link/latest/wasm_link"><img src="https://img.shields.io/docsrs/wasm-link" alt="documentation status" /></a>
    <img src="https://img.shields.io/badge/rustc-stable+-green.svg" alt="supported rustc stable" />
  </p>
</div>

## Highlights

- Composable applications with no interface limitations
- Performant language-agnostic plugin system

NOTE: Async types (`Future`, `Stream`, `ErrorContext`) are not yet supported for cross-plugin transfer and will return an error if encountered.

## Contents

- [Highlights](#highlights)
- [Contents](#contents)
- [Project Philosophy](#project-philosophy)
- [Quick Start](#quick-start)
- [Goals](#goals)
- [License](#license)
- [Contribution](#contribution)

## Project Philosophy

- **Single tool, single task:** Apps should be broken up into small chunks that are meant to be composed together to create a whole.
- **Build around your workflow, not services:** Everything **you** use for a single task should be working together instead of you trying to duct-tape it together yourself.
- **The client belongs to the user:** Any part should be able to be easily added, removed or switched out for something else.
- **Zero-trust by default:** Don't just use something and expect it behaves, assume malice and constrain it to the minimum capabilities required.

## Quick Start

```rust
use std::collections::{ HashMap, HashSet };
use wasm_link::{
    Binding, Interface, Function, ReturnKind,
    Plugin, PluginContext, Socket,
    Engine, Component, Linker, ResourceTable, Val,
};

// First, declare a plugin context, the data stored inside wasmtime `Store<T>`.
// It must contain a resource table to implement `PluginContext` which is needed
// for ownership tracking of wasm component model resources.
struct Context { resource_table: ResourceTable }

impl PluginContext for Context {
    fn resource_table( &mut self ) -> &mut ResourceTable {
        &mut self.resource_table
    }
}

// You create your own engine. This allows you to define your config but note that
// not all options are compatible. As a general rule of thumb, if an option changes
// the way you interact with wasm, it is likely not compatible since this is managed
// by `wasm_link` directly. If the option makes sense, it will likely be supported
// in the future through wasm_link options.
let engine = Engine::default();

// Similarly you may create your own linker, which you can add any exports into.
// Such exports will be available to all the plugins. It is your responsibility to
// make sure these don't conflict with re-exports of plugins that some other plugin
// depends on as these too have to be added to the same linker.
let linker = Linker::new( &engine );

// Build the DAG bottom-up: start with plugins that have no dependencies.
// Plugin IDs are specified in the Socket variant to prevent duplicate ids.
let leaf = Plugin::new(
    Component::new( &engine, "(component)" )?,
    Context { resource_table: ResourceTable::new() },
).instantiate( &engine, linker.clone())?;

// Bindings expose a plugin's exports to other plugins.
// Socket variant sets cardinality: ExactlyOne, AtMostOne (0-1), AtLeastOne (1+), Any (0+).
let leaf_binding = Binding::new(
    "empty:package",
    HashMap::new(),
    Socket::ExactlyOne( "leaf".to_string(), leaf ),
);

// `link()` wires up dependencies - this plugin can now import from leaf_binding.
let root = Plugin::new(
    Component::new( &engine, r#"(component
        (core module $m (func (export "f") (result i32) i32.const 42))
        (core instance $i (instantiate $m))
        (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
        (instance $inst (export "get-value" (func $f)))
        (export "my:package/example" (instance $inst))
    )"# )?,
    Context { resource_table: ResourceTable::new() },
).link( &engine, linker, vec![ leaf_binding ])?;

// Interface tells `wasm_link` which functions exist and how to handle returns.
let root_binding = Binding::new(
    "my:package",
    HashMap::from([( "example".to_string(), Interface::new(
        HashMap::from([
            ( "get-value".into(), Function::new( ReturnKind::MayContainResources, false ))
        ]),
        HashSet::new(),
    ))]),
    Socket::ExactlyOne( "root".to_string(), root ),
);

// Now you can call into the plugin graph from the host.
let result = root_binding.dispatch( "example", "get-value", &[ /* args */ ] )?;
match result {
    Socket::ExactlyOne( _, Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
    Socket::ExactlyOne( _, Err( err )) => panic!( "dispatch error: {}", err ),
    _ => unreachable!(),
}
```

## Goals

- ✅ Basic plugin linking
- ✅ Component model support
- ✅ Resource support
- 🚧 Epoch interrupt and fuel
- ⬛ Async, streams and threads

Further goals are yet to be determined.

## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contribution

Contributions are welcome. For major changes, please open an issue first for discussion.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.

### Development Setup

#### Using Nix (Recommended)

If you have Nix installed, please use the provided `flake.nix` to set up everything you need:

```bash
nix develop
```

#### Manual Setup

You should most likely be fine with just the [Rust toolchain](https://www.rust-lang.org/learn/get-started/)

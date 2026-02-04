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

- composable applications with no interface limitations
- performant language-agnostic plugin system

NOTE: Async types (`Future`, `Stream`, `ErrorContext`) are not yet supported for cross-plugin transfer and will return an error if encountered.

## Contents

- [Highlights](#highlights)
- [Contents](#contents)
- [Project Philosophy](#project-philosophy)
- [Example](#example)
- [Goals](#goals)
- [License](#license)
- [Contribution](#contribution)

## Project Philosophy

- **Single tool, single task:** Apps should be broken up into small chunks that are meant to be composed together to create a whole.
- **Build around your workflow, not services:** Everything **You** use for a single task should be working together instead of you trying to duct-tape it together yourself.
- **The client belongs to the user:** Any part should be able to be easily added, removed or switched out for something else.
- **Zero-trust by default:** Don't just use something and expect it behaves, assume malice and constraint it to the minimum capabilities required.

## Example

```rs
use wasm_link::{
    Binding, Interface, Function, Cardinality, ReturnKind,
    Plugin, PluginContext, PluginTree, Socket,
    Engine, Component, Linker, ResourceTable, Val,
};

// Define a context that implements PluginContext
struct Context {
    resource_table: ResourceTable,
}

impl PluginContext for Context {
    fn resource_table( &mut self ) -> &mut ResourceTable {
        &mut self.resource_table
    }
}

let engine = Engine::default();

// Start by defining your root binding that will be used to interface with the plugin tree
const ROOT_BINDING: &str = "root" ;
const EXAMPLE_INTERFACE: &str = "example" ;
const GET_VALUE: &str = "get-value" ;

let binding = Binding::new(
    ROOT_BINDING,
    Cardinality::ExactlyOne,
    "my:package",
    vec![ Interface::new(
        EXAMPLE_INTERFACE,
        vec![ Function::new( GET_VALUE, ReturnKind::MayContainResources, false ) ],
        Vec::<String>::with_capacity( 0 ),
    )],
);

// Now create a plugin that implements this binding
let plugin = Plugin::new(
    "foo",
    ROOT_BINDING,
    Vec::with_capacity( 0 ),
    Component::new( &engine, r#"(component
        (core module $m (func (export "f") (result i32) i32.const 42))
        (core instance $i (instantiate $m))
        (func $f (export "get-value") (result u32) (canon lift (core func $i "f")))
        (instance $inst (export "get-value" (func $f)))
        (export "my:package/example" (instance $inst))
    )"# )?,
    Context { resource_table: ResourceTable::new() },
);

// First you need to tell `wasm_link` about your plugins, bindings and where you want
// the execution to begin. `wasm_link` will try it's best to load in all the plugins,
// upon encountering an error, it will try to salvage as much of the remaining data
// as possible returning a list of failures alongside the `PluginTree`.
let ( tree, init_errors ) = PluginTree::new( ROOT_BINDING, vec![ binding ], vec![ plugin ] );
assert!( init_errors.is_empty() );

// Once you've got your `PluginTree` constructed, you can link the plugins together
// Since some plugins may fail to load, it is only at this point that the cardinality
// requirements are validated depending on the plugins that managed to get loaded,
// otherwise it tries to salvage as much of the tree as can be loaded returning a list
// of failures alongside the loaded `PluginTreeHead` - the root node of the `PluginTree`.
let linker = Linker::new( &engine );
let ( tree_head, load_errors ) = tree.load( &engine, &linker ).map_err(|( e, _ )| e )?;
assert!( load_errors.is_empty() );

// Dispatch a function call to plugins implementing the root binding
let result = tree_head.dispatch( EXAMPLE_INTERFACE, GET_VALUE, true, &[] );
match result {
    Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
    Socket::ExactlyOne( Err( err )) => panic!( "dispatch error: {}", err ),
    _ => panic!( "unexpected cardinality" ),
}
```

## Goals

- [x] basic plugin linking
- [x] component model support
- [x] resource support
- [ ] async, streams and threads

Further goals are yet to be determined

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

### Quick Start

#### Using Nix (Recommended)

If you have Nix installed, please use the provided `flake.nix` to set up everything you need:

```bash
nix develop
```

#### Manual Setup

Running this project will only require installing the [Rust toolchain](https://www.rust-lang.org/learn/get-started/)

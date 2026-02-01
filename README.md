<div align="center">
  <h1><code>wasm_link</code></h1>

  <p>
    <strong>A
    <a href="https://webassembly.org/">WebAssembly</a>
    plugin runtime based around
    <a href="https://wasmtime.dev/">Wasmtime</a>
    <br>intended for building fully modular applications</strong>
  </p>

  <p>
    <a href="https://docs.rs/wasm-link/latest/wasm_link"><img src="https://img.shields.io/badge/docs-passing-emerald" alt="documentation status" /></a>
    <img src="https://img.shields.io/badge/status-early_alpha-orange.svg" alt="project status" />
    <img src="https://img.shields.io/badge/rustc-stable+-green.svg" alt="supported rustc stable" />
  </p>
</div>

## Highlights

- composable applications with no interface limitations
- performant language-agnostic plugin system

IMPORTANT: Future, Stream and ErrorContext are not yet supported, as of now using them will lead to a panic

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
    InterfaceData, InterfaceCardinality, FunctionData, ReturnKind,
    PluginCtxView, PluginData, PluginTree, Socket,
    Engine, Component, Linker, ResourceTable, Val,
};

// Declare your fixture sources
#[derive( Clone )]
struct Func { name: String, return_kind: ReturnKind }
impl FunctionData for Func {
    fn name( &self ) -> &str { self.name.as_str() }
    fn return_kind( &self ) -> ReturnKind { self.return_kind.clone() }
    // Determine whether a function is a resource method
    // a constructor is not considered to be a method
    fn is_method( &self ) -> bool { false }
}

struct Interface { id: &'static str, funcs: Vec<Func> }
impl InterfaceData for Interface {
    type Id = &'static str ;
    type Error = std::convert::Infallible ;
    type Function = Func ;
    type FunctionIter<'a> = std::slice::Iter<'a, Func> ;
    type ResourceIter<'a> = std::iter::Empty<&'a String> ;
    fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
    fn cardinality( &self ) -> Result<&InterfaceCardinality, Self::Error> { Ok( &InterfaceCardinality::ExactlyOne ) }
    fn package_name( &self ) -> Result<&str, Self::Error> { Ok( "my:package/example" ) }
    fn functions( &self ) -> Result<Self::FunctionIter<'_>, Self::Error> { Ok( self.funcs.iter()) }
    fn resources( &self ) -> Result<Self::ResourceIter<'_>, Self::Error> { Ok( std::iter::empty()) }
}

struct Plugin { id: &'static str, plug: &'static str, resource_table: ResourceTable }
impl PluginCtxView for Plugin {
    fn resource_table( &mut self ) -> &mut ResourceTable { &mut self.resource_table }
}
impl PluginData for Plugin {
    type Id = &'static str ;
    type InterfaceId = &'static str ;
    type Error = std::convert::Infallible ;
    type SocketIter<'a> = std::iter::Empty<&'a Self::InterfaceId> ;
    fn id( &self ) -> Result<&Self::Id, Self::Error> { Ok( &self.id ) }
    fn plug( &self ) -> Result<&Self::InterfaceId, Self::Error> { Ok( &self.plug ) }
    fn sockets( &self ) -> Result<Self::SocketIter<'_>, Self::Error> { Ok( std::iter::empty()) }
    fn component( &self, engine: &Engine ) -> Result<Component, Self::Error> {
        /* inialise your component here */
    }
}

// Now construct some plugins and related data
let root_interface_id = "root" ;
let plugins = [ Plugin { id: "foo", plug: root_interface_id, resource_table: ResourceTable::new() }];
let interfaces = [ Interface { id: root_interface_id, funcs: vec![
    Func { name: "get-value".to_string(), return_kind: ReturnKind::MayContainResources }
]}];

// First you need to tell wasm_link about your plugins, interfaces and where you want
// the execution to begin. wasm_link will try it's best to load in all the plugins,
// upon encountering an error, it will try to salvage as much of the remaining data
// as possible returning a list of failures alongside the `PluginTree`.
let ( tree, build_errors ) = PluginTree::new( root_interface_id, interfaces, plugins );
assert!( build_errors.is_empty() );

// Once you've got your `PluginTree` constructed, you can link the plugins together
// Since some plugins may fail to load, it is only at this point that the cardinality
// requirements are satisfied by the plugins that managed to get loaded, otherwise it
// tries to salvage as much of the tree as can be loaded returning a list of failures
// alongside the loaded `PluginTreeHead` - the root node of the `PluginTree`.
let engine = Engine::default();
let linker = Linker::new( &engine );
let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
assert!( load_errors.is_empty() );

// Now you can dispatch any function on the root interface.
// This will dispatch the function for all plugins plugged in to the root socket returning
// a Result for each in the shape determined by the interface cardinality.
let result = tree_head.dispatch( "my:package/example", "get-value", true, &[] );
match result {
    Socket::ExactlyOne( Ok( Val::U32( n ))) => assert_eq!( n, 42 ),
    Socket::ExactlyOne( Err( e )) => panic!( "dispatch error: {e}" ),
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

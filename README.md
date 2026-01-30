# Wasm Compose

## Overview

Wasm Compose is a framework for building fully modular applications based around [WebAssembly](https://webassembly.org/) plugins. These plugins are meant to be very simplistic building blocks that can be easily switched out, built by either the app developers or 3rd parties. These may link together in a tree-like structure defining their own interfaces.

**Note:** Documentation is not yet available online, you have to generate it yourself using `cargo doc`

**Note:** This project is in early alpha. Expect breaking changes, incomplete features, and potential instability.

## Highlights

- composable applications with no interface limitations
- performant language-agnostic plugin system

## Contents

- [Overview](#overview)
- [Highlights](#highlights)
- [Contents](#contents)
- [Project Philosophy](#project-philosophy)
- [Usage](#usage)
- [Goals](#goals)
- [Contribution](#contribution)
- [Technical Details](#technical-details)
- [License](#license)

## Project Philosophy

- **Single tool, single task:** Apps should be broken up into small chunks that are meant to be composed together to create a whole.
- **Build around your workflow, not services:** Everything **You** use for a single task should be working together instead of you trying to duct-tape it together yourself.
- **The client belongs to the user:** Any part should be able to be easily added, removed or switched out for something else.
- **Zero-trust by default:** Don't just use something and expect it behaves, assume malice and constraint it to the minimum capabilities required.

## Usage

```rs
// Declare your sources of plugins and interfaces
#[derive( Clone )]
struct Func { name: String, return_kind: ReturnKind }
impl FunctionData for Func { /* accessors to basic function signature info */ }

struct Interface { id: InterfaceId, funcs: Vec<Func> }
impl InterfaceData for Interface { /* accessors to interface data required for linking */ }

struct Plugin { id: PluginId, plug: InterfaceId }
impl PluginData for Plugin { /* accessors to plugin metadata and the root executable */ }

// Create your fixtures
let root_interface_id = InterfaceId::new( 0 );
let plugins = [ Plugin { id: PluginId::new( 1 ), plug: root_interface_id }];
let interfaces = [ Interface { id: root_interface_id, funcs: vec![
    Func { name: "get-value".to_string(), return_kind: ReturnKind::MayContainResources }
]}];

// Initialise the plugin tree
let ( tree, build_errors ) = PluginTree::new( root_interface_id, interfaces, plugins );
assert!( build_errors.is_empty() );

// Load the plugins and perform linking
let engine = Engine::default();
let linker = Linker::new( &engine );
let ( tree_head, load_errors ) = tree.load( &engine, &linker ).unwrap();
assert!( load_errors.is_empty() );

// Dispatch any functions of the root interface
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

As of now, running this project will only require installing the [Rust toolchain](https://www.rust-lang.org/learn/get-started/)

## Technical Details

### Design Rationale

- **WebAssembly**: Easy language-agnostic low-overhead sandboxing.
- **WIT**: Standardized [IDL](https://en.wikipedia.org/wiki/Interface_description_language) designed for the WebAssembly Component Model.

### Plugin System

Plugins connect via abstract interfaces declaring a list of items the implementer is expected to export which the consumer may import. These are not tied to any specific plugin, instead, each plugin defines a 'plug' pointing to an interface it implements, and optionally, a list of 'sockets', pointing to interfaces it may import. Interfaces are allowed to declare any imports/exports supported by the wit format.

## License

This project is licensed under the Apache 2.0 License - see the [LICENSE](LICENSE) file for details.

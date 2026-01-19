# Philosophy & Architecture

## Project Philosophy and Overview

Omni is a platform for building fully modular applications based around WebAssembly plugins. These plugins are meant to be very simplistic building blocks that can be easily switched out, built by either the app developers or 3rd parties. These may link together in a tree-like structure defining their own interfaces. It offers fine-grained permission management only allowing each plugin to do the bare minimum it needs to function.

- **Single tool, single task:** Apps should be broken up into small chunks that are meant to be composed together to create a whole.
- **Build around your workflow, not services:** Everything **You** use for a single task should be working together instead of you trying to duct-tape it together yourself.
- **The client belongs to the user:** Any part should be able to be easily added, removed or switched out for something else.
- **Zero-trust by default:** Don't just use something and expect it behaves, assume malice and constraint it to the minimum capabilities required.

## Technical Architecture

### Design Rationale
- **WebAssembly**: Easy language-agnostic low-overhead sandboxing.
- **WIT**: Standardized IDL designed for the WebAssembly Component Model.
- **Cap'n Proto**: Provides efficient, zero-copy serialization ideal for network transmission and storing of plugin manifests.

### Plugin System
Plugins connect via abstract interfaces defined in the WIT format. These are not tied to any specific plugin, instead, each plugin defines a 'plug' pointing to an interface it implements, and optionally, a list of 'sockets', pointing to interfaces it expects to call into.

### Host interfaces
Host interfaces have their respective WIT declarations located at 'wit/'.
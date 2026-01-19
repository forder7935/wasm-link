# Project Structure

- **src/**:
    - initialisation/: Handles plugin discovery, loading, and tree construction.
        - discovery/: Discovers plugins and interfaces from filesystem, parses manifests and WIT files.
        - loading/: Loads plugins into wasmtime Engine, creates PluginTree with contexts and sockets.
        - types/: Defines IDs and types for plugins, interfaces, and cardinalities.
    - exports/: Host-defined exports that may be used by plugins
    - utils/: Functional utilities like MapScanTrait, Merge, and warning helpers (temporary until proper logging).
    - capnp.rs: Linking of capnp codegen
- **tests/**: Integration tests with TOML/WAT/WIT fixtures.
- **appdata/**: Do not use. Single-use fixtures for new feature testing
- **capnp/**: Schema definitions for manifests.
- **wit/**: Schema definitions for root socket and host exports

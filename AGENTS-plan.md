# Implementation Plan: Component Model Native Async Support

**Branch**: `feature/component-model-async`  
**Status**: Planning  
**Goal**: Add Component Model async support to enable plugins with async functions, WITHOUT forcing all functions to be async.

---

## Executive Summary

Enable Component Model's native async (`future<T>`, `stream<T>`, async WIT functions) to support:
- Plugins can export async WIT functions
- Plugins can import async WIT functions from each other (via host rewrap)
- Async functions are opt-in per-function, not all-or-nothing

**Critical Distinction**:
- ✅ `wasm_component_model_async(true)` - Only WIT `async` functions are async
- ❌ `async_support(true)` - Makes ALL wasmtime APIs async (don't want this)

**Explicit non-goals**:
- ❌ NOT adding WASI P3 interfaces (defer until permissions system ready)
- ❌ NOT making all functions async (defeats the purpose)
- ❌ NOT requiring wasmtime `async_support` if avoidable

---

## Understanding: Component Model Async vs Wasmtime Async

### Component Model Async (`wasm_component_model_async`)

**Config flag**: `config.wasm_component_model_async(true)`

**What it enables**:
- WIT `async foo() -> u32` functions (opt-in per function)
- `future<T>` and `stream<T>` types
- Automatic suspension at async points
- Canonical ABI async lifting/lowering

**Key feature**: Sync functions stay sync! Only WIT functions marked `async` use async ABI.

**Example WIT**:
```wit
interface example {
    // Sync function - uses sync ABI
    get-config: func() -> string;
    
    // Async function - uses async ABI
    async fetch-data: func(url: string) -> list<u8>;
}
```

**Canonical ABI Behavior**:

For sync function `get-config`:
```rust
// Core signature: (param i32) (result i32)
// Works exactly as before
let result = func.call(&mut store, &args, &mut results)?;
```

For async function `fetch-data`:
```rust
// Core signature: (param i32 i32) (result i32)
// Returns: 
//   0 = completed synchronously
//   nonzero = task index + state (starting/started)
let task_or_result = func.call(&mut store, &args, &mut results)?;
if task_or_result == 0 {
    // Completed immediately
} else {
    // Async task created, need to wait on it
}
```

**Critical insight**: Component Model async works at the Canonical ABI level. The host doesn't need to be async Rust - it just needs to handle the async protocol.

---

### Wasmtime Async Support (`async_support`)

**Config flag**: `config.async_support(true)`

**What it enables**:
- Host Rust functions can be `async fn`
- ALL wasmtime APIs become async: `call_async()`, `instantiate_async()`, etc.
- Fiber-based stack switching

**When you need it**:
- Host functions do I/O that should yield
- Want to avoid blocking host thread

**When you DON'T need it**:
- Host is okay blocking during async operations
- Can use thread pool for concurrency

**Problem for your use case**: Makes EVERYTHING async, including sync functions.

---

## The Core Problem: Your Rewrapping Architecture

**Current code** (`preload_plugin.rs:103`):
```rust
linker_instance.func_new(function.name(), move |ctx, _ty, args, results| Ok(
    results[0] = $dispatch(&socket_arc_clone, ctx, &interface_ident_clone, &function_clone, args)
))
```

**This uses `func_new()` which**:
- Is synchronous only
- Cannot await async operations
- Will block if calling an async function

**For async functions, you have three options**:

### Option 1: Blocking Approach (Simplest)
```rust
// Keep using func_new(), but it blocks
linker_instance.func_new(function.name(), move |ctx, _ty, args, results| {
    if function.is_async() {
        // Blocks until complete
        results[0] = dispatch_async_blocking(ctx, args);
    } else {
        results[0] = dispatch_sync(ctx, args);
    }
    Ok(())
})
```

**Pros**: Simple, no wasmtime async needed  
**Cons**: Blocks host thread during async operations  
**When acceptable**: If async operations are rare or fast

### Option 2: Async Rewrapping (Most Flexible)
```rust
if function.is_async() {
    linker_instance.func_new_async(function.name(), 
        move |ctx, _ty, args, results| Box::new(async move {
            results[0] = dispatch_async(ctx, args).await;
            Ok(())
        })
    )?;
} else {
    linker_instance.func_new(function.name(), 
        move |ctx, _ty, args, results| {
            results[0] = dispatch_sync(ctx, args);
            Ok(())
        }
    )?;
}
```

**Pros**: True async, non-blocking  
**Cons**: Requires `async_support(true)`, adds complexity  
**When needed**: If blocking is unacceptable

### Option 3: Native Composition (Most Elegant)
```bash
# Let Component Model handle linking
wasm-tools compose plugin-a.wasm -d plugin-b.wasm -o composed.wasm
```

**Pros**: Async preserved automatically, no host involvement  
**Cons**: Loses your cardinality logic (AtMostOne, Any, etc.)  
**When viable**: If cardinality can be pre-determined

---

## Current Architecture Analysis

### Engine Configuration
**File**: `src/initialisation.rs:37`
```rust
let engine = Engine::default();
```
- No Component Model async enabled
- No async_support enabled

### Plugin Rewrapping
**File**: `src/initialisation/loading/preload_plugin.rs:103`
```rust
linker_instance.func_new(function.name(), move |ctx, _ty, args, results| Ok(
    results[0] = $dispatch(&socket_arc_clone, ctx, &interface_ident_clone, &function_clone, args)
))
```
- Uses `func_new()` - synchronous only
- Single path for all functions (no async/sync distinction)

### Dispatch
**File**: `src/initialisation/loading/dispatch.rs:170-202`
```rust
fn dispatch(&mut self, ...) -> Result<Val, DispatchError> {
    func.call(&mut self.store, data, &mut buffer)
        .map_err(DispatchError::RuntimeException)?;
    func.post_return(&mut self.store);
    // ...
}
```
- Fully synchronous
- No handling of async protocol

### Async Rejection
**File**: `src/initialisation/discovery/raw_interface_data.rs:41`
```rust
FunctionKind::AsyncFreestanding | FunctionKind::AsyncMethod(_) | FunctionKind::AsyncStatic(_)
    => unimplemented!("Async functions are not yet implemented"),
```

**File**: `src/initialisation/loading/dispatch.rs:133`
```rust
Val::Future(_) => unimplemented!("'Val::Future' is not yet supported"),
```

---

## Implementation Approaches

### Approach A: Minimal - Blocking Async (Recommended Start)

**Goal**: Enable Component Model async with minimal changes, accepting blocking behavior.

**Effort**: 2-3 days

**Changes needed**:
1. Enable `wasm_component_model_async` in Engine config
2. Track which functions are async (metadata)
3. Handle async protocol in dispatch (block until complete)
4. Remove `unimplemented!()` rejections

**Pros**:
- ✅ Minimal code changes
- ✅ No wasmtime `async_support` needed
- ✅ Works with existing sync architecture
- ✅ Can load async components

**Cons**:
- ❌ Blocks host thread during async operations
- ❌ No true concurrency
- ❌ May be slow if async operations are common

**When acceptable**:
- Testing/prototyping
- Async operations are fast (<100ms)
- Low concurrency requirements

---

### Approach B: Hybrid - Async Rewrapping

**Goal**: Non-blocking async, but only for async functions.

**Effort**: 5-7 days

**Changes needed**:
1. Enable both `wasm_component_model_async` AND `async_support`
2. Dual rewrapping paths: `func_new()` for sync, `func_new_async()` for async
3. Async dispatch variants
4. Tokio runtime integration

**Pros**:
- ✅ Non-blocking async
- ✅ True concurrency
- ✅ Sync functions stay sync (no .await needed)
- ✅ Scales to many concurrent async operations

**Cons**:
- ❌ More complexity (two code paths)
- ❌ Requires tokio runtime
- ❌ Potential deadlock issues with RwLock

**When needed**:
- Async operations are slow (>100ms)
- High concurrency requirements
- Can't afford to block host

---

### Approach C: Native Composition

**Goal**: Use Component Model's native composition instead of host rewrapping.

**Effort**: Unknown (depends on cardinality requirements)

**Changes needed**:
1. Pre-compose plugins at build/load time
2. Rethink cardinality at composition level
3. May need custom composition tooling

**Pros**:
- ✅ Async automatic
- ✅ Best performance
- ✅ Less host code

**Cons**:
- ❌ Loses dynamic cardinality
- ❌ May not support your use case
- ❌ Large architectural change

**When viable**:
- If cardinality can be determined statically
- If you can change linking model

---

## Recommended Path: Staged Implementation

### Stage 1: Minimal Blocking (Start Here)

**Duration**: 2-3 days  
**Goal**: Validate Component Model async works, accept blocking

**Steps**:
1. Enable `wasm_component_model_async` in Engine
2. Track async functions in metadata
3. Update dispatch to handle async protocol (blocking)
4. Test with simple async component

**Success criteria**:
- ✅ Async components load
- ✅ Async functions callable (even if blocking)
- ✅ No regressions for sync functions

**Decision point**: Is blocking acceptable? If yes, stop here. If no, continue to Stage 2.

---

### Stage 2: Async Rewrapping (If Needed)

**Duration**: 5-7 days  
**Goal**: Non-blocking async

**Steps**:
1. Enable `async_support` in Engine
2. Implement dual rewrapping (sync vs async)
3. Add async dispatch variants
4. Integrate tokio runtime
5. Handle concurrent execution

**Success criteria**:
- ✅ Async operations don't block
- ✅ Multiple concurrent async calls work
- ✅ No deadlocks

---

## Stage 1 Implementation Details

### 1.1: Enable Component Model Async

**File**: `src/initialisation.rs`

**Add function before line 37**:
```rust
fn create_engine() -> Result<Engine, UnrecoverableStartupError> {
    let mut config = Config::new();
    
    // Enable Component Model (already implicit)
    config.wasm_component_model(true);
    
    // Enable Component Model async - THIS IS KEY
    config.wasm_component_model_async(true);
    
    // Enable future<T> and stream<T> builtins
    config.wasm_component_model_async_builtins(true);
    
    // Optional: Enable epoch interruption for timeout/cancellation
    config.epoch_interruption(true);
    
    // NOT enabling async_support - keep host sync
    
    Engine::new(&config)
        .map_err(UnrecoverableStartupError::EngineConfigError)
}

pub fn initialise_plugin_tree(
    source: &Path,
    root_interface_id: &InterfaceId
) -> PartialResult<PluginTree, UnrecoverableStartupError, RecoverableStartupError> {
    let (socket_map, discovery_errors) = discover_all(source, root_interface_id)
        .map_err(|err| (err.into(), vec![]))?;

    let engine = create_engine()
        .map_err(|e| (e, vec![]))?;

    let (linker, linker_errors) = crate::exports::exports(&engine);
    
    // ... rest unchanged
}
```

**Add error variant**:
```rust
#[derive(Error, Debug)]
pub enum UnrecoverableStartupError {
    #[error("Discovery Failure: {0}")] 
    DiscoveryError(#[from] DiscoveryFailure),
    
    #[error("Plugin Error: {0}")] 
    PreloadError(#[from] PreloadError),
    
    #[error("Engine Configuration Error: {0}")]
    EngineConfigError(#[source] wasmtime::Error),
}
```

---

### 1.2: Track Async Functions

**File**: `src/initialisation/discovery/raw_interface_data.rs`

**Replace unimplemented (line 41-43)**:
```rust
pub fn is_method(&self) -> bool { 
    match self.function.kind {
        // Async functions ARE still methods/freefunctions
        FunctionKind::Freestanding | FunctionKind::AsyncFreestanding => false,
        FunctionKind::Method(_) | FunctionKind::AsyncMethod(_) => true,
        FunctionKind::Static(_) | FunctionKind::AsyncStatic(_) => false,
        FunctionKind::Constructor(_) => false,
    }
}

pub fn is_async(&self) -> bool {
    matches!(
        self.function.kind,
        FunctionKind::AsyncFreestanding | 
        FunctionKind::AsyncMethod(_) | 
        FunctionKind::AsyncStatic(_)
    )
}
```

**Update FunctionData** (find the struct and add field):
```rust
pub struct FunctionData {
    // ... existing fields ...
    is_async: bool,
}

impl FunctionData {
    pub fn new(/* existing params */) -> Self {
        let is_async = raw_function.is_async();
        Self {
            // ... existing fields ...
            is_async,
        }
    }
    
    #[inline]
    pub fn is_async(&self) -> bool {
        self.is_async
    }
}
```

---

### 1.3: Handle Async Protocol in Dispatch

**File**: `src/initialisation/loading/dispatch.rs`

**Update dispatch function (around line 170)**:
```rust
fn dispatch(
    &mut self,
    interface_path: &str,
    function: &str,
    returns: bool,
    data: &[Val],
) -> Result<Val, DispatchError> {
    let mut buffer = match returns {
        true => vec![Self::PLACEHOLDER_VAL],
        false => Vec::with_capacity(0),
    };

    let interface_index = self.instance
        .get_export_index(&mut self.store, None, interface_path)
        .ok_or(DispatchError::InvalidInterface(interface_path.to_string()))?;
    
    let func_index = self.instance
        .get_export_index(&mut self.store, Some(&interface_index), function)
        .ok_or(DispatchError::InvalidFunction(format!("{}:{}", interface_path, function)))?;
    
    let func = self.instance
        .get_func(&mut self.store, func_index)
        .ok_or(DispatchError::InvalidFunction(format!("{}:{}", interface_path, function)))?;
    
    // Call function - may return async task index
    let call_result = func.call(&mut self.store, data, &mut buffer)
        .map_err(DispatchError::RuntimeException)?;
    
    // Check if this was an async function that returned a task
    // In Component Model async, async functions return an i32:
    //   0 = completed synchronously
    //   nonzero = (task_index << 4) | state
    if returns && buffer[0].unwrap_i32() != Some(0) {
        // This is an async task - need to wait for it
        // For Stage 1, we block until completion
        self.wait_for_async_completion(&func, &mut buffer)?;
    }
    
    func.post_return(&mut self.store)
        .ok(); // Ignore post_return errors

    Ok(match returns {
        true => buffer.pop().ok_or(DispatchError::MissingResponse)?,
        false => Self::PLACEHOLDER_VAL,
    })
}

// NEW: Helper to wait for async task completion (blocking)
fn wait_for_async_completion(
    &mut self,
    func: &wasmtime::component::Func,
    buffer: &mut [Val],
) -> Result<(), DispatchError> {
    // Extract task index from result
    let task_result = buffer[0].unwrap_i32()
        .ok_or(DispatchError::InvalidAsyncResult)?;
    
    if task_result == 0 {
        return Ok(()); // Already complete
    }
    
    let task_index = (task_result >> 4) as u32;
    let state = task_result & 0xF;
    
    // For Stage 1: Poll in a loop until complete
    // This blocks, but works without async_support
    loop {
        // Check task status via wasmtime API
        // Note: This is pseudo-code - actual wasmtime API may differ
        match self.store.poll_async_task(task_index) {
            AsyncTaskStatus::Complete => {
                // Task finished, result should be in buffer
                return Ok(());
            }
            AsyncTaskStatus::Pending => {
                // Still running, yield and retry
                std::thread::sleep(std::time::Duration::from_micros(10));
                continue;
            }
            AsyncTaskStatus::Error(e) => {
                return Err(DispatchError::AsyncTaskFailed(e));
            }
        }
    }
}
```

**Add error variants**:
```rust
#[derive(Error, Debug)]
pub enum DispatchError {
    // ... existing variants ...
    
    #[error("Invalid async result format")]
    InvalidAsyncResult,
    
    #[error("Async task failed: {0}")]
    AsyncTaskFailed(String),
}
```

**Important note**: The actual wasmtime API for polling async tasks may differ. Need to research `wasmtime::component` async task APIs.

---

### 1.4: Handle Val::Future

**File**: `src/initialisation/loading/dispatch.rs` (line 133)

**Replace unimplemented**:
```rust
Val::Future(f) => {
    // In Stage 1, we don't expose futures to Rust side
    // They should be handled by async dispatch protocol
    // If we see one here, something went wrong
    eprintln!("Warning: Received Val::Future({}) - should have been handled by async protocol", f);
    Ok(Val::Future(f))
}
```

---

### 1.5: Update Dependencies

**File**: `Cargo.toml`

```toml
[dependencies]
wasmtime = { version = "40.0", features = ["component-model-async"] }
# Not adding "async" feature yet - that's for Stage 2
```

---

### 1.6: Create Test Component

**Create test directory**: `tests/fixtures/async_component/`

**WIT** (`async_component.wit`):
```wit
package test:async-component@0.1.0;

interface example {
    // Sync function for comparison
    sync-hello: func() -> string;
    
    // Simple async function
    async async-hello: func() -> string;
    
    // Async function that might suspend
    async fetch-mock: func(url: string) -> list<u8>;
}

world test-world {
    export example;
}
```

**Rust implementation** (`src/lib.rs`):
```rust
wit_bindgen::generate!({
    world: "test-world",
    exports: {
        "test:async-component/example": Component,
    },
});

struct Component;

impl exports::test::async_component::example::Guest for Component {
    fn sync_hello() -> String {
        "Hello from sync!".to_string()
    }
    
    async fn async_hello() -> String {
        "Hello from async!".to_string()
    }
    
    async fn fetch_mock(url: String) -> Vec<u8> {
        // Mock async work
        format!("Data from {}", url).into_bytes()
    }
}
```

**Build**:
```bash
cargo component build --release
```

---

### 1.7: Integration Test

**File**: `tests/component_model_async.rs`

```rust
use desktop_host::initialisation;
use std::path::PathBuf;

#[test]
fn test_async_component_loads() {
    let engine = initialisation::create_engine().unwrap();
    let component_path = PathBuf::from("tests/fixtures/async_component/target/wasm32-wasi/release/async_component.wasm");
    
    // Should not panic
    let component = wasmtime::component::Component::from_file(&engine, &component_path).unwrap();
    
    // Basic validation
    assert!(component.exports(&engine).any(|e| e.name() == "example"));
}

#[test]
fn test_sync_function_still_works() {
    // Verify sync functions aren't broken
    let tree = initialisation::initialise_plugin_tree(
        &PathBuf::from("tests/fixtures/sync_plugin"),
        &InterfaceId::new(/* ... */),
    );
    
    // Call sync function
    let result = tree.dispatch_function_on_root("example", "sync-hello", true, &[]);
    assert!(result.is_ok());
}

#[test]
fn test_async_function_callable() {
    // This test will verify async functions work (even if blocking)
    let tree = initialisation::initialise_plugin_tree(
        &PathBuf::from("tests/fixtures/async_component"),
        &InterfaceId::new(/* ... */),
    );
    
    // Call async function - should block until complete
    let result = tree.dispatch_function_on_root("example", "async-hello", true, &[]);
    
    match result {
        Socket::ExactlyOne(Ok(Val::String(s))) => {
            assert_eq!(s, "Hello from async!");
        }
        _ => panic!("Unexpected result: {:?}", result),
    }
}
```

---

## Stage 1 Success Criteria

### Must Have
- ✅ Code compiles with `component-model-async` feature
- ✅ Engine creates successfully
- ✅ Sync components still work (no regressions)
- ✅ Async components load without panic
- ✅ Sync functions callable
- ✅ Async functions callable (even if blocking)

### Nice to Have
- ✅ Performance <20% overhead for sync functions
- ✅ Clear error messages for async issues
- ✅ Documentation of blocking behavior

### Validation Tests
1. Load sync-only component → should work exactly as before
2. Load async component with sync functions → should work
3. Load async component with async functions → should work (blocking)
4. Call sync function → no behavioral change
5. Call async function → blocks until complete, returns result
6. Mixed sync/async component → both types work

---

## Stage 2 Overview (If Stage 1 Blocking Unacceptable)

### Goals
- Non-blocking async operations
- True concurrency (multiple async calls in parallel)
- Minimal impact on sync functions

### Key Changes
1. **Enable async_support**: `config.async_support(true)`
2. **Dual rewrapping**:
   - Sync functions: `func_new()` (as before)
   - Async functions: `func_new_async()` (new)
3. **Async dispatch**: Add `dispatch_async()` variant
4. **Tokio integration**: Add runtime
5. **Update instantiation**: Use `instantiate_async()` for async components

### Challenges
- RwLock deadlocks (holding across await points)
- Complexity (two code paths)
- Testing concurrent scenarios

**Defer to Stage 2 plan if Stage 1 blocking is problematic.**

---

## Risks & Unknowns

### Risk 1: Wasmtime API for Async Tasks

**Problem**: Documentation unclear on how to poll Component Model async tasks from host.

**Impact**: May need different approach than `wait_for_async_completion()` shown above.

**Mitigation**:
- Research wasmtime source code
- Check wasmtime examples
- Ask on wasmtime Zulip if needed

### Risk 2: Performance Overhead

**Problem**: Blocking on async operations may be slow.

**Impact**: Unacceptable user experience if async ops are common/slow.

**Mitigation**:
- Profile early
- Measure async vs sync overhead
- Have Stage 2 plan ready if needed

### Risk 3: Component Model Async Maturity

**Problem**: Component Model async is Preview 3, still evolving.

**Impact**: API changes, bugs, incomplete features.

**Mitigation**:
- Pin wasmtime version
- Monitor releases
- Have fallback to sync-only

### Risk 4: Blocking Behavior Misunderstanding

**Problem**: Users expect async to be non-blocking.

**Impact**: Confusion, performance issues.

**Mitigation**:
- Document blocking behavior clearly
- Provide upgrade path to Stage 2
- Make Stage 1 clearly labeled as "minimal/blocking"

---

## Alternative Approaches (For Future Consideration)

### Alternative 1: Thread Pool for Async

Instead of Stage 2's tokio async, use thread pool:
- Spawn thread per async operation
- Block thread until complete
- Return to caller

**Pros**: Simpler than full async  
**Cons**: Heavy (OS threads), doesn't help with Component Model async protocol

### Alternative 2: Custom Event Loop

Build custom event loop for Component Model async:
- Manual task management
- Custom scheduler
- No tokio dependency

**Pros**: Full control  
**Cons**: Complex, reinventing wheel

### Alternative 3: Wait for Better Tooling

Defer until wasmtime provides higher-level async APIs.

**Pros**: Less risk  
**Cons**: May be waiting long time

---

## Open Questions

### Q1: Wasmtime Async Task API
How do we actually poll Component Model async tasks from host Rust?
- Is there a `Store::poll_task()` method?
- Do we use waitable sets?
- Need to research wasmtime API

### Q2: Performance Baseline
What's current performance for sync functions?
- Need metrics before measuring overhead
- What's acceptable regression %?

### Q3: Real Use Cases
Are there actual components with async functions to test?
- Or creating test fixtures?
- What async operations are realistic?

### Q4: Cardinality with Async
How does cardinality (AtMostOne, Any) work with async?
- Does blocking affect cardinality semantics?
- Can multiple async calls run in sequence?

### Q5: Error Handling
What errors can async operations produce?
- How do they propagate through rewrapping?
- What should user see?

---

## Next Steps

### Before Implementation
1. ✅ Create branch (done: `feature/component-model-async`)
2. Research wasmtime async task API
3. Create minimal test component
4. Measure baseline performance

### Stage 1 Implementation Order
1. Enable `wasm_component_model_async` (1.1)
2. Track async functions (1.2)
3. Update dependencies (1.5)
4. Create test component (1.6)
5. Handle async protocol in dispatch (1.3)
6. Handle Val::Future (1.4)
7. Integration tests (1.7)
8. Performance testing
9. Documentation

### After Stage 1
- Evaluate blocking behavior
- Decide: Stop or continue to Stage 2?
- If Stage 2: Create detailed plan

---

## Timeline

### Stage 1: Minimal Blocking
- Research & planning: 0.5 days
- Implementation: 1.5 days
- Testing: 0.5 days
- Documentation: 0.5 days
- **Total: 3 days**

### Stage 2: Non-blocking Async (if needed)
- Planning: 1 day
- Implementation: 4 days
- Testing: 1.5 days
- Documentation: 0.5 days
- **Total: 7 days**

---

## Recommendation

**Start with Stage 1 (Minimal Blocking)**:
1. Lowest risk
2. Validates Component Model async works
3. Can load async components
4. Unblocks development
5. Decision point after: continue or stop

**Only proceed to Stage 2 if**:
- Stage 1 works but blocking is problematic
- Real use cases need non-blocking
- Performance testing shows need

**Do NOT proceed to Stage 2 if**:
- Stage 1 blocking is acceptable
- No real async use cases yet
- Complexity not justified

---

## Success Metrics

### Stage 1 Success
- Can load and call async WIT functions (even if blocking)
- No regressions for sync functions
- Clear path to Stage 2 if needed

### Overall Success
- Plugins can use async WIT syntax
- Backward compatible with sync plugins
- Maintainable codebase
- Documented behavior

---

## Appendix: Wasmtime Component Model Async Resources

### Documentation
- [Component Model Async Explainer](https://github.com/WebAssembly/component-model/blob/main/design/mvp/Async.md)
- [Canonical ABI Async](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md)
- [wasmtime docs](https://docs.wasmtime.dev/)

### Examples
- Look in wasmtime repo: `crates/wasmtime/tests/` for async examples
- Search for `component-model-async` feature usage

### Community
- [Wasmtime Zulip](https://bytecodealliance.zulipchat.com/)
- [Component Model discussions](https://github.com/WebAssembly/component-model/discussions)

# Execution, cancellation, and ownership

`Plugin::instantiate` and `Plugin::link` provide synchronous public dispatch.
Their internal plugin imports are async-capable, so the same instances can be
used in asynchronous trees without rebuilding their state. `instantiate_async`
and `link_async` provide asynchronous public dispatch. Wasmtime concurrency
support must remain enabled for both forms.

Component Model threads are not supported. Future, stream, and error-context
values cannot cross wasm-link dispatch boundaries; passing them produces
`CallError::UnsupportedType`, including through synchronous dispatch.

## Contention and progress

Synchronous dispatch reserves an idle plugin for the invocation. If it is
already reserved or has outstanding calls, that target returns
[`SyncCallError::PluginBusy`](crate::SyncCallError::PluginBusy) without starting
the call. Synchronous dependency calls use the same policy. A host may retry or
skip a busy target; retrying inside a callback into that same active plugin
cannot succeed until the enclosing invocation returns.

Async dispatch waits for a synchronous reservation to end. This policy propagates
through the whole dependency tree, including dependencies linked with `link`.
Sync/async instance types select the public dispatch API, not the contention
policy of calls arriving from another plugin. Synchronous host functions and
CPU-bound guest code still occupy the thread polling the future while executing.

Each store owns its admitted calls. Polling a dispatch drives that store and the
dependency stores needed by its calls. It can also progress earlier calls whose
result futures were dropped. There are no background workers. Keeping an
unpolled future retains its operation; dropping it requests cancellation.
Waiting for async admission also drives outstanding guest work and cleanup.
Guest code may continue after returning a result; its call bookkeeping stays
alive until Wasmtime reports that the store has no active tasks.

Admission queues are unbounded. Each destination rotates between calling plugins
and then between their call paths, preserving FIFO order within a path. There
is no priority based on dependency depth, global completion ordering, or CPU
fairness guarantee. Applications needing backpressure must bound their own
outstanding dispatches. Epoch-limited stores serialize admitted calls.

Collection bindings start their targets concurrently and wait for every result.
An individual target failure does not discard successful results from other
targets. Cloned bindings share plugin instances and state.

## Error boundaries

The outer [`DispatchError`](crate::DispatchError) reports an interface or function
absent from the binding's metadata. Each async target returns a
[`CallError`](crate::CallError), describing a missing actual export, an unsupported
value, or a Wasmtime invocation/runtime failure. Synchronous targets return
[`SyncCallError`](crate::SyncCallError), adding only `PluginBusy` to those failures.
Neither synchronous error type contains asynchronous cancellation variants.

Dependency routing additionally handles resource ownership and malformed method
arguments. Its errors are encoded using the guest-facing `errors` interface in
`wit/wasm-link.wit`. That wire contract is independent of the Rust outer dispatch
error: the same guest import can run under synchronous or asynchronous dispatch.
A terminal store failure can fail multiple outstanding calls; independent
budgets do not imply independent failure domains. Failed stores are not restarted.

## Dropping a dispatch

Dropping an unpolled dispatch does no work. Dropping a polled dispatch cancels
requests that have not started and marks admitted operations as cancellation
requested. It does not roll back effects, reclaim arbitrary guest allocations,
or force an admitted Wasmtime task to stop.

Wasmtime 49 retains admitted guest tasks independently of their result futures.
They can resume when their store is driven again. A guest may cooperate with the
request or ignore it and finish. wasm-link does not start background computation
to finish abandoned calls or force cleanup. Pending work and cleanup progress
when the store is subsequently driven; no prompt-cleanup guarantee is made.

Plugins may optionally import `wasm-link:runtime/calls@0.5.0`:

```wit
interface calls {
    type call-id = u64;
    current-call: func() -> call-id;
    cancellation-requested: func() -> bool;
}
```

These imports are installed automatically and do not change application export
signatures. They are valid during dispatched guest invocations, not during
component initialization or resource destructors. `current-call` returns an
opaque ID for this invocation, including its callbacks. Treat it as an equality
token, not an ordering or persistent identifier. Dependency invocations receive
their own IDs and inherit cancellation requests from their callers.

`cancellation-requested` becomes true when the host abandons the dispatch or an
internal caller drops its wait for this invocation. Checking it neither suspends
nor wakes the guest. Cancellation is cooperative: guest code must check at safe
points, stop unwanted work, and release its own temporaries. A library integrating
these checks with a language's async runtime can make this easier.

A cancellation request is **not permission for another call to free this call's
memory**. The original continuation may still access it. Plugins must preserve
valid shared state across suspension and track ownership when temporary data
escapes into shared state. No particular allocator or guest language is required.
A panic or trap is not a substitute for normal cleanup: guest destructors need
not run after abrupt termination.

## Returned resources and deferred destruction

Until ownership is transferred, the plugin is responsible for its resources.
wasm-link retains ownership of completed results while they are in response
channels or waiting for other targets in a collection binding. If those results
are abandoned, their resource handles, including resources nested in composite
values, are queued for explicit destruction in their owning store. Delivery
transfers that responsibility to the recipient; raw host-visible `Val::Resource`
handles are not automatically destroyed by Rust's `Drop`. Return unused values
through `binding.release_resources(&plugin_id, value)`; do not use their handles
afterward. This queues destruction without running guest code.

Dropping a cross-plugin resource wrapper queues destruction of its underlying
resource. Cleanup is deferred because destructors can execute guest code and
require exclusive async access to the store. Cleanup waits until the store has
no active tasks; a continuously busy or stalled store can delay it indefinitely.
Per-call limiter callbacks also cover each destructor, using interface
`wasm-link:runtime/cleanup`, function `resource-drop`, and freestanding/void
metadata. This gives cleanup its own budget without resetting suspended calls.
There is no async-drop API or background cleanup
thread. Dropping the whole instance releases its store; it does not promise to
execute every guest destructor. Resources such as files and sockets can retain
external state while destruction is pending, even if linear memory remains the
same size.

## Limits and host responsibilities

Fuel budgets are independent per invocation and retained across suspension.
Dependency execution consumes the dependency's fuel, not the caller's. Initial
fuel covers component initialization; without a per-call limiter its remaining
fuel is consumed by subsequent execution.

**Epoch deadlines include dependency and host-function waits.** A stalled
dependency can exhaust its caller's deadline. To preserve independent deadlines,
epoch-limited stores admit one call at a time, including while suspended. Queued
calls receive a deadline on admission, not on enqueue. Other stores remain
independently runnable. Combining fuel and epoch limits retains serialization.
The host enables epoch interruption and advances `Engine::increment_epoch()`;
wasm-link does not supply a clock or ticker.

Epoch expiration is checked when Wasm executes; it does not wake pending host
operations. Fuel likewise does not meter host work. The host developer is
responsible for timeouts/cancellation of host-provided operations and for driving
the stores. Memory/table limits apply to the store's lifetime, shared by its
calls; they are not limits on wasm-link's queues or all host allocations.

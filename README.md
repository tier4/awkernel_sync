# awkernel_sync

Synchronization primitives for the [Awkernel](https://github.com/tier4/awkernel) operating system.

`awkernel_sync` is a `no_std` crate providing locks and interrupt guards that
work both inside a kernel (bare metal) and in a hosted `std` environment for
testing. Each locking primitive automatically disables interrupts while the
lock is held, via an `InterruptGuard`, so it is safe to use from interrupt
context.

## Primitives

| Type | Description |
| --- | --- |
| `mutex::Mutex` | Mutual-exclusion lock. Backed by an MCS lock (`no_std`), a spin lock (`spinlock` feature), or `parking_lot::Mutex` (`std` feature). |
| `mcs::MCSLock` | MCS queue lock; fair and cache-friendly under contention. Requires a per-caller `MCSNode`. |
| `spinlock::SpinLock` | Simple test-and-set spin lock. |
| `rwlock::RwLock` | Reader-writer lock. |
| `InterruptGuard` | RAII guard that disables interrupts on creation and restores the previous state on drop. |

## Features

You must enable **exactly one** architecture feature. If none is enabled the
crate fails to compile with an explanatory `compile_error!`.

| Feature | Description |
| --- | --- |
| `x86` | x86-64 bare-metal interrupt control (pulls in the `x86_64` crate). |
| `x86_mwait` | Use `monitor`/`mwait` for spin-wait backoff on x86-64. |
| `aarch64` | AArch64 bare-metal interrupt control. |
| `rv64` | RISC-V (64-bit) bare-metal interrupt control. |
| `rv32` | RISC-V (32-bit) bare-metal interrupt control. |
| `std` | Hosted build for tests/tooling; uses `parking_lot` and no-op interrupt guards. |
| `spinlock` | Make `mutex::Mutex` use a spin lock instead of the MCS lock (`no_std` only). |

There is no default feature.

## Usage

Add the crate with the architecture feature that matches your target:

```toml
[dependencies]
awkernel_sync = { version = "0.1", features = ["x86"] }
```

### `Mutex`

`Mutex::lock` takes a caller-provided `MCSNode`, which lives on the caller's
stack and avoids heap allocation in the queue lock:

```rust
use awkernel_sync::mutex::{MCSNode, Mutex};

let m = Mutex::new(0);

let mut node = MCSNode::new();
let mut guard = m.lock(&mut node);
*guard += 1;
```

### `RwLock`

```rust
use awkernel_sync::rwlock::RwLock;

let lock = RwLock::new(5);

{
    let r = lock.read();
    assert_eq!(*r, 5);
}

{
    let mut w = lock.write();
    *w += 1;
}
```

### `InterruptGuard`

```rust
use awkernel_sync::InterruptGuard;

{
    let _guard = InterruptGuard::new();
    // Interrupts are disabled in this scope.
}
// The previous interrupt state is restored here.
```

## Testing

`MCSLock` and `RwLock` are model-checked with [loom](https://crates.io/crates/loom).
The checks live in `tests/` and are gated behind the `loom` cfg:

```sh
RUSTFLAGS="--cfg loom" cargo test --features std
```

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

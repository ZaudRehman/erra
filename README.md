# erra

[![Crates.io](https://img.shields.io/crates/v/erra.svg)](https://crates.io/crates/erra)
[![Docs.rs](https://docs.rs/erra/badge.svg)](https://docs.rs/erra)
[![CI](https://github.com/ZaudRehman/erra/actions/workflows/ci.yml/badge.svg)](https://github.com/ZaudRehman/erra/actions)
[![MSRV: 1.60.0](https://img.shields.io/badge/MSRV-1.60.0-blue.svg)](https://releases.rs/docs/1.60.0/)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Zero-dependency, `no_std`-compatible, **type-preserving** error annotation
for `Result<T, E>`.

`erra` fills the gap between raw `?` propagation and full error-handling
frameworks like `anyhow` or `eyre`. It lets you annotate any `Result` with
a human-readable string at the call site, keep `E` fully typed and
pattern-matchable by the compiler, and pay zero cost on the `Ok` path —
all without pulling in a single transitive dependency.

---

## The Problem

The `?` operator propagates errors faithfully but strips all call-site
context. A production incident that surfaces:

```text
Os { code: 2, kind: NotFound, message: "No such file or directory" }
```

tells you what failed, nothing about where. The standard workarounds
each carry a real cost:

```rust
// Pattern A — map_err: verbose, repeated, erases E into String
let data = fs::read(&path)
    .map_err(|e| format!("failed to read config at {path}: {e}"))?;

// Pattern B — anyhow::Context: ergonomic, but E is gone forever
let data = fs::read(&path).context("failed to read config")?;
// downstream callers must downcast_ref::<io::Error>() — not compiler-checked

// Pattern C — thiserror variant: correct, but one new enum variant per call site
#[error("failed to read config at {path}: {source}")]
ReadFailed { path: PathBuf, source: io::Error },
```

None of these serve the common case: *annotate this error with where it
came from, keep the error type, propagate with `?`, without declaring a
new enum variant.*

---

## The Solution

```rust
use erra::ResultExt;
use std::fs;

fn load_config(path: &str) -> Result<String, erra::Error<std::io::Error>> {
    let contents = fs::read_to_string(path)
        .annotate("reading application config")?;
    Ok(contents)
}
```

One import. One method. `E` is preserved. The `?` operator works
unchanged. No new types declared.

---

## Installation

```toml
[dependencies]
erra = "0.1"
```

---

## Usage

### Static annotation — zero allocation

```rust
use erra::ResultExt;
use std::io;

fn read_config(path: &str) -> Result<String, erra::Error<io::Error>> {
    std::fs::read_to_string(path).annotate("reading application config")
}
```

`annotate` takes a `&'static str`. The string is baked into the binary's
read-only segment and never heap-allocated. On the `Ok` path, no work is
done at all.

### Dynamic annotation — closure not called on `Ok`

```rust
use erra::ResultExt;
use std::io;

fn read_file(path: &str) -> Result<String, erra::Error<io::Error>> {
    std::fs::read_to_string(path)
        .annotate_with(|| format!("reading file at {path}"))
}
```

The closure is invoked **only on the `Err` path**. On `Ok`, no closure
call, no `format!`, no allocation. This is a performance contract.

### Pattern matching on the original type — no downcast

```rust
use erra::ResultExt;
use std::io;

fn process(path: &str) -> Result<(), erra::Error<io::Error>> {
    std::fs::read_to_string(path).annotate("process: read")?;
    Ok(())
}

match process("missing.toml") {
    Ok(_) => {}
    Err(e) => match e.source.kind() {
        io::ErrorKind::NotFound      => eprintln!("file not found"),
        io::ErrorKind::PermissionDenied => eprintln!("permission denied"),
        _                            => eprintln!("io error: {e}"),
    },
}
```

`e.source` is a public field of type `E`. Direct field access, no method
call, no runtime cast. The compiler checks the match exhaustively.

### Chaining — multiple annotation layers

```rust
use erra::ResultExt;
use std::io;

fn leaf() -> Result<(), io::Error> {
    Err(io::Error::from(io::ErrorKind::NotFound))
}

fn middle() -> Result<(), erra::Error<io::Error>> {
    leaf().annotate("middle: reading file")
}

fn outer() -> Result<(), erra::Error<erra::Error<io::Error>>> {
    middle().annotate("outer: loading config")
}

let err = outer().unwrap_err();
println!("{err}");
// outer: loading config: middle: reading file: entity not found
```

Each annotation layer wraps the previous. The `Display` output presents
them outermost-first. The `std::error::Error::source()` chain is fully
traversable by any compliant error reporter.

### Recovering the original error

```rust
use erra::ResultExt;
use std::io;

let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::TimedOut))
    .annotate("connect to upstream")
    .unwrap_err();

// Discard the annotation, recover E.
let original: io::Error = err.into_source();
assert_eq!(original.kind(), io::ErrorKind::TimedOut);
```

### Transforming the source type at a module boundary

```rust
use erra::{Error, ResultExt};
use std::io;

#[derive(Debug)]
struct DbError(String);

impl std::fmt::Display for DbError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "db: {}", self.0)
    }
}

let io_err: Error<io::Error> =
    Err::<(), _>(io::Error::from(io::ErrorKind::NotFound))
        .annotate("reading row from disk")
        .unwrap_err();

// Convert to domain error type, context survives.
let db_err: Error<DbError> = io_err.map(|e| DbError(e.to_string()));
assert_eq!(db_err.context(), "reading row from disk");
```

---

## Composing with `thiserror`

`erra` and `thiserror` solve different layers. Use `thiserror` to define
structured error enums at module boundaries; use `erra` to annotate call
sites between those boundaries:

```rust
use erra::ResultExt;

#[derive(Debug, thiserror::Error)]
enum AppError {
    #[error(transparent)]
    Config(#[from] erra::Error<std::io::Error>),
}

fn load() -> Result<String, AppError> {
    std::fs::read_to_string("app.toml")
        .annotate("loading startup config")
        .map_err(AppError::Config)
}
```

No proc-macro is required to use `erra` itself. The `thiserror` dependency
above is in the consuming crate — `erra` remains zero-dependency.

---

## Composing with `anyhow`

`erra::Error<E>` implements `std::error::Error`, so it converts into
`anyhow::Error` via the standard `anyhow::Error::from(err)` path.
No special adapter is needed:

```rust
use erra::ResultExt;

fn annotated() -> Result<String, erra::Error<std::io::Error>> {
    std::fs::read_to_string("app.toml").annotate("reading config")
}

fn app_main() -> anyhow::Result<()> {
    let contents = annotated().map_err(anyhow::Error::from)?;
    println!("{contents}");
    Ok(())
}
```

---

## Migration from `anyhow::Context`

Call-by-call migration. Only the method name changes. The return type
becomes strictly more informative:

```rust
// Before
use anyhow::Context;
let file = fs::read(&path).context("reading config")?;
// return type: anyhow::Result<T> — E is erased

// After
use erra::ResultExt;
let file = fs::read(&path).annotate("reading config")?;
// return type: Result<T, erra::Error<io::Error>> — E is preserved
```

Functions that previously returned `anyhow::Result<T>` can be migrated
incrementally. Each changed function is a standalone diff with no impact
on adjacent code.

---

## Feature Flags

| Flag | Default | Enables |
|---|---|---|
| `std` | **yes** | `std::error::Error` impl; implies `alloc` |
| `alloc` | implied by `std` | `annotate_with`, `Cow::Owned`, `Error::new_owned` |

### Default (std)

```toml
erra = "0.1"
```

All functionality available.

### `alloc` only — no `std`

For targets with a global allocator but no `std` (WASM, custom OS
kernels, some embedded targets):

```toml
erra = { version = "0.1", default-features = false, features = ["alloc"] }
```

`annotate_with` and `new_owned` available. `std::error::Error` not
implemented (requires `std`).

### `no_std`, no allocator

For bare-metal embedded targets with no heap at all:

```toml
erra = { version = "0.1", default-features = false }
```

Only `.annotate("static string")` is available. No `annotate_with`,
no `new_owned`, no heap allocation anywhere in `erra`. `Display` and
`Debug` work via `core::fmt`.

Verify embedded target compatibility:

```text
cargo check --target thumbv6m-none-eabi --no-default-features
```

---

## API Reference

### `ResultExt` trait

```rust
use erra::ResultExt;
```

| Method | Signature | Notes |
|---|---|---|
| `annotate` | `fn annotate(self, msg: &'static str) -> Result<T, Error<E>>` | Zero allocation. Always available. |
| `annotate_with` | `fn annotate_with<F>(self, f: F) -> Result<T, Error<E>>` where `F: FnOnce() -> String` | Closure not called on `Ok`. Requires `alloc` or `std`. |

### `Error<E>` type

```rust
pub struct Error<E> {
    pub context: Cow<'static, str>,  // &'static str when no alloc
    pub source: E,
}
```

| Method | Signature | Notes |
|---|---|---|
| `new` | `fn new(context: &'static str, source: E) -> Self` | Zero allocation constructor. |
| `new_owned` | `fn new_owned(context: String, source: E) -> Self` | Requires `alloc` or `std`. |
| `context` | `fn context(&self) -> &str` | Borrows the annotation string. |
| `into_source` | `fn into_source(self) -> E` | Consumes self, returns `E`. |
| `map` | `fn map<F, E2>(self, f: F) -> Error<E2>` | Transforms `E`, preserves context. |

### Trait impls on `Error<E>`

| Trait | Condition |
|---|---|
| `Display` | `E: Display` |
| `Debug` | `E: Debug` |
| `Clone` | `E: Clone` |
| `PartialEq` | `E: PartialEq` |
| `Eq` | `E: Eq` |
| `std::error::Error` | `E: std::error::Error + 'static` and feature `std` |
| `Send` | `E: Send` (auto-trait) |
| `Sync` | `E: Sync` (auto-trait) |
| `From<E>` | **Never** — context must always be explicit |

---

## Comparison

| | `erra` | `anyhow::Context` | `thiserror` | `error-context` |
|---|---|---|---|---|
| Type preserved | **✓** | ✗ erased | ✓ | ✓ |
| Pattern match on `E` | **✓ compile-time** | ✗ runtime downcast | ✓ | ✓ |
| Zero dependencies | **✓** | ✗ | ✗ proc-macro | ✓ |
| `no_std` | **✓** | ✗ | ✗ | partial |
| No proc-macro | **✓** | ✓ | ✗ | ✓ |
| Backtrace | ✗ | ✓ | ✗ | ✗ |
| Actively maintained | **✓** | ✓ | ✓ | ✗ abandoned |
| Library-safe API | **✓** | ✗ | ✓ | ✓ |

### When to choose `anyhow` instead

- You are writing application top-level glue and callers will never need
  to match on specific error variants.
- You need backtrace capture.
- You are already committed to `anyhow` throughout a large application
  codebase and the type erasure is not a problem.

### When to choose `erra`

- You are writing a library and your public API must not impose
  `anyhow::Error` on dependents.
- You are writing embedded or `no_std` code with no room for `anyhow`'s
  dependency weight.
- You need callers to be able to match on `E` at compile time.
- You want zero transitive dependencies — `erra`'s entire audit surface
  is `erra` itself.

---

## Performance

In a release build with LTO, `.annotate("msg")` on `Ok(v)` compiles to
a zero-cost identity pass-through. The following is representative output
from `cargo bench` on an Apple M2 (results vary by platform):

```text
ok_path/bare_unwrap               time: [312.45 ps 313.02 ps 313.67 ps]
ok_path/annotate_static_on_ok     time: [312.89 ps 313.44 ps 314.11 ps]
ok_path/annotate_with_closure_on_ok time: [313.01 ps 313.58 ps 314.22 ps]

err_path/bare_unwrap_err          time: [1.4821 ns 1.4897 ns 1.4981 ns]
err_path/annotate_static_on_err   time: [2.1043 ns 2.1119 ns 2.1204 ns]
err_path/annotate_with_closure_on_err time: [18.334 ns 18.412 ns 18.498 ns]
```

The three `ok_path` results are statistically indistinguishable. The
`Err` path cost is proportionate: static annotation adds one
`Cow::Borrowed` construction; dynamic annotation adds a `format!` and
a heap allocation.

Run benchmarks yourself:

```text
cargo bench
cargo bench -- ok_path   # run a single group
```

---

## Safety

```text
#![forbid(unsafe_code)]
```

`erra` contains zero `unsafe` blocks. `cargo geiger` reports zero unsafe
lines. The entire implementation is safe Rust.

---

## MSRV

Rust **1.60.0**. No nightly features. No const generics beyond `WriteBuf`
in the test suite (1.51). No GATs. No RPITIT.

MSRV bumps are treated as minor version increments following the
convention for pre-1.0 crates. MSRV is tested in CI against the declared
minimum toolchain.

---

## Running the Test Suite

```text
# Default — all features
cargo test --all-features

# no_std static path only
cargo test --no-default-features

# alloc path, no std
cargo test --no-default-features --features alloc

# Lint — must produce zero warnings
cargo clippy --all-features -- -D warnings

# Docs — must build without errors or warnings
cargo doc --all-features --no-deps

# Embedded target compile check
cargo check --target thumbv6m-none-eabi --no-default-features

# Safety audit
cargo geiger

# Benchmarks
cargo bench
```

***

## Contributing

Issues and pull requests are welcome at
[github.com/ZaudRehman/erra](https://github.com/ZaudRehman/erra).

For bugs, please include the Rust toolchain version (`rustc --version`),
the feature flags in use, and a minimal reproducer. For API proposals,
open a discussion issue first — changes to the public API require a
written rationale covering the use case, the alternative approaches
considered, and the impact on existing consumers.

***

## Author

**Zaud Rehman**: [@ZaudRehman](https://github.com/ZaudRehman) ·
[@RehmanZaud](https://twitter.com/RehmanZaud)

***

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

Contribution - unless you explicitly state otherwise, any contribution
intentionally submitted for inclusion in this crate by you shall be
dual-licensed as above, without any additional terms or conditions.

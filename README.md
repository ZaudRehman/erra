# erra

[![Crates.io](https://img.shields.io/crates/v/erra.svg)](https://crates.io/crates/erra)
[![Docs.rs](https://docs.rs/erra/badge.svg)](https://docs.rs/erra)
[![CI](https://github.com/ZaudRehman/erra/actions/workflows/ci.yml/badge.svg)](https://github.com/ZaudRehman/erra/actions)
[![MSRV: 1.75.0](https://img.shields.io/badge/MSRV-1.75.0-blue.svg)](https://releases.rs/docs/1.75.0/)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#license)

Zero-dependency, `no_std`-compatible, **type-preserving** error annotation for `Result<T, E>`.

`erra` sits between raw `?` propagation and full frameworks like `anyhow` or `eyre`. Annotate any
`Result` with a human-readable string at the call site, keep `E` fully typed and pattern-matchable,
and pay zero cost on the `Ok` path — with no transitive dependencies.

---

## The Problem

The `?` operator propagates errors but strips all call-site context. A production incident that
surfaces:

```text
Os { code: 2, kind: NotFound, message: "No such file or directory" }
```

tells you what failed, not where. The standard workarounds each carry a cost:

```rust
// map_err: verbose and erases E into String
let data = fs::read(&path)
    .map_err(|e| format!("failed to read config at {path}: {e}"))?;

// anyhow::Context: ergonomic, but E is gone forever
let data = fs::read(&path).context("failed to read config")?;
// callers must downcast_ref::<io::Error>() -- not compiler-checked

// thiserror: correct, but one new enum variant per call site
#[error("failed to read config at {path}: {source}")]
ReadFailed { path: PathBuf, source: io::Error },
```

None of these cover the common case: annotate the error with context, keep the type, propagate
with `?`, without a new enum variant.

---

## The Solution

```rust
use erra::ResultExt;
use std::fs;

fn load_config(path: &str) -> erra::Result<String, std::io::Error> {
    let contents = fs::read_to_string(path)
        .annotate("reading application config")?;
    Ok(contents)
}
```

One import. One method. `E` is preserved. `?` works unchanged. No new types.

---

## Installation

```toml
[dependencies]
erra = "0.2"
```

---

## Usage

### Static annotation

```rust
use erra::ResultExt;
use std::io;

fn read_config(path: &str) -> erra::Result<String, io::Error> {
    std::fs::read_to_string(path).annotate("reading application config")
}
```

`annotate` takes a `&'static str`. The string lives in the binary's read-only segment and is never
heap-allocated. On the `Ok` path, no work is done.

### Dynamic annotation

```rust
use erra::ResultExt;
use std::io;

fn read_file(path: &str) -> erra::Result<String, io::Error> {
    std::fs::read_to_string(path)
        .annotate_with(|| format!("reading file at {path}"))
}
```

The closure is called **only on the `Err` path**. On `Ok`, there is no closure call, no `format!`,
and no allocation.

### Pattern matching without downcast

```rust
use erra::ResultExt;
use std::io;

fn process(path: &str) -> erra::Result<(), io::Error> {
    std::fs::read_to_string(path).annotate("process: read")?;
    Ok(())
}

match process("missing.toml") {
    Ok(_) => {}
    Err(e) => match e.source.kind() {
        io::ErrorKind::NotFound => eprintln!("file not found"),
        io::ErrorKind::PermissionDenied => eprintln!("permission denied"),
        _ => eprintln!("io error: {e}"),
    },
}
```

`e.source` is a public field of type `E`. Direct access, no method call, no runtime cast.

### Chaining

```rust
use erra::ResultExt;
use std::io;

fn leaf() -> Result<(), io::Error> {
    Err(io::Error::from(io::ErrorKind::NotFound))
}

fn middle() -> erra::Result<(), io::Error> {
    leaf().annotate("middle: reading file")
}

fn outer() -> erra::Result<(), erra::Error<io::Error>> {
    middle().annotate("outer: loading config")
}

let err = outer().unwrap_err();
println!("{err}");
```

Each annotation layer wraps the previous. `Display` presents them outermost-first. The
`std::error::Error::source()` chain is fully traversable by any compliant error reporter.

### Recovering the original error

```rust
use erra::ResultExt;
use std::io;

let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::TimedOut))
    .annotate("connect to upstream")
    .unwrap_err();

let original: io::Error = err.into_source();
assert_eq!(original.kind(), io::ErrorKind::TimedOut);
```

### Transforming the source type

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

let db_err: Error<DbError> = io_err.map(|e| DbError(e.to_string()));
assert_eq!(db_err.context(), "reading row from disk");
```

---

## Composing with `thiserror`

Use `thiserror` to define structured error enums at module boundaries and `erra` to annotate call
sites between them:

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

`erra` itself requires no proc-macro. The `thiserror` dependency above belongs to the consuming
crate.

---

## Composing with `anyhow`

`erra::Error<E>` implements `std::error::Error`, so it converts into `anyhow::Error` via the
standard `From` path. No adapter needed:

```rust
use erra::ResultExt;

fn annotated() -> erra::Result<String, std::io::Error> {
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

Only the method name changes. The return type becomes strictly more informative:

```rust
// Before
use anyhow::Context;
let file = fs::read(&path).context("reading config")?;
// return type: anyhow::Result<T> -- E is erased

// After
use erra::ResultExt;
let file = fs::read(&path).annotate("reading config")?;
// return type: erra::Result<T, io::Error> -- E is preserved
```

Migration is incremental. Each changed function is a self-contained diff with no impact on adjacent
code.

---

## Feature Flags

| Flag    | Default          | Enables                                          |
|---------|------------------|--------------------------------------------------|
| `std`   | yes              | `std::error::Error` impl; implies `alloc`        |
| `alloc` | implied by `std` | `annotate_with`, `Cow::Owned`, `Error::new_owned`|

### Default (`std`)

```toml
erra = "0.2"
```

All functionality available.

### `alloc` only

For targets with a global allocator but no `std`:

```toml
erra = { version = "0.2", default-features = false, features = ["alloc"] }
```

`annotate_with` and `new_owned` are available. `std::error::Error` is not implemented.

### `no_std`, no allocator

For bare-metal targets with no heap:

```toml
erra = { version = "0.2", default-features = false }
```

Only `.annotate("static string")` is available. No heap allocation anywhere in `erra`. `Display`
and `Debug` work via `core::fmt`.

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
|--------|-----------|-------|
| `annotate` | `fn annotate(self, msg: &'static str) -> erra::Result<T, E>` | Zero allocation. Always available. |
| `annotate_with` | `fn annotate_with<F: FnOnce() -> String>(self, f: F) -> erra::Result<T, E>` | Closure skipped on `Ok`. Requires `alloc`. |

### `Error<E>` type

```rust
pub struct Error<E> {
    pub context: core::borrow::Cow<'static, str>,
    pub source: E,
}
```

| Method | Signature | Notes |
|--------|-----------|-------|
| `new` | `fn new(context: &'static str, source: E) -> Self` | Zero allocation constructor. |
| `new_owned` | `fn new_owned(context: String, source: E) -> Self` | Requires `alloc` or `std`. |
| `context` | `fn context(&self) -> &str` | Borrows the annotation string. |
| `into_source` | `fn into_source(self) -> E` | Consumes self, returns `E`. |
| `map` | `fn map<F, E2>(self, f: F) -> Error<E2>` | Transforms `E`, preserves context. |

### Trait impls

| Trait | Condition |
|-------|-----------|
| `Display` | `E: Display` |
| `Debug` | `E: Debug` |
| `Clone` | `E: Clone` |
| `PartialEq` | `E: PartialEq` |
| `Eq` | `E: Eq` |
| `std::error::Error` | `E: std::error::Error + 'static` and feature `std` |
| `Send` | `E: Send` (auto-trait) |
| `Sync` | `E: Sync` (auto-trait) |
| `From<E>` | **Never** — context must always be explicit |

### Convenience alias

```rust
use erra::Result;

fn example() -> Result<(), std::io::Error> {
    Ok(())
}
```

`Result<T, E>` is a shorthand for `core::result::Result<T, erra::Error<E>>`.

---

## Comparison

|                      | `erra` | `anyhow::Context` | `thiserror` | `error-context` |
|----------------------|--------|-------------------|-------------|-----------------|
| Type preserved       | yes    | no (erased)       | yes         | yes             |
| Pattern match on `E` | compile-time | runtime downcast | yes    | yes             |
| Zero dependencies    | yes    | yes               | no (proc-macro) | yes         |
| `no_std`             | yes    | alloc only        | yes         | partial         |
| No proc-macro        | yes    | yes               | no          | yes             |
| Backtrace            | no     | yes               | yes         | no              |
| Actively maintained  | yes    | yes               | yes         | no (abandoned)  |
| Library-safe API     | yes    | no                | yes         | yes             |

### When to use `anyhow` instead

- Writing application glue where callers never need to match on specific error variants.
- Backtrace capture is required.
- Already committed to `anyhow` throughout a large codebase.

### When to use `erra`

- Writing a library whose public API must not impose `anyhow::Error` on dependents.
- Targeting embedded or `no_std` environments.
- Callers need to match on `E` at compile time.
- Zero transitive dependencies are a hard requirement.

---

## Performance

In a release build with LTO, `.annotate("msg")` on `Ok(v)` is intended to be a zero-cost identity
pass-through. `annotate_with` defers work until the `Err` path and does not invoke its closure on
the `Ok` path.

The exact microbenchmark numbers are intentionally omitted from the README so they do not age
faster than the code.

```text
cargo bench
cargo bench -- ok_path
```

---

## Safety

```text
#![forbid(unsafe_code)]
```

`erra` contains zero `unsafe` blocks. `cargo geiger` reports zero unsafe lines.

---

## MSRV

Rust **1.75.0**. No nightly features. No GATs. No RPITIT.

MSRV increases are treated as **minor** version bumps and are documented in
[CHANGELOG.md](CHANGELOG.md). CI tests the declared minimum on every push.

---

## Testing

```text
cargo test --all-features                           # all features
cargo test --no-default-features                    # no_std static path
cargo test --no-default-features --features alloc   # alloc, no std
cargo clippy --all-features -- -D warnings          # zero warnings
cargo doc --all-features --no-deps                  # docs check
cargo check --target thumbv6m-none-eabi --no-default-features
cargo geiger                                       # safety audit
cargo bench                                        # benchmarks
```

---

## Contributing

Issues and pull requests are welcome at [github.com/ZaudRehman/erra](https://github.com/ZaudRehman/erra).

For bugs, include the toolchain version (`rustc --version`), feature flags, and a minimal
reproducer. For API proposals, open a discussion issue first with a written rationale covering the
use case, alternatives considered, and impact on existing consumers.

---

## Author

**Zaud Rehman**: [@ZaudRehman](https://github.com/ZaudRehman) · [@RehmanZaud](https://twitter.com/RehmanZaud)

---

## License

Licensed under either of:

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in
this crate by you shall be dual-licensed as above, without any additional terms or conditions.

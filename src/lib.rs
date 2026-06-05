//! # erra
//!
//! Zero-dependency, `no_std`-compatible, **type-preserving** error annotation
//! for [`Result<T, E>`].
//!
//! ## The Problem
//!
//! The `?` operator propagates errors faithfully but strips every shred of
//! call-site context. A production incident that produces:
//!
//! ```text
//! Os { code: 2, kind: NotFound, message: "No such file or directory" }
//! ```
//!
//! tells you *what* failed but nothing about *where*, *which file*, or
//! *which layer* of your call stack produced it. Diagnosing it is slow
//! and expensive.
//!
//! The standard workarounds each carry a real cost:
//!
//! - **`map_err` + `format!`** — verbose, repeated at every call site, and
//!   erases the typed `E` into a `String`.
//! - **`anyhow::Context`** — ergonomic, but type-erasing. Once an error
//!   enters `anyhow::Error`, the only structured recovery path is
//!   `downcast_ref::<E>()` — a runtime operation the compiler cannot verify.
//!   Libraries cannot expose `anyhow::Error` in their public APIs without
//!   forcing the same choice on all dependents.
//! - **`thiserror` enum variants** — correct at public API boundaries but
//!   impractically verbose for internal call-site annotation, and adds a
//!   proc-macro compile dependency.
//!
//! `erra` fills the gap: annotate any `Result` with a string label at the
//! call site, keep `E` fully typed and pattern-matchable at compile time,
//! and pay zero cost on the `Ok` path.
//!
//! ## Quickstart
//!
//! Add to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! erra = "0.1"
//! ```
//!
//! Import the extension trait and annotate:
//!
//! ```rust
//! use erra::ResultExt;
//! use std::fs;
//!
//! fn load_config(path: &str) -> Result<String, erra::Error<std::io::Error>> {
//!     let contents = fs::read_to_string(path)
//!         .annotate("reading application config")?;
//!     Ok(contents)
//! }
//! ```
//!
//! Dynamic context — the closure is **not invoked** if the result is `Ok`:
//!
//! ```rust
//! # #[cfg(feature = "alloc")] {
//! use erra::ResultExt;
//! use std::fs;
//!
//! fn load_named(path: &str) -> Result<String, erra::Error<std::io::Error>> {
//!     fs::read_to_string(path)
//!         .annotate_with(|| format!("reading config at {path}"))
//! }
//! # }
//! ```
//!
//! Pattern-matching on the original typed error — **no downcast needed**:
//!
//! ```rust
//! use erra::ResultExt;
//! use std::io;
//!
//! fn process(path: &str) -> Result<(), erra::Error<io::Error>> {
//!     std::fs::read_to_string(path).annotate("process: read")?;
//!     Ok(())
//! }
//!
//! match process("missing.toml") {
//!     Ok(_) => {}
//!     Err(e) => match e.source.kind() {
//!         io::ErrorKind::NotFound => eprintln!("file not found"),
//!         _ => eprintln!("other io error: {e}"),
//!     },
//! }
//! ```
//!
//! ## Chaining
//!
//! Multiple annotations compose naturally. Each layer wraps the previous,
//! producing `Error<Error<E>>`. The `source()` chain is fully traversable
//! by any `std::error::Error`-compliant reporter:
//!
//! ```rust
//! use erra::ResultExt;
//! use std::io;
//!
//! fn inner() -> Result<(), io::Error> {
//!     Err(io::Error::from(io::ErrorKind::NotFound))
//! }
//!
//! fn middle() -> Result<(), erra::Error<io::Error>> {
//!     inner().annotate("middle: reading file")
//! }
//!
//! fn outer() -> Result<(), erra::Error<erra::Error<io::Error>>> {
//!     middle().annotate("outer: loading config")
//! }
//!
//! let err = outer().unwrap_err();
//! // Prints: "outer: loading config: middle: reading file: entity not found"
//! println!("{err}");
//! ```
//!
//! ## Composing with `thiserror`
//!
//! `erra` and `thiserror` solve different layers. Use `thiserror` to define
//! structured error enums at module boundaries; use `erra` to annotate call
//! sites between those boundaries without declaring a new variant per site:
//!
//! ```rust
//! use erra::ResultExt;
//!
//! #[derive(Debug)]
//! enum AppError {
//!     Config(erra::Error<std::io::Error>),
//! }
//!
//! impl std::fmt::Display for AppError {
//!     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//!         match self {
//!             AppError::Config(e) => write!(f, "config error: {e}"),
//!         }
//!     }
//! }
//!
//! impl std::error::Error for AppError {}
//! ```
//!
//! ## Compared to `anyhow`
//!
//! | Concern | `erra` | `anyhow` |
//! |---|---|---|
//! | Error type preserved | ✓ | ✗ (erased to `dyn Error`) |
//! | Pattern matching on `E` | ✓ compile-time | ✗ runtime downcast |
//! | Zero dependencies | ✓ | ✗ |
//! | `no_std` support | ✓ | ✗ |
//! | Backtrace capture | ✗ | ✓ |
//! | Library-safe public API | ✓ | ✗ |
//!
//! Choose `anyhow` when: you are writing application top-level glue code,
//! you need backtrace capture, or you have no interest in matching on
//! specific error variants after the fact.
//!
//! Choose `erra` when: you are writing a library, an embedded crate, or any
//! code where `E` must remain statically matchable by the caller.
//!
//! Note: `erra::Error<E>` converts naturally into `anyhow::Error` via
//! `anyhow::Error::from(err)` — since `erra::Error<E>: std::error::Error` —
//! so the two can coexist incrementally in the same codebase.
//!
//! ## `no_std` Usage
//!
//! Disable default features for the zero-allocation static-string path only.
//! No `annotate_with`, no heap allocation anywhere:
//!
//! ```toml
//! [dependencies]
//! erra = { version = "0.1", default-features = false }
//! ```
//!
//! Enable dynamic annotation on targets with a global allocator but no `std`
//! (WASM, custom OS kernels, etc.):
//!
//! ```toml
//! [dependencies]
//! erra = { version = "0.1", default-features = false, features = ["alloc"] }
//! ```
//!
//! ## Feature Flags
//!
//! | Flag | Default | Enables |
//! |---|---|---|
//! | `std` | **yes** | `std::error::Error` impl; implies `alloc` |
//! | `alloc` | implied by `std` | `annotate_with`, `Cow::Owned`, `Error::new_owned` |
//!
//! ## MSRV
//!
//! Rust **1.85.0**. No nightly features. No const generics. No GATs.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub,
    unused_qualifications
)]

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

mod error;
mod ext;

pub use error::Error;
pub use ext::ResultExt;
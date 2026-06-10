#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    rust_2018_idioms,
    unreachable_pub,
    unused_qualifications
)]

//! # erra
//!
//! A lightweight, `no_std`-compatible library for adding call-site context to
//! `Result<T, E>` without erasing or boxing the underlying error type.
//!
//! ## Motivation
//!
//! Rust's `?` operator is great at propagating errors, but it does not preserve
//! much context about the path that led there. If a production service surfaces
//! something like `Os { code: 2, kind: NotFound }`, you know what failed, but
//! not which operation triggered it or what the code was trying to do at the
//! time.
//!
//! The usual ways of adding context all involve trade-offs:
//!
//! - `map_err` with `format!` is repetitive, allocates eagerly, and usually
//!   collapses a structured error into a `String`.
//! - `anyhow` and `eyre` are a good fit for applications, but they erase the
//!   concrete error type behind `dyn Error`, which makes them a weaker fit for
//!   library APIs that want to preserve typed errors.
//! - `thiserror` works well for defining boundary error types, but it is too
//!   heavy for routine call-site annotation inside a module or subsystem.
//!
//! `erra` sits in the middle: it lets you attach context where the error
//! happens while keeping the original error type available for matching and
//! propagation. On the `Ok` path, it adds no allocation overhead.
//!
//! ## Quick start
//!
//! Bring [`ResultExt`] into scope to annotate any `Result<T, E>`:
//!
//! ```rust
//! use erra::ResultExt;
//! use std::fs;
//!
//! fn load_config(path: &str) -> core::result::Result<String, erra::Error<std::io::Error>> {
//!     fs::read_to_string(path)
//!         .annotate("failed to read application config layout")
//! }
//! ```
//!
//! If you prefer shorter signatures, use the crate's [`Result`] alias:
//!
//! ```rust
//! use erra::{Result, ResultExt};
//! use std::fs;
//!
//! fn load_config(path: &str) -> Result<String, std::io::Error> {
//!     fs::read_to_string(path)
//!         .annotate("failed to read application config layout")
//! }
//! ```
//!
//! For dynamic messages, use [`ResultExt::annotate_with`]. The closure runs
//! only on the `Err` path:
//!
//! ```rust
//! # #[cfg(feature = "alloc")] {
//! use erra::{Result, ResultExt};
//!
//! fn fetch_data(id: u64) -> Result<Vec<u8>, std::io::Error> {
//!     std::fs::read(format!("/data/{id}"))
//!         .annotate_with(|| format!("failed to pull record for id={id}"))
//! }
//! # }
//! ```
//!
//! ## Inspecting errors
//!
//! `erra::Error<E>` preserves the concrete type `E`, so callers can inspect the
//! wrapped error directly without downcasting:
//!
//! ```rust
//! use erra::{Result, ResultExt};
//! use std::io;
//!
//! fn run() -> Result<(), io::Error> {
//!     std::fs::read_to_string("missing.toml").annotate("reading setup file")?;
//!     Ok(())
//! }
//!
//! if let Err(err) = run() {
//!     match err.source.kind() {
//!         io::ErrorKind::NotFound => println!("file was missing"),
//!         _ => eprintln!("system error: {err}"),
//!     }
//! }
//! ```
//!
//! ## Nested context
//!
//! Annotations compose naturally. Multiple layers of `.annotate()` produce
//! nested wrappers such as `Error<Error<E>>`.
//!
//! With `std` enabled, [`std::error::Error::source`] walks that chain in the
//! usual way, so generic reporters and logging infrastructure can traverse it
//! without any special integration.
//!
//! ## Design notes
//!
//! ### No `From<E>`
//!
//! `erra` deliberately does not implement `From<E> for Error<E>`. If it did,
//! the `?` operator could wrap errors implicitly without adding any message,
//! which would defeat the purpose of explicit call-site annotation.
//!
//! ### `Display` and `source()`
//!
//! `Display` is meant for people: it formats the error as a readable outer-to-
//! inner message chain, such as `"outer context: inner context: underlying error"`.
//!
//! `source()` is meant for tools: it exposes the wrapped error through the
//! standard error-chain interface.
//!
//! ## Feature flags
//!
//! - `std` (default): implements `std::error::Error` for `Error<E>`. Implies
//!   `alloc`.
//! - `alloc`: enables owned context strings and [`ResultExt::annotate_with`].
//!
//! With `default-features = false`, `erra` still works in `no_std` builds using
//! static string context only.

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

mod error;
mod ext;

pub use error::Error;
pub use ext::ResultExt;

/// Shorthand for `core::result::Result<T, erra::Error<E>>`.
///
/// This alias keeps function signatures shorter when returning annotated
/// errors.
///
/// ```rust
/// use erra::{Result, ResultExt};
///
/// fn process() -> Result<i32, std::io::Error> {
///     std::fs::read_to_string("id.txt")
///         .annotate("failed to read id source")?;
///     Ok(42)
/// }
/// ```
///
/// This alias is not part of the prelude. Import it explicitly when you want
/// it.
pub type Result<T, E> = core::result::Result<T, Error<E>>;

/// The `erra` prelude.
///
/// This module re-exports [`ResultExt`] for convenient glob imports:
///
/// ```rust
/// use erra::prelude::*;
/// ```
pub mod prelude {
    pub use crate::ResultExt;
}

# Changelog

All notable changes to `erra` are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

For pre-1.0 releases, minor version bumps (`0.x.0`) may contain breaking
changes to the public API. Patch bumps (`0.x.y`) are strictly non-breaking.
MSRV bumps are treated as minor version increments.

---

## [Unreleased]

_Nothing yet._

---

## [0.2.0] — 2026-06-09

### Added

- `Result<T, E>` type alias at the crate root as shorthand for
  `core::result::Result<T, erra::Error<E>>`.
- `prelude` module exporting `ResultExt` for convenient glob imports.

### Changed

- Reworked crate-level documentation in `src/lib.rs` to be shorter, clearer,
  and easier to read.
- Rewrote `src/error.rs` documentation to use a more direct, maintainer-style
  tone while keeping the public API fully documented.
- Rewrote `src/ext.rs` documentation to match the rest of the crate and reduce
  overly polished or repetitive phrasing.
- Standardized examples across the crate so signatures, imports, and feature
  gating are more consistent.
- Clarified the role of the root `Result` alias and explicitly kept it out of
  the prelude to avoid shadowing the standard library `Result`.
- Tightened documentation around `Display`, `source()`, and nested annotation
  behavior.

### Performance

- Added and reviewed benchmark coverage for happy-path, error-path, chaining,
  throughput, and formatting overhead.
- Confirmed that static annotation remains effectively free on the `Ok` path in
  release builds.
- Confirmed that `annotate_with` does not evaluate its closure on the `Ok` path
  and shifts dynamic formatting cost to the `Err` path only.

### Tooling

- Expanded CI coverage to validate feature combinations, `no_std` compilation,
  docs, clippy, and benchmark compilation.
- Kept MSRV policy at Rust 1.60.0 and aligned release prep around enforcing it.

---

## [0.1.0] — 2026-06-03

### Added

- `Error<E>` — the core annotated error type with two public fields:
  - `source: E` — the original error, fully typed and directly accessible
  - `context: Cow<'static, str>` — the human-readable annotation string
- `ResultExt` trait with two methods on `Result<T, E>`:
  - `annotate(&'static str)` — zero-allocation static annotation; always
    available including in `no_std` builds with no allocator
  - `annotate_with(FnOnce() -> String)` — lazy dynamic annotation; the
    closure is invoked only on the `Err` path; requires `alloc` or `std`
- `Error::new(context: &'static str, source: E)` — zero-allocation
  constructor; always available
- `Error::new_owned(context: String, source: E)` — owned-string
  constructor; requires `alloc` or `std`
- `Error::context(&self) -> &str` — borrow the annotation string
- `Error::into_source(self) -> E` — consume the wrapper, recover `E`
- `Error::map<F, E2>(self, f: F) -> Error<E2>` — transform the source
  type while preserving the context string
- `Display` impl for `Error<E>` where `E: Display` — formats as
  `"context: source"`, outermost annotation first in nested chains
- `Debug` impl for `Error<E>` where `E: Debug`
- `Clone` impl for `Error<E>` where `E: Clone`
- `PartialEq` impl for `Error<E>` where `E: PartialEq`
- `Eq` impl for `Error<E>` where `E: Eq`
- `std::error::Error` impl for `Error<E>` where
  `E: std::error::Error + 'static`; `source()` returns `Some(&self.source)`,
  enabling full chain traversal by compliant error reporters; gated on
  feature `std`
- `Send` and `Sync` auto-trait derivation conditional on `E: Send` / `E: Sync`
- Feature flags:
  - `std` (default) — enables `std::error::Error` impl; implies `alloc`
  - `alloc` — enables `annotate_with` and `new_owned`; standalone flag
    for targets with a global allocator but no `std`
  - bare `no_std` (no features) — only `annotate` and `new` available;
    `Display` and `Debug` via `core::fmt`; zero heap allocation anywhere
    in the crate
- `#![forbid(unsafe_code)]` — no `unsafe` in the entire crate
- MSRV: Rust 1.60.0
- Dual-licensed MIT OR Apache-2.0

### Design decisions recorded

- `From<E> for Error<E>` is intentionally absent. Implementing it would
  allow `?` to silently construct `Error<E>` with no context string,
  defeating the purpose of the crate. Context must always be explicit.
- `context` is a `pub` field rather than a private field with only a
  getter, because consumers legitimately need to read and pattern-match
  on the string. Privacy here would force unnecessary `.context()` calls
  with no safety benefit.
- `source` is a `pub` field rather than accessed via a method, enabling
  direct field access patterns (`e.source.kind()`) without a runtime
  method call and preserving compiler exhaustiveness checking on the
  inner type.
- The `Cow<'static, str>` representation for `context` unifies the
  static (`&'static str`, zero allocation) and dynamic (`String`,
  one allocation) paths behind a single field type, avoiding a two-variant
  enum or a trait object.

---

[Unreleased]: https://github.com/ZaudRehman/erra/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/ZaudRehman/erra/releases/tag/v0.2.0
[0.1.0]: https://github.com/ZaudRehman/erra/releases/tag/v0.1.0
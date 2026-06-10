//! Compile-time trait bound verification.
//!
//! These tests contain no runtime assertions. They exist purely to make the
//! compiler verify that `Error<E>` implements the correct set of traits for
//! representative instantiations of `E`. If any impl is accidentally removed,
//! a bound is incorrectly tightened, or a `cfg` gate is misapplied, this
//! file will fail to compile — surfacing the regression at `cargo test` time
//! rather than at a downstream consumer's build.
//!
//! The `static_assertions` crate is used for `assert_impl_all!` and
//! `assert_not_impl_any!`. These expand to zero-cost compile-time checks.
//!
//! Test matrix:
//!
//! | Assertion | `E` | Trait(s) | Feature gate |
//! |---|---|---|---|
//! | Send + Sync | `io::Error` | auto-trait derivation for a common E | none |
//! | Send + Sync | `String` | auto-trait derivation for an owned heap type | none |
//! | Send + Sync | `Arc<str>` | auto-trait derivation for a shared ref type | none |
//! | !Send | `*const u8` | auto-trait NOT derived when E is !Send | none |
//! | std::error::Error | `io::Error` | std feature impl present | `std` |
//! | std::error::Error | `Error<io::Error>` | nested chain is also Error | `std` |
//! | Display + Debug | `io::Error` | fmt impls always present | none |
//! | Display + Debug | `u32` | fmt impls work for non-Error E | none |
//! | Clone | `u32` | Clone derived when E: Clone | none |
//! | PartialEq + Eq | `u32` | Eq derived when E: Eq | none |
//! | !Clone | `io::Error` | Clone NOT derived when E: !Clone | none |
//! | !PartialEq | `io::Error` | PartialEq NOT derived when E: !PartialEq | none |
//! | !From<E> | `io::Error` | implicit From conversion absent by design | none |
//! | UnwindSafe | `u32`, `String` | unwind safety mirrors E | `std` |

use erra::Error;
use static_assertions::{assert_impl_all, assert_not_impl_any};
use std::io;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Send + Sync
//
// `Error<E>` must be Send when E: Send, and Sync when E: Sync.
// These are auto-traits derived from E and the internal `&'static str` context
// field, which is always Send + Sync. We verify for three representative E
// types to catch any accidental introduction of a !Send or !Sync field.
// ─────────────────────────────────────────────────────────────────────────────

assert_impl_all!(Error<io::Error>: Send, Sync);
assert_impl_all!(Error<String>: Send, Sync);
assert_impl_all!(Error<Arc<str>>: Send, Sync);

// `Error<*const u8>` must NOT be Send or Sync because raw pointers are
// neither. This verifies that auto-trait derivation is conditional on E and
// not unconditionally asserted anywhere in the crate.
assert_not_impl_any!(Error<*const u8>: Send, Sync);

// ─────────────────────────────────────────────────────────────────────────────
// Display
//
// Implemented for all E: Display, unconditionally via `core::fmt`.
// Does not require the `std` or `alloc` feature. Does not require E to
// implement std::error::Error — only E: Display is needed.
// ─────────────────────────────────────────────────────────────────────────────

assert_impl_all!(Error<io::Error>: core::fmt::Display);
assert_impl_all!(Error<u32>: core::fmt::Display);
assert_impl_all!(Error<String>: core::fmt::Display);
assert_impl_all!(Error<&'static str>: core::fmt::Display);

// ─────────────────────────────────────────────────────────────────────────────
// Debug
//
// Implemented for all E: Debug, unconditionally via `core::fmt`.
// ─────────────────────────────────────────────────────────────────────────────

assert_impl_all!(Error<io::Error>: core::fmt::Debug);
assert_impl_all!(Error<u32>: core::fmt::Debug);
assert_impl_all!(Error<String>: core::fmt::Debug);
assert_impl_all!(Error<Vec<u8>>: core::fmt::Debug);

// ─────────────────────────────────────────────────────────────────────────────
// Clone
//
// Implemented only when E: Clone. io::Error does not implement Clone,
// so Error<io::Error> must not implement Clone.
// ─────────────────────────────────────────────────────────────────────────────

assert_impl_all!(Error<u32>: Clone);
assert_impl_all!(Error<String>: Clone);
assert_impl_all!(Error<Vec<u8>>: Clone);
assert_impl_all!(Error<&'static str>: Clone);

assert_not_impl_any!(Error<io::Error>: Clone);

// ─────────────────────────────────────────────────────────────────────────────
// PartialEq + Eq
//
// Implemented only when E: PartialEq / Eq. io::Error implements neither.
// ─────────────────────────────────────────────────────────────────────────────

assert_impl_all!(Error<u32>: PartialEq, Eq);
assert_impl_all!(Error<u64>: PartialEq, Eq);
assert_impl_all!(Error<String>: PartialEq, Eq);
assert_impl_all!(Error<&'static str>: PartialEq, Eq);

assert_not_impl_any!(Error<io::Error>: PartialEq);

// ─────────────────────────────────────────────────────────────────────────────
// From<E> is intentionally ABSENT
//
// Error<E> must not implement From<E>. If it did, the ? operator would
// silently construct an Error<E> with no context string — defeating the
// entire purpose of erra. This assertion locks that design decision in
// permanently. Removing it must require an explicit architectural decision.
// ─────────────────────────────────────────────────────────────────────────────

assert_not_impl_any!(Error<io::Error>: From<io::Error>);
assert_not_impl_any!(Error<u32>: From<u32>);
assert_not_impl_any!(Error<String>: From<String>);

// ─────────────────────────────────────────────────────────────────────────────
// std-only assertions
//
// The following impls are gated on the `std` feature and must not be
// verified when compiling without it.
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "std")]
mod std_checks {
    use super::*;

    // ── std::error::Error ────────────────────────────────────────────────────
    //
    // Implemented only when E: std::error::Error + 'static.
    // Verified for a concrete stdlib error, a nested chain at depth 2, and
    // a nested chain at depth 3 to confirm the impl recurses correctly.

    assert_impl_all!(Error<io::Error>: std::error::Error);
    assert_impl_all!(Error<Error<io::Error>>: std::error::Error);
    assert_impl_all!(Error<Error<Error<io::Error>>>: std::error::Error);

    // u32 does not implement std::error::Error, so Error<u32> must not either.
    assert_not_impl_any!(Error<u32>: std::error::Error);

    // ── UnwindSafe + RefUnwindSafe ────────────────────────────────────────────
    //
    // These auto-traits live in std::panic and are only available with `std`.
    // Error<E> must be unwind-safe when E is, to not restrict catch_unwind
    // call sites that propagate erra errors across the unwind boundary.

    assert_impl_all!(Error<u32>: std::panic::UnwindSafe, std::panic::RefUnwindSafe);
    assert_impl_all!(Error<String>: std::panic::UnwindSafe, std::panic::RefUnwindSafe);
}

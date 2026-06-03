//! The [`ResultExt`] extension trait and its blanket implementation on
//! [`Result<T, E>`].
//!
//! This module is intentionally thin. Its only responsibility is to define
//! the public-facing trait and wire it to [`Error::new`] / [`Error::new_owned`]
//! from [`crate::error`]. All type logic lives in `error.rs`.

#[cfg(any(feature = "alloc", feature = "std"))]
use alloc::string::String;

use crate::Error;

// ── Trait Definition ──────────────────────────────────────────────────────────

/// Extension trait that adds type-preserving error annotation to any
/// [`Result<T, E>`].
///
/// Import this trait to bring `.annotate(...)` and `.annotate_with(...)` into
/// scope on every `Result` in your codebase:
///
/// ```rust
/// use erra::ResultExt;
/// ```
///
/// That single `use` statement is the entire adoption cost.
///
/// # Blanket Implementation
///
/// `ResultExt` is implemented for **all** `Result<T, E>` with no bounds at
/// the trait level. You do not need `E: std::error::Error` to annotate —
/// that bound is only required if you later pass the annotated error to a
/// reporter that requires `std::error::Error`. This means `erra` works on
/// any error type, including plain integers, strings, and custom structs that
/// do not implement `std::error::Error`.
///
/// # No `From<E>` Interaction
///
/// `erra` deliberately does not implement `From<E> for Error<E>`. If it did,
/// the `?` operator would silently wrap errors with no context string —
/// exactly the failure mode this crate exists to prevent. Annotation is
/// always an explicit, authored act.
pub trait ResultExt<T, E>: Sized {
    /// Wraps `Err(e)` with a static annotation string.
    ///
    /// On `Ok(v)`, this is a zero-cost identity pass-through: no allocation,
    /// no closure invocation, no work of any kind. In release builds, the
    /// `Ok` branch inlines away entirely.
    ///
    /// The `&'static str` constraint is deliberate. Static strings:
    ///
    /// 1. Require zero heap allocation — the pointer is embedded in the
    ///    binary's read-only data segment.
    /// 2. Encourage callers to write meaningful, stable annotation strings
    ///    rather than dynamically constructed noise.
    /// 3. Work on `no_std` targets with no allocator at all.
    ///
    /// When you need a runtime-constructed string (e.g. one containing a
    /// file path or identifier), use [`annotate_with`] instead.
    ///
    /// [`annotate_with`]: ResultExt::annotate_with
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::io;
    ///
    /// fn read_config(path: &str) -> Result<String, erra::Error<io::Error>> {
    ///     std::fs::read_to_string(path).annotate("reading application config")
    /// }
    ///
    /// // The ? operator composes naturally with the wrapped type.
    /// fn start_app() -> Result<(), erra::Error<io::Error>> {
    ///     let _cfg = std::fs::read_to_string("app.toml")
    ///         .annotate("loading startup config")?;
    ///     Ok(())
    /// }
    /// ```
    fn annotate(self, msg: &'static str) -> Result<T, Error<E>>;

    /// Wraps `Err(e)` with a lazily-evaluated dynamic annotation string.
    ///
    /// **The closure is not invoked if the result is `Ok`.** This is a
    /// performance contract, not just an implementation detail. Any
    /// computation inside the closure — including `format!`, string
    /// concatenation, or path rendering — occurs exclusively on the error
    /// path. The `Ok` path pays zero cost: no closure call, no allocation,
    /// no branch.
    ///
    /// Use this when the annotation must embed runtime values such as file
    /// paths, resource identifiers, indices, or counts. For compile-time
    /// string literals, prefer [`annotate`] — it is simpler and avoids even
    /// the closure construction.
    ///
    /// Requires feature `alloc` or `std` (enabled by default).
    ///
    /// [`annotate`]: ResultExt::annotate
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::io;
    ///
    /// fn read_nth_chunk(path: &str, n: usize) -> Result<Vec<u8>, erra::Error<io::Error>> {
    ///     std::fs::read(path)
    ///         .annotate_with(|| format!("reading chunk {n} from {path}"))
    /// }
    ///
    /// // Closure is NOT called on the Ok path — verified in tests/basic.rs.
    /// fn no_cost_on_success() -> Result<String, erra::Error<io::Error>> {
    ///     Ok::<_, io::Error>("hello".to_string())
    ///         .annotate_with(|| {
    ///             // This body is never executed when the result is Ok.
    ///             format!("expensive context: {}", expensive_computation())
    ///         })
    /// }
    ///
    /// fn expensive_computation() -> String {
    ///     "computed".to_string()
    /// }
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    fn annotate_with<F>(self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> String;
}

// ── Blanket Implementation ────────────────────────────────────────────────────

impl<T, E> ResultExt<T, E> for Result<T, E> {
    /// Inlined match on `self`. In release builds with LTO, the `Ok` arm
    /// compiles to a direct identity — no branch, no stack frame, no cost
    /// beyond the `?` the caller would have written without `erra`.
    #[inline]
    fn annotate(self, msg: &'static str) -> Result<T, Error<E>> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::new(msg, e)),
        }
    }

    /// Inlined match on `self`. The closure `f` is placed in the `Err` arm
    /// only. The compiler sees that `f` is unreachable on the `Ok` arm and
    /// eliminates the closure construction entirely in release builds.
    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    fn annotate_with<F>(self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> String,
    {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::new_owned(f(), e)),
        }
    }
}
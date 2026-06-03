//! The [`Error<E>`] type: a typed, annotated error wrapper.
//!
//! This module owns the struct definition, all standard trait implementations,
//! and the utility method surface. Nothing in here performs I/O, allocates on
//! the `Ok` path, or requires `std` beyond the feature-gated `error::Error`
//! impl.

#[cfg(any(feature = "alloc", feature = "std"))]
use alloc::{borrow::Cow, string::String};

use core::fmt;

// ── Struct ────────────────────────────────────────────────────────────────────

/// A type-preserving annotated error.
///
/// `Error<E>` wraps an underlying error `E` with a human-readable annotation
/// string that describes the operation that failed. It preserves the original
/// error type entirely: downstream code can pattern-match on [`Error::source`]
/// at compile time without any runtime downcasting.
///
/// # Construction
///
/// Prefer constructing via [`ResultExt::annotate`] and
/// [`ResultExt::annotate_with`] rather than calling constructors directly.
/// The constructors are provided for cases where you need to build an
/// `Error<E>` outside of a `Result` chain.
///
/// # Size
///
/// On 64-bit platforms:
///
/// - With `std` or `alloc`: `context` is `Cow<'static, str>` — 24 bytes.
///   `source` is `E`. Total overhead: 24 bytes beyond the size of `E`.
/// - Without `alloc`: `context` is `&'static str` — 16 bytes (pointer +
///   length). Total overhead: 16 bytes beyond the size of `E`.
///
/// # Example
///
/// ```rust
/// use erra::{Error, ResultExt};
/// use std::io;
///
/// let err: Error<io::Error> = Err::<(), _>(io::Error::from(io::ErrorKind::PermissionDenied))
///     .annotate("checking socket permissions")
///     .unwrap_err();
///
/// // Direct typed access — no downcast.
/// assert_eq!(err.source.kind(), io::ErrorKind::PermissionDenied);
/// assert_eq!(err.context(), "checking socket permissions");
/// println!("{err}"); // "checking socket permissions: permission denied"
/// ```
pub struct Error<E> {
    /// The annotation string describing the operation that failed.
    ///
    /// - [`Error::new`] stores this as `Cow::Borrowed` — zero allocation.
    /// - [`Error::new_owned`] stores this as `Cow::Owned` — one heap
    ///   allocation on the error path only.
    ///
    /// Borrow as `&str` via [`Error::context`].
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub context: Cow<'static, str>,

    /// The annotation string (static-only path when neither `alloc` nor `std`
    /// is available).
    #[cfg(not(any(feature = "alloc", feature = "std")))]
    pub context: &'static str,

    /// The original typed error, preserved without modification.
    ///
    /// This field is intentionally `pub`. Exposing it as a field rather than
    /// an accessor method allows callers to move out of it (destructure,
    /// match, or pass by value) without cloning. An accessor would only
    /// provide a borrow.
    ///
    /// Pattern matching example:
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::io;
    ///
    /// fn handle(res: Result<(), io::Error>) {
    ///     match res.annotate("network connect") {
    ///         Ok(()) => {}
    ///         Err(e) => match e.source.kind() {
    ///             io::ErrorKind::ConnectionRefused => eprintln!("server down"),
    ///             io::ErrorKind::TimedOut          => eprintln!("timed out"),
    ///             _                                => eprintln!("io error: {e}"),
    ///         },
    ///     }
    /// }
    /// ```
    pub source: E,
}

// ── Constructors ──────────────────────────────────────────────────────────────

impl<E> Error<E> {
    /// Constructs a new `Error<E>` with a static annotation string.
    ///
    /// This is the zero-allocation constructor. The `&'static str` is stored
    /// as `Cow::Borrowed` internally and is never heap-allocated. Prefer this
    /// over [`new_owned`] whenever the annotation can be a compile-time
    /// string literal.
    ///
    /// [`new_owned`]: Error::new_owned
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    /// use std::io;
    ///
    /// let e = Error::new(
    ///     "loading application config",
    ///     io::Error::from(io::ErrorKind::NotFound),
    /// );
    /// assert_eq!(e.context(), "loading application config");
    /// ```
    #[inline]
    pub fn new(context: &'static str, source: E) -> Self {
        Self {
            #[cfg(any(feature = "alloc", feature = "std"))]
            context: Cow::Borrowed(context),
            #[cfg(not(any(feature = "alloc", feature = "std")))]
            context,
            source,
        }
    }

    /// Constructs a new `Error<E>` with a heap-allocated annotation string.
    ///
    /// Use this when the context message must be constructed dynamically at
    /// runtime — for example, when it includes a file path, a resource
    /// identifier, or a counter. Prefer [`new`] when a static string literal
    /// suffices; it avoids the heap allocation entirely.
    ///
    /// Requires feature `alloc` or `std` (enabled by default).
    ///
    /// [`new`]: Error::new
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    /// use std::io;
    ///
    /// let path = "/etc/app/config.toml";
    /// let e = Error::new_owned(
    ///     format!("reading config at {path}"),
    ///     io::Error::from(io::ErrorKind::NotFound),
    /// );
    /// assert_eq!(e.context(), "reading config at /etc/app/config.toml");
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    pub fn new_owned(context: String, source: E) -> Self {
        Self {
            context: Cow::Owned(context),
            source,
        }
    }

    /// Borrows the annotation string as `&str`.
    ///
    /// Works identically whether the context was constructed via [`new`]
    /// (static borrow, zero cost) or [`new_owned`] (heap-allocated string).
    ///
    /// [`new`]: Error::new
    /// [`new_owned`]: Error::new_owned
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    ///
    /// let e = Error::new("write failed", std::io::Error::from(std::io::ErrorKind::BrokenPipe));
    /// assert_eq!(e.context(), "write failed");
    /// ```
    #[inline]
    pub fn context(&self) -> &str {
        #[cfg(any(feature = "alloc", feature = "std"))]
        {
            &self.context
        }
        #[cfg(not(any(feature = "alloc", feature = "std")))]
        {
            self.context
        }
    }

    /// Consumes this error and returns the original source error, discarding
    /// the annotation string.
    ///
    /// This is the primary escape hatch for callers that need to recover the
    /// typed error for re-matching or re-wrapping at a different layer, and
    /// no longer need the context string.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::io;
    ///
    /// let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::TimedOut))
    ///     .annotate("connecting to upstream")
    ///     .unwrap_err();
    ///
    /// let original: io::Error = err.into_source();
    /// assert_eq!(original.kind(), io::ErrorKind::TimedOut);
    /// ```
    #[inline]
    pub fn into_source(self) -> E {
        self.source
    }

    /// Maps the source error to a new type, preserving the annotation string.
    ///
    /// This is the primary composition primitive for library authors who need
    /// to convert between error types at a module boundary without losing the
    /// call-site context that was already attached:
    ///
    /// ```rust
    /// use erra::{Error, ResultExt};
    /// use std::io;
    ///
    /// #[derive(Debug)]
    /// struct DbError(String);
    ///
    /// impl std::fmt::Display for DbError {
    ///     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    ///         write!(f, "db error: {}", self.0)
    ///     }
    /// }
    ///
    /// let io_err: Error<io::Error> =
    ///     Err::<(), _>(io::Error::from(io::ErrorKind::NotFound))
    ///         .annotate("reading row from disk")
    ///         .unwrap_err();
    ///
    /// // Convert to domain error at the module boundary, context survives.
    /// let db_err: Error<DbError> = io_err.map(|e| DbError(e.to_string()));
    /// assert_eq!(db_err.context(), "reading row from disk");
    /// ```
    #[inline]
    pub fn map<F, E2>(self, f: F) -> Error<E2>
    where
        F: FnOnce(E) -> E2,
    {
        Error {
            context: self.context,
            source: f(self.source),
        }
    }
}

// ── fmt::Display ──────────────────────────────────────────────────────────────

impl<E: fmt::Display> fmt::Display for Error<E> {
    /// Formats the error as `"{context}: {source}"`.
    ///
    /// When errors are chained (`Error<Error<E>>`), each layer emits its own
    /// context and then delegates to its source's `Display`, producing a
    /// naturally readable cause chain:
    ///
    /// ```text
    /// "outer: loading config: inner: reading file: entity not found"
    /// ```
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    ///
    /// let e = Error::new("parse step", "bad utf-8 sequence");
    /// assert_eq!(e.to_string(), "parse step: bad utf-8 sequence");
    /// ```
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context(), self.source)
    }
}

// ── fmt::Debug ────────────────────────────────────────────────────────────────

impl<E: fmt::Debug> fmt::Debug for Error<E> {
    /// Formats as `Error { context: "...", source: <E's Debug output> }`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    ///
    /// let e = Error::new("decode", 42u32);
    /// let s = format!("{e:?}");
    /// assert!(s.contains("decode"));
    /// assert!(s.contains("42"));
    /// ```
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("context", &self.context())
            .field("source", &self.source)
            .finish()
    }
}

// ── Clone ─────────────────────────────────────────────────────────────────────

impl<E: Clone> Clone for Error<E> {
    /// Clones both the context string and the source error.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    ///
    /// let e = Error::new("ctx", 7u8);
    /// let cloned = e.clone();
    /// assert_eq!(e, cloned);
    /// ```
    #[inline]
    fn clone(&self) -> Self {
        Self {
            #[cfg(any(feature = "alloc", feature = "std"))]
            context: self.context.clone(),
            #[cfg(not(any(feature = "alloc", feature = "std")))]
            context: self.context,
            source: self.source.clone(),
        }
    }
}

// ── PartialEq / Eq ────────────────────────────────────────────────────────────

impl<E: PartialEq> PartialEq for Error<E> {
    /// Two `Error<E>` values are equal when both their `context` strings and
    /// their `source` errors are equal.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    ///
    /// let a = Error::new("ctx", 1u32);
    /// let b = Error::new("ctx", 1u32);
    /// let c = Error::new("ctx", 2u32);
    ///
    /// assert_eq!(a, b);
    /// assert_ne!(a, c);
    /// ```
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.context() == other.context() && self.source == other.source
    }
}

impl<E: Eq> Eq for Error<E> {}

// ── std::error::Error ─────────────────────────────────────────────────────────

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    /// Returns the original source error as a `dyn std::error::Error` trait
    /// object, enabling full cause-chain traversal by any
    /// `std::error::Error`-compliant reporter (e.g. `anyhow`, `error-stack`,
    /// or a custom `Display`-walking reporter).
    ///
    /// When errors are nested (`Error<Error<E>>`), each layer's `source()`
    /// points to the next inner layer, making the full chain traversable
    /// down to the root `E`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::error::Error as StdError;
    /// use std::io;
    ///
    /// let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::NotFound))
    ///     .annotate("outer operation")
    ///     .unwrap_err();
    ///
    /// // The source chain is traversable.
    /// let src = err.source().expect("source must be present");
    /// assert!(src.is::<io::Error>());
    /// ```
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

// ── Send + Sync ───────────────────────────────────────────────────────────────
//
// `Error<E>` is `Send` when `E: Send` and `Sync` when `E: Sync`. These are
// auto-traits derived by the compiler from the struct fields:
//
//   - `Cow<'static, str>` is both `Send` and `Sync`.
//   - `&'static str` is both `Send` and `Sync`.
//   - `E` contributes its own `Send`/`Sync` bounds.
//
// No manual impl is required or desirable — writing one would bypass the
// compiler's automatic verification. The compile-time assertions in
// `tests/type_checks.rs` verify the auto-trait derivation is correct for
// the representative type `Error<std::io::Error>`.
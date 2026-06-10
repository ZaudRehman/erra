//! The [`Error<E>`] wrapper type and its standard trait implementations.

#[cfg(any(feature = "alloc", feature = "std"))]
use alloc::{borrow::Cow, string::String};

use core::fmt;

/// An error type that pairs a concrete error with a context string.
///
/// `Error<E>` keeps the underlying error type `E`, so callers can inspect or
/// match on it directly without downcasting.
///
/// Values of this type are usually created through [`crate::ResultExt`].
///
/// On 64-bit targets, this wrapper adds:
///
/// - 24 bytes with `alloc` or `std` enabled (`Cow<'static, str>`),
/// - 16 bytes without allocation support (`&'static str`).
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
/// assert_eq!(err.source.kind(), io::ErrorKind::PermissionDenied);
/// assert_eq!(err.context(), "checking socket permissions");
/// ```
pub struct Error<E> {
    /// Context describing the failed operation.
    #[cfg(any(feature = "alloc", feature = "std"))]
    pub context: Cow<'static, str>,

    /// Context describing the failed operation.
    #[cfg(not(any(feature = "alloc", feature = "std")))]
    pub context: &'static str,

    /// The wrapped error value.
    ///
    /// This field is public so callers can inspect, match on, or move out the
    /// original error without an accessor.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::ResultExt;
    /// use std::io;
    ///
    /// fn handle(res: Result<(), io::Error>) {
    ///     if let Err(err) = res.annotate("network connect") {
    ///         match err.source.kind() {
    ///             io::ErrorKind::ConnectionRefused => eprintln!("server down"),
    ///             io::ErrorKind::TimedOut => eprintln!("timed out"),
    ///             _ => eprintln!("io error: {err}"),
    ///         }
    ///     }
    /// }
    /// ```
    pub source: E,
}

impl<E> Error<E> {
    /// Creates a new error with a static context string.
    ///
    /// This does not allocate.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    /// use std::io;
    ///
    /// let err = Error::new(
    ///     "loading application config",
    ///     io::Error::from(io::ErrorKind::NotFound),
    /// );
    ///
    /// assert_eq!(err.context(), "loading application config");
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

    /// Creates a new error with an owned context string.
    ///
    /// Available when `alloc` or `std` is enabled.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::Error;
    /// use std::io;
    ///
    /// let path = "/etc/app/config.toml";
    /// let err = Error::new_owned(
    ///     format!("reading config at {path}"),
    ///     io::Error::from(io::ErrorKind::NotFound),
    /// );
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    #[inline]
    pub fn new_owned(context: String, source: E) -> Self {
        Self {
            context: Cow::Owned(context),
            source,
        }
    }

    /// Returns the stored context string.
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

    /// Consumes the wrapper and returns the original error.
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
    /// let original = err.into_source();
    /// assert_eq!(original.kind(), io::ErrorKind::TimedOut);
    /// ```
    #[inline]
    pub fn into_source(self) -> E {
        self.source
    }

    /// Maps the wrapped error to another type and keeps the same context.
    ///
    /// # Example
    ///
    /// ```rust
    /// use erra::{Error, ResultExt};
    /// use std::io;
    ///
    /// #[derive(Debug)]
    /// struct DbError(String);
    ///
    /// let io_err: Error<io::Error> = Err::<(), _>(io::Error::from(io::ErrorKind::NotFound))
    ///     .annotate("reading row from disk")
    ///     .unwrap_err();
    ///
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

impl<E: fmt::Display> fmt::Display for Error<E> {
    /// Formats the error as `"{context}: {source}"`.
    ///
    /// Nested `Error` values naturally produce a longer chain.
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.context(), self.source)
    }
}

/// Formats as `Error { context: ..., source: ... }`.
impl<E: fmt::Debug> fmt::Debug for Error<E> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("context", &self.context())
            .field("source", &self.source)
            .finish()
    }
}

/// Clones both the context string and the wrapped error.
impl<E: Clone> Clone for Error<E> {
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

/// Two errors are equal when both their contexts and wrapped errors are equal.
impl<E: PartialEq> PartialEq for Error<E> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.context() == other.context() && self.source == other.source
    }
}

/// Equality follows `PartialEq`.
impl<E: Eq> Eq for Error<E> {}

#[cfg(feature = "std")]
impl<E: std::error::Error + 'static> std::error::Error for Error<E> {
    /// Returns the wrapped error as the source.
    #[inline]
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

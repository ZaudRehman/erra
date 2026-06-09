//! The [`ResultExt`] extension trait.

#[cfg(any(feature = "alloc", feature = "std"))]
use alloc::string::String;

use crate::Error;

/// Adds context to `Result<T, E>`.
///
/// Import this trait to use `.annotate(...)` and `.annotate_with(...)` on
/// `Result` values:
///
/// ```rust
/// use erra::ResultExt;
/// ```
pub trait ResultExt<T, E>: Sized {
    /// Wraps `Err(e)` with a static context string.
    ///
    /// ```rust
    /// use erra::{Result, ResultExt};
    /// use std::io;
    ///
    /// fn read_config(path: &str) -> Result<String, io::Error> {
    ///     std::fs::read_to_string(path)
    ///         .annotate("reading application config")
    /// }
    /// ```
    fn annotate(self, msg: &'static str) -> Result<T, Error<E>>;

    /// Wraps `Err(e)` with a context string built on demand.
    ///
    /// The closure runs only on the `Err` path.
    ///
    /// ```rust
    /// use erra::{Result, ResultExt};
    /// use std::io;
    ///
    /// fn read_named(path: &str) -> Result<String, io::Error> {
    ///     std::fs::read_to_string(path)
    ///         .annotate_with(|| format!("reading config at {path}"))
    /// }
    /// ```
    #[cfg(any(feature = "alloc", feature = "std"))]
    fn annotate_with<F>(self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> String;
}

impl<T, E> ResultExt<T, E> for Result<T, E> {
    /// Wraps `Err(e)` with a static context string.
    #[inline]
    fn annotate(self, msg: &'static str) -> Result<T, Error<E>> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(Error::new(msg, e)),
        }
    }

    /// Wraps `Err(e)` with a lazily-built context string.
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
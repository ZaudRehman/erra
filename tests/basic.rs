//! Core correctness tests.
//!
//! Covers the public method contracts on `ResultExt` and `Error<E>` across
//! feature tiers:
//!
//! - always available: `annotate`, `new`, `Display`, `Debug`, `Clone`,
//!   `PartialEq`, `Eq`, `into_source`, `map`, `context()`
//! - `alloc` only: `annotate_with`, `new_owned`, owned-context behavior
//! - `std` only: `std::error::Error::source()` integration
//!
//! All tests are deterministic. No I/O beyond constructing `io::Error` values.

use erra::{Error, ResultExt};
use std::io;

// ─────────────────────────────────────────────────────────────────────────────
// annotate — Ok path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn annotate_ok_returns_inner_value_unchanged() {
    let result: Result<i32, &str> = Ok(42);
    let annotated = result.annotate("irrelevant context");
    assert!(annotated.is_ok());
    assert_eq!(annotated.unwrap(), 42);
}

#[test]
fn annotate_ok_unit_type() {
    let result: Result<(), u64> = Ok(());
    assert!(result.annotate("unit passthrough").is_ok());
}

#[test]
fn annotate_ok_preserves_value_through_question_mark() {
    fn inner() -> Result<u32, erra::Error<&'static str>> {
        let v = Ok::<u32, &str>(99).annotate("step")?;
        Ok(v)
    }

    assert_eq!(inner().unwrap(), 99);
}

// ─────────────────────────────────────────────────────────────────────────────
// annotate — Err path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn annotate_err_wraps_with_correct_context() {
    let result: Result<(), &str> = Err("underlying error");
    let err = result.annotate("outer context").unwrap_err();
    assert_eq!(err.context(), "outer context");
}

#[test]
fn annotate_err_preserves_source_value() {
    let result: Result<(), &str> = Err("underlying error");
    let err = result.annotate("ctx").unwrap_err();
    assert_eq!(err.source, "underlying error");
}

#[test]
fn annotate_err_with_io_error() {
    let result: Result<(), io::Error> = Err(io::Error::from(io::ErrorKind::NotFound));
    let err = result.annotate("reading config").unwrap_err();
    assert_eq!(err.context(), "reading config");
    assert_eq!(err.source.kind(), io::ErrorKind::NotFound);
}

#[test]
fn annotate_err_propagates_via_question_mark() {
    fn try_op() -> Result<(), erra::Error<io::Error>> {
        Err::<(), io::Error>(io::Error::from(io::ErrorKind::BrokenPipe))
            .annotate("write to socket")?;
        Ok(())
    }

    let err = try_op().unwrap_err();
    assert_eq!(err.context(), "write to socket");
    assert_eq!(err.source.kind(), io::ErrorKind::BrokenPipe);
}

// ─────────────────────────────────────────────────────────────────────────────
// Display
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn display_format_is_context_colon_space_source() {
    let e = Error::new("parse step", "bad input");
    assert_eq!(e.to_string(), "parse step: bad input");
}

#[test]
fn display_with_io_error() {
    let e = Error::new("reading manifest", io::Error::from(io::ErrorKind::NotFound));

    let s = e.to_string();
    assert!(
        s.starts_with("reading manifest: "),
        "unexpected Display output: {s}"
    );
}

#[test]
fn display_with_numeric_source() {
    let e = Error::new("decode byte", 0xFFu8);
    assert_eq!(e.to_string(), "decode byte: 255");
}

// ─────────────────────────────────────────────────────────────────────────────
// Debug
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn debug_output_contains_context_field() {
    let e = Error::new("my context", 0u32);
    let s = format!("{e:?}");
    assert!(
        s.contains("my context"),
        "Debug output missing context: {s}"
    );
}

#[test]
fn debug_output_contains_source_field() {
    let e = Error::new("ctx", 42u32);
    let s = format!("{e:?}");
    assert!(s.contains("42"), "Debug output missing source: {s}");
}

#[test]
fn debug_output_names_the_struct() {
    let e = Error::new("ctx", 0u8);
    let s = format!("{e:?}");
    assert!(s.contains("Error"), "Debug output missing struct name: {s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Clone
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn clone_produces_value_equal_to_original() {
    let e = Error::new("original context", 7u8);
    let cloned = e.clone();
    assert_eq!(e, cloned);
}

// ─────────────────────────────────────────────────────────────────────────────
// PartialEq / Eq
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn partial_eq_same_context_same_source_are_equal() {
    let a = Error::new("ctx", 1u32);
    let b = Error::new("ctx", 1u32);
    assert_eq!(a, b);
}

#[test]
fn partial_eq_different_context_not_equal() {
    let a = Error::new("ctx-a", 1u32);
    let b = Error::new("ctx-b", 1u32);
    assert_ne!(a, b);
}

#[test]
fn partial_eq_different_source_not_equal() {
    let a = Error::new("ctx", 1u32);
    let b = Error::new("ctx", 2u32);
    assert_ne!(a, b);
}

#[test]
fn eq_trait_is_reflexive() {
    let e = Error::new("ctx", 100u64);
    assert_eq!(e, e.clone());
}

// ─────────────────────────────────────────────────────────────────────────────
// into_source
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn into_source_returns_original_error_discarding_context() {
    let e = Error::new("ctx", 404u32);
    let source = e.into_source();
    assert_eq!(source, 404u32);
}

#[test]
fn into_source_with_io_error_preserves_kind() {
    let e = Error::new("connect", io::Error::from(io::ErrorKind::ConnectionRefused));

    let source = e.into_source();
    assert_eq!(source.kind(), io::ErrorKind::ConnectionRefused);
}

#[test]
fn into_source_on_annotated_result() {
    let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::TimedOut))
        .annotate("upstream request")
        .unwrap_err();

    let original: io::Error = err.into_source();
    assert_eq!(original.kind(), io::ErrorKind::TimedOut);
}

// ─────────────────────────────────────────────────────────────────────────────
// map
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn map_transforms_source_to_new_type() {
    let e: Error<u32> = Error::new("transform", 1u32);
    let mapped: Error<u64> = e.map(|v| v as u64 * 2);
    assert_eq!(mapped.source, 2u64);
}

#[test]
fn map_preserves_context_string() {
    let e: Error<u32> = Error::new("preserved context", 1u32);
    let mapped: Error<String> = e.map(|v| v.to_string());
    assert_eq!(mapped.context(), "preserved context");
}

#[test]
fn map_closure_receives_source_by_value() {
    let e: Error<Vec<u8>> = Error::new("ctx", vec![1u8, 2, 3]);
    let mapped: Error<usize> = e.map(|v| v.len());
    assert_eq!(mapped.source, 3);
}

#[test]
fn map_to_different_error_type_roundtrip() {
    #[derive(Debug, PartialEq)]
    struct Wrapped(u32);

    let e = Error::new("ctx", 7u32);
    let wrapped = e.map(Wrapped);
    assert_eq!(wrapped.source, Wrapped(7));
    assert_eq!(wrapped.context(), "ctx");
}

// ─────────────────────────────────────────────────────────────────────────────
// context() accessor
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn context_accessor_returns_str_for_static_string() {
    let e = Error::new("my static context", ());
    assert_eq!(e.context(), "my static context");
}

// ─────────────────────────────────────────────────────────────────────────────
// alloc-only tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "alloc")]
mod alloc_tests {
    use super::*;

    // ─────────────────────────────────────────────────────────────────────────
    // annotate_with — Ok path
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn annotate_with_ok_closure_is_never_called() {
        let mut call_count = 0usize;

        let result: Result<i32, &str> = Ok(7);
        let annotated = result.annotate_with(|| {
            call_count += 1;
            "this must not run".to_string()
        });

        assert_eq!(call_count, 0, "closure was invoked on the Ok path");
        assert_eq!(annotated.unwrap(), 7);
    }

    #[test]
    fn annotate_with_ok_returns_inner_value_unchanged() {
        let result: Result<&str, u32> = Ok("hello");
        let v = result
            .annotate_with(|| "should not matter".to_string())
            .unwrap();
        assert_eq!(v, "hello");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // annotate_with — Err path
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn annotate_with_err_closure_is_called_exactly_once() {
        let mut call_count = 0usize;

        let result: Result<(), u32> = Err(99);
        let err = result
            .annotate_with(|| {
                call_count += 1;
                "dynamic context".to_string()
            })
            .unwrap_err();

        assert_eq!(call_count, 1, "closure should be called exactly once");
        assert_eq!(err.context(), "dynamic context");
        assert_eq!(err.source, 99u32);
    }

    #[test]
    fn annotate_with_err_closure_receives_runtime_values() {
        let path = "/var/run/app.pid";
        let attempt = 3usize;

        let result: Result<(), io::Error> = Err(io::Error::from(io::ErrorKind::PermissionDenied));

        let err = result
            .annotate_with(|| format!("attempt {attempt} reading {path}"))
            .unwrap_err();

        assert_eq!(err.context(), "attempt 3 reading /var/run/app.pid");
        assert_eq!(err.source.kind(), io::ErrorKind::PermissionDenied);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Display / Clone / PartialEq with owned context
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn display_with_dynamic_context() {
        let e = Error::new_owned("reading file at /tmp/x".to_string(), 404u32);
        assert_eq!(e.to_string(), "reading file at /tmp/x: 404");
    }

    #[test]
    fn clone_is_independent_of_original() {
        let e = Error::new_owned("owned context".to_string(), 1u32);
        let cloned = e.clone();
        assert_eq!(e.context(), cloned.context());
        assert_eq!(e.source, cloned.source);
    }

    #[test]
    fn partial_eq_static_and_owned_context_with_same_content_are_equal() {
        let a = Error::new("ctx", 1u32);
        let b = Error::new_owned("ctx".to_string(), 1u32);
        assert_eq!(a, b);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // map / context / new_owned
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn map_preserves_owned_context_string() {
        let e = Error::new_owned("owned context".to_string(), 1u32);
        let mapped: Error<u64> = e.map(|v| v as u64);
        assert_eq!(mapped.context(), "owned context");
    }

    #[test]
    fn context_accessor_returns_str_for_owned_string() {
        let e = Error::new_owned(format!("dynamic {}", "context"), 0u8);
        assert_eq!(e.context(), "dynamic context");
    }

    #[test]
    fn new_owned_stores_full_dynamic_string() {
        let ctx = format!("reading shard {} of {}", 3, 10);
        let e = Error::new_owned(ctx.clone(), 0u8);
        assert_eq!(e.context(), ctx.as_str());
    }

    #[test]
    fn new_owned_source_is_accessible() {
        let e = Error::new_owned("ctx".to_string(), 255u8);
        assert_eq!(e.source, 255u8);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// std-only tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "std")]
mod std_tests {
    use super::*;
    use std::error::Error as StdError;

    #[test]
    fn std_error_source_returns_some() {
        let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::NotFound))
            .annotate("outer operation")
            .unwrap_err();

        assert!(
            err.source().is_some(),
            "source() must return Some for Error<io::Error>"
        );
    }

    #[test]
    fn std_error_source_is_downcasts_to_original_type() {
        let err = Err::<(), io::Error>(io::Error::from(io::ErrorKind::NotFound))
            .annotate("outer operation")
            .unwrap_err();

        let src = err.source().unwrap();
        assert!(
            src.downcast_ref::<io::Error>().is_some(),
            "source() must downcast to io::Error"
        );
    }

    #[test]
    fn std_error_display_matches_context_colon_source() {
        let err = Error::new("step failed", io::Error::from(io::ErrorKind::NotFound));
        let display = err.to_string();
        assert!(display.starts_with("step failed: "));
        let _ = err.source();
    }
}

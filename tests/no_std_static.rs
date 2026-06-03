//! Validates the zero-allocation static annotation path.
//!
//! These tests run under the default `std` feature but exercise only the
//! code paths that are present in every build configuration, including
//! `--no-default-features` (bare `no_std`, no allocator). They use only
//! `core`-compatible types and patterns throughout.
//!
//! # Why this file exists
//!
//! A true `no_std` binary cannot be executed by the standard `cargo test`
//! harness — the harness itself requires `std`. The correct verification
//! strategy for `no_std` compatibility is therefore two-pronged:
//!
//! 1. **Compile check**: `cargo check --target thumbv6m-none-eabi
//!    --no-default-features` proves the crate compiles for a bare-metal
//!    target with no allocator. This is part of the CI acceptance matrix
//!    but cannot be run as a `#[test]`.
//!
//! 2. **Runtime tests under `std`**: This file runs the same logic that
//!    the static path would execute on a `no_std` target, but inside the
//!    `std` harness. Where a `no_std` binary would write to a UART, we
//!    write into a fixed-size stack buffer via `core::fmt::Write`. This
//!    gives us runtime assertion coverage of every static-path code branch
//!    without requiring a target runner.
//!
//! Every test in this file intentionally avoids:
//! - `String`, `Vec`, `Box`, or any other heap type
//! - `std::io`, `std::fs`, `std::net`, or any `std`-only API
//! - `format!` macro (which allocates) — all formatting goes into the
//!   stack `WriteBuf` defined below
//! - `annotate_with` (requires `alloc`)
//!
//! # Stack write buffer
//!
//! `WriteBuf` is a fixed-capacity `[u8; N]` buffer that implements
//! `core::fmt::Write`. It is used everywhere a formatted string is needed
//! without heap allocation. It is intentionally defined here and not
//! exported — it is test infrastructure, not library code.

use core::fmt::Write;
use erra::{Error, ResultExt};

// ─────────────────────────────────────────────────────────────────────────────
// Stack write buffer — core::fmt::Write without heap allocation
// ─────────────────────────────────────────────────────────────────────────────

/// A fixed-capacity stack-allocated write buffer.
///
/// Capacity is chosen to be large enough for all test outputs while remaining
/// entirely on the stack. A `no_std` embedded implementation would typically
/// write directly to a peripheral register; this buffer serves as the test
/// stand-in.
struct WriteBuf<const N: usize> {
    buf: [u8; N],
    pos: usize,
}

impl<const N: usize> WriteBuf<N> {
    const fn new() -> Self {
        Self {
            buf: [0u8; N],
            pos: 0,
        }
    }

    /// Returns the written portion of the buffer as a `&str`.
    ///
    /// # Panics
    ///
    /// Panics if the written bytes are not valid UTF-8. All writes in this
    /// test file originate from `fmt::Display` / `fmt::Debug` impls that
    /// produce valid UTF-8, so this never triggers in practice.
    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.pos])
            .expect("WriteBuf contains invalid UTF-8")
    }
}

impl<const N: usize> Write for WriteBuf<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        let bytes = s.as_bytes();
        let end = self.pos + bytes.len();
        if end > N {
            return Err(core::fmt::Error);
        }
        self.buf[self.pos..end].copy_from_slice(bytes);
        self.pos = end;
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Embedded-style HAL error type
//
// Defined at module scope so it is available to all test functions below,
// including those outside the embedded-pattern section.
// ─────────────────────────────────────────────────────────────────────────────

/// A simplified HAL error type that does not require `std` or heap allocation.
#[derive(Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
enum HalError {
    Timeout,
    BusError,
    InvalidAddress,
}

impl core::fmt::Display for HalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HalError::Timeout        => f.write_str("hardware timeout"),
            HalError::BusError       => f.write_str("bus error"),
            HalError::InvalidAddress => f.write_str("invalid address"),
        }
    }
}

fn read_sensor_register() -> Result<u8, HalError> {
    Err(HalError::Timeout)
}

fn calibrate_sensor() -> Result<u8, Error<HalError>> {
    read_sensor_register().annotate("reading calibration register 0x42")
}

fn init_device() -> Result<u8, Error<Error<HalError>>> {
    calibrate_sensor().annotate("device initialisation")
}

// ─────────────────────────────────────────────────────────────────────────────
// annotate — Ok path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn static_annotate_ok_is_passthrough() {
    let result: Result<u32, u32> = Ok(42);
    let v = result.annotate("irrelevant").unwrap();
    assert_eq!(v, 42);
}

#[test]
fn static_annotate_ok_unit_type() {
    let result: Result<(), u32> = Ok(());
    assert!(result.annotate("step").is_ok());
}

#[test]
fn static_annotate_ok_with_static_str_source() {
    let result: Result<u8, &'static str> = Ok(0xFF);
    let v = result.annotate("decode").unwrap();
    assert_eq!(v, 0xFF);
}

// ─────────────────────────────────────────────────────────────────────────────
// annotate — Err path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn static_annotate_err_wraps_with_correct_context() {
    let result: Result<(), u32> = Err(5);
    let err = result.annotate("static message").unwrap_err();
    assert_eq!(err.context(), "static message");
}

#[test]
fn static_annotate_err_preserves_source() {
    let result: Result<(), u32> = Err(99);
    let err = result.annotate("ctx").unwrap_err();
    assert_eq!(err.source, 99u32);
}

#[test]
fn static_annotate_err_with_u8_source() {
    let result: Result<(), u8> = Err(0xDE);
    let err = result.annotate("parse byte").unwrap_err();
    assert_eq!(err.source, 0xDE_u8);
    assert_eq!(err.context(), "parse byte");
}

#[test]
fn static_annotate_err_with_static_str_source() {
    let result: Result<(), &'static str> = Err("device not ready");
    let err = result.annotate("init sequence").unwrap_err();
    assert_eq!(err.source, "device not ready");
    assert_eq!(err.context(), "init sequence");
}

#[test]
fn static_annotate_err_with_i32_source() {
    let result: Result<(), i32> = Err(-1);
    let err = result.annotate("syscall").unwrap_err();
    assert_eq!(err.source, -1i32);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error::new constructor — static path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn new_constructor_sets_context() {
    let e = Error::new("register write failed", 0u32);
    assert_eq!(e.context(), "register write failed");
}

#[test]
fn new_constructor_sets_source() {
    let e = Error::new("dma fault", 0xDEAD_BEEFu32);
    assert_eq!(e.source, 0xDEAD_BEEFu32);
}

// ─────────────────────────────────────────────────────────────────────────────
// Display — stack write buffer, no heap
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn display_formats_as_context_colon_source() {
    let e = Error::new("step", 0u8);
    let mut buf = WriteBuf::<64>::new();
    write!(buf, "{e}").expect("WriteBuf must not overflow for this input");
    assert_eq!(buf.as_str(), "step: 0");
}

#[test]
fn display_with_static_str_source() {
    let e = Error::new("boot", "sensor offline");
    let mut buf = WriteBuf::<64>::new();
    write!(buf, "{e}").unwrap();
    assert_eq!(buf.as_str(), "boot: sensor offline");
}

#[test]
fn display_with_u32_source() {
    let e = Error::new("fault code", 0xCAFEu32);
    let mut buf = WriteBuf::<64>::new();
    write!(buf, "{e}").unwrap();
    assert_eq!(buf.as_str(), "fault code: 51966");
}

#[test]
fn display_with_i32_negative_source() {
    let e = Error::new("errno", -22i32);
    let mut buf = WriteBuf::<64>::new();
    write!(buf, "{e}").unwrap();
    assert_eq!(buf.as_str(), "errno: -22");
}

#[test]
fn display_chained_two_layers_no_heap() {
    let err = Err::<(), u8>(7)
        .annotate("inner")
        .annotate("outer")
        .unwrap_err();

    let mut buf = WriteBuf::<128>::new();
    write!(buf, "{err}").unwrap();

    let s = buf.as_str();
    assert!(s.starts_with("outer:"), "outermost context must lead: {s}");
    assert!(s.contains("inner"), "inner context must be present: {s}");
    assert!(s.contains('7'), "root error value must appear: {s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Debug — stack write buffer, no heap
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn debug_contains_context_field() {
    let e = Error::new("my context", 0u8);
    let mut buf = WriteBuf::<128>::new();
    write!(buf, "{e:?}").unwrap();
    let s = buf.as_str();
    assert!(s.contains("my context"), "Debug missing context: {s}");
}

#[test]
fn debug_contains_source_field() {
    let e = Error::new("ctx", 42u32);
    let mut buf = WriteBuf::<128>::new();
    write!(buf, "{e:?}").unwrap();
    let s = buf.as_str();
    assert!(s.contains("42"), "Debug missing source value: {s}");
}

#[test]
fn debug_names_the_struct() {
    let e = Error::new("ctx", 0u8);
    let mut buf = WriteBuf::<128>::new();
    write!(buf, "{e:?}").unwrap();
    let s = buf.as_str();
    assert!(s.contains("Error"), "Debug output missing struct name: {s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// context() accessor — static path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn context_accessor_returns_correct_str() {
    let e = Error::new("interrupt handler", 0xFFu8);
    assert_eq!(e.context(), "interrupt handler");
}

#[test]
fn context_accessor_on_annotated_result() {
    let err = Err::<(), u32>(1)
        .annotate("context string")
        .unwrap_err();
    assert_eq!(err.context(), "context string");
}

// ─────────────────────────────────────────────────────────────────────────────
// into_source — static path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn into_source_recovers_original_value() {
    let e = Error::new("ctx", 0xABu8);
    assert_eq!(e.into_source(), 0xABu8);
}

#[test]
fn into_source_on_annotated_result() {
    let source = Err::<(), u32>(404)
        .annotate("not found")
        .unwrap_err()
        .into_source();
    assert_eq!(source, 404u32);
}

// ─────────────────────────────────────────────────────────────────────────────
// map — static path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn map_transforms_source_preserves_context_no_heap() {
    let e = Error::new("transform", 2u32);
    let mapped: Error<u64> = e.map(|v| v as u64 * 10);
    assert_eq!(mapped.source, 20u64);
    assert_eq!(mapped.context(), "transform");
}

#[test]
fn map_to_static_str_no_heap() {
    let e = Error::new("classify", 0u8);
    let mapped: Error<&'static str> = e.map(|v| if v == 0 { "zero" } else { "nonzero" });
    assert_eq!(mapped.source, "zero");
    assert_eq!(mapped.context(), "classify");
}

// ─────────────────────────────────────────────────────────────────────────────
// Clone / PartialEq — static path with cloneable E
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn clone_on_static_path_produces_equal_value() {
    let e = Error::new("ctx", 7u8);
    let cloned = e.clone();
    assert_eq!(e, cloned);
}

#[test]
fn partial_eq_on_static_path() {
    let a = Error::new("ctx", 1u32);
    let b = Error::new("ctx", 1u32);
    let c = Error::new("ctx", 2u32);
    assert_eq!(a, b);
    assert_ne!(a, c);
}

// ─────────────────────────────────────────────────────────────────────────────
// ? operator — static path
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn question_mark_propagates_annotated_error() {
    fn inner() -> Result<(), u32> {
        Err(77)
    }

    fn outer() -> Result<(), Error<u32>> {
        inner().annotate("outer step")?;
        Ok(())
    }

    let err = outer().unwrap_err();
    assert_eq!(err.context(), "outer step");
    assert_eq!(err.source, 77u32);
}

#[test]
fn question_mark_passes_through_ok() {
    fn inner() -> Result<u32, u32> {
        Ok(42)
    }

    fn outer() -> Result<u32, Error<u32>> {
        let v = inner().annotate("outer step")?;
        Ok(v)
    }

    assert_eq!(outer().unwrap(), 42);
}

// ─────────────────────────────────────────────────────────────────────────────
// Embedded-style error pattern
//
// Simulates a realistic embedded use case: a flat enum error type, no heap,
// no std, static annotations at call sites, and a match on the source field
// at the handler site.
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn embedded_pattern_context_is_correct_at_each_layer() {
    let err = init_device().unwrap_err();
    assert_eq!(err.context(), "device initialisation");
    assert_eq!(err.source.context(), "reading calibration register 0x42");
    assert_eq!(err.source.source, HalError::Timeout);
}

#[test]
fn embedded_pattern_match_on_source_without_downcast() {
    let err = init_device().unwrap_err();

    let root: HalError = err.into_source().into_source();
    match root {
        HalError::Timeout        => { /* handled */ }
        HalError::BusError       => panic!("unexpected bus error"),
        HalError::InvalidAddress => panic!("unexpected invalid address"),
    }
}

#[test]
fn embedded_pattern_display_no_heap() {
    let err = init_device().unwrap_err();
    let mut buf = WriteBuf::<256>::new();
    write!(buf, "{err}").unwrap();
    let s = buf.as_str();

    assert!(s.contains("device initialisation"), "{s}");
    assert!(s.contains("reading calibration register 0x42"), "{s}");
    assert!(s.contains("hardware timeout"), "{s}");
}

#[test]
fn embedded_pattern_clone_and_eq_on_hal_error() {
    let err = calibrate_sensor().unwrap_err();
    let cloned = err.clone();
    assert_eq!(err, cloned);
}
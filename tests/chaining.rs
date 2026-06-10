//! Tests for nested `Error<Error<E>>` chains.
//!
//! Covers:
//! - Nested type structure and field accessibility across layers
//! - `Display` ordering guarantee: outermost context first, innermost last
//! - `std::error::Error::source()` chain traversal to the root error
//! - Three-layer and four-layer chains
//! - `map` preserving context across a type boundary in a chain
//! - `into_source` unwinding a chain one layer at a time
//! - `annotate_with` at any layer in a chain (requires `alloc`)
//! - `std::error::Error` source chain (requires `std`)

use erra::ResultExt;
use std::io;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
//
// These functions represent a realistic layered call stack. Each layer
// annotates with its own context string and calls the layer below it.
// ─────────────────────────────────────────────────────────────────────────────

fn leaf_operation() -> Result<(), io::Error> {
    Err(io::Error::from(io::ErrorKind::NotFound))
}

fn middle_layer() -> Result<(), erra::Error<io::Error>> {
    leaf_operation().annotate("middle: reading file from disk")
}

fn outer_layer() -> Result<(), erra::Error<erra::Error<io::Error>>> {
    middle_layer().annotate("outer: loading application config")
}

fn top_layer() -> Result<(), erra::Error<erra::Error<erra::Error<io::Error>>>> {
    outer_layer().annotate("top: initialising subsystem")
}

// ─────────────────────────────────────────────────────────────────────────────
// Two-layer chain — structure
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn two_layer_outer_context_is_correct() {
    let err = outer_layer().unwrap_err();
    assert_eq!(err.context(), "outer: loading application config");
}

#[test]
fn two_layer_inner_context_is_correct() {
    let err = outer_layer().unwrap_err();
    assert_eq!(err.source.context(), "middle: reading file from disk");
}

#[test]
fn two_layer_root_error_kind_is_correct() {
    let err = outer_layer().unwrap_err();
    assert_eq!(err.source.source.kind(), io::ErrorKind::NotFound);
}

#[test]
fn two_layer_fields_are_directly_accessible_without_methods() {
    let err = outer_layer().unwrap_err();
    let _middle: &erra::Error<io::Error> = &err.source;
    let _leaf: &io::Error = &err.source.source;
}

// ─────────────────────────────────────────────────────────────────────────────
// Two-layer chain — Display ordering
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn two_layer_display_outermost_context_appears_first() {
    let err = outer_layer().unwrap_err();
    let s = err.to_string();
    assert!(
        s.starts_with("outer: loading application config"),
        "outermost context must be first in Display output: {s}"
    );
}

#[test]
fn two_layer_display_inner_context_appears_after_outer() {
    let err = outer_layer().unwrap_err();
    let s = err.to_string();

    let outer_pos = s
        .find("outer: loading application config")
        .expect("outer context missing from Display");
    let inner_pos = s
        .find("middle: reading file from disk")
        .expect("inner context missing from Display");

    assert!(
        outer_pos < inner_pos,
        "outer context must appear before inner context in Display output: {s}"
    );
}

#[test]
fn two_layer_display_all_levels_present() {
    let err = outer_layer().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("outer: loading application config"), "{s}");
    assert!(s.contains("middle: reading file from disk"), "{s}");
    assert!(
        s.len() > "outer: loading application config: middle: reading file from disk: ".len(),
        "io::Error Display should contribute text beyond the erra context strings: {s}"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// Inline chaining via .annotate().annotate()
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn inline_chain_display_ordering_is_last_annotate_first() {
    let err = leaf_operation()
        .annotate("layer 1")
        .annotate("layer 2")
        .annotate("layer 3")
        .unwrap_err();

    let s = err.to_string();

    assert!(
        s.starts_with("layer 3:"),
        "last annotation is the outermost wrapper and must appear first: {s}"
    );

    let pos1 = s.find("layer 1").unwrap();
    let pos2 = s.find("layer 2").unwrap();
    let pos3 = s.find("layer 3").unwrap();

    assert!(pos3 < pos2, "layer 3 must precede layer 2: {s}");
    assert!(pos2 < pos1, "layer 2 must precede layer 1: {s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// into_source unwinding
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn into_source_unwinds_one_layer_at_a_time() {
    let err = outer_layer().unwrap_err();

    let middle: erra::Error<io::Error> = err.into_source();
    assert_eq!(middle.context(), "middle: reading file from disk");

    let root: io::Error = middle.into_source();
    assert_eq!(root.kind(), io::ErrorKind::NotFound);
}

#[test]
fn into_source_on_three_layer_chain_reaches_root() {
    let err = top_layer().unwrap_err();

    let l2 = err.into_source();
    let l1 = l2.into_source();
    let root: io::Error = l1.into_source();

    assert_eq!(root.kind(), io::ErrorKind::NotFound);
}

// ─────────────────────────────────────────────────────────────────────────────
// map across a chain boundary
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn map_on_inner_layer_preserves_outer_context() {
    let err = outer_layer().unwrap_err();

    let mapped = err.map(|inner| inner.map(|e| e.to_string()));

    assert_eq!(mapped.context(), "outer: loading application config");
    assert_eq!(mapped.source.context(), "middle: reading file from disk");
}

// ─────────────────────────────────────────────────────────────────────────────
// Non-std error type in a chain
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn chain_works_with_non_error_source_type() {
    let err = Err::<(), u32>(42)
        .annotate("inner numeric error")
        .annotate("outer wrapper")
        .unwrap_err();

    assert_eq!(err.context(), "outer wrapper");
    assert_eq!(err.source.context(), "inner numeric error");
    assert_eq!(err.source.source, 42u32);
}

#[test]
fn chain_display_with_non_error_source_type() {
    let err = Err::<(), u32>(42)
        .annotate("inner")
        .annotate("outer")
        .unwrap_err();

    let s = err.to_string();
    assert!(s.starts_with("outer:"), "{s}");
    assert!(s.contains("inner"), "{s}");
    assert!(s.contains("42"), "{s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// Three-layer chain — always available tests
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn three_layer_display_starts_with_outermost_context() {
    let err = top_layer().unwrap_err();
    let s = err.to_string();
    assert!(
        s.starts_with("top: initialising subsystem"),
        "outermost context must lead Display output: {s}"
    );
}

#[test]
fn three_layer_display_contains_all_three_contexts() {
    let err = top_layer().unwrap_err();
    let s = err.to_string();
    assert!(s.contains("top: initialising subsystem"), "{s}");
    assert!(s.contains("outer: loading application config"), "{s}");
    assert!(s.contains("middle: reading file from disk"), "{s}");
}

#[test]
fn three_layer_display_ordering_is_outermost_to_innermost() {
    let err = top_layer().unwrap_err();
    let s = err.to_string();

    let top_pos = s.find("top: initialising subsystem").unwrap();
    let outer_pos = s.find("outer: loading application config").unwrap();
    let middle_pos = s.find("middle: reading file from disk").unwrap();

    assert!(top_pos < outer_pos, "top must precede outer: {s}");
    assert!(outer_pos < middle_pos, "outer must precede middle: {s}");
}

// ─────────────────────────────────────────────────────────────────────────────
// alloc-only tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "alloc")]
mod alloc_tests {
    use super::*;

    #[test]
    fn annotate_with_at_inner_layer_dynamic_context_is_preserved() {
        let shard = 7u32;
        let err = leaf_operation()
            .annotate_with(|| format!("reading shard {shard}"))
            .annotate("loading dataset")
            .unwrap_err();

        assert_eq!(err.context(), "loading dataset");
        assert_eq!(err.source.context(), "reading shard 7");
    }

    #[test]
    fn annotate_with_at_outer_layer_dynamic_context_is_preserved() {
        let job_id = "job-abc-123";
        let err = leaf_operation()
            .annotate("reading input file")
            .annotate_with(|| format!("processing {job_id}"))
            .unwrap_err();

        assert_eq!(err.context(), format!("processing {job_id}"));
        assert_eq!(err.source.context(), "reading input file");
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
    fn two_layer_source_at_outer_returns_some() {
        let err = outer_layer().unwrap_err();
        assert!(
            err.source().is_some(),
            "source() must return Some at the outer layer"
        );
    }

    #[test]
    fn two_layer_source_chain_reaches_root_io_error() {
        let err = outer_layer().unwrap_err();

        let mid = err.source().expect("source() at outer layer must be Some");
        let leaf = mid.source().expect("source() at middle layer must be Some");

        assert!(
            leaf.downcast_ref::<io::Error>().is_some(),
            "root source must downcast to io::Error"
        );
    }

    #[test]
    fn two_layer_source_chain_full_walk() {
        let err = outer_layer().unwrap_err();
        let boxed: &dyn StdError = &err;

        let mut depth = 0usize;
        let mut current: Option<&dyn StdError> = Some(boxed);
        while let Some(e) = current {
            depth += 1;
            current = e.source();
        }

        // Error<Error<io::Error>> -> Error<io::Error> -> io::Error -> None = 3
        assert_eq!(depth, 3, "chain depth must be 3");
    }

    #[test]
    fn three_layer_source_chain_depth_is_four() {
        let err = top_layer().unwrap_err();
        let boxed: &dyn StdError = &err;

        let mut depth = 0usize;
        let mut current: Option<&dyn StdError> = Some(boxed);
        while let Some(e) = current {
            depth += 1;
            current = e.source();
        }

        // top -> outer -> middle -> io::Error -> None = 4
        assert_eq!(depth, 4, "chain depth must be 4");
    }

    #[test]
    fn inline_chain_four_layers_depth_is_five() {
        let err = leaf_operation()
            .annotate("a")
            .annotate("b")
            .annotate("c")
            .annotate("d")
            .unwrap_err();

        let boxed: &dyn StdError = &err;
        let mut depth = 0usize;
        let mut current: Option<&dyn StdError> = Some(boxed);
        while let Some(e) = current {
            depth += 1;
            current = e.source();
        }

        // d -> c -> b -> a -> io::Error -> None = 5
        assert_eq!(depth, 5);
    }
}

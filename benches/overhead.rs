//! Benchmark: `erra` annotation overhead vs bare `?` on the hot path.
//!
//! # What this measures
//!
//! The central performance claim of `erra` is:
//!
//! > On the `Ok` path, `.annotate(...)` and `.annotate_with(...)` compile
//! > to a zero-cost identity pass-through in release builds. The caller
//! > pays nothing beyond what a bare `?` would cost.
//!
//! This benchmark exists to verify that claim is not broken by a future
//! change — a new field on `Error<E>`, a dropped `#[inline]`, a changed
//! match structure, or an updated LLVM codegen strategy. If the `Ok` path
//! benchmarks start diverging from the bare `?` baseline, that is a
//! regression worth investigating regardless of the absolute nanosecond
//! values.
//!
//! # Benchmark groups
//!
//! | Group | What it isolates |
//! |---|---|
//! | `ok_path` | `Ok` pass-through cost: bare `?` vs annotate vs annotate_with |
//! | `err_path` | `Err` wrapping cost: static context vs dynamic context |
//! | `chain` | Cost of multi-layer annotation chains on the `Err` path |
//! | `throughput` | Sustained annotate throughput across a tight loop |
//!
//! # Interpreting results
//!
//! In a release build with LTO, all three `ok_path` benchmarks should
//! report times that are statistically indistinguishable from each other.
//! Any meaningful difference (> 1ns on modern hardware) indicates that the
//! `#[inline]` on the hot path has stopped working and the optimizer is
//! no longer eliminating the dead `Err` branch.
//!
//! The `err_path` benchmarks will show measurable cost — that is expected
//! and correct. The `Err` path allocates a `Cow` and copies `E`. The
//! question is whether that cost is proportionate and stable.
//!
//! # Running
//!
//! ```text
//! cargo bench
//! cargo bench -- ok_path           # run only the ok_path group
//! cargo bench -- --save-baseline main
//! cargo bench -- --baseline main   # compare against saved baseline
//! ```

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use erra::ResultExt;
use std::io;

// ─────────────────────────────────────────────────────────────────────────────
// Source functions
//
// `#[inline(never)]` is mandatory here. Without it, the compiler may inline
// these into the benchmark loop body and constant-fold the entire expression,
// producing a measurement of zero rather than the real hot-path cost.
// black_box() on inputs prevents constant propagation; #[inline(never)] on
// the source function prevents the call itself from being erased.
// ─────────────────────────────────────────────────────────────────────────────

#[inline(never)]
fn make_ok(v: i32) -> Result<i32, io::Error> {
    Ok(v)
}

#[inline(never)]
fn make_err() -> Result<i32, io::Error> {
    Err(io::Error::from(io::ErrorKind::NotFound))
}

#[allow(dead_code)]
#[inline(never)]
fn make_ok_u32(v: u32) -> Result<u32, u32> {
    Ok(v)
}

#[inline(never)]
fn make_err_u32(code: u32) -> Result<u32, u32> {
    Err(code)
}

// ─────────────────────────────────────────────────────────────────────────────
// ok_path group
//
// All three benchmarks operate on Ok(42). In release builds with LTO, all
// three should be statistically indistinguishable from each other.
// ─────────────────────────────────────────────────────────────────────────────

fn bench_ok_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("ok_path");

    // Baseline: the cost of unwrapping a plain Ok result with no annotation.
    group.bench_function("bare_unwrap", |b| {
        b.iter(|| {
            let r = make_ok(black_box(42));
            r.unwrap()
        })
    });

    // Annotate with a static string on the Ok path.
    // The annotation string must never be evaluated — it is baked into the
    // binary as a static reference and the Err branch that would use it is
    // dead code on this path.
    group.bench_function("annotate_static_on_ok", |b| {
        b.iter(|| {
            let r = make_ok(black_box(42))
                .annotate("reading application config");
            r.unwrap()
        })
    });

    // Annotate with a closure on the Ok path.
    // The closure body — including the format! call — must never execute.
    // The closure object itself may be constructed and immediately dropped,
    // but with LTO the optimizer should eliminate it entirely.
    group.bench_function("annotate_with_closure_on_ok", |b| {
        b.iter(|| {
            let r = make_ok(black_box(42))
                .annotate_with(|| format!("reading file at path {}", black_box("/etc/app.toml")));
            r.unwrap()
        })
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// err_path group
//
// These benchmarks measure the actual wrapping cost on the Err path. This
// is expected to be non-zero: we are constructing a Cow<'static, str>,
// copying E, and boxing the result. The goal is that these costs are
// proportionate and stable across releases.
// ─────────────────────────────────────────────────────────────────────────────

fn bench_err_path(c: &mut Criterion) {
    let mut group = c.benchmark_group("err_path");

    // Baseline: unwrap_err on a plain Err — the minimum cost of handling an
    // Err value with no annotation. Measures Err construction + unwrap_err.
    group.bench_function("bare_unwrap_err", |b| {
        b.iter(|| {
            let r = make_err();
            r.unwrap_err()
        })
    });

    // Static annotation on Err: constructs Cow::Borrowed — zero heap alloc.
    // Cost beyond the bare baseline is: one Cow::Borrowed construction
    // (pointer + length from the &'static str) and one Error<E> struct init.
    group.bench_function("annotate_static_on_err", |b| {
        b.iter(|| {
            let r = make_err().annotate("reading config");
            r.unwrap_err()
        })
    });

    // Dynamic annotation on Err: invokes the closure and constructs
    // Cow::Owned — one String heap allocation. This is the maximum cost
    // of using erra on the error path.
    group.bench_function("annotate_with_closure_on_err", |b| {
        b.iter(|| {
            let r = make_err()
                .annotate_with(|| format!("reading config at {}", black_box("/etc/app.toml")));
            r.unwrap_err()
        })
    });

    // u32 error — smaller E type. Verifies that error size affects wrapping
    // cost proportionately, and that there is no unexpected per-call overhead
    // independent of E size.
    group.bench_function("annotate_static_on_err_u32", |b| {
        b.iter(|| {
            let r = make_err_u32(black_box(404)).annotate("step");
            r.unwrap_err()
        })
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// chain group
//
// Measures the marginal cost of each additional annotation layer on the
// Err path. Useful for callers who annotate across many layers and want to
// understand when annotation cost becomes a factor.
// ─────────────────────────────────────────────────────────────────────────────

fn bench_chain(c: &mut Criterion) {
    let mut group = c.benchmark_group("chain");

    group.bench_function("one_layer", |b| {
        b.iter(|| {
            make_err_u32(black_box(1))
                .annotate("layer 1")
                .unwrap_err()
        })
    });

    group.bench_function("two_layers", |b| {
        b.iter(|| {
            make_err_u32(black_box(1))
                .annotate("layer 1")
                .annotate("layer 2")
                .unwrap_err()
        })
    });

    group.bench_function("three_layers", |b| {
        b.iter(|| {
            make_err_u32(black_box(1))
                .annotate("layer 1")
                .annotate("layer 2")
                .annotate("layer 3")
                .unwrap_err()
        })
    });

    group.bench_function("four_layers", |b| {
        b.iter(|| {
            make_err_u32(black_box(1))
                .annotate("layer 1")
                .annotate("layer 2")
                .annotate("layer 3")
                .annotate("layer 4")
                .unwrap_err()
        })
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// throughput group
//
// Measures sustained annotate throughput across a batch of operations.
// Uses Criterion's Throughput API to report in elements/second, which is
// more meaningful than raw nanoseconds for batch-processing callers.
// ─────────────────────────────────────────────────────────────────────────────

fn bench_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("throughput");

    // Batch sizes represent realistic call patterns:
    // - 100:   tight inner loop in a parser or codec
    // - 1_000: request handling loop in a server
    // - 10_000: bulk data processing pipeline
    for batch_size in [100u64, 1_000, 10_000] {
        group.throughput(Throughput::Elements(batch_size));

        group.bench_with_input(
            BenchmarkId::new("ok_annotate_batch", batch_size),
            &batch_size,
            |b, &n| {
                b.iter(|| {
                    let mut sum = 0i32;
                    for i in 0..n {
                        let v = make_ok(black_box(i as i32))
                            .annotate("batch step")
                            .unwrap();
                        sum = sum.wrapping_add(v);
                    }
                    black_box(sum)
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("err_annotate_batch", batch_size),
            &batch_size,
            |b, &n| {
                b.iter(|| {
                    let mut count = 0usize;
                    for _ in 0..n {
                        let e = make_err_u32(black_box(1))
                            .annotate("batch err step")
                            .unwrap_err();
                        // Touch the result so the compiler cannot eliminate
                        // the allocation.
                        count += e.context().len();
                    }
                    black_box(count)
                })
            },
        );
    }

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// display_cost group
//
// Measures the cost of formatting an annotated error via Display and Debug.
// This is relevant for logging and error reporting code paths that format
// errors on every occurrence.
// ─────────────────────────────────────────────────────────────────────────────

fn bench_display_cost(c: &mut Criterion) {
    let mut group = c.benchmark_group("display_cost");

    group.bench_function("display_one_layer", |b| {
        let err = make_err()
            .annotate("reading config")
            .unwrap_err();
        b.iter(|| {
            let s = black_box(&err).to_string();
            black_box(s)
        })
    });

    group.bench_function("display_two_layers", |b| {
        let err = make_err()
            .annotate("inner layer")
            .annotate("outer layer")
            .unwrap_err();
        b.iter(|| {
            let s = black_box(&err).to_string();
            black_box(s)
        })
    });

    group.bench_function("debug_one_layer", |b| {
        let err = make_err()
            .annotate("reading config")
            .unwrap_err();
        b.iter(|| {
            let s = format!("{:?}", black_box(&err));
            black_box(s)
        })
    });

    group.finish();
}

// ─────────────────────────────────────────────────────────────────────────────
// Criterion entry point
// ─────────────────────────────────────────────────────────────────────────────

criterion_group!(
    benches,
    bench_ok_path,
    bench_err_path,
    bench_chain,
    bench_throughput,
    bench_display_cost,
);
criterion_main!(benches);
# Rust maintainer audit — 2026-09-27

Baseline revision: `1d19b08d8423d45fcce2168ca09380aee8938be3`; working tree initially clean.

## Scope and compatibility

Inspected both workspace manifests, lockfile, pinned toolchain, contribution
instructions, Rust/Python/release CI, all production Rust modules, relevant Rust
and Python callers/tests, and existing benchmark designs. No `AGENTS.md` or local
`.cargo` configuration was found. There was no existing Graphify graph; inspection
used the actual source and callers.

Both crates use edition 2024 and the pinned Rust 1.98.1 toolchain; neither declares
`rust-version` or project-defined features. The core has no production dependencies
and forbids unsafe code. The adapter produces `rlib`/`cdylib` artifacts with generated
Python/Polars FFI. Dependency contracts were checked against local locked sources:
Polars 0.55.2, PyO3 0.29.2, pyo3-polars 0.28.0 and its derive implementation 0.22.0.
The enabled Polars integer/date/datetime/decimal type features remain unchanged.

No public APIs, trait bounds, dependencies, feature settings, toolchain, target
support, serialization, tie rules or error contracts changed. Scoped capacity
workers remain bounded and joined; no async executor or application locks exist
in the handwritten production code. The only handwritten unsafe operations found
are the existing benchmark allocator's forwarding calls. Generated and dependency
unsafe code was not comprehensively audited.

## Implemented changes

### B — Benchmark instrumentation correctness and allocator contract

`crates/intervals-core/benches/support/allocations.rs`, `allocated` and `measure`:

- `LIVE.fetch_add(size, Relaxed) + size` could panic with overflow checks enabled.
  A safe reproduction calls `freed(16)` followed by `allocated(16)` inside a
  measurement, modeling release of a preexisting allocation. The atomic wraps,
  but the ordinary addition previously panicked. `wrapping_add` now matches the
  atomic arithmetic without an unwinding path through the allocation hook.
- A measured closure panic left `ENABLED` true. A small drop guard now disables
  tracking on unwind and at the same point as before on successful completion.
- The integration test `tests/allocation_accounting.rs` imports the helper without
  registering it as the global allocator. One serialized test checks ordinary
  peak/count behavior, wrapping, and panic cleanup. Both defects were reproduced
  against the original helper through safe calls, without inducing actual UB.

This is a benchmark-helper fix, not an observed production-library crash.
Statistics still require non-overlapping measurements, retained preexisting
allocations, no unrelated allocator activity, and joined workers. Pointer identity
tracking was deliberately not added. Atomic orderings were not changed.

References: checklist sections 3, 7, 8; [R6/R7 GlobalAlloc safety contract](https://doc.rust-lang.org/std/alloc/trait.GlobalAlloc.html#safety),
[R6 Drop](https://doc.rust-lang.org/std/ops/trait.Drop.html).

### A — Documentation of existing unsafe forwarding

Added specific safety explanations at the four existing `System` calls, including
layout, pointer ownership and reallocation failure requirements. No unsafe
operations or production/test lint suppressions were added. The standalone
measurement runners suppress unused helpers from their shared reference modules.
No runtime effect is claimed or measured for the safety comments.

### C — Measured reductions in scratch-buffer lifetime

- `containment.rs::containment_counts`: save the coordinate count and release
  `coords` after ranks have been extracted, before allocating the Fenwick tree.
- `weighted.rs::max_weight_non_overlapping`: release `sorted_ends` after predecessor
  construction, before allocating DP scores. Initial allocation order is retained.

Both buffers are unused afterwards. Endpoint comparison order, validation, mask
selection and errors remain unchanged. `T: Copy` excludes custom element
destructors; the intentional change is when scratch heap storage is released.
Existing exhaustive/property/boundary tests cover the algorithms, and measurement
harnesses also check results outside timing.

References: checklist sections 1, 5, 10; [R3 heap allocations](https://nnethercote.github.io/perf-book/heap-allocations.html),
[R6 Copy/Drop exclusivity](https://doc.rust-lang.org/std/ops/trait.Drop.html#copy-and-drop-are-exclusive).

## Measurements

Rust 1.98.1, `x86_64-pc-windows-msvc`, AMD Ryzen 9 3900X, Windows 11;
standalone optimized Rust builds (`rustc -O` for weighted;
`-C opt-level=3 -C debug-assertions=off -C overflow-checks=off` for containment),
edition 2024, default portable CPU settings and no new features. Exact baseline
and changed functions run in the same executable.
Existing benchmark fixtures/oracles and allocation instrumentation are reused;
the historical weighted/containment harnesses time separate reference algorithms,
so these focused runners measure the changed production functions directly.

Inputs, verification and returned-output destruction are outside timing; scratch
allocation/destruction is inside. Allocation tracking is a separate untimed pass,
disabled during runtime samples. Memory means peak live requested heap bytes,
including output but excluding inputs, allocator overhead and RSS. No concurrent
Cargo builds or agent benchmarks ran during these measurements.

Representative results at 1,000,000 rows:

| Workload | Peak bytes before → after | Allocation events | Median time before → after |
|---|---:|---:|---:|
| Containment, disjoint, i64 | 48,000,008 → 40,000,008 (−16.67%) | 4 → 4 | 72.5730 → 73.6537 ms |
| Containment, disjoint, i32 | 36,000,008 → 32,000,008 (−11.11%) | 4 → 4 | 66.8998 → 67.1408 ms |
| Containment, shuffled, i64 | 48,000,008 → 40,000,008 (−16.67%) | 4 → 4 | 183.6356 → 179.4258 ms |
| Containment, duplicates, i64 | 40,000,016 → 40,000,000 (16 bytes) | 4 → 4 | 17.2479 → 17.5497 ms |
| Weighted, shuffled disjoint, positive i64, run 1 | 41,000,016 → 33,000,016 (−19.51%) | 6 → 6 | 99.212 → 101.044 ms |
| Same weighted workload, run 2 | 41,000,016 → 33,000,016 (−19.51%) | 6 → 6 | 91.068 → 91.582 ms |

Containment covers 14 geometries × i32/i64 × 1K/100K/1M rows: 84 workloads,
two warmups and nine accepted samples per version, 1,512 samples total. Weighted
covers three geometries × positive/mixed weights × sorted/shuffled × 1K/100K/1M:
36 workloads, two warmups and five samples per version in each of two runs,
720 samples total. Candidate order varies. All correctness gates passed.

No allocation was eliminated; these changes reduce overlap between live buffers.
Timing includes both slower and faster cases and does not establish a runtime
speedup. Duplicate-heavy containment barely benefits because coordinate
compression remains its peak phase. Polars/Python end-to-end timing, RSS,
compile-time impact and binary-size impact were **not measured**.

Raw samples, per-workload ranges, harness source, exact build/replay commands and
source hashes are in the adjacent `rust-audit-containment-*` and
`rust-audit-weighted-*` artifacts. Results are specific to this host and workload.

## Verification

Baseline:

- `cargo fmt --all -- --check`: passed.
- `cargo test -p intervals-core --locked`: 164 tests and nine doctests passed.
- `cargo test --workspace --locked --lib --tests`: blocked before tests because
  PyO3 could not find Python. After setting `PYO3_PYTHON`,
  `cargo test --workspace --locked` failed compiling adapter tests with missing
  `.rlib` / E0463 errors in the existing target directory. No failing behavioral
  assertion was observed. The cause of those artifacts was not established.

The standalone safe allocator regression baseline was compiled with
`rustc --edition=2024 --test target/audit-allocator/baseline.rs -o target/audit-allocator/baseline.exe`;
`target/audit-allocator/baseline.exe --test-threads=1 --nocapture` produced the
two expected failures described above. The fixed integration wrapper was also
compiled directly with `rustc --edition=2024 --test -D warnings` and with the
same flags plus `-O`; both executable runs passed before Cargo validation.

Successful final checks:

| Exact command | Outcome |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `git diff --check` | Passed; only Git's line-ending notices |
| `cargo check --workspace --all-targets --locked` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 202 tests and 13 doctests passed with clean-target setup below |
| `cargo test -p intervals-core --locked --release` | 165 tests and nine doctests passed |
| `cargo test -p intervals-core --locked --test allocation_accounting` | Final regression passed |
| `cargo test -p intervals-core --locked --release --test allocation_accounting` | Final optimized regression passed |
| `cargo doc --workspace --no-deps --locked` with `RUSTDOCFLAGS=-D warnings` | Passed |

New failure during validation, fixed: the regression module's import was unused
when Clippy checked `harness=false` benchmarks. Moving it inside the test fixes
that without a suppression. An intermediate relative import typo also failed
compilation and was corrected. All-target checks and debug/release regression
tests were rerun successfully. This test-only adjustment does not affect measured
code; benchmark metadata identifies the source hashes used for measurement.

Windows setup for adapter checks (PowerShell):

```powershell
$env:PYO3_PYTHON = (Resolve-Path .venv/Scripts/python.exe).Path
$auditPythonBase = & .venv/Scripts/python.exe -c 'import sys; print(sys.base_prefix)'
$env:PATH = "$auditPythonBase;$env:PATH"
```

The successful complete workspace test used a fresh external target directory
and disabled debug information/incremental artifacts to limit disk use. Rust
debug assertions and overflow checks stayed enabled. These were process-local
verification settings; repository profiles were not changed:

```powershell
$env:CARGO_TARGET_DIR = Join-Path $env:TEMP 'polars-intervals-rust-audit-20260927'
$env:CARGO_PROFILE_DEV_DEBUG = '0'
$env:CARGO_INCREMENTAL = '0'
cargo test --workspace --locked
```

Check, Clippy and rustdoc also passed in the ordinary repository target directory
with the normal development profile. The complete core release run used Cargo's
unchanged release profile. Benchmark build/run commands and safe direct `rustc`
allocator reproduction are separate from these Cargo checks.

Python verification used the same Python/target-directory environment above:

- `uv sync --locked`: rebuilt and installed the changed plugin successfully.
  A cross-drive hardlink warning caused a successful copy fallback.
- `uv run --locked pytest --doctest-modules python/polars_intervals tests`:
  **1,560 passed** on CPython 3.14.0 and Polars 1.44.2. The runner rebuilt once
  more after the Rust regression-test source adjustment; the final tests used
  the rebuilt plugin. These are correctness tests, not end-to-end benchmarks.

Not run: Linux/macOS/other architectures, Python 3.12/3.13, full workspace release
tests, release-wheel matrix, all historical benchmark combinations, Loom, or Miri.
`cargo miri --version` reported that Miri is unavailable for the pinned
`1.98.1-x86_64-pc-windows-msvc` toolchain. No custom synchronization changed and
no Loom dependency was added. There are no project-defined Cargo feature
combinations to test. `cargo test --all-targets` was deliberately avoided because
the custom benchmark binaries require optimized execution; all targets were
instead compiled by check/Clippy. Workspace doctests ran as part of the workspace
test command, so a separate `cargo test --doc` was unnecessary.

## Deliberately unchanged

- Adapter `Cow` input borrowing and owned output transfer already avoid needless
  copies. Multi-chunk collection supports the existing slice APIs.
- `Vec<i128>` weight normalization preserves all supported integer values and
  avoids endpoint × weight monomorphization. Removing it needs runtime and
  compilation measurements.
- Weighted endpoint materialization gives contiguous hot-loop access. Removing
  it entirely trades allocation for indirect loads; no supporting comparison.
- Worker-local capacity rows/masks and coverage DP buffers have actual ownership
  and state-separation roles. No synchronization redesign was justified.
- Generic comparisons were not made lazy: doing so changes which custom `Ord`
  operations execute. Stable ties and output order were preserved.
- Plugin error strings, scalar serialization and datetime reconstruction encode
  existing contracts. Generated FFI callbacks intentionally use the pinned
  derive implementation's panic-catching path; replacing it would require a
  separate FFI review.

This audit found no demonstrated defect in the inspected production algorithms.
It is not a proof of repository soundness or a claim of exhaustive optimization.

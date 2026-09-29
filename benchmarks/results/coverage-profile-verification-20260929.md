# Coverage profile verification — 2026-09-29

This records the initial eager API, before the lazy follow-up described in
[the follow-up inventory](coverage-profile-lazy-verification-20260929.md).
The Rust engine and measured native binary are unchanged by that follow-up.

This inventory records completed checks and measured sources for the coverage
profile feature. Unmeasured items below are not claimed as passed. Local `target/`
logs are working-directory evidence; the benchmark CSVs, metadata, and source
archives linked here are retained artifacts.

## Completed checks

| Check | Result | Evidence |
| --- | --- | --- |
| `cargo test --workspace --locked` | 436 Rust tests and doctests passed, zero failures | `target/coverage-profile-cargo-test.log`; total of its successful test-summary counts |
| Higher-case profile tests | 25 core tests and 8 candidate tests passed with `PROPTEST_CASES=2048` | Recorded final test runs after the weighted-record production change; 13 core and 4 candidate properties, totaling 34,816 generated property instances, plus named tests |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed | `target/coverage-profile-clippy.log` |
| `cargo fmt --check` | Passed | Completed session output, including the final candidate harness |
| `uv lock --check` | Passed | Completed session output |
| `uv run --locked --no-sync ruff check .` | Passed | Completed session output |
| `uv run --locked --no-sync ruff format --check .` | Passed | Completed session output |
| `uv run --locked --no-sync pytest --doctest-modules python/polars_intervals tests` | 3,114 passed in 7.45 seconds | `target/coverage-profile-pytest.log` |
| Focused Python, native-binding, endpoint-validation, and native-baseline checks | 825 passed in 5.92 seconds | `target/coverage-profile-python-focused.log` |
| Benchmark table generation | Four coverage-profile tables generated from saved samples | `target/coverage-profile-report-generation.log`; generated Markdown and CSV in `docs/assets/benchmarks/` |
| Reporting unit tests | 6 passed | `target/coverage-profile-report-tests.log` |
| Standalone native baseline and release/CI helper tests | 62 passed | `target/coverage-profile-reporting-extra-tests.log`; `uv run --locked --no-sync python -m pytest benchmarks/test_coverage_profile.py .github/scripts/test_release_checks.py .github/scripts/test_ci_changes.py -q` |
| `uv run --locked --isolated --only-group docs mkdocs build --strict` | Passed | `target/coverage-profile-mkdocs.log` |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` | Passed | `target/coverage-profile-rustdoc.log` |
| Rendered report | Wide 1280px and narrow 390px layouts checked; narrow tables scroll to all comparison columns | Local browser inspection of generated site; temporary viewport reset afterward |
| Normal Windows CPython 3.14 wheel | Built, installed outside checkout with locked dependencies; 3,114 tests/doctests passed in 7.09 seconds | `target/coverage-profile-wheel.log`, `target/coverage-profile-wheel-tests.log`; `.github/scripts/release_checks.py installed --checkout D:/polars-intervals` |
| Source distribution | Built; metadata, lock/toolchain, new native/core/Python modules and benchmark support match source files | `target/coverage-profile-sdist.log`; inspected `target/package-check/polars_intervals-0.2.0.tar.gz` |

Reproduce the higher-case setting with:

```powershell
$env:PROPTEST_CASES = "2048"
cargo test -p intervals-core --test coverage_profile --test coverage_profile_candidates --locked
```

These checks exercise the retained Rust implementation and actual native binding.
Small cases use independent direct-membership cells and integer-tick oracles;
properties additionally cover canonical structure and metamorphic identities.
Large benchmark checks compare complete candidate outputs and structural
invariants, rather than constructing a quadratic oracle.

The standalone benchmark tests use `python -m pytest` so the checkout root is on
the import path for the shared test oracle. A direct `pytest benchmarks/...`
invocation without that path failed collection; the documented invocation above
was checked successfully.

## Release build and native identity

The following release command completed twice; the final build finished before
end-to-end measurements:

```powershell
uv sync --locked --reinstall-package polars-intervals --config-setting 'build-args=--profile release'
```

The [end-to-end metadata](coverage-profile-polars-20260929.metadata.json) records
identical SHA-256 hashes for the installed extension and fresh release DLL:

```text
3409adcbd34c571e73f99f54107799236d097af1d47f4d5d6af1086f24058699
```

The measured environment used Rust 1.98.1, Python 3.14.0, and Polars 1.44.2 on
Windows 11. The new native entry point was imported and exercised through the
public API, including exact `Int128` outputs and typed empty schemas.

After `uv lock --check`, Python checks used `uv run --locked --no-sync` to
preserve the verified release extension. Benchmark runners used
`uv run --no-sync`. These are the executed variants; this inventory does not
claim that plain `uv run --locked` checks were rerun afterward. On this machine,
uv's isolated builds used changing temporary Python paths in PyO3 fingerprints,
triggering broad native recompilation. Avoiding another synchronization preserved
the measured release binary; it did not skip Python test execution.

## Measured core evidence and source labels

All three core runs used two seeds, five samples, and two warmups. Timing covered
the complete core call, including validation, clipping, sortedness checks,
preparation, canonical output construction, and output destruction. Allocation
measurement was a separate untimed peak-requested-live-heap run, excluding caller
inputs, allocator overhead, stack, and process RSS.

| Run | Samples / instances | Source meaning |
| --- | --- | --- |
| [Initial matrix](coverage-profile-core-20260929.csv) · [metadata](coverage-profile-core-20260929.metadata.json) · [sources](coverage-profile-core-20260929.sources.zip) | 12,580 / 692 | Historical index-stream production, compared with event, heap, and weighted-record candidates; Int64/UInt64 endpoints and explicit Int64 weights |
| [Wide-weight follow-up](coverage-profile-core-i128-20260929.csv) · [metadata](coverage-profile-core-i128-20260929.metadata.json) · [sources](coverage-profile-core-i128-20260929.sources.zip) | 880 / 44 | Historical index-stream production, with Int128 weights matching the native adapter's accumulator input layout |
| [Final production](coverage-profile-core-final-20260929.csv) · [metadata](coverage-profile-core-final-20260929.metadata.json) · [sources](coverage-profile-core-final-20260929.sources.zip) | 1,240 / 68 | Retained weighted-record production and unchanged endpoint-only unit route; event, heap, and private index-stream comparisons; Int64 endpoints, unit or Int128 quantities |

An instance fixes geometry, order, load mode, endpoint dtype, domain, zero mode,
seed, and row count. The initial matrix includes 0, 1, 8, 1,000, 10,000, 100,000,
and focused 1,000,000-row cases. Follow-ups use 1,000, 100,000, and focused
1,000,000-row cases. CSVs retain `m`, `u`, `z`, concurrency, exact samples,
allocation counts, requested heap, and record size/alignment.

Historical `production` labels describe the implementation measured then; they
must not be relabeled as measurements of the final weighted-record route.
The first run's metadata flags a test-only source change during execution and
explains it: timed production/candidate sources were unchanged, and its archive
preserves the benchmark-start files. The wide-weight and final runs record all
captured sources unchanged during measurement.

The three end-to-end runs also completed: the [main one-thread matrix](coverage-profile-polars-20260929.metadata.json),
the [focused million-row run](coverage-profile-polars-million-20260929.metadata.json),
and the [24-thread comparison](coverage-profile-polars-threads24-20260929.metadata.json).
Each uses two seeds and five samples. Their complete-call scope includes native
validation, partitioning, planning, output/key construction, and the Python/native
crossing; output destruction is excluded. All timed outputs were checked outside
timing against the complete canonical production result. End-to-end memory was
not measured; core requested-heap figures do not include Polars/FFI storage.

## Distribution identity and limits

The non-editable wheel contains `py.typed`, the exported Python function, and a
native extension whose SHA-256 exactly matches the measured release hash above.
Wheel/source metadata match the checkout README and unchanged package version.
The installed-artifact check exercises the new native function with output above
UInt64's range before running all Python tests from outside the checkout.

```text
wheel SHA-256: 28e65ab7c1739109c1b31bffb5ab55e7038175f2dac1dbcb0753c064dc9b3aa2
sdist SHA-256: 547a54a3c933ee293bca56df433e0f8d01e5de877282024f1ecb6b21e3e2026d
```

Normal-wheel packaging used Maturin 1.15.0 `pep517 build-wheel`, with
`--compatibility off --profile release --locked --out target/package-check`,
omitting `--editable`. Recreating the previously deleted isolated interpreter
at its recorded path reused the final Cargo build. The separate wheel-test
environment used `uv export --locked --no-emit-project` requirements. The source
archive used `maturin sdist --out target/package-check`.

The full 15-platform/CPython release matrix and rebuilding/installing from the
source archive were not run. Measurements cover synthetic inputs on one Windows
machine, not real genomic/resource traces, other hardware, or other Python
versions. End-to-end requested heap/RSS was not measured. There were no new
production dependencies, unsafe code, package version changes, or optional
map/compression solver in this feature.

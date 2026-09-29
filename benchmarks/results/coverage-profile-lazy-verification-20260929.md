# Lazy coverage profile verification — 2026-09-29

The same public `coverage_profile` function now accepts either frame kind:
DataFrame inputs return DataFrames, and LazyFrame inputs return deferred
LazyFrames. The Rust core and native binding are unchanged from the
[initial implementation](coverage-profile-verification-20260929.md).

## Execution contract

The lazy path resolves the input schema, selects required columns by literal
name, and creates a `map_batches` node with an explicit output schema. Predicate,
projection and slice pushdown are disabled at this node; `streamable=False`
ensures the entire collection reaches the same native solver under both engines.
No Python row/group calculation or input-row collection happens during planning.
Caller-owned key lists and scalar-bound Series are captured as stable values.

Arguments/schema are checked at construction. Native bounds and row validation
run when the profile executes. Polars can eliminate an unused node entirely,
such as for `head(0)`; no row validation occurs for a node that does not execute.
The profile is a blocking operation and still needs its full input in memory.

## Completed checks

| Check | Result | Local log / retained evidence |
| --- | --- | --- |
| `uv run --locked --no-sync pytest --doctest-modules python/polars_intervals tests -q` | 3,244 passed | `target/coverage-profile-lazy-pytest.log` |
| New lazy integration suite | 130 passed, included above | `tests/test_coverage_profile_lazy.py` |
| Native baseline and release/CI helper tests | 62 passed | `target/coverage-profile-lazy-helper-tests.log` |
| Ruff check and format check | Passed | Completed session output |
| `uv lock --check` and `git diff --check` | Passed | Completed session output |
| Benchmark table generation | Passed; new lazy table has 18 measured values | `target/coverage-profile-lazy-tables.log`; `docs/assets/benchmarks/coverage-profile-lazy-table.*` |
| Reporting unit tests | 6 passed | `target/coverage-profile-lazy-plot-tests.log` |
| Strict MkDocs build | Passed | `target/coverage-profile-lazy-mkdocs.log` |
| Rendered report review | Passed at 1280px and 390px; narrow tables scroll to every result column | Local built documentation |
| Usage examples | All 7 Python blocks executed successfully | `docs/coverage-profile.md` |
| Non-editable Windows CPython 3.14 wheel | Built; all 3,244 tests passed from outside the checkout | `target/coverage-profile-lazy-wheel.log`, `target/coverage-profile-lazy-wheel-tests.log` |
| Source distribution | Built and inspected; updated API and unchanged native/core modules match source files | `target/coverage-profile-lazy-sdist.log` |

The new tests exercise actual native code, including deferred source execution,
schema-only inspection, all supported endpoint dtypes, temporal/null group keys,
empty inputs/domains, exact Int128 results, mutable option snapshots, literal
wildcard-looking names, multiple chunks, and partitioned Parquet/IPC scans.
Both collection engines compare complete outputs with eager solves and the
independent original-row membership oracle. Downstream filters, projections,
nonzero heads and slices cannot hide invalid rows or alter the solve.

No Rust source changed in this follow-up. The earlier workspace/Clippy/fmt,
Rustdoc and higher-case Proptest results apply to the same Rust source; they
were not rerun just for the Python lazy wrapper. The packaging command reused
the verified release library without recompiling it.

## Benchmark provenance

[Raw samples](coverage-profile-lazy-20260929.csv),
[metadata](coverage-profile-lazy-20260929.metadata.json), and
[source archive](coverage-profile-lazy-20260929.sources.zip) contain 540 rotated
samples from 24 fixtures: four cases, three sizes (8/1,000/100,000), two seeds,
five samples and two warmups with one Polars thread. The `--lazy` flag adds
complete plan construction and collection with auto and streaming engines to
the existing eager/native comparisons. Fixture creation, output checking and
destruction are outside timing. All recorded sources still matched their hashes
after measurement. Previous eager measurements retain their original sources
and are explicitly labeled as predating the lazy wrapper in the report.

## Distribution identity and limits

The wheel contains the current Python API and `py.typed`. Its embedded native
binary matches the installed extension and measured Cargo release library:

```text
native SHA-256: 3409adcbd34c571e73f99f54107799236d097af1d47f4d5d6af1086f24058699
wheel SHA-256: 114a9f09ad2b99594b0f739a03bdbfcbc29cd7894b1d676f1c6874855c64b7a9
sdist SHA-256: a027aa669c84560cfa23d7c0c342822e62dd7e7518fa066e6283406902ff2dfe
```

Artifacts are in `target/package-check-lazy/`. Packaging used Maturin 1.15.0
`pep517 build-wheel --compatibility off --profile release --locked` and
`maturin sdist`. The outside-checkout environment installed this wheel using
locked dependencies and ran `.github/scripts/release_checks.py installed`.
The smoke check now includes lazy streaming-engine collection and loads above
UInt64's range. Wheel/source metadata match the unchanged package version and
updated README.

Before the Ponytail follow-up below, million-row or 24-thread lazy calls,
file-I/O performance, end-to-end memory, other machines/Python versions, and
the full release wheel matrix were not measured here. No dependency, native
engine, or version change was needed.

## Ponytail review and PR preparation

The follow-up simplification pass retained the production core, native binding
and Python API unchanged. It reused the private candidates' existing segment
emitter and removed redundant test setup. Historical core timings retain the
original candidate source archives; no old measurement is attributed to the
edited candidate. The native Polars competitor's redundant metadata join was
also removed, and benchmark runners now reject invalid or empty measurement
configurations. Fresh comparison samples accompany that baseline change.

After the Rust test/candidate cleanup, the full workspace passed all 436 tests
and doctests, Clippy passed for all targets, and formatting passed. The full
Python suite passed all 3,244 tests again. Logs are
`target/coverage-profile-pr-cargo-test.log`,
`target/coverage-profile-pr-clippy.log` and
`target/coverage-profile-pr-pytest.log`. `uv lock --check` also passed.
The installed native binary still matches the release hash above; production
Rust source did not change, so no new extension build was necessary.

The higher-case pass was repeated after cleanup with `PROPTEST_CASES=2048`:
25 core and 8 candidate tests passed, including 34,816 generated property
instances (`target/coverage-profile-pr-proptest.log`). The benchmark and
release/CI helper suites passed all 76 tests, including the new invalid-option
and empty-measurement checks (`target/coverage-profile-pr-helpers.log`). Ruff
check and format check passed.

The [fresh baseline/lazy samples](coverage-profile-ponytail-20260929.csv) contain
540 samples across 24 fixtures. The
[grouped million-row follow-up](coverage-profile-ponytail-million-20260929.csv)
contains 90 samples across 4 fixtures. Both use one thread, two seeds, two warmups
and five samples, retain their own metadata/source archives, and compare every
complete output outside timing. These replace no historical artifacts.

The refreshed table contains 24 measured values; all six reporting tests and
the strict MkDocs build passed (`target/coverage-profile-pr-mkdocs.log`). The
rendered table was checked again at 1280px and 390px, including horizontal
scrolling to the native-baseline column. No unrelated generated assets changed.

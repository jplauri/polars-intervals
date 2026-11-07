# Repository guidance

Read [CONTRIBUTING.md](CONTRIBUTING.md) for builds, checks and Windows setup.
Check current `master` and existing patterns before expensive implementation or
benchmarking. Run checks appropriate to the files changed.

- Write documentation in short, direct sentences. Avoid heavy use of semicolons.
  Docstrings link to the usage conventions instead of restating them.
- Reuse core validation, Polars extraction/dispatch, Python `_plugin`, and the
  benchmark runner in `benchmarks/provenance.py`. The adapter has no public Rust
  API and is tested from Python only. Extend `tests/test_endpoint_validation.py`
  and keep operation-specific regressions in their own suites.
- Preserve each operation's empty-row, zero-value and tie semantics; they differ
  between algorithms. Validate every row before pruning or fast paths. Preserve
  exact integer/temporal types and whole-collection/group execution across chunks,
  including streaming. Preserve original row order for row-aligned outputs.
- Test optimizers against an independent oracle for the original problem.
  Candidates sharing a reduction do not independently validate that reduction.
  For tied optima, compare feasibility and objective unless the mask is specified.
  Distinguish exact guarantees from heuristic improvements.
- Rebuild the native plugin after Rust changes. For benchmarks, verify the
  installed plugin matches a fresh release build, then use `uv run --no-sync`
  to preserve it. Use isolated docs/plots environments for reporting-only work.
- Keep experimental solvers and benchmark-only helpers out of the shipped crates.
  When a benchmark decision ships, link the commit that still has the losing
  candidates from its report, then delete them. Reuse existing machinery before
  adding abstractions or dependencies.
- Benchmark a clean, committed tree and record its revision. Commit raw samples
  only for runs a report or table uses. Never relabel old timings as
  measurements of changed code. Separate Rust and complete Polars timings, name
  the memory metric, and retain meaningful losses and limitations.
- Follow [the benchmark guide](docs/benchmarking.md) and
  [report template](benchmarks/report-template.md): explain the operation before
  internals and lead with concrete measured sizes/runtimes. Update reports,
  `plots.toml`, the overview and navigation together.

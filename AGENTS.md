# Repository guidance

Read [CONTRIBUTING.md](CONTRIBUTING.md) for builds, checks and Windows setup.
Check current `master` and existing patterns before expensive implementation or
benchmarking. Run checks appropriate to the files changed.

- Reuse core validation, Polars extraction/dispatch, Python `_plugin`, and
  `benchmarks/provenance.py`. Extend the shared Rust/Python endpoint-validation
  tables; keep operation-specific regressions in their own suites.
- Preserve each operation's empty-row, zero-value and tie semantics; they differ
  between algorithms. Validate every row before pruning or fast paths. Preserve
  exact integer/temporal types, original row order and whole-collection/group
  execution across chunks, including streaming.
- Test optimizers against an independent oracle for the original problem.
  Candidates sharing a reduction do not independently validate that reduction.
  For tied optima, compare feasibility and objective unless the mask is specified.
  Distinguish exact guarantees from heuristic improvements.
- Check `#[path]` test/benchmark imports before moving Rust internals: some
  production files are also compiled as standalone modules.
- Rebuild the native plugin after Rust changes. For benchmarks, verify the
  installed plugin matches a fresh release build, then use `uv run --no-sync`
  to preserve it. Use isolated docs/plots environments for reporting-only work.
- Keep experimental solvers private and retain only useful or explicitly required
  comparisons. Reuse existing machinery before adding abstractions or dependencies.
- Preserve raw benchmark samples and source/build metadata. Never relabel old
  timings as measurements of changed code. Separate Rust and complete Polars
  timings, name the memory metric, and retain meaningful losses and limitations.
- Follow [the benchmark guide](docs/benchmarking.md) and
  [report template](benchmarks/report-template.md): explain the operation before
  internals and lead with concrete measured sizes/runtimes. Update reports,
  generated tables, the overview and navigation together.

# Benchmarks

See the [benchmark overview](../docs/benchmarks.md) for results and plots.

[Running and publishing benchmarks](../docs/benchmarking.md) covers setup,
plot generation, and adding benchmarks.

## Python scripts

Run commands from the repository root. Benchmark runners need an installed
release plugin. Summary and plotting scripts only read saved results.

| Script | Role | Input → output | Report |
| --- | --- | --- | --- |
| [`coverage_summary.py`](coverage_summary.py) | Report generator | Core candidate CSV → median runtime and allocation CSV | [Maximum k-coverage](../docs/coverage-benchmarks.md) |
| [`coverage_temporal.py`](coverage_temporal.py) | End-to-end benchmark | Integer/temporal fixtures → raw timing CSV | [Maximum k-coverage](../docs/coverage-benchmarks.md) |
| [`overlap_count.py`](overlap_count.py) | End-to-end benchmark | Generated workloads → timing samples, RSS and environment in JSON | [Overlap counting](../docs/overlap-count-benchmarks.md) |
| [`containment_count.py`](containment_count.py) | End-to-end benchmark | Generated workloads → samples, native plans and pair-count safety skips in JSON | [Containment counting](../docs/containment-benchmarks.md) |
| [`weighted_summary.py`](weighted_summary.py) | Report generator | Candidate CSV argument → runtime ratios and tables on stdout | [Weighted scheduling](../docs/weighted-scheduling-benchmarks.md) |
| [`capacity_summary.py`](capacity_summary.py) | Report generator | Candidate CSV arguments (default: recorded run) → median timing/allocation CSV on stdout | [Capacity scheduling](../docs/capacity-scheduling-benchmarks.md) |
| [`capacity_temporal.py`](capacity_temporal.py) | End-to-end benchmark | Integer/temporal cliques → raw timing CSV on stdout | [Capacity scheduling](../docs/capacity-scheduling-benchmarks.md) |
| [`capacity_profile_summary.py`](capacity_profile_summary.py) | Report generator | Profile candidate CSVs → median runtime, flow and allocation metrics | [Capacity profiles](../docs/capacity-profile-benchmarks.md) |
| [`capacity_profile_temporal.py`](capacity_profile_temporal.py) | End-to-end benchmark | Integer/temporal profiles → verified release-native timing CSV | [Capacity profiles](../docs/capacity-profile-benchmarks.md) |
| [`covering_summary.py`](covering_summary.py) | Report generator | Candidate CSV argument (default: recorded run) → ratios and tables on stdout | [Covering](../docs/covering-benchmarks.md), [cost covering](../docs/cost-covering-benchmarks.md) |
| [`covering_temporal.py`](covering_temporal.py) | End-to-end benchmark | Integer/temporal covering fixtures → raw timing CSV on stdout | [Covering](../docs/covering-benchmarks.md), [cost covering](../docs/cost-covering-benchmarks.md) |
| [`stabbing_summary.py`](stabbing_summary.py) | Report generator | Recorded core and optional temporal CSVs in `results/` → median/ratio tables on stdout | [Stabbing points](../docs/stabbing-benchmarks.md) |
| [`stabbing_temporal.py`](stabbing_temporal.py) | End-to-end benchmark | Integer/temporal fixtures → raw timing CSV on stdout | [Stabbing points](../docs/stabbing-benchmarks.md) |
| [`plot.py`](plot.py) | Shared figure generator | [`plots.toml`](plots.toml) and saved CSV/JSON → SVG, plotted CSV and Markdown tables | [Plot workflow](../docs/benchmarking.md#generate-plots) |
| [`test_plot.py`](test_plot.py) | Reporting checks | Small synthetic records → checks for units, repeats, invalid input and missing cases | [Plot workflow](../docs/benchmarking.md#generate-plots) |

Rust candidate runners live in [`crates/intervals-core/benches/`](../crates/intervals-core/benches/).
They do not require Python or the Polars plugin. Each report names its Cargo
target. Covering and cost covering share the `covering` target.

## Regenerate all figures

```sh
uv run --locked --isolated --only-group plots python benchmarks/plot.py
uv run --locked --isolated --only-group docs mkdocs build --strict
```

This reads saved measurements. It does not run benchmarks or compile Rust.
Commit regenerated files in `docs/assets/benchmarks/` with the source/configuration
change. The documentation CI build regenerates the figures before building the site.

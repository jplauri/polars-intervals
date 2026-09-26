"""Generate documentation figures and tables from saved runs; never run benchmarks."""

import argparse
import io
import json
import tomllib
from pathlib import Path

import matplotlib
import polars as pl

matplotlib.use("Agg")
import matplotlib.pyplot as plt
from matplotlib.ticker import FuncFormatter

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/jplauri/polars-intervals/blob/master/"


def load_samples(source, root=ROOT):
    """Read CSV samples, or flatten the original overlap-count JSON report."""
    path = root / source["path"]
    if path.suffix == ".csv":
        # Objectives can exceed Int64. Only cast the columns used by a chart.
        return pl.scan_csv(path, infer_schema=False)
    report = json.loads(path.read_text(encoding="utf-8"))
    rows = [
        {
            **{key: case[key] for key in source["dimensions"]},
            "rows": case["rows"],
            "method": method,
            "sample": sample,
            "ms": value,
        }
        for case in report["results"]
        for method, timing in case["timings"].items()
        for sample, value in enumerate(timing["samples_ms"])
    ]
    return pl.DataFrame(rows).lazy()


def summarize(samples, source, chart):
    """Aggregate repeats of one workload, keeping each method and size separate."""
    filters = chart["filters"]
    dimensions = set(source["dimensions"]) - {source["x"], source["method"]}
    if set(filters) != dimensions:
        raise ValueError(f"Filters must fix every workload dimension: {sorted(dimensions)}")
    selected = samples.filter(
        *(pl.col(key).cast(pl.String) == str(value) for key, value in filters.items()),
        pl.col(source["method"]).is_in(list(chart["methods"])),
    ).select(
        pl.col(source["x"]).cast(pl.Int64).alias("x"),
        pl.col(source["method"]).alias("method"),
        (pl.col(chart["value"]).cast(pl.Float64) / chart["divisor"]).alias("value"),
        pl.col("sample").cast(pl.Int64),
    )
    # One collection; validation and the grouped query operate on this small slice.
    selected = selected.collect()
    if selected.is_empty():
        raise ValueError("No samples match the selected workload")
    if set(selected["method"]) != set(chart["methods"]):
        raise ValueError("A requested method has no samples")
    for name in ("x", "value", "sample"):
        if selected[name].null_count() or not selected[name].is_finite().all():
            raise ValueError(f"Missing or non-finite {name}")
    if (selected["x"] <= 0).any() or (selected["value"] < 0).any():
        raise ValueError("Sizes must be positive and measurements nonnegative")
    if (selected["sample"] < 0).any():
        raise ValueError("Warmup samples must not appear in the recorded data")
    if selected.select("x", "method", "sample").is_duplicated().any():
        raise ValueError("Duplicate samples: select a single run and workload")
    return (
        selected.lazy()
        .group_by("method", "x")
        .agg(
            pl.col("value").median().alias("median"),
            pl.col("value").min().alias("min"),
            pl.col("value").max().alias("max"),
            pl.len().alias("samples"),
        )
        .sort("method", "x")
        .collect()
    )


def render(points, source, chart):
    """Render the same points as an SVG and an accessible Markdown table."""
    if chart.get("yscale", "log") == "log" and (points["min"] <= 0).any():
        raise ValueError("Logarithmic charts require positive measurements")
    sizes = sorted(set(points["x"]))
    filters = ", ".join(f"{key}={value}" for key, value in chart["filters"].items())
    with plt.rc_context({"svg.hashsalt": chart["id"], "font.family": "DejaVu Sans"}):
        fig, ax = plt.subplots(figsize=(9, 5), layout="constrained")
        for index, (method, label) in enumerate(chart["methods"].items()):
            rows = {
                row["x"]: row for row in points.iter_rows(named=True) if row["method"] == method
            }
            # Missing sizes remain gaps; never draw a line across omitted cases.
            values = [rows.get(size, {}).get("median", float("nan")) for size in sizes]
            low = [rows.get(size, {}).get("min", float("nan")) for size in sizes]
            high = [rows.get(size, {}).get("max", float("nan")) for size in sizes]
            line = ax.plot(
                sizes,
                values,
                marker=("o", "s", "^", "D", "v", "P", "X")[index % 7],
                label=label,
                linewidth=1.8,
                markersize=5,
            )[0]
            ax.fill_between(sizes, low, high, color=line.get_color(), alpha=0.12)
        ax.set_xscale("log")
        ax.set_yscale(chart.get("yscale", "log"))
        if chart.get("yscale") == "linear":
            ax.set_ylim(bottom=0)
        ax.set_xticks(sizes)
        ax.xaxis.set_major_formatter(FuncFormatter(lambda value, _: f"{value:,.0f}"))
        ax.set_xlabel(chart.get("xlabel", "Input rows"))
        ax.set_ylabel(chart["ylabel"])
        ax.set_title(f"{chart['title']}\n{chart['scope']} · {filters}", fontsize=11, pad=14)
        ax.grid(axis="y", alpha=0.2)
        ax.legend(fontsize=9)
        fig.get_layout_engine().set(rect=(0, 0.06, 1, 0.94))
        fig.text(
            0.5,
            0.015,
            "Lines show medians · bands show sample range",
            ha="center",
            fontsize=9,
            color="#444444",
        )
        svg = io.StringIO()
        fig.savefig(svg, format="svg", metadata={"Date": None, "Title": chart["title"]})
        plt.close(fig)

    lines = [
        (
            f"![{chart['title']}: {chart['scope']}. {filters}. {chart['ylabel']}.]"
            f"(assets/benchmarks/{chart['id']}.svg)"
        ),
        "",
        chart["caption"],
        "",
        (
            f"Source: [{Path(source['path']).name}]({REPOSITORY}{source['path']}). "
            f"[Download plotted values](assets/benchmarks/{chart['id']}.csv)."
        ),
        "",
        '<details markdown="1">',
        "<summary>Plotted values and sample counts</summary>",
        "",
        f"| {chart.get('xlabel', 'Input rows')} | Method | Median | Min | Max | Samples |",
        "| ---: | --- | ---: | ---: | ---: | ---: |",
    ]
    for method, label in chart["methods"].items():
        for row in points.iter_rows(named=True):
            if row["method"] == method:
                lines.append(
                    f"| {row['x']:,} | {label} | {row['median']:.4g} | {row['min']:.4g} "
                    f"| {row['max']:.4g} | {row['samples']} |"
                )
    lines.extend(
        [
            "",
            f"Measurement: **{chart['ylabel']}**.",
            "",
            "</details>",
            "",
        ]
    )
    return svg.getvalue(), "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=ROOT / "benchmarks/plots.toml")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/assets/benchmarks")
    args = parser.parse_args()
    config = tomllib.loads(args.config.read_text(encoding="utf-8"))
    args.output.mkdir(parents=True, exist_ok=True)
    sources = config["sources"]
    samples = {name: load_samples(source) for name, source in sources.items()}
    ids = [chart["id"] for chart in config["charts"]]
    if len(ids) != len(set(ids)):
        raise ValueError("Chart IDs must be unique")
    for chart in config["charts"]:
        source = sources[chart["source"]]
        points = summarize(samples[chart["source"]], source, chart)
        svg, markdown = render(points, source, chart)
        for suffix, content in (("svg", svg), ("md", markdown), ("csv", points.write_csv())):
            (args.output / f"{chart['id']}.{suffix}").write_text(
                content, encoding="utf-8", newline="\n"
            )
        print(f"{chart['id']}: {points.height} plotted points")


if __name__ == "__main__":
    main()

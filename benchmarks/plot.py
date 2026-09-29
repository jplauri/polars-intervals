"""Generate documentation tables from saved runs; never run benchmarks."""

import argparse
import json
import tomllib
from pathlib import Path

import polars as pl

ROOT = Path(__file__).resolve().parents[1]
REPOSITORY = "https://github.com/jplauri/polars-intervals/blob/master/"


def load_samples(source, root=ROOT):
    """Read CSV samples, or flatten legacy JSON timing reports."""
    path = root / source["path"]
    if path.suffix == ".csv":
        # Objectives can exceed Int64. Only cast the columns used by a table.
        return pl.scan_csv(path, infer_schema=False)
    report = json.loads(path.read_text(encoding="utf-8"))
    rows = [
        {
            **{key: case[key] for key in source["dimensions"]},
            source["x"]: case[source["x"]],
            source["method"]: method,
            "sample": sample,
            "ms": value,
        }
        for case in report["results"]
        for method, timing in case["timings"].items()
        for sample, value in enumerate(timing["samples_ms"])
    ]
    return pl.DataFrame(rows).lazy()


def summarize(samples, source, selection):
    """Aggregate repeats of one workload, keeping each method and size separate."""
    filters = selection["filters"]
    dimensions = set(source["dimensions"]) - {source["x"], source["method"]}
    if set(filters) != dimensions:
        raise ValueError(f"Filters must fix every workload dimension: {sorted(dimensions)}")
    selected = samples.filter(
        *(pl.col(key).cast(pl.String) == str(value) for key, value in filters.items()),
        pl.col(source["method"]).is_in(list(selection["methods"])),
    ).select(
        pl.col(source["x"]).cast(pl.Int64).alias("x"),
        pl.col(source["method"]).alias("method"),
        (pl.col(selection["value"]).cast(pl.Float64) / selection["divisor"]).alias("value"),
        pl.col("sample").cast(pl.Int64),
    )
    # One collection; validation and the grouped query operate on this small slice.
    selected = selected.collect()
    if selected.is_empty():
        raise ValueError("No samples match the selected workload")
    if set(selected["method"]) != set(selection["methods"]):
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


def summarize_table(samples, source, table):
    """Select representative sizes without combining distinct workloads."""
    labels = [case["label"] for case in table["cases"]]
    if len(labels) != len(set(labels)):
        raise ValueError("Table case labels must be unique")
    dimensions = [key for key in source["dimensions"] if key not in {source["x"], source["method"]}]
    points = []
    for case in table["cases"]:
        methods = case.get("methods", table["methods"])
        if not set(methods).issubset(table["methods"]):
            raise ValueError("Case methods must be included in table methods")
        sizes = case["sizes"]
        if not sizes or len(sizes) != len(set(sizes)) or any(size <= 0 for size in sizes):
            raise ValueError("Table sizes must be distinct positive values")
        selected_samples = samples.filter(pl.col(source["x"]).cast(pl.Int64).is_in(sizes))
        selected = (
            summarize(selected_samples, source, {**table, **case, "methods": methods})
            .lazy()
            .with_columns(
                pl.lit(case["label"]).alias("case"),
                *(
                    pl.lit(str(value)).alias(f"workload_{key}")
                    for key, value in case["filters"].items()
                ),
            )
            .select(
                "case",
                *(f"workload_{key}" for key in dimensions),
                "method",
                "x",
                "median",
                "min",
                "max",
                "samples",
            )
            .collect()
        )
        if set(sizes) != set(selected["x"]):
            raise ValueError("No samples match one or more selected table sizes")
        points.append(selected)
    return pl.concat(points)


def render_table(points, source, table):
    """Render a compact comparison; missing combinations remain empty measurements."""
    values = {
        (row["case"], row["x"], row["method"]): row["median"]
        for row in points.iter_rows(named=True)
    }
    lines = [
        f"**{table['ylabel']} · medians**",
        "",
        f"| Workload | {table.get('xlabel', 'Input rows')} | "
        + " | ".join(table["methods"].values())
        + " |",
        "| --- | ---: | " + " | ".join("---:" for _ in table["methods"]) + " |",
    ]
    has_missing = False
    for case in table["cases"]:
        for size in case["sizes"]:
            cells = []
            for method in table["methods"]:
                value = values.get((case["label"], size, method))
                has_missing |= value is None
                if value is None:
                    cells.append("—")
                else:
                    rounded = float(f"{value:.3g}")
                    cells.append(f"{rounded:,g}")
            lines.append(f"| {case['label']} | {size:,} | " + " | ".join(cells) + " |")
    if has_missing:
        lines.extend(["", "— means no recorded measurement for that combination."])
    lines.extend(
        [
            "",
            (
                f"[Exact values, sample ranges and counts](assets/benchmarks/{table['id']}.csv) · "
                f"[Source samples]({REPOSITORY}{source['path']})."
            ),
            "",
        ]
    )
    return "\n".join(lines)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config", type=Path, default=ROOT / "benchmarks/plots.toml")
    parser.add_argument("--output", type=Path, default=ROOT / "docs/assets/benchmarks")
    args = parser.parse_args()
    config = tomllib.loads(args.config.read_text(encoding="utf-8"))
    args.output.mkdir(parents=True, exist_ok=True)
    sources = config["sources"]
    samples = {name: load_samples(source) for name, source in sources.items()}
    tables = config.get("tables", [])
    ids = [table["id"] for table in tables]
    if len(ids) != len(set(ids)):
        raise ValueError("Table IDs must be unique")
    for table in tables:
        source = sources[table["source"]]
        points = summarize_table(samples[table["source"]], source, table)
        markdown = render_table(points, source, table)
        for suffix, content in (("md", markdown), ("csv", points.write_csv())):
            (args.output / f"{table['id']}.{suffix}").write_text(
                content, encoding="utf-8", newline="\n"
            )
        print(f"{table['id']}: {points.height} table values")


if __name__ == "__main__":
    main()

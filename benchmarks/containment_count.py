"""Containment integration and native Polars baselines; see containment.md."""

import argparse
import json
import random
from pathlib import Path
from statistics import median
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars.testing import assert_frame_equal
from provenance import environment, sha256

SCENARIOS = [
    "disjoint",
    "nested",
    "duplicates",
    "equal_starts",
    "equal_ends",
    "sparse",
    "dense",
    "broad",
    "crossing",
    "empty",
    "mixed",
    "sorted",
    "reverse",
    "shuffled",
]
PAIR_LIMIT = 2_000_000  # Includes self and conservatively bounds equality-join intermediates.
METHODS = ["plugin", "rank", "rank_by", "rank_runs", "join", "bands", "equality"]


def make_frame(scenario, n, groups=1, dtype=pl.Int64):
    rng = random.Random(42)
    rows = []
    length = (n + groups - 1) // groups
    for i in range(n):
        k, group = divmod(i, groups)
        if scenario == "disjoint":
            s, e = 3 * k, 3 * k + 1
        elif scenario in ("nested", "reverse"):
            s, e = k, 2 * length - k
        elif scenario in ("sorted", "shuffled"):
            s, e = length - k, length + k
        elif scenario == "duplicates":
            s, e = 1, 5
        elif scenario == "equal_starts":
            s, e = 0, k + 1
        elif scenario == "equal_ends":
            s, e = k, length
        elif scenario == "sparse":
            s = rng.randrange(max(1, 4 * length))
            e = s + rng.randrange(9)
        elif scenario == "dense":
            s = rng.randrange(1024)
            e = s + rng.randrange(1024)
        elif scenario == "broad":
            s, e = (-k, 4 * length + k) if k % 10 == 0 else (3 * k, 3 * k + 1)
        elif scenario == "crossing":
            s, e = k, k + length + 1
        elif scenario == "empty":
            s = e = k % 64
        elif scenario == "mixed":
            s, e = k % 64, k % 64 + k % 5
        else:
            raise ValueError(scenario)
        rows.append((s, e, group))
    if scenario == "shuffled":
        rng.shuffle(rows)
    return pl.DataFrame(
        rows, schema={"start": pl.Int64, "end": pl.Int64, "group": pl.Int64}, orient="row"
    ).with_columns(pl.col("start", "end").cast(dtype))


def plugin_query(frame, grouped=False):
    expr = pi.containment_count("start", "end")
    if grouped:
        expr = expr.over("group")
    return frame.lazy().select(expr.alias("count"))


def rank_query(frame, grouped=False, dynamic=False):
    """Expanding end rank after start-desc/end-asc sort; O(n log n), no pairs."""
    keys = (["group"] if grouped else []) + ["start", "end"]
    descending = ([False] if grouped else []) + [True, False]
    rows = frame.lazy().with_row_index("row").sort(keys, descending=descending)
    if dynamic:
        rows = rows.with_row_index("position")
        rank = (
            pl.col("end")
            .to_physical()
            .rolling_rank_by(
                "position",
                window_size=f"{max(1, frame.height)}i",
                method="max",
                closed="both",
                min_samples=1,
            )
        )
    else:
        rank = (
            pl.col("end")
            .to_physical()
            .rolling_rank(max(1, frame.height), method="max", min_samples=1)
        )
    if grouped:
        rank = rank.over("group")
    return (
        rows.with_columns(rank.alias("rank"))
        .with_columns((pl.col("rank").max().over(keys).cast(pl.UInt64) - 1).alias("count"))
        .sort("row")
        .select("count")
    )


def run_rank_query(frame, grouped=False):
    """Use complete equal-start windows directly, without duplicate correction."""
    keys = ["group", "start"] if grouped else ["start"]
    descending = [False, True] if grouped else [True]
    rows = frame.lazy().with_row_index("row").sort(keys, descending=descending)
    # Sorted runs give bounded increasing integer coordinates, without negating
    # extreme endpoints. A dynamic window includes ALL rows at the same start.
    count = (
        pl.col("end")
        .to_physical()
        .rolling_rank_by(
            pl.col("start").rle_id(),
            window_size=f"{max(1, frame.height)}i",
            closed="both",
            method="max",
            min_samples=1,
        )
        .cast(pl.UInt64)
        - 1
    )
    if grouped:
        count = count.over("group")
    return rows.with_columns(count.alias("count")).sort("row").select("count")


def join_query(frame, method="join"):
    rows = frame.lazy().with_row_index("row")
    if method == "bands":
        # General ordered groups; separate their coordinate ranges so the two
        # inequalities enforce group equality. Include this preprocessing in
        # timing. Safe for this benchmark's bounded physical endpoints only.
        start, end = (
            pl.col("start").to_physical().cast(pl.Int64),
            pl.col("end").to_physical().cast(pl.Int64),
        )
        width = end.max() - start.min() + 1
        offset = (pl.col("group").rank("dense").cast(pl.Int64) - 1) * width
        rows = rows.with_columns(
            (start - start.min() + offset).alias("start"), (end - start.min() + offset).alias("end")
        )
    predicates = [pl.col("start") <= pl.col("start_right"), pl.col("end_right") <= pl.col("end")]
    right = ["start", "end"]
    if method == "equality":
        right.append("group")
        predicates.append(pl.col("group") == pl.col("group_right"))
    # Every valid interval, including an empty one, matches itself once.
    # Subtraction avoids another row-id predicate and removes exactly self.
    return (
        rows.join_where(rows.select(right), *predicates)
        .group_by("row")
        .agg((pl.len().cast(pl.UInt64) - 1).alias("count"))
        .sort("row")
        .select("count")
    )


def oracle(frame, grouped):
    rows = list(frame.iter_rows())
    return pl.DataFrame(
        {
            "count": [
                sum(
                    i != j and s <= a and b <= e and (not grouped or g == h)
                    for j, (a, b, h) in enumerate(rows)
                )
                for i, (s, e, g) in enumerate(rows)
            ]
        },
        schema={"count": pl.UInt64},
    )


def check_baselines():
    for scenario in SCENARIOS:
        for groups in [1, 10]:
            frame = make_frame(scenario, 70, groups)
            expected = oracle(frame, groups > 1)
            methods = ["join"] if groups == 1 else ["equality", "bands"]
            assert_frame_equal(plugin_query(frame, groups > 1).collect(), expected)
            assert_frame_equal(run_rank_query(frame, groups > 1).collect(), expected)
            for dynamic in [False, True]:
                assert_frame_equal(rank_query(frame, groups > 1, dynamic).collect(), expected)
            for method in methods:
                assert_frame_equal(join_query(frame, method).collect(), expected)
    rng = random.Random(73)
    for _ in range(100):
        rows = []
        for _ in range(rng.randrange(31)):
            a, b = rng.randrange(-8, 9), rng.randrange(-8, 9)
            rows.append((min(a, b), max(a, b), rng.randrange(3)))
        frame = pl.DataFrame(
            rows, schema={"start": pl.Int64, "end": pl.Int64, "group": pl.Int64}, orient="row"
        )
        for grouped in [False, True]:
            expected = oracle(frame, grouped)
            assert_frame_equal(plugin_query(frame, grouped).collect(), expected)
            assert_frame_equal(run_rank_query(frame, grouped).collect(), expected)
            for dynamic in [False, True]:
                assert_frame_equal(rank_query(frame, grouped, dynamic).collect(), expected)
            for method in ["bands", "equality"] if grouped else ["join"]:
                assert_frame_equal(join_query(frame, method).collect(), expected)


def measure(frame, grouped, repeats, methods):
    queries = {
        "plugin": plugin_query(frame, grouped),
        "rank": rank_query(frame, grouped),
        "rank_by": rank_query(frame, grouped, dynamic=True),
        "rank_runs": run_rank_query(frame, grouped),
    }
    expected = queries["plugin"].collect()
    queries = {name: query for name, query in queries.items() if name in methods}
    pairs = int(expected["count"].sum())
    join_rows = pairs + frame.height
    skipped = {}
    join_methods = ["bands", "equality"] if grouped else ["join"]
    plans = {}
    for method in join_methods:
        if method not in methods:
            continue
        intermediate = join_rows
        if method == "equality":
            # The equality-first hash join may generate n_g^2 rows before filtering.
            intermediate = (
                frame.lazy()
                .group_by("group")
                .len()
                .select((pl.col("len").cast(pl.UInt64) ** 2).sum())
                .collect()
                .item()
            )
        if intermediate > PAIR_LIMIT:
            skipped[method] = {"reason": "intermediate row safety limit", "rows": intermediate}
        else:
            query = join_query(frame, method)
            plans[method] = query.explain()
            queries[method] = query
    samples = {key: [] for key in queries}
    names = list(queries)
    for repetition in range(repeats + 1):
        # The first pass verifies every candidate before accepting timed samples.
        for name in names[:: 1 if repetition % 2 == 0 else -1]:
            tick = perf_counter_ns()
            result = queries[name].collect()
            elapsed = (perf_counter_ns() - tick) / 1e6
            assert_frame_equal(result, expected)
            if repetition:
                samples[name].append(elapsed)
    return {
        "pairs_excluding_self": pairs,
        "join_rows_including_self": join_rows,
        "timings": {
            name: {"median_ms": median(values), "samples_ms": values}
            for name, values in samples.items()
        },
        "skipped": skipped,
        "native_plans": plans,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("target/containment-polars.json"))
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--methods", nargs="+", choices=METHODS, default=METHODS)
    parser.add_argument(
        "--sizes", nargs="+", type=int, default=[1000, 10000, 100000, 1000000, 3000000]
    )
    args = parser.parse_args()
    if args.repeats < 1 or not args.sizes or min(args.sizes) < 1:
        parser.error("repeats and sizes must be positive")
    check_baselines()
    report = {
        "environment": {
            **environment(),
            "engine": "in-memory",
            "kernel_profile": "release",
            "plugin_path": pi.__file__,
            "warmups": 1,
            "repeats": args.repeats,
            "methods": args.methods,
            "seed": 42,
            "source_sha256": {
                str(path): sha256(path)
                for path in [
                    Path(__file__),
                    Path("crates/intervals-core/src/containment.rs"),
                    Path("crates/intervals-core/benches/support/containment.rs"),
                ]
            },
        },
        "pair_limit": PAIR_LIMIT,
        "results": [],
    }
    cases = [(scenario, n, 1, pl.Int64, False) for scenario in SCENARIOS for n in args.sizes]
    cases += [
        (scenario, n, groups, pl.Int64, True)
        for scenario in ["sparse", "dense", "mixed"]
        for n in [10000, 100000]
        for groups in [1, 10, 100, 1000]
    ]
    cases += [
        ("sparse", n, groups, dtype, groups > 1)
        for dtype in [pl.Date, pl.Datetime("us", "UTC")]
        for n in [10000, 100000]
        for groups in [1, 100]
    ]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for scenario, n, groups, dtype, grouped in cases:
        result = measure(
            make_frame(scenario, n, groups, dtype), grouped, args.repeats, args.methods
        )
        result.update(scenario=scenario, n=n, groups=groups, dtype=str(dtype), window=grouped)
        report["results"].append(result)
        args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(
            f"{scenario} n={n} groups={groups} {dtype}: "
            + ", ".join(f"{k}={v['median_ms']:.3f}ms" for k, v in result["timings"].items()),
            flush=True,
        )


if __name__ == "__main__":
    main()

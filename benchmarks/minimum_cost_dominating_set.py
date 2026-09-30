"""Complete release-plugin collections; run with uv run --no-sync python.

Input construction, independent graph checks, and destruction are not timed.
"""

from __future__ import annotations

import argparse
import csv
import json
import random
from bisect import bisect_left
from collections import defaultdict
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars_intervals import _internal
from provenance import ROOT, environment, sha256, verify_release

CASES = [
    ("path", "start", "i64", 1, 1, "auto"),
    ("path", "shuffled", "i64", 1, 1, "auto"),
    ("disjoint", "shuffled", "u64", 1, 1, "auto"),
    ("nested", "reverse", "i64", 1, 1, "auto"),
    ("clique", "shuffled", "date", 1, 1, "auto"),
    ("duplicate", "shuffled", "datetime_us", 1, 1, "auto"),
    ("path", "shuffled", "datetime_ns_utc", 1, 7, "streaming"),
    ("empty", "start", "i64", 1, 1, "auto"),
    ("many_empty", "shuffled", "i64", 1, 7, "auto"),
    ("path", "shuffled", "i64", 32, 7, "auto"),
    ("duplicate", "shuffled", "date", 32, 1, "streaming"),
]
MODES = [("units", None), ("ones", "ones"), ("zero", "zero"), ("mixed", "mixed")]


def fixture(n, case, seed):
    family, order, dtype, groups, chunks, _ = case
    rng = random.Random(seed)
    rows = []
    for i in range(n):
        j = i // groups
        s, e = {
            "path": (3 * j, 3 * j + 4),
            "disjoint": (3 * j, 3 * j + 1),
            "nested": (j, 2 * n - j + 1),
            "clique": (0, n + 1),
            "duplicate": (j // 32 * 3, j // 32 * 3 + 2),
            "empty": (j % 16, j % 16),
            "many_empty": (j, j + 16 if j % 5 == 0 else j),
        }[family]
        rows.append((s, e, i % groups, rng.randrange(7)))
    if order == "shuffled":
        rng.shuffle(rows)
    elif order == "reverse":
        rows.reverse()
    frame = pl.DataFrame(
        rows,
        schema={"start": pl.Int64, "end": pl.Int64, "group": pl.Int64, "mixed": pl.Int64},
        orient="row",
    )
    target, offset = {
        "i64": (pl.Int64, 0),
        "u64": (pl.UInt64, 2**64 - 10_000_000),
        "date": (pl.Date, 20_000),
        "datetime_us": (pl.Datetime("us"), 2**53 + 1),
        "datetime_ns_utc": (pl.Datetime("ns", "UTC"), 2**53 + 1),
    }[dtype]
    frame = (
        frame.lazy()
        .with_columns(
            (pl.col("start", "end").cast(pl.UInt64 if dtype == "u64" else pl.Int64) + offset).cast(
                target
            ),
            pl.lit(1, dtype=pl.Int64).alias("ones"),
            pl.lit(0, dtype=pl.Int64).alias("zero"),
        )
        .collect()
    )
    if chunks > 1 and n:
        columns = []
        for i, series in enumerate(frame):
            step = max(1, n // chunks + i)
            columns.append(
                pl.concat([series[k : k + step] for k in range(0, n, step)], rechunk=False)
            )
        frame = pl.DataFrame(columns)
    return frame, rows


def check(rows, mode, mask, family):
    """Original adjacency only: subset optima at small n, analytic families otherwise."""
    assert mask.dtype == pl.Boolean and mask.null_count() == 0 and len(mask) == len(rows)
    groups = defaultdict(list)
    for (s, e, g, c), b in zip(rows, mask.to_list(), strict=True):
        groups[g].append((s, e, 0 if mode == "zero" else c if mode == "mixed" else 1, b))
    total_cost, total_count = 0, 0
    evidence = set()
    for group in groups.values():
        chosen = sorted((s, e) for s, e, _, b in group if b and s < e)
        starts, prefix = [], []
        for s, e in chosen:
            starts.append(s)
            prefix.append(max(e, prefix[-1]) if prefix else e)
        for s, e, _, b in group:
            if not b:
                j = bisect_left(starts, e)
                assert s < e and j and prefix[j - 1] > s
        objective = (sum(c for _, _, c, b in group if b), sum(b for _, _, _, b in group))
        optimum = None
        if len(group) <= 12:
            feasible = []
            for bits in range(1 << len(group)):
                if all(
                    bits >> i & 1
                    or (
                        s < e
                        and any(
                            bits >> j & 1 and a < b and s < b and a < e
                            for j, (a, b, _, _) in enumerate(group)
                        )
                    )
                    for i, (s, e, _, _) in enumerate(group)
                ):
                    feasible.append(
                        (
                            sum(c for i, (_, _, c, _) in enumerate(group) if bits >> i & 1),
                            bits.bit_count(),
                        )
                    )
            optimum = min(feasible)
            evidence.add("graph_subsets")
        elif family in ("empty", "disjoint"):
            optimum = (sum(r[2] for r in group), len(group))
        elif family in ("clique", "nested"):
            optimum = (min(r[2] for r in group), 1)
        elif family == "duplicate":
            costs = defaultdict(list)
            for s, e, c, _ in group:
                costs[s, e].append(c)
            optimum = (sum(map(min, costs.values())), len(costs))
        elif family == "path" and mode != "mixed":
            count = (len(group) + 2) // 3
            optimum = (0 if mode == "zero" else count, count)
        if optimum is not None:
            assert objective == optimum, (family, mode, objective, optimum)
            if len(group) > 12:
                evidence.add("analytic")
        else:
            evidence.add("feasibility_only")
        total_cost += objective[0]
        total_count += objective[1]
    return total_cost, total_count, "+".join(sorted(evidence)) or "empty"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", type=int, nargs="+", default=[0, 8, 64, 1000, 10000, 100000])
    parser.add_argument("--seeds", type=int, nargs="+", default=[7, 41])
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--release-library", type=Path)
    args = parser.parse_args()
    native = Path(_internal.__file__)
    try:
        release, inputs = verify_release(native, args.release_library)
    except ValueError as error:
        parser.error(str(error))
    output, meta = (
        args.output.with_suffix(".timings.csv"),
        args.output.with_suffix(".metadata.json"),
    )
    if output.exists() or meta.exists():
        parser.error("Use a new output prefix")
    if min(args.sizes) < 0 or args.samples < 1 or args.warmups < 1:
        parser.error("Need nonnegative sizes and positive samples/warmups")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    metadata = {
        **environment(),
        "native_sha256": sha256(native),
        "release_library": str(release),
        "release_sha256": sha256(release),
        "source_sha256": {
            str(p.relative_to(ROOT)): sha256(p)
            for p in inputs
            + [
                Path(__file__),
                ROOT / "benchmarks/provenance.py",
                ROOT / "python/polars_intervals/__init__.py",
                ROOT / "uv.lock",
            ]
        },
        "sizes": args.sizes,
        "seeds": args.seeds,
        "samples": args.samples,
        "warmups": args.warmups,
        "cases": CASES,
        "scope": "Prepared lazy collect, including plan optimization, validation/extraction, algorithm, reconstruction and mask. Excludes generation, checks and returned-result destruction; internal cleanup is timed.",
        "memory": "Not measured; core runner measures requested live heap separately.",
        "verification": "Every timed mask checked outside timing with original adjacency; subset optima for groups <=12 and analytic optima on labeled families. Other large weighted cases are feasibility-only. Determinism and unit/ones/zero objective agreement also checked.",
        "chunks": "Requested target; each column uses step=max(1,n//chunks+column), so boundaries differ.",
        "status": "running",
    }
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    count = 0
    with output.open("x", newline="", encoding="utf-8") as handle:
        writer = csv.writer(handle)
        writer.writerow(
            [
                "family",
                "order",
                "dtype",
                "groups",
                "chunks",
                "engine",
                "n",
                "seed",
                "costs",
                "method",
                "sample",
                "total_ns",
                "solution_cost",
                "solution_count",
                "evidence",
            ]
        )
        for n in args.sizes:
            for seed in args.seeds:
                for case in CASES:
                    frame, rows = fixture(n, case, seed)
                    queries, expected, objectives = {}, {}, {}
                    for mode, cost in MODES:
                        expr = pi.minimum_cost_dominating_set("start", "end", cost=cost)
                        if case[3] > 1:
                            expr = expr.over("group")
                        query = frame.lazy().select(expr.alias("selected"))
                        queries[mode] = query
                        expected[mode] = query.collect(engine=case[5])
                        objectives[mode] = check(rows, mode, expected[mode].to_series(), case[0])
                        for _ in range(args.warmups - 1):
                            assert query.collect(engine=case[5]).equals(expected[mode])
                    assert objectives["units"][:2] == objectives["ones"][:2]
                    assert objectives["units"][1] == objectives["zero"][1]
                    modes = list(queries)
                    for sample in range(args.samples):
                        for mode in modes[sample % 4 :] + modes[: sample % 4]:
                            tick = perf_counter_ns()
                            result = queries[mode].collect(engine=case[5])
                            elapsed = perf_counter_ns() - tick
                            assert result.equals(expected[mode])
                            assert (
                                check(rows, mode, result.to_series(), case[0]) == objectives[mode]
                            )
                            del result
                            writer.writerow(
                                [
                                    *case,
                                    n,
                                    seed,
                                    mode,
                                    "production",
                                    sample,
                                    elapsed,
                                    *objectives[mode],
                                ]
                            )
                            count += 1
                    handle.flush()
    metadata.update(status="completed", timing_samples=count, timing_sha256=sha256(output))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    print(f"Saved {count} samples to {output}")


if __name__ == "__main__":
    main()

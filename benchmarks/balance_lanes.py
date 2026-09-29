"""Stream interval corpora through the release Polars lane expressions.

Build once with the documented release-plugin procedure, then use ``uv run
--no-sync python benchmarks/balance_lanes.py --dataset PATH --output PREFIX``.
Accepts every existing smoke/exact/scaling or JAIST dataset without regeneration.
``exact`` is a corpus name, not an optimality assertion: the coloring oracle is
strictly capped. ``--handcrafted --stress-sizes 1000 10000`` adds separate fixtures.

Quality is recorded once per instance, workload, method, engine, and work budget;
raw timing samples live in a different CSV. A grouped workload repeats the same
logical graph in four groups: quality describes ONE graph, timing n counts ALL
collection rows. Production expressions receive only endpoints and, for repair,
the original starting lanes. Certificates and witnesses never enter the query.
Canonical duplicate dataset paths are collapsed. Distinct datasets retain their
own corpus dimension and are never pooled, including when their IDs overlap.
Private seed/stage experiments and actual work counters belong to the Rust core
benchmark; the public Series interface does not expose diagnostics.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import statistics
import sys
from collections import Counter, defaultdict
from datetime import UTC, datetime
from itertools import chain, groupby
from pathlib import Path
from time import perf_counter_ns

from balance_lanes_fixtures import iter_handcrafted
from generate_interval_graphs import (
    Instance,
    canonical_json,
    derive_seed,
    integer,
    iter_dataset,
    structural_stats,
    validate_instance,
)
from provenance import ROOT, environment, sha256

WORKLOADS = ("integer", "date", "datetime", "grouped", "sliced", "multi_chunk")
QUALITY_FIELDS = [
    "corpus",
    "id",
    "family",
    "population",
    "seed",
    "order",
    "workload",
    "engine",
    "groups",
    "collection_n",
    "method",
    "max_work",
    "status",
    "n",
    "nonempty_count",
    "empty_count",
    "omega",
    "k",
    "lane_min",
    "lane_max",
    "D",
    "Q",
    "delta",
    "D_minus_delta",
    "starting_D",
    "starting_Q",
    "production_D",
    "production_Q",
    "improvement_D",
    "improvement_Q",
    "score_relation",
    "D_star",
    "Q_star",
    "optimum_status",
    "additive_gap",
    "optimum_hit",
    "stop_reason",
    "work_used",
    "pairs_checked",
    "flips",
    "skips",
    "lanes_sha256",
]
TIMING_FIELDS = [
    "corpus",
    "id",
    "family",
    "population",
    "seed",
    "order",
    "workload",
    "engine",
    "groups",
    "n",
    "graph_n",
    "method",
    "max_work",
    "sample",
    "total_ns",
]


def validate_coloring(intervals, lanes):
    """Independent O(n log n) event concurrency and per-color feasibility check."""
    if len(intervals) != len(lanes):
        raise ValueError("coloring length differs from input")
    if any(type(c) is not int or not 0 <= c <= 2**32 - 1 for c in lanes):
        raise ValueError("coloring contains an invalid UInt32 lane")
    events, rows = [], defaultdict(list)
    for (start, end), lane in zip(intervals, lanes, strict=True):
        if type(start) is not int or type(end) is not int or start > end:
            raise ValueError("invalid interval endpoints")
        if start < end:
            events.extend(((start, 1), (end, -1)))
            rows[lane].append((start, end))
    active = omega = 0
    for _, change in sorted(events):
        active += change
        omega = max(omega, active)
    k = max(1, omega) if intervals else 0
    if set(lanes) != set(range(k)):
        raise ValueError("coloring must use exactly the contiguous minimum palette")
    for intervals_in_lane in rows.values():
        previous_end = None
        for start, end in sorted(intervals_in_lane):
            if previous_end is not None and start < previous_end:
                raise ValueError("coloring contains overlapping intervals in one lane")
            previous_end = end
    sizes = Counter(lanes)
    low, high = (min(sizes.values()), max(sizes.values())) if sizes else (0, 0)
    delta = int(bool(k and len(lanes) % k))
    return {
        "n": len(lanes),
        "nonempty_count": len(events) // 2,
        "empty_count": len(lanes) - len(events) // 2,
        "omega": omega,
        "k": k,
        "lane_min": low,
        "lane_max": high,
        "D": high - low,
        "Q": sum(size * size for size in sizes.values()),
        "delta": delta,
        "D_minus_delta": high - low - delta,
    }


def tiny_optimum(intervals, k, *, max_n=10, max_nodes=200_000):
    """Independent exhaustive coloring with label symmetry breaking and hard caps.

    Returning unknown after a node cap deliberately discards the incumbent: a
    best-so-far coloring is not an exact answer. n is hard-limited to 12 even if
    called outside the CLI. Empty intervals have no neighbors.
    """
    if not 0 <= max_n <= 12 or max_nodes < 1:
        raise ValueError("oracle limits require 0 <= max_n <= 12 and max_nodes >= 1")
    n = len(intervals)
    if n > max_n:
        return {"status": "unknown_size_limit", "D": None, "Q": None, "nodes": 0}
    if not n:
        return {"status": "exact", "D": 0, "Q": 0, "nodes": 1}
    neighbors = [
        [j for j in range(n) if j != i and max(s, intervals[j][0]) < min(e, intervals[j][1])]
        for i, (s, e) in enumerate(intervals)
    ]
    order = sorted(range(n), key=lambda i: (-len(neighbors[i]), i))
    colors, counts = [-1] * n, [0] * k
    quotient, remainder = divmod(n, k)
    bound = (int(bool(remainder)), remainder * (quotient + 1) ** 2 + (k - remainder) * quotient**2)
    best, nodes, aborted = None, 0, False

    def visit(position, used):
        nonlocal best, nodes, aborted
        if aborted or best == bound:
            return
        if nodes >= max_nodes:
            aborted = True
            return
        nodes += 1
        if used + n - position < k:
            return
        if position == n:
            if used == k:
                score = (max(counts) - min(counts), sum(x * x for x in counts))
                best = min(best, score) if best else score
            return
        row = order[position]
        blocked = {colors[j] for j in neighbors[row]}
        for color in range(min(used + 1, k)):
            if color not in blocked:
                colors[row] = color
                counts[color] += 1
                visit(position + 1, max(used, color + 1))
                counts[color] -= 1
                colors[row] = -1

    visit(0, 0)
    if aborted:
        return {"status": "unknown_work_limit", "D": None, "Q": None, "nodes": nodes}
    if best is None:
        raise ValueError("oracle found no coloring for stated minimum palette")
    return {"status": "exact", "D": best[0], "Q": best[1], "nodes": nodes}


def check_record(record):
    """Validate geometry, stored statistics, ID, and known generator contracts."""
    intervals = record["intervals"]
    starts, ends = [s for s, _ in intervals], [e for _, e in intervals]
    instance = Instance(
        record["family"],
        starts,
        ends,
        record["seed"],
        record["params"],
        record.get("certificate"),
        record.get("provenance"),
    )
    stats = (
        structural_stats(starts, ends)
        if instance.family == "handcrafted"
        else validate_instance(instance)
    )
    if instance.id != record["id"]:
        raise ValueError("record ID does not match its content")
    if stats != record["stats"] or len(intervals) != record["n"]:
        raise ValueError("record structural statistics do not match its geometry")
    if "supplied_lanes" in record:
        validate_coloring(intervals, record["supplied_lanes"])
    if "equity_witness" in record:
        score = validate_coloring(intervals, record["equity_witness"])
        if score["D"] != score["delta"]:
            raise ValueError("equity witness does not reach the elementary lower bound")
    return stats


def optimum_for(record, k, *, max_n=10, max_nodes=200_000):
    certificate = record.get("certificate")
    if certificate and certificate.get("kind") == "planted_balanced":
        n = record["n"]
        delta = int(bool(k and n % k))
        if (
            certificate.get("chromatic_number") != k
            or certificate.get("minimum_possible_class_size_spread") != delta
        ):
            raise ValueError("planted certificate disagrees with the minimum palette")
        quotient, remainder = divmod(n, k)
        return {
            "status": "certificate",
            "D": delta,
            "Q": remainder * (quotient + 1) ** 2 + (k - remainder) * quotient**2,
            "nodes": 0,
        }
    if "equity_witness" in record:
        score = validate_coloring(record["intervals"], record["equity_witness"])
        if score["D"] != score["delta"] or score["k"] != k:
            raise ValueError("invalid equity witness")
        return {"status": "validated_witness", "D": score["D"], "Q": score["Q"], "nodes": 0}
    return tiny_optimum(record["intervals"], k, max_n=max_n, max_nodes=max_nodes)


def quality_record(record, lanes, starting, production, optimum, **dimensions):
    score = validate_coloring(record["intervals"], lanes)
    current_score = score["D"], score["Q"]
    starting_score = starting["D"], starting["Q"]
    if dimensions["method"] != "baseline" and current_score > starting_score:
        raise ValueError("production balancing regressed against its designated starting coloring")
    known = optimum["D"] is not None
    if known and score["D"] < optimum["D"]:
        raise ValueError("measured spread contradicts the claimed optimum")
    return {
        **dimensions,
        "status": "ok",
        **score,
        "starting_D": starting["D"],
        "starting_Q": starting["Q"],
        "production_D": production["D"],
        "production_Q": production["Q"],
        "improvement_D": starting["D"] - score["D"],
        "improvement_Q": starting["Q"] - score["Q"],
        "score_relation": "improved" if current_score < starting_score else "unchanged",
        "D_star": optimum["D"],
        "Q_star": optimum["Q"],
        "optimum_status": optimum["status"],
        "additive_gap": score["D"] - optimum["D"] if known else None,
        "optimum_hit": int(score["D"] == optimum["D"]) if known else None,
        "stop_reason": (
            "equity"
            if score["D"] == score["delta"]
            else "baseline"
            if dimensions["method"] == "baseline"
            else "zero_budget"
            if dimensions["max_work"] == 0
            else "not_exposed"
        ),
        "work_used": None,
        "pairs_checked": None,
        "flips": None,
        "skips": None,
        "lanes_sha256": hashlib.sha256(canonical_json(lanes).encode("ascii")).hexdigest(),
    }


def aggregate_quality(records):
    """Aggregate one quality row per instance, never timing samples or repetitions."""
    totals = {}
    for row in records:
        key = tuple(
            str(row[field])
            for field in ("corpus", "population", "workload", "engine", "method", "max_work")
        )
        item = totals.setdefault(
            key,
            {
                "corpus": key[0],
                "population": key[1],
                "workload": key[2],
                "engine": key[3],
                "method": key[4],
                "max_work": key[5],
                "instances": 0,
                "known_optima": 0,
                "optimum_hits": 0,
                "improved": 0,
                "unchanged": 0,
                "D_sum": 0,
                "additive_gap_sum": 0,
                "additive_gap_distribution": Counter(),
                "stop_reasons": Counter(),
                "unknown_optimum_reasons": Counter(),
                "worst": [],
            },
        )
        item["instances"] += 1
        item["D_sum"] += int(row["D"])
        item[row["score_relation"]] += 1
        item["stop_reasons"][row["stop_reason"]] += 1
        if row["D_star"] is not None and row["D_star"] != "":
            gap = int(row["additive_gap"])
            item["known_optima"] += 1
            item["optimum_hits"] += int(row["optimum_hit"])
            item["additive_gap_sum"] += gap
            item["additive_gap_distribution"][gap] += 1
            item["worst"].append(
                {"id": row["id"], "family": row["family"], "gap": gap, "D": int(row["D"])}
            )
            item["worst"].sort(key=lambda x: (-x["gap"], x["id"]))
            del item["worst"][5:]
        else:
            item["unknown_optimum_reasons"][row["optimum_status"]] += 1
    for item in totals.values():
        known = item["known_optima"]
        item["mean_D"] = item.pop("D_sum") / item["instances"]
        item["optimum_hit_rate"] = item["optimum_hits"] / known if known else None
        gap_sum = item.pop("additive_gap_sum")
        item["mean_additive_gap"] = gap_sum / known if known else None
    return list(totals.values())


def timing_summary(records):
    """Stream within-case medians; input preserves the runner's contiguous case order."""
    for case, rows in groupby(
        records,
        key=lambda row: tuple(row[field] for field in ("corpus", "id", "workload", "engine")),
    ):
        groups = defaultdict(list)
        for row in rows:
            groups[row["method"], str(row["max_work"])].append(int(row["total_ns"]))
        if ("baseline", "0") not in groups:
            raise ValueError("runtime comparison has no matching production baseline")
        baseline = statistics.median(groups["baseline", "0"])
        if baseline <= 0:
            raise ValueError("production baseline runtime must be positive")
        for (method, work), times in groups.items():
            median = statistics.median(times)
            yield {
                **dict(zip(("corpus", "id", "workload", "engine"), case, strict=True)),
                "method": method,
                "max_work": work,
                "median_ns": median,
                "min_ns": min(times),
                "max_ns": max(times),
                "samples": len(times),
                "runtime_ratio": median / baseline,
            }


def build_frame(record, workload):
    import polars as pl

    rows = record["intervals"]
    frame = pl.DataFrame(
        {
            "start": pl.Series([s for s, _ in rows], dtype=pl.Int64),
            "end": pl.Series([e for _, e in rows], dtype=pl.Int64),
        }
    )
    groups = 4 if workload == "grouped" else 1
    if workload in ("date", "datetime"):
        # Rank compression preserves every comparison/equality, avoids overflow,
        # and makes arbitrary valid Int64 corpora usable with Date's Int32 width.
        rank = {x: i for i, x in enumerate(sorted({x for row in rows for x in row}))}
        if len(rank) > 2**31 - 20_001:
            raise ValueError("temporal fixture rank does not fit Date")
        frame = pl.DataFrame(
            {
                "start": [rank[s] + 20_000 for s, _ in rows],
                "end": [rank[e] + 20_000 for _, e in rows],
            },
            schema={"start": pl.Int64, "end": pl.Int64},
        )
        dtype = pl.Date if workload == "date" else pl.Datetime("us", "Europe/Helsinki")
        frame = frame.lazy().with_columns(pl.col("start", "end").cast(dtype)).collect()
    elif workload == "multi_chunk":
        n = len(rows)
        frame = pl.DataFrame(
            [
                pl.concat([frame["start"][: n // 3], frame["start"][n // 3 :]], rechunk=False),
                pl.concat([frame["end"][: n // 2], frame["end"][n // 2 :]], rechunk=False),
            ]
        )
    elif workload == "sliced":
        pad = pl.DataFrame(
            {"start": [-1], "end": [-1]}, schema={"start": pl.Int64, "end": pl.Int64}
        )
        frame = pl.concat([pad, frame, pad]).slice(1, len(rows))
    if groups > 1:
        frame = pl.concat(
            [frame.lazy().with_columns(pl.lit(i).alias("group")).collect() for i in range(groups)]
        )
    return frame, groups


def lane_plan(frame, method, max_work, groups):
    import polars_intervals as pi

    if method == "baseline":
        expr = pi.assign_lanes("start", "end")
    elif method in ("repair", "repair_supplied"):
        expr = pi.assign_balanced_lanes("start", "end", initial_lanes="initial", max_work=max_work)
    else:
        expr = pi.assign_balanced_lanes("start", "end", max_work=max_work)
    if groups > 1:
        expr = expr.over("group")
    return frame.lazy().select(expr.alias("lanes"))


def validated_result(result, record, groups, expected=None):
    import polars as pl

    if result.dtype != pl.UInt32 or result.null_count():
        raise ValueError("lane output must be non-null UInt32")
    values = result.to_list()
    n = record["n"]
    if len(values) != n * groups:
        raise ValueError("output length differs from collection rows")
    first = values[:n]
    for group in range(groups):
        group_lanes = values[group * n : (group + 1) * n]
        validate_coloring(record["intervals"], group_lanes)
        if group_lanes != first:
            raise ValueError("identical order-preserving groups gave different lanes")
    if expected is not None and values != expected:
        raise ValueError("identical query options gave a nondeterministic output")
    return values


def run_metadata(args):
    import polars_intervals as pi

    corpora = {}
    for path in args.dataset:
        manifest = json.loads((path / "dataset.json").read_text(encoding="ascii"))
        hashes = {"dataset.json": sha256(path / "dataset.json")}
        for shard in manifest["shards"]:
            target = (path / shard["path"]).resolve()
            if not target.is_relative_to(path.resolve()):
                raise ValueError("shard path escapes dataset directory")
            hashes[shard["path"]] = sha256(target)
        corpora[str(path)] = {"manifest": manifest, "sha256": hashes}
    native = list(Path(pi.__file__).parent.glob("*.pyd")) + list(
        Path(pi.__file__).parent.glob("*.so")
    )
    source_paths = [
        Path(__file__),
        Path(__file__).with_name("balance_lanes_fixtures.py"),
        ROOT / "python/polars_intervals/__init__.py",
    ]
    source_paths.extend((ROOT / "crates").glob("**/*.rs"))
    return {
        **environment(),
        "arguments": vars(args),
        "build_profile": "release (operator-declared)",
        "plugin": pi.__file__,
        "native_sha256": {p.name: sha256(p) for p in native},
        "source_sha256": {str(p.relative_to(ROOT)): sha256(p) for p in source_paths},
        "corpora": corpora,
        "warmups": args.warmups,
        "samples": args.repeats,
        "timed_scope": "Lazy planning/optimization, full collect, plugin validation/preprocessing/refinement and lane Series retrieval. Baseline construction for repair, fixture casts/copies, expression construction, correctness, oracle and output destruction are excluded. Each repair reuses the same original baseline column.",
        "quality_scope": "One logical graph per instance/method/options; grouped workload repeats it four times. No weighting by timing repeats.",
        "diagnostics": "Public Polars Series API exposes no counters or local-optimality status. equity is independently proven from output; all other positive-budget stops are not_exposed. See separate core benchmark.",
        "comparison_scope": "Public Polars baseline, repair of same baseline, balanced constructor.",
        "memory": "Not measured; neither buffer capacity nor requested live heap nor RSS is claimed.",
        "runtime_ratio": "candidate median / same-instance baseline median; above 1 means slower",
        "status": "running",
    }


def parse_args(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dataset", type=Path, action="append", default=[])
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="new file prefix; existing output is never overwritten",
    )
    parser.add_argument("--handcrafted", action="store_true")
    parser.add_argument("--stress-sizes", type=int, nargs="*", default=[])
    parser.add_argument("--max-work", type=int, nargs="+", default=[100_000])
    parser.add_argument("--workloads", choices=WORKLOADS, nargs="+", default=["integer"])
    parser.add_argument("--engines", choices=("auto", "streaming"), nargs="+", default=["auto"])
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--oracle-max-n", type=int, default=10)
    parser.add_argument("--oracle-max-nodes", type=int, default=200_000)
    parser.add_argument(
        "--max-instances",
        type=int,
        default=None,
        help="explicit bounded prefix; recorded as truncated",
    )
    args = parser.parse_args(argv)
    try:
        integer(args.repeats, "repeats", 1)
        integer(args.warmups, "warmups")
        integer(args.oracle_max_n, "oracle-max-n")
        integer(args.oracle_max_nodes, "oracle-max-nodes", 1)
        if args.oracle_max_n > 12:
            raise ValueError("oracle-max-n may not exceed the hard cap of 12")
        if args.max_instances is not None:
            integer(args.max_instances, "max-instances", 1)
        for work in args.max_work:
            integer(work, "max-work")
            if work > 2**64 - 1:
                raise ValueError("max-work must fit UInt64")
        for size in args.stress_sizes:
            integer(size, "stress size", 8)
        if not args.dataset and not args.handcrafted:
            raise ValueError("provide --dataset or --handcrafted")
        if args.stress_sizes and not args.handcrafted:
            raise ValueError("--stress-sizes requires --handcrafted")
    except ValueError as error:
        parser.error(str(error))
    args.max_work = list(dict.fromkeys(args.max_work))
    args.workloads = list(dict.fromkeys(args.workloads))
    args.engines = list(dict.fromkeys(args.engines))
    unique_paths = {}
    for path in args.dataset:
        unique_paths.setdefault(path.resolve(), path)
    args.dataset = list(unique_paths.values())
    return args


def main(argv=None):
    import polars as pl

    args = parse_args(argv)
    paths = {
        name: Path(f"{args.output}.{name}")
        for name in (
            "quality.csv",
            "timings.csv",
            "timing-summary.csv",
            "instances.jsonl",
            "metadata.json",
            "summary.json",
        )
    }
    for path in paths.values():
        if path.exists():
            raise FileExistsError(f"refusing to overwrite historical result {path}")
    args.output.parent.mkdir(parents=True, exist_ok=True)
    metadata = run_metadata(args)

    def save_metadata():
        paths["metadata.json"].write_text(
            json.dumps(metadata, indent=2, default=str) + "\n", encoding="utf-8"
        )

    save_metadata()
    records = chain.from_iterable(
        ((path.as_posix(), record) for record in iter_dataset(path)) for path in args.dataset
    )
    if args.handcrafted:
        records = chain(
            records, (("handcrafted", record) for record in iter_handcrafted(args.stress_sizes))
        )
    completed = 0
    current_case = {}
    try:
        with (
            paths["quality.csv"].open("w", newline="", encoding="utf-8") as quality_file,
            paths["timings.csv"].open("w", newline="", encoding="utf-8") as timing_file,
            paths["instances.jsonl"].open("w", encoding="utf-8") as instance_file,
        ):
            quality_writer = csv.DictWriter(quality_file, fieldnames=QUALITY_FIELDS)
            timing_writer = csv.DictWriter(timing_file, fieldnames=TIMING_FIELDS)
            quality_writer.writeheader()
            timing_writer.writeheader()
            for corpus, record in records:
                current_case = {
                    "corpus": corpus,
                    "id": record.get("id"),
                    "phase": "input_validation",
                }
                if args.max_instances is not None and completed >= args.max_instances:
                    metadata["input_truncated"] = True
                    break
                stats = check_record(record)
                preserved = {
                    key: value
                    for key, value in record.items()
                    if key not in ("intervals", "supplied_lanes", "equity_witness")
                }
                preserved["corpus"] = corpus
                for name in ("supplied_lanes", "equity_witness"):
                    if name in record:
                        preserved[f"{name}_sha256"] = hashlib.sha256(
                            canonical_json(record[name]).encode("ascii")
                        ).hexdigest()
                k = max(1, stats["omega"]) if record["n"] else 0
                optimum = optimum_for(
                    record, k, max_n=args.oracle_max_n, max_nodes=args.oracle_max_nodes
                )
                instance_file.write(canonical_json(preserved) + "\n")
                for workload in args.workloads:
                    frame, groups = build_frame(record, workload)
                    for engine in args.engines:
                        current_case.update(
                            workload=workload, engine=engine, method="baseline", max_work=0
                        )
                        baseline_plan = lane_plan(frame, "baseline", 0, groups)
                        baseline_values = validated_result(
                            baseline_plan.collect(engine=engine)["lanes"], record, groups
                        )
                        production = validate_coloring(
                            record["intervals"], baseline_values[: record["n"]]
                        )
                        repair_frame = frame.with_columns(
                            pl.Series("initial", baseline_values, dtype=pl.UInt32)
                        )
                        methods = [("baseline", 0, baseline_plan, production)]
                        for work in args.max_work:
                            methods.extend(
                                [
                                    (
                                        "repair",
                                        work,
                                        lane_plan(repair_frame, "repair", work, groups),
                                        production,
                                    ),
                                    (
                                        "balanced",
                                        work,
                                        lane_plan(frame, "balanced", work, groups),
                                        production,
                                    ),
                                ]
                            )
                            if "supplied_lanes" in record:
                                supplied = record["supplied_lanes"]
                                supplied_frame = frame.with_columns(
                                    pl.Series("initial", supplied * groups, dtype=pl.UInt32)
                                )
                                methods.append(
                                    (
                                        "repair_supplied",
                                        work,
                                        lane_plan(supplied_frame, "repair_supplied", work, groups),
                                        validate_coloring(record["intervals"], supplied),
                                    )
                                )
                        expected = {}
                        common = {
                            "corpus": corpus,
                            "id": record["id"],
                            "family": record["family"],
                            "population": (record.get("provenance") or {}).get(
                                "catalog_kind", record["family"]
                            ),
                            "seed": record["seed"],
                            "order": record["params"].get("order", "unknown"),
                            "workload": workload,
                            "engine": engine,
                            "groups": groups,
                        }
                        for method, work, plan, starting in methods:
                            current_case.update(method=method, max_work=work, phase="quality")
                            values = validated_result(
                                plan.collect(engine=engine)["lanes"], record, groups
                            )
                            expected[method, work] = values
                            quality = quality_record(
                                record,
                                values[: record["n"]],
                                starting,
                                production,
                                optimum,
                                **common,
                                collection_n=frame.height,
                                method=method,
                                max_work=work,
                            )
                            quality_writer.writerow(quality)
                        for sample in range(-args.warmups, args.repeats):
                            shift = derive_seed([record["id"], workload, engine, sample]) % len(
                                methods
                            )
                            for method, work, plan, _ in methods[shift:] + methods[:shift]:
                                current_case.update(
                                    method=method, max_work=work, phase="timing", sample=sample
                                )
                                tick = perf_counter_ns()
                                result = plan.collect(engine=engine)["lanes"]
                                elapsed = perf_counter_ns() - tick
                                validated_result(result, record, groups, expected[method, work])
                                del result
                                if sample >= 0:
                                    timing_writer.writerow(
                                        {
                                            **common,
                                            "n": frame.height,
                                            "graph_n": record["n"],
                                            "method": method,
                                            "max_work": work,
                                            "sample": sample,
                                            "total_ns": elapsed,
                                        }
                                    )
                completed += 1
                for file in (quality_file, timing_file, instance_file):
                    file.flush()
                print(
                    f"verified {completed}: {record['family']} n={record['n']} k={k}",
                    file=sys.stderr,
                )
        with paths["quality.csv"].open(encoding="utf-8", newline="") as source:
            summary = {"quality": aggregate_quality(csv.DictReader(source))}
        with (
            paths["timings.csv"].open(encoding="utf-8", newline="") as source,
            paths["timing-summary.csv"].open("w", encoding="utf-8", newline="") as destination,
        ):
            writer = csv.DictWriter(
                destination,
                fieldnames=[
                    "corpus",
                    "id",
                    "workload",
                    "engine",
                    "method",
                    "max_work",
                    "median_ns",
                    "min_ns",
                    "max_ns",
                    "samples",
                    "runtime_ratio",
                ],
            )
            writer.writeheader()
            writer.writerows(timing_summary(csv.DictReader(source)))
        summary["timing_summary"] = str(paths["timing-summary.csv"])
        paths["summary.json"].write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
        metadata["status"] = "completed"
    except Exception as error:
        metadata["status"] = "failed"
        metadata["error"] = f"{type(error).__name__}: {error}"
        metadata["failed_case"] = current_case
        raise
    finally:
        metadata["completed_instances"] = completed
        metadata["completed_utc"] = datetime.now(UTC).isoformat()
        save_metadata()


if __name__ == "__main__":
    main()

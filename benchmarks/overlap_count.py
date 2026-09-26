"""Standalone benchmark; see benchmarks/README.md for methodology and usage."""

import argparse
import ctypes
import gc
import hashlib
import json
import os
import platform
import random
import subprocess
import sys
from array import array
from datetime import UTC, datetime
from importlib.metadata import version
from itertools import accumulate
from pathlib import Path
from statistics import median
from tempfile import TemporaryDirectory
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars.plugins import register_plugin_function
from polars.testing import assert_frame_equal


def plugin_query(
    frame: pl.DataFrame, grouped: bool = False, plugin_path: Path | None = None
) -> pl.LazyFrame:
    start, end = pl.col("start"), pl.col("end")
    if plugin_path is not None:
        # Historical reference plugins only accept integer endpoints.
        start, end = start.to_physical(), end.to_physical()
    count = (
        pi.overlap_count(start, end)
        if plugin_path is None
        else register_plugin_function(
            plugin_path=plugin_path,
            function_name="overlap_count_plugin",
            args=[start, end],
            is_elementwise=False,
        )
    )
    return frame.lazy().select((count.over("group") if grouped else count).alias("count"))


def native_count(started: pl.Expr, ended: pl.Expr) -> pl.Expr:
    # Ended is a subset of started, even for empty query intervals. Cast the
    # difference before subtracting self to avoid unsigned underflow at zero.
    return (
        pl.when(pl.col("start") < pl.col("end"))
        .then((started - ended).cast(pl.Int64) - 1)
        .otherwise(0)
        .cast(pl.UInt64)
        .alias("count")
    )


def native_query(
    frame: pl.DataFrame, grouped: bool = False, parallel: bool = False
) -> pl.LazyFrame:
    """Pure expressions, including empty intervals; assumes valid non-null inputs."""
    start, end = pl.col("start"), pl.col("end")
    nonempty = start < end
    started = start.filter(nonempty).sort().search_sorted(end, side="left")
    ended = end.filter(nonempty).sort().search_sorted(start, side="right")
    if parallel:
        # Separate columns let Polars schedule the two searches independently.
        if grouped:
            started, ended = started.over("group"), ended.over("group")
        return (
            frame.lazy()
            .with_columns(started=started, ended=ended)
            .select(native_count(pl.col("started"), pl.col("ended")))
        )
    count = native_count(started, ended)
    return frame.lazy().select(count.over("group") if grouped else count)


def native_asof_query(frame: pl.DataFrame, grouped: bool = False) -> pl.LazyFrame:
    """Sorted as-of joins replace binary searches with linear merge scans."""
    rows = frame.lazy().with_row_index("row")
    valid = rows.filter(pl.col("start") < pl.col("end"))
    by = ["group"] if grouped else []
    position = pl.int_range(1, pl.len() + 1, dtype=pl.UInt64)
    if grouped:
        position = position.over("group")
    starts = (
        valid.select(*by, bound_start="start")
        .sort([*by, "bound_start"])
        .with_columns(started=position)
    )
    ends = valid.select(*by, bound_end="end").sort([*by, "bound_end"]).with_columns(ended=position)
    return (
        rows.sort([*by, "start"])
        .join_asof(
            ends,
            left_on="start",
            right_on="bound_end",
            by=by or None,
            strategy="backward",
            check_sortedness=not grouped,
        )
        .sort([*by, "end"])
        .join_asof(
            starts,
            left_on="end",
            right_on="bound_start",
            by=by or None,
            strategy="backward",
            allow_exact_matches=False,
            check_sortedness=not grouped,
        )
        .sort("row")
        .select(native_count(pl.col("started").fill_null(0), pl.col("ended").fill_null(0)))
    )


def native_sweep_query(frame: pl.DataFrame, grouped: bool = False) -> pl.LazyFrame:
    """A second algorithmic baseline: sorted endpoint events and prefix counts."""
    rows = frame.lazy().with_row_index("row")
    weight = (pl.col("start") < pl.col("end")).cast(pl.Int64)
    events = pl.concat(
        [
            rows.select("row", "group", endpoint=pl.col(endpoint), kind=pl.lit(kind), weight=weight)
            for endpoint, kind in [("start", 1), ("end", 0)]
        ]
    ).sort(["group", "endpoint", "kind"] if grouped else ["endpoint", "kind"])
    started = (pl.col("weight") * (pl.col("kind") == 1)).cum_sum()
    ended = (pl.col("weight") * (pl.col("kind") == 0)).cum_sum()
    if grouped:
        started, ended = started.over("group"), ended.over("group")
    contribution = (
        pl.when(pl.col("weight") == 0)
        .then(0)
        .when(pl.col("kind") == 1)
        .then(-pl.col("ended"))
        .otherwise(pl.col("started") - 1)
    )
    return (
        events.with_columns(started=started, ended=ended)
        .group_by("row")
        .agg(contribution.sum().cast(pl.UInt64).alias("count"))
        .sort("row")
        .select("count")
    )


def join_query(frame: pl.DataFrame, grouped: bool = False) -> pl.LazyFrame:
    rows = frame.lazy().with_row_index("row")
    nonempty = rows.filter(pl.col("start") < pl.col("end"))
    predicates = [
        pl.col("start") < pl.col("end_right"),
        pl.col("start_right") < pl.col("end"),
    ]
    if grouped:
        predicates.append(pl.col("group") == pl.col("group_right"))
    counts = (
        nonempty.join_where(
            nonempty.select("start", "end", "group"),
            *predicates,
        )
        .group_by("row")
        # Every non-empty row matches itself exactly once, even with duplicates.
        .agg((pl.len().cast(pl.UInt64) - 1).alias("count"))
    )
    return (
        rows.select("row")
        .join(counts, on="row", how="left", maintain_order="left")
        .select(pl.col("count").fill_null(0))
    )


def make_frame(scenario: str, size: int, seed: int, groups: int = 1) -> pl.DataFrame:
    rng = random.Random(seed)
    starts = array(
        "q",
        (4 * (i // groups) if scenario == "sparse" else rng.randrange(1000) for i in range(size)),
    )
    max_length = 8 if scenario == "sparse" else 1000
    ends = array("q", (start + rng.randint(0, max_length) for start in starts))
    return (
        pl.DataFrame({"start": starts, "end": ends}, schema={"start": pl.Int64, "end": pl.Int64})
        .lazy()
        .with_columns((pl.int_range(pl.len()) % groups).alias("group"))
        .collect()
        .sample(fraction=1, shuffle=True, seed=seed)
    )


def queries_for(
    frame: pl.DataFrame, grouped: bool, compare_plugin: Path | None = None
) -> dict[str, pl.LazyFrame]:
    queries = {
        "plugin": plugin_query(frame, grouped),
        "native_expr": native_query(frame, grouped),
        "native_parallel": native_query(frame, grouped, parallel=True),
        "native_asof": native_asof_query(frame, grouped),
        "native_sweep": native_sweep_query(frame, grouped),
        "inequality_join": join_query(frame, grouped),
    }
    if compare_plugin is not None:
        queries["plugin_reference"] = plugin_query(frame, grouped, compare_plugin)
    return queries


def check_semantics(compare_plugin: Path | None = None) -> None:
    """Check the benchmark baseline against fixed expectations before timing."""
    examples = [
        ([], [], []),
        ([2], [5], [0]),
        ([1, 1], [1, 1], [0, 0]),
        ([3, 1, 2, 1, 5], [5, 3, 2, 3, 6], [0, 1, 0, 1, 0]),
        ([5, 0, 2, 6], [7, 10, 3, 9], [2, 3, 1, 2]),
        ([2, 0, 4, 0, 1], [2, 4, 4, 0, 3], [0, 1, 0, 0, 1]),
        ([-(2**63), 0, 2**63 - 2], [2**63 - 1, 0, 2**63 - 1], [1, 0, 1]),
    ]
    for starts, ends, expected in examples:
        frame = pl.DataFrame(
            {"start": starts, "end": ends},
            schema={"start": pl.Int64, "end": pl.Int64},
        )
        frame = frame.lazy().with_columns(pl.lit(0).alias("group")).collect()
        expected_frame = pl.DataFrame({"count": expected}, schema={"count": pl.UInt64})
        for query in queries_for(frame, False, compare_plugin).values():
            assert_frame_equal(query.collect(engine="in-memory"), expected_frame)

    # Native temporal inputs must use the same compiled plugin as physical integers.
    frame = make_frame("dense", 40, 123, groups=7)
    for dtype in (
        pl.Date,
        pl.Datetime("ms"),
        pl.Datetime("us"),
        pl.Datetime("ns"),
        pl.Datetime("us", "Europe/Helsinki"),
    ):
        temporal = frame.lazy().with_columns(pl.col("start", "end").cast(dtype)).collect()
        physical = temporal.lazy().with_columns(pl.col("start", "end").to_physical()).collect()
        for grouped in (False, True):
            assert_frame_equal(
                plugin_query(temporal, grouped).collect(), plugin_query(physical, grouped).collect()
            )

    # Independent quadratic oracle: do not rely on agreement with the plugin alone.
    rng = random.Random(123)
    for dtype in (pl.Int64, pl.UInt64, pl.Datetime("us")):
        for grouped in (False, True):
            starts = [rng.randrange(12) for _ in range(40)]
            ends = [s + rng.randrange(6) for s in starts]
            groups = [rng.randrange(7) for _ in starts]
            expected = [
                sum(
                    i != j
                    and a < b
                    and c < d
                    and a < d
                    and c < b
                    and (not grouped or groups[i] == groups[j])
                    for j, (c, d) in enumerate(zip(starts, ends))
                )
                for i, (a, b) in enumerate(zip(starts, ends))
            ]
            frame = pl.DataFrame({"start": starts, "end": ends, "group": groups})
            frame = frame.lazy().with_columns(pl.col("start", "end").cast(dtype)).collect()
            expected_frame = pl.DataFrame({"count": expected}, schema={"count": pl.UInt64})
            for query in queries_for(frame, grouped, compare_plugin).values():
                for engine in ("in-memory", "streaming"):
                    assert_frame_equal(query.collect(engine=engine), expected_frame)

            # Confirm list aggregation composes as well as .over().
            start, end = pl.col("start"), pl.col("end")
            valid = start < end
            expression = native_count(
                start.filter(valid).sort().search_sorted(end, side="left"),
                end.filter(valid).sort().search_sorted(start, side="right"),
            )
            actual = frame.lazy().group_by("group").agg(expression).sort("group").collect()
            expected_grouped = (
                frame.with_columns(plugin_query(frame, True).collect()["count"])
                .lazy()
                .group_by("group")
                .agg("count")
                .sort("group")
                .collect()
            )
            assert_frame_equal(actual, expected_grouped)


def measure(
    frame: pl.DataFrame,
    grouped: bool,
    warmups: int,
    repeats: int,
    max_join_rows: int,
    compare_plugin: Path | None = None,
) -> tuple[dict, pl.DataFrame]:
    queries = queries_for(frame, grouped, compare_plugin)
    expected = queries["plugin"].collect(engine="in-memory")
    directed_overlaps = int(expected["count"].sum())
    nonempty = frame.lazy().filter(pl.col("start") < pl.col("end"))
    nonempty_count = nonempty.select(pl.len()).collect().item()
    join_matches = directed_overlaps + nonempty_count
    # With group equality, Polars 1.44 uses a hash join then filters: bound its
    # candidate rows, not just final matches, to avoid a quadratic allocation.
    join_candidates = (
        nonempty.group_by("group")
        .len()
        .select((pl.col("len").cast(pl.UInt64) ** 2).sum())
        .collect()
        .item()
        if grouped
        else join_matches
    )
    skipped = {}
    if join_candidates > max_join_rows:
        skipped["inequality_join"] = f"{join_candidates} candidate rows > limit {max_join_rows}"
        del queries["inequality_join"]
    for query in queries.values():
        assert_frame_equal(query.collect(engine="in-memory"), expected)
    samples = {name: [] for name in queries}
    names = list(queries)
    rng = random.Random(2026)
    for iteration in range(warmups + repeats):
        rng.shuffle(names)
        for name in names:
            start = perf_counter_ns()
            result = queries[name].collect(engine="in-memory")
            elapsed = (perf_counter_ns() - start) / 1_000_000
            assert_frame_equal(result, expected)  # Outside the timed region.
            del result
            if iteration >= warmups:
                samples[name].append(elapsed)

    return {
        "overlapping_pairs": directed_overlaps // 2,
        "overlap_density": (
            directed_overlaps / (frame.height * (frame.height - 1)) if frame.height > 1 else 0.0
        ),
        "join_match_rows_including_self": join_matches,
        "join_candidate_rows": join_candidates,
        "empty_intervals": frame.height - nonempty_count,
        "skipped": skipped,
        "timings": {
            name: {
                "median_ms": median(values),
                "min_ms": min(values),
                "max_ms": max(values),
                "samples_ms": values,
            }
            for name, values in samples.items()
        },
    }, expected


def resident_memory() -> dict:
    """OS process high-water mark; no sampling thread and no extra dependency."""
    if sys.platform == "win32":
        from ctypes import wintypes

        class Counters(ctypes.Structure):
            _fields_ = [("cb", wintypes.DWORD), ("PageFaultCount", wintypes.DWORD)] + [
                (name, ctypes.c_size_t)
                for name in (
                    "PeakWorkingSetSize",
                    "WorkingSetSize",
                    "QuotaPeakPagedPoolUsage",
                    "QuotaPagedPoolUsage",
                    "QuotaPeakNonPagedPoolUsage",
                    "QuotaNonPagedPoolUsage",
                    "PagefileUsage",
                    "PeakPagefileUsage",
                )
            ]

        counters = Counters()
        counters.cb = ctypes.sizeof(counters)
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = wintypes.HANDLE
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        psapi.GetProcessMemoryInfo.argtypes = [
            wintypes.HANDLE,
            ctypes.POINTER(Counters),
            wintypes.DWORD,
        ]
        if not psapi.GetProcessMemoryInfo(
            kernel.GetCurrentProcess(), ctypes.byref(counters), counters.cb
        ):
            raise ctypes.WinError(ctypes.get_last_error())
        return {"rss_bytes": counters.WorkingSetSize, "peak_rss_bytes": counters.PeakWorkingSetSize}
    import resource

    scale = 1 if sys.platform == "darwin" else 1024
    return {
        "rss_bytes": None,
        "peak_rss_bytes": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss * scale,
    }


def memory_worker(
    path: str, method: str, grouped: str, order: str, compare_plugin: str = ""
) -> None:
    frame = pl.read_ipc(path, memory_map=False)
    # IPC reloads consolidate chunks. Restore the timed input's physical layout.
    layout = json.loads(Path(path).with_suffix(".chunks.json").read_text())
    columns = []
    for name, lengths in layout.items():
        offsets = accumulate([0, *lengths])
        columns.append(
            pl.concat(
                [frame[name].slice(offset, length) for offset, length in zip(offsets, lengths)],
                rechunk=False,
            )
        )
    frame = pl.DataFrame(columns)
    assert {s.name: [len(c) for c in s.get_chunks()] for s in frame} == layout
    if order == "start_sorted":
        frame = frame.lazy().set_sorted("start").collect()
    query = queries_for(frame, grouped == "True", Path(compare_plugin) if compare_plugin else None)[
        method
    ]
    query.collect_schema()
    gc.collect()
    before = resident_memory()
    result = query.collect(engine="in-memory")
    after = resident_memory()
    print(
        json.dumps(
            {
                "before": before,
                "after": after,
                "peak_rss_increase_bytes": after["peak_rss_bytes"] - before["peak_rss_bytes"],
                "input_chunks": {s.name: s.n_chunks() for s in frame},
                "result_hash_sum": int(result.hash_rows(seed=42).sum()),
            }
        )
    )


def measure_memory(
    frame: pl.DataFrame,
    grouped: bool,
    order: str,
    methods: list[str],
    expected: pl.DataFrame,
    compare_plugin: Path | None = None,
) -> dict:
    measurements = {}
    expected_hash = int(expected.hash_rows(seed=42).sum())
    with TemporaryDirectory(prefix="overlap-memory-") as directory:
        path = Path(directory) / "input.arrow"
        frame.write_ipc(path)
        path.with_suffix(".chunks.json").write_text(
            json.dumps({s.name: [len(c) for c in s.get_chunks()] for s in frame})
        )
        for method in methods:
            process = subprocess.run(
                [
                    sys.executable,
                    str(Path(__file__).resolve()),
                    "--memory-worker",
                    str(path),
                    method,
                    str(grouped),
                    order,
                    str(compare_plugin) if compare_plugin else "",
                ],
                capture_output=True,
                text=True,
                check=True,
                timeout=180,
            )
            measurement = json.loads(process.stdout)
            assert measurement.pop("result_hash_sum") == expected_hash
            measurements[method] = measurement
    return measurements


def plugin_hashes(path: Path) -> dict[str, str]:
    paths = [path] if path.is_file() else path.iterdir()
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
        if path.suffix in (".pyd", ".so")
    }


def main() -> None:
    if len(sys.argv) > 1 and sys.argv[1] == "--memory-worker":
        memory_worker(*sys.argv[2:])
        return
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--sizes", type=int, nargs="+", default=[1000, 10000, 100000, 1000000, 3000000]
    )
    parser.add_argument(
        "--scenarios",
        nargs="+",
        choices=["sparse", "dense"],
        default=["sparse", "dense"],
    )
    parser.add_argument("--seed", type=int, default=42)
    parser.add_argument(
        "--compare-plugin", type=Path, help="Also time a saved plugin binary or directory"
    )
    parser.add_argument("--groups", type=int, nargs="+", default=[1, 100])
    parser.add_argument(
        "--orders",
        nargs="+",
        choices=["shuffled", "start_sorted"],
        default=["shuffled", "start_sorted"],
    )
    parser.add_argument(
        "--dtypes", nargs="+", choices=["int64", "datetime_us"], default=["int64", "datetime_us"]
    )
    parser.add_argument("--max-join-rows", type=int, default=2_000_000)
    parser.add_argument(
        "--memory", action="store_true", help="Measure each method in a fresh process"
    )
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--output", type=Path, default=Path("target/benchmarks/overlap-count.json"))
    args = parser.parse_args()
    if args.compare_plugin is not None:
        args.compare_plugin = args.compare_plugin.resolve()
        if not args.compare_plugin.exists():
            parser.error("compare-plugin must point to a saved plugin binary or directory")
    if min(args.sizes) < 0 or args.warmups < 0 or args.repeats < 1:
        parser.error("sizes and warmups must be non-negative; repeats must be positive")
    if min(args.groups) < 1 or args.max_join_rows < 0:
        parser.error("groups must be positive and max-join-rows must be non-negative")

    repo = Path(__file__).resolve().parents[1]
    report = {
        "environment": {
            "timestamp_utc": datetime.now(UTC).isoformat(),
            "git_commit": subprocess.check_output(
                ["git", "rev-parse", "HEAD"], cwd=repo, text=True
            ).strip(),
            "git_dirty": bool(
                subprocess.check_output(
                    ["git", "status", "--porcelain"], cwd=repo, text=True
                ).strip()
            ),
            "python": platform.python_version(),
            "polars": pl.__version__,
            "polars_intervals": version("polars-intervals"),
            "platform": platform.platform(),
            "processor": platform.processor(),
            "logical_cpus": os.cpu_count(),
            "polars_threads": pl.thread_pool_size(),
            "engine": "in-memory",
            "script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "plugin_binary_sha256": plugin_hashes(Path(pi.__file__).parent),
            "reference_plugin_binary_sha256": (
                plugin_hashes(args.compare_plugin) if args.compare_plugin else None
            ),
            "core_source_sha256": hashlib.sha256(
                (repo / "crates/intervals-core/src/lib.rs").read_bytes()
            ).hexdigest(),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "rustflags": os.environ.get("RUSTFLAGS", ""),
        },
        "configuration": {
            key: str(value) if isinstance(value, Path) else value
            for key, value in vars(args).items()
            if key != "output"
        },
        "results": [],
    }
    check_semantics(args.compare_plugin)
    methods = list(queries_for(make_frame("sparse", 0, args.seed), False, args.compare_plugin))
    print(
        "scenario rows groups order dtype | " + " ".join(methods) + " (median ms)",
        flush=True,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    for scenario in args.scenarios:
        for size in args.sizes:
            for groups in args.groups:
                base = make_frame(scenario, size, args.seed, groups)
                for order in args.orders:
                    ordered = (
                        base.lazy().sort("start").collect() if order == "start_sorted" else base
                    )
                    for dtype in args.dtypes:
                        frame = (
                            ordered.lazy()
                            .with_columns(pl.col("start", "end").cast(pl.Datetime("us")))
                            .collect()
                            if dtype == "datetime_us"
                            else ordered
                        )
                        result, expected = measure(
                            frame,
                            groups > 1,
                            args.warmups,
                            args.repeats,
                            args.max_join_rows,
                            args.compare_plugin,
                        )
                        result.update(
                            scenario=scenario, rows=size, groups=groups, order=order, dtype=dtype
                        )
                        if args.memory:
                            result["memory"] = measure_memory(
                                frame,
                                groups > 1,
                                order,
                                list(result["timings"]),
                                expected,
                                args.compare_plugin,
                            )
                        report["results"].append(result)
                        values = " ".join(
                            f"{result['timings'][name]['median_ms']:.3f}"
                            if name in result["timings"]
                            else "skipped"
                            for name in methods
                        )
                        print(f"{scenario} {size} {groups} {order} {dtype} | {values}", flush=True)
                        # Preserve completed cases if a later case is interrupted.
                        args.output.write_text(
                            json.dumps(report, indent=2) + "\n", encoding="utf-8"
                        )
    print(f"Raw timings and environment: {args.output}")


if __name__ == "__main__":
    main()

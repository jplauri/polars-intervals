"""Complete eager and optional lazy calls, with release verification and raw samples."""

import argparse
import csv
import json
from pathlib import Path
from time import perf_counter_ns

import polars as pl
import polars_intervals as pi
from polars.testing import assert_frame_equal
from polars_intervals import _internal

if __package__:
    from .coverage_profile_native import native_profile
    from .provenance import ROOT, archive_sources, environment, sha256, verify_release
else:
    from coverage_profile_native import native_profile
    from provenance import ROOT, archive_sources, environment, sha256, verify_release

# family, order, dtype, groups (0=ungrouped), chunks, load, domain, include_zero
CASES = [
    ("sequencing", "start", "i64", 0, 1, "units", "inferred", False),
    ("sequencing", "shuffled", "i64", 0, 1, "units", "inferred", False),
    ("sequencing", "start", "i64", 0, 1, "ones", "inferred", False),
    ("booking", "start", "i64", 0, 1, "mixed", "inferred", False),
    ("booking", "end", "i64", 0, 1, "mixed", "partial", True),
    ("booking", "partial", "datetime_ns", 0, 7, "mixed", "extended", True),
    ("disjoint", "shuffled", "u64", 0, 1, "uniform", "extended", True),
    ("touching", "reverse", "i64", 0, 1, "units", "inferred", False),
    ("nested", "start", "i64", 0, 1, "units", "inferred", False),
    ("clique", "shuffled", "i64", 0, 1, "mixed", "inferred", False),
    ("repeated", "shuffled", "date", 0, 1, "units", "inferred", True),
    ("sparse", "shuffled", "u64", 0, 1, "mixed", "inferred", False),
    ("empty", "start", "i64", 0, 1, "units", "extended", True),
    ("zero", "start", "i64", 0, 1, "mixed", "inferred", False),
    ("booking", "shuffled", "i64", 0, 1, "mixed", "outside", True),
    ("booking", "shuffled", "i64", 32, 7, "mixed", "partial", True),
    ("sequencing", "shuffled", "datetime_ns", 1000, 7, "units", "inferred", False),
    ("repeated", "shuffled", "date", 32, 1, "mixed", "extended", True),
]


def fixture(n, case, seed):
    family, order, dtype_name, groups, chunks, mode, domain, zero = case
    dtype, offset = {
        "i64": (pl.Int64, 10),
        "u64": (pl.UInt64, 2**63),
        "date": (pl.Date, 20000),
        "datetime_ns": (pl.Datetime("ns", "Europe/Helsinki"), 2**53 + 1),
    }[dtype_name]
    j = pl.col("i") // max(1, groups)
    start, end = {
        "sequencing": (j, j + 8),
        "booking": (j, j + 1 + ((j * 1664525 + seed) % 511)),
        "disjoint": (j * 3, j * 3 + 2),
        "touching": (j, j + 1),
        "nested": (j, 2 * n - j + 1),
        "clique": (pl.lit(0), pl.lit(n + 1)),
        "repeated": (j // 64 * 4, j // 64 * 4 + 3),
        "sparse": (j * 10**9, j * 10**9 + 2),
        "empty": (j, j),
        "zero": (j, j + 8),
    }[family]
    weight = (
        pl.lit(1) if mode == "ones" else pl.lit(7) if mode == "uniform" else (j * 17 + seed) % 23
    )
    if family == "zero":
        weight = pl.lit(0)
    df = (
        pl.DataFrame({"i": pl.int_range(n, eager=True)})
        .lazy()
        .select(
            (start.cast(pl.Int128) + offset).cast(dtype).alias("start"),
            (end.cast(pl.Int128) + offset).cast(dtype).alias("end"),
            weight.cast(pl.UInt64).alias("q"),
            pl.when(pl.col("i") % max(1, groups) == 0)
            .then(None)
            .otherwise(pl.col("i") % max(1, groups))
            .alias("group"),
        )
        .collect()
    )
    if order == "shuffled":
        df = df.sample(fraction=1, shuffle=True, seed=seed)
    elif order == "end":
        df = df.sort("end")
    elif order == "reverse":
        df = df.reverse()
    elif order == "partial":
        half = n // 2
        df = pl.concat([df.head(half), df.slice(half).reverse()])
    if chunks > 1 and n:
        df = pl.DataFrame(
            [
                pl.concat([s.slice(i, step) for i in range(0, n, step)], rechunk=False)
                for c, s in enumerate(df)
                for step in [max(1, n // chunks + c)]
            ]
        )
    options = {
        "weight": None if mode == "units" else "q",
        "by": "group" if groups else None,
        "include_zero": zero,
    }
    if domain != "inferred":
        horizon = (n + max(1, groups) - 1) // max(1, groups)
        left, right = {
            "extended": (-1, horizon * 3 + 513),
            "partial": (horizon // 4, horizon // 2),
            "outside": (4 * horizon + 1000, 4 * horizon + 1100),
        }[domain]
        options.update(
            domain_start=pl.Series([offset + left], dtype=dtype),
            domain_end=pl.Series([offset + right], dtype=dtype),
        )
    return df, options


def statistics(df, options, output):
    positive = df.lazy().filter(pl.col("start") < pl.col("end"))
    if options["weight"]:
        positive = positive.filter(pl.col("q") > 0)
    if "domain_start" in options:
        positive = positive.with_columns(
            pl.max_horizontal("start", pl.lit(options["domain_start"]).first()).alias("start"),
            pl.min_horizontal("end", pl.lit(options["domain_end"]).first()).alias("end"),
        ).filter(pl.col("start") < pl.col("end"))
    positive = positive.collect()
    group = ["group"] if options["by"] else []
    coords = pl.concat(
        [
            positive.select(*group, pl.col("start").alias("x")),
            positive.select(*group, pl.col("end").alias("x")),
        ]
    )
    if "domain_start" in options:
        domains = (
            df.select(group).unique() if group else pl.DataFrame({"_collection": [0]})
        ).with_columns(
            pl.lit(options["domain_start"]).first().alias("start"),
            pl.lit(options["domain_end"]).first().alias("end"),
        )
    else:
        nonempty = df.lazy().filter(pl.col("start") < pl.col("end"))
        hull = [pl.col("start").min(), pl.col("end").max()]
        domains = (
            nonempty.group_by(group).agg(*hull) if group else nonempty.select(*hull)
        ).collect()
    domains = domains.filter(pl.col("start") < pl.col("end"))
    coords = pl.concat(
        [
            coords,
            domains.select(*group, pl.col("start").alias("x")),
            domains.select(*group, pl.col("end").alias("x")),
        ]
    )
    peak = pi.coverage_profile(positive, by=options["by"])["load"].max() or 0
    return positive.height, coords.n_unique(), output.height, peak


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", nargs="+", type=int, default=[0, 8, 1000, 10000, 100000])
    parser.add_argument("--seeds", nargs="+", type=int, default=[7, 41])
    parser.add_argument(
        "--cases", nargs="+", type=int, choices=range(len(CASES)), default=list(range(len(CASES)))
    )
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument(
        "--lazy",
        action="store_true",
        help="Also time lazy plan construction and collection, including the blocking profile step",
    )
    args = parser.parse_args()
    if min(args.sizes) < 0 or args.samples < 1 or args.warmups < 1:
        parser.error("Need nonnegative sizes and positive samples/warmups")
    native = Path(_internal.__file__)
    try:
        release, inputs = verify_release(native)
    except ValueError as error:
        parser.error(str(error))
    csv_path, metadata_path = (
        args.output.with_suffix(".csv"),
        args.output.with_suffix(".metadata.json"),
    )
    archive_path = args.output.with_suffix(".sources.zip")
    if csv_path.exists() or metadata_path.exists() or archive_path.exists():
        parser.error("Use a new output prefix")
    csv_path.parent.mkdir(parents=True, exist_ok=True)
    metadata = environment() | {
        "settings": vars(args) | {"output": str(args.output)},
        "cases": CASES,
        "scope": "Complete eager public/native calls including argument checks, native validation, partitioning, clipping, planning, sorting, scan, coalescing, output and key assembly. Fixture creation, correctness checks, and output destruction excluded.",
        "lazy_execution": (
            "The lazy and lazy_streaming methods also construct df.lazy(), the coverage_profile "
            "plan and schema, and collect inside timing. Both execute the profile as a blocking "
            "whole-frame operation; lazy_streaming selects Polars' streaming engine for the "
            "surrounding query, not independent batch profiling."
            if args.lazy
            else None
        ),
        "memory": "Not measured end-to-end. Separate Rust requested-live-heap instrumentation excludes FFI/Polars grouping and output Series.",
        "verification": "Every timed output matches the complete production output outside timing. Independent small membership/tick oracles live in tests and benchmarks/test_coverage_profile.py; large instances use candidate agreement only.",
        "u_definition": "Distinct contributing clipped endpoints plus nonempty domain bounds per group. m excludes zero/empty/clipped-away rows; omega counts contributing rows. Inferred bounds include zero-weight nonempty intervals.",
        "fixture_counts": "groups is the requested group count (0 means ungrouped), chunks is the per-column split target. Observed_groups and column_chunks record actual counts; small collections may have fewer observed groups and different chunk counts. Grouped partial domains use the per-group coordinate horizon.",
        "baseline_omissions": "No claimed omissions for valid public fixtures; invalid error classes/text may differ. Public <=64-bit weights and addressable row counts cannot overflow i128; wider core overflow tests are separate. Native uses signed subtraction because unary Int128 negation is unsupported in Polars 1.44.2.",
        "native_sha256": sha256(native),
        "release_sha256": sha256(release),
        "source_sha256": {
            str(p.relative_to(ROOT)): sha256(p)
            for p in [
                *inputs,
                Path(__file__),
                ROOT / "benchmarks/coverage_profile_native.py",
                ROOT / "python/polars_intervals/__init__.py",
            ]
        },
        "status": "running",
    }
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    metadata.update(
        archive_sources(
            archive_path,
            [
                *metadata["source_sha256"],
                "Cargo.toml",
                "rust-toolchain.toml",
                "uv.lock",
                "pyproject.toml",
                "benchmarks/provenance.py",
            ],
        )
    )
    with csv_path.open("w", newline="", encoding="utf-8") as out:
        writer = csv.writer(out)
        writer.writerow(
            [
                "case",
                "family",
                "order",
                "dtype",
                "groups",
                "chunks",
                "weights",
                "domain",
                "include_zero",
                "seed",
                "n",
                "m",
                "u",
                "z",
                "omega",
                "method",
                "sample",
                "ns",
                "observed_groups",
                "column_chunks",
            ]
        )
        for case_id in args.cases:
            case = CASES[case_id]
            for n in args.sizes:
                for seed in args.seeds:
                    df, options = fixture(n, case, seed)
                    expected = pi.coverage_profile(df, **options)
                    stats = statistics(df, options, expected)
                    observed_groups = df["group"].n_unique() if options["by"] else 1
                    column_chunks = "|".join(str(column.n_chunks()) for column in df)
                    methods = {"production": pi.coverage_profile, "native": native_profile}
                    if options["weight"] is None:
                        methods["native_count"] = lambda df, **kw: native_profile(
                            df, count_units=True, **kw
                        )
                    if args.lazy:
                        methods["lazy"] = lambda df, **kw: pi.coverage_profile(
                            df.lazy(), **kw
                        ).collect()
                        methods["lazy_streaming"] = lambda df, **kw: pi.coverage_profile(
                            df.lazy(), **kw
                        ).collect(engine="streaming")
                    for solve in methods.values():
                        assert_frame_equal(solve(df, **options), expected)
                        for _ in range(args.warmups):
                            solve(df, **options)
                    names = list(methods)
                    for sample in range(args.samples):
                        for name in names[sample % len(names) :] + names[: sample % len(names)]:
                            begin = perf_counter_ns()
                            result = methods[name](df, **options)
                            elapsed = perf_counter_ns() - begin
                            assert_frame_equal(result, expected)
                            del result
                            writer.writerow(
                                [
                                    case_id,
                                    *case,
                                    seed,
                                    n,
                                    *stats,
                                    name,
                                    sample,
                                    elapsed,
                                    observed_groups,
                                    column_chunks,
                                ]
                            )
                    out.flush()
            print(f"case {case_id}: {case[0]} complete", flush=True)
    metadata["status"] = "complete"
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

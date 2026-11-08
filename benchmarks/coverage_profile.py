"""Complete eager and optional lazy calls, with release verification and raw samples."""

import polars as pl
import polars_intervals as pi

if __package__:
    from .coverage_profile_native import native_profile
    from .provenance import polars_parser, run_polars
else:
    from coverage_profile_native import native_profile
    from provenance import polars_parser, run_polars

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
DIMENSIONS = ["family", "order", "dtype", "groups", "chunks", "weights", "domain", "include_zero"]
COLUMNS = [
    "case",
    *DIMENSIONS,
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
METHODS = {
    "production": pi.coverage_profile,
    "native": native_profile,
    "native_count": lambda df, **kw: native_profile(df, count_units=True, **kw),
    "lazy": lambda df, **kw: pi.coverage_profile(df.lazy(), **kw).collect(),
    "lazy_streaming": lambda df, **kw: pi.coverage_profile(df.lazy(), **kw).collect(
        engine="streaming"
    ),
}


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


def solve(df, _operation, method, options):
    return METHODS[method](df, **options)


def statistics(df, options, _operation, output):
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
    return {
        "m": positive.height,
        "u": coords.n_unique(),
        "z": output.height,
        "omega": peak,
        "observed_groups": df["group"].n_unique() if options["by"] else 1,
        "column_chunks": "|".join(str(column.n_chunks()) for column in df),
    }


def main():
    parser = polars_parser(__doc__, CASES, [0, 8, 1000, 10000, 100000])
    parser.add_argument(
        "--lazy",
        action="store_true",
        help="Also time lazy plan construction and collection, including the blocking profile step",
    )
    args = parser.parse_args()

    def methods(options):
        return [
            "production",
            "native",
            *(["native_count"] if options["weight"] is None else []),
            *(["lazy", "lazy_streaming"] if args.lazy else []),
        ]

    notes = {
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
    }
    run_polars(
        "coverage_profile",
        parser,
        args,
        CASES,
        DIMENSIONS,
        fixture,
        solve,
        statistics,
        notes,
        methods=methods,
        columns=COLUMNS,
        omit=("native_path", "release_path", "build_environment", "raw_sha256"),
    )


if __name__ == "__main__":
    main()

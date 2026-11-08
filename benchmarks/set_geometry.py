"""Compare complete public set operations with the native-Polars event plan.

Run with ``uv run --no-sync python -m benchmarks.set_geometry --output ...``
after rebuilding the release plugin. Inputs and correctness are outside timing.
"""

import polars as pl
import polars_intervals as pi

from benchmarks.provenance import polars_parser, run_polars
from benchmarks.set_geometry_native import native_set_geometry

# Geometry, independent orders, dtype, groups, matching, chunks, payload, source.
DIMENSIONS = [
    "family",
    "left_order",
    "right_order",
    "dtype",
    "groups",
    "matching",
    "chunks",
    "payload",
    "source",
]
CASES = [
    ("availability", "start", "start", "i64", 0, "all", 1, 0, "memory"),
    ("availability", "shuffled", "reverse", "i64", 0, "all", 1, 0, "memory"),
    ("dense", "shuffled", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("nested", "start", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("duplicates", "shuffled", "reverse", "date", 0, "all", 1, 0, "memory"),
    ("touching", "reverse", "start", "i64", 0, "all", 1, 0, "memory"),
    ("disjoint", "shuffled", "shuffled", "u64", 0, "all", 1, 0, "memory"),
    ("left_heavy", "partial", "reverse", "i64", 0, "all", 7, 0, "memory"),
    ("right_heavy", "start", "shuffled", "i64", 0, "all", 7, 0, "memory"),
    ("empty_right", "shuffled", "start", "i64", 0, "all", 1, 0, "memory"),
    ("availability", "shuffled", "reverse", "datetime_ns", 1000, "all", 7, 0, "memory"),
    ("availability", "partial", "shuffled", "i64", 32, "sparse", 7, 8, "memory"),
    ("genomic", "shuffled", "shuffled", "i64", 8, "all", 7, 0, "parquet"),
    ("skew", "shuffled", "reverse", "i64", 32, "all", 7, 0, "memory"),
    ("sparse", "partial", "reverse", "u64", 0, "all", 1, 0, "memory"),
    ("empty_left", "start", "shuffled", "i64", 0, "all", 1, 0, "memory"),
    ("empty_rows", "start", "shuffled", "i64", 32, "all", 7, 0, "memory"),
]
SCOPES = (
    "eager",
    "lazy_complete",
    "lazy_plan",
    "lazy_collect",
    "mixed_complete",
    "streaming_complete",
)


def fixture(total, case, seed):
    family, left_order, right_order, dtype, groups, matching, chunks, payload, _ = case
    left_n = total // 2
    if family == "left_heavy":
        left_n = max(0, total - min(8, total))
    elif family == "right_heavy":
        left_n = min(1, total)
    elif family == "empty_right":
        left_n = total
    elif family == "empty_left":
        left_n = 0
    endpoint_dtype = {
        "i64": pl.Int64,
        "u64": pl.UInt64,
        "date": pl.Date,
        "datetime_ns": pl.Datetime("ns", "UTC"),
    }[dtype]

    def side(n, right, order):
        df = pl.DataFrame({"i": pl.int_range(n, eager=True, dtype=pl.Int64)})
        i = pl.col("i")
        start = 10 * i + (2 if right else 0)
        end = start + (4 if right else 8)
        if family in ("dense", "genomic"):
            start = i * (2 if family == "dense" else 7) + right
            end = start + ((i * 37 + seed % 101) % 101) + 1
        elif family == "nested":
            start, end = i, 2 * total - i
        elif family == "duplicates":
            start, end = i % 8, i % 8 + 12
        elif family == "touching":
            start, end = i * 4 + right, i * 4 + right + 4
        elif family == "disjoint":
            start, end = i * 10 + (5 if right else 0), i * 10 + (8 if right else 3)
        elif family == "right_heavy" and not right:
            start, end = pl.lit(0), pl.lit(total * 10)
        elif family == "left_heavy" and right:
            start, end = i * (total + 1), i * (total + 1) + total // 2
        elif family == "sparse":
            start = i * 10_000_000_000 + right * 2
            end = start + (4 if right else 8)
        elif family == "empty_rows":
            end = start
        if dtype == "u64":
            start, end = start.cast(pl.UInt64) + 2**63, end.cast(pl.UInt64) + 2**63
        df = df.with_columns(
            start.cast(endpoint_dtype).alias("start"), end.cast(endpoint_dtype).alias("end")
        )
        if groups:
            group = i % groups
            if matching == "sparse" and right:
                group = group + groups - 1
            group = pl.when(i % 19 == 0).then(None).otherwise(group)
            if family == "skew":
                group = pl.when(i < n * 3 // 4).then(None).otherwise(group)
            df = df.with_columns(group.cast(pl.Int64).alias("group"))
        if order == "reverse":
            df = df.reverse()
        elif order == "shuffled":
            df = df.sample(fraction=1, shuffle=True, seed=(seed + right) % 2**64)
        elif order == "partial":
            df = df.sort(pl.col("i") // 64, -(pl.col("i") % 64))
        if payload:
            df = df.with_columns(
                *[pl.lit("irrelevant payload " * 20).alias(f"payload{i}") for i in range(payload)]
            )
        df = df.drop("i")
        if chunks > 1 and n:
            df = pl.concat(
                [df.slice(i, max(1, n // chunks)) for i in range(0, n, max(1, n // chunks))],
                rechunk=False,
            )
        return df

    return (
        side(left_n, False, left_order),
        side(total - left_n, True, right_order),
        {"by": "group" if groups else None},
    )


def solve(left, right, operation, method, options):
    if method == "native":
        return native_set_geometry(left, right, intersection=operation == "intersect", **options)
    return (pi.intersect_intervals if operation == "intersect" else pi.subtract_intervals)(
        left, right, **options
    )


def statistics(left, right, options, _operation, output):
    by = options["by"]
    sizes = left.group_by(by).len()["len"] if by else pl.Series([left.height])
    keys_left = left.select(by).unique() if by else None
    matched = (
        keys_left.join(right.select(by).unique(), on=by, nulls_equal=True).height
        if by
        else int(bool(left.height and right.height))
    )
    return {
        "left_n": left.height,
        "right_n": right.height,
        "left_useful": left.select((pl.col("start") < pl.col("end")).sum()).item(),
        "right_useful": right.select((pl.col("start") < pl.col("end")).sum()).item(),
        "p": pi.merge_intervals(left, **options).height,
        "q": pi.merge_intervals(right, **options).height,
        "z": output.height,
        "distinct_endpoints": pl.concat(
            [frame[name] for frame in (left, right) for name in ("start", "end")]
        ).n_unique(),
        "left_groups": len(sizes) if by else int(left.height > 0),
        "matched_groups": matched,
        "key_match_fraction": matched / len(sizes) if by and len(sizes) else int(bool(matched)),
        "largest_left_group": sizes.max() or 0,
        "largest_left_group_fraction": (sizes.max() or 0) / left.height if left.height else 0,
        "left_chunks": left["start"].n_chunks(),
        "right_chunks": right["start"].n_chunks(),
    }


def memory_fields(_frames, before, after, result):
    return {"before": before, "after": after, "z": result.height}


def main():
    parser = polars_parser(
        __doc__, CASES, [8, 1000, 10000, 100000], SCOPES, ("eager", "lazy_complete")
    )
    notes = {
        "scope": "Both-side validation, preparation, grouping, sorting, set scan/events, output construction and native crossing. Eager/lazy_complete/mixed_complete/streaming_complete include planning. lazy_plan excludes execution; lazy_collect uses a prebuilt plan. Input construction, correctness, returned-output destruction and memory observations excluded.",
        "memory": "Separate cold process eager call: before/after RSS and process peak RSS, including runtime, inputs and retained output. Peak increase after fixture creation is not allocation count or algorithm heap usage.",
        "verification": "Small original-row Boolean cell oracle in benchmarks/test_set_geometry.py. Every measured complete output checked outside timing against production. Large instances use cross-implementation agreement, not an independent large oracle.",
        "native_candidate": "D: all-row Polars validation behind a whole-combined-input blocking barrier, grouped signed endpoint events, per-source cumulative counts, Boolean membership and gap-aware coalescing. No Python row/group loops or nested collect.",
        "ordering": "Two warmups by default; candidate order rotates per sample; both methods fully materialize outputs.",
        "limitations": "Synthetic single-machine in-memory and separately labeled warm Parquet workloads. No out-of-core claim. No all-pairs join comparator or CPU hardware counters.",
    }
    run_polars(
        "set_geometry",
        parser,
        parser.parse_args(),
        CASES,
        DIMENSIONS,
        fixture,
        solve,
        statistics,
        notes,
        operations=("subtract", "intersect"),
        memory_fields=memory_fields,
    )


if __name__ == "__main__":
    main()

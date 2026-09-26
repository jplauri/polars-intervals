"""Rust-backed interval expressions for Polars."""

from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

__all__ = ["assign_lanes", "max_weight_non_overlapping", "overlap_count"]


def max_weight_non_overlapping(
    start: str | pl.Expr, end: str | pl.Expr, *, weight: str | pl.Expr
) -> pl.Expr:
    """Select a globally maximum-weight subset of mutually non-overlapping intervals.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        weight: Column name or expression producing signed or unsigned integer weights.

    Returns:
        pl.Expr: Non-null Boolean mask, one value per original row. True selects
            a row in one globally optimal subset. Empty input returns an empty
            mask. Identical inputs give deterministic results; a particular
            optimal subset on ties is not promised across releases or row permutations.

    Raises:
        polars.exceptions.PolarsError: For unequal lengths, null endpoints or
            weights, unsupported dtypes, mismatched endpoint dtypes, reversed
            intervals (reporting the original row index), or i128 objective overflow.
            Validation occurs when evaluated. No implicit casting or broadcasting.

    Notes:
        Intervals are half-open `[start, end)`: touching endpoints are compatible.
        Empty intervals `[x, x)` conflict with nothing; every positive empty row
        is selected, including duplicates at the same coordinate. Negative and
        zero-weight rows are omitted. The empty subset is allowed with value 0,
        so all-negative input returns all False.

        Weights accept Int8/16/32/64 and UInt8/16/32/64 only. Accumulation uses
        checked i128 arithmetic without passing through floating point. Float,
        Decimal, Boolean, temporal and other weight dtypes are rejected.
        Floating-point weights may be considered in a separate design.

        Endpoint support matches `overlap_count`: matching integer dtypes up
        to 64 bits, Date, or Datetime with exactly matching time unit and timezone
        metadata. Physical integer days/timestamps preserve temporal precision.

        Use directly in `df.filter(...)`, or `.over("group")` to solve each
        group independently. `group_by(...).agg(...)` returns lists of Boolean
        values. All chunks belong to the same instance. The full collection is
        required even with the streaming engine; filtering before optimization
        changes the instance being solved.

        The exact Rust dynamic program sorts by finish and start, sweeps
        compatible predecessors, and reconstructs the optimal subset in
        O(n log n) time and O(n) additional space. This is a global optimization,
        not a per-row decision or a greedy selection by weight or finish time.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({
        ...     "start": [0, 0, 4, 7], "end": [10, 4, 7, 10],
        ...     "revenue": [15, 10, 10, 10],
        ... })
        >>> selected = df.filter(
        ...     pi.max_weight_non_overlapping("start", "end", weight="revenue")
        ... )
        >>> selected["revenue"].sum()
        30
        >>> selected["start"].to_list()
        [0, 4, 7]

        The three shorter intervals beat the single largest-weight interval (15).
    """
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="max_weight_non_overlapping_plugin",
        args=[start, end, weight],
        is_elementwise=False,
    )


def assign_lanes(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Assign intervals to the minimum number of non-overlapping lanes.

    Useful for calendar/timeline layout, machine/resource lanes, Gantt charts,
    genomic tracks, and concurrent-job visualization.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.

    Returns:
        pl.Expr: Non-null `UInt32` lane IDs in original row order. IDs are
            contiguous `0..k-1`, where `k` is globally minimum for the input
            collection (or separately for each group). Identical input gives
            deterministic results. No particular optimal coloring or stable
            lane numbering across releases or row permutations is promised.

    Raises:
        polars.exceptions.PolarsError: If lengths differ, logical dtypes are
            mismatched or unsupported, endpoints contain nulls, any start
            exceeds its end, or lane IDs exceed the UInt32 range. Reversed
            intervals report the first invalid original row index. Validation
            happens when the expression is evaluated.

    Notes:
        Intervals are half-open `[start, end)`: touching intervals may share a
        lane. For non-empty intervals, minimum number of lanes = maximum
        concurrency. Empty intervals `[x, x)` consume no capacity and receive
        lane `0`. Nonempty input containing only empty intervals uses one lane;
        empty input returns empty output.

        Endpoint rules match `overlap_count`: both columns must have the same
        dtype among `Int8`, `Int16`, `Int32`, `Int64`, `UInt8`, `UInt16`, `UInt32`,
        `UInt64`, `Date`, and `Datetime`. Datetime units (`ms`, `us`, `ns`) and
        timezone metadata must match exactly. Physical integer days/timestamps
        preserve temporal precision without timezone arithmetic. No implicit
        coercion, scalar broadcasting, or null filling is performed. Float,
        Time, Duration, and other dtypes are unsupported.

        Use `.over("group")` to assign lanes independently within each group,
        or `group_by(...).agg(...)` for lists of lane IDs. Assignment needs the
        full collection, including all chunks, even with the streaming engine.
        Filtering before assignment changes the collection being colored.

        The Rust algorithm sorts starts and reuses the earliest-ending lane
        with a min-heap: O(n log n) sorting and O(n log max(2, k)) assignment,
        with O(n + k) additional space. It does not construct a graph.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 1, 2], "end": [2, 3, 4]})
        >>> result = df.lazy().with_columns(
        ...     pi.assign_lanes("start", "end").alias("lane")
        ... ).collect()
        >>> result["lane"].dtype
        UInt32
        >>> result["lane"].n_unique()
        2
        >>> result["lane"][0] == result["lane"][2]
        True

        The first and last intervals touch, so two lanes suffice for all three.
    """
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="assign_lanes_plugin",
        args=[start, end],
        is_elementwise=False,
    )


def overlap_count(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Count other overlapping half-open intervals `[start, end)` per row.

    Two non-empty intervals overlap iff `a.start < b.end` and
    `b.start < a.end`. Touching intervals do not overlap; empty intervals
    (`start == end`) count zero. Each row excludes itself, while duplicate
    non-empty intervals count each other.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.

    Returns:
        pl.Expr: Non-null `UInt64` counts, one per row in input order. Use
            `.alias(...)` to name the output. Empty input with supported
            endpoint dtypes produces an empty result.

    Raises:
        polars.exceptions.PolarsError: If input lengths differ, dtypes are
            mismatched or unsupported, either endpoint contains nulls, or any
            start exceeds its end. Inputs are validated when the expression
            is evaluated; a reversed interval reports its zero-based input
            index. Null rows are not skipped or filled.

    Notes:
        Both inputs must have equal lengths and the same dtype: `Int8`,
        `Int16`, `Int32`, `Int64`, `UInt8`, `UInt16`, `UInt32`, `UInt64`,
        `Date`, or `Datetime`. Datetime supports `ms`, `us`, and `ns`, including
        timezone-aware columns. Both the time unit and timezone metadata must
        match exactly; naive and timezone-aware Datetime do not match.
        Date/Datetime, temporal/integer, differing time units, and differing
        timezones are rejected. No casting or scalar broadcasting is performed.
        Time, Duration, floating-point, and other dtypes are unsupported.

        Temporal endpoints pass their native physical integer days or timestamps
        to the same Rust algorithm, without rounding or timezone arithmetic.

        Counts use the whole input collection, or each group when used with
        `.over(...)` or `group_by`. Filtering before this expression changes
        which intervals are compared; filtering afterwards only removes
        rows from the result.

        The Rust algorithm takes O(n log n) time and O(n) additional space,
        without materializing overlapping pairs. It needs the full input
        collection even when the query uses the streaming engine.

    Examples:
        Column names in a lazy query:

        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
        >>> result = df.lazy().with_columns(
        ...     pi.overlap_count("start", "end").alias("overlaps")
        ... ).collect()
        >>> result["overlaps"].to_list()
        [1, 1, 2, 0]
        >>> result.schema["overlaps"]
        UInt64

        Count within groups while keeping the original rows:

        >>> grouped = pl.DataFrame({
        ...     "group": ["a", "b", "a", "b"],
        ...     "start": [1, 1, 2, 5],
        ...     "end": [4, 4, 3, 6],
        ... })
        >>> result = grouped.lazy().with_columns(
        ...     pi.overlap_count("start", "end").over("group").alias("overlaps")
        ... ).collect()
        >>> result["overlaps"].to_list()
        [1, 0, 1, 0]

        Expressions in an eager `select`:

        >>> df.select(
        ...     pi.overlap_count(pl.col("start"), pl.col("end")).alias("overlaps")
        ... )["overlaps"].to_list()
        [1, 1, 2, 0]

        Datetime endpoints with a touching boundary at 10:00:

        >>> from datetime import datetime
        >>> appointments = pl.DataFrame({
        ...     "start": [datetime(2026, 1, 1, 9), datetime(2026, 1, 1, 9, 30),
        ...               datetime(2026, 1, 1, 10)],
        ...     "end": [datetime(2026, 1, 1, 10), datetime(2026, 1, 1, 10, 30),
        ...             datetime(2026, 1, 1, 11)],
        ... })
        >>> appointments.with_columns(
        ...     pi.overlap_count("start", "end").alias("overlaps")
        ... )["overlaps"].to_list()
        [1, 2, 1]

        Python `date` values similarly produce supported `pl.Date` columns.
    """
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="overlap_count_plugin",
        args=[start, end],
        is_elementwise=False,
    )

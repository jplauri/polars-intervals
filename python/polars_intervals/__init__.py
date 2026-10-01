"""Rust-backed interval expressions and optimization functions for Polars."""

import sys
from datetime import UTC, date, datetime
from pathlib import Path
from typing import overload

import polars as pl
from polars.plugins import register_plugin_function

__all__ = [
    "assign_balanced_lanes",
    "assign_lanes",
    "cluster_intervals",
    "containment_count",
    "coverage_profile",
    "intersect_intervals",
    "interval_gaps",
    "max_k_coverage",
    "max_weight_clique",
    "max_weight_non_overlapping",
    "max_weight_with_capacity",
    "max_weight_with_capacity_profile",
    "merge_intervals",
    "minimum_cost_cover",
    "minimum_cost_dominating_set",
    "minimum_cover",
    "minimum_stabbing_points",
    "nesting_depth",
    "overlap_count",
    "subtract_intervals",
]

_INTEGERS = (pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64)
_USIZE_MAX = 2 * sys.maxsize + 1


def _plugin(function_name: str, args: list, **options) -> pl.Expr:
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name=function_name,
        args=args,
        is_elementwise=False,
        **options,
    )


def _nonnegative(value: int, name: str, limit: int, range_name: str) -> str:
    """Validate an integer option, returning decimal text for the plugin kwargs."""
    if not isinstance(value, int) or isinstance(value, bool):
        raise TypeError(f"{name} must be a nonnegative integer")
    if not 0 <= value <= limit:
        raise ValueError(f"{name} must be nonnegative and fit in {range_name}")
    return str(value)


def _frame_keys(by: str | list[str] | None, reserved: tuple[str, ...]) -> tuple[str, ...]:
    if by is None:
        by = []
    elif isinstance(by, str):
        by = [by]
    elif not isinstance(by, list) or not all(isinstance(key, str) for key in by):
        raise TypeError("by must be a column name or a list of column names")
    if len(set(by)) != len(by):
        raise ValueError("by must contain distinct group keys")
    if any(key in reserved for key in by):
        raise ValueError(f"group keys cannot use reserved output names {'/'.join(reserved)}")
    # Freeze caller-owned arguments captured by deferred plans.
    return tuple(by)


def _frame_dtypes(endpoint_dtypes, key_dtypes, name: str) -> None:
    # Guard optional Arrow types before importing Series at the FFI boundary.
    for dtype in endpoint_dtypes:
        if dtype not in _INTEGERS and dtype != pl.Date and not isinstance(dtype, pl.Datetime):
            raise pl.exceptions.InvalidOperationError(
                f"{name} requires an 8-, 16-, 32-, or 64-bit "
                f"integer dtype, Date, or Datetime, got {dtype}"
            )
    for dtype in key_dtypes:
        if dtype not in (*_INTEGERS, pl.String, pl.Boolean, pl.Date) and not isinstance(
            dtype, pl.Datetime
        ):
            raise pl.exceptions.InvalidOperationError(
                f"{name} requires String, Boolean, 8/16/32/64-bit integer, "
                f"Date, or Datetime group keys, got {dtype}"
            )
    if endpoint_dtypes[0] != endpoint_dtypes[1]:
        raise pl.exceptions.InvalidOperationError(
            f"{name} requires matching integer, Date, or Datetime dtypes "
            "(including Datetime time unit and timezone)"
        )


def _geometry_input(intervals, start, end, by, name):
    if not isinstance(intervals, (pl.DataFrame, pl.LazyFrame)):
        raise TypeError("intervals must be a Polars DataFrame or LazyFrame")
    if not isinstance(start, str) or not isinstance(end, str):
        raise TypeError("start and end column names must be strings")
    by = _frame_keys(by, ("start", "end"))
    required = list(dict.fromkeys([start, end, *by]))
    # Names such as '*' and regex-looking names are literal column references.
    intervals = intervals.select(pl.selectors.by_name(required))
    schema = intervals.collect_schema()
    _frame_dtypes((schema[start], schema[end]), [schema[key] for key in by], name)
    output_schema = {**{key: schema[key] for key in by}, "start": schema[start], "end": schema[end]}
    return intervals, by, output_schema


def _blocking_frame(intervals, solve, schema):
    if isinstance(intervals, pl.DataFrame):
        return solve(intervals)
    return intervals.map_batches(
        solve,
        schema=schema,
        validate_output_schema=True,
        predicate_pushdown=False,
        projection_pushdown=False,
        slice_pushdown=False,
        streamable=False,
    )


def _set_geometry(left, right, left_start, left_end, right_start, right_end, by, intersection):
    from polars_intervals import _internal

    name = "intersect_intervals" if intersection else "subtract_intervals"
    for side, frame in (("left", left), ("right", right)):
        if not isinstance(frame, (pl.DataFrame, pl.LazyFrame)):
            raise TypeError(f"{side} must be a Polars DataFrame or LazyFrame")
    left, keys, schema = _geometry_input(left, left_start, left_end, by, name)
    right, _, right_schema = _geometry_input(right, right_start, right_end, list(keys), name)
    _frame_dtypes((schema["start"], right_schema["start"]), (), name)
    for key in keys:
        if schema[key] != right_schema[key]:
            raise pl.exceptions.InvalidOperationError(
                f"{name} requires matching left/right group key dtypes for {key!r}"
            )

    def normalize(frame, start, end):
        return frame.select(
            pl.selectors.by_name(start).alias("start"),
            pl.selectors.by_name(end).alias("end"),
            pl.selectors.by_name(keys),
        )

    def solve(lhs, rhs):
        return getattr(_internal, name)(
            lhs["start"],
            lhs["end"],
            [lhs[key] for key in keys],
            rhs["start"],
            rhs["end"],
            [rhs[key] for key in keys],
        )

    left = normalize(left, left_start, left_end)
    right = normalize(right, right_start, right_end)
    if isinstance(left, pl.DataFrame) and isinstance(right, pl.DataFrame):
        return solve(left, right)

    # Only keys survive normalization, so the tag name need avoid only keys.
    tag = "__pi_left"
    while tag in keys:
        tag += "_"

    def split(frame):
        # Vertical LazyFrame concatenation keeps all left rows before all right rows.
        n = frame[tag].sum()
        if not frame[tag].head(n).all():
            raise pl.exceptions.ComputeError(f"{name} received reordered operands")
        return solve(frame.head(n), frame.slice(n))

    combined = pl.concat(
        [
            left.lazy().with_columns(pl.lit(True).alias(tag)),
            right.lazy().with_columns(pl.lit(False).alias(tag)),
        ],
        how="vertical",
    )
    return _blocking_frame(combined, split, schema)


def cluster_intervals(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    include_touching: bool = False,
) -> pl.Expr:
    """Label connected components of half-open intervals in original row order.

    Args:
        start: Start column name or expression.
        end: End column name or expression with exactly the same endpoint dtype.
        include_touching: Connect touching nonempty intervals as well as overlaps.
            Must be a real Boolean. Strict overlap is the default.

    Returns:
        A non-null UInt32 expression with one ID per original row. IDs are
        contiguous from zero, assigned in order of each component's first input
        row. Use ``.alias(...)`` to name the result.

    Raises:
        TypeError: If include_touching is not Boolean.
        polars.exceptions.PolarsError: For null, reversed, mismatched or unsupported
            endpoints, unequal lengths or cluster IDs exceeding UInt32.

    Notes:
        Connectivity is transitive. Members need not all overlap each other.
        Every empty interval is an isolated singleton, even in touching mode.
        Duplicate nonempty intervals connect. Duplicate empty rows stay separate.
        Input permutations can renumber IDs while preserving the partition.

        Supports matching Int8/16/32/64, UInt8/16/32/64, Date and Datetime endpoints.
        Datetime units and timezone metadata must match exactly. All input rows
        are validated before sorting. Interval errors name original row indices
        within the collection or group. No casting or broadcasting is performed.

        Works in eager/lazy select, with_columns, windows and group aggregations.
        Every chunk in a group forms one collection, including streaming queries.
        The operation blocks on that collection. Time is O(n log n) and extra
        space is O(n), including canonical ID restoration. A verified start-sorted
        collection permits linear work. Downstream filters retain assigned IDs.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [4, 0, 2, 2], "end": [6, 2, 4, 2]})
        >>> df.select(pi.cluster_intervals("start", "end")).to_series().to_list()
        [0, 1, 2, 3]
        >>> df.select(
        ...     pi.cluster_intervals("start", "end", include_touching=True)
        ... ).to_series().to_list()
        [0, 0, 0, 1]
    """
    if not isinstance(include_touching, bool):
        raise TypeError("include_touching must be a Boolean")
    return _plugin(
        "cluster_intervals_plugin", [start, end], kwargs={"include_touching": include_touching}
    )


@overload
def merge_intervals(
    intervals: pl.DataFrame,
    *,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame: ...


@overload
def merge_intervals(
    intervals: pl.LazyFrame,
    *,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


def merge_intervals(
    intervals: pl.DataFrame | pl.LazyFrame,
    *,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame | pl.LazyFrame:
    """Return the canonical exact union of half-open intervals.

    Args:
        intervals: Polars DataFrame or LazyFrame. Returns the same frame kind.
        start: Input start column name.
        end: Input end column name with exactly matching logical dtype.
        by: Group column or ordered list of distinct group columns. None and []
            solve one ungrouped collection. Null group values compare equal.

    Returns:
        Columns ``[group keys..., start, end]``. Endpoint names are fixed even
        with custom source names. Each group's nonempty ranges are start-sorted,
        maximal and strictly separated. Overlapping and touching intervals always
        coalesce. Empty rows contribute nothing. No input metadata is aggregated.

    Raises:
        TypeError: For invalid frame, column name or grouping argument types.
        ValueError: For duplicate or reserved group names.
        polars.exceptions.PolarsError: For missing columns, null endpoints,
            reversed intervals, or unsupported/mismatched endpoint or key dtypes.

    Notes:
        Endpoints support matching Int8/16/32/64, UInt8/16/32/64, Date and Datetime.
        Exact logical dtypes, units and timezone metadata survive empty results.
        Keys support those types plus String and Boolean, including null values.
        Keys cannot be named start/end. Groups appear in first-observed input
        order before pruning. Groups with no union output contribute no rows.
        Every original interval is validated, with original input row indices.

        A LazyFrame remains deferred until collection. Schema resolution reads no
        rows. The Rust adapter groups, sorts and builds output with the GIL
        released. The lazy boundary materializes the whole input, including with
        the streaming engine. Filters, projections and slices after the call
        remain after the solve. Input operations before the call define its data.
        An entirely optimizer-pruned node need not execute validation.
        Time is O(n log n + z), space O(n + z), for z output ranges.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 2, 6, 8], "end": [2, 4, 8, 8]})
        >>> pi.merge_intervals(df).rows()
        [(0, 4), (6, 8)]
        >>> query = df.lazy().pipe(pi.merge_intervals)
        >>> isinstance(query, pl.LazyFrame)
        True
        >>> query.filter(pl.col("start") > 0).collect().rows()
        [(6, 8)]
    """
    from polars_intervals._internal import merge_intervals as solve

    intervals, by, schema = _geometry_input(intervals, start, end, by, "merge_intervals")
    return _blocking_frame(
        intervals, lambda frame: solve(frame[start], frame[end], [frame[key] for key in by]), schema
    )


@overload
def subtract_intervals(
    left: pl.DataFrame,
    right: pl.DataFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame: ...


@overload
def subtract_intervals(
    left: pl.LazyFrame,
    right: pl.DataFrame | pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


@overload
def subtract_intervals(
    left: pl.DataFrame,
    right: pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


def subtract_intervals(
    left: pl.DataFrame | pl.LazyFrame,
    right: pl.DataFrame | pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame | pl.LazyFrame:
    """Remove right coverage from the union of left half-open intervals.

    Args:
        left: DataFrame or LazyFrame describing coverage to keep.
        right: DataFrame or LazyFrame describing coverage to remove.
        left_start: Literal left start column name.
        left_end: Literal left end column name.
        right_start: Literal right start column name.
        right_end: Literal right end column name.
        by: Shared key name or ordered list of distinct names. None and [] solve
            one ungrouped pair of collections. Null key values match nulls.

    Returns:
        Columns ``[group keys..., start, end]`` with maximal nonempty ranges.
        Overlapping and touching fragments coalesce. Groups follow first
        appearance in the original left input, including empty rows. Ranges
        within each group are sorted and strictly separated. Only groups with
        results emit rows. Both DataFrames return a DataFrame. Either LazyFrame
        makes the result lazy. Inputs are preserved and payloads are omitted.

    Raises:
        TypeError: For invalid frame, column name or grouping argument types.
        ValueError: For duplicate keys or keys named start/end.
        polars.exceptions.PolarsError: For missing columns, unsupported or
            mismatched dtypes, null endpoints, or reversed intervals.

    Notes:
        This computes exact set difference, not whole-row removal or fragments
        attached to source records. Duplicates add no multiplicity. Empty rows
        contribute nothing. A left-only group returns its union. Right-only
        groups emit nothing, but all their rows still validate. An empty left
        input returns a typed empty result. An empty right returns the left union.

        All four endpoints must have the same Int8/16/32/64, UInt8/16/32/64,
        Date or Datetime dtype, including unit and timezone. Corresponding key
        dtypes must match exactly. Keys support those types plus String and
        Boolean. Logical types survive empty results. Every evaluated input row
        validates before pruning. Errors name the side, with left errors checked
        first. Reversed intervals also report their index within that side.

        Both lazy sources stay deferred through construction, explain and schema
        resolution. One blocking native call evaluates the complete collections
        with the GIL released, including under the streaming engine. This needs
        both inputs in memory. Downstream filters, projections and slices remain
        after the operation. Upstream transformations define the input rows.
        Entirely eliminated nodes need not execute validation.

        Geometry uses only exact endpoint comparisons. For n left rows, m right
        rows and z output ranges, time is O(n log n + m log m + z) and auxiliary
        space is O(n + m + z). Verified start order permits linear core work.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> left = pl.DataFrame({"start": [0, 4, 12], "end": [5, 10, 15]})
        >>> right = pl.DataFrame({"start": [2, 6, 10], "end": [3, 8, 13]})
        >>> pi.subtract_intervals(left, right).rows()
        [(0, 2), (3, 6), (8, 10), (13, 15)]
        >>> query = left.lazy().pipe(pi.subtract_intervals, right)
        >>> query.filter(pl.col("start") >= 8).collect().rows()
        [(8, 10), (13, 15)]
    """
    return _set_geometry(left, right, left_start, left_end, right_start, right_end, by, False)


@overload
def intersect_intervals(
    left: pl.DataFrame,
    right: pl.DataFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame: ...


@overload
def intersect_intervals(
    left: pl.LazyFrame,
    right: pl.DataFrame | pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


@overload
def intersect_intervals(
    left: pl.DataFrame,
    right: pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


def intersect_intervals(
    left: pl.DataFrame | pl.LazyFrame,
    right: pl.DataFrame | pl.LazyFrame,
    *,
    left_start: str = "start",
    left_end: str = "end",
    right_start: str = "start",
    right_end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame | pl.LazyFrame:
    """Return coordinates covered by both collections of half-open intervals.

    Args:
        left: DataFrame or LazyFrame describing the first collection.
        right: DataFrame or LazyFrame describing the second collection.
        left_start: Literal left start column name.
        left_end: Literal left end column name.
        right_start: Literal right start column name.
        right_end: Literal right end column name.
        by: Shared key name or ordered list of distinct names. Null keys match.

    Returns:
        Canonical ``[group keys..., start, end]`` ranges with the schema and
        left-first order rules of subtract_intervals. Both DataFrames return a
        DataFrame. Either LazyFrame returns a genuinely deferred LazyFrame.

    Raises:
        TypeError: For invalid frame, column name or grouping argument types.
        ValueError: For duplicate keys or keys named start/end.
        polars.exceptions.PolarsError: For missing columns, unsupported or
            mismatched dtypes, null endpoints, or reversed intervals.

    Notes:
        This intersects the unions of both inputs. It does not enumerate
        overlapping row pairs or preserve provenance. Touching alone contributes
        nothing. Duplicates and empty rows do not change coverage. Overlapping
        and touching output fragments coalesce. Either empty operand, or a key
        present on only one side, produces no ranges. Every row on both sides
        still validates before geometry, including unmatched groups.

        Exact endpoint/key types, side-relative errors, blocking lazy execution,
        optimizer safeguards and complexity follow subtract_intervals. Geometry
        is commutative. Swapping grouped inputs can change group presentation
        order because each call follows its own left input's first appearances.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> left = pl.DataFrame({"start": [0, 4, 12], "end": [5, 10, 15]})
        >>> right = pl.DataFrame({"start": [2, 6, 10], "end": [3, 8, 13]})
        >>> pi.intersect_intervals(left, right).rows()
        [(2, 3), (6, 8), (12, 13)]
        >>> pi.intersect_intervals(left, right.lazy()).collect().rows()
        [(2, 3), (6, 8), (12, 13)]
    """
    return _set_geometry(left, right, left_start, left_end, right_start, right_end, by, True)


@overload
def interval_gaps(
    intervals: pl.DataFrame,
    *,
    domain_start: int | date | datetime | pl.Series,
    domain_end: int | date | datetime | pl.Series,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame: ...


@overload
def interval_gaps(
    intervals: pl.LazyFrame,
    *,
    domain_start: int | date | datetime | pl.Series,
    domain_end: int | date | datetime | pl.Series,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.LazyFrame: ...


def interval_gaps(
    intervals: pl.DataFrame | pl.LazyFrame,
    *,
    domain_start: int | date | datetime | pl.Series,
    domain_end: int | date | datetime | pl.Series,
    start: str = "start",
    end: str = "end",
    by: str | list[str] | None = None,
) -> pl.DataFrame | pl.LazyFrame:
    """Return maximal uncovered ranges within an explicit half-open domain.

    Args:
        intervals: Polars DataFrame or LazyFrame. Returns the same frame kind.
        domain_start: Inclusive scalar bound, shared by every observed group.
        domain_end: Exclusive scalar bound. Both bounds are required.
        start: Input start column name.
        end: Input end column name with exactly matching logical dtype.
        by: Group column or ordered list of distinct group columns.

    Returns:
        Columns ``[group keys..., start, end]`` with the schema and order rules
        of merge_intervals. Gaps are sorted, strictly separated, nonempty and
        within the domain. Leading and trailing uncovered ranges are included.

    Raises:
        TypeError: For invalid frame/options or unsupported scalar types.
        ValueError: For duplicate/reserved keys or non-singleton/null Series bounds.
        polars.exceptions.PolarsError: For merge_intervals validation failures,
            reversed bounds, out-of-range bounds or mismatched scalar metadata.

    Notes:
        Validate every original row before clipping, including rows entirely
        outside the domain and calls with equal bounds. Empty intervals never
        split gaps. A fully covered or empty domain returns zero rows. Ungrouped
        empty/empty-only input returns the whole nonempty domain. Observed groups
        with no clipped coverage also return the whole domain. Grouped empty
        input has no observed keys and returns no rows. Absent groups require a
        future domain-table API. No infinite outer gaps are inferred.

        Python integers are range-checked against the endpoint dtype. Typed
        singleton Series bounds must match endpoint logical metadata exactly.
        Python date values match Date. Python datetime values have microsecond
        resolution and must match Datetime unit/timezone metadata. Use typed
        Series for millisecond/nanosecond bounds. Boolean bounds are rejected.

        Uses merge_intervals' genuine lazy execution, optimizer safeguards,
        grouping contract and whole-input materialization boundary. The same
        blocking fallback is used under the streaming engine. All geometry uses
        exact endpoint comparisons. Time is O(n log n + z), space O(n + z).

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 2, 6, 8], "end": [2, 4, 8, 8]})
        >>> pi.interval_gaps(df, domain_start=-1, domain_end=10).rows()
        [(-1, 0), (4, 6), (8, 10)]
        >>> df.clear().lazy().pipe(
        ...     pi.interval_gaps, domain_start=0, domain_end=10,
        ... ).collect().rows()
        [(0, 10)]
    """
    from polars_intervals._internal import interval_gaps as solve

    intervals, by, schema = _geometry_input(intervals, start, end, by, "interval_gaps")
    domain = (_target(domain_start), _target(domain_end))
    return _blocking_frame(
        intervals,
        lambda frame: solve(frame[start], frame[end], [frame[key] for key in by], domain),
        schema,
    )


def coverage_profile(
    intervals: pl.DataFrame | pl.LazyFrame,
    *,
    start: str = "start",
    end: str = "end",
    weight: str | None = None,
    by: str | list[str] | None = None,
    domain_start: int | date | datetime | pl.Series | None = None,
    domain_end: int | date | datetime | pl.Series | None = None,
    include_zero: bool = False,
) -> pl.DataFrame | pl.LazyFrame:
    """Compute exact coverage depth or resource load as canonical segment rows.

    For each coordinate ``t``, load is the sum of quantities on intervals with
    ``start <= t < end``. Omitted/None weights mean one per interval; columns
    named weight or load are ignored unless explicitly selected. A quantity is
    constant throughout its interval, not profit or quantity per unit duration.

    Args:
        intervals: Polars DataFrame or LazyFrame. Lazy input returns a deferred
            LazyFrame without collecting input rows during construction.
        start: Input start column name.
        end: Input end column name, with exactly the same logical dtype as start.
        weight: Optional nonnegative integer quantity column name.
        by: One group column or an ordered list of distinct group column names.
            None and [] solve one ungrouped collection.
        domain_start: Inclusive scalar domain bound; supply both bounds or neither.
        domain_end: Exclusive scalar domain bound. See minimum_cover's scalar
            rules: typed singleton Series preserve exact dtype/unit/timezone;
            Python datetime values have microsecond resolution.
        include_zero: Include zero-load gaps and tails inside the defined domain.

    Returns:
        pl.DataFrame | pl.LazyFrame: Same frame kind as the input, with columns
            ``[group keys..., start, end, load]``. New segment
            endpoints preserve the input endpoint dtype and load is always
            non-null Int128, including empty results. Groups appear in first
            input appearance order; segments are sorted by start within groups.

    Raises:
        TypeError: For non-frame input, invalid column/options types or unsupported
            domain scalar types. include_zero must be a Boolean.
        ValueError: For duplicate/reserved group names, a missing domain bound,
            or a target Series that is not one non-null value.
        polars.exceptions.PolarsError: For missing columns, null endpoints or
            quantities, mismatched/unsupported dtypes, reversed intervals/bounds,
            negative quantities or load exceeding Int128. Invalid row indices
            refer to the original input rows at the profile boundary, including
            grouped calls. For lazy input, arguments and schema are checked at
            construction; row values and native domain checks run at execution.

    Notes:
        Endpoints support Int8/16/32/64, UInt8/16/32/64, Date and Datetime with
        exactly matching unit/timezone metadata. Quantities support only those
        8/16/32/64-bit integers. No implicit casts, null filling or broadcasting
        occur. Group keys support String, Boolean, those integers, Date and
        Datetime, including null keys. Keys cannot use reserved output names
        start/end/load. All chunks form one instance per observed key tuple.

        Segments are half-open and nonempty. Touching equal loads coalesce even
        when the contributing rows change; equal loads across omitted gaps do
        not. Duplicates contribute independently. Empty intervals contribute
        neither load nor domain bounds, including empty rows with large weights.

        Without explicit bounds, each group's domain is the hull of ALL its
        nonempty intervals, including zero-weight intervals. Empty-only input
        has no inferred domain. With bounds, intervals are clipped after every
        input row is validated. Equal bounds produce no segments. Ungrouped empty
        input with nonempty bounds produces one zero segment in full mode;
        grouped empty input has no observed groups and always produces zero rows.
        There is no infinite zero tail. Overflow outside a clipped domain is
        irrelevant, while validation errors outside it still reject the call.

        The native sweep uses exact checked Int128 loads without integrating
        duration times load. Time is O(n log n + z), space O(n + z) for n input
        rows and z output segments; verified ordered endpoint streams permit
        linear work. Grouping, gathering and output assembly run natively with
        the GIL released. Lazy input resolves its schema during construction and
        defers the same native solve until execution. All rows and chunks are
        materialized at that boundary, including with the streaming engine:
        this is a whole-collection operation, not a streaming profile algorithm.
        Input expressions and filters before the call define the collection.
        Downstream filters, projections and slices stay after the profile node.
        Polars may eliminate an unneeded node entirely, such as for head(0) or
        a constant-false filter; validation runs only when that node executes.

        Coverage is a function over coordinates. It is distinct from per-row
        overlap_count, a selected maximum-weight clique, and row selection.
        Existing APIs accepting only 64-bit quantities require explicit checked
        narrowing, e.g. ``profile.with_columns(pl.col("load").cast(pl.UInt64,
        strict=True))`` when every output load fits; never narrow silently.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 2, 5], "end": [4, 5, 7], "demand": [2, 3, 3]})
        >>> pi.coverage_profile(df).rows()
        [(0, 2, 1), (2, 4, 2), (4, 7, 1)]
        >>> pi.coverage_profile(df, weight="demand").rows()
        [(0, 2, 2), (2, 4, 5), (4, 7, 3)]
        >>> pi.coverage_profile(
        ...     df, weight="demand", domain_start=-1, domain_end=8, include_zero=True,
        ... ).rows()
        [(-1, 0, 0), (0, 2, 2), (2, 4, 5), (4, 7, 3), (7, 8, 0)]
        >>> grouped = df.with_columns(pl.Series("resource", ["a", "b", "a"]))
        >>> pi.coverage_profile(grouped, by="resource", weight="demand").rows()
        [('a', 0, 4, 2), ('a', 5, 7, 3), ('b', 2, 5, 3)]
        >>> query = pi.coverage_profile(df.lazy(), weight="demand")
        >>> isinstance(query, pl.LazyFrame)
        True
        >>> query.filter(pl.col("load") >= 3).collect().rows()
        [(2, 4, 5), (4, 7, 3)]
    """
    from polars_intervals._internal import coverage_profile as solve

    if not isinstance(intervals, (pl.DataFrame, pl.LazyFrame)):
        raise TypeError("intervals must be a Polars DataFrame or LazyFrame")
    if not isinstance(start, str) or not isinstance(end, str):
        raise TypeError("start and end column names must be strings")
    if weight is not None and not isinstance(weight, str):
        raise TypeError("weight must be a column name or None")
    if not isinstance(include_zero, bool):
        raise TypeError("include_zero must be a Boolean")
    by = _frame_keys(by, ("start", "end", "load"))
    if (domain_start is None) != (domain_end is None):
        raise ValueError("domain_start and domain_end must both be supplied or both omitted")
    domain = None if domain_start is None else (_target(domain_start), _target(domain_end))
    if isinstance(intervals, pl.LazyFrame):
        required = list(dict.fromkeys([start, end, *by, *([] if weight is None else [weight])]))
        # by_name treats '*' and regex-looking source names literally.
        intervals = intervals.select(pl.selectors.by_name(required))
        schema = intervals.collect_schema()
        endpoint_dtypes = (schema[start], schema[end])
        weight_dtype = None if weight is None else schema[weight]
        key_dtypes = [schema[key] for key in by]
    else:
        starts, ends = intervals[start], intervals[end]
        weights = None if weight is None else intervals[weight]
        keys = [intervals[key] for key in by]
        endpoint_dtypes = (starts.dtype, ends.dtype)
        weight_dtype = None if weights is None else weights.dtype
        key_dtypes = [key.dtype for key in keys]
    _frame_dtypes(endpoint_dtypes, key_dtypes, "coverage_profile")
    if weight_dtype is not None and weight_dtype not in _INTEGERS:
        raise pl.exceptions.InvalidOperationError(
            "coverage_profile requires an 8-, 16-, 32-, or 64-bit "
            f"integer weight dtype, got {weight_dtype}"
        )
    if isinstance(intervals, pl.LazyFrame):
        output_schema = {
            **{key: schema[key] for key in by},
            "start": endpoint_dtypes[0],
            "end": endpoint_dtypes[1],
            "load": pl.Int128,
        }
        return _blocking_frame(
            intervals,
            lambda frame: solve(
                frame[start],
                frame[end],
                None if weight is None else frame[weight],
                [frame[key] for key in by],
                domain,
                include_zero,
            ),
            output_schema,
        )
    return solve(starts, ends, weights, keys, domain, include_zero)


def minimum_cost_dominating_set(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    cost: str | pl.Expr | None = None,
) -> pl.Expr:
    """Select an exact minimum-cost dominating set of the interval graph.

    Every input row is both a vertex requiring domination and a candidate.
    Each row must be selected or overlap a selected row. Selected vertices
    dominate themselves. Costs are minimized first; among equal-cost sets,
    the set with the fewest selected rows wins.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        cost: Nonnegative integer column or expression. Omitted or None means
            unit costs, without constructing a ones column. A column named
            ``cost`` is not used implicitly. Zero costs are valid.

    Returns:
        pl.Expr: Non-null Boolean mask, one value per original row. Empty input
            returns an empty mask. Every nonempty input selects at least one row.
            Identical logical inputs give deterministic results across chunks
            and eager/lazy execution. Tied masks may change across releases or
            row permutations; the objective is always exact.

    Raises:
        polars.exceptions.PolarsError: For unequal lengths, nulls, unsupported
            or mismatched dtypes, negative costs, reversed intervals, or an
            optimal total exceeding i128. Reversed intervals and negative
            costs report their original zero-based collection/group row index.
            Every row is validated before reduction or fast paths.

    Notes:
        Intervals are half-open ``[start, end)``. Distinct rows overlap exactly
        when both are nonempty and ``s_i < e_j and s_j < e_i``. Touching rows do
        not overlap. Every empty row ``[x, x)`` is an isolated vertex and MUST
        be selected, including duplicate empties, zero-cost empties, and
        empties geometrically inside nonempty intervals.

        This is ordinary domination. Selected nonempty intervals may overlap
        or be disjoint. It covers input vertices by overlap, not every point
        of a continuous coordinate target; use ``minimum_cost_cover`` for that.

        Endpoints must have identical Int8/16/32/64, UInt8/16/32/64, Date, or
        Datetime dtypes, including Datetime unit and timezone metadata. Explicit
        costs accept only Int8/16/32/64 or UInt8/16/32/64. Physical integer
        precision is retained; costs use checked i128 arithmetic. There is no
        implicit casting, null filling, or scalar broadcasting.

        All chunks form one collection, including with the streaming engine.
        ``.over("group")`` solves each whole group and preserves row alignment;
        grouped aggregation returns lists of Boolean values. Filtering BEFORE
        optimization changes both the demand vertices and selectable candidates.

        The Rust core takes O(n log n) time and O(n) extra space, including
        validation and reconstruction. Unit and uniform explicit costs (including zero) use
        greedy selection: for the earliest-ending undominated interval, choose
        its furthest-reaching overlapping representative. Heterogeneous costs
        use a min-heap prefix dynamic program after reduction to interval
        covering. Nonempty inclusion-minimal intervals suffice as domination
        targets: every other interval contains one. Each candidate overlaps a
        consecutive block of these targets. Every original nonempty row remains
        eligible as a candidate.

    Examples:
        Unit costs choose the middle of a three-row path. With an expensive
        middle row, the two outer rows are cheaper:

        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({
        ...     "start": [0, 3, 6], "end": [4, 7, 10], "price": [1, 10, 1],
        ... })
        >>> df.select(pi.minimum_cost_dominating_set("start", "end")).to_series().to_list()
        [False, True, False]
        >>> df.filter(pi.minimum_cost_dominating_set("start", "end", cost="price"))["start"].to_list()
        [0, 6]
        >>> grouped = df.with_columns(pl.lit("a").alias("group"))
        >>> grouped.lazy().with_columns(
        ...     pi.minimum_cost_dominating_set(pl.col("start"), pl.col("end"), cost="price")
        ...     .over("group").alias("selected")
        ... ).collect()["selected"].to_list()
        [True, False, True]
        >>> empties = pl.DataFrame({"start": [2, 2], "end": [2, 2], "price": [0, 0]})
        >>> empties.select(
        ...     pi.minimum_cost_dominating_set("start", "end", cost="price")
        ... ).to_series().to_list()
        [True, True]
    """
    return _plugin(
        "minimum_cost_dominating_set_plugin",
        [start, end] if cost is None else [start, end, cost],
    )


def max_weight_with_capacity_profile(
    jobs: pl.DataFrame,
    capacity_profile: pl.DataFrame | None = None,
    *,
    start: str = "start",
    end: str = "end",
    weight: str = "weight",
    profile_start: str = "start",
    profile_end: str = "end",
    capacity: str = "capacity",
) -> pl.Series:
    """Select a globally maximum-weight subset under a piecewise-constant capacity.

    Args:
        jobs: Eager DataFrame containing fixed job intervals and integer weights.
        capacity_profile: Eager DataFrame of capacity segments. If omitted or
            None, read the profile columns from ``jobs``.
        start: Job start column name.
        end: Job end column name.
        weight: Job weight column name.
        profile_start: Capacity segment start column name.
        profile_end: Capacity segment end column name.
        capacity: Nonnegative integer capacity column name.

    Returns:
        pl.Series: Non-null Boolean Series named ``selected``, aligned to the
            original rows of ``jobs``. True selects a row in a globally optimal
            subset. Identical inputs are deterministic; choices under objective
            ties are unspecified across releases or job row permutations.

    Raises:
        TypeError: If either input is not an eager DataFrame or a column name is
            not a string. Collect LazyFrames explicitly before calling.
        polars.exceptions.PolarsError: For missing columns, nulls, unsupported or
            mismatched dtypes, reversed intervals, overlapping profile segments,
            negative capacities, or checked i128 objective overflow.

    Notes:
        Jobs and profile rows are half-open ``[start, end)``. Each selected
        non-empty job consumes one unit of capacity throughout its lifetime.
        Profile rows may be unsorted. Overlapping non-empty segments are
        rejected; touching segments are allowed. Gaps and time outside the
        supplied profile have capacity zero. Valid empty profile rows have no
        effect, but all their values are still validated.

        Endpoint columns must have exactly matching Int8/16/32/64, UInt8/16/32/64,
        Date, or Datetime dtypes, including Datetime unit and timezone metadata.
        Weights and capacities accept signed/unsigned integers up to 64 bits;
        floats, Decimal and implicit casts are rejected. Objectives accumulate
        exactly in checked i128. Capacity may be clamped to the number of
        relevant non-empty jobs: no subset can use more capacity than that.

        Positive empty jobs consume no capacity and are always selected, even
        with an empty profile. Nonpositive weights are omitted. An empty subset
        is allowed. A profile constant at k throughout the relevant job horizon
        uses the existing ``max_weight_with_capacity(..., capacity=k)`` kernel;
        k=1 reaches the specialized non-overlapping weighted scheduler.

        Job and profile intervals are independent collections, even when their
        columns share one DataFrame; they are not paired by row. This eager
        function calls the native Rust optimizer directly with both collections,
        including all their chunks. It does not hide a LazyFrame collection or
        perform optimization in Python. Global exact optimization uses a compact
        timeline flow network whose size depends on breakpoints, never on elapsed
        Date/Datetime ticks.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> jobs = pl.DataFrame({
        ...     "start": [9, 9, 12, 14], "end": [18, 12, 14, 18],
        ...     "weight": [100, 60, 70, 80],
        ... })
        >>> profile = pl.DataFrame({
        ...     "start": [9, 12, 14], "end": [12, 14, 18], "capacity": [3, 1, 4],
        ... })
        >>> selected = pi.max_weight_with_capacity_profile(jobs, profile)
        >>> selected.to_list()
        [True, True, False, True]
        >>> jobs.filter(selected)["weight"].sum()
        240
    """
    from polars_intervals._internal import max_weight_with_capacity_profile as solve

    if capacity_profile is None:
        capacity_profile = jobs
    if not isinstance(jobs, pl.DataFrame) or not isinstance(capacity_profile, pl.DataFrame):
        raise TypeError("jobs and capacity_profile must be eager Polars DataFrames")
    if not all(
        isinstance(name, str) for name in (start, end, weight, profile_start, profile_end, capacity)
    ):
        raise TypeError("column names must be strings")
    columns = (
        jobs[start],
        jobs[end],
        jobs[weight],
        capacity_profile[profile_start],
        capacity_profile[profile_end],
        capacity_profile[capacity],
    )
    # Reject unsupported Arrow types before PySeries imports them: optional
    # Polars features (e.g. categorical/object) may panic at the FFI boundary.
    # Rust independently validates supported Series and performs all optimization.
    for column, role in ((columns[2], "weight"), (columns[5], "capacity")):
        if column.dtype not in _INTEGERS:
            raise pl.exceptions.InvalidOperationError(
                "max_weight_with_capacity_profile requires an 8-, 16-, 32-, or 64-bit "
                f"integer {role} dtype, got {column.dtype}"
            )
    for column in (columns[0], columns[1], columns[3], columns[4]):
        if (
            column.dtype not in _INTEGERS
            and column.dtype != pl.Date
            and not isinstance(column.dtype, pl.Datetime)
        ):
            raise pl.exceptions.InvalidOperationError(
                "max_weight_with_capacity_profile requires an 8-, 16-, 32-, or 64-bit "
                f"integer dtype, Date, or Datetime, got {column.dtype}"
            )
    return solve(*columns)


def max_k_coverage(start: str | pl.Expr, end: str | pl.Expr, *, k: int) -> pl.Expr:
    """Select at most `k` intervals whose union has maximum total measure.

    Among maximum-coverage solutions, use the fewest intervals.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression with exactly the same logical dtype.
        k: Nonnegative integer budget (Boolean is rejected).

    Returns:
        pl.Expr: Non-null Boolean mask, one value per original row. True selects
            the row in one globally optimal solution. Identical inputs give a
            deterministic result; no particular mask is promised under remaining ties.

    Raises:
        TypeError: If k is not an integer or is Boolean.
        ValueError: If k is negative or exceeds the platform usize range.
        polars.exceptions.PolarsError: For null endpoints, unequal lengths,
            unsupported or mismatched logical dtypes, or start greater than end.
            Reversed intervals report the original zero-based row within the group.

    Notes:
        Intervals are half-open [start, end). Empty intervals are valid and never
        selected. Empty input returns an empty mask; k=0 returns all False after
        validation. Each window/group independently uses the same scalar budget.
        Supports Int8/16/32/64, UInt8/16/32/64, Date, and Datetime ms/us/ns,
        including matching timezones. No coercion is performed. Measure is exact
        physical distance: days for Date, column ticks for Datetime, using i128
        internally. A large budget still excludes redundant intervals.

        Uses Li et al.'s exact offline two-state dynamic program, with containment
        pruning, a linear predecessor sweep, rolling objective rows and compact
        reconstruction decisions. Worst-case time O(n log n + min(k,n) n), space
        O(n + min(k,n) n). Validated k=0/1 takes O(n); a sufficient budget returns
        a minimum full-union cover after sorting in O(n log n).

        Longest-first is not exact: for [0,10), [-5,4), [6,15), k=2, the last
        two cover 18, whereas either pair containing the longest covers only 15.
        Top-k lengths also fails: [0,10) and [1,11) cover 11, while [0,10)
        and the shorter [10,18) cover 18.

    Examples:
        >>> df = pl.DataFrame({"start": [0, -5, 6], "end": [10, 4, 15]})
        >>> df.filter(max_k_coverage("start", "end", k=2))["start"].to_list()
        [-5, 6]
    """
    k = _nonnegative(k, "k", _USIZE_MAX, "the platform usize range")
    return _plugin("max_k_coverage_plugin", [start, end], kwargs={"k": k})


def minimum_stabbing_points(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Select the minimum number of discrete points that hit every half-open interval.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.

    Returns:
        pl.Expr: One list of globally optimal, sorted, unique coordinates, with dtype
            List(endpoint dtype). Identical geometry gives deterministic output.
            Empty input returns one empty list.

    Raises:
        polars.exceptions.PolarsError: For null endpoints, unequal lengths, unsupported
            or mismatched dtypes, reversed intervals, or any empty interval. Empty
            intervals raise ``cannot stab empty interval at index N`` with the original
            zero-based row index within the collection (or group).

    Notes:
        Intervals are half-open [start, end): end is excluded. Greedy selection uses
        the exact predecessor of the earliest uncovered end, never end itself.
        Endpoints support matching Int8/16/32/64, UInt8/16/32/64, Date, and Datetime.
        Date uses one day; Datetime uses one physical ms/us/ns tick and preserves
        timezone metadata. No floating-point or epsilon arithmetic is used.

        Each select or group is solved independently. Group aggregation returns one
        list per group; a window broadcasts that list to each row in the group.
        Sorting and scanning take O(n log n) time and O(n) additional space.
        Already nondecreasing ends use O(n) time and only output space.
        For intervals, the minimum stabbing number equals the maximum number of
        pairwise disjoint intervals.

    Examples:
        >>> df = pl.DataFrame({"start": [0, 2, 5], "end": [4, 6, 9]})
        >>> df.select(minimum_stabbing_points("start", "end")).to_series().to_list()
        [[3, 8]]
        >>> grouped = df.with_columns(pl.Series("group", ["a", "a", "b"]))
        >>> grouped.group_by("group", maintain_order=True).agg(
        ...     minimum_stabbing_points("start", "end").alias("points")
        ... ).to_dict(as_series=False)
        {'group': ['a', 'b'], 'points': [[3], [8]]}
    """
    return _plugin("minimum_stabbing_points_plugin", [start, end], returns_scalar=True)


def _target(value: int | date | datetime | pl.Series) -> dict:
    if isinstance(value, int) and not isinstance(value, bool):
        return {"kind": "integer", "value": str(value)}
    if isinstance(value, datetime):
        zone = None
        if value.tzinfo is UTC:
            zone = "UTC"
        elif value.tzinfo is not None:
            zone = getattr(value.tzinfo, "key", None) or getattr(value.tzinfo, "zone", None)
            if zone is None:
                raise TypeError("datetime target timezone must be UTC or named; use a typed Series")
        value = pl.Series([value])
        if value.dtype.time_zone != zone:
            raise TypeError("datetime target timezone metadata must be preserved exactly")
    elif isinstance(value, date):
        value = pl.Series([value])
    if not isinstance(value, pl.Series):
        raise TypeError("target must be an integer, date, datetime, or one-element typed Series")
    if len(value) != 1 or value.null_count():
        raise ValueError("target Series must contain exactly one non-null value")
    dtype = value.dtype
    if dtype in _INTEGERS:
        return {"kind": "integer", "value": str(value.item()), "dtype": str(dtype)}
    if dtype == pl.Date:
        return {"kind": "date", "value": str(value.to_physical().item())}
    if isinstance(dtype, pl.Datetime):
        return {
            "kind": "datetime",
            "value": str(value.to_physical().item()),
            "unit": dtype.time_unit,
            "timezone": dtype.time_zone,
        }
    raise TypeError("target must have a supported integer, Date, or Datetime dtype")


def minimum_cover(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    target_start: int | date | datetime | pl.Series,
    target_end: int | date | datetime | pl.Series,
) -> pl.Expr:
    """Select the fewest intervals whose union continuously covers a target interval.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        target_start: Scalar inclusive start of the target.
        target_end: Scalar exclusive end of the target.

    Returns:
        pl.Expr: Non-null Boolean mask in original row order, selecting one exact
            minimum-cardinality cover. Identical inputs give deterministic masks;
            a particular mask under ties is not promised across releases.

    Raises:
        TypeError: For unsupported target scalar types.
        ValueError: For a target Series that is not one non-null value.
        polars.exceptions.PolarsError: For reversed intervals/targets, null endpoints,
            unequal lengths, unsupported or incompatible dtypes, out-of-range
            targets, or a target that cannot be covered by the supplied intervals.

    Notes:
        Intervals and target are half-open: [start, end). Touching intervals chain
        perfectly. Empty targets select nothing; empty input intervals are never
        selected. Intervals may extend outside the target. Every input row is
        validated, including on empty targets. Infeasible non-empty targets raise.

        Endpoints support matching Int8/16/32/64, UInt8/16/32/64, Date, and Datetime
        dtypes. Python integers must fit the endpoint dtype exactly. Python dates
        require Date; Python datetimes require Datetime with microsecond units and
        matching timezone metadata (naive, UTC, or a named timezone). Use a typed
        Series for other timezone metadata. A one-element Series supplies an explicitly
        typed scalar, including millisecond/nanosecond Datetime targets. Its dtype
        must match exactly; no Date/Datetime, unit, timezone, or lossy numeric casts
        occur. Targets are scalar plugin configuration, never row-valued inputs.

        Use in eager select, lazy with_columns, or directly in df.filter(...).
        With .over("group"), each group covers the same scalar target independently;
        group_by aggregation produces Boolean lists. All chunks form one instance.
        The complete collection is required even with the streaming engine.

        The exact Rust greedy algorithm sorts packed candidates by start, then
        repeatedly selects the eligible interval reaching furthest right. It uses
        O(n log n) time and O(n) additional space. Equal effective ends prefer the
        original row index. Endpoints are compared without subtraction.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 0, 4, 6, 7], "end": [4, 6, 7, 10, 10]})
        >>> df.filter(pi.minimum_cover(
        ...     "start", "end", target_start=0, target_end=10,
        ... )).rows()
        [(0, 6), (6, 10)]

        Reaching 6 first permits a two-interval cover; choosing [0,4) first can
        require three intervals.
    """
    return _plugin(
        "minimum_cover_plugin",
        [start, end],
        kwargs={"target_start": _target(target_start), "target_end": _target(target_end)},
    )


def minimum_cost_cover(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    cost: str | pl.Expr,
    target_start: int | date | datetime | pl.Series,
    target_end: int | date | datetime | pl.Series,
) -> pl.Expr:
    """Select a minimum-cost set of intervals whose union continuously covers a target interval.

    Equal-cost ties use fewer intervals, preventing gratuitous zero-cost selections.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        cost: Column name or expression producing nonnegative integer costs.
        target_start: Scalar inclusive target start; see minimum_cover's scalar rules.
        target_end: Scalar exclusive target end; see minimum_cover's scalar rules.

    Returns:
        pl.Expr: Non-null Boolean mask in original row order, minimizing total cost
            and then selected count exactly. Tied masks are deterministic for
            identical input, but are not uniquely specified by the public API.

    Raises:
        TypeError: For unsupported target scalar types.
        ValueError: For a target Series that is not one non-null value.
        polars.exceptions.PolarsError: For minimum_cover's invalid/infeasible inputs,
            unsupported/null/negative costs, cost length mismatch, or i128 overflow.

    Notes:
        Target conversion, half-open semantics, validation, grouping, chunk handling,
        empty targets, and infeasibility follow minimum_cover. Empty intervals never
        help. Costs must be Int8/16/32/64 or UInt8/16/32/64 with no nulls or negatives.
        Float, Decimal, Boolean, temporal and Int128 costs are rejected without
        implicit casts. Accumulation is exact checked i128. Overflowing candidate
        paths cannot improve a representable optimum; an overflow error is raised
        if every feasible cover exceeds i128::MAX.

        The Rust solver uses exact frontier dynamic programming, a reversed Fenwick
        suffix-min tree, and backpointer reconstruction. It processes equal-right-end
        intervals as a batch so they cannot chain through one another. Time is
        O(n log n), additional space O(n). Internal objective ties use original row
        and predecessor coordinate order. Python performs no optimization.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({
        ...     "start": [0, 0, 5], "end": [10, 5, 10], "cost": [100, 10, 10],
        ... })
        >>> df.filter(pi.minimum_cost_cover(
        ...     "start", "end", cost="cost", target_start=0, target_end=10,
        ... )).rows()
        [(0, 5, 10), (5, 10, 10)]

        The minimum-cardinality answer uses one interval costing 100. The
        minimum-cost answer uses two intervals costing 20 in total.
    """
    return _plugin(
        "minimum_cost_cover_plugin",
        [start, end, cost],
        kwargs={"target_start": _target(target_start), "target_end": _target(target_end)},
    )


def max_weight_with_capacity(
    start: str | pl.Expr, end: str | pl.Expr, *, weight: str | pl.Expr, capacity: int
) -> pl.Expr:
    """Select a globally maximum-weight subset of intervals subject to a maximum simultaneous capacity.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        weight: Column name or expression producing signed or unsigned integer weights.
        capacity: Nonnegative integer maximum simultaneous selected non-empty intervals.

    Returns:
        pl.Expr: Non-null Boolean mask with one value per original row. True selects
            that row in an exact globally optimal subset. Empty input returns an
            empty mask. Ties are deterministic for identical input, but the exact
            optimal mask is not part of the stable public contract.

    Raises:
        TypeError: If capacity is not an integer (Boolean is rejected).
        ValueError: If capacity is negative or exceeds the platform usize range.
        polars.exceptions.PolarsError: For unequal lengths, null endpoints/weights,
            unsupported dtypes, mismatched endpoint logical dtypes, reversed intervals
            (with original row index), or checked i128 overflow. Validated on evaluation.

    Notes:
        Intervals are half-open `[start, end)`: touching endpoints do not overlap.
        Positive empty intervals `[x, x)` are always selected and consume no
        capacity. Negative and zero-weight rows are omitted. The empty subset
        is allowed with objective zero. Capacity zero permits only empty rows.
        Capacity one is equivalent in objective to `max_weight_non_overlapping`
        and delegates to that specialized dynamic program.

        Weights accept Int8/16/32/64 and UInt8/16/32/64, with exact checked i128
        accumulation. Null, floating-point, Decimal, Boolean, temporal and Int128
        weights are rejected. No implicit casts or scalar broadcasting occur.
        Endpoints accept matching integer dtypes up to 64 bits, Date, or Datetime
        with exactly matching units and timezone metadata, preserving precision.

        Use directly in `df.filter(...)`, in eager or lazy queries, or with
        `.over("group")` to optimize each group independently. Grouped aggregation
        returns Boolean lists. All chunks form one instance. The full collection
        is required even with the streaming engine. Filtering before optimization
        changes the instance being solved.

        The exact Rust solver uses an interval min-cost flow network with residual
        edges and successive shortest paths. Empty/nonpositive rows are removed;
        sufficient capacity selects all useful rows directly; independent overlap
        components are solved separately. For n rows and constrained components
        of sizes n_c, time is O(n log n + sum(capacity * n_c * log(n_c + 1)))
        and additional space is O(n). Capacity zero is O(n), capacity one O(n log n).
        Substantial independent component workloads use at most eight Rust workers;
        small inputs and single components remain serial.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({
        ...     "start": [0, 0, 4, 7], "end": [10, 4, 7, 10],
        ...     "weight": [15, 10, 10, 10],
        ... })
        >>> df.filter(pi.max_weight_with_capacity(
        ...     "start", "end", weight="weight", capacity=1,
        ... ))["weight"].sum()
        30
        >>> df.filter(pi.max_weight_with_capacity(
        ...     "start", "end", weight="weight", capacity=2,
        ... ))["weight"].sum()
        45

        Capacity one chooses the three shorter intervals. Capacity two allows
        the long interval to coexist with that schedule.
    """
    capacity = _nonnegative(capacity, "capacity", _USIZE_MAX, "the platform usize range")
    return _plugin(
        "max_weight_with_capacity_plugin", [start, end, weight], kwargs={"capacity": capacity}
    )


def max_weight_clique(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    weight: str | pl.Expr | None = None,
) -> pl.Expr:
    """Select one globally maximum-weight clique of overlapping intervals.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression with exactly the same logical dtype.
        weight: Integer weight column/expression, or None for unit weight per row.
            Omitting this argument always means units, even if a column named
            ``weight`` exists. The unit path does not construct a column of ones.

    Returns:
        pl.Expr: Non-null Boolean mask, one value per original row, selecting
            one maximum-weight clique. Empty input returns an empty mask.

    Raises:
        polars.exceptions.PolarsError: For unequal lengths, null endpoints or
            weights, unsupported or mismatched dtypes, reversed intervals, or
            checked i128 objective overflow. Reversed intervals report their
            original zero-based index within the collection or group. All rows
            are validated, including rows with nonpositive weights.

    Notes:
        A clique has every pair of distinct rows adjacent. Nonempty half-open
        intervals ``[start, end)`` are adjacent exactly when they overlap;
        touching intervals are not adjacent. An empty interval ``[x, x)`` is
        isolated: it may be a singleton clique but cannot be combined with any
        other row, including another empty at x. Duplicate nonempty rows are
        distinct vertices and their positive weights accumulate.

        This finds a *maximum* clique by total weight, not merely a *maximal*
        clique that cannot be enlarged. It optimizes weight only; there is no
        secondary cardinality objective. Zero and negative weights are omitted.
        The empty clique is allowed with objective zero, so all-nonpositive
        weights return all False. Unit weights select at least one row whenever
        the input is nonempty, even if every interval is empty.

        Ties use the earliest maximizing coordinate for nonempty cliques. A
        nonempty clique wins a tie with an empty singleton; equal-weight empty
        singletons choose the lowest original row index. Identical inputs give
        identical masks across chunks and eager/lazy execution. Explicit ones
        and omitted weights agree. Tied masks are not promised stable across
        releases or row permutations.

        Endpoints support matching Int8/16/32/64, UInt8/16/32/64, Date, or
        Datetime, including exactly matching time unit and timezone metadata.
        Physical integer days/timestamps preserve full precision. Explicit
        weights support Int8/16/32/64 and UInt8/16/32/64 only. Float, Decimal,
        Boolean, temporal, null-valued, and other weight dtypes are rejected.
        There is no implicit casting, null filling, or scalar broadcasting.

        Use ``df.filter(...)``, ``select``, or ``with_columns`` in eager or lazy
        queries. ``.over("group")`` solves each whole group in original row
        order; ``group_by(...).agg(...)`` returns lists of Boolean values. All
        chunks form one instance, including with the streaming engine. Filtering
        before optimization changes the instance being solved.

        Positive nonempty intervals in a clique share a point. The Rust core
        finds the greatest active weight at a start coordinate, compares it with
        the best empty singleton, and reconstructs the mask in one final pass.
        It uses O(n log n) time including sorting and O(n) additional space
        including output, without constructing an adjacency graph.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({
        ...     "start": [0, 1, 2, 10], "end": [5, 4, 3, 11],
        ...     "value": [1, 1, 1, 9], "weight": [0, 0, 0, 100],
        ... })
        >>> df.filter(pi.max_weight_clique("start", "end"))["start"].to_list()
        [0, 1, 2]
        >>> df.filter(
        ...     pi.max_weight_clique("start", "end", weight="value")
        ... )["start"].to_list()
        [10]
        >>> df.lazy().with_columns(
        ...     pi.max_weight_clique(pl.col("start"), "end", weight=None).alias("selected")
        ... ).collect()["selected"].to_list()
        [True, True, True, False]
    """
    return _plugin(
        "max_weight_clique_plugin",
        [start, end] if weight is None else [start, end, weight],
    )


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
    return _plugin("max_weight_non_overlapping_plugin", [start, end, weight])


def assign_balanced_lanes(
    start: str | pl.Expr,
    end: str | pl.Expr,
    *,
    initial_lanes: str | pl.Expr | None = None,
    max_work: int = 100_000,
) -> pl.Expr:
    """Assign a minimum number of lanes and heuristically balance their row counts.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.
        initial_lanes: Optional column name or expression containing a proper,
            contiguous minimum-lane assignment to improve. None constructs a new
            assignment. Supplied labels are validated; individual rows may move.
        max_work: Nonnegative integer up to 2**64 - 1. Defaults to 100,000
            deterministic search-work units, not milliseconds. Zero returns the
            supplied initial_lanes unchanged, or the existing ``assign_lanes``
            result when initial_lanes is None, after validating all inputs.

    Returns:
        pl.Expr: Non-null UInt32 lane IDs in original row order, with the minimum
            palette ``0..k-1``. Empty input returns no rows and uses zero lanes.

    Raises:
        TypeError: If max_work is not an integer, or is Boolean.
        ValueError: If max_work is negative or exceeds the UInt64 range.
        polars.exceptions.PolarsError: For unequal lengths, null endpoints,
            unsupported or mismatched logical dtypes, or reversed intervals
            (reporting the first original row index). Also rejects invalid initial
            lanes: unequal lengths, nulls, unsupported integer dtypes, negative or
            out-of-UInt32 values, sparse IDs, conflicts, or a nonminimum palette.
            Validation runs at zero budget and on empty or balanced collections.

    Notes:
        For lane cardinalities s, minimize ``(D, Q)`` lexicographically, where
        ``D = max(s) - min(s)`` and ``Q = sum(size * size for size in s)``.
        Exact integer comparisons count ROWS, including empty intervals, never
        occupied duration. A smaller D wins even with larger Q. The result is
        never worse than initial_lanes when supplied, otherwise never worse than
        ``assign_lanes`` on the same input under this score.
        Properness and the minimum lane count are guaranteed. Balance is a
        heuristic, without a global optimality or approximation-ratio guarantee.

        Half-open intervals [start, end) can touch in one lane. An empty [x, x)
        conflicts with nothing, even inside a nonempty interval. Unlike
        ``assign_lanes``, which always places empties in lane 0, this expression
        counts every empty row and may put it in ANY existing lane. It never
        adds lanes to spread empties. Nonempty input uses
        ``k = max(1, maximum_nonempty_concurrency)``; empty-only input uses one.

        Endpoints must have exactly matching Int8/16/32/64, UInt8/16/32/64,
        Date, or Datetime dtypes, including time unit and timezone metadata.
        Physical integer values retain precision. There is no coercion, scalar
        broadcasting, null filling, or Python row loop.

        Initial lane columns accept Int8/16/32/64 and UInt8/16/32/64, checked
        losslessly. IDs must be contiguous ``0..k-1`` and k must be minimum.
        Boolean, floats, temporal IDs and wider integers are unsupported. An
        invalid starting assignment is rejected, never silently replaced.

        All chunks form one collection. ``.over("group")`` solves each whole
        group and preserves row order; grouped aggregation returns lists of
        UInt32 IDs. The full collection is required even with streaming.
        Identical input and options give identical labels across chunking,
        eager/lazy execution, and equivalent order-preserving endpoint types.
        Row permutations need not preserve labels or heuristic quality.

        Without initial_lanes, the Rust core compares the existing baseline with
        forward and backward least-loaded-free-lane seeds, then repairs the best
        seed. With initial_lanes, it repairs the supplied assignment. Both paths
        use exact two-lane component subset sums. Repair is bounded by max_work,
        including pair enumeration, row scans, and bitset-word processing;
        an operation that cannot fit leaves the incumbent intact. Fixed
        validation and O(n log n) preparation are outside the work budget.
        Total time includes that preparation PLUS bounded search; scratch space
        is linear in the input, with checked allocation limits. Exhausting the
        budget does not establish pairwise local optimality. For two lanes,
        a completed exact pair operation gives globally optimal balance.
        A budget-limited result need not be idempotent: passing it as initial_lanes
        to another call can improve it further. Each whole group shares one budget.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 0, 1, 1], "end": [3, 3, 1, 1]})
        >>> labels = df.select(pi.assign_balanced_lanes("start", "end")).to_series()
        >>> sorted(labels.value_counts()["count"].to_list())
        [2, 2]
        >>> labels.dtype
        UInt32
        >>> df.lazy().select(
        ...     pi.assign_balanced_lanes(pl.col("start"), "end", max_work=0)
        ... ).collect().to_series().to_list() == df.select(
        ...     pi.assign_lanes("start", "end")
        ... ).to_series().to_list()
        True

        >>> df = pl.DataFrame({
        ...     "start": [0, 0, 1, 1], "end": [3, 3, 1, 1], "lane": [0, 1, 0, 0],
        ... })
        >>> result = df.lazy().with_columns(
        ...     pi.assign_balanced_lanes("start", "end", initial_lanes="lane").alias("balanced")
        ... ).collect()
        >>> sorted(result["balanced"].value_counts()["count"].to_list())
        [2, 2]
        >>> df.select(
        ...     pi.assign_balanced_lanes("start", "end", initial_lanes="lane", max_work=0)
        ... ).to_series().to_list()
        [0, 1, 0, 0]
    """
    max_work = _nonnegative(max_work, "max_work", 2**64 - 1, "UInt64")
    return _plugin(
        "assign_balanced_lanes_plugin",
        [start, end] if initial_lanes is None else [start, end, initial_lanes],
        kwargs={"max_work": max_work},
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
    return _plugin("assign_lanes_plugin", [start, end])


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
    return _plugin("overlap_count_plugin", [start, end])


def nesting_depth(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Return the length of the longest strict containment chain above each interval.

    Args:
        start: Column name or expression producing integer, Date, or Datetime starts.
        end: Column name or expression producing ends with the same logical dtype.

    Returns:
        pl.Expr: Non-null UInt64 depth for each original row, in deterministic row
            order. Outermost intervals have depth 0. Empty input returns empty output.

    Raises:
        polars.exceptions.PolarsError: For null endpoints, unequal lengths,
            unsupported or mismatched logical dtypes, or start greater than end.
            Reversed intervals report the first original zero-based row in the group.

    Notes:
        A strictly contains B exactly when ``A.start <= B.start`` and
        ``B.end <= A.end`` and at least one inequality is strict. Thus identical
        intervals never chain through each other and always receive equal depths.
        Equal starts with different ends, or equal ends with different starts,
        can form strict containment chains.

        Empty intervals follow the same endpoint predicate, not informal set
        containment: [0, 5) strictly contains [5, 5), giving depths [0, 1].
        Identical empty intervals such as [3, 3), [3, 3) both have depth 0.

        This differs from ``containment_count``, which counts other rows contained
        by each row, including duplicates. Counting containers is also different:
        [0, 8) and [2, 10) both contain [4, 5), but cannot contain each other, so
        the innermost interval has two containers and nesting depth only 1.

        Supports Int8/16/32/64, UInt8/16/32/64, Date, and Datetime ms/us/ns,
        including matching timezones. Logical dtypes must match exactly; no
        coercion or scalar broadcasting occurs. Temporal endpoints use their
        physical integer values without timezone arithmetic.

        Each select computes globally across all chunks. With ``.over("group")``,
        each group is solved independently and results retain row alignment.
        Group aggregation returns a list of depths per group. The complete
        collection is required, including with the streaming engine.

        The exact Rust dynamic program sorts packed records by start ascending
        and end descending, then maintains a monotone frontier of the greatest
        ending endpoint achievable at each chain length. It takes O(n log n)
        time and O(n) additional space. Identical geometry is processed atomically
        to prevent duplicate rows from creating extra levels.

    Examples:
        >>> df = pl.DataFrame({"start": [0, 1, 2, 3], "end": [10, 9, 8, 7]})
        >>> df.with_columns(nesting_depth("start", "end").alias("depth"))["depth"].to_list()
        [0, 1, 2, 3]
        >>> equal_starts = pl.DataFrame({"start": [1, 1, 1], "end": [10, 8, 5]})
        >>> equal_starts.select(nesting_depth("start", "end")).to_series().to_list()
        [0, 1, 2]
        >>> duplicates = pl.DataFrame({"start": [0, 2, 2, 3], "end": [10, 8, 8, 7]})
        >>> duplicates.select(nesting_depth("start", "end")).to_series().to_list()
        [0, 1, 1, 2]
        >>> empties = pl.DataFrame({"start": [0, 5, 5], "end": [5, 5, 5]})
        >>> empties.select(nesting_depth("start", "end")).to_series().to_list()
        [0, 1, 1]
    """
    return _plugin("nesting_depth_plugin", [start, end])


def containment_count(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Count how many other intervals are contained by each row.

    A contains B iff `A.start <= B.start` and `B.end <= A.end`, with self
    excluded. Intervals are represented as half-open `[start, end)`, but
    containment uses exactly these non-strict endpoint inequalities.

    Duplicate rows count one another: each of m identical rows counts m - 1.
    Empty intervals follow the same predicate: `[0, 5)` contains `[5, 5)`;
    identical `[3, 3)` rows contain each other. An empty outer interval only
    contains empty intervals at its own coordinate.

    Unlike `overlap_count`, crossing intervals do not count one another.
    For A = `[0, 10)`, B = `[2, 5)`, C = `[4, 12)`, A has overlap count 2
    and containment count 1.

    Args:
        start: Column name or expression producing interval starts.
        end: Column name or expression producing interval ends.

    Returns:
        pl.Expr: Non-null `UInt64` counts, one per original input row.
            Empty input with a supported dtype returns empty output.

    Raises:
        polars.exceptions.PolarsError: If lengths differ, logical dtypes do
            not match exactly, endpoints are null, dtypes are unsupported,
            or any start exceeds its end. Invalid intervals report the first
            zero-based index within the input collection (or group).

    Notes:
        Supports `Int8`, `Int16`, `Int32`, `Int64`, `UInt8`, `UInt16`,
        `UInt32`, `UInt64`, `Date`, and `Datetime` (ms/us/ns, including
        timezone-aware types). Datetime units and timezones must match.
        No silent coercion, scalar broadcasting, or null skipping is performed.

        Counts are exact over the entire input collection, across chunks.
        Use `.over("group")` to count separately within each group while
        preserving row order. `group_by(...).agg(...)` returns lists of counts.
        Filtering before counting changes the collection; filtering afterwards
        only removes result rows. Streaming still requires the full collection.

        The Rust core sorts packed records by descending start, compresses end
        coordinates, and counts with a Fenwick tree. It inserts each complete
        equal-start group before querying inclusive end prefixes and subtracting
        self. Time is O(n log n), auxiliary memory is O(n), without enumerating
        containment pairs.

    Examples:
        >>> import polars as pl
        >>> import polars_intervals as pi
        >>> df = pl.DataFrame({"start": [0, 2, 4], "end": [10, 5, 12]})
        >>> df.with_columns(
        ...     pi.containment_count("start", "end").alias("contained")
        ... )["contained"].to_list()
        [1, 0, 0]
        >>> df.lazy().select(
        ...     pi.containment_count(pl.col("start"), pl.col("end"))
        ... ).collect().to_series().dtype
        UInt64
        >>> grouped = pl.DataFrame({
        ...     "group": ["a", "b", "a", "b"],
        ...     "start": [0, 0, 2, 5], "end": [10, 4, 5, 5],
        ... })
        >>> grouped.select(
        ...     pi.containment_count("start", "end").over("group")
        ... ).to_series().to_list()
        [1, 0, 0, 0]
    """
    return _plugin("containment_count_plugin", [start, end])

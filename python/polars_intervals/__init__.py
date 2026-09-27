"""Rust-backed interval expressions and optimization functions for Polars."""

from datetime import UTC, date, datetime
from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

__all__ = [
    "assign_lanes",
    "containment_count",
    "max_k_coverage",
    "max_weight_non_overlapping",
    "max_weight_with_capacity",
    "max_weight_with_capacity_profile",
    "minimum_cost_cover",
    "minimum_cover",
    "minimum_stabbing_points",
    "overlap_count",
]


def max_weight_with_capacity_profile(
    jobs: pl.DataFrame,
    capacity_profile: pl.DataFrame,
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
        capacity_profile: Independent eager DataFrame of capacity segments.
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

        This eager function calls the native Rust optimizer directly with two
        independent collections, including all their chunks. It does not hide a
        LazyFrame collection or perform optimization in Python. Global exact
        optimization uses a compact timeline flow network whose size depends
        on breakpoints, never on elapsed Date/Datetime ticks.

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
    integers = (pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64)
    for column, role in ((columns[2], "weight"), (columns[5], "capacity")):
        if column.dtype not in integers:
            raise pl.exceptions.InvalidOperationError(
                "max_weight_with_capacity_profile requires an 8-, 16-, 32-, or 64-bit "
                f"integer {role} dtype, got {column.dtype}"
            )
    for column in (columns[0], columns[1], columns[3], columns[4]):
        if (
            column.dtype not in integers
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
    import sys

    if not isinstance(k, int) or isinstance(k, bool):
        raise TypeError("k must be a nonnegative integer")
    if not 0 <= k <= 2 * sys.maxsize + 1:
        raise ValueError("k must be nonnegative and fit in the platform usize range")
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="max_k_coverage_plugin",
        args=[start, end],
        kwargs={"k": str(k)},
        is_elementwise=False,
    )


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
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="minimum_stabbing_points_plugin",
        args=[start, end],
        is_elementwise=False,
        returns_scalar=True,
    )


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
    if dtype in (pl.Int8, pl.Int16, pl.Int32, pl.Int64, pl.UInt8, pl.UInt16, pl.UInt32, pl.UInt64):
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
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="minimum_cover_plugin",
        args=[start, end],
        kwargs={"target_start": _target(target_start), "target_end": _target(target_end)},
        is_elementwise=False,
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
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="minimum_cost_cover_plugin",
        args=[start, end, cost],
        kwargs={"target_start": _target(target_start), "target_end": _target(target_end)},
        is_elementwise=False,
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
    import sys

    if not isinstance(capacity, int) or isinstance(capacity, bool):
        raise TypeError("capacity must be a nonnegative integer")
    if not 0 <= capacity <= 2 * sys.maxsize + 1:
        raise ValueError("capacity must be nonnegative and fit in the platform usize range")
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="max_weight_with_capacity_plugin",
        args=[start, end, weight],
        kwargs={"capacity": str(capacity)},
        is_elementwise=False,
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
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="containment_count_plugin",
        args=[start, end],
        is_elementwise=False,
    )

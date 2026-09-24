"""Rust-backed interval expressions for Polars."""

from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

__all__ = ["overlap_count"]


def overlap_count(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Count other overlapping half-open intervals `[start, end)` per row.

    Two non-empty intervals overlap iff `a.start < b.end` and
    `b.start < a.end`. Touching intervals do not overlap; empty intervals
    (`start == end`) count zero. Each row excludes itself, while duplicate
    non-empty intervals count each other.

    Args:
        start: Column name or expression producing interval starts.
        end: Column name or expression producing interval ends. Both inputs
            must have equal lengths and the same dtype: `Int8`, `Int16`,
            `Int32`, `Int64`, `UInt8`, `UInt16`, `UInt32`, or `UInt64`.
            No casting or scalar broadcasting is performed.

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
        Counts use the whole input collection, or each group when used with
        `.over(...)` or `group_by`. The Rust algorithm takes O(n log n) time
        and O(n) additional space, without materializing overlapping pairs.

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

        Expressions in an eager `select`:

        >>> df.select(
        ...     pi.overlap_count(pl.col("start"), pl.col("end")).alias("overlaps")
        ... )["overlaps"].to_list()
        [1, 1, 2, 0]
    """
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="overlap_count_plugin",
        args=[start, end],
        is_elementwise=False,
    )

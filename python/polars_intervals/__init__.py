"""Rust-backed interval expressions for Polars."""

from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

__all__ = ["overlap_count"]


def overlap_count(start: str | pl.Expr, end: str | pl.Expr) -> pl.Expr:
    """Count other overlapping half-open intervals ``[start, end)`` per row.

    ``start`` and ``end`` are column names or expressions producing equal-length
    columns with the same integer dtype (signed or unsigned, 8-64 bits). Null
    endpoints and ``start > end`` are rejected when the expression is evaluated.
    Inputs are not cast or broadcast.

    Two non-empty intervals overlap iff ``a.start < b.end`` and
    ``b.start < a.end``. Touching intervals do not overlap; empty intervals
    (``start == end``) count zero. Self-overlap is excluded, while duplicate
    non-empty intervals count each other.

    Returns a ``UInt64`` expression preserving input row order. Counts use the
    whole input collection, or each group when used with ``over``/``group_by``.
    The Rust algorithm takes O(n log n) time and O(n) additional space.
    """
    return register_plugin_function(
        plugin_path=Path(__file__).parent,
        function_name="overlap_count_plugin",
        args=[start, end],
        is_elementwise=False,
    )

"""Private native-Polars event competitor with complete two-input validation."""

import polars as pl
from polars_intervals import _blocking_frame, _geometry_input


def native_set_geometry(
    left,
    right,
    *,
    intersection=False,
    left_start="start",
    left_end="end",
    right_start="start",
    right_end="end",
    by=None,
):
    """Boolean coverage from grouped signed event counts, never overlap pairs."""
    eager = isinstance(left, pl.DataFrame) and isinstance(right, pl.DataFrame)
    left, keys, schema = _geometry_input(left, left_start, left_end, by, "native_set_geometry")
    right, _, other = _geometry_input(
        right, right_start, right_end, list(keys), "native_set_geometry"
    )
    if schema != other:
        raise pl.exceptions.SchemaError("left and right endpoint and group dtypes must match")
    aliases = [f"_k{i}" for i in range(len(keys))] or ["_k0"]

    def branch(frame, start, end, side):
        return (
            frame.lazy()
            .select(
                pl.selectors.by_name(start).alias("_s"),
                pl.selectors.by_name(end).alias("_e"),
                *[pl.selectors.by_name(key).alias(alias) for key, alias in zip(keys, aliases)],
                *([] if keys else [pl.lit(0, dtype=pl.UInt8).alias("_k0")]),
            )
            .with_row_index("_row")
            .with_columns(pl.lit(side, dtype=pl.UInt8).alias("_side"))
        )

    combined = pl.concat(
        [branch(left, left_start, left_end, 0), branch(right, right_start, right_end, 1)],
        how="vertical",
    )

    def validate(frame):
        null = pl.col("_s").is_null() | pl.col("_e").is_null()
        invalid = (
            frame.filter(null | (pl.col("_s") > pl.col("_e")))
            .sort("_side", "_row")
            .select("_side", "_row", null.alias("null"))
            .head(1)
        )
        if invalid.height:
            side, row, is_null = invalid.row(0)
            reason = "null endpoints" if is_null else "start greater than end"
            raise pl.exceptions.ComputeError(
                f"{'right' if side else 'left'} interval at index {row}: {reason}"
            )
        return frame

    source = _blocking_frame(combined, validate, combined.collect_schema())
    groups = (
        source.group_by(aliases)
        .agg(pl.col("_row").filter(pl.col("_side") == 0).min().alias("_g"))
        .filter(pl.col("_g").is_not_null())
    )
    events = (
        source.filter(pl.col("_s") < pl.col("_e"))
        .unpivot(on=["_s", "_e"], index=[*aliases, "_side"], value_name="_x")
        .with_columns(
            pl.when(pl.col("variable") == "_s")
            .then(pl.lit(1, dtype=pl.Int64))
            .otherwise(pl.lit(-1, dtype=pl.Int64))
            .alias("_delta")
        )
        .group_by(*aliases, "_x")
        .agg(
            pl.col("_delta").filter(pl.col("_side") == 0).sum().alias("_left"),
            pl.col("_delta").filter(pl.col("_side") == 1).sum().alias("_right"),
        )
        .sort(*aliases, "_x")
        .with_columns(
            pl.col("_left").cum_sum().over(aliases),
            pl.col("_right").cum_sum().over(aliases),
            pl.col("_x").shift(-1).over(aliases).alias("_end"),
        )
    )
    occupied = (pl.col("_left") > 0) & (
        (pl.col("_right") > 0) if intersection else (pl.col("_right") == 0)
    )
    result = (
        events.filter(occupied & (pl.col("_x") < pl.col("_end")))
        .with_columns(
            (pl.col("_x") != pl.col("_end").shift().over(aliases)).fill_null(True).alias("_new")
        )
        .with_columns(pl.col("_new").cast(pl.UInt64).cum_sum().over(aliases).alias("_run"))
        .group_by(*aliases, "_run")
        .agg(pl.col("_x").min().alias("start"), pl.col("_end").max().alias("end"))
        .join(groups, on=aliases, nulls_equal=True)
        .sort("_g", "start")
        .select(*[pl.col(alias).alias(key) for alias, key in zip(aliases, keys)], "start", "end")
    )
    return result.collect() if eager else result

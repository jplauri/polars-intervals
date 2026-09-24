"""Test the installed Rust plugin from the repository root:

    python -m pip install .
    python -B -I -m unittest discover -s tests -v

Use a Python 3.12+ virtual environment with a Rust toolchain available.
"""

import unittest

import polars as pl
from polars.testing import assert_frame_equal, assert_series_equal

import polars_intervals as pi


class OverlapCountTests(unittest.TestCase):
    def test_interval_semantics_in_select_and_with_columns(self):
        for name, starts, ends, counts in [
            ("empty_input", [], [], []),
            ("single_interval", [1], [4], [0]),
            ("basic_overlaps", [5, 1, 3, 2], [7, 4, 6, 3], [1, 2, 2, 1]),
            ("disjoint", [8, 1, 4], [9, 2, 6], [0, 0, 0]),
            ("touching", [3, 1, 5], [5, 3, 7], [0, 0, 0]),
            ("nested", [2, 0, 1, 6], [3, 10, 5, 9], [2, 3, 2, 1]),
            ("duplicates", [1, 1, 1], [4, 4, 4], [2, 2, 2]),
            ("empty_intervals", [1, 1, 4], [1, 1, 4], [0, 0, 0]),
            (
                "empty_intervals_inside_and_at_boundaries",
                [2, 0, 4, 0, 1],
                [2, 4, 4, 0, 3],
                [0, 1, 0, 0, 1],
            ),
        ]:
            frame = pl.DataFrame(
                {"start": starts, "end": ends},
                schema={"start": pl.Int64, "end": pl.Int64},
            ).with_row_index("row")
            expected = frame.with_columns(pl.Series("count", counts, dtype=pl.UInt64))
            expression = pi.overlap_count("start", "end").alias("count")

            with self.subTest(case=name, context="select"):
                assert_frame_equal(
                    frame.select("row", expression), expected.select("row", "count")
                )
            with self.subTest(case=name, context="with_columns"):
                assert_frame_equal(frame.with_columns(expression), expected)

    def test_invalid_interval_reports_original_row_in_select_and_with_columns(self):
        frame = pl.DataFrame({"start": [2, 0, 5, 6], "end": [2, 1, 4, 8]})
        expression = pi.overlap_count("start", "end")

        with self.subTest(context="select"):
            with self.assertRaisesRegex(pl.exceptions.ComputeError, "index 2"):
                frame.select(expression)
        with self.subTest(context="with_columns"):
            with self.assertRaisesRegex(pl.exceptions.ComputeError, "index 2"):
                frame.with_columns(expression)

    def test_column_names_return_an_expression_with_half_open_counts(self):
        frame = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
        expression = pi.overlap_count("start", "end")
        self.assertIsInstance(expression, pl.Expr)
        result = frame.lazy().select(expression.alias("count")).collect()
        assert_series_equal(
            result["count"], pl.Series("count", [1, 1, 2, 0], dtype=pl.UInt64)
        )

    def test_expressions_and_mixed_arguments_preserve_row_order(self):
        frame = pl.DataFrame({"start": [4, 1, 3, 2, 2], "end": [8, 3, 5, 4, 2]})
        for start, end in [
            (pl.col("start"), "end"),
            ("start", pl.col("end")),
            (pl.col("start") + 10, pl.col("end") + 10),
        ]:
            with self.subTest(start=start, end=end):
                result = (
                    frame.lazy()
                    .with_columns(pi.overlap_count(start, end).alias("count"))
                    .collect()
                )
                self.assertEqual(result["start"].to_list(), [4, 1, 3, 2, 2])
                assert_series_equal(
                    result["count"],
                    pl.Series("count", [1, 1, 2, 2, 0], dtype=pl.UInt64),
                )

    def test_all_supported_integer_dtypes(self):
        for dtype in [
            pl.Int8,
            pl.Int16,
            pl.Int32,
            pl.Int64,
            pl.UInt8,
            pl.UInt16,
            pl.UInt32,
            pl.UInt64,
        ]:
            with self.subTest(dtype=dtype):
                frame = pl.DataFrame(
                    {"start": [1, 2, 2, 3, 6], "end": [9, 5, 5, 3, 7]},
                    schema={"start": dtype, "end": dtype},
                )
                result = (
                    frame.lazy()
                    .select(pi.overlap_count("start", "end").alias("count"))
                    .collect()
                )
                assert_series_equal(
                    result["count"],
                    pl.Series("count", [3, 2, 2, 0, 1], dtype=pl.UInt64),
                )

    def test_empty_single_and_touching_collections(self):
        for starts, ends, counts in [
            ([], [], []),
            ([1], [4], [0]),
            ([2], [2], [0]),
            ([1, 3, 5], [3, 5, 8], [0, 0, 0]),
        ]:
            with self.subTest(starts=starts, ends=ends):
                frame = pl.DataFrame(
                    {"start": starts, "end": ends},
                    schema={"start": pl.Int64, "end": pl.Int64},
                )
                query = frame.lazy().select(
                    pi.overlap_count("start", "end").alias("count")
                )
                self.assertEqual(query.collect_schema()["count"], pl.UInt64)
                assert_series_equal(
                    query.collect()["count"],
                    pl.Series("count", counts, dtype=pl.UInt64),
                )

    def test_counts_across_chunks_and_streaming_batches(self):
        frame = pl.concat(
            [pl.DataFrame({"start": [1, 1], "end": [5, 5]}) for _ in range(4)],
            rechunk=False,
        )
        self.assertGreater(frame["start"].n_chunks(), 1)
        query = frame.lazy().select(pi.overlap_count("start", "end").alias("count"))
        for engine in ["auto", "streaming"]:
            with self.subTest(engine=engine):
                assert_series_equal(
                    query.collect(engine=engine)["count"],
                    pl.Series("count", [7] * 8, dtype=pl.UInt64),
                )

    def test_filter_and_slice_after_counts_preserve_the_collection(self):
        frame = pl.DataFrame({"start": [1, 3, 2, 2], "end": [3, 5, 4, 2]})
        result = (
            frame.lazy()
            .with_columns(pi.overlap_count("start", "end").alias("count"))
            .filter(pl.col("start") < 3)
            .head(2)
            .collect()
        )
        self.assertEqual(result["count"].to_list(), [1, 2])

    def test_window_and_group_by_count_within_each_group(self):
        frame = pl.DataFrame(
            {
                "group": ["a", "b", "a", "b"],
                "start": [1, 1, 2, 5],
                "end": [4, 4, 3, 6],
            }
        )
        expression = pi.overlap_count("start", "end")
        window = frame.lazy().with_columns(expression.over("group").alias("count"))
        self.assertEqual(window.collect()["count"].to_list(), [1, 0, 1, 0])
        grouped = (
            frame.lazy()
            .group_by("group", maintain_order=True)
            .agg(expression.alias("count"))
            .collect()
        )
        self.assertEqual(grouped["count"].to_list(), [[1, 1], [0, 0]])

    def test_rejects_nulls_invalid_intervals_and_unsupported_dtypes(self):
        for starts, ends, message in [
            (pl.Series([1, None]), pl.Series([3, 4]), "null endpoints"),
            (pl.Series([1, 2]), pl.Series([3, None]), "null endpoints"),
            (
                pl.Series([None], dtype=pl.Int64),
                pl.Series([None], dtype=pl.Int64),
                "null endpoints",
            ),
            (pl.Series([4]), pl.Series([3]), "index 0"),
            (
                pl.Series([1], dtype=pl.Int32),
                pl.Series([3], dtype=pl.Int64),
                "matching integer dtypes",
            ),
            (pl.Series([1.0]), pl.Series([3.0]), "integer dtype"),
            (pl.Series(["a"]), pl.Series(["b"]), "integer dtype"),
        ]:
            with self.subTest(starts=starts, ends=ends):
                frame = pl.DataFrame({"start": starts, "end": ends})
                with self.assertRaisesRegex(pl.exceptions.PolarsError, message):
                    frame.lazy().select(pi.overlap_count("start", "end")).collect()

    def test_rejects_unequal_expression_lengths_without_broadcasting(self):
        frame = pl.DataFrame({"start": [1, 2], "end": [3, 4]})
        for start in [pl.col("start").head(1), pl.lit(1, dtype=pl.Int64)]:
            with self.subTest(start=start):
                with self.assertRaisesRegex(pl.exceptions.PolarsError, "equal lengths"):
                    frame.lazy().select(pi.overlap_count(start, "end")).collect()


if __name__ == "__main__":
    unittest.main()

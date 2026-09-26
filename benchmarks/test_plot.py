"""Check that documentation charts preserve samples, units, and missing cases."""

import json
import tempfile
import unittest
from pathlib import Path

import polars as pl
from plot import load_samples, render, summarize


class PlotTests(unittest.TestCase):
    def setUp(self):
        self.source = {"path": "run.csv", "dimensions": ["family"], "x": "n", "method": "method"}
        self.chart = {
            "id": "example",
            "title": "Example",
            "scope": "Rust core",
            "caption": "One run.",
            "ylabel": "Runtime (ms)",
            "filters": {"family": "dense"},
            "methods": {"A": "A", "B": "B"},
            "value": "ns",
            "divisor": 1_000_000,
        }
        self.samples = pl.LazyFrame(
            {
                "family": ["dense"] * 4 + ["sparse"],
                "n": [1000, 1000, 10000, 1000, 1000],
                "method": ["A", "A", "A", "B", "A"],
                "ns": [1_000_000, 3_000_000, 10_000_000, 4_000_000, 999_000_000],
                "sample": [0, 1, 0, 0, 0],
            }
        )

    def test_units_repeats_and_missing_cases(self):
        points = summarize(self.samples, self.source, self.chart)
        self.assertEqual(
            points.to_dicts(),
            [
                {"method": "A", "x": 1000, "median": 2.0, "min": 1.0, "max": 3.0, "samples": 2},
                {"method": "A", "x": 10000, "median": 10.0, "min": 10.0, "max": 10.0, "samples": 1},
                {"method": "B", "x": 1000, "median": 4.0, "min": 4.0, "max": 4.0, "samples": 1},
            ],
        )
        svg, markdown = render(points, self.source, self.chart)
        self.assertIn("<svg", svg)
        self.assertIn("| 1,000 | A | 2 | 1 | 3 | 2 |", markdown)
        self.assertNotIn("| 10,000 | B |", markdown)
        self.assertEqual(svg, render(points, self.source, self.chart)[0])

    def test_rejects_ambiguous_and_invalid_data(self):
        for chart in (
            {**self.chart, "filters": {}},
            {**self.chart, "filters": {"family": "absent"}},
            {**self.chart, "methods": {"unknown": "Missing"}},
        ):
            with self.subTest(chart=chart), self.assertRaises(ValueError):
                summarize(self.samples, self.source, chart)
        for samples in (
            pl.concat([self.samples, self.samples]),
            self.samples.with_columns(pl.lit(-1).alias("ns")),
            self.samples.with_columns(pl.lit(float("nan")).alias("ns")),
            self.samples.with_columns(pl.lit(None).alias("ns")),
            self.samples.with_columns(pl.lit(-1).alias("sample")),
        ):
            with self.subTest(samples=samples), self.assertRaises(ValueError):
                summarize(samples, self.source, self.chart)

    def test_csv_large_objectives_and_legacy_json(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "run.csv").write_text(
                "family,n,method,sample,ns,objective\ndense,1000,A,0,2000000,170141183460469231731687303715884105727\n",
                encoding="utf-8",
            )
            chart = {**self.chart, "methods": {"A": "A"}}
            csv_points = summarize(load_samples(self.source, root), self.source, chart)
            (root / "run.json").write_text(
                json.dumps(
                    {
                        "results": [
                            {
                                "family": "dense",
                                "rows": 1000,
                                "timings": {"A": {"samples_ms": [2.0], "median_ms": 999}},
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            source = {**self.source, "path": "run.json", "x": "rows"}
            json_points = summarize(
                load_samples(source, root), source, {**chart, "value": "ms", "divisor": 1}
            )
            self.assertEqual(csv_points.to_dicts(), json_points.to_dicts())


if __name__ == "__main__":
    unittest.main()

"""Check that documentation tables preserve samples, units, and missing cases."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import polars as pl
from plot import load_samples, main, render_table, summarize, summarize_table


class PlotTests(unittest.TestCase):
    def setUp(self):
        self.source = {"path": "run.csv", "dimensions": ["family"], "x": "n", "method": "method"}
        self.chart = {
            "id": "example",
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

            # JSON reports can name the size and method columns differently.
            report = json.loads((root / "run.json").read_text(encoding="utf-8"))
            report["results"][0]["n"] = report["results"][0].pop("rows")
            (root / "run.json").write_text(json.dumps(report), encoding="utf-8")
            source = {**source, "x": "n", "method": "algorithm"}
            self.assertEqual(
                summarize(
                    load_samples(source, root), source, {**chart, "value": "ms", "divisor": 1}
                ).to_dicts(),
                csv_points.to_dicts(),
            )

    def test_table_preserves_workloads_and_missing_combinations(self):
        table = {
            **self.chart,
            "methods": {"A": "Production", "B": "Baseline"},
            "cases": [
                {"label": "Dense", "filters": {"family": "dense"}, "sizes": [1000, 10000]},
                {
                    "label": "Sparse",
                    "filters": {"family": "sparse"},
                    "sizes": [1000],
                    "methods": {"A": "Production"},
                },
            ],
        }
        points = summarize_table(self.samples, self.source, table)
        markdown = render_table(points, self.source, table)
        self.assertIn("| Workload | Input rows | Production | Baseline |", markdown)
        self.assertIn("| Dense | 1,000 | 2 | 4 |", markdown)
        self.assertIn("| Dense | 10,000 | 10 | — |", markdown)
        self.assertIn("| Sparse | 1,000 | 999 | — |", markdown)
        self.assertIn("Runtime (ms)", markdown)
        self.assertEqual(points.height, 4)
        self.assertEqual(
            points.columns,
            ["case", "workload_family", "method", "x", "median", "min", "max", "samples"],
        )
        self.assertEqual(points.row(0, named=True)["min"], 1.0)
        self.assertEqual(points.row(0, named=True)["max"], 3.0)
        self.assertEqual(points.row(0, named=True)["samples"], 2)
        # Empty-input measurements outside the selected sizes are not table samples.
        with_empty_input = pl.concat(
            [
                self.samples,
                pl.LazyFrame(
                    {"family": ["dense"], "n": [0], "method": ["A"], "ns": [0], "sample": [0]}
                ),
            ]
        )
        self.assertEqual(
            summarize_table(with_empty_input, self.source, table).to_dicts(), points.to_dicts()
        )
        for case in (
            {**table["cases"][0], "filters": {}},
            {**table["cases"][0], "sizes": [123]},
            {**table["cases"][0], "sizes": [1000, 123]},
            {**table["cases"][0], "sizes": [1000, 1000]},
            {**table["cases"][0], "methods": {"unknown": "Unknown"}},
        ):
            with self.subTest(case=case), self.assertRaises(ValueError):
                summarize_table(self.samples, self.source, {**table, "cases": [case]})
        with self.assertRaisesRegex(ValueError, "Duplicate samples"):
            summarize_table(pl.concat([self.samples, self.samples]), self.source, table)
        complete = {**table, "cases": [{**table["cases"][0], "sizes": [1000]}]}
        self.assertNotIn("—", render_table(points, self.source, complete))

    def test_table_preserves_method_as_workload_dimension(self):
        samples = self.samples.with_columns(
            pl.col("method").alias("dtype"), pl.lit("minimum_cover").alias("method")
        )
        source = {**self.source, "method": "dtype", "dimensions": ["family", "method"]}
        table = {
            **self.chart,
            "cases": [
                {
                    "label": "Dense",
                    "filters": {"family": "dense", "method": "minimum_cover"},
                    "sizes": [1000, 10000],
                }
            ],
        }
        points = summarize_table(samples, source, table)
        self.assertEqual(points["method"].to_list(), ["A", "A", "B"])
        self.assertEqual(points["workload_method"].to_list(), ["minimum_cover"] * 3)
        self.assertEqual(points["workload_family"].to_list(), ["dense"] * 3)
        self.assertIn("| Dense | 1,000 | 2 | 4 |", render_table(points, source, table))

    def test_native_comparison_uses_fastest_available_median_per_size(self):
        samples = pl.concat(
            [
                self.samples,
                pl.LazyFrame(
                    {
                        "family": ["dense"] * 3,
                        "n": [1000, 10000, 100000],
                        "method": ["C", "C", "A"],
                        "ns": [8_000_000, 5_000_000, 20_000_000],
                        "sample": [0, 0, 0],
                    }
                ),
            ]
        )
        table = {
            **self.chart,
            "methods": {"A": "Package (ms)", "B": "Native B", "C": "Native C"},
            "native_comparison": {"package": "A", "baselines": ["B", "C"]},
            "cases": [
                {"label": "Dense", "sizes": [1000, 10000, 100000], "filters": {"family": "dense"}}
            ],
        }
        points = summarize_table(samples, self.source, table)
        markdown = render_table(points, self.source, table)
        self.assertIn("| Package (ms) | Native Polars (ms) | Compared with native |", markdown)
        self.assertIn("| Dense | 1,000 | 2 | 4 | 2× faster |", markdown)
        self.assertIn("| Dense | 10,000 | 10 | 5 | 2× slower |", markdown)
        self.assertIn("| Dense | 100,000 | 20 | — | — |", markdown)
        self.assertEqual(set(points["method"]), {"A", "B", "C"})
        inconclusive = {
            **table,
            "cases": [{**table["cases"][0], "inconclusive_sizes": [1000]}],
        }
        self.assertIn(
            "| Dense | 1,000 | 2 | 4 | about the same |",
            render_table(points, self.source, inconclusive),
        )
        zeros = render_table(points.with_columns(pl.lit(0.0).alias("median")), self.source, table)
        self.assertIn("| Dense | 1,000 | 0 | 0 | — |", zeros)
        with self.assertRaisesRegex(ValueError, "distinct measured baselines"):
            render_table(
                points,
                self.source,
                {**table, "native_comparison": {"package": "A", "baselines": ["A"]}},
            )

    def test_config_writes_table_outputs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            samples_path = root / "run.csv"
            self.samples.collect().write_csv(samples_path)
            config = root / "tables.toml"
            config.write_text(
                f'''[sources.example]
path = "{samples_path.as_posix()}"
dimensions = ["family"]
x = "n"
method = "method"
[[tables]]
id = "headline"
source = "example"
value = "ns"
divisor = 1000000
ylabel = "Runtime (ms)"
methods = {{ A = "Production", B = "Baseline" }}
cases = [{{ label = "Dense", sizes = [1000, 10000], filters = {{ family = "dense" }} }}]
''',
                encoding="utf-8",
            )
            output = root / "output"
            with patch("sys.argv", ["plot.py", "--config", str(config), "--output", str(output)]):
                main()
            self.assertEqual(
                {path.name for path in output.iterdir()}, {"headline.md", "headline.csv"}
            )
            saved = pl.read_csv(output / "headline.csv")
            self.assertEqual(saved.height, 3)
            self.assertEqual(saved.row(0, named=True)["median"], 2.0)


if __name__ == "__main__":
    unittest.main()

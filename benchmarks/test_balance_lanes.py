"""Offline tests of balance scoring, bounded oracles, corpora and report calculations."""

import csv
import io
import itertools
import tempfile
import unittest
from contextlib import redirect_stderr
from copy import deepcopy
from pathlib import Path

from balance_lanes import (
    aggregate_quality,
    build_frame,
    check_record,
    optimum_for,
    parse_args,
    quality_record,
    timing_summary,
    tiny_optimum,
    validate_coloring,
)
from balance_lanes_fixtures import iter_handcrafted, record
from generate_interval_graphs import (
    FAMILIES,
    Instance,
    generate_instance,
    iter_dataset,
    iter_jaist,
    iter_suite,
    write_dataset,
)

HERE = Path(__file__).resolve().parent


def brute_optimum(rows, k):
    """No production helpers: all k**n assignments for at most six vertices."""
    assert len(rows) <= 6
    scores = []
    for lanes in itertools.product(range(k), repeat=len(rows)):
        if set(lanes) != set(range(k)):
            continue
        if any(
            lanes[i] == lanes[j] and max(rows[i][0], rows[j][0]) < min(rows[i][1], rows[j][1])
            for i in range(len(rows))
            for j in range(i)
        ):
            continue
        sizes = [lanes.count(color) for color in range(k)]
        scores.append((max(sizes) - min(sizes), sum(size * size for size in sizes)))
    return min(scores)


def orientation_scores(rows, lanes, a, b):
    """Independent adjacency/BFS and 2**components pair-orientation enumeration."""
    selected = {i for i, color in enumerate(lanes) if color in (a, b)}
    components = []
    while selected:
        pending = [min(selected)]
        component = []
        while pending:
            i = pending.pop()
            if i not in selected:
                continue
            selected.remove(i)
            component.append(i)
            pending.extend(
                j for j in selected if max(rows[i][0], rows[j][0]) < min(rows[i][1], rows[j][1])
            )
        components.append(component)
    assert len(components) <= 12
    results = []
    for flips in itertools.product((False, True), repeat=len(components)):
        candidate = lanes.copy()
        for component, flip in zip(components, flips, strict=True):
            if flip:
                for i in component:
                    candidate[i] = a if lanes[i] == b else b
        results.append((abs(candidate.count(a) - candidate.count(b)), candidate))
    return components, results


class ValidationTests(unittest.TestCase):
    def test_empty_and_empty_only_conventions(self):
        empty = validate_coloring([], [])
        self.assertEqual((empty["k"], empty["D"], empty["Q"], empty["delta"]), (0, 0, 0, 0))
        rows = [(7, 7)] * 5
        score = validate_coloring(rows, [0] * 5)
        self.assertEqual((score["omega"], score["k"], score["Q"]), (0, 1, 25))
        with self.assertRaisesRegex(ValueError, "minimum palette"):
            validate_coloring(rows, [0, 1, 0, 0, 0])

    def test_half_open_and_interior_empty_rows(self):
        rows = [(0, 10), (10, 20), (5, 5), (5, 8)]
        score = validate_coloring(rows, [0, 0, 1, 1])
        self.assertEqual((score["D"], score["Q"], score["empty_count"]), (0, 8, 1))
        with self.assertRaisesRegex(ValueError, "overlapping"):
            validate_coloring(rows, [0, 0, 1, 0])

    def test_invalid_lanes_and_lengths(self):
        for lanes in ([True], [-1], [2**32], [1.0], [None], []):
            with self.subTest(lanes=lanes), self.assertRaises(ValueError):
                validate_coloring([(0, 1)], lanes)
        for lanes in ([1], [99999999]):
            with self.assertRaisesRegex(ValueError, "minimum palette"):
                validate_coloring([(0, 1)], lanes)
        with self.assertRaisesRegex(ValueError, "minimum palette"):
            validate_coloring([(0, 1), (1, 2)], [0, 1])
        with self.assertRaisesRegex(ValueError, "endpoints"):
            validate_coloring([(3, 2)], [0])

    def test_handcrafted_witnesses_and_pair_limit(self):
        fixtures = {item["params"]["name"]: item for item in iter_handcrafted([16])}
        for item in fixtures.values():
            check_record(item)
        simultaneous = fixtures["simultaneous_flip"]
        rows, lanes = simultaneous["intervals"], simultaneous["supplied_lanes"]
        self.assertEqual((lanes.count(0), lanes.count(1)), (19, 15))
        components, candidates = orientation_scores(rows, lanes, 0, 1)
        self.assertEqual(min(diff for diff, _ in candidates), 0)
        for component in components:
            difference = lanes.count(0) - lanes.count(1)
            difference -= 2 * sum(1 if lanes[i] == 0 else -1 for i in component)
            self.assertGreaterEqual(abs(difference), 4)
        limitation = fixtures["pairwise_limitation"]
        rows, lanes = limitation["intervals"], limitation["supplied_lanes"]
        self.assertEqual([lanes.count(c) for c in range(3)], [15, 16, 17])
        for a, b in itertools.combinations(range(3), 2):
            _, candidates = orientation_scores(rows, lanes, a, b)
            self.assertEqual(
                min(diff for diff, _ in candidates), abs(lanes.count(a) - lanes.count(b))
            )
        self.assertEqual(optimum_for(limitation, 3)["D"], 0)
        self.assertEqual(optimum_for(limitation, 3)["status"], "validated_witness")


class OracleTests(unittest.TestCase):
    def test_tiny_exhaustive_agrees_with_independent_product(self):
        for rows, k in [
            ([(0, 5), (1, 2), (2, 3), (3, 4)], 2),
            ([(0, 4), (0, 4), (1, 2), (2, 3), (2, 2)], 3),
            ([(0, 1), (1, 2), (2, 3)], 1),
            ([(3, 3)] * 4, 1),
            ([(0, 1)] * 4, 4),
        ]:
            with self.subTest(rows=rows):
                optimum = tiny_optimum(rows, k)
                self.assertEqual(optimum["status"], "exact")
                self.assertEqual((optimum["D"], optimum["Q"]), brute_optimum(rows, k))

    def test_caps_return_explicit_unknown(self):
        size_limited = tiny_optimum([(0, 1)] * 200, 200)
        self.assertEqual(size_limited["status"], "unknown_size_limit")
        self.assertIsNone(size_limited["D"])
        work_limited = tiny_optimum([(0, 5), (1, 2), (2, 3)], 2, max_nodes=1)
        self.assertEqual(work_limited["status"], "unknown_work_limit")
        self.assertIsNone(work_limited["Q"])
        with self.assertRaisesRegex(ValueError, "limits"):
            tiny_optimum([], 0, max_n=1000)
        self.assertEqual(tiny_optimum([], 0)["D"], 0)

    def test_certificate_is_scoring_only_and_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            instance = generate_instance("planted_balanced", {"n": 20, "k": 3}, seed=42)
            write_dataset([instance], directory)
            item = next(iter_dataset(directory))
            check_record(item)
            optimum = optimum_for(item, 3)
            self.assertEqual(
                (optimum["D"], optimum["Q"], optimum["status"]), (1, 134, "certificate")
            )
            self.assertNotIn("lanes", item["certificate"])
            malformed = deepcopy(item)
            malformed["certificate"]["chromatic_number"] = 4
            with self.assertRaises(ValueError):
                optimum_for(malformed, 3)
            item["certificate"] = {"kind": "unrecognized", "minimum_possible_class_size_spread": 0}
            self.assertEqual(optimum_for(item, 3)["status"], "unknown_size_limit")


class CorpusTests(unittest.TestCase):
    def test_every_smoke_family_round_trips_without_mutating_record(self):
        with tempfile.TemporaryDirectory() as directory:
            write_dataset(iter_suite("smoke", 42), directory, shard_size=3)
            records = list(iter_dataset(directory))
            self.assertEqual({item["family"] for item in records}, set(FAMILIES))
            self.assertEqual({item["params"]["order"] for item in records}, {"sorted", "shuffled"})
            for item in records:
                original = deepcopy(item)
                check_record(item)
                self.assertEqual(item, original)
            records[0]["stats"]["n"] += 1
            with self.assertRaisesRegex(ValueError, "statistics"):
                check_record(records[0])

    def test_offline_jaist_populations_stay_separate(self):
        for kind, filename, count in [
            ("all", "jaist_interval_list_tiny.txt", 7),
            ("connected", "jaist_interval_connected_tiny.txt", 4),
        ]:
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                source = HERE / "fixtures" / filename
                write_dataset(
                    iter_jaist([{"path": source}], kind),
                    directory,
                    kind="jaist",
                    suite=kind,
                    seed=None,
                )
                records = list(iter_dataset(directory))
                self.assertEqual(len(records), count)
                for item in records:
                    check_record(item)
                    self.assertEqual(item["provenance"]["catalog_kind"], kind)
                    self.assertEqual(len(item["provenance"]["source_sha256"]), 64)

    def test_external_adapter_uses_validated_existing_format(self):
        starts, ends = [0, 4, 2], [3, 4, 8]
        instance = Instance(
            "with_empties",
            starts,
            ends,
            None,
            {"n": 3, "empty_count": 1, "order": "external"},
            provenance={
                "source": "local export",
                "source_sha256": "example",
                "units": "microseconds",
            },
        )
        with tempfile.TemporaryDirectory() as directory:
            write_dataset([instance], directory, kind="external", suite="local", seed=None)
            item = next(iter_dataset(directory))
            check_record(item)
            self.assertEqual(item["provenance"], instance.provenance)
            self.assertEqual(item["intervals"], [[0, 3], [4, 4], [2, 8]])

    def test_polars_workload_geometry_and_chunk_boundaries(self):
        try:
            import polars as pl
        except ImportError:
            self.skipTest("Polars is absent; pure offline corpus/report tests remain available")
        item = record("workload", [(0, 3), (1, 1), (2, 5), (5, 6), (6, 7), (8, 9)])
        for workload in ("integer", "sliced", "multi_chunk", "grouped", "date", "datetime"):
            frame, groups = build_frame(item, workload)
            self.assertEqual(frame.height, 6 * groups)
            self.assertEqual(
                set(frame.columns), {"start", "end", "group"} if groups > 1 else {"start", "end"}
            )
            if workload == "multi_chunk":
                self.assertEqual(frame["start"].chunk_lengths(), [2, 4])
                self.assertEqual(frame["end"].chunk_lengths(), [3, 3])
            if workload == "datetime":
                self.assertEqual(frame["start"].dtype, pl.Datetime("us", "Europe/Helsinki"))


class ReportingTests(unittest.TestCase):
    def quality(self, *, budget=100, lanes=None, optimum=None):
        item = record("report", [(0, 5), (1, 2), (2, 3), (3, 4)])
        baseline = validate_coloring(item["intervals"], [0, 1, 1, 1])
        if optimum is None:
            optimum = {"D": None, "Q": None, "status": "unknown_size_limit"}
        return quality_record(
            item,
            lanes or [0, 1, 1, 1],
            baseline,
            baseline,
            optimum,
            id=item["id"],
            corpus="test",
            family="handcrafted",
            population="handcrafted",
            seed=0,
            order="fixture",
            workload="integer",
            engine="auto",
            groups=1,
            collection_n=4,
            method="repair",
            max_work=budget,
        )

    def test_budget_status_and_missing_optima_are_not_claims(self):
        zero = self.quality(budget=0)
        limited = self.quality()
        self.assertEqual(zero["stop_reason"], "zero_budget")
        self.assertEqual(limited["stop_reason"], "not_exposed")
        for row in (zero, limited):
            self.assertIsNone(row["D_star"])
            self.assertIsNone(row["additive_gap"])
            self.assertIsNone(row["skips"])
            self.assertIsNone(row["work_used"])
        summary = aggregate_quality([limited])[0]
        self.assertIsNone(summary["optimum_hit_rate"])
        self.assertEqual(summary["unknown_optimum_reasons"], {"unknown_size_limit": 1})

    def test_quality_is_deterministic_and_uses_correct_starting_baseline(self):
        self.assertEqual(self.quality(), self.quality())
        rows = [(0, 3), (1, 2), (3, 4), (4, 5)]
        item = record("baseline_alignment", rows)
        production = validate_coloring(rows, [0, 1, 0, 0])
        supplied = validate_coloring(rows, [0, 1, 1, 1])
        output = quality_record(
            item,
            [0, 1, 1, 0],
            supplied,
            production,
            {"D": 0, "Q": 8, "status": "exact"},
            method="repair_supplied",
            max_work=100,
        )
        self.assertEqual((output["starting_D"], output["improvement_D"], output["D"]), (2, 2, 0))
        self.assertEqual(output["stop_reason"], "equity")
        with self.assertRaisesRegex(ValueError, "regressed"):
            quality_record(
                item,
                [0, 1, 0, 0],
                validate_coloring(rows, [0, 1, 1, 0]),
                production,
                {"D": 0},
                method="repair",
                max_work=100,
            )

    def test_quality_aggregation_does_not_use_timing_samples(self):
        rows = [self.quality(optimum={"D": 2, "Q": 10, "status": "exact"}), self.quality()]
        rows[1]["id"] = "another-instance"
        summary = aggregate_quality(rows)[0]
        self.assertEqual(
            (summary["instances"], summary["known_optima"], summary["optimum_hit_rate"]), (2, 1, 1)
        )
        timing = []
        for sample in range(100):
            for method, work, total in [("baseline", "0", 10), ("repair", "100", 20)]:
                timing.append(
                    {
                        "id": "one",
                        "corpus": "test",
                        "workload": "integer",
                        "engine": "auto",
                        "method": method,
                        "max_work": work,
                        "sample": sample,
                        "total_ns": total,
                    }
                )
        timed = list(timing_summary(timing))
        self.assertEqual([row["runtime_ratio"] for row in timed], [1, 2])
        self.assertEqual([row["samples"] for row in timed], [100, 100])
        self.assertEqual(aggregate_quality(rows), [summary])

    def test_csv_roundtrip_unknown_fields_remain_missing(self):
        row = self.quality()
        output = io.StringIO(newline="")
        writer = csv.DictWriter(output, fieldnames=row)
        writer.writeheader()
        writer.writerow(row)
        output.seek(0)
        summary = aggregate_quality(csv.DictReader(output))[0]
        self.assertEqual(summary["known_optima"], 0)
        self.assertIsNone(summary["mean_additive_gap"])

    def test_overlapping_corpora_are_never_pooled(self):
        first = self.quality()
        second = {**first, "corpus": "separate-suite"}
        summaries = aggregate_quality([first, second])
        self.assertEqual([row["instances"] for row in summaries], [1, 1])
        self.assertEqual({row["corpus"] for row in summaries}, {"test", "separate-suite"})

    def test_timing_requires_actual_baseline(self):
        with self.assertRaisesRegex(ValueError, "matching production baseline"):
            list(
                timing_summary(
                    [
                        {
                            "corpus": "test",
                            "id": "one",
                            "workload": "integer",
                            "engine": "auto",
                            "method": "repair",
                            "max_work": "100",
                            "total_ns": 10,
                        }
                    ]
                )
            )

    def test_cli_caps_and_full_supported_budget(self):
        args = parse_args(
            ["--handcrafted", "--output", "target/result", "--max-work", str(2**64 - 1), "0", "0"]
        )
        self.assertEqual(args.max_work, [2**64 - 1, 0])
        repeated = parse_args(
            [
                "--dataset",
                "target/example",
                "--dataset",
                "target/../target/example",
                "--output",
                "target/result",
            ]
        )
        self.assertEqual(len(repeated.dataset), 1)
        for options in (
            ["--max-work", "-1"],
            ["--max-work", str(2**64)],
            ["--oracle-max-n", "13"],
            ["--repeats", "0"],
        ):
            with (
                self.subTest(options=options),
                redirect_stderr(io.StringIO()),
                self.assertRaises(SystemExit),
            ):
                parse_args(["--handcrafted", "--output", "target/result", *options])


if __name__ == "__main__":
    unittest.main()

"""Offline checks for interval corpus geometry, provenance, and reproducibility."""

import gzip
import io
import json
import os
import random
import subprocess
import sys
import tempfile
import unittest
import urllib.error
from collections import Counter
from dataclasses import replace
from itertools import pairwise
from pathlib import Path
from unittest.mock import patch

from generate_interval_graphs import (
    FAMILY_DEFAULTS,
    SMOKE_CASES,
    Instance,
    _validate_planted_lanes,
    derive_seed,
    generate_instance,
    iter_dataset,
    iter_suite,
    structural_stats,
    validate_instance,
    write_dataset,
)
from jaist_interval_catalog import (
    PAGE_URL,
    download_catalogs,
    iter_catalog,
    parse_catalog_links,
    parse_endpoint_sequence,
)

ROOT = Path(__file__).resolve().parent
SCRIPT = ROOT / "generate_interval_graphs.py"
FIXTURES = ROOT / "fixtures"
FAMILIES = {
    "random_endpoints",
    "start_duration",
    "fixed_duration",
    "heavy_tailed",
    "bursty",
    "quantized",
    "nested",
    "staircase",
    "multi_component",
    "planted_balanced",
    "star_components",
    "disjoint",
    "clique",
    "duplicates",
    "with_empties",
}


def overlaps(left: tuple[int, int], right: tuple[int, int]) -> bool:
    """An empty interval intersects nothing, including an interval containing its point."""
    return max(left[0], right[0]) < min(left[1], right[1])


def naive_stats(starts: list[int], ends: list[int]) -> dict[str, object]:
    """Quadratic graph construction and DFS, used only for tiny test instances."""
    intervals = list(zip(starts, ends, strict=True))
    neighbors = [set() for _ in intervals]
    edges = 0
    for i, left in enumerate(intervals):
        for j in range(i):
            if overlaps(left, intervals[j]):
                neighbors[i].add(j)
                neighbors[j].add(i)
                edges += 1
    seen = set()
    components = 0
    for vertex in range(len(intervals)):
        if vertex in seen:
            continue
        components += 1
        pending = [vertex]
        while pending:
            vertex = pending.pop()
            if vertex not in seen:
                seen.add(vertex)
                pending.extend(neighbors[vertex] - seen)
    # Maximum concurrency occurs at a start coordinate. Counting membership at
    # each coordinate is independent of the implementation's sorted event sweep.
    omega = max((sum(s <= point < e for s, e in intervals) for point in starts), default=0)
    n = len(intervals)
    nonempty = sum(s < e for s, e in intervals)
    endpoints = starts + ends
    return {
        "n": n,
        "nonempty_count": nonempty,
        "empty_count": n - nonempty,
        "edge_count": edges,
        "edge_density": edges / (n * (n - 1) // 2) if n > 1 else 0.0,
        "omega": omega,
        "component_count": components,
        "min_endpoint": min(endpoints, default=None),
        "max_endpoint": max(endpoints, default=None),
    }


def intervals(instance: Instance) -> list[tuple[int, int]]:
    return list(zip(instance.starts, instance.ends, strict=True))


def dataset_bytes(directory: Path) -> dict[str, bytes]:
    return {
        path.relative_to(directory).as_posix(): path.read_bytes()
        for path in sorted(directory.rglob("*"))
        if path.is_file()
    }


def run_cli(*args: str, hash_seed: str = "1") -> subprocess.CompletedProcess[str]:
    # -S disables site packages, demonstrating that no compiled extension or
    # third-party package is needed. Block networking inside the subprocess too.
    bootstrap = (
        "import runpy, socket, sys, urllib.request; "
        "deny = lambda *a, **k: (_ for _ in ()).throw(AssertionError('network forbidden')); "
        "socket.create_connection = urllib.request.urlopen = deny; "
        "sys.path.insert(0, sys.argv.pop(2)); sys.argv = sys.argv[1:]; "
        "runpy.run_path(sys.argv[0], run_name='__main__')"
    )
    return subprocess.run(
        [sys.executable, "-S", "-c", bootstrap, str(SCRIPT), str(ROOT), *args],
        env={**os.environ, "PYTHONHASHSEED": hash_seed},
        capture_output=True,
        text=True,
        check=False,
        timeout=60,
    )


class OfflineTests(unittest.TestCase):
    def setUp(self):
        for target in ("urllib.request.urlopen", "socket.create_connection"):
            blocker = patch(target, side_effect=AssertionError("tests must not access the network"))
            blocker.start()
            self.addCleanup(blocker.stop)


class StructuralStatsTests(OfflineTests):
    def test_half_open_ties_and_isolated_empties(self):
        for starts, ends in (
            ([], []),
            ([0, 1], [1, 2]),
            ([0, 0, 1], [1, 1, 2]),
            ([0, 1, 1, 2, -7], [3, 1, 2, 2, -7]),
            ([1, 1, 1], [1, 1, 1]),
        ):
            with self.subTest(starts=starts, ends=ends):
                self.assertEqual(structural_stats(starts, ends), naive_stats(starts, ends))
        self.assertFalse(overlaps((0, 1), (1, 2)))
        self.assertFalse(overlaps((1, 1), (0, 3)))
        self.assertEqual(structural_stats([0, 1], [1, 2])["edge_count"], 0)
        self.assertEqual(structural_stats([0, 1, 2], [3, 1, 2])["component_count"], 3)

    def test_sweeps_match_independent_tiny_graph_oracle(self):
        rng = random.Random(7419)
        for case in range(300):
            rows = [sorted((rng.randrange(-5, 9), rng.randrange(-5, 9))) for _ in range(case % 21)]
            starts = [row[0] for row in rows]
            ends = [row[1] for row in rows]
            with self.subTest(case=case):
                self.assertEqual(structural_stats(starts, ends), naive_stats(starts, ends))

    def test_invalid_geometry_and_declared_size_are_rejected(self):
        for instance in (
            Instance("disjoint", [0], [], 1, {"n": 1}),
            Instance("disjoint", [2], [1], 1, {"n": 1}),
            Instance("disjoint", [0], [1], 1, {"n": 2}),
            Instance("disjoint", [0], [0], 1, {"n": 1}),
            Instance("disjoint", [0.5], [1], 1, {"n": 1}),
        ):
            with self.subTest(instance=instance), self.assertRaises(ValueError):
                validate_instance(instance)


class FamilyTests(OfflineTests):
    def test_all_families_are_valid_deterministic_and_respect_n(self):
        self.assertEqual(set(FAMILY_DEFAULTS), FAMILIES)
        for family, params in FAMILY_DEFAULTS.items():
            with self.subTest(family=family):
                first = generate_instance(family, params, 923)
                second = generate_instance(family, dict(params), 923)
                self.assertEqual(first, second)
                self.assertEqual(first.id, second.id)
                self.assertEqual(len(first.starts), first.params["n"])
                self.assertEqual(intervals(first), sorted(intervals(first)))
                self.assertEqual(first.params["order"], "sorted")
                self.assertEqual(validate_instance(first), naive_stats(first.starts, first.ends))
                for start, end in intervals(first):
                    self.assertIs(type(start), int)
                    self.assertIs(type(end), int)
                    self.assertLessEqual(start, end)
                    if family != "with_empties":
                        self.assertLess(start, end)

    def test_fixed_seeds_change_randomized_geometry(self):
        for family in (
            "random_endpoints",
            "start_duration",
            "fixed_duration",
            "heavy_tailed",
            "bursty",
            "quantized",
            "multi_component",
            "planted_balanced",
        ):
            with self.subTest(family=family):
                left = generate_instance(family, FAMILY_DEFAULTS[family], 11)
                right = generate_instance(family, FAMILY_DEFAULTS[family], 29)
                self.assertNotEqual(intervals(left), intervals(right))

    def test_seeds_and_ids_use_canonical_identifying_data(self):
        self.assertEqual(derive_seed({"a": 2, "b": 3}), derive_seed({"b": 3, "a": 2}))
        params = {"n": 20, "horizon": 100}
        first = generate_instance("random_endpoints", params, 15)
        self.assertEqual(
            first.id,
            generate_instance("random_endpoints", dict(reversed(list(params.items()))), 15).id,
        )
        self.assertNotEqual(
            first.id, generate_instance("random_endpoints", params, 15, replicate=1).id
        )

    def test_order_changes_rows_without_changing_geometry(self):
        for family, params in FAMILY_DEFAULTS.items():
            with self.subTest(family=family):
                ordered = generate_instance(family, params, 712, order="sorted")
                shuffled = generate_instance(family, params, 712, order="shuffled")
                self.assertEqual(Counter(intervals(ordered)), Counter(intervals(shuffled)))
                self.assertNotEqual(ordered.id, shuffled.id)
                self.assertEqual(shuffled.params["order"], "shuffled")
                self.assertEqual(shuffled, generate_instance(family, params, 712, order="shuffled"))
        example = generate_instance("random_endpoints", {"n": 50, "horizon": 100}, 712)
        shuffled = generate_instance(
            "random_endpoints", {"n": 50, "horizon": 100}, 712, order="shuffled"
        )
        self.assertNotEqual(intervals(example), intervals(shuffled))

    def test_control_family_invariants(self):
        for n in (0, 1, 19):
            for family in ("disjoint", "clique", "duplicates"):
                with self.subTest(n=n, family=family):
                    instance = generate_instance(family, {"n": n}, 8)
                    stats = validate_instance(instance)
                    if family == "disjoint":
                        self.assertEqual(stats["edge_count"], 0)
                        self.assertLessEqual(stats["omega"], 1)
                        self.assertEqual(stats["component_count"], n)
                    else:
                        self.assertEqual(stats["edge_count"], n * (n - 1) // 2)
                        self.assertEqual(stats["omega"], n)
                    if family == "duplicates" and n:
                        self.assertEqual(len(set(intervals(instance))), 1)

    def test_fixed_duration_and_staircase(self):
        instance = generate_instance("fixed_duration", {"n": 27, "horizon": 90, "duration": 17}, 5)
        self.assertEqual([end - start for start, end in intervals(instance)], [17] * 27)
        instance = generate_instance("staircase", {"n": 18, "step": 3, "width": 10}, 5)
        self.assertEqual(intervals(instance), [(3 * i, 3 * i + 10) for i in range(18)])
        self.assertEqual(validate_instance(instance)["omega"], 4)

    def test_quantized_endpoints_and_explicit_empties(self):
        instance = generate_instance(
            "quantized",
            {"n": 45, "horizon": 100, "grid": 5, "min_duration": 5, "max_duration": 20},
            8,
        )
        self.assertTrue(all(value % 5 == 0 for value in instance.starts + instance.ends))
        instance = generate_instance("with_empties", {"n": 21, "empty_count": 9}, 8)
        stats = validate_instance(instance)
        self.assertEqual(stats["empty_count"], 9)
        self.assertEqual(stats, naive_stats(instance.starts, instance.ends))

    def test_nested_and_multicomponent_structures(self):
        nested = generate_instance("nested", {"n": 23, "components": 3}, 4)
        stats = validate_instance(nested)
        self.assertEqual(stats["component_count"], 3)
        for left, right in pairwise(intervals(nested)):
            if overlaps(left, right):
                self.assertLessEqual(left[0], right[0])
                self.assertGreaterEqual(left[1], right[1])
        instance = generate_instance("multi_component", {"n": 47, "component_count": 4}, 4)
        self.assertEqual(validate_instance(instance)["component_count"], 4)
        # Sharing an outer interval does not make two disjoint inner intervals a chain.
        invalid = Instance("nested", [0, 1, 3], [10, 2, 4], 4, {"n": 3, "components": 1})
        with self.assertRaises(ValueError):
            validate_instance(invalid)

    def test_planted_balanced_certificates_and_no_assignment_leak(self):
        for n, k in ((1, 1), (15, 1), (15, 15), (30, 5), (31, 5), (73, 8)):
            with self.subTest(n=n, k=k):
                instance = generate_instance("planted_balanced", {"n": n, "k": k}, 456)
                self.assertEqual(
                    instance.certificate,
                    {
                        "kind": "planted_balanced",
                        "chromatic_number": k,
                        "minimum_possible_class_size_spread": int(n % k != 0),
                    },
                )
                self.assertEqual(validate_instance(instance)["omega"], k)
                self.assertEqual(naive_stats(instance.starts, instance.ends)["omega"], k)
                self.assertNotIn("lanes", instance.params)
                self.assertNotIn("assignment", instance.params)
        instance = generate_instance("planted_balanced", {"n": 73, "k": 8}, 456)
        stats = validate_instance(instance)
        self.assertGreater(stats["edge_count"], 8 * 7 // 2)
        invalid = replace(instance, certificate={**instance.certificate, "chromatic_number": 9})
        with self.assertRaises(ValueError):
            validate_instance(invalid)

    def test_planted_construction_checks_reject_invalid_witnesses(self):
        _validate_planted_lanes([[(0, 3), (3, 7)], [(1, 4), (4, 6)]], 4, 2)
        for lanes, n, k in (
            ([[(0, 3), (2, 7)], [(1, 4), (4, 6)]], 4, 2),  # One lane overlaps itself.
            ([[(0, 1), (2, 3)], [(1, 2), (3, 4)]], 4, 2),  # Anchors merely touch.
            ([[(0, 3), (3, 4), (4, 5)], [(1, 4)]], 4, 2),  # Unbalanced lane sizes.
            ([[(0, 3)], []], 1, 2),
        ):
            with self.subTest(lanes=lanes), self.assertRaises(ValueError):
                _validate_planted_lanes(lanes, n, k)

    def test_star_components_exact_graph_structure(self):
        instance = generate_instance("star_components", {"star_count": 4, "leaf_count": 5}, 17)
        rows = intervals(instance)
        self.assertEqual(instance.params["leaf_counts"], [5] * 4)
        self.assertEqual(len(rows), 24)
        stats = validate_instance(instance)
        self.assertEqual(stats["edge_count"], 20)
        self.assertEqual(stats["omega"], 2)
        self.assertEqual(stats["component_count"], 4)
        # The long center can be identified from geometry; no latent labels are needed.
        for offset in range(0, len(rows), 6):
            component = rows[offset : offset + 6]
            center = max(component, key=lambda row: row[1] - row[0])
            leaves = [row for row in component if row != center]
            self.assertEqual(len(leaves), 5)
            self.assertTrue(all(overlaps(center, leaf) for leaf in leaves))
            self.assertTrue(all(not overlaps(a, b) for a, b in pairwise(leaves)))
            outside = rows[:offset] + rows[offset + 6 :]
            self.assertFalse(any(overlaps(row, other) for row in component for other in outside))

    def test_star_leaf_ranges_and_zero_leaf_components(self):
        params = {"star_count": 8, "min_leaves": 0, "max_leaves": 9}
        instance = generate_instance("star_components", params, 16)
        self.assertEqual(instance, generate_instance("star_components", params, 16))
        leaves = instance.params["leaf_counts"]
        self.assertEqual(len(leaves), 8)
        self.assertTrue(all(0 <= count <= 9 for count in leaves))
        self.assertGreater(len(set(leaves)), 1)
        self.assertEqual(len(instance.starts), 8 + sum(leaves))
        self.assertEqual(validate_instance(instance), naive_stats(instance.starts, instance.ends))
        isolated = generate_instance("star_components", {"star_count": 3, "leaf_count": 0}, 16)
        self.assertEqual(validate_instance(isolated)["component_count"], 3)
        self.assertEqual(validate_instance(isolated)["edge_count"], 0)

    def test_invalid_family_parameters(self):
        cases = (
            ("unknown", {}),
            ("random_endpoints", {"n": -1}),
            ("random_endpoints", {"n": 3, "horizon": 0}),
            ("start_duration", {"min_duration": 8, "max_duration": 2}),
            ("fixed_duration", {"duration": 0}),
            ("quantized", {"grid": 0}),
            ("bursty", {"burst_count": 0}),
            ("staircase", {"width": 0}),
            ("nested", {"components": 0}),
            ("planted_balanced", {"n": 4, "k": 0}),
            ("planted_balanced", {"n": 4, "k": 5}),
            ("with_empties", {"n": 4, "empty_count": 5}),
            ("star_components", {"star_count": -1}),
        )
        for family, params in cases:
            with self.subTest(family=family, params=params), self.assertRaises(ValueError):
                generate_instance(family, params, 1)
        with self.assertRaises(ValueError):
            generate_instance("disjoint", {"n": 3}, 1, order="unknown")

    def test_smoke_suite_covers_families_with_unique_ids(self):
        instances = list(iter_suite("smoke", 42))
        self.assertEqual({instance.family for instance in instances}, FAMILIES)
        self.assertEqual(len({instance.id for instance in instances}), len(instances))
        self.assertEqual(instances, list(iter_suite("smoke", 42)))
        self.assertEqual(
            {instance.params["order"] for instance in instances}, {"sorted", "shuffled"}
        )
        pairs = {}
        for instance in instances:
            key = instance.family, instance.seed
            if key in pairs:
                self.assertEqual(Counter(intervals(instance)), Counter(intervals(pairs[key])))
            else:
                pairs[key] = instance
        self.assertLess(len(pairs), len(instances))

    def test_suite_seeds_survive_reordering_and_unrelated_cases(self):
        original = {instance.id: instance for instance in iter_suite("smoke", 59)}
        cases = list(reversed(SMOKE_CASES)) + [("fixed_duration", {"n": 17, "duration": 31})]
        with patch.dict("generate_interval_graphs.SUITES", {"smoke": (cases, 1)}):
            extended = {instance.id: instance for instance in iter_suite("smoke", 59)}
        self.assertEqual(len(extended), len(original) + 2)
        for identity, instance in original.items():
            self.assertIn(identity, extended)
            self.assertEqual(instance, extended[identity])


class DatasetTests(OfflineTests):
    def test_streaming_roundtrip_shards_and_deterministic_gzip(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = list(iter_suite("smoke", 42))
            manifest = write_dataset(iter(source), root / "one", shard_size=3)
            write_dataset(iter(source), root / "two", shard_size=3)
            self.assertEqual(dataset_bytes(root / "one"), dataset_bytes(root / "two"))
            self.assertEqual(manifest["schema_version"], 1)
            self.assertEqual(manifest["instance_count"], len(source))
            self.assertEqual(
                sum(shard["instance_count"] for shard in manifest["shards"]), len(source)
            )
            records = list(iter_dataset(root / "one"))
            self.assertEqual([record["id"] for record in records], [item.id for item in source])
            for record, instance in zip(records, source, strict=True):
                self.assertEqual(record["intervals"], [list(row) for row in intervals(instance)])
                self.assertEqual(record["stats"], validate_instance(instance))
                self.assertEqual(record["seed"], instance.seed)
                self.assertEqual(record["params"], instance.params)
                self.assertEqual(record["certificate"], instance.certificate)
                self.assertEqual(record["provenance"], instance.provenance)
                self.assertEqual(record["n"], len(instance.starts))
                self.assertNotIn("lanes", record)
                self.assertNotIn("assignment", record)
            for shard in manifest["shards"]:
                path = root / "one" / shard["path"]
                data = path.read_bytes()
                self.assertEqual(data[:2], b"\x1f\x8b")
                self.assertEqual(data[4:8], b"\x00\x00\x00\x00")
                self.assertFalse(data[3] & 8, "gzip filename header must be absent")
                lines = gzip.decompress(data).decode("utf-8").splitlines()
                self.assertEqual(len(lines), shard["instance_count"])
                self.assertLessEqual(len(lines), 3)
                for line in lines:
                    self.assertEqual(
                        line, json.dumps(json.loads(line), sort_keys=True, separators=(",", ":"))
                    )

    def test_output_rejection_and_explicit_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "dataset"
            output.mkdir()
            (output / "existing.txt").write_text("keep until overwrite", encoding="utf-8")
            with self.assertRaises((ValueError, FileExistsError)):
                write_dataset(iter_suite(), output)
            self.assertEqual(
                (output / "existing.txt").read_text(encoding="utf-8"), "keep until overwrite"
            )
            write_dataset(iter_suite(), output, overwrite=True)
            self.assertFalse((output / "existing.txt").exists())
            self.assertGreater(len(list(iter_dataset(output))), 0)

    def test_duplicate_ids_are_rejected(self):
        instance = generate_instance("disjoint", {"n": 5}, 1)
        with tempfile.TemporaryDirectory() as directory, self.assertRaises(ValueError):
            write_dataset(iter((instance, instance)), Path(directory) / "dataset")

    def test_writer_and_reader_are_lazy_between_shards(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "dataset"

            def source():
                for index in range(4):
                    if index >= 2:
                        self.assertTrue(any((output / "shards").glob("*.jsonl.gz")))
                    yield generate_instance("disjoint", {"n": index + 1}, index)

            manifest = write_dataset(source(), output, shard_size=1)
            second = output / manifest["shards"][1]["path"]
            second.write_bytes(b"invalid gzip")
            reader = iter_dataset(output)
            self.assertEqual(next(reader)["n"], 1)
            with self.assertRaises((OSError, ValueError, EOFError)):
                next(reader)

    def test_empty_dataset_and_invalid_shard_size(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "empty"
            manifest = write_dataset(iter(()), output)
            self.assertEqual(manifest["instance_count"], 0)
            self.assertEqual(manifest["shards"], [])
            self.assertEqual(list(iter_dataset(output)), [])
            for shard_size in (0, -1):
                with self.subTest(shard_size=shard_size), self.assertRaises(ValueError):
                    write_dataset(
                        iter(()), Path(directory) / str(shard_size), shard_size=shard_size
                    )

    def test_complete_smoke_cli_is_reproducible_across_hash_seeds(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, seed in (("first", "3"), ("second", "789")):
                result = run_cli(
                    "synthetic",
                    "--suite",
                    "smoke",
                    "--seed",
                    "42",
                    "--shard-size",
                    "7",
                    "--output",
                    str(root / name),
                    hash_seed=seed,
                )
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(dataset_bytes(root / "first"), dataset_bytes(root / "second"))
            manifest = json.loads((root / "first" / "dataset.json").read_text(encoding="utf-8"))
            records = list(iter_dataset(root / "first"))
            self.assertEqual(manifest["instance_count"], len(records))
            self.assertEqual({record["family"] for record in records}, FAMILIES)
            self.assertEqual(manifest["seed"], 42)
            rerun = run_cli("synthetic", "--output", str(root / "first"))
            self.assertNotEqual(rerun.returncode, 0)
            overwrite = run_cli("synthetic", "--output", str(root / "first"), "--overwrite")
            self.assertEqual(overwrite.returncode, 0, overwrite.stderr)

    def test_list_families_cli(self):
        result = run_cli("list-families")
        self.assertEqual(result.returncode, 0, result.stderr)
        for family in FAMILIES:
            self.assertIn(family, result.stdout)


class JaistTests(OfflineTests):
    def test_documented_endpoint_sequences(self):
        for encoding, edges, omega, components in (
            ("1 1 2 2 3 3", 0, 1, 3),
            ("1 2 1 3 2 3", 2, 2, 1),
            ("1 2 3 3 2 1", 3, 3, 1),
        ):
            with self.subTest(encoding=encoding):
                starts, ends = parse_endpoint_sequence(encoding, expected_n=3)
                stats = structural_stats(starts, ends)
                self.assertEqual(stats["edge_count"], edges)
                self.assertEqual(stats["omega"], omega)
                self.assertEqual(stats["component_count"], components)
                self.assertEqual(stats, naive_stats(starts, ends))
                self.assertTrue(all(start < end for start, end in zip(starts, ends, strict=True)))
        self.assertEqual(parse_endpoint_sequence("1 2 1 3 2 3"), ([0, 1, 3], [3, 5, 6]))
        # Labels, rather than first-appearance order, define normalized rows.
        self.assertEqual(parse_endpoint_sequence("3 1 3 2 1 2"), ([1, 3, 0], [5, 6, 3]))

    def test_malformed_sequences_are_rejected(self):
        for encoding, expected_n in (
            ("1 1 2", None),  # Odd endpoint count.
            ("1 1 2 3", None),  # Labels appearing once.
            ("1 1 1 2 2 2", None),  # Labels appearing three times.
            ("1 1 2 2", 3),
            ("1 1 nonsense nonsense", None),
        ):
            with self.subTest(encoding=encoding), self.assertRaises(ValueError):
                parse_endpoint_sequence(encoding, expected_n=expected_n)

    def test_local_plain_and_combined_fixtures(self):
        raw = list(iter_catalog(FIXTURES / "jaist_interval_disconnected_3.txt", expected_n=3))
        self.assertEqual(len(raw), 4)
        self.assertEqual([item[2]["catalog_index"] for item in raw], list(range(4)))
        self.assertEqual([item[2]["catalog_n"] for item in raw], [3] * 4)
        self.assertTrue(all(item[2]["source"] == "jaist_interval_catalog" for item in raw))
        combined = list(iter_catalog(FIXTURES / "jaist_interval_list_tiny.txt"))
        self.assertEqual(len(combined), 7)
        self.assertEqual(Counter(item[2]["catalog_n"] for item in combined), {1: 1, 2: 2, 3: 4})
        for starts, ends, provenance in raw + combined:
            self.assertEqual(structural_stats(starts, ends), naive_stats(starts, ends))
            self.assertIn("source_sha256", provenance)
            self.assertNotIn(str(FIXTURES), json.dumps(provenance))

    def test_catalog_footer_and_expected_counts_are_checked(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.txt"
            for content in (
                "size : 1\n1 1\n# of size 1 : 2\n",
                "size : 2\n1 1\n# of size 2 : 1\n",
                "arbitrary header\n1 1\n",
            ):
                path.write_text(content, encoding="utf-8")
                with self.subTest(content=content), self.assertRaises(ValueError):
                    list(iter_catalog(path))
            path.write_text("1 1\n", encoding="utf-8")
            with self.assertRaises(ValueError):
                list(iter_catalog(path, expected_count=2))
            with self.assertRaises(ValueError):
                list(iter_catalog(path, expected_n=2))

    def test_catalog_records_are_streamed(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "catalog.txt"
            path.write_text("1 1\ninvalid\n", encoding="utf-8")
            records = iter_catalog(path)
            self.assertEqual(next(records)[:2], ([0], [2]))
            with self.assertRaises(ValueError):
                next(records)

    def test_gzip_catalog_input_and_explicit_catalog_kind(self):
        fixture = FIXTURES / "jaist_interval_disconnected_3.txt"
        with tempfile.TemporaryDirectory() as directory:
            compressed = Path(directory) / "interval_disconnected_3.txt.gz"
            compressed.write_bytes(gzip.compress(fixture.read_bytes(), mtime=0))
            raw = list(iter_catalog(fixture))
            decoded = list(iter_catalog(compressed))
            self.assertEqual([record[:2] for record in raw], [record[:2] for record in decoded])
            with self.assertRaises(ValueError):
                list(iter_catalog(compressed, catalog_kind="connected"))

    def test_known_filenames_cannot_mislabel_catalog_kind(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for filename in ("interval_connected_3.txt", "interval-1-9-connected.txt"):
                path = root / filename
                path.write_bytes((FIXTURES / "jaist_interval_list_tiny.txt").read_bytes())
                with self.subTest(filename=filename), self.assertRaises(ValueError):
                    list(iter_catalog(path, catalog_kind="all"))

    def test_catalog_link_discovery_is_offline_and_preserves_selection(self):
        html = (
            '<a href="interval_disconnected_3.txt">4</a>'
            '<a href="interval_connected_3.txt">2</a>'
            '<a href="interval_disconnected_12.txt">938394</a>'
            '<a href="unrelated.txt">999</a>'
        )
        links = parse_catalog_links(html)
        self.assertEqual(set(links), {("all", 3), ("connected", 3), ("all", 12)})
        self.assertEqual(links["all", 3]["count"], 4)
        self.assertEqual(links["connected", 3]["count"], 2)
        self.assertEqual(links["all", 12]["url"], PAGE_URL + "interval_disconnected_12.txt")
        for changed in (
            '<a href="renamed_3.txt">4</a>',
            '<a href="interval_disconnected_3.txt">unknown count</a>',
        ):
            with self.subTest(html=changed), self.assertRaises(ValueError):
                parse_catalog_links(changed)

    def test_offline_jaist_import_cli(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name in ("first", "second"):
                result = run_cli(
                    "jaist",
                    "--input",
                    str(FIXTURES / "jaist_interval_disconnected_3.txt"),
                    "--catalog-kind",
                    "all",
                    "--output",
                    str(root / name),
                    "--shard-size",
                    "2",
                )
                self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual(dataset_bytes(root / "first"), dataset_bytes(root / "second"))
            records = list(iter_dataset(root / "first"))
            self.assertEqual(len(records), 4)
            self.assertEqual(len({record["id"] for record in records}), 4)
            for index, record in enumerate(records):
                self.assertIsNone(record["seed"])
                self.assertEqual(record["provenance"]["catalog_index"], index)
                self.assertEqual(record["provenance"]["catalog_n"], 3)

    def test_failed_imports_never_publish_a_manifest(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bad_footer = root / "bad-footer.txt"
            bad_footer.write_text("size : 1\n1 1\n# of size 1 : 2\n", encoding="utf-8")
            fixture = str(FIXTURES / "jaist_interval_disconnected_3.txt")
            for name, inputs, kind in (
                ("footer", [str(bad_footer)], "all"),
                ("duplicate", [fixture, fixture], "all"),
                ("connected", [fixture], "connected"),
            ):
                with self.subTest(case=name):
                    output = root / name
                    result = run_cli(
                        "jaist",
                        "--input",
                        *inputs,
                        "--catalog-kind",
                        kind,
                        "--output",
                        str(output),
                        "--shard-size",
                        "1",
                    )
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse((output / "dataset.json").exists())

    def test_explicit_downloader_with_mocked_network_and_cache(self):
        html = b'<a href="interval_disconnected_1.txt">1</a>'
        with tempfile.TemporaryDirectory() as directory:
            cache = Path(directory) / "cache"
            with patch(
                "urllib.request.urlopen", side_effect=[io.BytesIO(html), io.BytesIO(b"1 1\n")]
            ) as network:
                sources = download_catalogs([1], cache)
            self.assertEqual(network.call_count, 2)
            self.assertEqual(sources[0]["path"].read_bytes(), b"1 1\n")
            self.assertEqual(sources[0]["count"], 1)
            with patch("urllib.request.urlopen", side_effect=[io.BytesIO(html)]) as network:
                self.assertEqual(download_catalogs([1, 1], cache), sources)
            self.assertEqual(network.call_count, 1)
            failed_cache = Path(directory) / "failed"
            with (
                patch(
                    "urllib.request.urlopen",
                    side_effect=[io.BytesIO(html), urllib.error.URLError("offline test failure")],
                ),
                self.assertRaisesRegex(ValueError, "could not download"),
            ):
                download_catalogs([1], failed_cache)
            self.assertEqual(list(failed_cache.iterdir()), [])
            with (
                patch("urllib.request.urlopen", side_effect=urllib.error.URLError("offline")),
                self.assertRaisesRegex(ValueError, "could not read"),
            ):
                download_catalogs([1], cache)


if __name__ == "__main__":
    unittest.main()

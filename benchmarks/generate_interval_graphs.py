"""Offline interval-graph corpora; standard library only, no optimization algorithms."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import random
import shutil
from collections.abc import Iterable, Iterator
from dataclasses import dataclass
from itertools import chain, islice, pairwise
from pathlib import Path

from jaist_interval_catalog import download_catalogs, iter_catalog


def canonical_json(value: object) -> str:
    return json.dumps(
        value, sort_keys=True, separators=(",", ":"), ensure_ascii=True, allow_nan=False
    )


def derive_seed(value: object) -> int:
    """Stable, independent seeds: never depend on process hash or iteration position."""
    return int.from_bytes(hashlib.sha256(canonical_json(value).encode("ascii")).digest()[:16])


@dataclass
class Instance:
    family: str
    starts: list[int]
    ends: list[int]
    seed: int | None
    params: dict[str, object]
    certificate: dict[str, object] | None = None
    provenance: dict[str, object] | None = None

    @property
    def id(self) -> str:
        # Geometry is included so manually constructed instances also have honest identities.
        identity = {
            "family": self.family,
            "seed": self.seed,
            "params": self.params,
            "provenance": self.provenance,
            "certificate": self.certificate,
            "starts": self.starts,
            "ends": self.ends,
        }
        return hashlib.sha256(canonical_json(identity).encode("ascii")).hexdigest()


def integer(value: object, name: str, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum:
        raise ValueError(f"{name} must be an integer >= {minimum}")
    return value


def structural_stats(starts: list[int], ends: list[int]) -> dict[str, object]:
    """O(n log n) time / O(n) space, with no adjacency representation.

    Remove every end <= the next start before adding that start (END before START).
    Empty intervals never enter the sweep and each contributes one isolated component.
    """
    if len(starts) != len(ends):
        raise ValueError("starts and ends must have equal lengths")
    rows = []
    for start, end in zip(starts, ends, strict=True):
        if type(start) is not int or type(end) is not int or start > end:
            raise ValueError("interval endpoints must be integers satisfying start <= end")
        if start < end:
            rows.append((start, end))
    rows.sort()
    sorted_ends = sorted(end for _, end in rows)
    ended = edges = omega = components = 0
    furthest = None
    for started, (start, end) in enumerate(rows):
        while ended < len(sorted_ends) and sorted_ends[ended] <= start:
            ended += 1
        active = started - ended
        edges += active
        omega = max(omega, active + 1)
        if furthest is None or start >= furthest:
            components += 1
        furthest = end if furthest is None else max(furthest, end)
    n = len(starts)
    empty = n - len(rows)
    return {
        "n": n,
        "nonempty_count": len(rows),
        "empty_count": empty,
        "edge_count": edges,
        "edge_density": edges / (n * (n - 1) // 2) if n > 1 else 0.0,
        "omega": omega,
        "component_count": components + empty,
        "min_endpoint": min(starts) if n else None,
        "max_endpoint": max(ends) if n else None,
    }


def _durations(params: dict) -> tuple[int, int, int]:
    horizon = integer(params["horizon"], "horizon", 1)
    low = integer(params["min_duration"], "min_duration", 1)
    high = integer(params["max_duration"], "max_duration", low)
    return horizon, low, high


def _random_endpoints(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon = integer(p["horizon"], "horizon", 1)
    # No tie within a row; independent rows can share endpoints.
    return [tuple(sorted(rng.sample(range(horizon + 1), 2))) for _ in range(p["n"])]


def _start_duration(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon, low, high = _durations(p)
    # Horizon bounds starts only: preserve the independent positive duration at the right edge.
    return [(s := rng.randrange(horizon), s + rng.randint(low, high)) for _ in range(p["n"])]


def _fixed_duration(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon = integer(p["horizon"], "horizon", 1)
    duration = integer(p["duration"], "duration", 1)
    return [(s := rng.randrange(horizon), s + duration) for _ in range(p["n"])]


def _heavy_tailed(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon = integer(p["horizon"], "horizon", 1)
    scales, weights = p["duration_scales"], p["weights"]
    if not isinstance(scales, (list, tuple)) or not isinstance(weights, (list, tuple)):
        raise TypeError("duration_scales and weights must be sequences of positive integers")
    if not scales or len(scales) != len(weights):
        raise ValueError("duration_scales and weights must have equal nonzero lengths")
    for duration, weight in zip(scales, weights, strict=True):
        integer(duration, "duration scale", 1)
        integer(weight, "duration weight", 1)
    return [
        (s := rng.randrange(horizon), s + duration)
        for duration in rng.choices(scales, weights=weights, k=p["n"])
    ]


def _bursty(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon, low, high = _durations(p)
    count = integer(p["burst_count"], "burst_count", 1)
    radius = integer(p["burst_radius"], "burst_radius")
    centers = [rng.randrange(horizon) for _ in range(count)]
    p["burst_centers"] = centers
    rows = []
    for _ in range(p["n"]):
        start = min(horizon - 1, max(0, rng.choice(centers) + rng.randint(-radius, radius)))
        rows.append((start, start + rng.randint(low, high)))
    return rows


def _quantized(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    horizon, low, high = _durations(p)
    grid = integer(p["grid"], "grid", 1)
    low_units, high_units = (low + grid - 1) // grid, high // grid
    if low_units > high_units:
        raise ValueError("duration range contains no positive multiple of grid")
    return [
        (
            s := rng.randrange((horizon - 1) // grid + 1) * grid,
            s + grid * rng.randint(low_units, high_units),
        )
        for _ in range(p["n"])
    ]


def _nested(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    count = integer(p["components"], "components", 1)
    if count > p["n"]:
        raise ValueError("components must be <= n")
    rows, offset = [], 0
    for i in range(count):
        size = p["n"] // count + (i < p["n"] % count)
        rows.extend((offset + j, offset + 2 * size - j) for j in range(size))
        offset += 2 * size + 3
    return rows


def _staircase(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    step = integer(p["step"], "step", 1)
    width = integer(p["width"], "width", 1)
    return [(i * step, i * step + width) for i in range(p["n"])]


def _multi_component(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    count = integer(p["component_count"], "component_count", 1)
    if count > p["n"]:
        raise ValueError("component_count must be <= n")
    cuts = [0, *sorted(rng.sample(range(1, p["n"]), count - 1)), p["n"]]
    sizes = [right - left for left, right in pairwise(cuts)]
    p["component_sizes"] = sizes
    rows, offset = [], 0
    # Each local construction is connected, including size-one components.
    for i, size in enumerate(sizes):
        scale = rng.randint(1, 5)
        if i % 3 == 0:
            local = [(j, 2 * size - j) for j in range(size)]
        elif i % 3 == 1:
            width = rng.randint(3, 9)
            local = [(2 * j, 2 * j + width) for j in range(size)]
        else:
            local = [(0, 2 * size)] + [(2 * j + 1, 2 * j + 2) for j in range(size - 1)]
        rows.extend((offset + s * scale, offset + e * scale) for s, e in local)
        offset += max(e for _, e in local) * scale + rng.randint(2, 10)
    return rows


def _validate_planted_lanes(lanes: list[list[tuple[int, int]]], n: int, k: int) -> None:
    if len(lanes) != k or any(not lane for lane in lanes):
        raise ValueError("planted construction needs k nonempty lanes")
    sizes = [len(lane) for lane in lanes]
    if sum(sizes) != n or max(sizes) - min(sizes) > 1:
        raise ValueError("planted lane sizes must sum to n and differ by at most one")
    for lane in lanes:
        ordered = sorted(lane)
        if any(s >= e for s, e in ordered):
            raise ValueError("planted intervals must be nonempty")
        if any(left[1] > right[0] for left, right in pairwise(ordered)):
            raise ValueError("planted lane intervals overlap")
    if max(lane[0][0] for lane in lanes) >= min(lane[0][1] for lane in lanes):
        raise ValueError("planted anchors must have a common nonempty intersection")


def _planted_balanced(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    n, k = p["n"], integer(p["k"], "k", 1)
    if k > n:
        raise ValueError("k must be <= n")
    lanes = []
    for i in range(k):
        size = n // k + (i < n % k)
        lane = [(rng.randint(0, 8), rng.randint(12, 20))]
        for _ in range(size - 1):
            start = lane[-1][1] + rng.randint(0, 4)
            lane.append((start, start + rng.randint(6, 16)))
        lanes.append(lane)
    # Independent lane check gives a proper k-partition with balanced sizes. Anchors
    # share [8,12), proving a k-clique; together these certify chi = omega = k.
    # Subsequent intervals interact across lanes, rather than being isolated vertices.
    _validate_planted_lanes(lanes, n, k)
    return [row for lane in lanes for row in lane]


def _star_components(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    count = integer(p["star_count"], "star_count", 1)
    if p["min_leaves"] is not None or p["max_leaves"] is not None:
        low = integer(p["min_leaves"], "min_leaves")
        high = integer(p["max_leaves"], "max_leaves", low)
        leaves = [rng.randint(low, high) for _ in range(count)]
        p["leaf_count"] = None
    else:
        leaves = [integer(p["leaf_count"], "leaf_count")] * count
    p["leaf_counts"] = leaves
    p["n"] = count + sum(leaves)
    rows, offset = [], 0
    for count in leaves:
        # Center strictly contains disjoint unit leaves; the next center is beyond its end.
        rows.append((offset, offset + 2 * count + 1))
        rows.extend((offset + 2 * j + 1, offset + 2 * j + 2) for j in range(count))
        offset += 2 * count + 4
    return rows


def _validate_stars(rows: list[tuple[int, int]], leaf_counts: list[int]) -> None:
    ordered = sorted(rows)
    cursor, previous_end = 0, None
    for count in leaf_counts:
        integer(count, "leaf count")
        star = ordered[cursor : cursor + count + 1]
        if len(star) != count + 1:
            raise ValueError("star size differs from leaf_counts")
        center = star[0]
        if previous_end is not None and center[0] < previous_end:
            raise ValueError("stars overlap one another")
        leaf_end = center[0]
        for start, end in star[1:]:
            if not center[0] <= start < end <= center[1] or start < leaf_end:
                raise ValueError("star leaves must be independent and contained in their center")
            leaf_end = end
        previous_end = center[1]
        cursor += count + 1
    if cursor != len(rows):
        raise ValueError("star leaf_counts do not cover all intervals")


def _disjoint(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    width = integer(p["duration"], "duration", 1)
    step = width + integer(p["gap"], "gap")
    return [(i * step, i * step + width) for i in range(p["n"])]


def _clique(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    n = p["n"]
    return [(rng.randint(0, n), rng.randint(n + 1, 2 * n + 2)) for _ in range(n)]


def _duplicates(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    duration = integer(p["duration"], "duration", 1)
    return [(0, duration)] * p["n"]


def _with_empties(rng: random.Random, p: dict) -> list[tuple[int, int]]:
    empty = integer(p["empty_count"], "empty_count")
    if empty > p["n"]:
        raise ValueError("empty_count must be <= n")
    rows = _start_duration(rng, {**p, "n": p["n"] - empty})
    rows.extend((s := rng.randrange(p["horizon"]), s) for _ in range(empty))
    return rows


FAMILIES = {
    "random_endpoints": _random_endpoints,
    "start_duration": _start_duration,
    "fixed_duration": _fixed_duration,
    "heavy_tailed": _heavy_tailed,
    "bursty": _bursty,
    "quantized": _quantized,
    "nested": _nested,
    "staircase": _staircase,
    "multi_component": _multi_component,
    "planted_balanced": _planted_balanced,
    "star_components": _star_components,
    "disjoint": _disjoint,
    "clique": _clique,
    "duplicates": _duplicates,
    "with_empties": _with_empties,
}

FAMILY_DEFAULTS = {
    "random_endpoints": {"n": 40, "horizon": 200},
    "start_duration": {"n": 40, "horizon": 200, "min_duration": 1, "max_duration": 20},
    "fixed_duration": {"n": 40, "horizon": 200, "duration": 12},
    "heavy_tailed": {
        "n": 40,
        "horizon": 200,
        "duration_scales": [1, 4, 16, 64],
        "weights": [75, 18, 6, 1],
    },
    "bursty": {
        "n": 40,
        "horizon": 200,
        "burst_count": 4,
        "burst_radius": 5,
        "min_duration": 1,
        "max_duration": 20,
    },
    "quantized": {"n": 40, "horizon": 200, "grid": 5, "min_duration": 5, "max_duration": 25},
    "nested": {"n": 40, "components": 3},
    "staircase": {"n": 40, "step": 3, "width": 13},
    "multi_component": {"n": 40, "component_count": 4},
    "planted_balanced": {"n": 40, "k": 6},
    "star_components": {"star_count": 4, "leaf_count": 8, "min_leaves": None, "max_leaves": None},
    "disjoint": {"n": 40, "duration": 2, "gap": 1},
    "clique": {"n": 40},
    "duplicates": {"n": 40, "duration": 10},
    "with_empties": {
        "n": 40,
        "empty_count": 10,
        "horizon": 200,
        "min_duration": 1,
        "max_duration": 20,
    },
}


def generate_instance(
    family: str, params: dict, seed: int, *, order: str = "sorted", replicate: int = 0
) -> Instance:
    if family not in FAMILIES:
        raise ValueError(f"unknown family: {family}")
    if order not in ("sorted", "shuffled"):
        raise ValueError("order must be sorted or shuffled")
    integer(seed, "seed")
    integer(replicate, "replicate")
    unknown = params.keys() - FAMILY_DEFAULTS[family].keys()
    if unknown:
        raise ValueError(f"unknown {family} parameters: {sorted(unknown)}")
    p = {**FAMILY_DEFAULTS[family], **params}
    if family != "star_components":
        integer(p["n"], "n")
    rows = FAMILIES[family](random.Random(seed), p)
    if order == "sorted":
        # Python's stable sort preserves original_index for identical endpoints.
        rows.sort()
    else:
        random.Random(derive_seed({"seed": seed, "purpose": "row_order"})).shuffle(rows)
    certificate = None
    if family == "planted_balanced":
        certificate = {
            "kind": "planted_balanced",
            "chromatic_number": p["k"],
            "minimum_possible_class_size_spread": int(p["n"] % p["k"] != 0),
        }
    instance = Instance(
        family,
        [s for s, _ in rows],
        [e for _, e in rows],
        seed,
        {**p, "order": order, "replicate": replicate},
        certificate,
    )
    validate_instance(instance)
    return instance


def validate_instance(instance: Instance) -> dict[str, object]:
    stats = structural_stats(instance.starts, instance.ends)
    n = integer(instance.params.get("n"), "n")
    if stats["n"] != n:
        raise ValueError("actual interval count differs from requested n")
    family, p = instance.family, instance.params
    if family not in (*FAMILIES, "jaist"):
        raise ValueError(f"unknown family: {family}")
    if stats["empty_count"] and family != "with_empties":
        raise ValueError(f"{family} must not contain empty intervals")
    rows = list(zip(instance.starts, instance.ends, strict=True))
    if p.get("order") == "sorted" and rows != sorted(rows):
        raise ValueError("sorted row order does not match geometry")
    if family == "disjoint" and (stats["edge_count"] != 0 or stats["omega"] > 1):
        raise ValueError("disjoint intervals overlap")
    if family in ("clique", "duplicates") and (
        stats["edge_count"] != n * (n - 1) // 2 or stats["omega"] != n
    ):
        raise ValueError("clique does not have all possible edges")
    if family == "duplicates" and len(set(rows)) > 1:
        raise ValueError("duplicate intervals differ")
    if family == "fixed_duration" and any(e - s != p["duration"] for s, e in rows):
        raise ValueError("fixed interval duration differs from requested duration")
    if family == "with_empties" and stats["empty_count"] != p["empty_count"]:
        raise ValueError("empty_count differs from requested count")
    if family == "multi_component" and stats["component_count"] != p["component_count"]:
        raise ValueError("multi_component count differs from requested component_count")
    if family == "nested":
        if stats["component_count"] != p["components"]:
            raise ValueError("nested chain count differs from requested components")
        component_end = inner_end = None
        for start, end in sorted(rows):
            if component_end is None or start >= component_end:
                component_end = inner_end = end
            elif end >= inner_end:
                raise ValueError("nested chain does not have strict containment")
            else:
                inner_end = end
    if family == "quantized" and any(s % p["grid"] or e % p["grid"] for s, e in rows):
        raise ValueError("quantized endpoint is off grid")
    if family == "star_components":
        _validate_stars(rows, p["leaf_counts"])
        if (
            stats["edge_count"] != sum(p["leaf_counts"])
            or stats["component_count"] != p["star_count"]
        ):
            raise ValueError("star structure disagrees with sweep")
    if family == "planted_balanced":
        k = integer(p["k"], "k", 1)
        expected = {
            "kind": "planted_balanced",
            "chromatic_number": k,
            "minimum_possible_class_size_spread": int(n % k != 0),
        }
        if instance.certificate != expected or stats["omega"] != k:
            raise ValueError("planted certificate disagrees with structural validation")
    if family == "jaist":
        provenance = instance.provenance or {}
        if provenance.get("catalog_n") != n or instance.seed is not None:
            raise ValueError("JAIST provenance or seed disagrees with instance")
        if provenance.get("catalog_kind") == "connected" and stats["component_count"] != 1:
            raise ValueError("connected JAIST catalog contains a disconnected graph")
    return stats


# Workload lists are intentionally ordinary data. Geometry seeds use the complete
# resolved parameters, not a workload's list position or its row order.
SMOKE_CASES = [(family, {}) for family in FAMILIES]
EXACT_CASES = (
    [
        (family, {"n": n})
        for n in (20, 50, 100, 200)
        for family in FAMILIES
        if family != "star_components"
    ]
    + [
        ("start_duration", {"n": n, "horizon": n * 5, "min_duration": 1, "max_duration": duration})
        for n in (20, 50, 100, 200)
        for duration in (2, n * 4)
    ]
    + [("planted_balanced", {"n": n, "k": k}) for n in (20, 50, 100, 200) for k in (3, 10)]
    + [
        ("star_components", {"star_count": count, "min_leaves": 3, "max_leaves": 12})
        for count in (4, 8, 12)
    ]
)
SCALING_CASES = [
    (family, {"n": n, **({"horizon": n * 2} if "horizon" in FAMILY_DEFAULTS[family] else {})})
    for n in (1_000, 10_000, 100_000)
    for family in (
        "random_endpoints",
        "start_duration",
        "fixed_duration",
        "heavy_tailed",
        "bursty",
        "quantized",
        "nested",
        "staircase",
        "multi_component",
    )
] + [("planted_balanced", {"n": n, "k": k}) for n in (1_000, 10_000, 100_000) for k in (8, 64)]
SUITES = {
    "smoke": (SMOKE_CASES, 1),
    "exact": (EXACT_CASES, 3),
    "scaling": (SCALING_CASES, 2),
}


def iter_suite(suite: str = "smoke", seed: int = 42) -> Iterator[Instance]:
    if suite not in SUITES:
        raise ValueError(f"unknown suite: {suite}")
    integer(seed, "suite seed")
    cases, replicates = SUITES[suite]
    for family, overrides in cases:
        params = {**FAMILY_DEFAULTS[family], **overrides}
        for replicate in range(replicates):
            instance_seed = derive_seed(
                {
                    "suite_seed": seed,
                    "family": family,
                    "params": params,
                    "replicate": replicate,
                }
            )
            for order in ("sorted", "shuffled"):
                yield generate_instance(
                    family, overrides, instance_seed, order=order, replicate=replicate
                )


def _prepare_output(output: Path, overwrite: bool) -> None:
    resolved = output.resolve()
    # Only the explicitly named output tree can be replaced. Never follow a junction,
    # symlink, or remove the working directory / any of its ancestors.
    if output.is_symlink() or output.is_junction() or Path.cwd().resolve().is_relative_to(resolved):
        raise ValueError(
            "output must be a dedicated dataset directory, not a link or workspace ancestor"
        )
    if output.exists():
        if not output.is_dir():
            raise ValueError("output exists and is not a directory")
        if any(output.iterdir()):
            if not overwrite:
                raise ValueError("output directory is nonempty; use --overwrite to replace it")
            shutil.rmtree(resolved)
    (output / "shards").mkdir(parents=True, exist_ok=True)


def write_dataset(
    instances: Iterable[Instance],
    output: Path | str,
    *,
    kind: str = "synthetic",
    suite: str = "smoke",
    seed: int | None = 42,
    shard_size: int = 50_000,
    overwrite: bool = False,
) -> dict[str, object]:
    """Stream one record at a time; publish the manifest only after full validation.

    Synthetic suites have only hundreds of IDs. Catalog uniqueness instead follows
    from strictly increasing source indices and SHA-256 identities, requiring one
    high-water mark per source rather than a set with millions of entries.
    """
    integer(shard_size, "shard_size", 1)
    output = Path(output)
    _prepare_output(output, overwrite)
    manifest = {
        "schema_version": 1,
        "kind": kind,
        "suite": suite,
        "seed": seed,
        "instance_count": 0,
        "shard_size": shard_size,
        "shards": [],
    }
    seen_synthetic: set[str] = set()
    catalog_positions: dict[str, int] = {}
    remaining = iter(instances)
    while (first := next(remaining, None)) is not None:
        relative = f"shards/part-{len(manifest['shards']):05d}.jsonl.gz"
        count = 0
        # GzipFile rather than gzip.open: erase both timestamp AND original filename.
        with (
            (output / relative).open("wb") as raw,
            gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0, compresslevel=6) as out,
        ):
            for instance in chain((first,), islice(remaining, shard_size - 1)):
                stats = validate_instance(instance)
                identity = instance.id
                if instance.family == "jaist":
                    provenance = instance.provenance or {}
                    index = integer(provenance.get("catalog_index"), "catalog_index")
                    source_key = canonical_json(
                        {
                            k: v
                            for k, v in provenance.items()
                            if k not in ("catalog_index", "catalog_n")
                        }
                    )
                    if index != catalog_positions.get(source_key, -1) + 1:
                        raise ValueError("duplicate or nonsequential JAIST catalog index")
                    catalog_positions[source_key] = index
                else:
                    if identity in seen_synthetic:
                        raise ValueError(f"duplicate instance ID: {identity}")
                    seen_synthetic.add(identity)
                record = {
                    "id": identity,
                    "family": instance.family,
                    "seed": instance.seed,
                    "n": stats["n"],
                    "params": instance.params,
                    "stats": stats,
                    "certificate": instance.certificate,
                    "provenance": instance.provenance,
                    "intervals": list(zip(instance.starts, instance.ends, strict=True)),
                }
                out.write((canonical_json(record) + "\n").encode("ascii"))
                count += 1
        manifest["shards"].append({"path": relative, "instance_count": count})
        manifest["instance_count"] += count
    (output / "dataset.json").write_bytes((canonical_json(manifest) + "\n").encode("ascii"))
    return manifest


def iter_dataset(path: Path | str) -> Iterator[dict[str, object]]:
    """Read one complete record at a time, checking shard and dataset counts."""
    path = Path(path)
    manifest = json.loads((path / "dataset.json").read_text(encoding="ascii"))
    if manifest.get("schema_version") != 1:
        raise ValueError("unsupported dataset schema_version")
    total = 0
    for shard in manifest["shards"]:
        shard_path = (path / shard["path"]).resolve()
        if not shard_path.is_relative_to(path.resolve()):
            raise ValueError("shard path escapes dataset directory")
        count = 0
        with gzip.open(shard_path, "rt", encoding="ascii") as lines:
            for line in lines:
                record = json.loads(line)
                if not isinstance(record, dict):
                    raise TypeError("dataset record must be a JSON object")
                count += 1
                yield record
        if count != shard["instance_count"]:
            raise ValueError("shard instance_count differs from manifest")
        total += count
    if total != manifest["instance_count"]:
        raise ValueError("dataset instance_count differs from manifest")


def iter_jaist(sources: list[dict], catalog_kind: str) -> Iterator[Instance]:
    for source in sources:
        for starts, ends, provenance in iter_catalog(
            source["path"],
            expected_n=source.get("n"),
            catalog_kind=catalog_kind,
            source_url=source.get("url"),
            expected_count=source.get("count"),
        ):
            yield Instance(
                "jaist",
                starts,
                ends,
                None,
                {"n": len(starts), "order": "catalog_label"},
                provenance=provenance,
            )


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    commands.add_parser("list-families", help="list available synthetic families and defaults")
    synthetic = commands.add_parser("synthetic", help="generate a deterministic synthetic suite")
    synthetic.add_argument("--suite", choices=SUITES, default="smoke")
    synthetic.add_argument("--seed", type=int, default=42)
    jaist = commands.add_parser(
        "jaist", help="import local JAIST files or explicitly download them"
    )
    source = jaist.add_mutually_exclusive_group(required=True)
    source.add_argument("--input", type=Path, nargs="+")
    source.add_argument("--download", action="store_true")
    jaist.add_argument(
        "--n", type=int, nargs="+", help="orders to download; local files infer order"
    )
    jaist.add_argument("--catalog-kind", choices=("all", "connected"), required=True)
    jaist.add_argument("--cache", type=Path, default=Path("benchmarks/generated/jaist-cache"))
    for command in (synthetic, jaist):
        command.add_argument("--output", type=Path, required=True)
        command.add_argument("--shard-size", type=int, default=50_000)
        command.add_argument("--overwrite", action="store_true")
    args = parser.parse_args(argv)
    try:
        if args.command == "list-families":
            for family, defaults in FAMILY_DEFAULTS.items():
                print(f"{family}: {canonical_json(defaults)}")
            return
        if args.command == "synthetic":
            instances = iter_suite(args.suite, args.seed)
            suite, seed = args.suite, args.seed
        else:
            if args.download:
                if not args.n:
                    raise ValueError("--download requires --n")
                if args.cache.resolve().is_relative_to(args.output.resolve()):
                    raise ValueError("download cache must be outside the output dataset directory")
                sources = download_catalogs(args.n, args.cache, args.catalog_kind)
            else:
                if args.n:
                    raise ValueError(
                        "--n is only for downloads; local files declare or infer order"
                    )
                sources = [{"path": path} for path in args.input]
                if any(path.resolve().is_relative_to(args.output.resolve()) for path in args.input):
                    raise ValueError("input sources must be outside the output dataset directory")
            instances = iter_jaist(sources, args.catalog_kind)
            suite, seed = args.catalog_kind, None
        manifest = write_dataset(
            instances,
            args.output,
            kind=args.command,
            suite=suite,
            seed=seed,
            shard_size=args.shard_size,
            overwrite=args.overwrite,
        )
    except (ValueError, TypeError, OSError, EOFError, UnicodeError) as error:
        parser.error(str(error))
    print(f"Wrote {manifest['instance_count']} instances in {len(manifest['shards'])} shards")


if __name__ == "__main__":
    main()

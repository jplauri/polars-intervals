"""Run complete core geometry comparisons and preserve raw/source provenance."""

import argparse
import json
import os
import subprocess
from pathlib import Path

from benchmarks.provenance import (
    ROOT,
    archive_sources,
    build_environment,
    command,
    environment,
    sha256,
    source_changes,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", default="0,1,8,1000,10000,100000,1000000")
    parser.add_argument("--seeds", default="7,41")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--dtypes", default="i64,u64,i16")
    parser.add_argument("--cases", default="")
    parser.add_argument("--operations", default="cluster,merge,gaps")
    args = parser.parse_args()
    try:
        sizes = [int(value) for value in args.sizes.split(",")]
        seeds = [int(value) for value in args.seeds.split(",")]
    except ValueError:
        parser.error("Sizes and seeds must be comma-separated integers")
    if min(sizes) < 0 or args.samples < 1 or args.warmups < 1:
        parser.error("Need nonnegative sizes and positive samples/warmups")
    if any(seed < 0 or seed > 2**64 - 1 for seed in seeds):
        parser.error("Seeds must fit UInt64")
    if not set(args.dtypes.split(",")) <= {"i64", "u64", "i16"}:
        parser.error("Endpoint dtypes must be i64, u64 or i16")
    if not set(args.operations.split(",")) <= {"cluster", "merge", "gaps"}:
        parser.error("Operations must be cluster, merge or gaps")
    raw = args.output.resolve().with_suffix(".csv")
    meta, archive = raw.with_suffix(".metadata.json"), raw.with_suffix(".sources.zip")
    if any(path.exists() for path in (raw, meta, archive)):
        parser.error("Use a new output prefix")
    raw.parent.mkdir(parents=True, exist_ok=True)
    paths = [
        *ROOT.glob("crates/intervals-core/**/*.rs"),
        ROOT / "crates/intervals-core/Cargo.toml",
        ROOT / "Cargo.toml",
        ROOT / "Cargo.lock",
        ROOT / "rust-toolchain.toml",
        ROOT / "benchmarks/interval_geometry_core.py",
        ROOT / "benchmarks/provenance.py",
    ]
    settings = {
        "GEOMETRY_CSV": str(raw),
        "GEOMETRY_SIZES": args.sizes,
        "GEOMETRY_SEEDS": args.seeds,
        "GEOMETRY_SAMPLES": str(args.samples),
        "GEOMETRY_WARMUPS": str(args.warmups),
        "GEOMETRY_DTYPES": args.dtypes,
        "GEOMETRY_CASES": args.cases,
        "GEOMETRY_OPERATIONS": args.operations,
    }
    invocation = ["cargo", "bench", "-p", "intervals-core", "--bench", "geometry", "--locked"]
    metadata = environment() | {
        "settings": settings,
        "cargo_command": invocation,
        "cargo_version": command("cargo", "--version"),
        "build_environment": build_environment(),
        "scope": "Complete core call including validation, preparation, sortedness check when selected, clipping, sorting, scan, canonical cluster remapping, output allocation and destruction. No Python crossing or Polars. Compilation, fixtures, correctness and memory instrumentation excluded.",
        "memory": "Separate untimed call: peak requested live heap and allocation count, including algorithm buffers, output and reallocations. Excludes caller inputs, verification, stack, allocator overhead and RSS.",
        "verification": "Private candidates checked against independent original pair-relation graph and direct-membership elementary-cell oracles in geometry candidate tests, plus bitmap checks in core properties. Large timing instances compare full canonical output with production, not an independent large oracle.",
        "methods": {
            "production": "Actual public implementation: scan original endpoint buffers after verifying relevant start order, otherwise sort packed records. Canonical row-aligned IDs and fused bounded gaps.",
            "packed_sort": "Packed records, unconditional sorting, direct frontier scan.",
            "packed_sorted": "Private packed records with verified start order, isolating the sortedness check and fused gap emission.",
            "indices_sorted": "Indexed candidate with verified start-sorted fast path.",
            "materialized": "Packed records with sortedness detection, materialized union then complement.",
        },
        "limitations": "Single-machine synthetic core measurements. Not grouped/temporal/chunked Polars timings. Narrow i16 cases are omitted when any endpoint or domain bound is outside its range. See raw dimensions and separate complete-call report.",
        "source_sha256": {str(path.relative_to(ROOT)): sha256(path) for path in paths},
        "status": "running",
    }
    metadata.update(archive_sources(archive, paths))
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    completed = subprocess.run(invocation, cwd=ROOT, env={**os.environ, **settings}, check=False)
    metadata["returncode"] = completed.returncode
    if completed.returncode == 0 and (
        not raw.exists() or len(raw.read_text(encoding="utf-8").splitlines()) < 2
    ):
        metadata["returncode"] = 1
        metadata["error"] = "No samples emitted; check selected cases and sizes"
    metadata.update(source_changes(metadata["source_sha256"]))
    metadata["status"] = "complete" if metadata["returncode"] == 0 else "failed"
    if raw.exists():
        metadata["raw_sha256"] = sha256(raw)
    meta.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(metadata["returncode"])


if __name__ == "__main__":
    main()

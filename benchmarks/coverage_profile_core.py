"""Run complete Rust coverage-profile comparisons and preserve source evidence.

Run sequentially on an idle machine. This does not build or measure the Python
extension. Sources are archived because a base revision alone cannot reproduce
uncommitted candidate experiments.
"""

import argparse
import json
import os
import platform
import subprocess
import zipfile
from pathlib import Path

from provenance import ROOT, command, environment, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", default="0,1,8,1000,10000,100000,1000000")
    parser.add_argument("--seeds", default="7,41")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--dtypes", default="i64,u64")
    parser.add_argument("--weight-dtypes", default="i64")
    parser.add_argument("--cases", default="")
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
    if not set(args.dtypes.split(",")) <= {"i64", "u64"}:
        parser.error("Endpoint dtypes must be i64 or u64")
    if not set(args.weight_dtypes.split(",")) <= {"i64", "i128"}:
        parser.error("Weight dtypes must be i64 or i128")
    output = args.output.resolve()
    metadata_path = output.with_suffix(".metadata.json")
    archive_path = output.with_suffix(".sources.zip")
    for path in (output, metadata_path, archive_path):
        if path.exists():
            parser.error(f"refusing to overwrite historical evidence: {path}")
    output.parent.mkdir(parents=True, exist_ok=True)
    paths = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates/intervals-core/Cargo.toml",
        "crates/intervals-core/src/lib.rs",
        "crates/intervals-core/src/coverage_profile.rs",
        "crates/intervals-core/benches/coverage_profile.rs",
        "crates/intervals-core/benches/support/coverage_profile_candidates.rs",
        "crates/intervals-core/benches/support/allocations.rs",
        "crates/intervals-core/benches/support/random.rs",
        "crates/intervals-core/tests/coverage_profile_candidates.rs",
        "benchmarks/coverage_profile_core.py",
        "benchmarks/provenance.py",
    ]
    settings = {
        "PROFILE_CSV": str(output),
        "PROFILE_SIZES": args.sizes,
        "PROFILE_SEEDS": args.seeds,
        "PROFILE_SAMPLES": str(args.samples),
        "PROFILE_WARMUPS": str(args.warmups),
        "PROFILE_DTYPES": args.dtypes,
        "PROFILE_WEIGHT_DTYPES": args.weight_dtypes,
        "PROFILE_CASES": args.cases,
    }
    invocation = [
        "cargo",
        "bench",
        "-p",
        "intervals-core",
        "--bench",
        "coverage_profile",
        "--locked",
    ]
    metadata = {
        **environment(),
        "cargo_version": command("cargo", "--version"),
        "build_environment": {
            name: os.environ.get(name)
            for name in (
                "RUSTFLAGS",
                "CARGO_ENCODED_RUSTFLAGS",
                "CARGO_PROFILE_RELEASE_OPT_LEVEL",
                "CARGO_PROFILE_RELEASE_LTO",
                "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
            )
        },
        "cargo_command": invocation,
        "settings": settings,
        "scope": (
            "Complete core call: validation, domain inference/clipping, sortedness checks, "
            "preparation, sweep, coalescing, output allocation, output destruction. "
            "Fixtures, compilation, correctness checks and memory runs excluded. "
            "No Python crossing or Polars work."
        ),
        "timing": "Methods rotated each sample; tiny calls batched; warmups precede samples.",
        "memory": (
            "Separate untimed allocator run: peak requested live heap and allocation count, "
            "including output and reallocations; excludes input buffers, verification, "
            "allocator overhead, stack and process RSS."
        ),
        "correctness": (
            "All candidates equal the independent direct-membership cell oracle at n<=1000; "
            "large calls compare entire canonical outputs and structural/boundary invariants. "
            "Named and Proptest candidate tests additionally use direct per-integer-tick checks."
        ),
        "candidates": {
            "production": (
                "Retained public core: independently sorted endpoint-only unit streams or "
                "natural-alignment (coordinate,weight) records."
            ),
            "events": "One sorted coordinate/tagged-row-index array; departures before arrivals.",
            "heap": "Verified/sorted start row indices, dynamically growing active-end heap.",
            "streams_indices": (
                "Weighted only: original preparation and shared-iterator sweep over row-index "
                "streams, with source endpoint/weight lookups. Lower-memory comparison."
            ),
        },
        "dimensions": (
            "Synthetic family, order, load mode, endpoint dtype, domain mode, zero mode, "
            "seed and n. Million-row cases are a focused subset marked in the harness. "
            "m counts positive clipped nonempty rows, u their distinct endpoints plus domain edges, "
            "z canonical segments, omega peak positive active row count. "
            "Explicit core weights use the separately labelled weight_dtype. "
            "The public native adapter widens input weights to i128 before core dispatch."
        ),
        "source_sha256": {path: sha256(ROOT / path) for path in paths},
        "source_archive": archive_path.name,
        "limitations": (
            "Single-machine synthetic core experiment. No grouped, chunked, temporal Polars or "
            "native-Polars timing here; those belong to a separate end-to-end run. "
            "No quadratic oracle is constructed for large cases."
        ),
    }
    if platform.system() == "Windows":
        metadata["cpu_name"] = command(
            "powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor).Name"
        )
    with zipfile.ZipFile(archive_path, "x", compression=zipfile.ZIP_DEFLATED) as archive:
        for path in paths:
            archive.write(ROOT / path, path)
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    result = subprocess.run(invocation, cwd=ROOT, env={**os.environ, **settings}, check=False)
    metadata["returncode"] = result.returncode
    if result.returncode == 0:
        with output.open(encoding="utf-8") as raw:
            next(raw, None)
            if next(raw, None) is None:
                metadata["returncode"] = 1
                metadata["error"] = "No samples emitted; check the selected cases and sizes"
    metadata["source_archive_sha256"] = sha256(archive_path)
    metadata["sources_unchanged_during_run"] = all(
        sha256(ROOT / path) == digest for path, digest in metadata["source_sha256"].items()
    )
    metadata["changed_sources_during_run"] = [
        path for path, digest in metadata["source_sha256"].items() if sha256(ROOT / path) != digest
    ]
    if output.exists():
        metadata["raw_sha256"] = sha256(output)
    metadata_path.write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
    raise SystemExit(metadata["returncode"])


if __name__ == "__main__":
    main()

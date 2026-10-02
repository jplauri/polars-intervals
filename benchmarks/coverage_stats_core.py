"""Measure full Rust per-query coverage calls with reproducible source archives."""

import argparse
import platform
from pathlib import Path

from provenance import ROOT, command, run_cargo_bench, sha256


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--sizes", default="0,1,8,1000,10000,100000,1000000")
    parser.add_argument("--seeds", default="7,41")
    parser.add_argument("--samples", type=int, default=5)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--dtypes", default="i64,u64,i16")
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
    if not set(args.dtypes.split(",")) <= {"i64", "u64", "i16"}:
        parser.error("Endpoint dtypes must be i64, u64 or i16")
    output = args.output.resolve()
    paths = [
        "Cargo.toml",
        "Cargo.lock",
        "rust-toolchain.toml",
        "crates/intervals-core/Cargo.toml",
        "crates/intervals-core/src/lib.rs",
        "crates/intervals-core/src/coverage.rs",
        "crates/intervals-core/src/geometry.rs",
        "crates/intervals-core/src/coverage_stats.rs",
        "crates/intervals-core/benches/coverage_stats.rs",
        "crates/intervals-core/benches/support/coverage_stats_candidates.rs",
        "crates/intervals-core/benches/support/allocations.rs",
        "crates/intervals-core/benches/support/random.rs",
        "crates/intervals-core/tests/coverage_stats.rs",
        "crates/intervals-core/tests/support/coverage_stats.rs",
        "benchmarks/coverage_stats_core.py",
        "benchmarks/provenance.py",
    ]
    settings = {
        "STATS_CSV": str(output),
        "STATS_SIZES": args.sizes,
        "STATS_SEEDS": args.seeds,
        "STATS_SAMPLES": str(args.samples),
        "STATS_WARMUPS": str(args.warmups),
        "STATS_DTYPES": args.dtypes,
        "STATS_CASES": args.cases,
    }
    metadata = {
        "scope": (
            "Complete core call including both validations, source preparation, sorting, union "
            "prefixes, query searches/scans, output restoration and fraction construction, "
            "allocation and output destruction. Excludes fixture construction, compilation, "
            "correctness checks and memory runs. No Python crossing or Polars work."
        ),
        "timing": "Candidates rotate per sample; tiny calls batched; warmups precede samples.",
        "phase_clocks": "Only whole-call wall time is measured; no internal phase clocks.",
        "memory": (
            "Separate untimed run: peak requested live heap and allocation count, including "
            "preparation, output and reallocations. Excludes caller inputs, verification, "
            "allocator bookkeeping, stack and RSS."
        ),
        "correctness": (
            "Original-source membership cell oracle for fixtures with n*m*m<=2000000. "
            "Every fixture checks whole-output candidate equality and structural bounds. "
            "Large fixtures additionally check direct original-source counts at eight query "
            "positions. Related candidates are not independent reduction validation. "
            "Named/Proptest tests compare cell and integer-tick oracles independently."
        ),
        "candidates": {
            "production": (
                "Public core: direct scans for verified sorted starts AND ends at n>=m; "
                "otherwise packed sweeps at n<=m and p>=32; otherwise preallocated binary."
            ),
            "binary": "Preallocated binary queries using shared production source preparation.",
            "sweep": (
                "Private packed query-boundary sweeps with independent verified-order checks "
                "for query starts and ends; original-row restoration. Shares source preparation."
            ),
        },
        "dimensions": (
            "Synthetic family, independent query/source order, n:m ratio, dtype, seed, n and m. "
            "p is source union run count. Also records nonempty/duplicate source rows, distinct "
            "source endpoints, and query span minimum/upper median/maximum. "
            "i16 fixtures are omitted when any generated endpoint is outside its range."
        ),
        "limitations": (
            "Synthetic single-machine Rust-only experiment. No grouping, temporal metadata, "
            "payload preservation, native Polars, scans or lazy execution measured here. "
            "Sparse geometry is not measured with i16. Large lengths are verified in tests."
        ),
        "source_sha256": {path: sha256(ROOT / path) for path in paths},
    }
    if platform.system() == "Windows":
        metadata["cpu_name"] = command(
            "powershell", "-NoProfile", "-Command", "(Get-CimInstance Win32_Processor).Name"
        )
    invocation = ["cargo", "bench", "-p", "intervals-core", "--bench", "coverage_stats", "--locked"]
    try:
        returncode = run_cargo_bench(output, paths, settings, invocation, metadata)
    except FileExistsError as error:
        parser.error(str(error))
    raise SystemExit(returncode)


if __name__ == "__main__":
    main()

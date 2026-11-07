"""Run complete core set-geometry comparisons and preserve raw samples and provenance."""

import argparse
from pathlib import Path

from benchmarks.provenance import run_cargo_bench


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
    raw = args.output.resolve().with_suffix(".csv")
    settings = {
        "SET_GEOMETRY_CSV": str(raw),
        "SET_GEOMETRY_SIZES": args.sizes,
        "SET_GEOMETRY_SEEDS": args.seeds,
        "SET_GEOMETRY_SAMPLES": str(args.samples),
        "SET_GEOMETRY_WARMUPS": str(args.warmups),
        "SET_GEOMETRY_DTYPES": args.dtypes,
        "SET_GEOMETRY_CASES": args.cases,
    }
    invocation = ["cargo", "bench", "-p", "intervals-core", "--bench", "set_geometry", "--locked"]
    metadata = {
        "scope": "Complete core call including both-side validation, preparation, sortedness detection, sorting, union/scan/events, output allocation and destruction. No Python crossing or Polars. Compilation, fixtures, correctness and memory instrumentation excluded.",
        "memory": "Separate untimed call: peak requested live heap and allocation count, including algorithm buffers, output and reallocations. Excludes caller inputs, verification, stack, allocator overhead and RSS.",
        "verification": "All retained candidates have independent original-row cell and bounded bitmap oracle tests. Timing instances at <=64 rows use cell oracle; large instances compare full output and invariants, not an independent large oracle.",
        "methods": {
            "union_scan": "A: actual public implementation, canonical union then two-pointer scan.",
            "fused": "B: validated sorted packed pairs with on-demand merged runs.",
            "events": "C: tagged endpoint-event sweep with separate source counts.",
        },
        "limitations": "Single-machine synthetic core measurements. Not grouped/temporal/chunked Polars timings. Narrow i16 cases omitted when any endpoint is outside its range.",
    }
    try:
        returncode = run_cargo_bench(raw, settings, invocation, metadata)
    except FileExistsError as error:
        parser.error(str(error))
    raise SystemExit(returncode)


if __name__ == "__main__":
    main()

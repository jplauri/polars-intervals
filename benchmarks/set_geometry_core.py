"""Measure complete core set-geometry calls and record their provenance."""

from benchmarks.provenance import run_core


def main():
    metadata = {
        "scope": "Complete core call including both-side validation, preparation, sortedness detection, sorting, union and scan, output allocation and destruction. No Python crossing or Polars. Compilation, fixtures, correctness and memory instrumentation excluded.",
        "memory": "Separate untimed call: peak requested live heap and allocation count, including algorithm buffers, output and reallocations. Excludes caller inputs, verification, stack, allocator overhead and RSS.",
        "verification": "Public calls have independent original-row cell and bounded bitmap oracle tests. Timing instances at <=64 rows use the cell oracle; large instances check invariants, not an independent large oracle.",
        "methods": {
            "union_scan": "Actual public implementation, canonical union then two-pointer scan.",
        },
        "retired_candidates": "The fused and events comparisons remain at d6c3b01.",
        "limitations": "Single-machine synthetic core measurements. Not grouped/temporal/chunked Polars timings. Narrow i16 cases omitted when any endpoint is outside its range.",
    }
    run_core(__doc__, "set_geometry", "SET_GEOMETRY", metadata)


if __name__ == "__main__":
    main()

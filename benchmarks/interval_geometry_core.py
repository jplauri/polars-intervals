"""Measure complete core geometry calls and record their provenance."""

from benchmarks.provenance import DTYPES, run_core


def main():
    metadata = {
        "scope": "Complete core call including validation, preparation, sortedness check when selected, clipping, sorting, scan, canonical cluster remapping, output allocation and destruction. No Python crossing or Polars. Compilation, fixtures, correctness and memory instrumentation excluded.",
        "memory": "Separate untimed call: peak requested live heap and allocation count, including algorithm buffers, output and reallocations. Excludes caller inputs, verification, stack, allocator overhead and RSS.",
        "verification": "Public calls checked against independent original pair-relation graph and direct-membership elementary-cell oracles in the geometry tests, plus bitmap checks in core properties. Timing instances at <=64 rows use those oracles; larger instances check invariants, not an independent large oracle.",
        "methods": {
            "production": "Actual public implementation: scan original endpoint buffers after verifying relevant start order, otherwise sort packed records. Canonical row-aligned IDs and fused bounded gaps.",
        },
        "retired_candidates": "The packed_sort, packed_sorted, indices_sorted and materialized comparisons remain at d6c3b01.",
        "limitations": "Single-machine synthetic core measurements. Not grouped/temporal/chunked Polars timings. Narrow i16 cases are omitted when any endpoint or domain bound is outside its range. See raw dimensions and separate complete-call report.",
    }
    operations = {"--operations": ("cluster,merge,gaps", {"cluster", "merge", "gaps"})}
    run_core(__doc__, "geometry", "GEOMETRY", metadata, DTYPES | operations)


if __name__ == "__main__":
    main()

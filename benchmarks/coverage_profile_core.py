"""Run complete Rust coverage-profile comparisons and preserve source evidence.

Run sequentially on an idle machine. This does not build or measure the Python
extension.
"""

from provenance import run_core


def main():
    metadata = {
        "scope": (
            "Complete core call: validation, domain inference/clipping, sortedness checks, "
            "preparation, sweep, coalescing, output allocation, output destruction. "
            "Fixtures, compilation, correctness checks and memory runs excluded. "
            "No Python crossing or Polars work."
        ),
        "timing": "Tiny calls batched; warmups precede samples.",
        "memory": (
            "Separate untimed allocator run: peak requested live heap and allocation count, "
            "including output and reallocations; excludes input buffers, verification, "
            "allocator overhead, stack and process RSS."
        ),
        "correctness": (
            "The public call equals the independent direct-membership cell oracle at n<=1000; "
            "large calls check structural and boundary invariants. "
            "Named and Proptest tests additionally use direct per-integer-tick checks."
        ),
        "candidates": {
            "production": (
                "Retained public core: independently sorted endpoint-only unit streams or "
                "natural-alignment (coordinate,weight) records."
            ),
        },
        "retired_candidates": "The events, heap and streams_indices comparisons remain at d6c3b01.",
        "dimensions": (
            "Synthetic family, order, load mode, endpoint dtype, domain mode, zero mode, "
            "seed and n. Million-row cases are a focused subset marked in the harness. "
            "m counts positive clipped nonempty rows, u their distinct endpoints plus domain edges, "
            "z canonical segments, omega peak positive active row count. "
            "Explicit core weights use the separately labelled weight_dtype. "
            "The public native adapter widens input weights to i128 before core dispatch."
        ),
        "limitations": (
            "Single-machine synthetic core experiment. No grouped, chunked, temporal Polars or "
            "native-Polars timing here; those belong to a separate end-to-end run. "
            "No quadratic oracle is constructed for large cases."
        ),
    }
    selections = {
        "--dtypes": ("i64,u64", {"i64", "u64"}),
        "--weight-dtypes": ("i64", {"i64", "i128"}),
    }
    run_core(__doc__, "coverage_profile", "PROFILE", metadata, selections, record_cpu=True)


if __name__ == "__main__":
    main()

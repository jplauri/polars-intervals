"""Measure full Rust per-query coverage calls and record their provenance."""

from provenance import run_core


def main():
    metadata = {
        "scope": (
            "Complete core call including both validations, source preparation, sorting, union "
            "prefixes, query searches/scans, output restoration and fraction construction, "
            "allocation and output destruction. Excludes fixture construction, compilation, "
            "correctness checks and memory runs. No Python crossing or Polars work."
        ),
        "timing": "Tiny calls batched; warmups precede samples.",
        "phase_clocks": "Only whole-call wall time is measured; no internal phase clocks.",
        "memory": (
            "Separate untimed run: peak requested live heap and allocation count, including "
            "preparation, output and reallocations. Excludes caller inputs, verification, "
            "allocator bookkeeping, stack and RSS."
        ),
        "correctness": (
            "Original-source membership cell oracle for fixtures with n*m*m<=2000000. "
            "Every fixture checks structural bounds. "
            "Large fixtures additionally check direct original-source counts at eight query "
            "positions. Unit tests check every route against the cell oracle. "
            "Named/Proptest tests compare cell and integer-tick oracles independently."
        ),
        "candidates": {
            "production": (
                "Public core: direct scans for verified sorted starts AND ends at n>=m; "
                "otherwise packed sweeps at n<=m and p>=32; otherwise preallocated binary."
            ),
        },
        "retired_candidates": "The forced binary and sweep route comparisons remain at d6c3b01.",
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
    }
    run_core(__doc__, "coverage_stats", "STATS", metadata, record_cpu=True)


if __name__ == "__main__":
    main()

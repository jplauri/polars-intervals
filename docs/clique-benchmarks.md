# Maximum-weight clique benchmarks

## Summary

[`max_weight_clique`](usage.md#select-a-maximum-weight-clique) selects one exact
maximum-weight set of pairwise intersecting intervals. Production uses separately
sorted endpoint streams: endpoint-only arrays for implicit units and row-index
arrays for explicit weights. A validated common-intersection fast path avoids
sorting when all useful nonempty intervals form one clique. Reconstruction is
one final pass.

This choice favors unit performance, repeated endpoints and low scratch memory.
The heap remains faster on some shuffled, low-concurrency weighted inputs;
native-weight packed streams sometimes win for Rust callers with i64 weights.
There is no universal-fastest claim. See the [shared methodology](benchmarking.md),
[hardware](benchmarks.md#hardware), and
[correctness and candidate notes](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/clique-notes.md).

## Results

**Release Polars collections · one Polars thread · median of 5 samples · seed 42.**
Collection optimization, validation, extraction and Boolean mask materialization
are timed; fixture/query creation, checks and result destruction are excluded.
The runner verifies the installed extension against the local release library.
[Metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.metadata.json)
records hashes and workload settings.

--8<-- "docs/assets/benchmarks/clique-polars-table.md"

At 1m shuffled Int64 rows, implicit units took 43.1 ms versus 148 ms for explicit
ones. The same unit workload across 32 interleaved groups took 49.6 ms. The
default path avoids weight extraction and payloads.

**Rust core · one thread · median of 5 samples · seed 7.** Complete calls include
validation, preparation, sorting, optimization, allocation, reconstruction and
result destruction. Tiny calls are batched; correctness checks are untimed.
[Metadata](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-windows-20260928.metadata.json)
identifies the measured production source.

--8<-- "docs/assets/benchmarks/clique-core-table.md"

At 1m rows, implicit units with shuffled concurrency 64 took 40.6 ms versus
75.4 ms for endpoint events and 119 ms for the heap. Full cliques with i128 weights
took 6.04 ms versus 97.0 ms for packed streams. The losses matter: at 100k
shuffled concurrency-2 rows, the heap took 4.58 ms versus production's 6.56 ms.
At 1m, packed i128 streams took 99.3 ms versus 116 ms, using 64 MB versus 16 MB
of requested heap. Native i64 packed streams were faster still on that family
(61.9 ms), using 32 MB.

The separate repeat reproduced the tradeoffs: 135 ms versus 99.2 ms packed for
1m i128 low-concurrency rows; 5.87 ms versus 99.4 ms for full cliques. This
variation cautions against treating small differences as reliable crossovers.
Runs are not pooled.

Requested heap is measured separately, including scratch/output and excluding
inputs, allocator overhead, stack and RSS. General streams need 16 MB per million
useful rows, released before the 1 MB mask. The common path needs only the mask.
Packed i128 streams and signed-delta events need 64 MB; tagged-index events need
32 MB. These figures include record padding. The active heap grows with
concurrency, alongside an O(m) sorted index array.

## Coverage and limitations

The matrix covers tiny inputs through 1m rows, signed/unsigned endpoints, five
weight modes, varied row orders, sparse through dense intersections,
duplicates, repeated endpoints, disconnected blocks, empties and integer extremes.
Polars adds temporal, grouped and multichunk streaming collections.
Quadratic scanning stops at 64 rows; only selected families reach 1m. Coordinate
compression was not implemented. One machine and synthetic fixtures do not
establish performance on other platforms or application distributions.

Exploration also compared independent sortedness detection and quadratic
scanning. Neither justified a dispatcher: extra sortedness checks had no
convincing general benefit, and tiny scanning wins varied by geometry and size.
The common path addresses the clearest dense-input gain.

## Reproduce and data

Raw [core samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-windows-20260928.csv),
[repeat samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-repeat-windows-20260928.csv),
[exploratory samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-core-exploratory-windows-20260928.csv),
and [Polars samples](https://github.com/jplauri/polars-intervals/blob/master/benchmarks/results/clique-polars-windows-20260928.timings.csv)
remain separate; each run has matching metadata. Tables retain exact medians,
sample ranges and counts. Independent subset and quadratic oracles validate all
candidates in Rust tests; full-size benchmark checks use a separate exact
coordinate-score oracle and linear clique feasibility checks.

??? example "Commands"

    Follow [release setup](benchmarking.md#setup), then use new output filenames:

    ```powershell
    $env:CLIQUE_CSV = "$PWD/benchmarks/results/clique-core-new.csv"
    cargo bench -p intervals-core --bench max_weight_clique --locked
    $env:POLARS_MAX_THREADS = "1"
    uv run --no-sync python benchmarks/max_weight_clique.py --output benchmarks/results/clique-polars-new
    ```

    `CLIQUE_SIZES`, `CLIQUE_CASES`, `CLIQUE_SEEDS`, `CLIQUE_DTYPES`,
    `CLIQUE_WEIGHT_DTYPES`, `CLIQUE_METHODS`, `CLIQUE_SAMPLES`, and
    `CLIQUE_WARMUPS` select focused core runs. Run metadata records the settings
    actually used. Regenerate tables with the [reporting commands](benchmarking.md#generate-plots).

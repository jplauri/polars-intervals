# Coverage profile sort cleanup — 2026-09-29

For 100,000 start-ordered synthetic short intervals with unit loads, seed 7,
the first pair measured **4.59 ms before and 5.12 ms after** removing the explicit
sortedness guards. Reversing run order reduced the difference to **1.3% slower**.
This is a code simplification, not an established performance improvement.

The core now calls the standard library sorts directly. Rust 1.98.1 already
detects ordered inputs in linear time. The benchmark wrapper also computes its
closing source-change list once. That bookkeeping is outside the timed call.
The four private/public methods, validation, clipping, sweep and coalescing
contracts are unchanged.

## Measurements

Each timing includes the complete Rust call and output destruction. Fixtures,
compilation, correctness checks and allocator measurements are outside timing.
Methods rotate within samples. The runs were sequential on the same Windows
Ryzen 9 3900X machine with pinned Rust 1.98.1. Metadata records the commands,
environment, source hashes, domain, load mode, ordering and n/m/u/z/concurrency.

The first pair covers eight cases at 8, 1,000 and 100,000 rows, seeds 7 and 41,
five samples and two warmups. Each run has 870 samples. Endpoints are Int64,
with implicit units or explicit i128 core weights matching the native adapter.
The repeat covers five cases at 100,000 rows, the same seeds, nine samples and
three warmups. Its order is after, then before, opposite to the first pair.
Each repeat has 306 samples. Runs are summarized separately, never pooled.

Production median change, **after / before - 1**, at 100,000 rows:

| Synthetic workload | First pair, seed 7 | First pair, seed 41 | Repeat, seed 7 | Repeat, seed 41 |
| --- | ---: | ---: | ---: | ---: |
| Short intervals, start ordered, units | +11.4% | +10.6% | +1.3% | +2.0% |
| Short intervals, shuffled, units | +11.9% | +5.4% | +11.0% | +0.8% |
| Nested intervals, units | +11.2% | +5.2% | +0.9% | +3.2% |
| Touching, reverse ordered, uniform weight | +12.1% | +0.1% | -2.5% | -6.7% |
| Variable durations, start ordered, heterogeneous weights | +0.6% | +0.2% | +10.9% | +2.0% |

Positive values mean slower. Unchanged candidates also drifted. For example,
the repeated shuffled-unit seed-7 event candidate slowed by 11.0%, almost the
same as production. The weighted seed-7 repeat remains an unfavorable result,
with more variation in the after samples. The ordered-unit repeats allow a
small constant-factor cost. These samples do not establish performance neutrality
or a consistent speedup. The change retains the simpler standard-library calls
and the same linear ordered-input complexity.

Peak requested live heap, allocation counts, record layouts and geometry were
identical for every matched result. The harness checked complete outputs against
the independent membership oracle for sizes up to 1,000. Larger cases checked
candidate equality and structural invariants. These are core measurements only.
No new complete Polars, temporal, grouped or million-row timings were taken.
Earlier results remain historical measurements of their archived sources.

## Reproduction and source evidence

Each metadata file records the exact runner arguments. Use a new output name
when repeating a command. Restore the relevant source archive to an isolated
checkout to reproduce either implementation. The before repeat restored only
the original core file and retained the untimed wrapper cleanup.

| Run, in execution order | Raw samples | Metadata | Measured source |
| --- | --- | --- | --- |
| Before | [CSV](coverage-profile-sort-before-20260929.csv) | [JSON](coverage-profile-sort-before-20260929.metadata.json) | [ZIP](coverage-profile-sort-before-20260929.sources.zip) |
| After | [CSV](coverage-profile-sort-after-20260929.csv) | [JSON](coverage-profile-sort-after-20260929.metadata.json) | [ZIP](coverage-profile-sort-after-20260929.sources.zip) |
| After repeat | [CSV](coverage-profile-sort-after-repeat-20260929.csv) | [JSON](coverage-profile-sort-after-repeat-20260929.metadata.json) | [ZIP](coverage-profile-sort-after-repeat-20260929.sources.zip) |
| Before repeat | [CSV](coverage-profile-sort-before-repeat-20260929.csv) | [JSON](coverage-profile-sort-before-repeat-20260929.metadata.json) | [ZIP](coverage-profile-sort-before-repeat-20260929.sources.zip) |

All 52 recorded source hashes, four archive hashes and four CSV hashes were
verified. Every run reports unchanged sources during measurement.

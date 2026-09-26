# Operation benchmarks

## Summary

State the problem, production choice, and main measured tradeoff. Identify
coverage that is not yet measured instead of implying all variants are covered.

## Compared implementations

| Method | Approach | Role |
| --- | --- | --- |
| Method name | Algorithm | Production, reference, or baseline |

Explain any semantic differences and why each baseline computes the same result.

## Results

- **CPU:** model, cores, and logical processors.
- **OS:** system, version, and architecture.
- **Rust:** compiler version and target.
- **Python / Polars:** versions for plugin timings, if applicable.
- **Build:** optimization profile and relevant flags.
- **Threads:** settings for the core and plugin runs.
- **Samples:** warmups and timed samples per case.

Add memory or other library versions where relevant. Include the generated chart/table
snippet, then discuss the wider matrix, repeat runs, losing cases, and memory.
Separate core timings from end-to-end plugin timings.

## Workloads and correctness

Describe sizes, families, dtypes, orders, weights/capacities, seed, repetitions,
timed scope, independent checks, and omitted combinations.

## Reproduce

Link to the shared setup guide and give exact commands, smoke-run controls,
and a file table distinguishing benchmark runners from summary scripts.

## Limitations

Define the memory metric and exclusions. Record missing baselines, unmeasured
platforms, small-sample uncertainty, and limits of the selected workloads.

## Raw data

Link raw samples, repeat runs, environment/source hashes, and omission logs.
Put historical validation records in an expandable section labeled with the
recorded revision.

# Two-collection set geometry experiment

The public functions solve Boolean set difference and intersection of interval
collections. Each side represents a union. Multiplicity has no effect. The
result consists of maximal nonempty half-open ranges.

## Candidates

- A is the production union-plus-scan route. It reuses the existing canonical
  merge preparation. Start-sorted inputs avoid sorting, even when ends nest.
  The two-pointer scan retains a right run spanning several left runs.
- B sorts packed endpoint pairs only when needed, then generates merged runs
  on demand. It avoids two materialized union vectors but retains both packed
  input vectors. It is a private benchmark candidate.
- C sorts tagged start/end events. It maintains independent left and right
  active counts and applies Boolean membership after equal-coordinate events.
  It is a private benchmark candidate.
- D is a native Polars lazy event query. It uses separate signed Int64 deltas,
  reduces ties, computes per-group prefix sums, selects true cells and merges
  adjacent cells. The preceding selected end is compared with the current
  start, so a real excluded gap cannot disappear during coalescing.

D uses the package's schema/key checks and a blocking all-row Polars validator.
The callback performs vectorized filtering and reports the first invalid row
by source and original row ordinal. Geometry uses native lazy expressions.
Both branches enter one strict concatenation. No callback collects another
query, iterates rows/groups or carries state. Null key tuples match. A separate
left ordinal aggregate preserves group order before empty rows are removed.

## Production decision

Keep A for both operations. Repeated million-row Int64 runs show that B can
improve shuffled, sparse intersection. With seed 7, shuffled availability
intersection took 26.3 ms with B and 31.6 ms with A. Subtraction took 31.4 and
33.3 ms. These are complete Rust calls, including validation and destruction.
They are not Polars timings.

The improvement does not hold across geometry. For nested million-row inputs,
A took 17.5–17.6 ms and B took 21.1–21.4 ms. Dense inputs were close, with A
at 31.9–32.2 ms and B at 32.8–33.4 ms. At 100,000 rows, touching chains took
0.593–0.621 ms with A and 0.856–0.908 ms with B. Repeats use separate raw
files and seeds. Small differences do not establish a general winner.

B also fails to reduce peak requested heap on output-linear availability:
both used 3,145,728 bytes for 100,000-row intersection and 4,194,304 bytes
for subtraction. On dense inputs, A used 800,128 bytes versus B's 2,097,216.
These separate allocator measurements include preparation and output. They
exclude caller inputs, allocator overhead, stack and RSS. C used more memory
and was consistently slower on the large measured workloads.

The broad result favors shared merge code and simple scans over a public
dispatcher. B and C remain private because the task requires reproducible
comparisons. No all-pairs join, interval index or new dependency is needed.

Complete Polars runs also favor A on nonempty geometry. The headline repeat
at one million shuffled availability rows took 83.8/81.0 ms for subtraction/
intersection, versus D's 509/394 ms. A retains one clear limitation: a million
empty intervals split among nullable groups took 84.7/85.6 ms in lazy calls,
versus D's 18.7/18.4 ms. Repeating with seed 41 preserves the loss. The report
and overview keep it visible. No workload-specific dispatcher was added.

## Workloads and correctness

The fixtures are synthetic. Availability has repeated long windows split by
short busy windows. Dense intervals and nesting produce tiny outputs from
large inputs. Duplicates test set membership rather than signed depth
subtraction. Touching chains test coalescing. Disjoint inputs have empty
intersection. A singleton left domain with many right intervals stresses
linear output fragmentation. Other cases have empty sides, huge size ratios,
sparse coordinates, nullable/skewed/unmatched groups, wide payloads and chunks.

`start` means generated start order. `reverse` reverses the source sequence.
For duplicate patterns with periodic starts, this does not mean descending
start order. `partial` reverses each block of 64 rows. Shuffling is independent
on both sides. Sizes count both input collections together.
The `groups` setting counts numeric key values. Nullable fixtures can have
one additional observed null group. The CSV records actual observed groups,
matching fraction and largest-group share separately.

Small inputs compare entire outputs with direct original-row membership on
distinct boundary cells. Rust also uses a bounded bitmap oracle. Larger
timing inputs check full cross-implementation equality and canonical output
invariants. Agreement between large candidates is not an independent oracle.

## Saved evidence

`set-geometry-core-20260930` is the broad 0–100,000-row run. The `repeat` run
uses five samples and two seeds at 100,000 rows. The `million` run does the
same at one million. Each has raw CSV, metadata and a measured source ZIP.
All recorded sources remained unchanged during each run. A later harness-only
fix wraps the right-side seed increment and bounds the seed contribution before
converting to Int64. It leaves measured seeds 7 and 41 unchanged. The original
measured harness remains in each archive.

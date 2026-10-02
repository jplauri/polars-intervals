# Per-query coverage statistics

[`coverage_stats`](api.md#polars_intervals.coverage_stats) measures source coverage
inside each reporting window. It preserves every query row, its original columns,
and its original order. Overlapping windows and duplicate query IDs are valid.

```python
import polars as pl
import polars_intervals as pi

windows = pl.DataFrame({"start": [0, 5, 12, 7], "end": [10, 10, 15, 7]})
reads = pl.DataFrame({"start": [1, 4], "end": [7, 9]})
result = pi.coverage_stats(windows, reads)
```

| Query | overlap_count | covered_length | query_length | covered_fraction |
| --- | ---: | ---: | ---: | ---: |
| [0, 10) | 2 | 8 | 10 | 0.8 |
| [5, 10) | 2 | 4 | 5 | 0.8 |
| [12, 15) | 0 | 0 | 3 | 0.0 |
| [7, 7) | 0 | 0 | 0 | null |

All intervals include their start and exclude their end. Touching alone does
not overlap. The four appended columns always appear in the order shown.
Counts use `UInt64`. Both lengths use `Int128`. Fractions use `Float64`.
Existing query columns with any of these four names are rejected, even on empty
inputs. Rename those columns before calling the function. Other source columns
are ignored.

## Count, breadth, and depth

`overlap_count` counts original nonempty source rows that overlap the query.
`covered_length` measures the union of those sources inside the query.
For the first window above, the union has length eight. The overlapping part
counts once toward covered length.

Two identical source rows `[1, 7)` give a count of two and a covered length of
six inside `[0, 10)`. Source duplication increases counts but never duplicates
covered length. A weight column has no effect.

Coverage depth is a different quantity. Use [coverage profiles](coverage-profile.md)
to report simultaneous depth at each coordinate. Use [set intersection](interval-geometry.md#subtract-and-intersect-two-collections)
to return canonical covered coordinates. `coverage_stats` reports independently
for every query and never merges queries.

Empty queries return `(0, 0, 0, null)`, even inside a covered region. Empty sources
do not contribute. Passing the same frame as both operands includes a nonempty
row's own source copy. This differs from the one-collection `overlap_count`
expression, which counts only other rows.

## Groups and custom columns

```python
windows = pl.DataFrame(
    {
        "chromosome": ["chr1", None, "chr2", "chr1"],
        "window_start": [0, 0, 20, 5],
        "window_end": [10, 5, 25, 10],
        "label": ["first", "unknown", "unmatched", "last"],
    }
)
reads = pl.DataFrame(
    {
        "chromosome": ["chr1", None, "chr1"],
        "start": [1, 1, 4],
        "end": [7, 3, 9],
    }
)
result = pi.coverage_stats(
    windows,
    reads,
    query_start="window_start",
    query_end="window_end",
    by="chromosome",
)
# covered_length: [8, 2, 0, 4], in original window order.
```

Use a list for composite keys. Nulls match nulls in each key position.
Queries with no matching source group have zero coverage and keep their full
length. Source-only groups produce no rows, but still validate. `by=None` and
`by=[]` both mean ungrouped collections. Key dtypes must match exactly on both
inputs.
Supported keys are String, Boolean, 8/16/32/64-bit integers, Date, and Datetime.
Keys named `start` or `end` are allowed with custom endpoint columns.

Column options are literal names. Names such as `*` and `^end$` do not expand as
selectors. Other query columns may include nullable, nested, and categorical
columns supported by the selected Polars execution mode. They keep their
original dtypes and column order.

## Exact lengths and approximate fractions

All four endpoint columns must have the same logical dtype, including Datetime
unit and timezone. Supported types are signed and unsigned 8/16/32/64-bit
integers, Date, and Datetime. Null, floating-point, and Int128 endpoints are not
accepted. Typed empty operands must also match. There is no implicit casting.

Lengths use the input coordinate units. Date lengths are days. Datetime lengths
are elapsed physical milliseconds, microseconds, or nanoseconds. Across a clock
change, elapsed ticks determine the answer, rather than wall-clock subtraction.

Endpoints widen before subtraction. A full `Int64` span has length `2**64 - 1`.
Each group's source-union length is bounded by its coordinate span, also at most
`2**64 - 1`. Union prefixes and their differences remain exact in `Int128`.

The fraction converts covered length and query length separately to `Float64`,
then divides. Empty queries have null fractions. Uncovered and fully covered
nonempty queries report 0.0 and 1.0. Very large, nearly covered queries can round
to 1.0 despite a small gap. Use the exact integer columns to distinguish them.

## Deferred execution with both inputs

```python
query = (
    windows.lazy()
    .pipe(
        pi.coverage_stats,
        reads.lazy(),
        query_start="window_start",
        query_end="window_end",
        by="chromosome",
    )
    .filter(pl.col("covered_fraction") < 0.9)
)
out = query.collect()
```

Two DataFrames return a DataFrame. Any LazyFrame input returns a LazyFrame,
including either mixed eager/lazy combination. Construction, `explain()`, and
`collect_schema()` do not execute either input. Both inputs are evaluated together
when you collect the result. Query rows keep their original columns and positions
without a join on coordinates, group keys, or user IDs.

All chunks in each operand contribute to the calculation. Collection with
`engine="streaming"` gives the same result, but the coverage calculation needs
both complete inputs in memory. It is not an out-of-core streaming algorithm.

Downstream filters and slices cannot truncate the sources. Filtering query rows
upstream, while keeping the sources fixed, preserves surviving rows' statistics.
Every endpoint row remaining after upstream filters is validated, including rows
in unmatched groups and calls with an empty input. Errors name `queries` or
`intervals` and the row's position in that input after upstream operations.

## Algorithm and evidence

Sources are prepared once per group. Two sorted arrays retain every nonempty
source start and end, including duplicates. For a nonempty query `[a, b)`, its
count is `starts < b` minus `ends <= a`. Every row ending by `a` also starts
before `b`, so this subtraction removes exactly the sources to the left.
Sources starting at `b` and those ending at `a` never count.

The source union is merged into sorted runs. An exact cumulative length stores
the total length before each run. At boundary `x`, the covered prefix contains
all runs ending by `x`, plus the partial current run if its start precedes `x`.
Gaps add nothing. Subtracting the prefixes at `a` and `b` gives covered length.
Empty queries are handled separately.

With `n` query rows, `m` original source rows, and `p` union runs in a group,
the implementation chooses one of three ways to answer boundaries:

- If both query starts and ends are verified sorted and `n >= m`, scan the
  source arrays with four advancing positions. Start order alone is insufficient.
- Otherwise, if `n <= m` and `p >= 32`, sort query-boundary requests, scan the
  source arrays, and restore the original query positions.
- Otherwise, use binary searches. Length searches use only the `p` union runs.

This rule comes from repeated measurements. It does not guarantee the fastest
route for every input. Counts always use original source rows. Empty source rows
still contribute to `m`, and every row validates before choosing a route.

Worst-case time is `O(m log(m+1) + n log(m+1) + n)` with `O(m+n)` extra storage.
The sorted-request route fits this bound because it is used only when `n <= m`.
The ordered scan takes linear work after source preparation. Actual source order
is also checked to avoid unnecessary sorting. No route enumerates overlapping
pairs or expands coordinates.

See the [benchmark report](coverage-stats-benchmarks.md) for complete public-call
measurements, the separate binary-search and sweep candidates, and the native
Polars comparison.
Tests use independent original-row cell and bitmap oracles. Agreement between
implementations is an additional check, not the correctness oracle.

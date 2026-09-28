"""Handwritten balance regressions; separate from the corpus consumer and generators.

These records use the generator's record shape. No optimizer runs here. Witnesses
are checked by the consumer only for scoring, and never passed to a lane method.
"""

from generate_interval_graphs import Instance, structural_stats


def record(name, rows, *, supplied=None, witness=None, params=None):
    starts = [s for s, _ in rows]
    ends = [e for _, e in rows]
    instance = Instance(
        "handcrafted",
        starts,
        ends,
        0,
        {"n": len(rows), "order": "fixture", "name": name, **(params or {})},
        provenance={"source": "balance_lanes_fixtures.py", "fixture": name},
    )
    result = {
        "id": instance.id,
        "family": instance.family,
        "seed": instance.seed,
        "n": len(rows),
        "params": instance.params,
        "stats": structural_stats(starts, ends),
        "certificate": None,
        "provenance": instance.provenance,
        "intervals": rows,
    }
    if supplied is not None:
        result["supplied_lanes"] = supplied
    if witness is not None:
        result["equity_witness"] = witness
    return result


def iter_handcrafted(sizes=()):
    rows, supplied, witness = [], [], []
    offset = 0
    for i, leaves in enumerate([9, 8, 7, 6]):
        rows.append((offset, offset + 2 * leaves + 1))
        rows.extend((offset + 2 * j + 1, offset + 2 * j + 2) for j in range(leaves))
        color, optimal_color = int(i >= 2), int(i in (1, 2))
        supplied.extend([1 - color] + [color] * leaves)
        witness.extend([1 - optimal_color] + [optimal_color] * leaves)
        offset += 2 * leaves + 4
    yield record("simultaneous_flip", rows, supplied=supplied, witness=witness)

    rows, supplied, witness = [], [], []
    offset = 0
    # F(w) = K2 joined to w+1 disjoint leaves. The supplied 15/16/17
    # coloring is pairwise optimal; the witness attains global 16/16/16.
    for weight, color, optimal_color in zip(
        [9, 6, 5, 5, 4, 1], [0, 2, 1, 2, 1, 1], [0, 1, 2, 2, 1, 0], strict=True
    ):
        leaves = weight + 1
        rows.extend([(offset, offset + 2 * leaves + 1)] * 2)
        rows.extend((offset + 2 * j + 1, offset + 2 * j + 2) for j in range(leaves))
        supplied.extend([c for c in range(3) if c != color] + [color] * leaves)
        witness.extend([c for c in range(3) if c != optimal_color] + [optimal_color] * leaves)
        offset += 2 * leaves + 4
    yield record("pairwise_limitation", rows, supplied=supplied, witness=witness)
    yield record("empty_input", [])
    yield record("empty_only", [(2, 2)] * 7)
    yield record("interior_empties", [(0, 10), (1, 9)] + [(5, 5)] * 9)
    for n in sizes:
        yield record(
            "late_clique", [(2 * i, 2 * i + 1) for i in range(n - 8)] + [(2 * n, 2 * n + 1)] * 8
        )
        yield record("long_short", [(0, 2 * n)] + [(2 * i, 2 * i + 1) for i in range(n - 1)])
        yield record("tie_heavy", [(i // 16, i // 16 + 3) for i in range(n)])
        yield record("large_clique", [(0, 1)] * n)
        yield record(
            "nearly_clique",
            [(0, 2 * n)] * (n // 2) + [(2 * i, 2 * i + 1) for i in range(n - n // 2)],
        )

"""Independent scheduling-oracle checks without benchmark timings."""

import random
import unittest

from balance_lanes import validate_coloring
from scheduling_polars import weighted_optimum


class SchedulingOracleTests(unittest.TestCase):
    def test_weighted_suffix_matches_exhaustive_original_problem(self):
        rng = random.Random(7)
        cases = [
            [],
            [(0, 5, 10), (2, 2, 3), (2, 2, 4), (3, 3, 0), (4, 4, -7)],
            [(0, 2, -1), (1, 3, 0), (2, 4, 5), (4, 6, 5)],
            [(0, 4, 8), (0, 2, 4), (2, 4, 4)],
        ]
        for _ in range(50):
            rows = []
            for _ in range(7):
                start = rng.randrange(5)
                rows.append((start, start + rng.randrange(4), rng.randrange(-3, 6)))
            cases.append(rows)
        for rows in cases:
            best = 0
            for bits in range(1 << len(rows)):
                chosen = [row for i, row in enumerate(rows) if bits & (1 << i)]
                if all(
                    start == end
                    or other_start == other_end
                    or end <= other_start
                    or other_end <= start
                    for i, (start, end, _) in enumerate(chosen)
                    for other_start, other_end, _ in chosen[i + 1 :]
                ):
                    best = max(best, sum(weight for _, _, weight in chosen))
            with self.subTest(rows=rows):
                self.assertEqual(weighted_optimum(rows), best)

    def test_lane_oracle_rejects_conflicts_and_extra_lanes(self):
        with self.assertRaisesRegex(ValueError, "overlapping intervals"):
            validate_coloring([(0, 3), (1, 2), (4, 5)], [0, 0, 1])
        with self.assertRaisesRegex(ValueError, "minimum palette"):
            validate_coloring([(0, 1), (1, 2)], [0, 1])
        self.assertEqual(validate_coloring([(0, 3), (1, 2), (1, 1)], [0, 1, 0])["k"], 2)


if __name__ == "__main__":
    unittest.main()

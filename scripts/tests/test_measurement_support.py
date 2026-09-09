from __future__ import annotations

import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from measurement_support import (  # noqa: E402
    MeasurementFailure,
    alternating_order,
    deterministic_projection,
    run_checked,
    timing_summary,
    unique_run_directory,
)


class MeasurementSupportTests(unittest.TestCase):
    def test_deterministic_projection_excludes_operational_metadata(self) -> None:
        left = {"deterministic": {"artifact": "abc"}, "operational": {"wall_ms": 1.0}}
        right = {"deterministic": {"artifact": "abc"}, "operational": {"wall_ms": 99.0}}

        self.assertEqual(deterministic_projection(left), deterministic_projection(right))

    def test_timing_summary_records_median_and_dispersion(self) -> None:
        self.assertEqual(
            timing_summary([1.0, 2.0, 10.0]),
            {
                "samples": 3,
                "median_ms": 2.0,
                "median_absolute_deviation_ms": 1.0,
                "min_ms": 1.0,
                "max_ms": 10.0,
            },
        )

    def test_alternating_order_balances_a_pair(self) -> None:
        self.assertEqual(alternating_order(("left", "right"), 0), ("left", "right"))
        self.assertEqual(alternating_order(("left", "right"), 1), ("right", "left"))

    def test_run_directories_are_unique_and_contained(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            first = unique_run_directory(parent, "measurement")
            second = unique_run_directory(parent, "measurement")
            self.assertNotEqual(first, second)
            self.assertEqual(first.parent, parent)
            self.assertEqual(second.parent, parent)

    def test_watchdog_terminates_a_slow_process_group(self) -> None:
        with self.assertRaisesRegex(MeasurementFailure, "exceeded"):
            run_checked(
                [sys.executable, "-c", "import time; time.sleep(10)"],
                operation="slow fixture",
                timeout_seconds=0.05,
            )


if __name__ == "__main__":
    unittest.main()

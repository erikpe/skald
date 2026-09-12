from __future__ import annotations

import sys
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from measure_cleanup_baseline import parse_analysis_usage  # noqa: E402
from measurement_support import MeasurementFailure  # noqa: E402


class CleanupBaselineTests(unittest.TestCase):
    def test_analysis_usage_is_attached_to_the_exact_schedule_occurrence(self) -> None:
        stderr = b"\n".join(
            [
                b"skac: trace: proof-rich MIR pass `fold` (pass identity 2, schedule position 3, occurrence 1) unchanged in 0.100 ms",
                b"skac: trace stats: processed callables: 2",
                b"skac: trace analysis: local-constants: requests 2, computations 2, repeated snapshot requests 1, results before 0, inserted 0, discarded 0",
            ]
        )

        self.assertEqual(
            parse_analysis_usage(stderr),
            [
                {
                    "position": 3,
                    "pass": "fold",
                    "stage": "proof-rich",
                    "occurrence": 1,
                    "analysis": "local-constants",
                    "requests": 2,
                    "computations": 2,
                    "repeated_snapshot_requests": 1,
                    "distinct_callable_snapshot_keys": 1,
                    "results_before": 0,
                    "results_inserted": 0,
                    "results_discarded": 0,
                }
            ],
        )

    def test_orphaned_analysis_usage_is_rejected(self) -> None:
        with self.assertRaisesRegex(MeasurementFailure, "preceded"):
            parse_analysis_usage(
                b"skac: trace analysis: local-constants: requests 1, computations 1, repeated snapshot requests 0, results before 0, inserted 0, discarded 0"
            )


if __name__ == "__main__":
    unittest.main()

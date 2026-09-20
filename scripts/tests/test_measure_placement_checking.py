from __future__ import annotations

import json
import sys
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(SCRIPTS))

from measurement_support import MeasurementFailure  # noqa: E402
from measure_placement_checking import (  # noqa: E402
    NATIVE_MARKER,
    PLACEMENT_MARKER,
    profile_records,
    select_witnesses,
    summarize_run,
)


class PlacementCheckingMeasurementTests(unittest.TestCase):
    def test_profile_records_accept_test_harness_prefixes(self) -> None:
        record = {"format": 1, "total_ns": 7}
        output = f"test example ... {PLACEMENT_MARKER}{json.dumps(record)}\n".encode()
        self.assertEqual(profile_records(output, PLACEMENT_MARKER), [record])

    def test_summarize_run_separates_structure_from_time(self) -> None:
        placement = {
            "format": 1,
            "total_ns": 10,
            "planning_ns": 1,
            "discovery_lowering_ns": 1,
            "executable_lowering_ns": 1,
            "selection_ns": 1,
            "placement_production_ns": 1,
            "placement_checking_ns": 2,
            "frame_planning_ns": 1,
            "realization_checking_ns": 1,
            "publication_ns": 1,
            "callables": 2,
            "resource_locations": 3,
            "storage_locations": 4,
            "abi_locations": 5,
            "tokens": 6,
            "state_bits": 7,
            "selected_events": 8,
            "transfers": 9,
            "reachable_blocks": 10,
            "edge_occurrences": 11,
            "convergence_rounds": 12,
            "block_visits": 13,
            "edge_visits": 14,
            "fact_removals": 15,
            "peak_pending_blocks": 16,
        }
        native = {"format": 1, "link_ns": 2, "execution_ns": 3}
        output = (
            PLACEMENT_MARKER
            + json.dumps(placement)
            + "\n"
            + NATIVE_MARKER
            + json.dumps(native)
        ).encode()
        deterministic, operational = summarize_run(output)
        self.assertEqual(deterministic["state_bits"], 7)
        self.assertEqual(deterministic["peak_pending_blocks"], 16)
        self.assertEqual(operational["placement_checking_ms"], 0.000002)
        self.assertEqual(operational["native_execution_ms"], 0.000003)

    def test_missing_profiles_fail_instead_of_creating_partial_reports(self) -> None:
        with self.assertRaisesRegex(MeasurementFailure, "no placement profile"):
            summarize_run(b"test result: ok")

    def test_witness_selection_rejects_unknown_ids(self) -> None:
        with self.assertRaisesRegex(MeasurementFailure, "unknown witnesses"):
            select_witnesses(["missing"])


if __name__ == "__main__":
    unittest.main()

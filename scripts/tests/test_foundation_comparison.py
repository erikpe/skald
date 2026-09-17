from __future__ import annotations

import copy
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from cleanup_measurements.comparison import classify_samples, compare_runs
from cleanup_measurements.manifest import POLICY


def capture(identity="first"):
    semantics = {"status": 0, "stdout_sha256": "out", "stderr_sha256": "err"}
    entry = {"id": "kernel", "assembly_sha256": "asm", "assembly_bytes": 300,
        "reporting_equivalent": True, "native_semantics": semantics,
        "compile_samples": [{"wall_ms": 100, "peak_rss_kib": 100} for _ in range(5)],
        "native_samples_ms": [100] * 9, "native_text": {"status": "supported", "bytes": 100},
        "frames": {"status": "supported", "functions": {"fn": {"status": "supported",
            "frame_bytes": 32, "max_stack_bytes": 32, "static_frame_accesses": 5}}}}
    return {"schema": 1, "kind": "foundation", "policy": POLICY, "run_id": identity,
        "qualification": "full", "compatibility": {"manifest_sha256": "manifest",
            "inputs": ["same sources"], "runtime": "same runtime", "host": "same host", "harness": "same harness",
            "configuration": {"target": "x86_64-sysv", "mir_profile": "default", "compiler_profile": "golden",
                "mir_exclusions": [], "compile_repeats": 5, "native_repeats": 9,
                "compile_warmups": 1, "native_warmups": 1},
            "workloads": [{"id": "kernel", "native": True, "expected": semantics}]},
        "variants": {role: {"compiler": {"revision": role, "executable_sha256": role,
            "profile": "golden", "build_toolchain": "rustc", "build_flags": "", "dirty": False}, "workloads": [copy.deepcopy(entry)]}
            for role in ("baseline", "candidate")}}


class FoundationComparisonTests(unittest.TestCase):
    def setUp(self):
        self.first, self.second = capture(), capture("second")

    def candidate(self, run):
        return run["variants"]["candidate"]["workloads"][0]

    def test_equal_runs_and_cross_compiler_assembly_changes_are_allowed(self):
        for run in (self.first, self.second):
            self.candidate(run)["assembly_sha256"] = "different candidate code"
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "within-review-limits")

    def test_repeatable_time_rss_and_text_regressions_require_review(self):
        for field, value in (("wall_ms", 111), ("peak_rss_kib", 116), ("native", 111), ("text", 116)):
            with self.subTest(field=field):
                first, second = capture(), capture("second")
                for run in (first, second):
                    entry = self.candidate(run)
                    if field == "native":
                        entry["native_samples_ms"] = [value] * 9
                    elif field == "text":
                        entry["native_text"]["bytes"] = value
                    else:
                        for sample in entry["compile_samples"]:
                            sample[field] = value
                self.assertEqual(compare_runs(first, second)["outcome"], "review-required")

    def test_threshold_equality_and_twice_larger_mad_are_strict(self):
        self.assertEqual(classify_samples([100] * 5, [110] * 5, .10, 5, timing=True)["outcome"],
                         "within-review-limits")
        self.assertEqual(classify_samples([100] * 5, [115] * 5, .15, 5, timing=False)["outcome"],
                         "within-review-limits")
        # Delta 20 equals twice the larger MAD (10); it cannot qualify a regression.
        self.assertEqual(classify_samples([90, 90, 100, 110, 110], [120] * 5,
            .10, 5, timing=True)["outcome"], "inconclusive")

    def test_one_regressing_pair_or_noise_cannot_be_declared_pass(self):
        self.candidate(self.first)["native_samples_ms"] = [111] * 9
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "inconclusive")
        for run in (self.first, self.second):
            self.candidate(run)["native_samples_ms"] = [80, 90, 90, 90, 100, 110, 110, 110, 120]
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "inconclusive")

    def test_mismatched_inputs_provenance_or_reused_capture_are_incompatible(self):
        for field in ("manifest_sha256", "inputs", "runtime", "host", "harness", "configuration"):
            second = capture("second")
            second["compatibility"][field] = "different"
            self.assertEqual(compare_runs(self.first, second)["outcome"], "incompatible")
        self.assertEqual(compare_runs(self.first, self.first)["outcome"], "incompatible")
        self.second["variants"]["baseline"]["compiler"]["revision"] = "different"
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "incompatible")

    def test_missing_metrics_and_short_samples_are_inconclusive(self):
        for field in ("frames", "native_text", "native_samples_ms", "compile_samples"):
            first = capture()
            self.candidate(first).pop(field)
            self.assertEqual(compare_runs(first, self.second)["outcome"], "inconclusive")
        for value in ([100] * 4, [float("nan")] * 5, [0] * 5, None, [None] * 5):
            self.assertEqual(classify_samples(value, [100] * 5, .1, 5, timing=True)["outcome"],
                             "inconclusive")
        self.candidate(self.first)["frames"]["functions"]["fn"].pop("frame_bytes")
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "inconclusive")

    def test_semantics_reporting_and_within_compiler_nondeterminism_block_evidence(self):
        for field, value in (("native_semantics", {"status": 0, "stdout_sha256": "wrong"}),
                             ("reporting_equivalent", False), ("assembly_sha256", "unstable")):
            first = capture()
            self.candidate(first)[field] = value
            self.assertEqual(compare_runs(first, self.second)["outcome"], "invalid")

    def test_smoke_subset_and_single_build_do_not_qualify_adoption(self):
        for qualification in ("smoke", "subset"):
            self.first["qualification"] = qualification
            self.assertEqual(compare_runs(self.first, self.second)["outcome"], "inconclusive")
        self.first["variants"].pop("candidate")
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "incompatible")

    def test_missing_compatibility_and_changed_build_or_mode_attestations_are_rejected(self):
        self.first.pop("compatibility")
        self.second.pop("compatibility")
        self.assertEqual(compare_runs(self.first, self.second)["outcome"], "incompatible")
        for field, value in (("build_flags", "-C opt-level=0"), ("build_toolchain", "other rustc")):
            first, second = capture(), capture("second")
            for run in (first, second):
                run["variants"]["candidate"]["compiler"][field] = value
            self.assertEqual(compare_runs(first, second)["outcome"], "incompatible")
        for run in (first, second):
            run["compatibility"]["configuration"]["mir_profile"] = "none"
        self.assertEqual(compare_runs(first, second)["outcome"], "incompatible")

    def test_frame_growth_and_added_removed_callables_remain_visible(self):
        for run in (self.first, self.second):
            frames = self.candidate(run)["frames"]["functions"]
            frames["fn"]["frame_bytes"] = 64
            frames["added"] = {"status": "supported", "frame_bytes": 8, "max_stack_bytes": 8, "static_frame_accesses": 0}
        result = compare_runs(self.first, self.second)
        self.assertEqual(result["outcome"], "within-review-limits")
        changes = result["workloads"][0]["frames"][0]["changes"]
        self.assertEqual(changes["fn"]["candidate"]["frame_bytes"], 64)
        self.assertIsNone(changes["added"]["baseline"])


if __name__ == "__main__":
    unittest.main()

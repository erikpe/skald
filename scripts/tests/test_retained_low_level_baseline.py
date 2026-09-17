from __future__ import annotations

import gzip
import json
import shutil
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from cleanup_measurements.comparison import compare_runs
from measurement_support import MeasurementFailure, sha256_bytes
from verify_low_level_baseline import read_json, verify_bundle


EVIDENCE = Path(__file__).resolve().parents[2] / "tests/measurements/low_level_compiler/pre_migration_9e3cebb1"


class RetainedBaselineTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.bundle = Path(self.temporary.name) / "evidence"
        shutil.copytree(EVIDENCE, self.bundle)

    def edit(self, name, change, *, restamp=True):
        path = self.bundle / name
        value = read_json(path)
        change(value)
        contents = (json.dumps(value) + "\n").encode()
        path.write_bytes(gzip.compress(contents, mtime=0) if path.suffix == ".gz" else contents)
        if restamp:
            index_path = self.bundle / "index.json"
            index = json.loads(index_path.read_text())
            index["files"][name] = sha256_bytes(path.read_bytes())
            index_path.write_text(json.dumps(index) + "\n")

    def test_retained_full_corpus_replays_without_measuring(self):
        result = verify_bundle(self.bundle)
        self.assertEqual(len(result["workloads"]), 21)
        self.assertEqual(result["outcome"], "inconclusive")
        self.assertEqual(result["issues"], [])

    def replay_comparison(self):
        result = compare_runs(read_json(self.bundle / "first.json.gz"),
                              read_json(self.bundle / "second.json.gz"))
        self.edit("comparison.json", lambda comparison: comparison.update(result))

    def candidate(self, report):
        return report["variants"]["candidate"]["workloads"][0]

    def test_unsupported_metrics_cannot_be_accepted_as_noisy_timing(self):
        self.edit("first.json.gz", lambda report: self.candidate(report)["frames"].update(status="unsupported"))
        self.replay_comparison()
        with self.assertRaisesRegex(MeasurementFailure, "missing/unsupported per-callable"):
            verify_bundle(self.bundle)

    def test_missing_raw_samples_cannot_be_accepted_as_noisy_timing(self):
        self.edit("first.json.gz", lambda report: self.candidate(report).update(compile_samples=[]))
        self.replay_comparison()
        with self.assertRaisesRegex(MeasurementFailure, "missing/invalid raw baseline"):
            verify_bundle(self.bundle)

    def test_corrupted_bytes_and_missing_files_are_rejected(self):
        self.edit("first.json.gz", lambda report: report.update(run_id="corrupted"), restamp=False)
        with self.assertRaisesRegex(MeasurementFailure, "hash mismatch: first.json.gz"):
            verify_bundle(self.bundle)
        (self.bundle / "first.json.gz").unlink()
        with self.assertRaises(OSError):
            verify_bundle(self.bundle)

    def test_comparison_must_reproduce_from_raw_samples(self):
        self.edit("comparison.json", lambda result: result.update(issues=["invented result"]))
        with self.assertRaisesRegex(MeasurementFailure, "does not reproduce"):
            verify_bundle(self.bundle)

    def test_raw_execution_log_cannot_lose_a_warmup(self):
        self.edit("first.json.gz", lambda report: report["events"].pop(0))
        with self.assertRaisesRegex(MeasurementFailure, "order/counts"):
            verify_bundle(self.bundle)

    def test_raw_rows_must_match_the_execution_log(self):
        self.edit("first.json.gz", lambda report: report["events"][2].update(wall_ms=123456))
        with self.assertRaisesRegex(MeasurementFailure, "rows differ from raw events"):
            verify_bundle(self.bundle)

    def test_build_attestation_must_match_clean_equivalent_baselines(self):
        self.edit("build-record.json", lambda build: build.update(dirty=True))
        with self.assertRaisesRegex(MeasurementFailure, "clean-state"):
            verify_bundle(self.bundle)

    def test_untimed_observations_must_match_report(self):
        self.edit("observations.json.gz", lambda observations: observations[0].update(trace_stderr=""))
        with self.assertRaisesRegex(MeasurementFailure, "untimed observations"):
            verify_bundle(self.bundle)

    def test_retained_native_output_must_match_semantic_digest(self):
        self.edit("observations.json.gz", lambda observations: next(
            o for o in observations if o["native_stdout"] is not None).update(native_stdout="corrupted"))
        with self.assertRaisesRegex(MeasurementFailure, "native output differs"):
            verify_bundle(self.bundle)


if __name__ == "__main__":
    unittest.main()

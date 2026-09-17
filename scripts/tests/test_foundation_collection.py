from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from cleanup_measurements.foundation import collect_compiles, collect_native
from cleanup_measurements.manifest import DEFAULT_MANIFEST, input_identity, load_manifest
from cleanup_measurements.provenance import compiler_identity
from measurement_support import MeasurementFailure, ProcessUsage, REPOSITORY, sha256_bytes
from test_foundation_metrics import ASSEMBLY


class FoundationCollectionTests(unittest.TestCase):
    def setUp(self):
        self.manifest, self.workloads, self.expected = load_manifest(DEFAULT_MANIFEST)
        self.compilers = {"baseline": Path("old"), "candidate": Path("new")}

    def test_frozen_manifest_has_required_families_inputs_and_independent_expectations(self):
        self.assertEqual(len(self.workloads), 21)
        self.assertEqual(sum(w.identity.startswith("compile/") for w in self.workloads), 4)
        scalar = self.expected["native/scalar-calls-omitted"]
        self.assertEqual(scalar["stdout_sha256"], sha256_bytes(b"-993156\n"))
        files = input_identity(self.manifest, self.workloads, REPOSITORY / "std")
        self.assertTrue(any(f["path"] == "std/std/test.ska" for f in files))
        self.assertTrue(all(len(f["sha256"]) == 64 for f in files))

    def test_changed_policy_or_duplicate_workload_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "manifest.json"
            manifest = json.loads(DEFAULT_MANIFEST.read_text())
            manifest["policy"]["timing_increase"] = .20
            path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(MeasurementFailure, "policy"):
                load_manifest(path)
            manifest = json.loads(DEFAULT_MANIFEST.read_text())
            manifest["workloads"].append(manifest["workloads"][0])
            path.write_text(json.dumps(manifest))
            with self.assertRaisesRegex(MeasurementFailure, "duplicate"):
                load_manifest(path)

    def test_binary_revision_is_attested_not_assigned_from_current_checkout(self):
        with tempfile.TemporaryDirectory() as temporary:
            binary = Path(temporary) / "skac"
            binary.write_bytes(b"preserved binary")
            with patch("cleanup_measurements.provenance.run_checked") as command:
                command.return_value.stdout = b"preserved-revision\n"
                identity = compiler_identity(binary, "old-tag", "golden", "rustc-old", True, 1)
                self.assertEqual(command.call_args.args[0][-1], "old-tag^{commit}")
                self.assertEqual(identity["revision"], "preserved-revision")
                self.assertEqual(identity["executable_sha256"], sha256_bytes(b"preserved binary"))
                self.assertTrue(identity["dirty"])

    def measured(self, command, **kwargs):
        Path(command[command.index("-o") + 1]).write_text(ASSEMBLY)
        return ProcessUsage(subprocess.CompletedProcess(command, 0, b"", b""), 100, 200)

    def observed(self, command, **kwargs):
        Path(command[command.index("-o") + 1]).write_text(ASSEMBLY)
        return subprocess.CompletedProcess(command, 0, b"", b"trace observation")

    def test_compiles_alternate_builds_exclude_warmups_and_keep_raw_events(self):
        with tempfile.TemporaryDirectory() as temporary, patch(
            "cleanup_measurements.foundation.measured_process", side_effect=self.measured
        ) as measured, patch("cleanup_measurements.foundation.run_checked", side_effect=self.observed):
            events = []
            entries = collect_compiles(self.compilers, self.workloads[0], Path(temporary),
                                       {"CC": "selected"}, 1, 5, 10, 0, events)
            self.assertEqual([e["role"] for e in events[:6]],
                             ["baseline", "candidate", "candidate", "baseline", "baseline", "candidate"])
            self.assertEqual(len(entries["baseline"]["compile_samples"]), 5)
            self.assertEqual(len(events), 12)
            self.assertEqual(measured.call_args.kwargs["env"], {"CC": "selected"})
            self.assertEqual(entries["candidate"]["compiler_rss"]["median"], 200)
            self.assertTrue(entries["baseline"]["reporting_equivalent"])

    def test_nondeterministic_or_reporting_changed_assembly_is_rejected(self):
        for reporting in (False, True):
            calls = 0
            def changed(command, **kwargs):
                nonlocal calls
                result = self.measured(command, **kwargs)
                calls += 1
                if calls > 1 or reporting:
                    Path(command[command.index("-o") + 1]).write_text(ASSEMBLY + "\n# changed")
                return result.completed if reporting else result
            with tempfile.TemporaryDirectory() as temporary, patch(
                "cleanup_measurements.foundation.measured_process",
                side_effect=self.measured if reporting else changed
            ), patch("cleanup_measurements.foundation.run_checked", side_effect=changed):
                with self.assertRaisesRegex(MeasurementFailure, "reporting|nondeterministic"):
                    collect_compiles({"baseline": Path("old")}, self.workloads[0], Path(temporary),
                                     {}, 1, 5, 10, 0, [])

    def test_native_alternation_raw_samples_and_manifest_divergence(self):
        workload = next(w for w in self.workloads if w.identity == "native/scalar-calls-omitted")
        expected = self.expected[workload.identity]
        def link(command, **kwargs):
            Path(command[command.index("-o") + 1]).write_bytes(b"executable")
        for stdout in (b"-993156\n", b"wrong result\n"):
            with tempfile.TemporaryDirectory() as temporary, patch(
                "cleanup_measurements.foundation.run_checked", side_effect=link
            ), patch("cleanup_measurements.foundation.native_text_size", return_value={"status": "supported", "bytes": 40}), patch(
                "cleanup_measurements.foundation.timed_process",
                return_value=(subprocess.CompletedProcess([], 0, stdout, b""), 100)
            ):
                events = []
                entries = {role: {"native_samples_ms": []} for role in self.compilers}
                if stdout != b"-993156\n":
                    with self.assertRaisesRegex(MeasurementFailure, "semantics"):
                        collect_native(self.compilers, workload, Path(temporary), {}, entries, expected, 1, 9, 10, 0, events)
                else:
                    collect_native(self.compilers, workload, Path(temporary), {}, entries, expected, 1, 9, 10, 0, events)
                    self.assertEqual([e["role"] for e in events[:4]],
                                     ["baseline", "candidate", "candidate", "baseline"])
                    self.assertEqual(entries["baseline"]["native_samples_ms"], [100] * 9)


if __name__ == "__main__":
    unittest.main()

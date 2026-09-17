#!/usr/bin/env python3
"""Verify retained foundation evidence and replay its comparison without timing."""

from __future__ import annotations

import argparse
import gzip
import json
from pathlib import Path

from cleanup_measurements.comparison import compare_runs
from cleanup_measurements.manifest import POLICY
from cleanup_measurements.observations import parse_analysis_usage, parse_pass_occurrences
from measurement_support import MeasurementFailure, alternating_order, sha256_bytes


EVIDENCE_FILES = {"manifest.json", "build-record.json", "first.json.gz", "second.json.gz",
                  "comparison.json", "observations.json.gz"}


def read_json(path: Path) -> dict | list:
    contents = gzip.decompress(path.read_bytes()) if path.suffix == ".gz" else path.read_bytes()
    return json.loads(contents)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise MeasurementFailure(message)


def verify_events(report: dict) -> None:
    """Check that raw rows correspond to the retained alternating execution log."""
    configuration = report["compatibility"]["configuration"]
    indexes = {role: {entry["id"]: entry for entry in variant["workloads"]}
               for role, variant in report["variants"].items()}
    expected_events = []
    for index, spec in enumerate(report["compatibility"]["workloads"]):
        for phase in ("compile", "native") if spec["native"] else ("compile",):
            warmups, repeats = configuration[f"{phase}_warmups"], configuration[f"{phase}_repeats"]
            for iteration in range(warmups + repeats):
                for role in alternating_order(("baseline", "candidate"),
                        iteration + index + report["provenance"]["order_start"]):
                    expected_events.append((spec["id"], phase, role, iteration, iteration < warmups))
    events = report["events"]
    actual_events = [(e["workload"], e["phase"], e["role"], e["iteration"], e["warmup"])
                     for e in events]
    require(actual_events == expected_events, "raw execution order/counts differ from protocol")
    for role, entries in indexes.items():
        for identity, entry in entries.items():
            measured = [e for e in events if e["role"] == role and e["workload"] == identity
                        and not e["warmup"]]
            require(entry["compile_samples"] == [
                {"wall_ms": e["wall_ms"], "peak_rss_kib": e["peak_rss_kib"]}
                for e in measured if e["phase"] == "compile"], "compile rows differ from raw events")
            require(entry["native_samples_ms"] == [e["wall_ms"] for e in measured
                if e["phase"] == "native"], "native rows differ from raw events")


def verify_bundle(directory: Path) -> dict:
    index = json.loads((directory / "index.json").read_text())
    require(index.get("schema") == 1 and index.get("kind") == "foundation-baseline-evidence",
            "unsupported evidence index")
    files = index["files"]
    require(set(files) == EVIDENCE_FILES, "evidence files are missing or unexpected")
    for name, expected in files.items():
        require(sha256_bytes((directory / name).read_bytes()) == expected, f"hash mismatch: {name}")
    manifest = json.loads((directory / "manifest.json").read_text())
    require(manifest.get("schema") == 1 and manifest.get("version") == 1
            and manifest.get("policy") == POLICY, "unsupported retained manifest")
    build = json.loads((directory / "build-record.json").read_text())
    first, second = [read_json(directory / name) for name in ("first.json.gz", "second.json.gz")]
    stored = json.loads((directory / "comparison.json").read_text())
    observations = read_json(directory / "observations.json.gz")
    result = compare_runs(first, second)
    require(result == {key: value for key, value in stored.items() if key != "captures"},
            "stored comparison does not reproduce from raw samples")
    require(result["outcome"] in ("within-review-limits", "inconclusive"),
            "baseline has incompatible, invalid or repeatable regressing evidence")
    require(build["revision"] == index["revision"] and build["dirty"] is False,
            "baseline build revision/clean-state attestation differs")
    input_hashes = {entry["path"]: entry["sha256"] for entry in first["compatibility"]["inputs"]}
    specs = []
    for workload in manifest["workloads"]:
        observation = workload["expected"]
        stdout_hash = (input_hashes[observation["stdout_file"]] if "stdout_file" in observation
                       else sha256_bytes(observation["stdout"].encode()))
        specs.append({"id": workload["id"], "native": workload["native_group"] is not None,
                      "expected": {"status": observation["status"], "stdout_sha256": stdout_hash,
                                   "stderr_sha256": sha256_bytes(observation["stderr"].encode())}})
    for row in result["workloads"]:
        require(all(frame["outcome"] == "recorded" for frame in row["frames"]),
                "missing/unsupported per-callable baseline metrics")
        require(all(not frame["changes"] for frame in row["frames"]),
                "equivalent baseline builds have different frame metrics")
        require(all("baseline" in pair and "candidate" in pair
                    for metric in row["metrics"].values() for pair in metric["pairs"]),
                "missing/invalid raw baseline metrics")
    for report in (first, second):
        require(report["qualification"] == "full", "baseline is a smoke/subset capture")
        compatibility = report["compatibility"]
        require(all(compatibility["configuration"][field] == manifest[field]
                    for field in ("target", "mir_profile", "compiler_profile")),
                "capture modes differ from retained manifest")
        require(compatibility["manifest_sha256"] == files["manifest.json"]
                and compatibility["workloads"] == specs, "capture differs from retained full manifest")
        require(compatibility["runtime"]["archive_sha256"] == build["runtime"]["sha256"],
                "runtime differs from build record")
        for variant in report["variants"].values():
            compiler = variant["compiler"]
            require(compiler["revision"] == build["revision"] and compiler["dirty"] is False
                    and compiler["executable_sha256"] == build["compiler"]["sha256"]
                    and all(compiler[field] == build[field] for field in
                            ("profile", "build_toolchain", "build_flags")),
                    "equivalent baseline compiler differs from build record")
            for entry in variant["workloads"]:
                require(entry["frames"]["status"] == "supported", "unsupported baseline frame metric")
                if entry["native_semantics"] is not None:
                    require(entry["native_text"]["status"] == "supported", "unsupported baseline text metric")
        verify_events(report)
    observation_index = {(o["capture"], o["workload"], o["role"]): o for o in observations}
    require(len(observation_index) == len(observations) == len(specs) * 4,
            "missing/duplicate untimed observations")
    for capture, report in (("first", first), ("second", second)):
        for role, variant in report["variants"].items():
            for entry in variant["workloads"]:
                observation = observation_index[(capture, entry["id"], role)]
                stderr = observation["trace_stderr"].encode()
                require(entry["observations"] == {"passes": parse_pass_occurrences(stderr),
                    "analysis": parse_analysis_usage(stderr)}, "untimed observations differ from report")
                if entry["native_semantics"] is not None:
                    require(all(sha256_bytes(observation[f"native_{stream}"].encode())
                        == entry["native_semantics"][f"{stream}_sha256"] for stream in ("stdout", "stderr")),
                        "retained native output differs from semantic digest")
    reference = first["variants"]["baseline"]
    for report in (first, second):
        for variant in report["variants"].values():
            require(variant["schedule"] == reference["schedule"], "baseline MIR schedule differs")
            for original, entry in zip(reference["workloads"], variant["workloads"]):
                require(all(entry.get(field) == original.get(field) for field in (
                    "id", "assembly_sha256", "assembly_bytes", "frames", "native_text", "executable_sha256")),
                    "equivalent baseline emitted different artifacts/static metrics")
    require({first["provenance"]["order_start"], second["provenance"]["order_start"]} == {0, 1},
            "independent captures must reverse their starting order")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    try:
        result = verify_bundle(args.directory)
    except (OSError, ValueError, KeyError, TypeError, MeasurementFailure) as error:
        parser.exit(1, f"baseline evidence verification failed: {error}\n")
    print(f"baseline evidence verified; comparison: {result['outcome']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

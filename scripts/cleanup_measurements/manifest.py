"""Versioned foundation inputs and independent expected native observations."""

import json
from pathlib import Path

from measurement_support import REPOSITORY, MeasurementFailure, sha256_bytes, display_path
from .workloads import Workload


DEFAULT_MANIFEST = REPOSITORY / "tests/benchmarks/low_level_compiler/manifest.json"
POLICY = {"compile_repeats": 5, "native_repeats": 9, "warmups": 1,
          "timing_increase": 0.10, "rss_text_increase": 0.15, "mad_multiplier": 2}


def repository_input(value: str) -> Path:
    path = (REPOSITORY / value).resolve()
    if not path.is_relative_to(REPOSITORY) or not path.exists():
        raise MeasurementFailure(f"missing or non-repository manifest input: {value}")
    return path


def load_manifest(path: Path) -> tuple[dict, tuple[Workload, ...], dict[str, dict]]:
    manifest = json.loads(path.read_text(encoding="utf-8"))
    if (manifest.get("schema") != 1 or manifest.get("version") != 1
            or manifest.get("target") != "x86_64-sysv" or manifest.get("compiler_profile") != "golden"
            or manifest.get("mir_profile") != "default" or manifest.get("policy") != POLICY):
        raise MeasurementFailure("unsupported foundation manifest or changed frozen policy")
    selected = []
    expected = {}
    for entry in manifest["workloads"]:
        identity = entry["id"]
        if identity in expected or not entry["input_paths"]:
            raise MeasurementFailure(f"duplicate or empty manifest workload: {identity}")
        selected.append(Workload(identity, tuple(entry["dimensions"]),
            tuple(entry["compiler_arguments"]),
            tuple(repository_input(p) for p in entry["input_paths"]),
            entry["runtime_trace"], entry["native_group"], tuple(entry["native_arguments"])))
        observation = entry["expected"]
        stdout = (repository_input(observation["stdout_file"]).read_bytes()
                  if "stdout_file" in observation else observation["stdout"].encode())
        expected[identity] = {"status": observation["status"],
                             "stdout_sha256": sha256_bytes(stdout),
                             "stderr_sha256": sha256_bytes(observation["stderr"].encode())}
    if not selected:
        raise MeasurementFailure("empty foundation manifest")
    return manifest, tuple(selected), expected


def input_identity(manifest: dict, selected: tuple[Workload, ...], stdlib: Path) -> list[dict]:
    paths = set(stdlib.glob("**/*.ska"))
    for workload in selected:
        for path in workload.input_paths:
            paths.update(path.glob("**/*.ska") if path.is_dir() else [path])
    for entry in manifest["workloads"]:
        if "stdout_file" in entry["expected"]:
            paths.add(repository_input(entry["expected"]["stdout_file"]))
    return [{"path": display_path(path.resolve()), "sha256": sha256_bytes(path.read_bytes()),
             "bytes": path.stat().st_size} for path in sorted(paths)]

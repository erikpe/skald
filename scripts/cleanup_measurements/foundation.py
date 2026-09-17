"""Paired foundation collection using the existing cleanup corpus/process helpers."""

import argparse
import datetime
import json
import os
from pathlib import Path

from measurement_support import (
    REPOSITORY, MeasurementFailure, alternating_order, display_path, measured_process,
    numeric_summary, repository_identity, resolve_repository_path, run_checked, runtime_artifact_identity,
    sha256_bytes, timed_process, timing_summary, unique_run_directory,
)
from .manifest import DEFAULT_MANIFEST, POLICY, input_identity, load_manifest
from .metrics import assembly_metrics, native_text_size
from .observations import parse_analysis_usage, parse_pass_occurrences, resolved_schedule
from .provenance import compiler_identity, host_identity


def semantic_result(completed) -> dict:
    return {"status": completed.returncode, "stdout_sha256": sha256_bytes(completed.stdout),
            "stderr_sha256": sha256_bytes(completed.stderr)}


def compiler_command(compiler: Path, workload) -> list:
    return [compiler, *workload.compiler_arguments, "--target", "x86_64-sysv",
            "--mir-optimization", "default"]


def collect_compiles(compilers: dict[str, Path], workload, directory: Path, env: dict,
                     warmups: int, repeats: int, timeout: float, order_start: int,
                     events: list[dict]) -> dict[str, dict]:
    entries = {role: {"compile_samples": [], "native_samples_ms": []} for role in compilers}
    assemblies = {}
    for iteration in range(warmups + repeats):
        for role in alternating_order(tuple(compilers), iteration + order_start):
            output = directory / f"{role}-{iteration}.s"
            usage = measured_process([*compiler_command(compilers[role], workload),
                "--emit", "asm", "-o", output], operation=f"{workload.identity}/{role} compile",
                run_directory=directory, timeout_seconds=timeout, env=env)
            value = output.read_bytes()
            if role in assemblies and value != assemblies[role]:
                raise MeasurementFailure(f"{workload.identity}/{role} nondeterministic assembly")
            assemblies[role] = value
            event = {"workload": workload.identity, "phase": "compile", "role": role,
                     "iteration": iteration, "warmup": iteration < warmups,
                     "wall_ms": usage.wall_ms, "peak_rss_kib": usage.peak_rss_kib}
            events.append(event)
            if iteration >= warmups:
                entries[role]["compile_samples"].append({
                    "wall_ms": usage.wall_ms, "peak_rss_kib": usage.peak_rss_kib})
    for role, compiler in compilers.items():
        trace = directory / f"{role}-trace.s"
        observation = run_checked([*compiler_command(compiler, workload), "--emit", "asm",
            "--report-level", "trace", "-o", trace], env=env, timeout_seconds=timeout,
            operation=f"{workload.identity}/{role} untimed observation")
        (directory / f"{role}-trace.stderr").write_bytes(observation.stderr)
        if trace.read_bytes() != assemblies[role]:
            raise MeasurementFailure(f"{workload.identity}/{role} reporting changed assembly")
        samples = entries[role]["compile_samples"]
        entries[role].update({"id": workload.identity,
            "assembly_sha256": sha256_bytes(assemblies[role]),
            "assembly_bytes": len(assemblies[role]), "reporting_equivalent": True,
            "frames": assembly_metrics(assemblies[role].decode()),
            "observations": {"passes": parse_pass_occurrences(observation.stderr),
                             "analysis": parse_analysis_usage(observation.stderr)},
            "compiler_wall": timing_summary([s["wall_ms"] for s in samples]),
            "compiler_rss": numeric_summary([s["peak_rss_kib"] for s in samples]),
            "native_semantics": None, "native_text": None, "native_wall": None})
    return entries


def collect_native(compilers: dict[str, Path], workload, directory: Path, env: dict,
                   entries: dict[str, dict], expected: dict, warmups: int, repeats: int,
                   timeout: float, order_start: int, events: list[dict]) -> None:
    executables = {}
    for role, compiler in compilers.items():
        executable = directory / role
        run_checked([*compiler_command(compiler, workload), "-o", executable],
                    env=env, timeout_seconds=timeout, operation=f"{workload.identity}/{role} link")
        entries[role]["native_text"] = native_text_size(executable, timeout)
        entries[role]["executable_sha256"] = sha256_bytes(executable.read_bytes())
        executables[role] = executable
    for iteration in range(warmups + repeats):
        for role in alternating_order(tuple(compilers), iteration + order_start):
            result, wall = timed_process([executables[role], *workload.native_arguments],
                env=env, timeout_seconds=timeout, operation=f"{workload.identity}/{role} native",
                expected_statuses=(expected["status"],))
            if semantic_result(result) != expected:
                raise MeasurementFailure(f"{workload.identity}/{role} diverged from manifest semantics")
            (directory / f"{role}.stdout").write_bytes(result.stdout)
            (directory / f"{role}.stderr").write_bytes(result.stderr)
            entries[role]["native_semantics"] = expected
            events.append({"workload": workload.identity, "phase": "native", "role": role,
                           "iteration": iteration, "warmup": iteration < warmups, "wall_ms": wall})
            if iteration >= warmups:
                entries[role]["native_samples_ms"].append(wall)
    for entry in entries.values():
        entry["native_wall"] = timing_summary(entry["native_samples_ms"])


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default="target/golden/skac")
    parser.add_argument("--compiler-profile", default="golden")
    parser.add_argument("--compiler-revision", required=True)
    parser.add_argument("--candidate-compiler")
    parser.add_argument("--candidate-revision")
    parser.add_argument("--compiler-dirty", action="store_true")
    parser.add_argument("--candidate-dirty", action="store_true")
    parser.add_argument("--build-toolchain", required=True,
                        help="Attested rustc version used to build BOTH binaries")
    parser.add_argument("--build-flags", default="",
                        help="Attested extra Cargo/Rust build flags for BOTH binaries; empty means none")
    parser.add_argument("--host-notes", required=True,
                        help="Record CPU policy, load control and other controlled conditions")
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--workload", action="append")
    parser.add_argument("--compile-repeats", type=int, default=5)
    parser.add_argument("--native-repeats", type=int, default=9)
    parser.add_argument("--compile-warmups", type=int, default=1)
    parser.add_argument("--native-warmups", type=int, default=1)
    parser.add_argument("--order-start", type=int, choices=(0, 1), default=0)
    parser.add_argument("--timeout", type=float, default=30.0)
    parser.add_argument("--smoke", action="store_true", help="Nonqualifying harness check")
    parser.add_argument("--output-root", type=Path,
                        default=REPOSITORY / "build/measurements/low-level-compiler")
    args = parser.parse_args(argv)
    if bool(args.candidate_compiler) != bool(args.candidate_revision):
        parser.error("candidate binary and attested revision must be supplied together")
    minimum_compile, minimum_native = (2, 1) if args.smoke else (5, 9)
    if (args.compile_repeats < minimum_compile or args.native_repeats < minimum_native
            or min(args.compile_warmups, args.native_warmups) < 1 or args.timeout <= 0):
        parser.error("insufficient repetitions/warmups or nonpositive timeout")
    manifest, available, expected = load_manifest(args.manifest)
    if args.compiler_profile != manifest["compiler_profile"]:
        parser.error("compiler profile differs from the frozen manifest")
    requested = set(args.workload or ())
    if requested.difference(w.identity for w in available):
        parser.error("unknown foundation workload")
    selected = tuple(w for w in available if not requested or w.identity in requested)
    compilers = {"baseline": resolve_repository_path(args.compiler)}
    identities = {"baseline": compiler_identity(compilers["baseline"], args.compiler_revision,
        manifest["compiler_profile"], args.build_toolchain, args.compiler_dirty, args.timeout,
        build_flags=args.build_flags)}
    if args.candidate_compiler:
        compilers["candidate"] = resolve_repository_path(args.candidate_compiler)
        identities["candidate"] = compiler_identity(compilers["candidate"], args.candidate_revision,
            manifest["compiler_profile"], args.build_toolchain, args.candidate_dirty, args.timeout,
            build_flags=args.build_flags)
    runtime = runtime_artifact_identity()
    stdlib = resolve_repository_path(os.environ.get("SKALD_STDLIB_ROOT", "std"))
    if not stdlib.is_dir() or not list(stdlib.glob("**/*.ska")):
        parser.error("standard library root is absent or empty")
    host = host_identity(args.host_notes, args.timeout)
    env = dict(os.environ, CC=host["tools"]["cc"]["path"],
               SKALD_RUNTIME_ARCHIVE=str(resolve_repository_path(runtime["archive"])),
               SKALD_STDLIB_ROOT=str(stdlib))
    inputs = input_identity(manifest, selected, stdlib)
    harness_paths = [Path(__file__).resolve().parents[1] / name for name in
                     ("measure_cleanup_baseline.py", "measurement_support.py")]
    harness_paths.extend(sorted(Path(__file__).parent.glob("*.py")))
    harness = [{"path": display_path(path), "sha256": sha256_bytes(path.read_bytes())}
               for path in harness_paths]
    directory = unique_run_directory(args.output_root.resolve(), "pair")
    (directory / "manifest.json").write_bytes(args.manifest.read_bytes())
    events = []
    variants = {role: {"compiler": identity, "workloads": []} for role, identity in identities.items()}
    compatibility = {"manifest_sha256": sha256_bytes(args.manifest.read_bytes()),
        "inputs": inputs, "runtime": runtime, "host": host, "harness": harness,
        "configuration": {"target": manifest["target"], "mir_profile": manifest["mir_profile"],
            "compiler_profile": manifest["compiler_profile"], "mir_exclusions": [],
            "compile_repeats": args.compile_repeats, "native_repeats": args.native_repeats,
            "compile_warmups": args.compile_warmups, "native_warmups": args.native_warmups,
            "timeout": args.timeout},
        "workloads": [{"id": w.identity, "native": w.native_group is not None,
                       "expected": expected[w.identity]} for w in selected]}
    report = {"schema": 1, "kind": "foundation", "run_id": directory.name,
        "qualification": "smoke" if args.smoke else ("full" if selected == available else "subset"),
        "policy": POLICY, "compatibility": compatibility, "variants": variants,
        "provenance": {"checkout": repository_identity(args.timeout),
            "compiler_paths": {role: str(path) for role, path in compilers.items()},
            "utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "order_start": args.order_start, "run_directory": display_path(directory),
            "load_start": os.getloadavg()}, "events": events}
    try:
        for role, compiler in compilers.items():
            schedule_directory = directory / role
            schedule_directory.mkdir()
            variants[role]["schedule"] = resolved_schedule(compiler, schedule_directory, args.timeout,
                env=env, compiler_arguments=("--target", "x86_64-sysv", "--mir-optimization", "default"))
        for index, workload in enumerate(selected):
            workdir = directory / workload.identity
            workdir.mkdir(parents=True)
            entries = collect_compiles(compilers, workload, workdir, env, args.compile_warmups,
                args.compile_repeats, args.timeout, args.order_start + index, events)
            if workload.native_group is not None:
                collect_native(compilers, workload, workdir, env, entries, expected[workload.identity],
                    args.native_warmups, args.native_repeats, args.timeout, args.order_start + index, events)
            for role, entry in entries.items():
                variants[role]["workloads"].append(entry)
        if (inputs != input_identity(manifest, selected, stdlib)
                or runtime != runtime_artifact_identity()
                or compatibility["manifest_sha256"] != sha256_bytes(args.manifest.read_bytes())):
            raise MeasurementFailure("source/runtime inputs changed during collection")
        for role, path in compilers.items():
            if sha256_bytes(path.read_bytes()) != identities[role]["executable_sha256"]:
                raise MeasurementFailure("compiler binary changed during collection")
        if host != host_identity(args.host_notes, args.timeout):
            raise MeasurementFailure("host policy/toolchain changed during collection")
        if any(entry["sha256"] != sha256_bytes(resolve_repository_path(entry["path"]).read_bytes())
               for entry in harness):
            raise MeasurementFailure("measurement harness changed during collection")
    except (OSError, MeasurementFailure) as error:
        report["failure"] = str(error)
        (directory / "failure.json").write_text(json.dumps(report, indent=2) + "\n")
        raise
    report["provenance"]["load_end"] = os.getloadavg()
    (directory / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"foundation {report['qualification']}: {directory / 'report.json'}")
    return 0

#!/usr/bin/env python3
"""Capture deterministic and operational cleanup baselines over representative workloads."""

from __future__ import annotations

import argparse
import json
import re
from dataclasses import asdict, dataclass
from pathlib import Path

from measurement_support import (
    REPOSITORY,
    MeasurementFailure,
    alternating_order,
    deterministic_projection,
    measured_process,
    repository_identity,
    resolve_repository_path,
    run_checked,
    sha256_bytes,
    source_inventory,
    timed_process,
    timing_summary,
    unique_run_directory,
)


DEFAULT_OUTPUT_ROOT = REPOSITORY / "build/measurements/cleanup-baseline"
PASS_PATTERN = re.compile(
    rb"^skac: trace: (proof-rich|proof-transition|final) MIR pass `([^`]+)` "
    rb"\(pass identity \d+, schedule position (\d+), occurrence (\d+)\) ",
    re.MULTILINE,
)
ANALYSIS_PATTERN = re.compile(
    rb"^skac: trace analysis: ([^:]+): requests (\d+), computations (\d+), hits (\d+), "
    rb"repeated snapshot requests (\d+), results before (\d+), inserted (\d+), discarded (\d+)$"
)


@dataclass(frozen=True)
class Workload:
    identity: str
    dimensions: tuple[str, ...]
    compiler_arguments: tuple[str, ...]
    input_paths: tuple[Path, ...]
    runtime_trace: str
    native_group: str | None = None
    native_arguments: tuple[str, ...] = ()


def direct_workload(
    identity: str,
    dimensions: tuple[str, ...],
    source: str,
    *,
    compiler_arguments: tuple[str, ...] = (),
    runtime_trace: str = "enabled",
    native_group: str | None = None,
    native_arguments: tuple[str, ...] = (),
) -> Workload:
    source_path = REPOSITORY / source
    trace_arguments = ("--omit-runtime-trace",) if runtime_trace == "omitted" else ()
    return Workload(
        identity,
        dimensions,
        (source, *compiler_arguments, *trace_arguments),
        (source_path,),
        runtime_trace,
        native_group,
        native_arguments,
    )


def workloads() -> tuple[Workload, ...]:
    module_root = REPOSITORY / "tests/golden/vm_benchmark/cases/modules"
    compile_baselines = (
        direct_workload(
            "compile/small-source",
            ("small-source",),
            "samples/vertical/exit_42.ska",
            compiler_arguments=("--no-stdlib",),
            runtime_trace="omitted",
        ),
        Workload(
            "compile/many-modules-large-cfg",
            ("many-modules", "large-callable-cfg"),
            ("--entry", "app", "--module-root", str(module_root.relative_to(REPOSITORY))),
            tuple(sorted(module_root.glob("**/*.ska"))),
            "enabled",
        ),
        direct_workload(
            "compile/many-generic-applications",
            ("many-generic-applications",),
            "tests/golden/generic_interfaces/conformance_matrix.ska",
            compiler_arguments=("--no-stdlib",),
            runtime_trace="omitted",
        ),
        direct_workload(
            "compile/nested-ownership",
            ("nested-ownership",),
            "tests/golden/standard_vec/vec_generic_type_shapes.ska",
            runtime_trace="omitted",
        ),
    )
    native = [
        direct_workload(
            "native/generic-vector-growth",
            ("existing-native", "generic-applications", "ownership"),
            "tests/benchmarks/generic_vec/growth.ska",
            native_group="generic-vector",
        )
    ]
    for integer in ("u8", "u64", "i64"):
        for form in ("range", "while"):
            native.append(
                direct_workload(
                    f"native/range-{integer}-{form}",
                    ("existing-native", "loop"),
                    f"tests/benchmarks/range_loop/{integer}_{form}.ska",
                    runtime_trace="omitted",
                    native_group=f"range-{integer}",
                )
            )
    trace_sources = (
        ("call-recursion", "tests/benchmarks/panic_runtime_trace/call_recursion.ska"),
        ("tight-loop", "tests/benchmarks/panic_runtime_trace/tight_loop.ska"),
        ("allocation", "tests/benchmarks/panic_runtime_trace/allocation.ska"),
        ("representative-golden", "tests/golden/operators/primitive_operator_profile.ska"),
    )
    for name, source in trace_sources:
        for policy in ("enabled", "omitted"):
            native.append(
                direct_workload(
                    f"native/runtime-trace-{name}-{policy}",
                    ("existing-native", "runtime-trace"),
                    source,
                    runtime_trace=policy,
                    native_group=f"runtime-trace-{name}",
                )
            )
    return (*compile_baselines, *native)


def resolved_schedule(
    compiler: Path, run_directory: Path, timeout_seconds: float
) -> list[dict[str, object]]:
    output = run_directory / "schedule.s"
    completed = run_checked(
        [
            compiler,
            "samples/vertical/exit_42.ska",
            "--no-stdlib",
            "--omit-runtime-trace",
            "--emit",
            "asm",
            "--report-level",
            "trace",
            "-o",
            output,
        ],
        operation="default MIR schedule inspection",
        timeout_seconds=timeout_seconds,
    )
    schedule = [
        {
            "position": int(position),
            "pass": name.decode("utf-8"),
            "stage": stage.decode("ascii"),
            "occurrence": int(occurrence),
        }
        for stage, name, position, occurrence in PASS_PATTERN.findall(completed.stderr)
    ]
    positions = [entry["position"] for entry in schedule]
    if not schedule or positions != list(range(len(schedule))):
        raise MeasurementFailure(
            "compiler trace did not contain one contiguous resolved MIR schedule"
        )
    return schedule


def parse_analysis_usage(stderr: bytes) -> list[dict[str, object]]:
    current: dict[str, object] | None = None
    usage: list[dict[str, object]] = []
    for line in stderr.splitlines():
        if match := PASS_PATTERN.match(line):
            stage, name, position, occurrence = match.groups()
            current = {
                "position": int(position),
                "pass": name.decode("utf-8"),
                "stage": stage.decode("ascii"),
                "occurrence": int(occurrence),
            }
            continue
        match = ANALYSIS_PATTERN.match(line)
        if match is None:
            continue
        if current is None:
            raise MeasurementFailure("analysis usage preceded its MIR pass occurrence")
        kind, requests, computations, hits, repeated, before, inserted, discarded = match.groups()
        usage.append(
            {
                **current,
                "analysis": kind.decode("ascii"),
                "requests": int(requests),
                "computations": int(computations),
                "hits": int(hits),
                "repeated_snapshot_requests": int(repeated),
                "distinct_callable_snapshot_keys": int(requests) - int(repeated),
                "results_before": int(before),
                "results_inserted": int(inserted),
                "results_discarded": int(discarded),
            }
        )
    return usage


def inspect_analysis_usage(
    compiler: Path,
    workload: Workload,
    run_directory: Path,
    timeout_seconds: float,
) -> list[dict[str, object]]:
    output = run_directory / f"{workload.identity.replace('/', '-')}-analysis.s"
    completed = run_checked(
        [
            compiler,
            *workload.compiler_arguments,
            "--emit",
            "asm",
            "--report-level",
            "trace",
            "-o",
            output,
        ],
        operation=f"{workload.identity} analysis-usage inspection",
        timeout_seconds=timeout_seconds,
    )
    return parse_analysis_usage(completed.stderr)


def numeric_summary(samples: list[int]) -> dict[str, int | float]:
    summary = timing_summary([float(sample) for sample in samples])
    return {
        "samples": summary["samples"],
        "median": summary["median_ms"],
        "median_absolute_deviation": summary["median_absolute_deviation_ms"],
        "min": summary["min_ms"],
        "max": summary["max_ms"],
    }


def compile_workload(
    compiler: Path,
    workload: Workload,
    run_directory: Path,
    repeats: int,
    timeout_seconds: float,
) -> tuple[dict[str, object], dict[str, object], Path | None]:
    analysis_usage = inspect_analysis_usage(
        compiler, workload, run_directory, timeout_seconds
    )
    assembly_values: list[bytes] = []
    compile_wall_ms: list[float] = []
    compile_peak_rss_kib: list[int] = []
    for repeat in range(repeats):
        assembly = run_directory / f"{workload.identity.replace('/', '-')}-{repeat}.s"
        usage = measured_process(
            [compiler, *workload.compiler_arguments, "--emit", "asm", "-o", assembly],
            operation=f"{workload.identity} assembly compilation",
            run_directory=run_directory,
            timeout_seconds=timeout_seconds,
        )
        assembly_values.append(assembly.read_bytes())
        compile_wall_ms.append(usage.wall_ms)
        compile_peak_rss_kib.append(usage.peak_rss_kib)
    if any(value != assembly_values[0] for value in assembly_values[1:]):
        raise MeasurementFailure(f"{workload.identity} emitted nondeterministic assembly")

    executable = None
    executable_bytes = None
    executable_build_usage = None
    if workload.native_group is not None:
        executable = run_directory / workload.identity.replace("/", "-")
        link = measured_process(
            [compiler, *workload.compiler_arguments, "-o", executable],
            operation=f"{workload.identity} executable compilation",
            run_directory=run_directory,
            timeout_seconds=timeout_seconds,
        )
        executable_bytes = executable.stat().st_size
        executable_build_usage = {
            "wall_ms": link.wall_ms,
            "peak_rss_kib": link.peak_rss_kib,
        }

    deterministic = {
        "id": workload.identity,
        "dimensions": list(workload.dimensions),
        "compiler_arguments": list(workload.compiler_arguments),
        "runtime_trace": workload.runtime_trace,
        "native_group": workload.native_group,
        "input": source_inventory(workload.input_paths),
        "artifacts": {
            "assembly_bytes": len(assembly_values[0]),
            "assembly_sha256": sha256_bytes(assembly_values[0]),
            "executable_bytes": executable_bytes,
        },
        "analysis_usage": analysis_usage,
    }
    operational = {
        "id": workload.identity,
        "compiler": {
            "wall": timing_summary(compile_wall_ms),
            "peak_rss_kib": numeric_summary(compile_peak_rss_kib),
        },
        "executable_build": executable_build_usage,
        "native": None,
    }
    return deterministic, operational, executable


def run_native_groups(
    selected: tuple[Workload, ...],
    executables: dict[str, Path],
    operational: dict[str, dict[str, object]],
    deterministic: dict[str, dict[str, object]],
    warmups: int,
    repeats: int,
    timeout_seconds: float,
) -> None:
    groups: dict[str, list[Workload]] = {}
    for workload in selected:
        if workload.native_group is not None:
            groups.setdefault(workload.native_group, []).append(workload)
    for group in groups.values():
        for iteration in range(warmups + repeats):
            identities = alternating_order([workload.identity for workload in group], iteration)
            for identity in identities:
                workload = next(item for item in group if item.identity == identity)
                completed, elapsed_ms = timed_process(
                    [executables[identity], *workload.native_arguments],
                    operation=f"{identity} native run",
                    timeout_seconds=timeout_seconds,
                )
                semantic = {
                    "status": completed.returncode,
                    "stdout_sha256": sha256_bytes(completed.stdout),
                    "stderr_sha256": sha256_bytes(completed.stderr),
                }
                prior = deterministic[identity].get("native_semantics")
                if prior is not None and prior != semantic:
                    raise MeasurementFailure(
                        f"{identity} native result changed between repetitions"
                    )
                deterministic[identity]["native_semantics"] = semantic
                if iteration >= warmups:
                    samples = operational[identity].setdefault("native_samples_ms", [])
                    assert isinstance(samples, list)
                    samples.append(elapsed_ms)
        reference = deterministic[group[0].identity]["native_semantics"]
        if any(
            deterministic[workload.identity]["native_semantics"] != reference
            for workload in group[1:]
        ):
            raise MeasurementFailure(
                f"native variants in {group[0].native_group} produced different results"
            )
    for workload in selected:
        if workload.native_group is None:
            continue
        entry = operational[workload.identity]
        samples = entry.pop("native_samples_ms")
        assert isinstance(samples, list)
        entry["native"] = timing_summary(samples)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default="target/golden/skac")
    parser.add_argument("--compiler-profile", default="golden")
    parser.add_argument("--compile-repeats", type=int, default=3)
    parser.add_argument("--native-warmups", type=int, default=1)
    parser.add_argument("--native-repeats", type=int, default=5)
    parser.add_argument("--timeout", type=float, default=30.0, dest="timeout_seconds")
    parser.add_argument("--output-root", type=Path, default=DEFAULT_OUTPUT_ROOT)
    parser.add_argument(
        "--workload", action="append", help="Limit the baseline to an exact workload ID"
    )
    parser.add_argument("--json", action="store_true", help="Print the complete report")
    arguments = parser.parse_args()
    if arguments.compile_repeats < 2:
        parser.error("--compile-repeats must be at least 2 to verify deterministic assembly")
    if arguments.native_warmups < 0 or arguments.native_repeats < 1:
        parser.error("native warmups must be nonnegative and repeats must be positive")
    if arguments.timeout_seconds <= 0:
        parser.error("--timeout must be positive")

    compiler = resolve_repository_path(arguments.compiler)
    if not compiler.is_file():
        parser.error(f"compiler does not exist: {compiler}")
    available = workloads()
    requested = set(arguments.workload or ())
    unknown = requested.difference(workload.identity for workload in available)
    if unknown:
        parser.error(f"unknown workload IDs: {', '.join(sorted(unknown))}")
    selected = tuple(
        workload for workload in available if not requested or workload.identity in requested
    )
    run_directory = unique_run_directory(arguments.output_root.resolve(), "run")
    schedule = resolved_schedule(compiler, run_directory, arguments.timeout_seconds)

    deterministic_workloads: dict[str, dict[str, object]] = {}
    operational_workloads: dict[str, dict[str, object]] = {}
    executables: dict[str, Path] = {}
    for workload in selected:
        deterministic, operational, executable = compile_workload(
            compiler,
            workload,
            run_directory,
            arguments.compile_repeats,
            arguments.timeout_seconds,
        )
        deterministic_workloads[workload.identity] = deterministic
        operational_workloads[workload.identity] = operational
        if executable is not None:
            executables[workload.identity] = executable
    run_native_groups(
        selected,
        executables,
        operational_workloads,
        deterministic_workloads,
        arguments.native_warmups,
        arguments.native_repeats,
        arguments.timeout_seconds,
    )

    deterministic = {
        "schema": 1,
        "compiler": {
            **repository_identity(arguments.timeout_seconds),
            "profile": arguments.compiler_profile,
        },
        "configuration": {
            "target": "x86_64-sysv",
            "mir_profile": "default",
            "mir_exclusions": [],
            "resolved_pass_schedule": schedule,
            "compile_repeats": arguments.compile_repeats,
            "native_warmups": arguments.native_warmups,
            "native_repeats": arguments.native_repeats,
            "timeout_seconds": arguments.timeout_seconds,
        },
        "workloads": [deterministic_workloads[workload.identity] for workload in selected],
    }
    report = {
        "deterministic": deterministic,
        "operational": {
            "run_directory": str(run_directory.relative_to(REPOSITORY)),
            "workloads": [operational_workloads[workload.identity] for workload in selected],
        },
    }
    (run_directory / "deterministic.json").write_text(
        json.dumps(deterministic_projection(report), indent=2) + "\n", encoding="utf-8"
    )
    (run_directory / "report.json").write_text(
        json.dumps(report, indent=2) + "\n", encoding="utf-8"
    )
    if arguments.json:
        print(json.dumps(report, indent=2))
    else:
        print(f"cleanup baseline: {len(selected)} workloads")
        print(f"compiler: {deterministic['compiler']['revision']} ({arguments.compiler_profile})")
        print(f"resolved MIR schedule: {len(schedule)} occurrences")
        print(f"report: {run_directory.relative_to(REPOSITORY) / 'report.json'}")
        print("timing is observational and has no pass threshold")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

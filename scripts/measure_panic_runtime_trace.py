#!/usr/bin/env python3
"""Compare enabled and omitted panic runtime-trace code and execution costs."""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import time
from dataclasses import asdict, dataclass
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
DEFAULT_BUILD_DIRECTORY = REPOSITORY / "build" / "measurements" / "panic-runtime-trace"


@dataclass(frozen=True)
class Workload:
    name: str
    source: Path


WORKLOADS = (
    Workload(
        "call_recursion",
        REPOSITORY / "tests/benchmarks/panic_runtime_trace/call_recursion.ska",
    ),
    Workload(
        "tight_loop",
        REPOSITORY / "tests/benchmarks/panic_runtime_trace/tight_loop.ska",
    ),
    Workload(
        "allocation",
        REPOSITORY / "tests/benchmarks/panic_runtime_trace/allocation.ska",
    ),
    Workload(
        "representative_golden",
        REPOSITORY / "tests/golden/operators/primitive_operator_profile.ska",
    ),
)


@dataclass(frozen=True)
class AssemblyMetrics:
    bytes: int
    instructions: int
    trace_pushes: int
    trace_pops: int
    trace_replacements: int
    trace_instructions: int
    trace_contexts: int
    trace_locations: int


@dataclass(frozen=True)
class RuntimeMetrics:
    repeats: int
    median_ms: float
    mean_ms: float
    min_ms: float
    max_ms: float


@dataclass(frozen=True)
class VariantMeasurement:
    policy: str
    assembly: AssemblyMetrics
    executable_bytes: int
    runtime: RuntimeMetrics


@dataclass(frozen=True)
class WorkloadMeasurement:
    workload: str
    source: str
    enabled: VariantMeasurement
    omitted: VariantMeasurement


def run_checked(arguments: list[str], *, capture: bool = True) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(
        arguments,
        cwd=REPOSITORY,
        stdout=subprocess.PIPE if capture else subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
    )
    if result.returncode != 0:
        stderr = result.stderr.decode("utf-8", errors="backslashreplace")
        raise RuntimeError(f"command failed with status {result.returncode}: {arguments!r}\n{stderr}")
    return result


def compile_variant(
    compiler: Path,
    workload: Workload,
    policy: str,
    build_directory: Path,
) -> tuple[Path, Path]:
    stem = build_directory / f"{workload.name}-{policy}"
    assembly = stem.with_suffix(".s")
    executable = stem
    policy_arguments = [] if policy == "enabled" else ["--omit-runtime-trace"]
    run_checked(
        [
            str(compiler),
            str(workload.source),
            "--emit",
            "asm",
            *policy_arguments,
            "-o",
            str(assembly),
        ]
    )
    run_checked(
        [
            str(compiler),
            str(workload.source),
            *policy_arguments,
            "-o",
            str(executable),
        ]
    )
    return assembly, executable


def instruction_lines(assembly: str) -> list[str]:
    in_text = False
    instructions: list[str] = []
    for line in assembly.splitlines():
        if line == ".text":
            in_text = True
            continue
        if line.startswith(".section "):
            in_text = False
            continue
        stripped = line.strip()
        if in_text and line.startswith("    ") and stripped and not stripped.startswith("."):
            instructions.append(stripped)
    return instructions


def count_trace_sequences(instructions: list[str]) -> tuple[int, int, int]:
    tls_load = "mov r11, qword ptr fs:ska_rt_trace_top@tpoff"
    tls_store = "mov qword ptr fs:ska_rt_trace_top@tpoff, r11"
    location_prefix = "lea r11, [rip + .Lska.trace.location."
    pushes = 0
    pops = 0
    location_pairs = 0

    for index, instruction in enumerate(instructions):
        if instruction.startswith(location_prefix):
            if index + 1 >= len(instructions) or not instructions[index + 1].startswith(
                "mov qword ptr [rbp - "
            ):
                raise RuntimeError("trace location load is not followed by its frame-home store")
            location_pairs += 1
        if instruction == tls_load:
            sequence = instructions[index : index + 6]
            if len(sequence) != 6 or sequence[5] != tls_store:
                raise RuntimeError("runtime-trace push is not the frozen six-instruction sequence")
            if not sequence[1].startswith("mov qword ptr [rbp - "):
                raise RuntimeError("runtime-trace push does not store the previous link")
            if not sequence[2].startswith(location_prefix):
                raise RuntimeError("runtime-trace push does not load its initial location")
            if not sequence[3].startswith("mov qword ptr [rbp - "):
                raise RuntimeError("runtime-trace push does not store its initial location")
            if not sequence[4].startswith("lea r11, [rbp - "):
                raise RuntimeError("runtime-trace push does not materialize its frame address")
            pushes += 1
        if instruction == tls_store and index > 0:
            previous = instructions[index - 1]
            if previous.startswith("mov r11, qword ptr [rbp - "):
                pops += 1

    replacements = location_pairs - pushes
    if replacements < 0:
        raise RuntimeError("runtime-trace sequence counts are inconsistent")
    return pushes, pops, replacements


def analyze_assembly(path: Path, policy: str) -> AssemblyMetrics:
    assembly = path.read_text(encoding="utf-8")
    instructions = instruction_lines(assembly)
    pushes, pops, replacements = count_trace_sequences(instructions)
    contexts = assembly.count(".size .Lska.trace.context.")
    locations = assembly.count(".size .Lska.trace.location.")
    if policy == "enabled":
        if pushes == 0 or pops < pushes or contexts != pushes:
            raise RuntimeError("enabled output has inconsistent trace activation counts")
    elif any((pushes, pops, replacements, contexts, locations)):
        raise RuntimeError("omitted output contains runtime-trace artifacts")
    if policy == "omitted" and "ska_rt_trace_top" in assembly:
        raise RuntimeError("omitted output references the runtime-trace TLS symbol")

    return AssemblyMetrics(
        bytes=path.stat().st_size,
        instructions=len(instructions),
        trace_pushes=pushes,
        trace_pops=pops,
        trace_replacements=replacements,
        trace_instructions=pushes * 6 + pops * 2 + replacements * 2,
        trace_contexts=contexts,
        trace_locations=locations,
    )


def timed_run(executable: Path) -> float:
    start = time.perf_counter_ns()
    run_checked([str(executable)], capture=False)
    return (time.perf_counter_ns() - start) / 1_000_000.0


def runtime_metrics(timings: list[float]) -> RuntimeMetrics:
    return RuntimeMetrics(
        repeats=len(timings),
        median_ms=statistics.median(timings),
        mean_ms=statistics.fmean(timings),
        min_ms=min(timings),
        max_ms=max(timings),
    )


def benchmark_pair(
    enabled: Path,
    omitted: Path,
    warmups: int,
    repeats: int,
) -> tuple[RuntimeMetrics, RuntimeMetrics]:
    for _ in range(warmups):
        run_checked([str(enabled)], capture=False)
        run_checked([str(omitted)], capture=False)
    timings = {"enabled": [], "omitted": []}
    variants = {"enabled": enabled, "omitted": omitted}
    for repeat in range(repeats):
        order = ("enabled", "omitted") if repeat % 2 == 0 else ("omitted", "enabled")
        for policy in order:
            timings[policy].append(timed_run(variants[policy]))
    return runtime_metrics(timings["enabled"]), runtime_metrics(timings["omitted"])


def measure_workload(
    compiler: Path,
    workload: Workload,
    build_directory: Path,
    warmups: int,
    repeats: int,
) -> WorkloadMeasurement:
    enabled_assembly, enabled_executable = compile_variant(
        compiler, workload, "enabled", build_directory
    )
    omitted_assembly, omitted_executable = compile_variant(
        compiler, workload, "omitted", build_directory
    )
    enabled_runtime, omitted_runtime = benchmark_pair(
        enabled_executable, omitted_executable, warmups, repeats
    )
    return WorkloadMeasurement(
        workload=workload.name,
        source=str(workload.source.relative_to(REPOSITORY)),
        enabled=VariantMeasurement(
            policy="enabled",
            assembly=analyze_assembly(enabled_assembly, "enabled"),
            executable_bytes=enabled_executable.stat().st_size,
            runtime=enabled_runtime,
        ),
        omitted=VariantMeasurement(
            policy="omitted",
            assembly=analyze_assembly(omitted_assembly, "omitted"),
            executable_bytes=omitted_executable.stat().st_size,
            runtime=omitted_runtime,
        ),
    )


def percentage_delta(enabled: float, omitted: float) -> float:
    return (enabled - omitted) * 100.0 / omitted


def print_table(measurements: list[WorkloadMeasurement]) -> None:
    header = (
        "workload",
        "policy",
        "asm_bytes",
        "instructions",
        "push/pop/replace",
        "trace_instr",
        "binary_bytes",
        "median_ms",
        "time_delta",
    )
    rows: list[tuple[str, ...]] = []
    for measurement in measurements:
        for variant in (measurement.enabled, measurement.omitted):
            delta = "-"
            if variant.policy == "enabled":
                delta = f"{percentage_delta(variant.runtime.median_ms, measurement.omitted.runtime.median_ms):+.1f}%"
            assembly = variant.assembly
            rows.append(
                (
                    measurement.workload,
                    variant.policy,
                    str(assembly.bytes),
                    str(assembly.instructions),
                    f"{assembly.trace_pushes}/{assembly.trace_pops}/{assembly.trace_replacements}",
                    str(assembly.trace_instructions),
                    str(variant.executable_bytes),
                    f"{variant.runtime.median_ms:.3f}",
                    delta,
                )
            )

    widths = [len(value) for value in header]
    for row in rows:
        for index, value in enumerate(row):
            widths[index] = max(widths[index], len(value))

    def render(row: tuple[str, ...]) -> str:
        return "  ".join(value.ljust(widths[index]) for index, value in enumerate(row))

    print(render(header))
    print("  ".join("-" * width for width in widths))
    for row in rows:
        print(render(row))


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--compiler",
        type=Path,
        default=REPOSITORY / "target/debug/skac",
        help="Path to the skac executable",
    )
    parser.add_argument(
        "--build-directory",
        type=Path,
        default=DEFAULT_BUILD_DIRECTORY,
        help="Directory for generated assembly and executables",
    )
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--repeats", type=int, default=9)
    parser.add_argument(
        "--workload",
        action="append",
        choices=[workload.name for workload in WORKLOADS],
        help="Limit measurement to one or more workloads",
    )
    parser.add_argument("--json", action="store_true", help="Emit machine-readable JSON")
    arguments = parser.parse_args()
    if arguments.warmups < 0 or arguments.repeats < 1:
        parser.error("--warmups must be nonnegative and --repeats must be positive")

    compiler = arguments.compiler.resolve()
    if not compiler.is_file():
        parser.error(f"compiler does not exist: {compiler}")
    build_directory = arguments.build_directory.resolve()
    build_directory.mkdir(parents=True, exist_ok=True)
    selected = [
        workload
        for workload in WORKLOADS
        if arguments.workload is None or workload.name in set(arguments.workload)
    ]
    measurements = [
        measure_workload(
            compiler,
            workload,
            build_directory,
            arguments.warmups,
            arguments.repeats,
        )
        for workload in selected
    ]

    if arguments.json:
        print(json.dumps([asdict(measurement) for measurement in measurements], indent=2))
    else:
        print_table(measurements)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

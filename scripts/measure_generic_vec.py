#!/usr/bin/env python3
"""Measure a representative generic Vec workload without enforcing timing."""

from __future__ import annotations

import argparse
import json

from measurement_support import (
    REPOSITORY,
    repository_identity,
    resolve_repository_path,
    timed_process,
    timing_summary,
    unique_run_directory,
)


SOURCE = REPOSITORY / "tests/benchmarks/generic_vec/growth.ska"
OUTPUT_ROOT = REPOSITORY / "build/measurements/generic-vec"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default="target/golden/skac")
    parser.add_argument("--compiler-profile", default="golden")
    parser.add_argument("--warmups", type=int, default=1)
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--timeout", type=float, default=30.0, dest="timeout_seconds")
    parser.add_argument("--json", action="store_true", dest="as_json")
    arguments = parser.parse_args()
    if arguments.warmups < 0 or arguments.repeats < 1:
        parser.error("--warmups must be nonnegative and --repeats must be positive")
    if arguments.timeout_seconds <= 0:
        parser.error("--timeout must be positive")

    compiler = resolve_repository_path(arguments.compiler)
    build_directory = unique_run_directory(OUTPUT_ROOT, "run")
    executable = build_directory / "growth"
    assembly = build_directory / "growth.s"

    _, compile_ms = timed_process(
        [compiler, SOURCE, "-o", executable],
        operation="generic Vec compilation",
        timeout_seconds=arguments.timeout_seconds,
    )
    _, assembly_ms = timed_process(
        [compiler, SOURCE, "--emit", "asm", "-o", assembly],
        operation="generic Vec assembly emission",
        timeout_seconds=arguments.timeout_seconds,
    )

    run_times = []
    for iteration in range(arguments.warmups + arguments.repeats):
        _, elapsed_ms = timed_process(
            [executable],
            operation="generic Vec workload",
            timeout_seconds=arguments.timeout_seconds,
        )
        if iteration >= arguments.warmups:
            run_times.append(elapsed_ms)

    run_summary = timing_summary(run_times)
    measurement = {
        "compiler": {
            **repository_identity(arguments.timeout_seconds),
            "profile": arguments.compiler_profile,
        },
        "target": "x86_64-sysv",
        "runtime_trace": "enabled",
        "workload": str(SOURCE.relative_to(REPOSITORY)),
        "input_bytes": SOURCE.stat().st_size,
        "warmups": arguments.warmups,
        "repeats": arguments.repeats,
        "timeout_seconds": arguments.timeout_seconds,
        "run_directory": str(build_directory.relative_to(REPOSITORY)),
        "compile_ms": compile_ms,
        "assembly_emit_ms": assembly_ms,
        "median_run_ms": run_summary["median_ms"],
        "median_absolute_deviation_ms": run_summary[
            "median_absolute_deviation_ms"
        ],
        "min_run_ms": run_summary["min_ms"],
        "max_run_ms": run_summary["max_ms"],
        "assembly_bytes": assembly.stat().st_size,
        "executable_bytes": executable.stat().st_size,
    }
    if arguments.as_json:
        print(json.dumps(measurement, indent=2))
        return
    for key, value in measurement.items():
        print(f"{key}: {value}")


if __name__ == "__main__":
    main()

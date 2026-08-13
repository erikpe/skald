#!/usr/bin/env python3
"""Measure a representative generic Vec workload without enforcing timing."""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import time
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
SOURCE = REPOSITORY / "tests/benchmarks/generic_vec/growth.ska"
BUILD_DIRECTORY = REPOSITORY / "build/measurements/generic-vec"


def timed_process(command: list[str]) -> tuple[subprocess.CompletedProcess[bytes], float]:
    started = time.perf_counter()
    completed = subprocess.run(command, cwd=REPOSITORY, capture_output=True, check=False)
    return completed, (time.perf_counter() - started) * 1000.0


def require_success(completed: subprocess.CompletedProcess[bytes], operation: str) -> None:
    if completed.returncode == 0:
        return
    stderr = completed.stderr.decode("utf-8", errors="replace")
    raise SystemExit(f"{operation} failed with exit code {completed.returncode}:\n{stderr}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default="target/debug/skac")
    parser.add_argument("--repeats", type=int, default=7)
    parser.add_argument("--json", action="store_true", dest="as_json")
    arguments = parser.parse_args()
    if arguments.repeats < 1:
        parser.error("--repeats must be at least 1")

    compiler = Path(arguments.compiler)
    if not compiler.is_absolute():
        compiler = REPOSITORY / compiler
    BUILD_DIRECTORY.mkdir(parents=True, exist_ok=True)
    executable = BUILD_DIRECTORY / "growth"
    assembly = BUILD_DIRECTORY / "growth.s"

    compiled, compile_ms = timed_process([str(compiler), str(SOURCE), "-o", str(executable)])
    require_success(compiled, "generic Vec compilation")
    emitted, assembly_ms = timed_process(
        [str(compiler), str(SOURCE), "--emit", "asm", "-o", str(assembly)]
    )
    require_success(emitted, "generic Vec assembly emission")

    run_times = []
    for _ in range(arguments.repeats):
        completed, elapsed_ms = timed_process([str(executable)])
        require_success(completed, "generic Vec workload")
        run_times.append(elapsed_ms)

    measurement = {
        "workload": str(SOURCE.relative_to(REPOSITORY)),
        "repeats": arguments.repeats,
        "compile_ms": compile_ms,
        "assembly_emit_ms": assembly_ms,
        "median_run_ms": statistics.median(run_times),
        "min_run_ms": min(run_times),
        "max_run_ms": max(run_times),
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

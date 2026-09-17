"""Untimed MIR schedule and analysis observations."""

import re
from pathlib import Path

from measurement_support import MeasurementFailure, run_checked
from .workloads import Workload


PASS_PATTERN = re.compile(
    rb"^skac: trace: (proof-rich|proof-transition|final) MIR pass `([^`]+)` "
    rb"\(pass identity \d+, schedule position (\d+), occurrence (\d+)\) "
    rb"(unchanged|changed|failed) ",
    re.MULTILINE,
)
ANALYSIS_PATTERN = re.compile(
    rb"^skac: trace analysis: ([^:]+): requests (\d+), computations (\d+), hits (\d+), "
    rb"repeated snapshot requests (\d+), results before (\d+), inserted (\d+), discarded (\d+)$"
)


def resolved_schedule(
    compiler: Path, run_directory: Path, timeout_seconds: float,
    *, env: dict[str, str] | None = None,
    compiler_arguments: tuple[str, ...] = (),
) -> list[dict[str, object]]:
    output = run_directory / "schedule.s"
    completed = run_checked(
        [
            compiler,
            *compiler_arguments,
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
        env=env,
    )
    schedule = [
        {
            "position": occurrence["position"],
            "pass": occurrence["pass"],
            "stage": occurrence["stage"],
            "occurrence": occurrence["occurrence"],
        }
        for occurrence in parse_pass_occurrences(completed.stderr)
    ]
    positions = [entry["position"] for entry in schedule]
    if not schedule or positions != list(range(len(schedule))):
        raise MeasurementFailure(
            "compiler trace did not contain one contiguous resolved MIR schedule"
        )
    return schedule


def parse_pass_occurrences(stderr: bytes) -> list[dict[str, object]]:
    return [
        {
            "position": int(position),
            "pass": name.decode("utf-8"),
            "stage": stage.decode("ascii"),
            "occurrence": int(occurrence),
            "outcome": outcome.decode("ascii"),
        }
        for stage, name, position, occurrence, outcome in PASS_PATTERN.findall(stderr)
    ]


def parse_analysis_usage(stderr: bytes) -> list[dict[str, object]]:
    current: dict[str, object] | None = None
    usage: list[dict[str, object]] = []
    for line in stderr.splitlines():
        if match := PASS_PATTERN.match(line):
            stage, name, position, occurrence, outcome = match.groups()
            current = {
                "position": int(position),
                "pass": name.decode("utf-8"),
                "stage": stage.decode("ascii"),
                "occurrence": int(occurrence),
                "outcome": outcome.decode("ascii"),
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
    *, env: dict[str, str] | None = None,
) -> tuple[list[dict[str, object]], list[dict[str, object]]]:
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
        env=env,
    )
    return (
        parse_analysis_usage(completed.stderr),
        parse_pass_occurrences(completed.stderr),
    )

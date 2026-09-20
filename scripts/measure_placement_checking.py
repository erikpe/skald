#!/usr/bin/env python3
"""Measure native-pilot phase cost without turning elapsed time into a test gate."""

from __future__ import annotations

import argparse
import json
import os
import platform
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

from measurement_support import (
    MeasurementFailure,
    alternating_order,
    deterministic_projection,
    measured_process,
    numeric_summary,
    repository_identity,
    run_checked,
    timing_summary,
    unique_run_directory,
)


REPOSITORY = Path(__file__).resolve().parents[1]
OUTPUT_ROOT = REPOSITORY / "build/measurements/placement-checking"
TEST_PREFIX = "backend::x86_64_sysv::native::pilot::tests::"
PLACEMENT_MARKER = "SKALD_PLACEMENT_PROFILE "
NATIVE_MARKER = "SKALD_NATIVE_PROFILE "


@dataclass(frozen=True)
class Witness:
    identity: str
    category: str
    test: str


WITNESSES = (
    Witness(
        "recursive-optionals",
        "d01",
        "primitive_and_shared_optionals_execute_through_the_verified_path",
    ),
    Witness(
        "primitive-array-allocation",
        "d02",
        "primitive_array_allocation_positions_and_length_execute_through_the_verified_path",
    ),
    Witness(
        "array-element-lifecycle",
        "d02",
        "nontrivial_array_element_lifecycle_executes_through_generated_helpers",
    ),
    Witness(
        "indexed-arrays-and-aliases",
        "d02",
        "indexed_arrays_and_array_aliases_execute_through_the_verified_path",
    ),
    Witness(
        "copied-array-slices",
        "d02",
        "copied_array_slices_execute_through_the_verified_path",
    ),
    Witness(
        "primitive-slice-assignment",
        "d02",
        "primitive_slice_assignment_executes_through_the_verified_path",
    ),
    Witness(
        "empty-inline-array-control",
        "control",
        "empty_inline_array_uses_its_null_representation_without_header_access",
    ),
    Witness(
        "small-scalar-control",
        "control",
        "primitive_integer_float_byte_boolean_and_cast_cells_execute",
    ),
)

TIMING_FIELDS = (
    "total_ns",
    "planning_ns",
    "discovery_lowering_ns",
    "executable_lowering_ns",
    "selection_ns",
    "placement_production_ns",
    "placement_checking_ns",
    "frame_planning_ns",
    "realization_checking_ns",
    "publication_ns",
)
STRUCTURAL_SUM_FIELDS = (
    "callables",
    "resource_locations",
    "storage_locations",
    "abi_locations",
    "tokens",
    "state_bits",
    "selected_events",
    "transfers",
    "reachable_blocks",
    "edge_occurrences",
    "convergence_rounds",
    "block_visits",
    "edge_visits",
    "fact_removals",
)


def profile_records(output: bytes, marker: str) -> list[dict[str, int]]:
    records: list[dict[str, int]] = []
    for line in output.decode("utf-8", errors="replace").splitlines():
        position = line.find(marker)
        if position < 0:
            continue
        try:
            value = json.loads(line[position + len(marker) :])
        except json.JSONDecodeError as error:
            raise MeasurementFailure(f"invalid {marker.strip()} record: {line}") from error
        if value.get("format") != 1:
            raise MeasurementFailure(f"unsupported {marker.strip()} format: {value!r}")
        if any(not isinstance(item, int) or item < 0 for item in value.values()):
            raise MeasurementFailure(f"invalid {marker.strip()} values: {value!r}")
        records.append(value)
    return records


def summarize_run(output: bytes) -> tuple[dict[str, int], dict[str, float]]:
    placements = profile_records(output, PLACEMENT_MARKER)
    native = profile_records(output, NATIVE_MARKER)
    if not placements:
        raise MeasurementFailure("test emitted no placement profile")
    if not native:
        raise MeasurementFailure("test emitted no native execution profile")
    deterministic = {
        "compile_invocations": len(placements),
        "native_invocations": len(native),
        **{
            field: sum(record[field] for record in placements)
            for field in STRUCTURAL_SUM_FIELDS
        },
        "peak_pending_blocks": max(
            record["peak_pending_blocks"] for record in placements
        ),
    }
    operational = {
        **{
            field.removesuffix("_ns") + "_ms": sum(
                record[field] for record in placements
            )
            / 1_000_000.0
            for field in TIMING_FIELDS
        },
        "native_link_ms": sum(record["link_ns"] for record in native) / 1_000_000.0,
        "native_execution_ms": sum(record["execution_ns"] for record in native)
        / 1_000_000.0,
    }
    return deterministic, operational


def select_witnesses(requested: Iterable[str]) -> tuple[Witness, ...]:
    selected_ids = set(requested)
    available = {witness.identity: witness for witness in WITNESSES}
    unknown = selected_ids.difference(available)
    if unknown:
        raise MeasurementFailure(f"unknown witnesses: {', '.join(sorted(unknown))}")
    return tuple(
        witness for witness in WITNESSES if not selected_ids or witness.identity in selected_ids
    )


def exact_command(witness: Witness) -> list[str]:
    return [
        "cargo",
        "test",
        "--locked",
        "-p",
        "skald-compiler",
        "--lib",
        f"{TEST_PREFIX}{witness.test}",
        "--",
        "--exact",
        "--nocapture",
        "--test-threads=1",
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repeats", type=int, default=2)
    parser.add_argument("--warmups", type=int, default=0)
    parser.add_argument("--timeout", type=float, default=600.0, dest="timeout_seconds")
    parser.add_argument("--output-root", type=Path, default=OUTPUT_ROOT)
    parser.add_argument("--witness", action="append", default=[])
    parser.add_argument("--json", action="store_true")
    arguments = parser.parse_args()
    if arguments.repeats < 2:
        parser.error("--repeats must be at least 2")
    if arguments.warmups < 0 or arguments.timeout_seconds <= 0:
        parser.error("warmups must be nonnegative and timeout must be positive")
    try:
        selected = select_witnesses(arguments.witness)
    except MeasurementFailure as error:
        parser.error(str(error))
    if not selected:
        parser.error("at least one witness is required")

    run_checked(
        ["cargo", "test", "--locked", "-p", "skald-compiler", "--lib", "--no-run"],
        operation="placement measurement test build",
        timeout_seconds=arguments.timeout_seconds,
    )
    run_directory = unique_run_directory(arguments.output_root.resolve(), "run")
    environment = dict(os.environ)
    environment["SKALD_PLACEMENT_MEASUREMENT"] = "1"
    deterministic: dict[str, dict[str, object]] = {}
    samples: dict[str, list[dict[str, float]]] = {
        witness.identity: [] for witness in selected
    }
    process_samples: dict[str, list[tuple[float, int]]] = {
        witness.identity: [] for witness in selected
    }

    identities = [witness.identity for witness in selected]
    by_identity = {witness.identity: witness for witness in selected}
    for iteration in range(arguments.warmups + arguments.repeats):
        for identity in alternating_order(identities, iteration):
            witness = by_identity[identity]
            usage = measured_process(
                exact_command(witness),
                operation=f"placement witness {identity}",
                run_directory=run_directory,
                timeout_seconds=arguments.timeout_seconds,
                env=environment,
            )
            observed, operational = summarize_run(
                usage.completed.stdout + b"\n" + usage.completed.stderr
            )
            prior = deterministic.get(identity)
            if prior is not None and prior["dimensions"] != observed:
                raise MeasurementFailure(f"{identity} structural metrics changed between runs")
            deterministic[identity] = {
                "category": witness.category,
                "test": f"{TEST_PREFIX}{witness.test}",
                "dimensions": observed,
            }
            if iteration >= arguments.warmups:
                samples[identity].append(operational)
                process_samples[identity].append((usage.wall_ms, usage.peak_rss_kib))
            print(
                f"{identity}: {usage.wall_ms / 1000:.2f}s, "
                f"placement check {operational['placement_checking_ms'] / 1000:.2f}s, "
                f"peak {usage.peak_rss_kib / 1024:.1f} MiB",
                file=sys.stderr,
            )

    operational_report: dict[str, dict[str, object]] = {}
    for witness in selected:
        identity = witness.identity
        phase_names = samples[identity][0].keys()
        operational_report[identity] = {
            "process_wall": timing_summary([sample[0] for sample in process_samples[identity]]),
            "process_peak_rss_kib": numeric_summary(
                [sample[1] for sample in process_samples[identity]]
            ),
            "phases": {
                phase: timing_summary([sample[phase] for sample in samples[identity]])
                for phase in phase_names
            },
        }

    report: dict[str, object] = {
        "format": 1,
        "deterministic": {
            "repository": repository_identity(arguments.timeout_seconds),
            "profile": "test (unoptimized + debuginfo)",
            "protocol": {
                "warmups": arguments.warmups,
                "repeats": arguments.repeats,
                "timeout_seconds": arguments.timeout_seconds,
                "order": "alternating",
            },
            "host": {
                "system": platform.system(),
                "release": platform.release(),
                "machine": platform.machine(),
                "node": platform.node(),
                "logical_cpus": os.cpu_count(),
                "python": platform.python_version(),
            },
            "workloads": deterministic,
        },
        "operational": {"workloads": operational_report},
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
        print(run_directory)
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except MeasurementFailure as error:
        print(f"measurement failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error

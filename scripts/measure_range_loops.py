#!/usr/bin/env python3
"""Compare fused primitive range loops with matched handwritten while loops."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

from measurement_support import (
    REPOSITORY,
    alternating_order,
    repository_identity,
    resolve_repository_path,
    timed_process,
    timing_summary,
    unique_run_directory,
)

SOURCE_DIRECTORY = REPOSITORY / "tests/benchmarks/range_loop"
OUTPUT_ROOT = REPOSITORY / "build/measurements/range-loop"
INTEGER_TYPES = ("u8", "u64", "i64")
SOURCE_FUNCTION = re.compile(r"^\.type (\.Lska\.fn\..*\.main\.f\d+), @function$")


def source_instruction_profile(assembly: Path) -> dict[str, int]:
    lines = assembly.read_text(encoding="utf-8").splitlines()
    symbol = next(
        (match.group(1) for line in lines if (match := SOURCE_FUNCTION.match(line))),
        None,
    )
    if symbol is None:
        raise SystemExit(f"could not locate the Skald main function in {assembly}")
    start = lines.index(f"{symbol}:") + 1
    end = lines.index(f".size {symbol}, .-{symbol}")
    profile: dict[str, int] = {}
    for line in lines[start:end]:
        stripped = line.strip()
        if not stripped or stripped.startswith(".") or stripped.endswith(":"):
            continue
        mnemonic = stripped.split(maxsplit=1)[0]
        profile[mnemonic] = profile.get(mnemonic, 0) + 1
    return profile


def compile_workloads(
    compiler: Path, build_directory: Path, timeout_seconds: float
) -> dict[tuple[str, str], dict[str, object]]:
    products: dict[tuple[str, str], dict[str, object]] = {}
    for integer in INTEGER_TYPES:
        for form in ("range", "while"):
            source = SOURCE_DIRECTORY / f"{integer}_{form}.ska"
            executable = build_directory / f"{integer}_{form}"
            assembly = build_directory / f"{integer}_{form}.s"
            _, compile_ms = timed_process(
                [compiler, source, "--omit-runtime-trace", "-o", executable],
                operation=f"{integer} {form} compilation",
                timeout_seconds=timeout_seconds,
            )
            _, assembly_ms = timed_process(
                [
                    compiler,
                    source,
                    "--omit-runtime-trace",
                    "--emit",
                    "asm",
                    "-o",
                    assembly,
                ],
                operation=f"{integer} {form} assembly emission",
                timeout_seconds=timeout_seconds,
            )
            products[(integer, form)] = {
                "source": str(source.relative_to(REPOSITORY)),
                "executable": executable,
                "compile_ms": compile_ms,
                "assembly_emit_ms": assembly_ms,
                "assembly_bytes": assembly.stat().st_size,
                "executable_bytes": executable.stat().st_size,
                "source_instruction_profile": source_instruction_profile(assembly),
                "run_times_ms": [],
            }
    return products


def run_workloads(
    products: dict[tuple[str, str], dict[str, object]],
    repeats: int,
    warmups: int,
    timeout_seconds: float,
) -> None:
    for iteration in range(warmups + repeats):
        forms = alternating_order(("range", "while"), iteration)
        for integer in INTEGER_TYPES:
            for form in forms:
                product = products[(integer, form)]
                _, elapsed_ms = timed_process(
                    [product["executable"]],
                    operation=f"{integer} {form} workload",
                    timeout_seconds=timeout_seconds,
                )
                if iteration >= warmups:
                    product["run_times_ms"].append(elapsed_ms)


def measurements(
    products: dict[tuple[str, str], dict[str, object]], repeats: int, threshold: float
) -> dict[str, object]:
    rows: dict[str, object] = {}
    all_within_target = True
    for integer in INTEGER_TYPES:
        forms: dict[str, object] = {}
        medians: dict[str, float] = {}
        for form in ("range", "while"):
            product = products[(integer, form)]
            times = product["run_times_ms"]
            summary = timing_summary(times)
            median = summary["median_ms"]
            medians[form] = median
            forms[form] = {
                **{
                    key: value
                    for key, value in product.items()
                    if key not in {"executable", "run_times_ms"}
                },
                "median_run_ms": median,
                "median_absolute_deviation_ms": summary[
                    "median_absolute_deviation_ms"
                ],
                "min_run_ms": min(times),
                "max_run_ms": max(times),
            }
        ratio = medians["range"] / medians["while"]
        within_target = ratio <= 1.0 + threshold
        all_within_target &= within_target
        rows[integer] = {
            "forms": forms,
            "range_to_while_ratio": ratio,
            "range_overhead_percent": (ratio - 1.0) * 100.0,
            "within_target": within_target,
        }
    return {
        "repeats": repeats,
        "maximum_range_overhead_percent": threshold * 100.0,
        "all_within_target": all_within_target,
        "integer_types": rows,
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", default="target/golden/skac")
    parser.add_argument("--compiler-profile", default="golden")
    parser.add_argument("--repeats", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--maximum-overhead", type=float, default=0.10)
    parser.add_argument("--require-target", action="store_true")
    parser.add_argument("--timeout", type=float, default=30.0, dest="timeout_seconds")
    parser.add_argument("--json", action="store_true", dest="as_json")
    arguments = parser.parse_args()
    if arguments.repeats < 1:
        parser.error("--repeats must be at least 1")
    if arguments.warmups < 0:
        parser.error("--warmups must not be negative")
    if arguments.maximum_overhead < 0:
        parser.error("--maximum-overhead must not be negative")
    if arguments.timeout_seconds <= 0:
        parser.error("--timeout must be positive")

    compiler = resolve_repository_path(arguments.compiler)
    build_directory = unique_run_directory(OUTPUT_ROOT, "run")
    products = compile_workloads(compiler, build_directory, arguments.timeout_seconds)
    run_workloads(products, arguments.repeats, arguments.warmups, arguments.timeout_seconds)
    result = measurements(products, arguments.repeats, arguments.maximum_overhead)
    result.update(
        {
            "compiler": {
                **repository_identity(arguments.timeout_seconds),
                "profile": arguments.compiler_profile,
            },
            "target": "x86_64-sysv",
            "runtime_trace": "omitted",
            "warmups": arguments.warmups,
            "timeout_seconds": arguments.timeout_seconds,
            "run_directory": str(build_directory.relative_to(REPOSITORY)),
        }
    )
    if arguments.as_json:
        print(json.dumps(result, indent=2))
    else:
        print(f"repeats: {result['repeats']}")
        print(f"run_directory: {result['run_directory']}")
        print(f"maximum_range_overhead_percent: {result['maximum_range_overhead_percent']}")
        for integer, row in result["integer_types"].items():
            print(
                f"{integer}: range={row['forms']['range']['median_run_ms']:.3f} ms "
                f"while={row['forms']['while']['median_run_ms']:.3f} ms "
                f"overhead={row['range_overhead_percent']:.2f}% "
                f"within_target={row['within_target']}"
            )
        print(f"all_within_target: {result['all_within_target']}")
    if arguments.require_target and not result["all_within_target"]:
        raise SystemExit("one or more fused range measurements exceeded the overhead target")


if __name__ == "__main__":
    main()

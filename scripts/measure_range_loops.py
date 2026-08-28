#!/usr/bin/env python3
"""Compare fused primitive range loops with matched handwritten while loops."""

from __future__ import annotations

import argparse
import json
import re
import statistics
import subprocess
import time
from pathlib import Path


REPOSITORY = Path(__file__).resolve().parents[1]
SOURCE_DIRECTORY = REPOSITORY / "tests/benchmarks/range_loop"
BUILD_DIRECTORY = REPOSITORY / "build/measurements/range-loop"
INTEGER_TYPES = ("u8", "u64", "i64")
SOURCE_FUNCTION = re.compile(r"^\.type (\.Lska\.fn\..*\.main\.f\d+), @function$")


def timed_process(command: list[str]) -> tuple[subprocess.CompletedProcess[bytes], float]:
    started = time.perf_counter()
    completed = subprocess.run(command, cwd=REPOSITORY, capture_output=True, check=False)
    return completed, (time.perf_counter() - started) * 1000.0


def require_success(completed: subprocess.CompletedProcess[bytes], operation: str) -> None:
    if completed.returncode == 0:
        return
    stderr = completed.stderr.decode("utf-8", errors="replace")
    raise SystemExit(f"{operation} failed with exit code {completed.returncode}:\n{stderr}")


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


def compile_workloads(compiler: Path) -> dict[tuple[str, str], dict[str, object]]:
    BUILD_DIRECTORY.mkdir(parents=True, exist_ok=True)
    products: dict[tuple[str, str], dict[str, object]] = {}
    for integer in INTEGER_TYPES:
        for form in ("range", "while"):
            source = SOURCE_DIRECTORY / f"{integer}_{form}.ska"
            executable = BUILD_DIRECTORY / f"{integer}_{form}"
            assembly = BUILD_DIRECTORY / f"{integer}_{form}.s"
            compiled, compile_ms = timed_process(
                [str(compiler), str(source), "--omit-runtime-trace", "-o", str(executable)]
            )
            require_success(compiled, f"{integer} {form} compilation")
            emitted, assembly_ms = timed_process(
                [
                    str(compiler),
                    str(source),
                    "--omit-runtime-trace",
                    "--emit",
                    "asm",
                    "-o",
                    str(assembly),
                ]
            )
            require_success(emitted, f"{integer} {form} assembly emission")
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
    products: dict[tuple[str, str], dict[str, object]], repeats: int, warmups: int
) -> None:
    for iteration in range(warmups + repeats):
        forms = ("range", "while") if iteration % 2 == 0 else ("while", "range")
        for integer in INTEGER_TYPES:
            for form in forms:
                product = products[(integer, form)]
                completed, elapsed_ms = timed_process([str(product["executable"])])
                require_success(completed, f"{integer} {form} workload")
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
            median = statistics.median(times)
            medians[form] = median
            forms[form] = {
                **{
                    key: value
                    for key, value in product.items()
                    if key not in {"executable", "run_times_ms"}
                },
                "median_run_ms": median,
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
    parser.add_argument("--compiler", default="target/debug/skac")
    parser.add_argument("--repeats", type=int, default=9)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--maximum-overhead", type=float, default=0.10)
    parser.add_argument("--require-target", action="store_true")
    parser.add_argument("--json", action="store_true", dest="as_json")
    arguments = parser.parse_args()
    if arguments.repeats < 1:
        parser.error("--repeats must be at least 1")
    if arguments.warmups < 0:
        parser.error("--warmups must not be negative")
    if arguments.maximum_overhead < 0:
        parser.error("--maximum-overhead must not be negative")

    compiler = Path(arguments.compiler)
    if not compiler.is_absolute():
        compiler = REPOSITORY / compiler
    products = compile_workloads(compiler)
    run_workloads(products, arguments.repeats, arguments.warmups)
    result = measurements(products, arguments.repeats, arguments.maximum_overhead)
    if arguments.as_json:
        print(json.dumps(result, indent=2))
    else:
        print(f"repeats: {result['repeats']}")
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

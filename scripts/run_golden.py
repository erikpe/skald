from __future__ import annotations

import difflib
import re
import subprocess
import sys
from pathlib import Path
from typing import Iterable, List, Tuple


def main() -> int:
    root = Path(__file__).resolve().parents[1]
    golden_root = root / "test" / "golden"
    if not golden_root.exists():
        print(f"Golden test root not found: {golden_root}")
        return 1

    toy_files = sorted(golden_root.rglob("test_*.toy"))
    if not toy_files:
        print(f"No golden tests found under {golden_root}")
        return 1

    build_root = root / "build" / "golden"
    build_root.mkdir(parents=True, exist_ok=True)

    failures: List[str] = []
    total_execs = 0
    passed_execs = 0
    total_compiles = 0
    passed_compiles = 0
    for toy_path in toy_files:
        rel = toy_path.relative_to(golden_root)
        test_id = "__".join(rel.with_suffix("").parts)
        out_s = build_root / f"{test_id}.s"
        out_bin = build_root / f"{test_id}.out"

        cases = collect_cases(toy_path)
        if not cases:
            failures.append(f"{rel}: no expected output files")
            continue

        compile_src = prepare_compile_source(root, toy_path, build_root, test_id)

        total_compiles += 1
        compile_ok = compile_program(root, compile_src, out_s, out_bin)
        print_result(f"COMPILE {rel}", compile_ok)
        if compile_ok:
            passed_compiles += 1
        else:
            failures.append(f"{rel}: compile failed")
            continue

        for case in cases:
            total_execs += 1
            case_desc = case_label(rel, case)
            ok = run_case(out_bin, case)
            print_result(f"RUN     {case_desc}", ok)
            if not ok:
                failures.append(case_desc)
            else:
                passed_execs += 1

    print("\nSummary:")
    print(f"- Programs: {len(toy_files)}")
    print(f"- Compiles: {passed_compiles}/{total_compiles}")
    print(f"- Runs:     {passed_execs}/{total_execs}")

    if failures:
        print("\nFailures:")
        for item in failures:
            print(f"- {item}")
        return 1

    print(f"All tests passed ({len(toy_files)} programs).")
    return 0


def compile_program(root: Path, src: Path, out_s: Path, out_bin: Path) -> bool:
    cmd_emit = [
        "python3",
        str(root / "src" / "main.py"),
        str(src),
        "--emit",
        str(out_s),
    ]
    res = subprocess.run(cmd_emit, capture_output=True, text=True)
    if res.returncode != 0:
        print(res.stdout)
        print(res.stderr)
        return False

    cmd_link = [
        "gcc",
        str(out_s),
        str(root / "runtime" / "runtime.c"),
        "-o",
        str(out_bin),
    ]
    res = subprocess.run(cmd_link, capture_output=True, text=True)
    if res.returncode != 0:
        print(res.stdout)
        print(res.stderr)
        return False

    return True


def prepare_compile_source(root: Path, src: Path, build_root: Path, test_id: str) -> Path:
    source_text = src.read_text(encoding="utf-8")
    libs = parse_stdlib_directives(source_text)
    if not libs:
        return src

    merged_parts: List[str] = []
    for lib in libs:
        lib_path = root / "stdlib" / f"{lib}.toy"
        if not lib_path.exists():
            raise SystemExit(f"Missing stdlib file for {src}: {lib_path}")
        merged_parts.append(lib_path.read_text(encoding="utf-8").rstrip())

    filtered_lines = [
        line for line in source_text.splitlines() if not _is_stdlib_directive_line(line)
    ]
    merged_parts.append("\n".join(filtered_lines).rstrip())

    merged_source = "\n\n".join(part for part in merged_parts if part) + "\n"
    merged_path = build_root / f"{test_id}.merged.toy"
    merged_path.write_text(merged_source, encoding="utf-8")
    return merged_path


def parse_stdlib_directives(source_text: str) -> List[str]:
    libs: List[str] = []
    seen: set[str] = set()
    for line in source_text.splitlines():
        if not _is_stdlib_directive_line(line):
            continue
        match = re.match(r"\s*//\s*stdlib\s*:\s*(.*)$", line)
        if match is None:
            continue
        names = [part.strip() for part in match.group(1).split(",")]
        for name in names:
            if not name:
                continue
            if not re.match(r"^[A-Za-z_][A-Za-z0-9_]*$", name):
                raise SystemExit(f"Invalid stdlib module name in directive: {name}")
            if name not in seen:
                seen.add(name)
                libs.append(name)
    return libs


def _is_stdlib_directive_line(line: str) -> bool:
    return re.match(r"\s*//\s*stdlib\s*:", line) is not None


def collect_cases(src: Path) -> List[Tuple[Path | None, Path]]:
    base = src.with_suffix("")
    out_default = base.with_suffix(".output.txt")
    cases: List[Tuple[Path | None, Path]] = []

    if out_default.exists():
        cases.append((None, out_default))

    outputs = sorted(src.parent.glob(f"{src.stem}.output_*.txt"))
    for out_path in outputs:
        suffix = out_path.suffixes[-2]  # .output_N
        idx = suffix.split("_")[-1]
        in_path = src.parent / f"{src.stem}.input_{idx}.txt"
        if not in_path.exists():
            raise SystemExit(f"Missing input for {out_path}: {in_path}")
        cases.append((in_path, out_path))

    inputs = sorted(src.parent.glob(f"{src.stem}.input_*.txt"))
    for in_path in inputs:
        suffix = in_path.suffixes[-2]  # .input_N
        idx = suffix.split("_")[-1]
        out_path = src.parent / f"{src.stem}.output_{idx}.txt"
        if not out_path.exists():
            raise SystemExit(f"Missing output for {in_path}: {out_path}")

    return cases


def run_case(binary: Path, case: Tuple[Path | None, Path]) -> bool:
    in_path, out_path = case
    expected = out_path.read_text(encoding="utf-8").replace("\r\n", "\n")
    stdin_data = None
    if in_path is not None:
        stdin_data = in_path.read_text(encoding="utf-8")

    res = subprocess.run(
        [str(binary)],
        input=stdin_data,
        capture_output=True,
        text=True,
    )
    if res.returncode != 0:
        print(f"Program exited with code {res.returncode}")
        print(res.stderr)
        return False

    actual = res.stdout.replace("\r\n", "\n")
    if actual != expected:
        print(f"Output mismatch for {out_path}")
        diff = difflib.unified_diff(
            expected.splitlines(),
            actual.splitlines(),
            fromfile="expected",
            tofile="actual",
            lineterm="",
        )
        print("\n".join(diff))
        return False

    return True


def case_label(rel: Path, case: Tuple[Path | None, Path]) -> str:
    in_path, out_path = case
    if in_path is None:
        return f"{rel} (no input)"
    return f"{rel} ({in_path.name})"


def print_result(label: str, ok: bool) -> None:
    status = "SUCCESS" if ok else "FAIL"
    print(f"{label} ... {status}")


if __name__ == "__main__":
    raise SystemExit(main())

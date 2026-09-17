"""Target-specific static metrics, independent of semantic artifact retention."""

import re
from pathlib import Path

from measurement_support import MeasurementFailure, run_checked
from .stack import explicit_stack_peak


def unsupported(reason: str) -> dict[str, object]:
    return {"status": "unsupported", "reason": reason}


def assembly_metrics(assembly: str) -> dict[str, object]:
    """Recognize the current x86 frame recipe; never infer an unknown frame as zero."""
    functions = {}
    pattern = r"^\.type ([^,\s]+), @function\n(.*?)^\.size \1,[^\n]*$"
    for match in re.finditer(pattern, assembly, re.MULTILINE | re.DOTALL):
        name, body = match.groups()
        instructions = [line.strip() for line in body.splitlines()
                        if line.strip() and not line.strip().startswith(".")
                        and not line.strip().endswith(":")]
        peak, reason = explicit_stack_peak(body)
        if reason:
            functions[name] = unsupported(reason)
            continue
        if "push rbp" not in instructions:
            # Optional copy helpers use balanced nested rsp scratch areas;
            # retain helpers have a zero fixed frame, including failure calls.
            if any(re.search(r"\b(?:rbp|ebp|bp)\b", line) or line == "leave" for line in instructions):
                functions[name] = unsupported("unrecognized frame prologue")
                continue
            start = None
            body_instructions = instructions
        else:
            start = instructions.index("push rbp")
            if (instructions[start:start + 2] != ["push rbp", "mov rbp, rsp"] or any(
                re.search(r"\b(?:rbp|rsp)\b", line) for line in instructions[:start])):
                functions[name] = unsupported("unrecognized frame prologue")
                continue
            body_instructions = instructions[:start] + instructions[start + 2:]
        # Array lifecycle helpers may compute addresses before setting up their
        # one fixed frame, including conditional paths that skip it entirely.
        local_bytes = 0
        if start is not None and len(instructions) > start + 2 and instructions[start + 2].startswith("sub rsp,"):
            reserve = re.fullmatch(r"sub rsp, (\d+)", instructions[start + 2])
            if reserve is None:
                functions[name] = unsupported("nonconstant frame reservation")
                continue
            local_bytes = int(reserve[1])
        # Other save/alignment/frame-base recipes need their own extraction tests.
        if any(re.match(r"(?:push\w*|pop\w*|enter)\b", line) for line in body_instructions) or any(
            re.match(r"(?:mov|lea|and|add) (?:rbp|rsp),", line)
            for line in body_instructions
            if not re.fullmatch(r"add rsp, \d+", line)
        ):
            functions[name] = unsupported("unrecognized frame/save recipe")
            continue
        if any(line.startswith(("sub rsp,", "add rsp,")) and not
               re.fullmatch(r"(?:sub|add) rsp, \d+", line) for line in instructions):
            functions[name] = unsupported("nonconstant stack adjustment")
            continue
        accesses = 0
        bad = False
        for instruction in instructions:
            for operand in re.findall(r"\[([^\]]+)\]", instruction):
                if not re.search(r"\b(?:rbp|rsp)\b", operand):
                    continue
                if not re.fullmatch(r"(?:rbp|rsp)(?:\s*[+-]\s*\d+)?", operand):
                    bad = True
                elif not instruction.startswith("lea "):
                    accesses += 1
        functions[name] = (unsupported("nonconstant frame operand") if bad else {
            "status": "supported", "frame_bytes": 8 + local_bytes if start is not None else 0,
            "max_stack_bytes": peak,
            "static_frame_accesses": accesses,
        })
    declared = re.findall(r"^\.type ([^,\s]+), @function$", assembly, re.MULTILINE)
    for name in declared:
        functions.setdefault(name, unsupported("missing function end marker"))
    if not functions:
        return unsupported("no recognized function declarations")
    return {"status": "supported" if all(f["status"] == "supported"
            for f in functions.values()) else "unsupported", "functions": functions}


def parse_text_size(output: str) -> dict[str, object]:
    sizes = []
    for line in output.splitlines():
        fields = line.split()
        if fields and (fields[0] == ".text" or fields[0].startswith(".text.")):
            if len(fields) != 3 or not fields[1].isdigit():
                return unsupported("unrecognized ELF text-section size")
            sizes.append(int(fields[1]))
    return ({"status": "supported", "bytes": sum(sizes)} if sizes
            else unsupported("ELF text sections absent"))


def native_text_size(executable: Path, timeout: float) -> dict[str, object]:
    try:
        output = run_checked(["size", "-A", executable], timeout_seconds=timeout,
                             operation="native text-section inspection")
    except (OSError, MeasurementFailure) as error:
        return unsupported(str(error))
    return parse_text_size(output.stdout.decode("utf-8", errors="replace"))

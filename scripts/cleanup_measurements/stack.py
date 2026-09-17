"""Bounded stack-depth extraction for the current explicit x86 helper recipes."""

import re


def explicit_stack_peak(body: str) -> tuple[int | None, str | None]:
    labels, instructions = {}, []
    for line in body.splitlines():
        line = line.strip()
        if line.endswith(":"):
            labels[line[:-1]] = len(instructions)
        elif line and not line.startswith("."):
            instructions.append(line)
    if not instructions:
        return None, "empty function body"
    effects = []
    for line in instructions:
        if line == "push rbp":
            effects.append(8)
        elif line == "mov rbp, rsp":
            effects.append(0)
        elif reserve := re.fullmatch(r"(sub|add) rsp, (\d+)", line):
            effects.append(int(reserve[2]) * (1 if reserve[1] == "sub" else -1))
        elif re.match(r"(?:push\w*|pop\w*|enter|iret\w*)\b", line):
            return None, "unrecognized stack/save instruction"
        elif re.match(r"\w+ (?:rsp|rbp|esp|ebp|sp|bp)(?:,|$)", line) and not line.startswith(("cmp ", "test ")):
            return None, "unrecognized stack/base write"
        elif line.startswith("ret "):
            return None, "unrecognized return stack adjustment"
        else:
            effects.append(0)
    depths, pending, peak = {}, [(0, 0)], 0
    while pending:
        position, depth = pending.pop()
        if position >= len(instructions):
            return None, "control flow falls outside function"
        # A shared hard-trap sink can be reached before or after a nonreturning
        # reporter's alignment reservation. There is no continuation to merge.
        if instructions[position] == "ud2":
            peak = max(peak, depth)
            continue
        if position in depths:
            if depths[position] != depth:
                return None, "inconsistent stack depth at join/loop"
            continue
        depths[position] = depth
        line = instructions[position]
        depth = 0 if line == "leave" else depth + effects[position]
        if depth < 0:
            return None, "stack release exceeds reservation"
        peak = max(peak, depth)
        if line == "ret":
            if depth:
                return None, "return with outstanding stack reservation"
            continue
        if jump := re.fullmatch(r"j\w+ (\S+)", line):
            if jump[1] not in labels:
                return None, "unrecognized jump target"
            pending.append((labels[jump[1]], depth))
            if line.startswith("jmp "):
                continue
        pending.append((position + 1, depth))
    return peak, None

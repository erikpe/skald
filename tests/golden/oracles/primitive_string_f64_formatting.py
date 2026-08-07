#!/usr/bin/env python3
"""Generate the stable shortest-binary64-formatting corpus and oracle.

The corpus spans every finite exponent field and exhaustively sweeps bounded
significand neighborhoods at representative exponent fields. Python's
binary64 ``repr`` supplies an implementation-independent shortest digit
oracle independent of the Skald implementation; this script applies Skald's
separately frozen presentation thresholds and verifies every emitted decimal
by parsing it back to the original bits. Run it from the repository root and
compare its labelled sections with `data/f64-formatting-corpus.stdin` and
`data/f64-formatting-corpus.stdout`, owned by
`primitive_strings/conversions.golden.toml`.
"""

import struct


FRACTION_MASK = (1 << 52) - 1
SIGN_BIT = 1 << 63


def from_bits(bits: int) -> float:
    return struct.unpack(">d", bits.to_bytes(8, "big"))[0]


def to_bits(value: float) -> int:
    return int.from_bytes(struct.pack(">d", value), "big")


def shortest_parts(value: float) -> tuple[bool, str, int]:
    negative = value < 0.0
    spelling = repr(-value if negative else value)
    if "e" in spelling:
        coefficient, exponent_text = spelling.split("e")
        decimal_exponent = int(exponent_text)
        digits = coefficient.replace(".", "").rstrip("0")
        return negative, digits, decimal_exponent

    whole, fractional = spelling.split(".")
    combined = whole + fractional
    first_nonzero = next(index for index, digit in enumerate(combined) if digit != "0")
    decimal_exponent = len(whole) - first_nonzero - 1
    digits = combined[first_nonzero:].rstrip("0")
    return negative, digits, decimal_exponent


def canonical(value: float) -> str:
    if value == 0.0:
        return "-0.0" if to_bits(value) & SIGN_BIT else "0.0"

    negative, digits, exponent = shortest_parts(value)
    sign = "-" if negative else ""
    if -3 <= exponent < 7:
        if exponent < 0:
            return f"{sign}0.{('0' * (-exponent - 1))}{digits}"
        integer_digits = exponent + 1
        if len(digits) <= integer_digits:
            return f"{sign}{digits}{('0' * (integer_digits - len(digits)))}.0"
        return f"{sign}{digits[:integer_digits]}.{digits[integer_digits:]}"

    fraction = digits[1:] or "0"
    return f"{sign}{digits[0]}.{fraction}E{exponent}"


def corpus_bits() -> list[int]:
    cases = {
        0x0000000000000000,
        0x8000000000000000,
        0x0000000000000001,
        0x000FFFFFFFFFFFFF,
        0x0010000000000000,
        0x3FB999999999999A,
        0x3FEFFFFFFFFFFFFF,
        0x3FF0000000000000,
        0x3FF0000000000001,
        0x4330000000000000,
        0x4340000000000000,
        0x7FDFFFFFFFFFFFFF,
        0x7FEFFFFFFFFFFFFF,
    }

    # One rotating significand edge for every finite exponent field.
    edges = [0, 1, 1 << 51, FRACTION_MASK]
    for exponent_field in range(2047):
        fraction = edges[exponent_field % len(edges)]
        bits = (exponent_field << 52) | fraction
        if bits != 0:
            cases.add(bits | (SIGN_BIT if exponent_field % 2 else 0))

    # Exhaustive bounded sweeps around low, middle, and high significand
    # neighborhoods at representative subnormal, normal, and extreme fields.
    for exponent_field in [0, 1, 2, 1022, 1023, 1024, 2045, 2046]:
        exponent_bits = exponent_field << 52
        for offset in range(32):
            for fraction in [
                offset,
                (1 << 51) - 16 + offset,
                FRACTION_MASK - offset,
            ]:
                bits = exponent_bits | fraction
                cases.add(bits)
                if offset in [0, 1, 15, 16, 31]:
                    cases.add(bits | SIGN_BIT)

    return sorted(cases)


def source_spelling(value: float) -> str:
    if to_bits(value) == SIGN_BIT:
        return "-0.0"
    return repr(value)


if __name__ == "__main__":
    values = [(bits, from_bits(bits)) for bits in corpus_bits()]
    print("--- stdin ---")
    print("\n".join(source_spelling(value) for _, value in values))
    print("--- stdout ---")
    output: list[str] = []
    for bits, value in values:
        formatted = canonical(value)
        assert to_bits(float(formatted)) == bits
        output.extend([formatted, f"0x{bits:016x}"])
    print("\n".join(output))

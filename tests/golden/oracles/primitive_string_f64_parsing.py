#!/usr/bin/env python3
"""Print the checked-in binary64 parsing corpus and its exact oracle output.

The oracle uses integer fractions and an explicit nearest/ties-to-even search;
it deliberately does not call a host floating-point parser. Run it from the
repository root and compare the two labelled sections with the golden stdin
and stdout sidecars.
"""

from fractions import Fraction
import re


MAX_FINITE_BITS = 0x7FEFFFFFFFFFFFFF
SIGN_BIT = 1 << 63
DECIMAL = re.compile(
    r"(?P<sign>-?)(?:(?P<whole>[0-9]+)(?:\.(?P<fraction>[0-9]*))?|\.(?P<only_fraction>[0-9]+))"
    r"(?:[eE](?P<exponent>[+-]?[0-9]+))?\Z"
)


def bits_fraction(bits: int) -> Fraction:
    exponent_field = bits >> 52
    fraction_field = bits & ((1 << 52) - 1)
    if exponent_field == 0:
        significand = fraction_field
        exponent = -1074
    else:
        significand = (1 << 52) | fraction_field
        exponent = exponent_field - 1023 - 52
    if exponent >= 0:
        return Fraction(significand << exponent)
    return Fraction(significand, 1 << -exponent)


def decimal_fraction(text: str) -> tuple[bool, Fraction | None] | None:
    match = DECIMAL.fullmatch(text)
    if match is None:
        return None
    negative = match.group("sign") == "-"
    whole = match.group("whole") or ""
    fractional = match.group("fraction")
    if fractional is None:
        fractional = match.group("only_fraction") or ""
    digits = whole + fractional
    exponent = int(match.group("exponent") or "0") - len(fractional)
    integer = int(digits)
    if integer == 0:
        return negative, Fraction(0)
    scientific_exponent = len(digits.lstrip("0")) - 1 + exponent
    if scientific_exponent >= 309:
        return negative, None
    if scientific_exponent <= -325:
        return negative, Fraction(0)
    value = Fraction(integer)
    if exponent >= 0:
        value *= 10**exponent
    else:
        value /= 10**-exponent
    return negative, value


def round_fraction(value: Fraction) -> int | None:
    maximum = bits_fraction(MAX_FINITE_BITS)
    overflow_midpoint = maximum + Fraction(1 << 970)
    if value >= overflow_midpoint:
        return None
    if value > maximum:
        return MAX_FINITE_BITS

    low = 0
    high = MAX_FINITE_BITS
    while low < high:
        middle = (low + high + 1) // 2
        if bits_fraction(middle) <= value:
            low = middle
        else:
            high = middle - 1
    if bits_fraction(low) == value:
        return low

    upper = low + 1
    lower_distance = value - bits_fraction(low)
    upper_distance = bits_fraction(upper) - value
    if lower_distance < upper_distance:
        return low
    if lower_distance > upper_distance:
        return upper
    return low if low % 2 == 0 else upper


def expected(text: str) -> str:
    if text == "NaN":
        return "nan"
    if text == "Infinity":
        return "0x7ff0000000000000"
    if text == "-Infinity":
        return "0xfff0000000000000"
    parsed = decimal_fraction(text)
    if parsed is None:
        return "none"
    negative, value = parsed
    if value is None:
        return "none"
    bits = round_fraction(value)
    if bits is None:
        return "none"
    if negative:
        bits |= SIGN_BIT
    return f"0x{bits:016x}"


def terminating_decimal(value: Fraction) -> tuple[int, int]:
    denominator = value.denominator
    power = 0
    while denominator > 1:
        assert denominator % 2 == 0
        denominator //= 2
        power += 1
    return value.numerator * 5**power, -power


def midpoint_triplet(lower_bits: int) -> list[str]:
    midpoint = (bits_fraction(lower_bits) + bits_fraction(lower_bits + 1)) / 2
    digits, exponent = terminating_decimal(midpoint)
    return [
        f"{digits - 1}e{exponent}",
        f"{digits}e{exponent}",
        f"{digits + 1}e{exponent}",
    ]


def corpus() -> list[str]:
    cases = [
        "NaN",
        "Infinity",
        "-Infinity",
        "0",
        "-0",
        "0.0",
        "-0.0",
        "000e999999999999999999999999",
        "-000e-999999999999999999999999",
        "1",
        "-1",
        ".5",
        "-.5",
        "1.",
        "01.0",
        "1e+2",
        "1E-2",
        # Allocation-free integer, exact-power, and disguised-power paths,
        # with adjacent values that must retain the exact fallback.
        "9007199254740992",
        "9007199254740993",
        "18446744073709551615",
        "9007199254740992e-22",
        "9007199254740992e-23",
        "1e22",
        "1e23",
        "1e37",
        "1e38",
        "4.9406564584124654e-324",
        "2.2250738585072014e-308",
        "1.7976931348623157e308",
        "1e309",
        "1e-400",
        "-1e-400",
    ]
    for lower in [
        0x0000000000000000,
        0x0000000000000001,
        0x000FFFFFFFFFFFFF,
        0x3FEFFFFFFFFFFFFF,
        0x3FF0000000000000,
        0x3FF0000000000001,
        0x4330000000000000,
        0x7FDFFFFFFFFFFFFF,
    ]:
        cases.extend(midpoint_triplet(lower))

    maximum = bits_fraction(MAX_FINITE_BITS)
    overflow_midpoint = maximum + Fraction(1 << 970)
    overflow_digits, overflow_exponent = terminating_decimal(overflow_midpoint)
    cases.extend(
        [
            str(maximum.numerator),
            f"{overflow_digits - 1}e{overflow_exponent}",
            f"{overflow_digits}e{overflow_exponent}",
            f"{overflow_digits + 1}e{overflow_exponent}",
        ]
    )

    zero_midpoint_digits, zero_midpoint_exponent = terminating_decimal(
        bits_fraction(1) / 2
    )
    scaled_midpoint = zero_midpoint_digits * 10**60
    cases.extend(
        [
            f"{scaled_midpoint}e{zero_midpoint_exponent - 60}",
            f"{scaled_midpoint + 1}e{zero_midpoint_exponent - 60}",
            "1" + "0" * 1000 + "e-1000",
            "1" + "0" * 1000 + "1e-1001",
            "1e" + "9" * 1000,
            "1e-" + "9" * 1000,
            "-1e-" + "9" * 1000,
        ]
    )

    cases.extend(
        [
            "",
            "-",
            ".",
            "-.",
            "+1",
            "e1",
            "1e",
            "1e+",
            "1e-",
            "1.2.3",
            "1e2e3",
            " 1",
            "1 ",
            "1f64",
            "nan",
            "infinity",
            "+Infinity",
            "-NaN",
            "Infinity ",
            "0x1",
        ]
    )
    return cases


if __name__ == "__main__":
    cases = corpus()
    print("--- stdin ---")
    print("\n".join(cases))
    print("--- stdout ---")
    # The native source appends one embedded-zero case that cannot live in
    # this line-oriented ASCII stdin corpus.
    print("\n".join([*(expected(case) for case in cases), "none"]))

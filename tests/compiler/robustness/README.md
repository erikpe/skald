# Compiler Robustness Corpus

The `frontend/` directory retains small hostile inputs that are clearer as
files than as Rust constructors:

- `malformed.ska` is decoded as ordinary UTF-8 source;
- `arbitrary-bytes.hex` is whitespace-insensitive hexadecimal data decoded to
  bytes and then lossily converted at the compiler's UTF-8 source boundary.

The fixed-seed bounded frontend cases and structured MIR mutations run with
`make compiler-test` as part of the ordinary compiler suite. Run
`make robustness-long` for a larger, less frequent external, scheduled, or
pre-release frontend run; `SKALD_ROBUSTNESS_CASES` controls the case count and
must be a positive integer. Both forms remain deterministic.

Retain a discovered regression at its narrowest owning layer. Add corpus data
here only when the exact source or bytes should be shared or preserved. See
the [testing guide](../../../docs/development/TESTING.md#robustness) for the
general policy.

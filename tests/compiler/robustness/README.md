# Compiler Robustness Corpus

The `frontend/` directory retains small hostile inputs that are clearer as
files than as Rust constructors:

- `malformed.ska` is decoded as ordinary UTF-8 source;
- `arbitrary-bytes.hex` is whitespace-insensitive hexadecimal data decoded to
  bytes and then lossily converted at the compiler's UTF-8 source boundary.

Run the fixed-seed bounded suite with `make robustness-smoke`. Run
`make robustness-long` for the larger scheduled or pre-release case count;
`SKALD_ROBUSTNESS_CASES` controls that count and must be a positive integer.
Both commands also cover structured MIR mutation in Rust.

Retain a discovered regression at its narrowest owning layer. Add corpus data
here only when the exact source or bytes should be shared or preserved. See
the [testing guide](../../../docs/development/TESTING.md#robustness) for the
general policy.

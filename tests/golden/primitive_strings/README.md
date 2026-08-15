# Primitive string fixtures

`values.golden.toml` owns shared default-empty construction, byte-preserving
values, slices, copies, conversions, concatenation, equality, joining,
delimiter splitting, and line splitting across LF, CRLF, and CR separators.
`conversions.golden.toml` owns primitive parsing and formatting observations.
The larger binary64 stdin/stdout corpora remain external byte files and are
generated independently by the scripts in `../oracles/`.

Run this group with `scripts/golden.sh --filter 'primitive_strings/**'`.

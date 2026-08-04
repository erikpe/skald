# Integer String Helper Modules Roadmap

Status: **completed**.

## Goal

Move integer text conversion algorithms out of the `std::str::Str` descriptor
and into focused descendant modules without changing the frozen `Str` API,
text contract, allocation behavior, or failure behavior.

## ISH0 — Extract and verify integer conversion helpers

- [x] Add `std::str::format_integer` with type-named `i64`, `u64`, and `u8`
  formatting entry points.
- [x] Add `std::str::parse_integer` with type-named range-borrowing parsing
  entry points.
- [x] Keep `Str` as the facade and remove its private integer algorithms.
- [x] Extend canonical standard-library test fixtures for both modules.
- [x] Verify the existing integer formatting and parsing corpus.
- [x] Document module ownership and archive this completed roadmap.

Acceptance: all existing integer conversion output and rejection behavior is
unchanged, direct helper range validation is covered, and the full repository
check suite passes.

# Syntax fixtures

`lexing.golden.toml` owns invalid characters and malformed string escapes.
`parsing.golden.toml` owns top-level declaration recovery, missing delimiters,
statement and external-declaration terminators, conditional punctuation, and
rejected import forms. Diagnostics match their stable identity, message, and
primary repository-relative location without freezing recovery context.

Run this group with `scripts/golden.sh --filter 'syntax/**'` and audit
diagnostic determinism with `scripts/golden.sh --determinism compile --filter
'syntax/**'`.

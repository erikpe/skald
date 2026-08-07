# Alias fixtures

`parameters.golden.toml` owns primitive and object alias parameters, mutable
and read-only access, mixed register/stack ABI pressure, produced aliases, and
their rejected source and forwarding forms. Produced-alias lifecycle output
remains an exact external byte trace; rich diagnostics match their stable code,
message, and primary repository-relative location.

Run this group with `scripts/golden.sh --filter 'aliases/**'` and audit
compiler determinism with `scripts/golden.sh --determinism compile --filter
'aliases/**'`.

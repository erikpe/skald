# Array fixtures

`storage.golden.toml` owns primitive, inline, nontrivial, static, indexing, and
ABI-pressure behavior. `views.golden.toml` owns array aliases and slices plus
their rejected rebinding, optional-payload, and whole-pointee operations.
Lifecycle and value traces remain exact external byte expectations.

Run this group with `scripts/golden.sh --filter 'arrays/**'` and audit compiler
determinism with `scripts/golden.sh --determinism compile --filter
'arrays/**'`.

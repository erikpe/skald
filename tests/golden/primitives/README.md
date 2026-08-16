# Primitive value fixtures

The specs in this directory own primitive literals, conversions, explicit box
classes, local bindings, and reassignment. Small expected values are inline;
larger value matrices remain exact external byte files. Compile-fail cases
match the stable diagnostic identity, primary message, and repository-relative
location without freezing renderer context.

Run this group with `scripts/golden.sh --filter 'primitives/**'` and audit
compiler determinism with `scripts/golden.sh --determinism compile --filter
'primitives/**'`.

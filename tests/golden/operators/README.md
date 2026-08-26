# Operator fixtures

These specs cover the primitive operator matrix and value-producing class
operator protocols. They separate arithmetic, comparisons, bitwise and shift
operations, boolean evaluation, evaluation order, cleanup, skipped failure,
and operand failure so observations stay explicit. Native panic stderr uses
stable prefixes that allow future stack traces; compile diagnostics match
their stable leading identity and location.

Run this group with `scripts/golden.sh --filter 'operators/**'` and audit
compiler determinism with `scripts/golden.sh --determinism compile --filter
'operators/**'`.

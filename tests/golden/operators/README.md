# Primitive operator fixtures

These specs separate arithmetic, comparisons, bitwise and shift operations,
and boolean evaluation. Evaluation-order, cleanup, skipped-failure, and
operand-failure programs remain separate sources so their observations stay
explicit. Native panic stderr uses stable prefixes that allow future stack
traces; compile diagnostics match their stable leading identity and location.

Run this group with `scripts/golden.sh --filter 'operators/**'` and audit
compiler determinism with `scripts/golden.sh --determinism compile --filter
'operators/**'`.

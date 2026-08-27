# Operator fixtures

These specs cover the primitive operator matrix and the complete non-generic
class operator surface, including one-call inequality and four direct ordering
predicates. Non-generic call-equivalence coverage exercises produced and
effectful operands, exact/inherited/override/interface dispatch, explicit
shared and optional crossings, primitive alias storage, reverse cleanup, and
panic traces. The fixtures separate arithmetic, comparisons, bitwise and shift
operations, boolean evaluation, evaluation order, cleanup, skipped failure,
and operand failure so observations stay explicit. Native panic stderr uses
stable prefixes that allow future stack traces; compile diagnostics match
their stable leading identity and location.

Run this group with `scripts/golden.sh --filter 'operators/**'` and audit
compiler determinism with `scripts/golden.sh --determinism compile --filter
'operators/**'`.

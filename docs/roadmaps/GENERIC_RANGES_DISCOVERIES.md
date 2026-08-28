# Generic Ranges Follow-up Discoveries

Status: pending after the generic ranges and tight range loops roadmap.

## Suppress dependent body diagnostics for unsatisfied specializations

**Priority:** medium.

**Problem:** A requested generic specialization whose declared bound is
unsatisfied is still closed far enough to diagnose dependent member syntax in
its body. For example, `Range<f64>` correctly reports the missing canonical
`Successor<f64>` evidence, but can additionally report that the specialized
`state.successor()` target is not an object. The latter is a consequence of
the failed bound rather than an independent source error.

**Evidence:** `tests/golden/ranges/failures.ska` requests unsupported primitive
and class applications. Its stable expectations intentionally match the
owning bound diagnostics while allowing the current dependent cascade.

**Likely owner:** generic-specialization validation and body specialization,
including diagnostic provenance for failed closed keys.

**Useful boundary:** Preserve declaration/header validation and one canonical
unsatisfied-bound diagnostic per failed application, but skip or suppress only
body diagnostics whose selected bound witness could not close. Independent
body errors and repeated-request context must remain deterministic. Address
this as a general generic-diagnostic improvement after the active range
roadmap, with existing generic-class and generic-interface failure goldens as
regression coverage.

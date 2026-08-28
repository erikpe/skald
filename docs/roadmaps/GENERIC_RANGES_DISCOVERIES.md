# Generic Ranges Follow-up Discoveries

Status: pending after the generic ranges and tight range loops roadmap.

## Replace syntax-level range request inference with a semantic request pass

**Priority:** high.

**Problem:** Generic class specializations are currently fixed before ordinary
function and class bodies are resolved. RG2 therefore discovers
`Range<T>` requests with a deliberately small source-type closer. It covers
literals, declared locals and parameters, constructors, casts, local function
results, common primitive expressions, and closed generic-template terms, but
cannot in general know the result of imported calls, method calls, or an
overloaded operator whose output differs from its receiver. A semantically
valid range using one of those producers can consequently receive the focused
unsupported-range diagnostic before ordinary endpoint resolution would have
identified `T`.

**Evidence:**
`resolve/resolver/program/specialization/requests/source_request_scanner.rs`
contains the conservative `static_type` bridge. RG2 resolved tests cover the
literal, class-construction, local-call, and closed generic-template request
families.

**Likely owner:** generic-specialization discovery and program phase ordering,
not range syntax or ordinary body resolution.

**Useful boundary:** Add a semantic, diagnostic-suppressing request pass after
ordinary callable/class signatures are available and before specialized
declarations and bodies are materialized. It should resolve endpoint types
with the ordinary expression-selection rules, collect exact `Range<T>` keys,
iterate inner-to-outer for nested ranges, and then perform normal body
resolution once. Remove the syntax-level result inference rather than growing
a second operator, call, inheritance, and method type system. Preserve
source-order diagnostics and specialization provenance at the `..` span. This
should be addressed before RG3 admits concise ranges to executable HIR.

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

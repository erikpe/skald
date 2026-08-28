# Generic Ranges Follow-up Discoveries

Status: pending follow-up work.

## Replace syntax-level range request inference with a semantic request pass

**Priority:** high.

**Problem:** Generic class specializations are currently fixed before ordinary
function and class bodies are resolved. The concise-range frontend therefore
discovers
`Range<T>` requests with a deliberately small source-type closer. It covers
literals, declared locals and parameters, constructors, casts, local and
imported function results, common primitive expressions, and closed
generic-template terms, but cannot in general know the result of method calls
or an overloaded operator whose output differs from its receiver. A
semantically valid range using one of those producers can consequently receive
the focused unsupported-range diagnostic before ordinary endpoint resolution
would have identified `T`.

**Evidence:**
`resolve/resolver/program/specialization/requests/source_request_scanner.rs`
contains the conservative `static_type` bridge. Concise-range resolved tests
cover the
literal, class-construction, local/imported-call, and closed generic-template
request families.

**Likely owner:** generic-specialization discovery and program phase ordering,
not range syntax or ordinary body resolution.

**Useful boundary:** Add a semantic, diagnostic-suppressing request pass after
ordinary callable/class signatures are available and before specialized
declarations and bodies are materialized. It should resolve endpoint types
with the ordinary expression-selection rules, collect exact `Range<T>` keys,
iterate inner-to-outer for nested ranges, and then perform normal body
resolution once. Remove the syntax-level result inference rather than growing
a second operator, call, inheritance, and method type system. Preserve
source-order diagnostics and specialization provenance at the `..` span. The
implemented concise-expression path keeps this as a documented
specialization-discovery limitation rather than
growing a second method and operator type system.

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

## Refine fusion provenance inside specialized generic bodies

**Priority:** low.

**Problem:** Immediate fusion conservatively keeps every range loop in a
specialized generic class body on the protocol path. This correctly excludes
ranges whose endpoint values depend on a generic parameter, but also misses an
optimization for a wholly concrete literal range that happens to appear in the
same body.
Source semantics and eligibility safety are unaffected; only this incidental
optimization opportunity is lost.

**Evidence:** `typeck/function/range_iteration.rs` tests the specialized class
owner before accepting an immediate primitive range. The fusion negative matrix
proves that a `T`-origin range specialized to `u64` remains ordinary.

**Likely owner:** resolved/type-checked expression provenance for specialized
generic bodies, not range lowering or MIR verification.

**Useful boundary:** Record whether each closed endpoint expression depends on
a substituted type or value producer. Permit fusion only when both endpoints
are independently concrete and all other immediate-origin requirements hold.
Do not infer this from the post-substitution primitive type or from source
spelling. Implement only if representative generic code demonstrates a useful
miss; the conservative rule is long-term sound.

## Prune unused canonical range artifacts after fused-only use

**Priority:** medium.

**Problem:** A source module using only an immediately fused range still
activates the canonical `std::range` dependency and emits reachable generic
range declarations and bodies into textual assembly. The system linker removes
nearly all of this fixed material from the executable, and the fused source
function has the intended tight shape, but assembly emission and inspection
carry avoidable noise and work.

**Evidence:** The recorded tight-loop benchmark found range assembly files
roughly 6 KiB larger than their matched `while` files while linked executables
were only 80 bytes larger. Source-function mnemonic profiles were identical
except for the specified cold cleanup jump, and median execution differed by
at most 0.102%, isolating the issue to whole-program artifact retention rather
than the hot loop.

**Likely owner:** closed-world callable/declaration reachability before backend
emission, not range eligibility, HIR lowering, or target instruction selection.

**Useful boundary:** Teach the existing retention analysis to distinguish
language-item declarations needed to validate and type an erased operation
from bodies and metadata still needed by final MIR. Preserve complete
deterministic dumps before erasure, replacement-standard-library validation,
explicit/stored range execution, and any specialization reachable from an
ordinary path. Do not add a range-specific backend filter.

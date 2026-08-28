# Generic Ranges Follow-up Discoveries

Status: pending follow-up work.

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

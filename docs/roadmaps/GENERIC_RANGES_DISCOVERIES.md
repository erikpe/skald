# Generic Ranges Follow-up Discoveries

Status: pending follow-up work.

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

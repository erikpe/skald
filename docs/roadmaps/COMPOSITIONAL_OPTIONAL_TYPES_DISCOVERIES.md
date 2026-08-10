# Compositional Optional Types Discoveries

Status: pending; one lower-priority maintainability follow-up remains.

These findings were recorded while closing the compositional optional type
work. Neither changes the implemented language contract or fixes a known
correctness defect, so they remain separate from the completed feature
roadmap.

## Use payload-neutral names for aggregate optional operations

**Priority:** Medium.

**Status:** Resolved.

**Problem:** HIR and MIR operations named `NestedOptional*` now implement both
nested-optional and optional-array lifecycle. Deterministic dumps consequently
use `nested-optional-*` wording for optional arrays. The operations are correct,
but their historical names no longer describe their complete responsibility.

**Evidence:** The operation family and its lowering, verifier, static-lifecycle,
and backend consumers span more than 60 Rust files. Optional-array MIR tests
currently assert `nested-optional-initialize`, `nested-optional-assign`, and
`nested-optional-cleanup` for array payloads.

**Likely owner:** `crates/skald-compiler/src/hir/ir/optional.rs`,
`crates/skald-compiler/src/mir/model/optional.rs`, and their lowering,
verification, dump, static-lifecycle, and backend consumers.

**Useful boundary:** Rename the generic operation family and payload projection
to `AggregateOptional*` in one behavior-preserving change. Preserve optional
identities, operation order, diagnostics, verification, assembly, and native
behavior; update deterministic dump expectations atomically. Keep genuinely
nested source tests named for nested optionals.

**Resolution:** HIR operands, initialization and assignment operations, MIR
sources, instructions and payload projections, cleanup planning, verification,
static lifecycle, generated array-helper selection, and backend lowering now
use `AggregateOptional*` and `aggregate_optional_*`. HIR and MIR dumps use
`AggregateOptional*` and `aggregate-optional-*`. The genuinely nested
`HirNestedOptionalUnwrap` operation, source diagnostics, generated helper
symbols, instruction order, and native behavior remain unchanged. Focused
dump, verifier, backend, static-lifecycle, and independent-process determinism
tests plus the complete repository and MSRV gates pass.

## Split large optional and array-helper lowering owners

**Priority:** Low.

**Problem:** Target lowering remains correctly encapsulated, but the optional
instruction selector and generated array-helper implementation each combine
several independently understandable lowering responsibilities in one large
file.

**Evidence:** `backend/x86_64_sysv/lower/optional.rs` is about 1,200 lines and
contains shared-owner, class, aggregate, guard, state, and payload-address
lowering. `backend/x86_64_sysv/lower/array/helpers.rs` is about 1,400 lines and
contains initializer, clone, copier, destroyer, release, and element-category
helper generation.

**Likely owner:** `crates/skald-compiler/src/backend/x86_64_sysv/lower/`.

**Useful boundary:** After the payload-neutral operation rename, introduce
private responsibility-named submodules behind the existing `lower` and
`array` facades. Preserve helper symbols, emission order, diagnostics,
instruction sequences, runtime-trace locations, and public module paths. Do
not combine the split with ABI or lifecycle changes.

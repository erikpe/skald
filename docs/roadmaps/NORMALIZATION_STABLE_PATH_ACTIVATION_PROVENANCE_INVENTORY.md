# Normalization-Stable Path-Activation Provenance Inventory

Status: completed NSR0 baseline for the active
[normalization-stable path-activation provenance roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md).

This inventory records the `MirStorageKind` ownership boundary captured before
`NormalizedPathActivation` was added. It remains an implementation aid for
the later roadmap tasks, not a second architecture contract. The frozen
[design](../archive/NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DESIGN_PROPOSAL.md)
and living [phase contract](../compiler/PHASES_AND_IR.md#frozen-normalization-stable-path-activation-direction)
remain authoritative.

## Reproducing the inventory

The implementation manifest comes from repository-local searches over all
`MirStorageKind` references and focused searches for `PathCondition` and
`ScalarSpill`. Tests and fixtures are searched separately so test-only imports
do not hide production owners:

```sh
rg -l 'MirStorageKind' crates/skald-compiler/src
rg -n 'MirStorageKind::PathCondition|MirStorageKind::ScalarSpill' crates/skald-compiler/src
```

The exhaustive classifiers in `mir::verify::contract`, `mir::dump`, and
`mir::rewrite::import` are compile-time maintenance points. The remaining
references are role-specific producers or consumers which require a semantic
audit when the new variant is introduced.

## Ownership manifest

| Concern | Current owner files | Required migration attention |
|---|---|---|
| Model and facade | `mir/model/definition.rs`, `mir/model/mod.rs`, `mir/mod.rs` | Add and document the unit variant; preserve the existing public facade |
| General lowering | `mir/lower.rs`, `mir/lower/array.rs`, `mir/lower/call.rs`, `mir/lower/cleanup.rs`, `mir/lower/expression.rs`, `mir/lower/iteration.rs`, `mir/lower/logical.rs`, `mir/lower/object_values.rs`, `mir/lower/optional.rs`, `mir/lower/optional_box.rs`, `mir/lower/places.rs`, `mir/lower/range_iteration.rs`, `mir/lower/shared.rs` | Lowering must continue producing `PathCondition` and ordinary `ScalarSpill`; it must never produce the final-only kind |
| Phase and declaration verification | `mir/verify/contract.rs`, `mir/verify/body.rs` | The contract classifier owns phase legality; declaration verification owns source/type shape |
| Protocol and use verification | `mir/verify/arguments.rs`, `mir/verify/array/anchor.rs`, `mir/verify/array/ownership.rs`, `mir/verify/array/storage.rs`, `mir/verify/call.rs`, `mir/verify/cleanup.rs`, `mir/verify/final_write.rs`, `mir/verify/function_values/provenance.rs`, `mir/verify/instructions.rs`, `mir/verify/integer_division.rs`, `mir/verify/io.rs`, `mir/verify/lifetime/mod.rs`, `mir/verify/logical.rs`, `mir/verify/optional/initialization/state.rs`, `mir/verify/optional/structural.rs`, `mir/verify/path_conditions.rs`, `mir/verify/place.rs`, `mir/verify/primitive_alias.rs`, `mir/verify/primitive_cast.rs`, `mir/verify/scalar_initialization.rs`, `mir/verify/shared/ownership/transitions.rs`, `mir/verify/shared/structural.rs`, `mir/verify/shift.rs`, `mir/verify/strings.rs`, `mir/verify/type_operations.rs`, `mir/verify/view.rs` | Most checks retain protocol-specific equality tests; scalar initialization is the later consumer of the narrowed exception |
| Mandatory normalization | `passes/pipeline/normalization/plan.rs`, `passes/pipeline/normalization/error.rs` | This remains the sole production conversion owner and must preserve the existing atomic inventory and statistics |
| General rewriting | `mir/rewrite/edit/operations.rs`, `mir/rewrite/error.rs`, `mir/rewrite/import/model.rs`, `mir/rewrite/import/prepare.rs` | Storage-kind edits already require expected/replacement kinds; import classification must handle the final-only kind exhaustively and phase verification must reject it in proof-rich output |
| Final analyses | `passes/pipeline/optimizations/checked_integer_topology.rs`, `passes/pipeline/optimizations/local_constant/carrier.rs`, `passes/pipeline/optimizations/logical_topology.rs`, `passes/redundancy/scalar_spill.rs` | Checked carriers remain restricted to ordinary `ScalarSpill`; read-only redundancy measurement must classify the new role deliberately |
| MIR observation | `mir/dump.rs` | Both the role label and generated-source placeholder are exhaustive and will gain the frozen final spelling |
| Backend layout and legality | `backend/x86_64_sysv/frame.rs`, `backend/x86_64_sysv/array_legality.rs`, `backend/x86_64_sysv/lower/array/anchors.rs` | The new boolean home follows ordinary stack layout; array-only classifiers must reject it by type/role as they reject current scalar storage |
| Test construction support | `mir/test_fixtures.rs`, `mir/test_fixtures/integer_division.rs`, `mir/test_fixtures/primitive_cast.rs`, `mir/test_fixtures/shifts.rs`, `mir/rewrite/edit/test_support.rs` | Synthetic fixtures must be able to express wrong-stage and malformed cases without creating a production constructor |

Compiler tests which mention `MirStorageKind` are distributed across the
lowering, verifier, normalization, analysis, rewrite, dump, and backend test
suites listed above.
Their cohesive owners are the MIR lowering/verification suites, rewrite/import
suites, proof-normalization and optimization pipeline suites, redundancy
measurement suites, and x86-64 backend suites. The roadmap deliberately keeps
new focused tests with those owners rather than creating a cross-cutting test
module.

## Frozen before-state

| Boundary | Current behavior pinned by NSR0 |
|---|---|
| Proof-rich phase | `PathCondition` storage is accepted as the boolean executable carrier with consumable proof |
| Normalized phase | `PathCondition` storage is rejected through the existing normalized-provenance diagnostic |
| Phase-stable kinds | Every other current storage variant, including all alias access and array-anchor payloads, is explicitly classified as legal in both phases |
| Normalization | Each validated activation keeps its `StorageId`, declaration fields, load/store operations, value and block identities, and spans, but its kind becomes `ScalarSpill` |
| Initialization | Both contracts accept an initialized ordinary scalar spill; proof-rich verification rejects an uninitialized one, while the current broad normalized exception accepts it |
| Dump and metrics | Proof-rich dumps say `path-condition`; normalized dumps currently say `scalar-spill`; one logical expression reports exactly one path record, logical record, path read, reclassified activation, and changed callable |
| Backend | A dynamic logical activation receives ordinary stack-frame storage and executes identically and deterministically through the no-optimization and default profiles |

This baseline intentionally records the overly broad normalized spill exception
rather than endorsing it. The representation and verifier tasks will replace
that one row while preserving the remaining rows' executable observations.

## NSR4 rewrite and analysis audit

NSR4 repeated the consumer audit after the final-only role became production
MIR. The result separates proof-rich transformations, normalized CFG
transformations, and whole-definition retention instead of treating every
pass as if it shared one mutation surface.

| Owner | Stage and authority | Normalized-activation disposition |
|---|---|---|
| Dead-pure-definition elimination | Proof-rich value/instruction rewrite | Cannot observe the final-only role. It removes only unused pure value assignments before mandatory normalization and receives no final seal or final CFG capability. |
| Primitive, checked-integer, algebraic, logical, and proof-aware CFG constant consumers | Proof-rich analysis/rewrite or proof-transition plan | Observe `PathCondition` while proof exists. The checked-carrier certificate requires a same-snapshot census entry and declaration whose kind is exactly `ScalarSpill`; `NormalizedPathActivation` is not eligible. |
| Post-proof unreachable-block elimination | Final guarded CFG rewrite | May remove a complete unreachable block and the activation operations owned by that block. It cannot edit a retained storage operation or declaration independently. |
| Post-proof empty-block forwarding | Final guarded CFG rewrite | Redirects edges and removes instruction-empty blocks only, so it neither edits nor relocates activation operations. |
| Post-proof basic-block merging | Final guarded CFG rewrite | May move the complete successor instruction sequence into its sole predecessor. It cannot select, synthesize, retarget, or reorder an individual activation operation. |
| Whole-world reachability analysis | Read-only final analysis | Follows executable references without classifying compiler storage by name, span, topology, or role. It does not mutate MIR. |
| Whole-world reachability retention | Final whole-program retention | Removes complete unreachable executable definitions, including all storage they own; it cannot partially edit a retained definition. The changed product receives a fresh final seal and reachability snapshot. |

The storage-use census now records the semantic kind from the same immutable
callable snapshot as its declarations and uses. Consumers must recompute the
census after rewriting; an identity set from an earlier product is not a role
classifier. Current normalized CFG passes receive only reviewed whole-block
operations. The capability snapshots exact storage declarations around every
callable edit and rejects any creation, deletion, reclassification, or field
change before dense commit. This guard is deliberately redundant with the
narrow API surface so a future storage-mutating capability fails closed until
it defines an explicit normalized-activation policy.

Every changed final CFG or definition-retention outcome is unverified. The
pipeline reseals it before the next observation or pass, repeating normalized
phase legality, activation shape, reference, lifetime, initialization, and
reachability checks. An unchanged pass retains the already verified exact
product and does not manufacture a redundant seal.

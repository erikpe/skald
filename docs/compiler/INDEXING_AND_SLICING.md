# Structural Indexing and Slicing Compiler Contract

Status: **frozen design; neutral source-AST representation implemented,
structural resolution not yet implemented**.

This document owns the compiler representation and phase boundaries for the
planned [structural indexing and slicing language contract](../language/INDEXING_AND_SLICING.md).
The source AST now preserves every bracket projection with type-neutral
vocabulary. Resolution still maps each projection to an array operation, and
type checking rejects non-array receivers. The
[status matrix](../language/STATUS.md) is authoritative for availability.

## Responsibility split

| Layer | Frozen responsibility |
|---|---|
| Syntax | Preserve a type-neutral bracket projection, ordinary versus shared-arrow spelling, index versus slice shape, optional bounds, punctuation spans, and the complete expression span. |
| Resolution | Classify the receiver, retain intrinsic array operations, or select and validate one structural class method or interface requirement. Normalize accepted structural uses to ordinary resolved calls. |
| Type checking and HIR | Apply ordinary receiver access, argument compatibility, optional injection, result typing, ownership, and dispatch to the selected call. Retain no structural-sugar operation. |
| MIR and verification | See only existing direct, virtual, or interface calls and their ordinary storage, lifetime, cleanup, and call invariants. |
| Backend and runtime | Reuse the existing native call paths. Add no collection instruction, descriptor field, vtable shape, witness shape, target ABI rule, or C runtime symbol. |

## Syntax representation

The source AST uses `BracketProjectionExpr`, `BracketProjectionOperator`, and
`BracketProjectionBounds`. This source-only representation retains the
receiver, `[` and `]`, optional `:`, optional `->`, every supplied expression,
and their exact spans. AST dumps use `BracketProjection`; the AST makes no
type, array, or protocol choice.

The implemented grammar already parses every required read and
assignment-shaped bracket form. Structural meaning must not be claimed there
until the corresponding implementation milestone lands.

## Resolution and validation

One cohesive resolver owner centralizes the exact protocol spellings
`index_get`, `index_set`, `slice_get`, and `slice_set`. After resolving the
receiver's static kind it applies this precedence:

1. retain the existing resolved intrinsic operation for an exact array;
2. retain intrinsic projection for a shared array reached through `->` or
   explicit `*`;
3. select an accessible method from an exact class hierarchy;
4. select an exact requirement declared by a static interface; or
5. reject the receiver.

The selector reuses ordinary hierarchy, privacy, explicit-dereference,
generic-specialization, virtual-family, and interface-witness services. It
must not manufacture a synthetic member-access source AST or resolve the
receiver twice.

Before handing the operation to ordinary call checking, resolution validates
the protocol-specific declaration shape: instance versus static, getter
read-only access, setter mutable access and exact `unit` result, arity, exact
optional-`i64` value slice bounds, and permitted value or read-only-alias key
and replacement modes. Getter and setter availability and value types remain
independent.

For structural slicing, resolution creates typed `none` operands for omitted
bounds and uses ordinary one-layer injection for supplied `i64` bounds. For a
write, the replacement remains the final argument. A structural assignment
becomes an ordinary unit-call statement without selecting a getter.

## Lower-phase boundary

Accepted class sugar becomes the same resolved method-call form as an explicit
call. Accepted interface sugar becomes the same resolved interface-call form.
HIR records canonical declaration identities and dispatch metadata, not raw
protocol strings. It contains no structural indexing or slicing node.

Existing call processing therefore owns target-directed arguments and
results, receiver carriers, shared-owner anchors, full-expression cleanup,
reachability, static effects, direct or dynamic dispatch, MIR lowering,
verification, and native ABI realization. Existing sequencing must preserve
receiver first, supplied operands left to right, replacement last, and one
evaluation of every expression.

True arrays remain on their dedicated HIR, MIR, verifier, backend, and failure
paths. Structural work must not weaken or route around any array invariant.

## Diagnostics, dumps, and determinism

Diagnostics should distinguish unsupported read/write index/slice operations
from malformed protocol declarations, inaccessible members, immutable
receivers, missing explicit shared dereference, and ordinary argument or
result incompatibility. Protocol-shape diagnostics belong to selection;
ordinary call diagnostics remain owned by call checking. Punctuation spans
identify the bracket or colon operation without losing operand spans.

AST dumps preserve neutral bracket source shape. Resolved dumps distinguish
intrinsic arrays from selected class or interface calls and show canonical
identities. HIR and MIR dumps show ordinary call targets only. Diagnostics,
dumps, and generated artifacts must remain deterministic.

Implementation order and validation commands belong to the
[active roadmap](../roadmaps/STRUCTURAL_INDEXING_AND_SLICING_ROADMAP.md).

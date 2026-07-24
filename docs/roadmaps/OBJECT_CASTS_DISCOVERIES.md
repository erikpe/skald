# Object Casts Discoveries

Status: pending follow-up items discovered during checked-place implementation.

## Cast-relative receiver paths

**Priority:** medium.

Name resolution currently keeps ordinary receiver tests and downstream member
selection stable by representing every `ResolvedObjectReceiver` with an
`ObjectPath`. A cast over a produced inline object has no source binding, so
its path root is an explicitly never-lowered sentinel while the semantic cast
source and post-cast projections are carried separately.

The sentinel is contained: type checking selects the checked cast source,
access, origin, target, and projections without checking or lowering that root.
It nevertheless makes the resolved representation less self-describing than
the HIR/MIR representation.

When owning cast consumers add more cast-rooted place operations, replace this
with a resolved receiver base that distinguishes a stable binding path from a
cast-relative projection path. Preserve direct field/member selection and
existing ordinary-place test ergonomics without manufacturing binding
identity for produced storage.

**Likely owner:** `resolve/ir/object_place.rs` and
`resolve/resolver/body/place.rs`, coordinated with the owning-cast integration
task.

## Control-effect discovery during MIR lowering

**Priority:** low.

Runtime checked casts introduce CFG edges inside expressions while MIR scalar
values deliberately remain block-local. Lowering therefore detects later
runtime casts and spills earlier scalar operands/arguments into explicit
`ScalarSpill` storage before crossing the edge.

The current recursive discovery is small and exhaustive over the current HIR
expression surface. If another expression-level operation later introduces
control flow, move this property onto a shared HIR control-effect query rather
than adding a second independent traversal.

**Likely owner:** HIR expression utilities and MIR expression/call lowering.

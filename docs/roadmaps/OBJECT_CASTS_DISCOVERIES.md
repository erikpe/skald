# Object Casts Discoveries

Status: one pending representation follow-up discovered during checked-place
implementation.

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

When the cast representation is next simplified, replace this with a resolved
receiver base that distinguishes a stable binding path from a cast-relative
projection path. Preserve direct field/member selection, owning cast-field
sources, and existing ordinary-place test ergonomics without manufacturing
binding identity for produced storage.

**Likely owner:** `resolve/ir/object_place.rs` and
`resolve/resolver/body/place.rs`.

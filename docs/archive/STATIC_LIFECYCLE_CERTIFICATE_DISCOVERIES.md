# Resolved Static-Lifecycle Certificate Follow-up Discoveries

Status: resolved and archived.

This record preserves the implementation finding discovered while building
static-lifecycle certificate analysis fixtures. The living mutable static-field
contract remains authoritative for current behavior.

## Direct exact-class static assignment reaches an internal panic

**Resolution:** Mutable direct exact-class statics support source replacement,
as required by the existing stored-value and copy-assignment contracts. Type
checking now selects the class's exact copy-assignment capability and carries a
receiver-free static replacement through dedicated HIR into the ordinary MIR
`CopyAssign` operation. MIR verification, static-effect inference, lifecycle
planning, and the backend reuse that operation with a static destination.

Focused type-check and MIR tests cover named, produced, and self sources. The
static-lifecycle analysis fixture proves that initializer-reachable replacement
is retained as a `Replace` root effect, and the static-field native golden
executes produced replacement of an initialized exact-class slot. Final static
root assignment remains rejected before statement-family selection.

**Original evidence:** A well-formed source shape with an initialized
exact-class static and a later assignment such as `State.item = replacement`
reached the fallback
`unreachable!("enabled static storage type must have a statement family")` in
`typeck/function/statement.rs`. Primitive, optional-class, shared,
optional-shared, and array static assignments had explicit statement families,
but a direct exact-class static fell through the exhaustive-looking dispatch.

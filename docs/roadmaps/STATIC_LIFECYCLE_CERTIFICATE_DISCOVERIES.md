# Static-Lifecycle Certificate Follow-up Discoveries

Status: pending follow-up outside the active certificate redesign roadmap.

This record holds implementation findings that do not change the frozen
certificate design and should not expand an active PR-sized task.

## Direct exact-class static assignment reaches an internal panic

**Priority:** Medium correctness and robustness follow-up after the certificate
roadmap.

**Evidence:** A well-formed source shape with an initialized exact-class static
and a later assignment such as `State.item = replacement` reaches the fallback
`unreachable!("enabled static storage type must have a statement family")` in
`typeck/function/statement.rs`. Primitive, optional-class, shared, optional-
shared, and array static assignments have explicit statement families, but a
direct exact-class static falls through the exhaustive-looking dispatch.

**Likely owner:** Static-field assignment capability selection in type checking,
plus the corresponding HIR/MIR replacement carrier if the operation is meant
to be supported.

**Useful boundary:** First decide from the existing stored-value and copy-
assignment contracts whether mutable direct exact-class statics support source
replacement. If supported, reuse the selected class assignment and destruction
plans through HIR, MIR, verification, lifecycle inference, and backend tests. If
not supported, reject the assignment with a typed source diagnostic. In either
case, add a regression proving this source shape never panics. Do not fold that
language/capability decision into normalized lifecycle-root analysis.

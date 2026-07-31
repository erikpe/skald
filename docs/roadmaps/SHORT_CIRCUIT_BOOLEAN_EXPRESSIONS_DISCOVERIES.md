# Short-Circuit Boolean Expressions Discoveries

Status: pending follow-up after the completed short-circuit implementation.

## Split shared ownership verification by state responsibility

- **Problem:** `mir/verify/shared/ownership.rs` owns allocation publication,
  ordinary and static owners, transfers, call handoffs, field initialization,
  shared provenance, checked-view lifetime, full-expression state, and
  path-sensitive fixed-point propagation in one module.
- **Evidence:** Short-circuit ownership work made the existing state
  path-sensitive without weakening ordinary joins. The module now exceeds
  1,000 lines, and its instruction transition logic is independent from CFG
  alternative selection and condition convergence.
- **Likely owner:** MIR shared-ownership verification.
- **Priority:** Medium, after the short-circuit representation and verifier
  contracts stabilize.
- **Useful boundary:** Keep the existing `shared` verifier facade and exact
  diagnostics. Separate path-sensitive propagation from owner/allocation
  transitions and shared-place/checked-view use checks; keep `SharedState`
  private to the resulting sibling modules.

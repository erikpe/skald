# Generic Interfaces Discoveries

Status: pending follow-up. The closure audit found no high-priority ownership
defect: public phase facades remain narrow coordinators or explicit data-model
exports, template body resolution is one cohesive AST visitor, specialization
coordination owns one worklist/state machine, and lower phases retain no
generic-interface implementation concern. The bounded opportunities below do
not block the implemented contract.

## Separate syntax closing from source request discovery

- **Problem:** Generic specialization request collection has two distinct
  traversal responsibilities in one implementation module: closing syntax
  types under a substitution and discovering application requests throughout
  source declarations and expressions.
- **Evidence:** `specialization/requests.rs` contains the `SyntaxTypeCloser`
  implementation and its diagnostics followed by the independent
  `SourceRequestScanner` AST traversal; the module is roughly 700 lines and
  changes to either traversal require navigating the other.
- **Likely owner:** The specialization request facade, with private
  `syntax_type_closer` and `source_request_scanner` implementation modules.
- **Priority:** Low; the responsibilities are internally coherent and no
  correctness or testability defect is known.
- **Boundary:** Move the two private implementations without changing the
  public request API, diagnostic ordering, traversal order, or specialization
  coordinator. Keep this separate from semantic feature work because it is a
  structural move rather than a correctness fix.

## Diagnose unsupported operators before object-result copying

- **Problem:** An operator expression over a class-specialized generic type is
  correctly rejected, but type checking can report that the expression is not
  an existing object place rather than identifying the unsupported operator.
- **Evidence:** `generic_interface_bounds_do_not_enable_operators` specializes
  `left + right` with an exact class satisfying an ordinary `Add<T>` bound and
  receives `TYP014` from object copy-source validation.
- **Likely owner:** Generic expression specialization and the type-checker
  boundary that selects primitive binary operations before object-result
  materialization.
- **Priority:** Medium; behavior and safety are correct, but the source
  diagnostic points at a downstream object-copy consequence.
- **Boundary:** Reject an unselected binary operator at its source operation
  before result-context processing, retaining the existing primitive operator
  matrix and without treating interface bounds as operator protocols.

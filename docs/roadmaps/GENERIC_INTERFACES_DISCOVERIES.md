# Generic Interfaces Discoveries

Status: pending follow-up. The closure audit found no high-priority ownership
defect: public phase facades remain narrow coordinators or explicit data-model
exports, template body resolution is one cohesive AST visitor, specialization
coordination owns one worklist/state machine, and lower phases retain no
generic-interface implementation concern. The bounded opportunity below does
not block the implemented contract.

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

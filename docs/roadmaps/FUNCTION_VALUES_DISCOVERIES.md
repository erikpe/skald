# Function-Value Implementation Discoveries

This document records maintainability work discovered while implementing the
active [function-values roadmap](FUNCTION_VALUES_ROADMAP.md) that is useful but
outside the current task boundary.

## Centralize recursive resolved-type source rendering

**Priority:** medium, preferably before generic function-type substitution
substantially expands the type graph.

**Problem and evidence:** Recursive source-facing resolved-type rendering is
implemented separately in `resolve/dump.rs`, generic specialization naming,
and generic specialization validation. Adding `FunctionTypeId` required three
parallel recursive implementations for parameter modes, children, and result
types. Their surrounding output policies differ, but their traversal and
function-signature spelling are the same, creating a drift risk as later
roadmap tasks add template and specialized function terms.

**Likely owner:** the resolved-IR facade, with narrow naming policies supplied
by the dump and specialization consumers.

**Useful boundary:** introduce one cycle-aware recursive resolved-type name
renderer that owns arrays, optionals, shared targets, function signatures, and
parameter-mode spelling. Keep dump structure, ID-oriented debug labels, module
qualification policy, and diagnostic phrasing in their current consumers.
Treat this as a focused refactor with snapshot coverage rather than coupling it
to callable-reference semantics.

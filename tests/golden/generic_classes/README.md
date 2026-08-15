# Generic class goldens

This suite freezes source-facing failures at template definition and generic
application sites. Multi-module cases additionally cover argument lookup and
template privacy at their respective source contexts.

Native cases exercise the public compiler and runtime path for closed
construction, copy and assignment, cleanup, optional arrays, shared owners,
per-application statics, bound-selected interface dispatch, checked failure,
and definition/application module separation. Explicit generic copy lifecycle
additionally covers array-specialized fields across primitive, owning-element,
optional-array, and recursively nested generic values.

The dot-selection module fixture composes generic static-field reads and
writes, direct and function-valued static methods, template-local selection,
distinct specializations, inherited members, declaring-class privacy, nested
generic closers, module qualification, and produced generic receivers. Focused
failures freeze private selection and the rejected legacy `>::` separator.

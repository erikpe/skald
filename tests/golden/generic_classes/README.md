# Generic class goldens

This suite freezes source-facing failures at template definition and generic
application sites. Multi-module cases additionally cover argument lookup and
template privacy at their respective source contexts.

Native cases exercise the public compiler and runtime path for closed
construction, copy and assignment, cleanup, optional arrays, shared owners,
per-application statics, bound-selected interface dispatch, checked failure,
and definition/application module separation.

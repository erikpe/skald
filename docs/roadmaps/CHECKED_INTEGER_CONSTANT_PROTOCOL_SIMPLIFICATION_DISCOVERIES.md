# Checked Integer Constant Protocol Simplification Discoveries

Status: open follow-up record for the active
[checked integer constant protocol simplification roadmap](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md).

Use this file for concrete maintainability findings or optimization
opportunities discovered while implementing the roadmap that do not belong in
its reviewed FMC-01/FMC-02 scope. Each finding should record the problem,
implementation evidence, likely owner, priority, and a bounded future
direction.

Do not duplicate the complete
[optimization candidate catalog](OPTIMIZATION_CANDIDATE_CATALOG.md). The
catalog owns concise cross-domain placement, effort, value, prerequisite, and
status summaries. This record owns implementation-specific evidence needed to
make a later task reviewable.

Expected but not pre-approved topics include direct folding of statically
failing protocols, eliminating a successful check around a dynamic operation,
nested checked-constant propagation, redundant private scalar-spill cleanup,
proof-provenance normalization, and broader checked scalar families. Recording
a topic here does not add it to the active roadmap.

No implementation discovery has been recorded yet.

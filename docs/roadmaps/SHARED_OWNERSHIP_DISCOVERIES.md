# Shared-Ownership Maintainability Discoveries

Status: pending shared-ownership phase-test organization follow-up.

The remaining item is maintainability work rather than missing language
behavior.

## Partition shared-ownership phase tests

**Priority:** low.

The type-check and MIR shared-ownership test files have grown into broad
milestone accumulations. Their coverage is valuable, but locating the owner of
a regression requires scanning unrelated call, field, cast, anchor, and copy
allocation cases.

Split each test module by semantic responsibility—core owners, calls/results,
fields, casts/views, anchors, and copy allocation—while retaining private
helpers only in the narrowest common parent. Keep native behavior in the
backend shared-ownership suite and complete source behavior in golden tests.
No production API or diagnostic change should accompany this reorganization.

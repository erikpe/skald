# Range fixtures

These fixtures cover the canonical successor protocol, ordinary generic range
class, concise range expressions, and immediate primitive fusion. Native
coverage proves explicit and concise primitive and class iteration, stored
concise ranges, exact endpoint order, equal/descending/maximum bounds,
continue/break/return, mixed nesting, and fused-body panic attribution without
a new runtime service.

Run this group with `scripts/golden.sh --filter 'ranges/**'`.

# Shared-ownership fixtures

These specs separate ordinary ownership, guarded places and allocation, array
elements and outer storage, and polymorphic or cyclic graphs. Exact stdout
files preserve allocation counts, anchor lifetime, element cleanup, last-owner
finalization, and cycle behavior. ABI, graph, guarded-place, and lifecycle
programs remain distinct sources so combining cases cannot hide their
ownership boundaries. Checked-place coverage includes copying inline fields
through stable, replaceable, and produced shared-pointee receivers.

Run this group with `scripts/golden.sh --filter 'shared_ownership/**'`. Use
`scripts/golden.sh --determinism full --filter 'shared_ownership/**'` for a
complete graph and lifecycle audit.

# Structural bracket fixtures

`dispatch.golden.toml` owns native direct, virtual, interface, shared-interface,
produced, and static-field dispatch for structural index and slice reads and
assignments.

`ownership.golden.toml` owns primitive and owning value families, target-directed
copy and adoption, self-aliasing replacements, receiver/operand/call order,
reverse cleanup, and failure-prefix ordering.

Run this group with `scripts/golden.sh --filter 'structural_indexing/**'`.

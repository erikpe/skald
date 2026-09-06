# Array fixtures

`storage.golden.toml` owns primitive, inline, nontrivial, static, indexing, and
ABI-pressure behavior. `element_lists.golden.toml` owns every executable
element family, both outer ownership modes, lifecycle traces, and deliberate
syntax failures for inferred lists and invalid separators.
`indexed.golden.toml` owns executable primitive indexed construction across
zero, one, and many lengths, every primitive element type, both outer
ownership modes, index-dependent effects, and postfix observation. It also
owns exact-class direct placement, copying, grouping, call results, ordinary
array consumers, shared outer backing, and reverse destruction, plus optional
presence/payload completion, jagged nested indexed construction, exact row
copy versus produced-backing adoption, and recursive reverse destruction.
It also owns shared exact, interface, `Obj`, and shared-array targets; named
retention versus produced adoption; optional-owner absence and presence;
receiver anchors; independent shared outer ownership; and last-owner cleanup.
`views.golden.toml` owns array aliases and slices plus
their rejected rebinding and whole-pointee operations, and the core
optional-array function-value boundary.
Lifecycle and value traces remain exact external byte expectations.

Run this group with `scripts/golden.sh --filter 'arrays/**'` and audit compiler
determinism with `scripts/golden.sh --determinism compile --filter
'arrays/**'`.

# Object-model fixtures

The specs in this directory separate inline storage, object values,
initialization, deterministic lifecycle, casts, and member access. Exact
external stdout files preserve construction, evaluation, copy, return,
destruction, and dispatch order. ABI-pressure, cleanup, selected-failure, and
full-expression programs remain separate sources so their ownership boundaries
stay visible.

Native cast failure matches its stable panic prefix and allows future stack
traces. Compile failures match the diagnostic identity, primary message, and
primary repository-relative location without freezing richer renderer context.

Run this group with `scripts/golden.sh --filter 'objects/**'`. Use
`scripts/golden.sh --determinism full --filter 'objects/**'` for a complete
compiler-and-runtime lifecycle audit.

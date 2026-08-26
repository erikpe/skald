# Control-flow fixtures

The specs in this directory own conditional and loop behavior, including
branch scope, parser failures, `while`, nominal `Iterable` iteration, break,
continue, lifecycle cleanup, and typed conditions. Lifecycle and
nested-control sources remain focused so exact effect traces are not hidden
behind selector programs.

Run this group with `scripts/golden.sh --filter 'control_flow/**'`. Use
`scripts/golden.sh --determinism full --filter 'control_flow/**'` for a full
compiler-and-runtime audit of effectful control flow.

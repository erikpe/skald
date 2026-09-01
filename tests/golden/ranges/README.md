# Range fixtures

These fixtures cover the canonical successor protocol, ordinary generic range
class, concise direct loop sources, and immediate primitive fusion. Native
coverage proves explicit values and concise primitive and class iteration,
exact endpoint order, equal/descending/maximum bounds,
continue/break/return, mixed nesting, and fused-body panic attribution without
a new runtime service.

Run this group with `scripts/golden.sh --filter 'ranges/**'`.

## Conformance ownership

| Fixture | Contract evidence |
|---|---|
| `successor` | primitive successor intrinsics, wrapping manual use, class witnesses |
| `explicit_primitives` | ordinary explicit/stored/copied/argument/result primitive ranges and boundary values |
| `explicit_classes` | class ordering/successor effects, lifecycle, advance-before-body, and loop exits |
| `concise_ranges` | fused primitive syntax, explicit stored/argument/result ranges, direct class syntax, grouped endpoints, ordered endpoint effects, equal/descending/maximum bounds, return, nesting, and mixed fused/unfused execution |
| `concise_range_failure` | fused body failure and runtime-trace attribution |
| `direct_source_only` | exact diagnostics for stored, argument, result, and grouped-complete concise ranges |
| `failures` | malformed or unsupported canonical bounds, types, capabilities, and syntax diagnostics |

Compiler-owned HIR/MIR mutation, shape, provider-reordering, pipeline-
determinism, backend, runtime-symbol, ABI, and robustness tests complement
these source-to-native observations. The performance corpus is deliberately
separate under `tests/benchmarks/range_loop` because wall time is not a golden
or correctness gate. The complete cross-layer mapping is maintained in the
[range conformance matrix](../../../docs/compiler/RANGES_TEST_MATRIX.md).

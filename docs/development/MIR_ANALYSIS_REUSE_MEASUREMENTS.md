# MIR Analysis Reuse Measurements

Status: authoritative evidence for the staged proof-snapshot local constant
reuse experiment. The implementation and decision sequence is owned by the
[MIR snapshot analysis reuse roadmap](../roadmaps/MIR_SNAPSHOT_ANALYSIS_REUSE_ROADMAP.md).

## Uncached Gate 1 baseline

Gate 1 was measured with memoization disabled. Every request therefore ran the
local constant solver, every request count equals its computation count, and
all result-table counts are zero.

The baseline was captured from repository revision
`c40cb9a132c29675c72bdebf56a5921c0fad09b4` with a dirty working tree containing
the S01 instrumentation. It used the optimized assertion-enabled `golden`
compiler, the default 17-occurrence MIR schedule, no MIR exclusions, three
assembly compilations per workload, one native warmup, five measured native
runs, and a 30-second child-process timeout. The reviewed inputs and their
source inventories are the complete workload matrix defined by the
[cleanup measurement procedure](CLEANUP_MEASUREMENTS.md).

Commands:

```text
make measurement-support-test
make cleanup-baseline
```

Raw artifacts:

- `build/measurements/cleanup-baseline/run-599voa4p/deterministic.json`
- `build/measurements/cleanup-baseline/run-599voa4p/report.json`

These paths are intentionally ignored build evidence. Rerunning the command
creates a new unique directory rather than overwriting the recorded run.

| Workload | Requests / computations | Repeated | Distinct snapshot keys | Compile median ± MAD (ms) | Peak RSS median (KiB) |
| --- | ---: | ---: | ---: | ---: | ---: |
| `compile/small-source` | 8 / 8 | 7 | 1 | 2.321 ± 0.217 | 8724 |
| `compile/many-modules-large-cfg` | 3744 / 3744 | 936 | 2808 | 1623.093 ± 28.733 | 74048 |
| `compile/many-generic-applications` | 264 / 264 | 231 | 33 | 6.687 ± 0.099 | 10260 |
| `compile/nested-ownership` | 4304 / 4304 | 2690 | 1614 | 1060.724 ± 1.324 | 59212 |
| `native/generic-vector-growth` | 1168 / 1168 | 730 | 438 | 472.917 ± 3.650 | 29260 |
| `native/range-u8-range` | 32 / 32 | 28 | 4 | 4.660 ± 0.053 | 9576 |
| `native/range-u8-while` | 8 / 8 | 7 | 1 | 3.423 ± 0.001 | 9220 |
| `native/range-u64-range` | 32 / 32 | 28 | 4 | 3.975 ± 0.044 | 9488 |
| `native/range-u64-while` | 8 / 8 | 7 | 1 | 2.806 ± 0.053 | 9184 |
| `native/range-i64-range` | 32 / 32 | 20 | 12 | 4.410 ± 0.035 | 9508 |
| `native/range-i64-while` | 8 / 8 | 5 | 3 | 2.924 ± 0.023 | 9176 |
| `native/runtime-trace-call-recursion-enabled` | 16 / 16 | 14 | 2 | 3.137 ± 0.111 | 9384 |
| `native/runtime-trace-call-recursion-omitted` | 16 / 16 | 14 | 2 | 3.012 ± 0.054 | 9368 |
| `native/runtime-trace-tight-loop-enabled` | 8 / 8 | 7 | 1 | 4.459 ± 0.127 | 9180 |
| `native/runtime-trace-tight-loop-omitted` | 8 / 8 | 7 | 1 | 4.322 ± 0.139 | 9236 |
| `native/runtime-trace-allocation-enabled` | 16 / 16 | 14 | 2 | 3.288 ± 0.081 | 9344 |
| `native/runtime-trace-allocation-omitted` | 16 / 16 | 14 | 2 | 3.085 ± 0.052 | 9460 |
| `native/runtime-trace-representative-golden-enabled` | 1176 / 1176 | 441 | 735 | 812.179 ± 1.853 | 32748 |
| `native/runtime-trace-representative-golden-omitted` | 1176 / 1176 | 441 | 735 | 808.261 ± 1.803 | 32860 |

Across the matrix, the uncached boundary recorded 12,040 requests and
computations, 6,399 distinct callable-snapshot keys, and 5,641 repeated
same-snapshot requests. The latter are potentially avoidable computations;
their actual cost and retained-memory tradeoff remain subjects of Gate 2.

The smallest workload provides an exact Gate 1 witness. Its first request was
`primitive-constant-folding#0` at schedule position 1. The same callable and
unchanged snapshot were then requested by
`primitive-algebraic-simplification#0` at position 2,
`primitive-constant-folding#1` at position 3,
`checked-integer-constant-folding#0` at position 5,
`checked-f64-to-integer-constant-folding#0` at position 6,
`primitive-constant-folding#2` at position 7,
`conservative-cfg-cleanup#0` at position 9, and
`constant-short-circuit-folding#0` at proof-transition position 11. Each
occurrence made one request; the first established the one distinct key and
the next seven were same-snapshot repetitions.

## Gate 1 decision

**Go.** The maintained workload matrix demonstrates same-callable repeated
local constant requests without an intervening changed outcome or proof
normalization boundary. S02 may establish the snapshot-bound session and
complete-invalidation contract while production remains uncached.

This decision does not establish a speedup. The operational observations above
are an uncached reference only; Gate 2 requires paired cache-disabled and
cache-enabled comparisons under its stricter correctness, timing, and RSS
criteria.

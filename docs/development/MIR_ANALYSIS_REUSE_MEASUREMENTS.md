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

## Snapshot-bound session contract

The pipeline now owns a lazy callable-keyed proof-rich session beside the
verified MIR product. Successful local constant results use immutable shared
handles. An unchanged pass preserves the session; every changed pass clears
the complete table before output verification; and proof normalization clears
it unconditionally. Unknown executable identities fail before a solver request
is recorded. No session or result handle appears in pass outcomes, inspectors,
final-stage MIR, or the public compiler API.

A private test policy exercises memoized and measurement-only execution through
the same runner and callbacks. Production selected memoization after the paired
experiment below passed Gate 2. Analysis observations include cache hits, entries
present before each occurrence, successful insertions, and entries discarded
by complete invalidation. Consequently the original Gate 1 trace has an
implicit hit count of zero and remains the authoritative uncached baseline.

## Memoized Gate 2 comparison

The bounded production experiment compared the preserved S02 measurement-only
compiler with the S03 memoized compiler. Both are optimized,
assertion-enabled `golden` binaries built with the same toolchain and source
checkout. Reports identify them independently of the dirty checkout metadata:

- measurement-only SHA-256:
  `022af3fd6952c1dbb9b484f4112138a9c6113a8b7c0c2a48a3013dd952807ca0`;
- memoized SHA-256:
  `ac86868f99713734421ea82c76d0dba1fab9edca2ac551c4a442c59784e57ec6`.

Every run used all 19 reviewed workloads, the default 17-occurrence schedule,
three compiler samples, one native warmup, five native samples, the workload's
recorded runtime-trace policy, and a 30-second timeout. Pair order alternated to
reduce ordering bias: memoized/measurement-only, measurement-only/memoized,
then memoized/measurement-only for the required noisy-result tie-break.

Raw reports:

| Pair | Measurement-only | Memoized |
| --- | --- | --- |
| 1 | `build/measurements/cleanup-baseline/run-lucrlz6h/report.json` | `build/measurements/cleanup-baseline/run-cjjs43q3/report.json` |
| 2 | `build/measurements/cleanup-baseline/run-wlzlhuw4/report.json` | `build/measurements/cleanup-baseline/run-57ka5suz/report.json` |
| 3 | `build/measurements/cleanup-baseline/run-ef74x67o/report.json` | `build/measurements/cleanup-baseline/run-usaz5nvg/report.json` |

The final production `make cleanup-baseline` validation is retained at
`build/measurements/cleanup-baseline/run-84mqsu00/report.json`.

The following calculations use `Δ = measurement-only median - memoized
median`, so a positive value favors memoization. `band` is the sum of the two
median absolute deviations. A timing result crosses the gate only when the
absolute delta exceeds that band. The RSS column is the largest memoized
median percentage increase across the three pairs.

| Workload | Computations and reduction | Pair 1 Δ / band (ms) | Pair 2 Δ / band (ms) | Pair 3 Δ / band (ms) | Maximum RSS increase |
| --- | ---: | ---: | ---: | ---: | ---: |
| `compile/small-source` | 8→1 (87.5%) | +0.159 / 0.060 | -0.014 / 0.012 | +0.016 / 0.017 | +2.34% |
| `compile/many-modules-large-cfg` | 3744→2808 (25.0%) | +65.195 / 1.329 | +37.418 / 16.800 | +60.998 / 47.991 | +0.24% |
| `compile/many-generic-applications` | 264→33 (87.5%) | +0.769 / 0.087 | +0.730 / 0.510 | +0.982 / 0.286 | +0.12% |
| `compile/nested-ownership` | 4304→1614 (62.5%) | +200.526 / 20.624 | +216.232 / 10.577 | +214.817 / 14.308 | +0.77% |
| `native/generic-vector-growth` | 1168→438 (62.5%) | +100.874 / 2.150 | +115.334 / 14.479 | +108.170 / 0.575 | +0.37% |
| `native/range-u8-range` | 32→4 (87.5%) | +0.703 / 0.413 | +0.305 / 0.290 | +0.730 / 0.394 | -0.13% |
| `native/range-u8-while` | 8→1 (87.5%) | +0.471 / 0.318 | +0.565 / 0.222 | +0.268 / 0.082 | +1.13% |
| `native/range-u64-range` | 32→4 (87.5%) | +0.335 / 0.166 | +0.490 / 0.146 | +0.110 / 0.281 | +0.42% |
| `native/range-u64-while` | 8→1 (87.5%) | +0.286 / 0.305 | -0.106 / 0.091 | +0.298 / 0.064 | +1.01% |
| `native/range-i64-range` | 32→12 (62.5%) | +0.255 / 0.093 | +0.128 / 0.120 | +0.347 / 0.133 | +0.84% |
| `native/range-i64-while` | 8→3 (62.5%) | +0.180 / 0.050 | -0.178 / 0.189 | +0.180 / 0.249 | +0.30% |
| `native/runtime-trace-call-recursion-enabled` | 16→2 (87.5%) | +0.271 / 0.041 | +0.307 / 0.108 | +0.246 / 0.103 | +1.71% |
| `native/runtime-trace-call-recursion-omitted` | 16→2 (87.5%) | +0.069 / 0.075 | +0.087 / 0.402 | +0.377 / 0.065 | +0.09% |
| `native/runtime-trace-tight-loop-enabled` | 8→1 (87.5%) | +0.436 / 0.176 | +0.811 / 0.148 | +0.669 / 0.170 | +1.11% |
| `native/runtime-trace-tight-loop-omitted` | 8→1 (87.5%) | +0.896 / 0.052 | +0.401 / 0.171 | +0.881 / 0.190 | +0.91% |
| `native/runtime-trace-allocation-enabled` | 16→2 (87.5%) | +0.026 / 0.303 | +0.509 / 0.137 | +0.050 / 0.186 | +0.98% |
| `native/runtime-trace-allocation-omitted` | 16→2 (87.5%) | -0.041 / 0.085 | +0.368 / 0.268 | -0.126 / 0.068 | +1.76% |
| `native/runtime-trace-representative-golden-enabled` | 1176→735 (37.5%) | +77.382 / 3.939 | +75.239 / 7.284 | +75.198 / 6.732 | +0.78% |
| `native/runtime-trace-representative-golden-omitted` | 1176→735 (37.5%) | +64.620 / 2.632 | +58.746 / 8.231 | +67.582 / 11.209 | +0.27% |

Across the matrix, all three memoized runs recorded exactly 12,040 requests,
6,399 computations, 5,641 hits, 6,399 successful insertions, and 6,399
discarded entries. Every workload reduced computations by at least 25 percent.
The large-CFG, nested-ownership, generic-vector, and representative-golden
workloads each improved beyond combined dispersion in all three pairs. The
small-source, `u64`-while, and allocation-omitted noise produced isolated
crossings, but no workload produced an adjusted regression in two runs. The
largest median RSS increase was 2.34 percent.

The deterministic projections matched after excluding the expected compiler
identity and analysis-usage differences. This comparison covers source
inventories, compiler arguments, the complete ordered pass outcome sequence,
assembly sizes and hashes, executable sizes, and native status/stdout/stderr
digests for every workload. Colocated policy-equivalence tests additionally
require identical typed local constant solutions, final MIR, pass-owned
measurements, verification counts, diagnostics, and assembly.

## Gate 2 decision

**Go.** Correctness parity is complete, deterministic computations fall by
25–87.5 percent, multiple nontrivial workloads show repeatable improvements
beyond dispersion, no workload has a repeatable adjusted regression, and peak
RSS remains below the five-percent limit. S04 must take the Gate 2 go branch:
retain the typed production session, remove only comparison scaffolding without
regression value, and close A19 with the bounded measured claim.

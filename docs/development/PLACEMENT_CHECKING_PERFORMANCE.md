# Placement Checking Performance

Status: maintained baseline and measurement protocol for the placement-checking
scalability roadmap. Timings are observational evidence, not test assertions.
The accepted scope and thresholds live in the
[design proposal](../roadmaps/PLACEMENT_CHECKING_SCALABILITY_DESIGN_PROPOSAL.md)
and its [implementation roadmap](../roadmaps/PLACEMENT_CHECKING_SCALABILITY_ROADMAP.md).

## Reproducing the measurement

Run the complete witness matrix from the repository root:

```sh
make placement-checking-benchmark
```

The Make target accepts additional script arguments through
`PLACEMENT_CHECKING_ARGS`. For example, this measures one witness while
developing an owner-local change:

```sh
make placement-checking-benchmark \
  PLACEMENT_CHECKING_ARGS='--witness recursive-optionals --repeats 2'
```

The command prebuilds the compiler tests, then runs every selected test in an
isolated process with one test thread. It alternates witness order across
repetitions and writes `deterministic.json` and `report.json` to a unique,
ignored directory below `build/measurements/placement-checking/`. The report
records the repository revision and dirty state, Cargo profile, host identity,
warm-up and repetition counts, timeout, raw samples, medians, median absolute
deviations and deterministic structural observations. Use at least two
repetitions; the baseline deliberately used no warm-up because the test binary
was prebuilt and every witness starts in a new process.

The measurement environment enables a test-only observer. It separates native
pilot planning, discovery lowering, executable lowering, selection, placement
production, placement checking, frame planning, realization/checking and
publication. Link and native execution time are reported separately. Ordinary
compiler execution does not emit timing records.

## PS01 baseline

The baseline was captured on 2026-09-20 with revision `9eafca80` plus the
uncommitted PS01 observation machinery, using the unoptimized Cargo test profile
on x86-64 WSL2 Linux with 16 logical CPUs, Python 3.12.3, zero warm-ups and two
alternating repetitions. The retained local report is
`build/measurements/placement-checking/run-8e3lj1eg/report.json`; generated
reports are intentionally ignored and should not be treated as repository
artifacts.

| Witness | Discovery | Wall median | Check median | Check share of pilot | Production median | Peak RSS median |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `recursive-optionals` | D01 | 46.59 s | 45.54 s | 99.52% | 28.28 ms | 117.9 MiB |
| `primitive-array-allocation` | D02 | 39.93 s | 39.62 s | 99.87% | 6.78 ms | 225.5 MiB |
| `array-element-lifecycle` | D02 | 194.85 s | 194.61 s | 99.97% | 6.21 ms | 891.8 MiB |
| `indexed-arrays-and-aliases` | D02 | 88.07 s | 87.84 s | 99.94% | 6.49 ms | 640.7 MiB |
| `copied-array-slices` | D02 | 90.69 s | 90.48 s | 99.96% | 5.18 ms | 550.4 MiB |
| `primitive-slice-assignment` | D02 | 117.77 s | 117.54 s | 99.96% | 6.13 ms | 765.6 MiB |
| `empty-inline-array-control` | control | 0.80 s | 0.65 s | 97.96% | 1.57 ms | 61.3 MiB |
| `small-scalar-control` | control | 0.86 s | 0.75 s | 98.50% | 1.46 ms | 66.1 MiB |

The deterministic observations explain the scaling shape. The recursive
optional witness has 480 reachable blocks, 1,992 tokens and 389,224 state bits;
244 full rounds perform 4,568 block visits and process 80,995,620 fact
removals. The array-element lifecycle witness has 105 reachable blocks, 459
tokens and 142,073 state bits; 68 rounds perform 3,586 block visits and process
341,291,293 fact removals. The other D02 witnesses process between 74 million
and 206 million removals. Both repetitions produced identical structural
dimensions and work counts.

## Diagnosis

The PS01 checkpoint is **go for D01 and D02**. Independent checking dominates
both shapes; draft production takes milliseconds, and every other observed
compiler phase is negligible beside the check. Native execution is also
sub-millisecond or a few milliseconds, while linking is outside the compiler
pipeline. The evidence therefore assigns the regression to the placement
checker and justifies PS02's equivalence scaffolding before its representation
and scheduling are changed.

This baseline cannot compare absolute time across hosts, build profiles or
unrelated repository states. Process wall time and peak RSS include the Cargo
test process around the isolated witness. Internal pilot time starts after the
fixture has produced its verified program, so it intentionally diagnoses the
low-level pipeline rather than frontend cost. `fact_removals` counts facts
discarded while processing intersections across all rounds; it is a work count,
not the number of distinct final lattice facts. Re-run the same witness set and
host/profile protocol for roadmap checkpoints, and use structural observations
to distinguish algorithmic change from machine noise.

## PS03 compact-state checkpoint

The compact-state checkpoint was captured on 2026-09-20 from revision
`8957e382` plus the uncommitted PS03 implementation. It used the same host,
unoptimized Cargo test profile, zero warm-ups, two alternating repetitions and
complete witness matrix as PS01. The retained local report is
`build/measurements/placement-checking/run-_7pn92f0/report.json`.

| Witness | Wall median | Wall improvement | Check median | Check improvement | Peak RSS | RSS reduction |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `recursive-optionals` | 2.432 s | 19.2x | 1.395 s | 32.6x | 48.4 MiB | 59.0% |
| `primitive-array-allocation` | 0.872 s | 45.8x | 0.576 s | 68.8x | 47.6 MiB | 78.9% |
| `array-element-lifecycle` | 1.613 s | 120.8x | 1.409 s | 138.1x | 51.2 MiB | 94.3% |
| `indexed-arrays-and-aliases` | 1.069 s | 82.4x | 0.867 s | 101.3x | 50.4 MiB | 92.1% |
| `copied-array-slices` | 1.020 s | 88.9x | 0.839 s | 107.8x | 49.6 MiB | 91.0% |
| `primitive-slice-assignment` | 1.266 s | 93.0x | 1.078 s | 109.0x | 51.0 MiB | 93.3% |
| `empty-inline-array-control` | 0.187 s | 4.3x | 0.038 s | 17.0x | 46.4 MiB | 24.4% |
| `small-scalar-control` | 0.132 s | 6.5x | 0.039 s | 19.1x | 46.9 MiB | 29.0% |

Placement production stayed within baseline noise at 5.20--28.58 ms. Every
deterministic dimension and work counter exactly matches PS01, including 244
rounds and 80,995,620 processed removals for recursive optionals and 68 rounds
and 341,291,293 removals for array-element lifecycle. The speedup therefore
comes from compact storage and word operations rather than changed convergence
semantics or a smaller workload.

The compact representation by itself clears the accepted per-witness goals:
every discovery witness improves by more than 5x, the worst wall median is well
below 30 seconds, and the worst baseline RSS falls by more than 50%. Controls
also improve rather than regress. A cached `make compiler-test` completed in
50.30 seconds after this change; the final cached workspace and `make check`
acceptance measurements are recorded under PS06 below.

No immutable relation has a demonstrated material cost at this checkpoint.
Although round visits remain structurally unchanged, absolute witness time is
already below 2.5 seconds and all current performance thresholds are met.
Consequently PS04 should add no relation cache without new profile evidence,
and PS05's worklist is presently a justified skip under the roadmap's schedule
stop condition.

## PS04 and PS05 stop-condition disposition

PS04 retained the direct target and draft relation paths. The PS03 report did
not identify any relation whose remaining absolute cost justified a cache, so
no index, alternate interpretation or cache lifetime was introduced.

PS05 retained deterministic Jacobi rounds. Structural round work remains
observable, but placement checking takes only 0.58--1.41 seconds across the D01
and D02 witnesses and all accepted witness and memory thresholds already pass.
A worklist would therefore change convergence control flow without a current
performance requirement. The living placement contract and test-only round
oracle remain unchanged for PS07 review. PS06 subsequently ran the repeated
full acceptance measurements and repository gates below.

## PS06 acceptance checkpoint

The acceptance run used the clean committed revision `c3978118`, the same host
and unoptimized Cargo test profile as PS01, zero warm-ups and two alternating
repetitions. The retained raw report is
`build/measurements/placement-checking/run-d2qhw974/report.json`.

| Witness | PS01 wall | PS06 wall | Improvement | PS06 check | PS06 peak RSS | RSS reduction |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| `recursive-optionals` | 46.59 s | 2.419 s | 19.3x | 1.378 s | 48.4 MiB | 59.0% |
| `primitive-array-allocation` | 39.93 s | 0.878 s | 45.5x | 0.585 s | 47.6 MiB | 78.9% |
| `array-element-lifecycle` | 194.85 s | 1.626 s | 119.9x | 1.422 s | 51.1 MiB | 94.3% |
| `indexed-arrays-and-aliases` | 88.07 s | 1.084 s | 81.3x | 0.878 s | 50.4 MiB | 92.1% |
| `copied-array-slices` | 90.69 s | 1.025 s | 88.5x | 0.839 s | 49.6 MiB | 91.0% |
| `primitive-slice-assignment` | 117.77 s | 1.276 s | 92.3x | 1.084 s | 50.6 MiB | 93.4% |
| `empty-inline-array-control` | 0.80 s | 0.183 s | 4.4x | 0.039 s | 46.1 MiB | 24.7% |
| `small-scalar-control` | 0.86 s | 0.139 s | 6.2x | 0.040 s | 46.9 MiB | 29.1% |

Two complete cached `cargo test --locked --workspace` samples passed in 52.14
and 52.73 seconds, producing a 52.44-second median. The six discovery witnesses
remain ordinary enabled tests, and `make check` passed the complete workspace,
placement/oracle, runtime and 650-case golden suites.

The PS06 decision is **pass for D01 and D02 independently**:

- D01 exceeds the 5x requirement, is below 30 seconds and reduces RSS by 59.0%.
- Every D02 witness exceeds the 5x requirement; the former worst witness is
  below 2 seconds and reduces RSS by 94.3%.
- The cached workspace median is below 90 seconds.
- Both small controls improve substantially, so the 20% regression guard passes.
- Correctness, deterministic diagnostics and native execution remain unchanged.

No fallback, test reclassification or design amendment is needed. PS07 may
close the two discovery records after its cumulative artifact and diff review.

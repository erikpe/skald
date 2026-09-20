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

# Copy-Capability Materialization Measurements

Status: CP01 baseline accepted and CP02 transition recorded on 2026-09-14.
Gate 1 is **go**; CP03 is next.

This document records the structural and operational baseline for the
[copy-capability materialization roadmap](../roadmaps/COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md).
The structural report measures the current duplicate availability solvers and
HIR reconstruction directly. The ordinary
[cleanup measurement baseline](CLEANUP_MEASUREMENTS.md) supplies the broader
compiler-time and peak-memory reference for the later retention decision.

## Reproducing the baseline

From the repository root, run:

```text
cargo test --locked -p skald-compiler capability_baseline --no-fail-fast
make cleanup-baseline
```

The focused report is request-local and compiled only for Rust unit tests. It
does not use process-global counters and is absent from the compiler's public
API, diagnostics, dumps, driver output, and release build. Saturating counters
record completed structural work without affecting capability decisions.

The accepted baseline used repository revision
`7a19c16de1713e390b771dee43565365c01efdd9`. The working tree was dirty with
the CP01 test instrumentation, regression fixture, and this documentation.
The operational compiler was built with the `golden` profile without
`cfg(test)`, so the test-only probes were absent from that executable. Its
SHA-256 was
`b2db11e4acd0906181843af924c4454c135569fd7bb2992359dda940c4676c4e`.

The complete ignored report is
`build/measurements/cleanup-baseline/run-ttc4wgda/report.json`. It covers all
19 maintained workloads with three compiler samples, one native warmup, five
native samples, the default 17-occurrence MIR schedule, the `x86_64-sysv`
target, and a 30-second subprocess timeout.

## Structural fixture

The maintained `capability_baseline` fixture assigns nine class identities and
five canonical array identities in a fixed order:

| Shape | Fixture coverage |
| --- | --- |
| Empty and direct operations | Empty synthesized class plus a class with user copy construction and assignment |
| Inheritance and ordered plans | A synthesized derived class with a direct base, ordered fields, and a direct final field |
| Nested class and optional types | Direct class, optional class, and nested optional class fields |
| Class arrays and nested arrays | Optional-class array, class array, and array-of-class-arrays |
| Direct independent failures | One class lacks only construction and another lacks only assignment |
| Propagated direct failures | A containing class fails construction and assignment through different fields |
| Recursive dependency | A class contains itself inline and terminates as unavailable |
| Array-mediated failures | A containing class depends on separate constructor-unavailable and assignment-unavailable class arrays |

The exact regression expectations cover every class and array identity. They
also preserve outer-to-inner failure paths; selected user and synthesized
operations; direct-base selection; constructor and assignment field order;
assignment permission for the direct final field; and all default, copy,
assignment, and destruction array lifecycle slots.

## Structural counts

The fixture contains nine classes and five arrays. The test reports these
exact counts:

| Current type-check reconstruction | Count |
| --- | ---: |
| Constructor convergence rounds | 2 |
| Assignment convergence rounds | 2 |
| Constructor capability records cloned into provisional views | 36 |
| Assignment capability records cloned into provisional views | 36 |
| Provisional HIR array-table builds | 4 |
| Provisional HIR array entries constructed | 20 |
| Final HIR array-table builds | 1 |
| Final HIR array entries constructed | 5 |
| Final publication clones | 1 |
| Final publication entries cloned | 5 |
| Constructor class-plan constructions | 9 |
| Assignment class-plan constructions | 18 |

Assignment plans are constructed once as the provisional input to constructor
convergence and once again after constructors settle. Each of the four
convergence rounds clones both nine-entry class capability sets and constructs
all five HIR array entries. The final table is then constructed independently
and cloned for publication.

The compact phase-neutral computation reports:

| Neutral lifecycle computation | Count |
| --- | ---: |
| Constructor convergence rounds | 2 |
| Assignment convergence rounds | 2 |
| Constructor array-entry evaluations during convergence | 10 |
| Assignment array-entry evaluations during convergence | 10 |
| Final array-entry evaluations | 10 |

These neutral counts describe boolean availability evaluation. They do not
construct or clone HIR operation plans.

## Operational context

The operational baseline is observational. CP03 must capture paired runs under
the roadmap's matching-host policy before applying Gate 2. The four compile
pressure workloads began at:

| Workload | Median compiler time | Median peak RSS |
| --- | ---: | ---: |
| `compile/small-source` | 2.070 ms | 8,792 KiB |
| `compile/many-modules-large-cfg` | 1,627.617 ms | 73,136 KiB |
| `compile/many-generic-applications` | 6.035 ms | 10,328 KiB |
| `compile/nested-ownership` | 895.815 ms | 59,548 KiB |

The full report retains ranges, median absolute deviations, deterministic
artifact hashes, semantic observations, and the remaining 15 workloads. A
single unpaired delta against this table is not a retention decision.

## Gate 1 decision

Gate 1 is **go**. Every mandatory condition is satisfied:

- the array-mediated failure fixture clones 72 class capability records across
  four provisional views;
- those views construct 20 provisional HIR array entries before the five-entry
  final table;
- both constructor and assignment convergence take two recorded rounds;
- explicit expectations and exhaustive identity-indexed comparisons agree for
  neutral availability, deterministic failure paths, and HIR plan
  availability; and
- empty, direct, inherited, optional, nested optional, array, nested array,
  recursive, and independently unavailable shapes all terminate without an
  arbitrary cap.

CP02 may therefore establish one immutable neutral result at the type-check
request boundary while retaining the current HIR construction for transition
assertions.

## CP02 transition cost

The type-check capability facade now computes and retains one fresh neutral
result from the final selected `ResolvedProgram`. Its test-only report embeds
exactly one neutral computation report. On the maintained fixture that adds:

| Added neutral work while the old HIR solver remains | Count |
| --- | ---: |
| Neutral computations per type-check capability request | 1 |
| Constructor availability rounds | 2 |
| Assignment availability rounds | 2 |
| Array-entry evaluations during convergence | 20 |
| Final array-entry evaluations | 10 |

The CP01 HIR reconstruction counts remain unchanged during this transition:
72 class capability records are still cloned, 20 provisional HIR array entries
are still constructed, and assignment plans are still constructed twice.
This temporary combined cost is accepted only to keep the old HIR builder as
an exhaustive transition oracle. CP03 must remove that duplicate solver and
provisional construction before Gate 2 can retain the design.

Failure diagnostics now borrow their paths from the retained neutral result.
The fixture checks pointer identity for representative constructor and
assignment failures as well as exact path contents. Completed-boundary
assertions compare every class availability, every array availability, and
every transient old-solver failure path with the neutral authority before the
completed facade discards those duplicate paths.
Resolver candidate queries remain unchanged: their lazy neutral result is
owned by `GenericCapabilityQuery` over its borrowed candidate view and is
never stored in or transferred through `ResolvedProgram`.

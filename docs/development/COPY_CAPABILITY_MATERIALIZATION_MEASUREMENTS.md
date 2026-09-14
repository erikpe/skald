# Copy-Capability Materialization Measurements

Status: complete on 2026-09-14. Gate 1 was **go**, Gate 2 was **no-go**, and
the original type-check implementation was restored.

This document records the structural and operational baseline for the
[archived copy-capability materialization roadmap](../archive/COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md).
The structural report measures the baseline duplicate availability solvers,
the CP02 transition, and the experimental one-pass materializer directly. The
ordinary
[cleanup measurement baseline](CLEANUP_MEASUREMENTS.md) supplies the broader
compiler-time and peak-memory reference for the later retention decision.

## Reproducing the retained behavior

From the repository root, run:

```text
cargo test --locked -p skald-compiler copy_capability_facts --no-fail-fast
make cleanup-baseline
```

The focused test retains the exact semantic fixture without measurement
instrumentation. The structural counts below were captured through temporary,
request-local test probes in CP01–CP03. CP04 removed those probes from both
solvers after the retention decision; the recorded counts remain the durable
evidence.

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

The maintained lifecycle-capability fixture assigns nine class identities and
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

## Operational baseline

The operational baseline is observational. CP03 captured paired runs under the
roadmap's matching-host policy before applying Gate 2. The four compile-pressure
workloads began at:

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

This result authorized CP02 to establish one immutable neutral result at the
type-check request boundary while retaining the current HIR construction for
transition assertions.

## CP02 transition cost

During CP02, the type-check capability facade computed and retained one fresh neutral
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
This temporary combined cost was accepted only to keep the old HIR builder as
an exhaustive transition oracle. CP03 removed that duplicate solver and
provisional construction before Gate 2 was evaluated.

During CP02, failure diagnostics borrowed their paths from the retained neutral
result. The fixture checked pointer identity for representative failures as
well as exact path contents, while completed-boundary assertions compared every
class and array availability and every transient old-solver failure path.
Resolver candidate queries remained unchanged: their lazy neutral result is
owned by `GenericCapabilityQuery` over its borrowed candidate view and is
never stored in or transferred through `ResolvedProgram`.

## CP03 structural result

The neutral-guided materializer removes the type-check availability solver and
uses identity-indexed visit state only to order construction and reject an
available dependency cycle as an internal consistency defect. The maintained
nine-class, five-array fixture reports:

| Type-check materialization | CP01/CP02 | CP03 |
| --- | ---: | ---: |
| Legacy constructor convergence rounds | 2 | 0 |
| Legacy assignment convergence rounds | 2 | 0 |
| Class plan records cloned into provisional views | 72 | 0 |
| Provisional HIR array-table builds | 4 | 0 |
| Provisional HIR array entries | 20 | 0 |
| Constructor class-plan constructions | 9 | 9 |
| Assignment class-plan constructions | 18 | 9 |
| Final HIR array-table builds | 1 | 1 |
| Final HIR array entries | 5 | 5 |
| Final publication clones / entries | 1 / 5 | 0 / 0 |
| Final publication moves / entries | 0 / 0 | 1 / 5 |

The neutral computation remains one result with the same two constructor and
two assignment rounds and the same 30 total array-entry evaluations. Exact
expected facts, failure paths, selected operations, base and field order,
final fields, and array lifecycle slots remain unchanged. Focused defect tests
also confirm that a stale available fact with no resolved operation or with a
new synthesized dependency cycle terminates as a compiler consistency defect.
The materializer implementation reduces `typeck/capabilities.rs` from 688 to
556 lines while removing the second availability algorithm and its failure
paths.

## Gate 2 operational comparison

Three complete pairs used the same `golden` profile, 19-workload inventory,
three compiler samples, one native warmup, five native samples, 30-second
timeout, 17-occurrence MIR schedule, target, and runtime-trace policies. The
clean CP02 compiler was preserved before editing and paired with the CP03
compiler in these ignored reports:

| Pair | CP02 report | CP03 report |
| ---: | --- | --- |
| 1 | `run-57b9ma8t/report.json` | `run-f2zw02lm/report.json` |
| 2 | `run-scxvxdou/report.json` | `run-44mtmjyb/report.json` |
| 3 | `run-n78i_9aa/report.json` | `run-2vg3zqk1/report.json` |

Every workload retained identical deterministic compiler arguments, source
inventory, pass observations, assembly artifacts, and native semantic
observations. Peak RSS stayed within the five-percent threshold in every pair.
The four compile-pressure workloads also stayed within the compiler-time
threshold in all three pairs:

| Workload | Pair 1 | Pair 2 | Pair 3 |
| --- | ---: | ---: | ---: |
| `compile/small-source` | +3.0% | +4.2% | -0.4% |
| `compile/many-modules-large-cfg` | -0.5% | +1.1% | +1.6% |
| `compile/many-generic-applications` | -0.7% | -2.0% | -0.7% |
| `compile/nested-ownership` | +1.1% | +0.6% | +1.8% |

One short compiler workload crossed the threshold in two pairs. For
`native/runtime-trace-call-recursion-omitted`, the compiler-time medians were
3.121 to 3.039 ms (-2.6%), 2.915 to 3.247 ms (+11.4%), and 2.887 to 3.393 ms
(+17.5%). Its corresponding peak-RSS deltas were +0.4%, +0.1%, and +0.6%.
No other workload crossed the same compiler-time or RSS threshold in two
pairs.

Gate 2 was therefore **no-go** under the accepted rule, despite the structural,
ownership, correctness, and complexity conditions passing. The second and
third pairs are the required two agreeing outcomes for a repeatable
compiler-time regression above five percent.

## Final retained state

CP04 restored the original type-check solver and final array-table publication
path byte-for-byte from the pre-experiment implementation revision
`7a19c16de1713e390b771dee43565365c01efdd9`. The request-local CP01 counters,
CP02 neutral-authority wiring and parity assertions, CP03 materializer, and
materializer-only defect tests were removed. The maintained explicit fixture
continues to protect neutral facts and failure paths, concrete HIR operations
and order, and every array lifecycle slot.

The final repository intentionally contains two lifecycle computations with
different phase-owned products. Resolver generic candidate validation lazily
computes compact neutral facts within `GenericCapabilityQuery`. Type checking
separately computes concrete HIR plans, owns their failure paths, rebuilds
provisional HIR array tables during convergence, and clones the final table
into `HirProgram`. No lifecycle result crosses resolver publication and the
neutral service retains its phase-dependency guard.

No additional operational comparison was needed after restoration because the
five production files are identical to the pre-experiment revision measured by
the baseline. This document is the final retained measurement and decision
record; it does not claim a speedup. Final acceptance from an artifact-free
snapshot passed `make check`, all 629 golden leaves, and the Rust 1.82.0
workspace all-target check.

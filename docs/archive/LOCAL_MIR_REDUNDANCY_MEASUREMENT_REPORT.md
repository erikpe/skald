# Local Final-MIR Redundancy Measurement Report

Status: durable result for corpus version 1 at compiler revision
`16b5585df1456cd60599025d68d26d0fe70d320f`.

This report applies the frozen
[local final-MIR redundancy measurement contract](LOCAL_MIR_REDUNDANCY_MEASUREMENT_CONTRACT.md)
to the complete reviewed corpus. It compares narrow scalar-spill constant
provenance (FMV-15), redundant primitive casts (FMV-02), and exact same-block
primitive common subexpressions (FMV-03). Counts describe structural upper
bounds, not measured runtime improvements.

## Reproducibility identity

The study ran from a clean working tree on 2026-09-04 with:

```text
make mir-redundancy-measure
```

The ignored canonical JSON result was 16,519,861 bytes with SHA-256
`9a59999d27d83442267aa018aabca942fcaa1695ebbfd0767f21be8c0324966a`.
Two independent processes produced byte-identical JSON. Regeneration records
the complete per-callable detail; this durable report retains the comparison
needed to interpret that generated evidence.

- Corpus: `local-final-mir-redundancy`, version 1, 16 unique whole-world
  compilation roots.
- Compiler: `16b5585df1456cd60599025d68d26d0fe70d320f`, clean.
- Configuration: `x86_64-sysv`, runtime traces omitted, `default` final-MIR
  profile, no exclusions.
- Schedule: dead-pure-definition elimination; primitive constant folding;
  primitive algebraic simplification; repeated primitive constant folding;
  checked-integer constant folding; repeated dead-pure-definition elimination;
  conservative CFG cleanup; repeated dead-pure-definition elimination; and
  whole-world reachability.

## Aggregate result

`I`, `P`, and `B` mean interesting, proven, and blocked sites. The removable
instruction ceiling equals the proven count for each candidate in this study.

| Snapshot | MIR instructions | Executable definitions | Scalar spill I/P/B | Cast I/P/B | CSE I/P/B |
|---|---:|---:|---:|---:|---:|
| Input | 118,299 | 1,760 | 3,670 / 0 / 3,670 | 6 / 5 / 1 | 0 / 0 / 0 |
| Pre-reachability | 117,849 | 1,760 | 3,646 / 25 / 3,621 | 1 / 1 / 0 | 0 / 0 / 0 |
| Final | 20,259 | 407 | 336 / 25 / 311 | 1 / 1 / 0 | 0 / 0 / 0 |

Local simplification removes 450 instructions before reachability. Whole-world
retention then removes 1,353 executable bodies and 97,590 instructions from
the corpus aggregate. This large delta is expected because standard-library
definitions are present independently in several whole-world compilations; it
is not a count of unique repository instructions.

### Per-workload result

Each candidate cell is `proven+blocked`, ordered as scalar spill / cast / CSE.
Assembly bytes use the measurement configuration and are context rather than a
predicted saving.

| Workload | Input spill/cast/CSE | Pre spill/cast/CSE | Final spill/cast/CSE | Final instructions | Assembly bytes |
|---|---:|---:|---:|---:|---:|
| `focused/local-simplification` | 0+333 / 0+0 / 0+0 | 0+333 / 0+0 / 0+0 | 0+13 / 0+0 / 0+0 | 561 | 128,266 |
| `focused/checked-protocols` | 0+393 / 0+0 / 0+0 | 25+344 / 0+0 / 0+0 | 25+24 / 0+0 / 0+0 | 1,060 | 162,304 |
| `primitives/cast-matrix` | 0+334 / 5+1 / 0+0 | 0+334 / 1+0 / 0+0 | 0+121 / 1+0 / 0+0 | 6,206 | 974,889 |
| `control-flow/loop-lifecycle` | 0+330 / 0+0 / 0+0 | 0+330 / 0+0 / 0+0 | 0+8 / 0+0 / 0+0 | 650 | 133,607 |
| `standard-library/integer-strings` | 0+330 / 0+0 / 0+0 | 0+330 / 0+0 / 0+0 | 0+25 / 0+0 / 0+0 | 1,140 | 236,172 |
| `standard-library/generic-vector-values` | 0+448 / 0+0 / 0+0 | 0+448 / 0+0 / 0+0 | 0+32 / 0+0 / 0+0 | 2,856 | 1,040,578 |
| `standard-library/generic-vector-iteration` | 0+469 / 0+0 / 0+0 | 0+469 / 0+0 / 0+0 | 0+8 / 0+0 / 0+0 | 2,525 | 822,220 |
| `whole-world/reachability` | 0+330 / 0+0 / 0+0 | 0+330 / 0+0 / 0+0 | 0+8 / 0+0 / 0+0 | 512 | 108,061 |
| `solver/full-input` | 0+357 / 0+0 / 0+0 | 0+357 / 0+0 / 0+0 | 0+66 / 0+0 / 0+0 | 4,214 | 803,163 |
| `benchmark/generic-vector-growth` | 0+346 / 0+0 / 0+0 | 0+346 / 0+0 / 0+0 | 0+6 / 0+0 / 0+0 | 307 | 78,570 |
| `benchmark/range-i64` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 33 | 3,207 |
| `benchmark/while-i64` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 35 | 3,128 |
| `benchmark/range-u64` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 33 | 3,187 |
| `benchmark/while-u64` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 35 | 3,108 |
| `benchmark/range-u8` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 46 | 4,571 |
| `benchmark/while-u8` | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 0+0 / 0+0 / 0+0 | 46 | 4,492 |

No proven candidate appears in the control-flow, standard-library,
whole-world, solver, or benchmark categories.

## Candidate comparison

| Candidate | Retained evidence | Creation or blocking boundary | Effort, risk, and reuse | Threshold result |
|---|---|---|---|---|
| Narrow scalar-spill provenance | 25 proven final substitutions, all in `focused/checked-protocols`; ceiling 25 instructions and values | The checked-integer pass exposes constants but preserves result carriers. Of 311 final blocked sites, primary blockers are protected use (150), ambiguous writes (94), unsupported type/operation (44), and lifecycle participation (23). Secondary missing-dominance and ambiguous-write barriers overlap heavily. | Medium effort with meaningful soundness obligations; potentially reusable by checked, primitive, branch, and cast simplification | Fails: no proven site outside the two focused optimization fixtures |
| Redundant primitive casts | One proven final identity cast in `primitives/cast-matrix`; ceiling one instruction and value | Five identity casts exist at input and current local passes remove four. The remaining `f64 -> f64` identity is emitted for explicit `(f64) -0.0`; floating negation is deliberately not folded. | Medium effort and moderate primitive-domain risk; little measured current-corpus leverage | Fails: one non-focused-category site, but no second category or material standard-library, solver, or benchmark consumer |
| Exact same-block primitive CSE | Zero interesting or proven sites at every checkpoint across 811 retained inspected operations | Neither lowering nor existing local rewrites produce the exact repeated key shape in this corpus | Medium effort; exact local form is comparatively narrow and does not establish global or memory CSE infrastructure | Fails: no site exists to audit or remove |

The scalar census records 45 potential branch-folding and 167 direct-
substitution unlocks in final MIR, but these edges are classified before all
provenance barriers are applied. They are architectural hints, not 212 safe
secondary optimizations, and do not override the eligibility result.

## Manual audit

All 26 proven final sites were inspected against deterministic final-MIR dumps
and their source:

- The 25 scalar-spill sites are in `main` of
  `tests/golden/optimizations/checked_integer_constant_protocols.ska`. They
  consist of 23 direct loads of folded division, remainder, or shift results
  passed to print calls and two loads stored as operands of the nested checked
  division. Every carrier has one exact dominating constant store, matching
  type, no protected use, and the recorded direct depth.
- The cast site is in `main` of
  `tests/golden/primitives/primitive_cast_matrix.ska`. MIR contains a
  `f64 -> f64` identity cast immediately after the floating negation for the
  explicit `(f64) -0.0` expression, followed by an ordinary call use.
- The corpus planner accepted 16 distinct canonical compilation identities;
  no duplicated root inflated the totals. No classification discrepancy was
  found.

At the time of this version-one study, the aggregate analyzer API retained
callable examples rather than instruction-level examples, so a bounded
temporary probe was used and then removed. The analyzer API now retains owned,
bounded site examples with callable, block, instruction, value,
classification, and reason detail. This later maintainability improvement does
not change the archived measurements or recommendation.

## Benchmark context

These are one-run host observations, not correctness thresholds or estimated
optimization deltas. Compile time includes executable linking; native time is
the median of seven generic-vector runs or nine range-loop runs. Every native
self-check succeeded. Range workloads omit runtime traces and their assembly
sizes equal the structural report. The established generic-vector harness uses
the compiler's default trace policy, so its artifact size is intentionally not
compared with the trace-omitted structural row.

| Workload | Compile ms | Assembly bytes | Executable bytes | Median native ms |
|---|---:|---:|---:|---:|
| `benchmark/generic-vector-growth` | 1,275.277 | 112,341 | 25,256 | 1.013 |
| `benchmark/range-i64` | 51.867 | 3,207 | 16,984 | 96.582 |
| `benchmark/while-i64` | 47.506 | 3,128 | 16,984 | 96.815 |
| `benchmark/range-u64` | 50.961 | 3,187 | 16,984 | 96.517 |
| `benchmark/while-u64` | 46.528 | 3,108 | 16,984 | 96.107 |
| `benchmark/range-u8` | 56.365 | 4,571 | 16,984 | 141.130 |
| `benchmark/while-u8` | 47.736 | 4,492 | 16,984 | 141.368 |

Driver tests prove byte-identical assembly with MIR inspection enabled and
disabled. The benchmark executables therefore exercise the same assembly path
whose observer parity is checked, while successful self-checks confirm native
behavior. No timing comparison is made between inspection modes because the
inspection tool is opt-in and does not participate in ordinary compilation.

## Limitations and rejected interpretations

- Corpus aggregates count each independent whole-world compilation. Repeated
  standard-library bodies are useful workload observations, not unique-code
  totals.
- Interesting and unlock counts include blocked shapes. They must not be
  presented as safe transformations or added to proven counts.
- Instruction and value ceilings assume only the counted direct rewrite. They
  do not predict assembly bytes or runtime saved.
- Baseline timing describes the current compiler and programs. With no
  candidate implemented, there is no treatment arm and therefore no speedup,
  compile-time improvement, or size reduction to claim.
- Zero exact local CSE sites does not reject cross-block value numbering,
  commutative matching, memory redundancy, or CSE after future transformations.
- The corpus cannot establish that the focused nested checked pattern is
  common in unrepresented applications. Conversely, adding workloads merely
  to make a candidate cross the threshold would invalidate the study's
  selection discipline.
- Dense callable identities are revision-local audit aids. Regeneration must
  use the recorded source revision and schedule rather than comparing those
  IDs across unrelated compiler versions.

## Decision

Select **no candidate-specific optimization yet**. FMV-15, FMV-02, and FMV-03
remain valid follow-up ideas, but none satisfies the frozen evidence threshold.
The current corpus provides no basis for a design or implementation roadmap
for any of them, and no analyzer defect justifies changing the workload set.

The measurement boundary was subsequently hardened and its roadmap closed.
Optimization-architecture selection resumes from the candidate catalog; new
evidence may reopen these three candidates without changing this recorded
decision.

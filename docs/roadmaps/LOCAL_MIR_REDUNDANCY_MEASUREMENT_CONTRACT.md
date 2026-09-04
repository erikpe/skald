# Local Final-MIR Redundancy Measurement Contract

Status: frozen for the active
[local final-MIR redundancy measurement roadmap](LOCAL_MIR_REDUNDANCY_MEASUREMENT_ROADMAP.md).

This document defines what the local-redundancy study measures, which
whole-world compilations it observes, how results are serialized, and what
evidence may select a next optimization project. It is a measurement contract,
not an optimization specification. A counted opportunity does not authorize a
rewrite or establish a native performance improvement.

## Measurement unit and identity

One **workload** is one unique configured whole-world compilation. Loading the
same module through several imports does not create additional workloads or
duplicate its definitions within that workload.

The canonical compilation identity consists of:

1. canonical repository-relative entry path;
2. canonical ordered provider roots;
3. standard-library selection;
4. target and runtime-trace policy;
5. MIR optimization profile and exclusions; and
6. compiler arguments that affect compilation.

Working-directory spelling, native program arguments, stdin, output paths,
linker selection, and repetition count do not distinguish compilation
identities. Corpus planning rejects two entries with equal canonical
compilation identities. Several native runs may refer to one compilation.

A golden-backed workload refers to the existing `PlannedBuild` identity
`<spec>::<test>::<variant`. It may additionally name one or more complete
planned-run identities to obtain native arguments, stdin, working-directory,
and prepared-file context. The golden plan remains authoritative for source
ownership and configuration; the measurement corpus does not copy its sources
or expectations.

A benchmark-only workload records an explicit entry and structured compilation
configuration because it has no golden build owner. Explicit entries must use
repository-relative contained paths. Compiler arguments are retained in
source order but must not override the frozen default MIR profile, target, or
runtime-trace policy. Native arguments and stdin belong to optional run records,
not the compilation identity.

## Checkpoints

Every workload uses the current resolved `default` schedule and observes three
verified snapshots:

| Stable snapshot name | Selection rule | Interpretation |
|---|---|---|
| `input` | The pipeline `Input` checkpoint | Redundancy presented to the optimization pipeline |
| `pre-reachability` | The `After` checkpoint immediately preceding the sole `whole-world-reachability` occurrence | Redundancy left by local passes, including definitions that semantic retention may remove |
| `final` | The pipeline `Final` checkpoint after `whole-world-reachability` | Redundancy reaching backend lowering in the retained whole-world program |

The selector resolves positions from stable pass names and occurrence order. It
never embeds a numeric schedule position. Measurement fails before producing a
report if `whole-world-reachability` is absent, repeated, or not the last pass,
or if any required checkpoint is missing. The report records every resolved
schedule element as `(position, stable name, occurrence)` so historical data
cannot silently cross a schedule change.

Input-to-pre-reachability deltas describe effects of the current local
pipeline. Pre-reachability-to-final deltas describe removal with unreachable
definitions. The final snapshot is primary when estimating optimization work
that would reach the backend; earlier snapshots diagnose where redundancy was
created, exposed, removed, or confined to dead code.

## Common census vocabulary

Each candidate family classifies a deterministic universe of **inspected
sites**. Within that universe:

- An **interesting site** has the structural shape of possible redundancy.
  It is not necessarily safe to rewrite.
- A **proven candidate** is an interesting site for which the census can prove
  every condition in its deliberately conservative candidate model.
- A **blocked candidate** is interesting but fails at least one required
  condition. It records one primary blocker and every applicable barrier.
- A **non-candidate** is inspected but does not have a redundancy shape. It is
  retained for denominator and lowering-profile information, not reported as
  missed optimization.
- An **estimated downstream unlock** is one directly identified consumer that
  would become interesting or proven after a single virtual substitution. It
  is neither a candidate removal nor recursively propagated.

For every family and snapshot:

```text
inspected sites = interesting sites + non-candidates
interesting sites = proven candidates + blocked candidates
```

All site sets are unique before aggregation. Counts use saturating `u64`
addition. Every object containing a saturated value sets `saturated` to
`true`; consumers must not interpret that object's totals as exact.

Primary blocker selection is deterministic and uses this precedence:
malformed identity, unsupported type or operation, noncanonical place,
protected metadata or use, alias exposure, lifecycle participation, ambiguous
writes, missing dominance, control-flow boundary, missing value-domain fact,
and other unsupported producer. The complete barrier set remains sorted by
this same order, so primary accounting is exclusive without discarding
secondary architectural evidence.

Candidate observations also record distinct affected callables, supporting
values and instructions, a conservative upper bound on removable values and
instructions, consumer families, and deterministic callable/site examples.
Upper bounds are structural hypotheses and must be labeled as such.

## Scalar-spill constant provenance

The scalar-spill unit of opportunity is a use of a value defined by a load from
an exact `ScalarSpill` whose constant can be proven through the canonical
storage chain and substituted at that use without crossing a protected role.
Supporting carrier and load counts are reported separately and are not added to
the opportunity count.

Provenance depth is:

- `direct`: the queried spill's unique dominating store takes a literal typed
  constant definition;
- `one-hop`: that store takes a load whose queried spill has direct provenance;
- `transitive`: two or more intermediate canonical spill load/store steps.

The analyzer separately reports checked integer protocol, total primitive,
primitive cast, conditional branch, store, return, call, and other consumers.
It reports direct unlocks for checked folding, primitive folding, cast
simplification, branch folding, and CSE. It stops after one virtual
substitution and uses existing exact evaluation and protocol semantics.

Interesting spill sites include direct or chained constant-shaped producers
even when rejected by write, dominance, type, place, alias, lifecycle, or
metadata checks. Arbitrary loads without a constant-shaped chain are
non-candidates, not blocked optimization opportunities.

## Redundant primitive casts

The cast census inspects every ordinary `PrimitiveCast` assignment. Checked
floating-to-integer rvalues and their range-check diamonds are counted only as
an excluded family and never as ordinary cast candidates.

Interesting cast sites are:

- exact identity casts; or
- a later cast in an adjacent same-block cast chain for which composing the
  two conversions could equal either the original input or one direct cast.

A site is proven only when equality holds over the complete source value
domain using exact Skald type, width, signedness, and boolean canonicalization
semantics. Narrowing followed by widening, integer/boolean crossings without a
domain fact, floating numeric conversion, raw-bit reinterpretation involving
binary64, checked conversion, and nonadjacent chains remain blocked or
non-candidates as their structural shape dictates. Equal host register width
is never proof.

The census records identity, removable chain, required semantic conversion,
raw-bit, floating, checked, and unsupported distributions. Checkpoint evidence
may attribute a pattern to initial lowering or an earlier pass, but source
origin is reported only when that attribution is direct and deterministic.

## Same-block primitive common subexpressions

The CSE census inspects total integer and boolean unary, binary, and comparison
assignments. An exact key contains operation identity, result type, and ordered
operand `ValueId`s. Constants, casts, loads, floating operations, checked
operations, calls, ownership operations, path/proof queries, and semantic
memory queries are outside the key universe.

An interesting site is a later assignment in the same block with the exact
key of an earlier assignment. Instruction order supplies dominance. It is a
proven candidate only when replacing every use of the later result with the
earlier result is accepted by the current ordinary value-use boundary. Dead
later results, protected uses, malformed identities, and source-observation
barriers are classified separately.

Commutative operand reordering, reassociation, algebraic equivalence, constants
known only through storage, and equivalence across blocks are not direct CSE.
They may appear only as explicit overlap or blocker evidence.

## Primary attribution and overlaps

Direct candidates use this primary attribution:

1. A redundant `PrimitiveCast` assignment belongs to the cast census even if
   it could share an expression key.
2. A repeated in-scope non-cast rvalue belongs to CSE.
3. A scalar-spill candidate belongs to provenance when substitution at its use
   is the enabling action; any newly visible cast, CSE, checked fold, primitive
   fold, or branch fold is an unlock edge rather than another direct removal.

The overlap matrix is directed and records `(enabler, consumer family, sites)`.
It contains only immediate edges from scalar provenance to cast, CSE, checked
folding, primitive folding, or branch folding, plus a `none` bucket for direct
substitution value. Cast-to-CSE and recursive unlock chains are excluded from
the first study. Direct candidate totals therefore remain disjoint while the
matrix preserves architectural leverage.

## Version-one corpus

Corpus name is `local-final-mir-redundancy`; version is `1`. All golden-backed
entries select the `default` build and reuse the named existing plan. The
measurement tool must verify every referenced identity against the complete
golden plan before compiling any workload.

| Workload ID | Existing build or explicit entry | Optional native context | Coverage role |
|---|---|---|---|
| `focused/local-simplification` | `optimizations/local_final_mir_simplification::profile_equivalence::default` | `scalars_proof_lifecycle_function_values_and_reachability` | Existing scalar, CFG, proof, lifecycle, and reachability positive/negative matrix |
| `focused/checked-protocols` | `optimizations/checked_integer_constant_protocols::successful_protocol_equivalence::default` | `integer_types_boundaries_effects_static_lifecycle_and_ownership` | Nested spill constants and checked-consumer unlocks |
| `primitives/cast-matrix` | `primitives/conversions::primitive_cast_matrix::default` | `type_matrix` | Complete ordinary primitive cast shapes |
| `control-flow/loop-lifecycle` | `control_flow/loops::loop_lifecycle_matrix::default` | `cleanup_order` | Loop boundaries, mutable epochs, and lifecycle-heavy negative cases |
| `standard-library/integer-strings` | `primitive_strings/conversions::integer_helpers::default` | `default` | Standard-library-backed integer helper arithmetic and casts |
| `standard-library/generic-vector-values` | `standard_vec/vectors::generic_values::default` | `complete_element_and_lifecycle_profile` | Generic specialization, growth, ownership, and library code |
| `standard-library/generic-vector-iteration` | `standard_vec/vectors::iteration::default` | `stored_values_generic_dispatch_and_loop_exits` | Generic iteration and stored scalar consumers |
| `whole-world/reachability` | `whole_world_reachability/reachability::profile_equivalence::default` | `startup_entry_shutdown_and_ownership` | Reachability-adjusted definitions and static lifecycle |
| `solver/full-input` | `aoc/2025/10/part2/solver::solver::default` | `full_input` | Largest available application-shaped solver workload |
| `benchmark/generic-vector-growth` | `tests/benchmarks/generic_vec/growth.ska` | default process invocation | Allocation-heavy generic benchmark |
| `benchmark/range-i64` | `tests/benchmarks/range_loop/i64_range.ska` | default process invocation | Signed range lowering |
| `benchmark/while-i64` | `tests/benchmarks/range_loop/i64_while.ska` | default process invocation | Signed while-loop comparison |
| `benchmark/range-u64` | `tests/benchmarks/range_loop/u64_range.ska` | default process invocation | Unsigned range lowering |
| `benchmark/while-u64` | `tests/benchmarks/range_loop/u64_while.ska` | default process invocation | Unsigned while-loop comparison |
| `benchmark/range-u8` | `tests/benchmarks/range_loop/u8_range.ska` | default process invocation | Byte range and canonicalization |
| `benchmark/while-u8` | `tests/benchmarks/range_loop/u8_while.ska` | default process invocation | Byte while-loop comparison |

The focused entries establish positive and negative classification coverage;
the standard-library, control-flow, whole-world, solver, and benchmark entries
determine breadth. Results must always retain workload/category breakdowns.
Adding, removing, or changing an entry increments the corpus version and makes
aggregate results incomparable until explicitly rebased.

The frozen census compilation configuration is x86-64 System V, repository
standard library where selected by the source plan, runtime traces omitted,
and the exact current `default` MIR profile without exclusions. Native baseline
runs reuse the listed golden run context or benchmark default. Runtime inputs
do not affect MIR census counts.

## Canonical report schema

The machine report is UTF-8 JSON with schema version `1`. Object fields render
in the order shown below; arrays use manifest, schedule, checkpoint, blocker,
consumer, and stable identity order. Paths use `/` and are repository-relative.
The human report is a projection of the same typed model.

```text
report
  schema: 1
  corpus: { name, version }
  compiler: { revision, dirty }
  configuration: { target, runtime_trace, mir_profile, mir_exclusions }
  schedule[]: { position, pass, occurrence }
  workloads[]
    id
    category
    compilation: { kind, identity, entry, provider_roots,
                   standard_library, compiler_arguments,
                   artifacts: { assembly_bytes, executable_bytes | absent } }
    native_runs[]: { identity, arguments, stdin }
    snapshots[]
      name: input | pre-reachability | final
      structure: { definitions, executable_definitions, blocks,
                   instructions, values, storages, saturated }
      scalar_spill: candidate_counts
      redundant_casts: candidate_counts
      local_cse: candidate_counts
      overlaps[]: { enabler, consumer, sites }
      callables[]: callable_counts
      saturated
    operational: { compile_nanoseconds, native_nanoseconds[] } | absent
  totals
    snapshots[]: snapshot totals with the same census shape
    workload_categories: { category, workloads_with_proven_candidates[] }
    saturated
```

`candidate_counts` contains `inspected`, `interesting`, `proven`, `blocked`,
`non_candidates`, `affected_callables`, `supporting_values`,
`supporting_instructions`, `removable_values_upper_bound`,
`removable_instructions_upper_bound`, ordered `outcomes`, ordered
`primary_blockers`, ordered `barriers`, ordered `consumers`, ordered `unlocks`,
deterministic `examples`, and `saturated`. Candidate-specific outcomes use the
terms frozen in the preceding sections.

Callable counts identify the MIR definition kind, dense identity, and
deterministic semantic label. Examples additionally identify block,
instruction position, value when applicable, classification, and reasons.
They exist for auditing within one compiler revision; aggregate comparison
must not assume dense identities survive unrelated compiler changes.

Operational fields are emitted only on request and are excluded from the
canonical structural fingerprint. Revision and dirty-state context remain in
the report; a dirty report is valid evidence only when its diff is retained or
the result is regenerated from a clean revision.

Artifact sizes are deterministic structural context when the corresponding
artifact is produced. The structural census always produces assembly through
the real driver and therefore records `assembly_bytes`; it leaves
`executable_bytes` absent because it does not invoke host linking. Likewise,
native input context is retained without executing programs during corpus
aggregation, so requested operational output records compile duration and an
empty native-duration array.

Native arguments use exact byte strings encoded as either UTF-8 text or
lowercase hexadecimal bytes. Stdin records its `none`, `inline`, or `file`
origin, repository-relative file path when applicable, byte count, and SHA-256
digest rather than embedding an unbounded payload. The corpus or golden plan
remains the content owner. These records establish reproducibility context and
do not affect compilation deduplication.

### Accounting examples

An empty family is valid:

```json
{"inspected":0,"interesting":0,"proven":0,"blocked":0,"non_candidates":0,"saturated":false}
```

A family with positive, rejected, and overlapping observations satisfies both
accounting identities while overlap remains separate:

```json
{
  "inspected": 9,
  "interesting": 5,
  "proven": 3,
  "blocked": 2,
  "non_candidates": 4,
  "primary_blockers": [
    {"reason": "protected-use", "sites": 1},
    {"reason": "missing-dominance", "sites": 1}
  ],
  "overlaps": [
    {"enabler": "scalar-spill", "consumer": "local-cse", "sites": 2}
  ],
  "saturated": false
}
```

Saturation is explicit and sticky through parent aggregates:

```json
{"inspected":18446744073709551615,"saturated":true}
```

Two corpus entries that resolve to the same canonical compilation identity are
an error even when they select different native runs. They must become one
workload with two `native_runs` records. Conversely, equal entry paths with
different provider roots or standard-library selection are distinct
compilations and require distinct workload IDs.

## Comparison and recommendation rules

The final report compares candidates across these dimensions without reducing
them to one weighted score:

1. proven candidates in the final retained snapshot;
2. breadth across focused, standard-library, control-flow, whole-world,
   solver, and benchmark categories;
3. conservative removable instruction/value ceiling;
4. direct downstream unlocks and overlap with the other candidates;
5. input-to-pre-reachability and pre-reachability-to-final behavior;
6. dominant blocker classes and likely creation phase;
7. estimated implementation effort and semantic risk from the candidate
   catalog; and
8. reuse by other cataloged optimizations or evidence for a larger
   representation/analysis boundary.

A candidate may be recommended for an implementation design or roadmap only
when it has at least one manually confirmed proven final site outside the two
focused optimization fixtures and either spans two non-focused workload
categories or exposes a material direct consumer in a standard-library,
solver, or benchmark workload. Manual review covers every proven site when
there are at most 50, otherwise a deterministic sample of at least 20 spanning
all reported classifications and workload categories. Any classification
error blocks recommendation until fixed and regenerated.

Raw count alone never breaks a close comparison. Prefer broader retained
coverage and reusable downstream leverage when semantic risk and effort are
comparable. Recommend a lowering cleanup when redundancy is systematic at
input and tied to one lowering owner. Recommend a prerequisite analysis when
one sound reusable blocker dominates multiple candidates. Recommend more
workloads when results are confined to focused fixtures. Recommend no new
optimization when no candidate crosses the evidence threshold.

The conclusion must select exactly one next action and update the optimization
candidate catalog. Implementing that action remains separately reviewed work.

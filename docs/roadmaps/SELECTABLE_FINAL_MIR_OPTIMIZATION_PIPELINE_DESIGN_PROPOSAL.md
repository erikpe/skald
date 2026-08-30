# Selectable Final-MIR Optimization Pipeline Design Proposal

Status: frozen design proposal. MOP1 through MOP12 were confirmed together on
2026-08-30 and promoted into the
[compiler phase contract](../compiler/PHASES_AND_IR.md#frozen-selectable-final-mir-optimization-pipeline-direction),
[driver contract](../compiler/DRIVER_AND_ARTIFACTS.md#frozen-final-mir-optimization-selection),
and
[reporting contract](../compiler/REPORTING.md#frozen-final-mir-pass-reporting).
The
[implementation roadmap](SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md)
owns delivery; none of the planned framework or production optimization is
implemented yet.

This proposal defines the first production target-independent optimization
framework for Skald. It builds on the implemented static-lifecycle authority
and dense callable-local MIR rewriting foundations, turns the currently
dormant transforming pipeline shape into a selectable deterministic pass
runner, and ends with one deliberately small dead-pure-definition elimination
pass that proves the complete boundary.

The objective is not merely to run one optimization. It is to establish the
stable ownership, verification, configuration, observation, and testing
contracts under which later constant folding, copy propagation, CFG cleanup,
whole-program reachability, devirtualization, and inlining can be added as
independent passes. Its place in the broader preparation sequence is recorded
by the
[optimization architecture discoveries](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md).

The language contract does not change. Evaluation order, checked failures,
panic behavior, allocation behavior, deterministic destruction, aliasing,
ownership, and mutable access through shared pointees retain their current
meaning. Every program is still compiled as one closed world, and the
resulting program is still single threaded. Those assumptions help later
analysis but do not weaken the pass-verification boundary.

## Intended outcome

The design should provide:

- one deterministic, compile-time registry of named final-MIR passes;
- typed optimization profiles that expand to explicit ordered pass schedules;
- request and CLI selection of the profile plus deterministic pass disabling;
- a crate-private exact-schedule surface for focused tests and compiler tools;
- one capability boundary through which a pass may inspect verified MIR and
  atomically rewrite it;
- immediate central reverification after every pass that actually changes MIR;
- no redundant verification for an unchanged pass result;
- structured pass, rewrite, and verification failures attributed to the exact
  pass occurrence;
- explicit analysis lifetime and invalidation without a speculative global
  analysis manager;
- deterministic aggregate and per-pass measurements owned by the pipeline;
- optional verified MIR inspection checkpoints before, after, and at the end
  of the selected schedule;
- optimization-off parity for source diagnostics and the current final MIR;
- modular pass implementations that do not perform logging, file I/O, CLI
  parsing, verification, or dense table surgery; and
- a production dead-pure-definition elimination canary that removes only an
  exact conservative family of unused scalar definitions.

## Current architecture and evidence

Skald already has an explicit target-independent MIR pipeline. Static
lifecycle synthesis produces raw final `MirProgram`, and
`passes::run_mir_pipeline` currently performs one ordinary and lifecycle-
realization verification before returning `VerifiedFinalMirProgram`.
`BackendInput` accepts only that sealed product.

The pipeline also has a test-only transforming coordinator. It:

1. verifies the input;
2. privately invalidates the final-MIR seal;
3. sends the raw program through `mir::rewrite::rewrite_program`;
4. records commit-owned identity changes; and
5. verifies the dense result before returning it.

That path proves the important safety relation, but it is deliberately dormant
in production. There is no pass descriptor, schedule, profile, request option,
CLI selection, per-pass failure identity, or production optimization.

The implemented rewrite facade already provides:

- stable sparse callable edit slots;
- functional instruction-list replacement;
- value and storage use substitution;
- explicit storage, value, block, path, logical-record, and guard deletion;
- deterministic dense compaction and complete identity maps;
- one exhaustive local-identity traversal that distinguishes value definitions
  from value uses;
- atomic all-program rewriting across functions, members, and static
  initializers; and
- structured errors for deleted, unknown, foreign, and malformed references.

The reporting layer already owns one `MirPipeline` phase and deterministic
aggregate metrics for verification executions, pass executions, final MIR
size, and structural rewrite changes. Pass executions are currently zero in
production. Reports intentionally do not contain phase dumps, and passes do
not log.

`CompilationRequest` currently records source, module, standard-library,
target, artifact, runtime-trace, and environment policy. It has no
optimization configuration. The CLI likewise has no optimization selection or
MIR dump option.

These facts make the next implementation boundary relatively narrow: promote
the tested transforming shape into a production runner without weakening the
seal, then prove it using a pass whose semantics and analysis requirements are
small.

## Relationship to the completed foundations

The proposal depends directly on two completed designs:

- the
  [static-lifecycle certificate redesign](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
  permits a transformed final program to realize a subset of immutable
  baseline lifecycle authority; and
- the
  [dense callable-local MIR rewriting design](../archive/DENSE_MIR_IDENTITY_REWRITING_DESIGN_PROPOSAL.md)
  permits a pass to delete and rewrite local entities before one atomic dense
  commit.

The pass pipeline must use both rather than duplicating either responsibility.
Passes receive no lifecycle-authority mutation API, no final-seal constructor,
and no mutable dense definition tables.

## Comparison with Niflheim

Niflheim's semantic and backend optimization pipelines use small immutable
pass descriptors containing a name and transformation, ordered default pass
tuples, explicit disabling by pass name, and centralized sequencing. Its
backend optimizer verifies before and after registered passes. The default
sequences deliberately repeat cleanup passes after transformations that expose
new opportunities.

Those are useful organizational precedents:

- a registry entry should be small and declarative;
- the schedule, not module discovery or map order, should define execution;
- one implementation may appear more than once in a schedule;
- users should disable a pass by stable name rather than by Rust module path;
- passes should compose as whole-program ownership transfers; and
- central verification should localize a malformed result.

Skald should not copy Niflheim's exact function signature or logging behavior.
Skald has a stronger final-MIR seal, immutable lifecycle authority, structured
rewrite results, request-scoped reporting, and dense callable commit. Its
pipeline must preserve those owners. A pass must return measurements as data
rather than logging, and a changed pass must return through
`verify_final_mir` before the next pass or backend can inspect its output.

## Constraints and non-goals

- Source acceptance, source diagnostics, warning ordering, and static
  lifecycle diagnostics remain independent of optimization selection.
- Optimization may change final MIR identities, block order, dumps, assembly,
  stack layout, and performance when enabled.
- Optimization-off mode must retain the current verification-only final-MIR
  path and exact unoptimized MIR.
- Pass order is deterministic and request-local. Compiler implementation
  parallelism may not change schedules, products, measurements, or dumps.
- Pass selection never affects target selection or runtime-trace policy.
- The initial registry is compiled into the compiler. Dynamic libraries,
  runtime plugin loading, external pass ABIs, and user-authored passes are out
  of scope.
- Target-specific optimization remains behind the backend boundary and does
  not enter this registry.
- No SSA, block parameters, proof-provenance normalization, general alias or
  effect analysis, interprocedural reachability, devirtualization, inlining,
  constant folding, copy propagation, CFG simplification, register allocation,
  or target LIR is introduced here.
- The canary does not remove calls, loads, storage, blocks, terminators,
  lifecycle operations, callable-address operations, checked-operation
  diamonds, proof metadata, or ownership operations.
- No pass may repair invalid producer MIR. Initial verification happens before
  the first selected pass.
- No pass may create source diagnostics. Invalid selection is a request or CLI
  configuration error; malformed pass output is an internal pipeline failure.
- No repository CI is added. The root Makefile remains the quality-gate
  interface.

## Design principles

### Profiles choose policy; passes own transformations

A profile owns an ordered list of pass identities. A pass module owns one
transformation and its local measurements. The driver chooses policy but does
not invoke transformations individually.

### Verified input, verified handoff

Every pass starts from a verified final-MIR product. An unchanged pass retains
that product. A changed pass yields raw dense MIR and cannot hand it directly
to another pass or backend.

### Conservative first contracts

The initial runner re-verifies every changed result. It does not trust
pass-declared preservation sets or cache analyses across changes. Narrow
preserving APIs may be introduced later only with a separate proof and
measured need.

### Explicit schedule, no discovery order

Filesystem layout, Rust module order, maps, sets, linker order, or compiler
worker completion may not decide pass order. Profiles and exact test schedules
are immutable ordered values.

### Passes return data

Passes do not log, render dumps, write files, or emit report events. They
return structured outcomes and already-known counts. The pipeline and driver
own observation and presentation.

### One real canary

The framework is incomplete until one production transformation exercises
selection, verified ownership transfer, instruction deletion, dense value
compaction, metrics, dumps, errors, and backend handoff.

## Vocabulary and pipeline invariant

The proposal uses these terms:

- **pass identity:** one typed compiler-known optimization identity;
- **pass name:** the stable CLI and inspection spelling of that identity;
- **registry:** the unique mapping from pass identity to name and
  implementation;
- **profile:** a named policy that expands to an ordered schedule;
- **schedule:** the exact ordered sequence of pass identities for one request;
- **occurrence:** one position in a schedule, including repeated uses of the
  same pass;
- **unchanged outcome:** the pass found no valid edit and returns the existing
  verified product;
- **changed outcome:** the pass atomically committed raw dense MIR plus its
  change data;
- **checkpoint:** a borrowed verified MIR product exposed to an optional
  inspection owner at a named schedule boundary; and
- **canary:** the first narrow production optimization proving the framework.

Let `V(P)` be central final-MIR verification, including ordinary verification
and static-lifecycle realization, and let the selected schedule be
`S = [s0, s1, ..., sn]`.

The runner establishes:

```text
verified0 = V(synthesized MIR)

for each occurrence si:
    unchanged(verified_i)       => verified_(i+1) = verified_i
    changed(raw_program_i)      => verified_(i+1) = V(raw_program_i)
    pass or verification error  => stop; publish no later product

backend input = verified_(n+1)
```

At no point may raw changed MIR become the input of another pass, an inspection
checkpoint, or a backend. A failed pass leaves no partially rewritten program
available to the driver.

## Decision register

| Decision | Question | Frozen decision | Status |
|---|---|---|---|
| [MOP1](#mop1--use-one-static-named-pass-registry) | How are passes registered? | One typed compile-time registry with unique stable names | **Confirmed** |
| [MOP2](#mop2--expand-profiles-to-explicit-ordered-schedules) | Who owns order and repetition? | Profiles expand to immutable schedules; exact schedules are test/tool inputs | **Confirmed** |
| [MOP3](#mop3--make-selection-typed-request-policy) | How are passes selected? | Typed `none`/`default` request profile plus deterministic disabling | **Confirmed** |
| [MOP4](#mop4--give-passes-a-narrow-verified-rewrite-capability) | What may a pass access? | Read-only verified MIR and one atomic rewrite capability, never raw mutable tables | **Confirmed** |
| [MOP5](#mop5--reverify-every-changed-pass-result) | When does verification run? | Once initially and immediately after every changed occurrence | **Confirmed** |
| [MOP6](#mop6--attribute-failures-and-stop-atomically) | How do failures behave? | Structured pass-attributed failure; no continuation or partial product | **Confirmed** |
| [MOP7](#mop7--keep-analysis-lifetime-explicit-and-conservative) | How are analyses managed? | Pass-local by default; every change invalidates prior local-ID analyses | **Confirmed** |
| [MOP8](#mop8--return-structured-per-pass-measurements) | How is work observed? | Pipeline-owned occurrence records and deterministic aggregate reporting | **Confirmed** |
| [MOP9](#mop9--inspect-only-verified-checkpoints) | How are optimized dumps exposed? | Optional input, after-pass, and final verified checkpoints outside reports | **Confirmed** |
| [MOP10](#mop10--keep-policy-runner-pass-and-driver-ownership-separate) | Where does implementation live? | Cohesive registry, runner, pass, inspection, and driver owners behind facades | **Confirmed** |
| [MOP11](#mop11--use-an-exact-dead-pure-canary-whitelist) | What may the canary remove? | Unused assignments from a small exhaustive non-failing scalar whitelist | **Confirmed** |
| [MOP12](#mop12--eliminate-dead-pure-trees-to-a-deterministic-fixed-point) | How does the canary compose? | Per-callable fixed point, atomic declaration/instruction deletion, full parity tests | **Confirmed** |

## MOP1 — Use one static named pass registry

Introduce one target-independent final-MIR registry. Each registered pass has:

- a typed identity used by profiles and internal APIs;
- one unique stable kebab-case name used by CLI selection, dumps, and reports;
- one transformation entry point;
- concise descriptive metadata suitable for help and internal inspection; and
- no mutable global state.

The registry is a compile-time compiler component, not a runtime extension
mechanism. Its iteration order is not semantically meaningful; schedules
refer to typed identities explicitly. Registry validation rejects duplicate
identities or names in focused tests.

The first registered identity is
`dead-pure-definition-elimination`. Later passes add one module, one registry
entry, focused tests, and deliberate placement in zero or more profiles.

The concrete Rust representation—enum, descriptor struct, static slice, or
function pointer—is private. The durable rule is that a pass cannot become
active merely because its source module exists.

## MOP2 — Expand profiles to explicit ordered schedules

Provide these initial profiles:

- `none`: no transformation occurrences; central final verification still
  runs; and
- `default`: the compiler's supported default target-independent optimization
  sequence.

At completion of this design, `default` contains the canary exactly once.
Before the canary lands, the in-progress implementation may temporarily keep
`default` empty, but the roadmap may not close in that state.

Profiles expand once per request to an immutable ordered schedule. A schedule
may contain the same pass identity more than once when later transformations
make repetition useful. Occurrence identity consists of schedule position,
pass identity, and that pass's zero-based occurrence number. Reports and dump
labels use this identity, so repeated passes are unambiguous.

A crate-private exact-schedule API supports:

- running one pass in isolation;
- intentional repeated passes;
- pass-order composition tests;
- determinism tests; and
- future compiler-internal experiments.

The production CLI does not expose arbitrary pass reordering initially.
Every pass must nevertheless be correct for any verified MIR input rather than
assuming a preceding optimization ran. Profiles may order passes for
effectiveness, never to establish validity.

## MOP3 — Make selection typed request policy

Add target-independent optimization policy to `CompilationRequest` through a
typed options value and a non-breaking builder. Existing request construction
and singleton compilation helpers use `default`.

The initial CLI surface is:

```text
--mir-optimization <none|default>
--disable-mir-pass <name>
```

`--mir-optimization` may be specified once. `--disable-mir-pass` is repeatable;
it removes every occurrence of the named pass from the selected profile.
Duplicate disabling is idempotent. Names are validated against the complete
registry, and unknown names produce one deterministic usage error with known
names sorted lexically.

No `-O` shorthand or numerical optimization levels are introduced yet.
Numerical levels would imply distinctions that do not exist with one canary.
An aggressive profile should be added only when the compiler has a reviewed
pass whose compile-time, size, or semantic-risk tradeoff justifies exclusion
from `default`.

Optimization policy is semantic compilation configuration and belongs in the
request. Report detail and dump observers remain invocation services and do
not participate in request equality.

The `none` profile is the reference unoptimized mode. It preserves the current
final MIR and assembly path except for the already-required verification and
backend behavior. The `default` profile becomes the compiler default when the
canary is registered.

## MOP4 — Give passes a narrow verified rewrite capability

One pass occurrence receives a pipeline-owned capability with two conceptual
operations:

1. inspect the current `VerifiedFinalMirProgram` read-only to compute
   pass-local analysis; and
2. if an edit is required, consume that seal through the supported atomic
   whole-program rewrite coordinator.

The pass does not receive:

- a public constructor for `VerifiedFinalMirProgram`;
- unrestricted mutable `MirProgram`;
- mutable function or member definition tables;
- static-lifecycle authority mutation;
- driver, reporting, filesystem, target, or source-diagnostic services; or
- another pass's private analysis state.

A pass returns either:

- `Unchanged` with the same verified product and pass-owned measurements; or
- `Changed` with one committed raw dense program, callable rewrite maps,
  structural change summaries, explicit changed-callable accounting, and
  pass-owned measurements.

The exact Rust type remains private. The capability should make it difficult
to accidentally clone raw MIR, mutate around the rewrite facade, or forget
whether the seal was invalidated.

Whole-program analyses may inspect every definition before editing begins.
Edits still commit through the all-definition atomic coordinator. A later
interprocedural pass therefore does not need a different verification or
ownership escape hatch.

## MOP5 — Reverify every changed pass result

The runner verifies synthesized MIR before the first selected occurrence.
That proves passes never repair producer defects.

After an unchanged result, the next occurrence receives the same verified
product and the verification count does not increase. After a changed result,
the runner immediately invokes `verify_final_mir`. Only the resulting seal may
be inspected, passed onward, dumped as a checkpoint, or sent to the backend.

The initial framework has no pass-declared preservation categories. Even the
dead-pure canary re-enters ordinary and lifecycle-realization verification
when it changes MIR. This is intentionally conservative:

- the canary removes executable value definitions;
- future passes may alter static effects or proof metadata;
- per-pass verification localizes defects to one occurrence; and
- the extra cost should be measured before weakening the boundary.

A later preserving API requires a separate design that defines the exact
property preserved and demonstrates a material verification cost. A boolean
`preserves_verification` flag is insufficient and is rejected by this
proposal.

## MOP6 — Attribute failures and stop atomically

The pipeline owns one structured failure vocabulary:

- initial final-MIR verification failure;
- pass execution or rewrite failure attributed to pass identity and
  occurrence;
- changed-output verification failure attributed to pass identity and
  occurrence; and
- invalid internal schedule or registry configuration.

The driver renders these as internal compiler failures through the existing
compiler-error boundary. They do not become source diagnostics, do not alter
diagnostic ordering, and do not continue to the backend.

A pass failure returns no partially transformed `MirProgram`. A verification
failure may retain pass measurements and rewrite summaries for reporting, but
the malformed raw program is not returned through the successful pipeline
surface or exposed to a later checkpoint.

Failure ordering is schedule order, followed by the existing deterministic
rewrite and verifier ordering. No pass may catch a verification failure and
continue with the preceding or malformed program.

## MOP7 — Keep analysis lifetime explicit and conservative

The first framework introduces no general analysis manager or cache.

By default:

- analysis computed by a pass belongs to that occurrence;
- analysis keyed by local MIR identities expires when that pass commits;
- an unchanged outcome may discard its analysis without affecting the next
  occurrence;
- a changed outcome invalidates every earlier MIR analysis;
- a later occurrence recomputes what it needs from its verified input; and
- no hidden global or thread-local state survives requests.

Small reusable read-only analyses should live under the owner that has a
demonstrated repeated responsibility. The canary needs one narrow value-use
census. It should be exposed through the exhaustive local-identity traversal
or callable editor, not implemented as a second handwritten inventory of MIR
value references.

The census distinguishes:

- value declarations;
- value definitions; and
- actual uses in instructions, terminators, places, path metadata, and logical
  metadata.

The implemented mapper already distinguishes definition mapping from ordinary
value mapping. The new query should reuse that distinction and count every
retained reference form. New value-bearing MIR variants must continue to force
review at the exhaustive traversal owner.

General liveness, dominators, call graphs, alias/effect summaries, and
incremental invalidation are deferred until real passes share them.

## MOP8 — Return structured per-pass measurements

Every occurrence returns measurements as structured data. The pipeline records:

- pass identity, stable name, schedule position, and occurrence number;
- completed, unchanged, changed, or failed outcome;
- elapsed duration measured by the runner;
- processed and explicitly changed callable counts;
- structural rewrite changes already known at commit;
- deterministic pass-owned integer counters; and
- verification executions caused by the occurrence.

The dead-pure canary owns these counters:

- removed assignment instructions;
- removed value declarations;
- changed callables.

The existing `MirPipeline` phase retains aggregate metrics in deterministic
owner order. Trace-level reporting additionally emits one typed
pass-finished event per attempted occurrence. Reports do not derive counts by
parsing MIR dumps, and pass modules do not depend on the reporting facade.

Elapsed durations remain observations rather than deterministic products.
Tests assert occurrence order, outcomes, and exact integer metrics, but not
live duration values.

The current ambiguous `rewritten callables` aggregate should be clarified
during implementation. Visiting and committing every callable is not the same
as changing it. The durable vocabulary should distinguish processed callables
from pass-reported changed callables.

## MOP9 — Inspect only verified checkpoints

Add an optional pipeline inspection service separate from operational
reporting. It receives borrowed verified products at these deterministic
stages:

- `input` after initial verification;
- `after-<schedule-position>-<pass-name>-<occurrence>` after each successfully
  completed pass, including unchanged occurrences when requested; and
- `final` after the complete schedule.

The service may call the existing phase-owned `mir::dump_mir` renderer or
collect statistics. It never receives sparse edit state or an unverified
changed program.

Inspection is disabled by default and performs no allocation or rendering when
disabled. Dump contents do not become report messages, pass logs, or semantic
request identity. The initial implementation may expose an in-memory
inspection API for tests and tools; filesystem publication and general CLI
phase-dump policy may remain separate driver work.

Checkpoint labels are deterministic and unambiguous for repeated passes.
Optimized dumps need not preserve unoptimized local IDs, but the same request,
profile, exclusions, source graph, compiler binary, and target-independent
inputs must produce identical checkpoint sequences and bytes across
independent processes.

## MOP10 — Keep policy, runner, pass, and driver ownership separate

The intended responsibility split is:

```text
driver request / CLI
    selects profile and exclusions
            |
            v
passes::pipeline facade
    resolves schedule, verifies, runs, measures, checkpoints, reseals
            |
            +--> pass registry and profiles
            |
            +--> pass implementation modules
            |        analyze verified MIR
            |        request atomic edits
            |        return pass-owned counts
            |
            +--> mir::rewrite facade
                     owns sparse edit and dense commit
```

The pipeline facade exposes the ordinary production entry point,
`verify_final_mir`, typed configuration needed by the driver, and narrow
measured or inspected variants. Registry validation, schedule resolution,
execution, measurement, errors, and inspection belong in cohesive
implementation modules rather than one growing file.

Each optimization lives in its own responsibility-oriented module under the
pass owner. The dead-pure canary owns its eligibility classification,
fixed-point algorithm, and focused tests. It reuses MIR traversal and rewrite
services rather than extending `mir` with optimization policy.

`mod.rs` files remain concise facades. The driver owns option parsing and error
presentation. Reporting owns event rendering. Backends remain unaware of
which target-independent passes ran.

## MOP11 — Use an exact dead-pure canary whitelist

The canary removes an instruction only when all of these are true:

1. the instruction is `MirInstruction::Assign`;
2. its result has zero retained uses anywhere in the callable package;
3. its rvalue kind appears in the explicit canary whitelist;
4. removing it does not require changing proof, path, logical, ownership, or
   lifecycle metadata; and
5. the matching `MirValue` declaration is removed in the same atomic edit.

The initial whitelist is:

| Rvalue family | Eligible | Reason |
|---|---:|---|
| `ConstantI64`, `ConstantU64`, `ConstantU8`, `ConstantF64Bits`, `ConstantBool` | Yes | Produces only one unused scalar value |
| `Unary` | Yes | Exact primitive operation with no failure or memory effect |
| `Binary` | Yes | Wrapping integer or IEEE binary64 operation with no separate failure edge |
| `PrimitiveComparison` | Yes | Exact primitive comparison producing only a scalar result |
| `PrimitiveCast` | Yes | This MIR family contains the non-checked primitive conversions |
| `CallableAddress` | No | Address-taken target retention and later reachability remain explicit |
| `PathCondition` | No | Coupled to path activation and proof metadata |
| `Load` | No | Memory, alias, storage-lifetime, and future effect reasoning are deferred |
| `IntegerDivision` | No | Coupled to the verified checked-operation diamond |
| `Shift` | No | Coupled to the verified checked-shift diamond |
| `CheckedF64ToInteger` | No | Coupled to range-check and failure structure |
| `TypeTest` | No | Object-view provenance and metadata reasoning are deferred |
| `OptionalPresence` and `OptionalBoxPresence` | No | Optional guard, owner-liveness, and memory reasoning are deferred |
| `ArrayLength` | No | Array storage, alias, and lifetime reasoning are deferred |

Every `MirRvalueKind` variant is matched explicitly without a wildcard.
Adding a new rvalue therefore forces a reviewed eligibility decision.

All other `MirInstruction` variants are ineligible, including calls and I/O
even when they define an unused scalar result. Allocation, initialization,
copying, stores, cleanup, ownership transitions, checked-view operations, and
array operations remain untouched.

This whitelist is narrower than everything that may ultimately be proven
pure. Its purpose is to prove the framework without silently creating a
general effect contract. Later widening should occur in the pass or a reviewed
shared effect classification when another pass demonstrates a common need.

## MOP12 — Eliminate dead pure trees to a deterministic fixed point

The canary runs independently for every executable callable package in the
atomic whole-program rewrite:

1. compute exact value-use counts through the exhaustive identity traversal;
2. select unused eligible assignments in stable block and instruction order;
3. functionally remove those instructions;
4. remove their matching value declarations;
5. recompute the census and repeat while at least one definition was removed;
6. commit the callable once, compacting surviving values deterministically;
   and
7. return pass-owned counts plus ordinary rewrite maps and change summaries.

Recomputing by wave is intentionally simple for the canary. Each successful
wave removes at least one value, so termination is bounded by the original
value count. A later worklist implementation may replace it without changing
the maximal fixed-point result or stable measurements, provided equivalence is
tested.

Actual uses in terminators, calls, stores, places, path conditions, logical
records, and other proof metadata prevent removal. Declarations and definition
sites do not count as uses. Because verified MIR requires exactly one
definition for every declared value, instruction and declaration deletion stay
paired.

The pass makes no CFG edit and does not remove newly empty blocks. It does not
remove unused storage. It does not fold constants, replace uses, or reorder
surviving instructions. It preserves spans and relative order for everything
retained.

The pass is considered a successful framework canary only when:

- it runs through the production registry and default profile;
- `none` retains exact current MIR and assembly behavior;
- selective disabling makes `default` equivalent to `none` with one pass;
- direct, cascading, and no-op cases work across functions, every member kind,
  and static initializers;
- checked, memory-reading, callable-address, metadata-used, call, I/O,
  ownership, and lifecycle definitions remain;
- every changed result passes ordinary and lifecycle-realization verification;
- optimized native behavior and source diagnostics match unoptimized behavior;
- metrics distinguish processed from changed callables and report exact
  removals;
- input, after-pass, and final dumps are deterministic across independent
  processes; and
- the full repository, golden determinism, MSRV, documentation, formatting,
  lint, and diff gates pass.

## Frozen profile and selection examples

With the completed initial registry:

| Request | Resolved schedule |
|---|---|
| default request | [`dead-pure-definition-elimination`] |
| `--mir-optimization default` | [`dead-pure-definition-elimination`] |
| `--mir-optimization none` | [] |
| `--disable-mir-pass dead-pure-definition-elimination` | [] |
| default plus the same disable twice | [] |
| unknown disabled name | deterministic usage error before compilation |

When later profiles repeat cleanup passes, disabling one name removes all of
its occurrences. Exact schedule tests may still select one occurrence or
construct deliberate repetitions.

## Verification and failure matrix

| Situation | Required result |
|---|---|
| Invalid synthesized MIR, any profile | Initial verification failure; no pass executes |
| `none` profile, valid MIR | One verification; unchanged verified backend input |
| Selected pass finds no candidate | Unchanged outcome; no additional verification |
| Selected pass changes one or more callables | Atomic commit followed by immediate central verification |
| Rewrite detects a dangling or malformed identity | Pass-attributed rewrite failure; no checkpoint or backend |
| Changed result violates MIR semantics | Pass-attributed output-verification failure |
| First pass succeeds, second pass fails | No successful pipeline product is published |
| Pass occurs twice | Each occurrence has distinct ordered measurement and checkpoint identity |
| Pass disabled from a repeated profile | Every occurrence of that identity is absent |
| Runtime tracing enabled or omitted | Same target-independent schedule and MIR result |
| Compiler later parallelizes callable analysis | Same schedule, commit order, metrics, dumps, and errors |

## Testing strategy

### Registry and configuration

- unique identity and name validation;
- exact profile expansion and repeated occurrence numbering;
- deterministic exclusion resolution and sorted unknown-name errors;
- request builder defaults and equality;
- CLI parsing, help, conflicts, repetition, and usage status;
- exact schedule test API boundaries; and
- no pass activation through module or registry iteration order.

### Runner and trust boundary

- invalid input prevents every pass callback;
- unchanged pass retains the same seal and verification count;
- changed pass invalidates and immediately reseals;
- rewrite and output-verification failures identify exact occurrences;
- a later pass and backend cannot receive raw or failed MIR;
- compile-fail coverage for seal construction and rewrite capability leakage;
- functions, members, and static initializers run in deterministic order; and
- pipeline composition remains atomic across multiple selected passes.

### Reporting and inspection

- exact aggregate and per-pass integer measurements;
- changed versus processed callable accounting;
- pass events in schedule order;
- no pass logging or dump text in reports;
- input, after-pass, and final checkpoint ordering;
- unchanged checkpoint parity;
- repeated-pass labels; and
- independent-process dump, metric, schedule, and error determinism.

### Dead-pure canary

- one unused eligible assignment for every whitelisted rvalue family;
- one used eligible assignment for every family;
- cascading dead expression trees requiring multiple waves;
- mixed live and dead siblings with stable retained order;
- exact removal of instruction and value declaration;
- dense remapping of later value declarations and uses;
- function, instance method, static method, initializer, copy constructor,
  copy assignment, finalizer, and static initializer coverage;
- every excluded rvalue and value-defining non-assignment instruction;
- logical, path, checked-operation, function-value, optional, array, shared,
  I/O, and static-lifecycle fixtures;
- no-op exact MIR dump parity;
- optimized/unoptimized diagnostic parity;
- optimized/unoptimized native observation parity; and
- default, none, disabled, repeated test schedule, and full corpus coverage.

## Alternatives considered

### Add the first pass directly to `run_mir_pipeline`

This is initially smaller but hides identity, selection, ordering, reporting,
and failure policy in one function. The second pass would force the framework
design under compatibility pressure. The proposal establishes the durable
runner first and proves it with one canary.

### Build the framework without a production transformation

The current synthetic coordinator already demonstrates why this is
insufficient. A registry with only no-op callbacks cannot prove real edits,
change accounting, optimized dumps, default selection, or backend effects.

### Use dynamic trait objects or runtime plugins

They add object-safety, lifetime, ABI, discovery, and configuration complexity
without a current external-pass requirement. A typed static registry provides
modularity and selection inside the compiler.

### Let passes mutate raw `MirProgram` directly

This bypasses sparse edit transactions, dense compaction, atomic program
replacement, structured rewrite errors, and the single traversal authority.
It recreates the architectural constraint the identity-rewriting foundation
was built to remove.

### Verify only after the complete schedule

This reduces verification executions but permits one malformed pass product to
flow into later passes and obscures which occurrence violated the contract.
Per-changed-pass verification is the safe initial policy.

### Let passes declare that they preserve verification

A general preservation boolean has no precise proof meaning. It is especially
unsafe across lifecycle effects, proof metadata, ownership, and CFG changes.
The initial framework measures central verification before considering any
narrow shortcut.

### Expose arbitrary CLI pass order immediately

Exact schedules are useful for tests and internal experiments, but a public
ordering interface creates a larger compatibility and support surface.
Profiles plus disabling provide useful selection while keeping supported
composition explicit.

### Put pass timings and messages in logging

This bypasses request-scoped reporting, makes tests parse text, and couples
pass code to presentation. Structured measurements retain one owner for data
and one owner for rendering.

### Use constant folding as the canary

Constant folding requires a broader exact operation evaluator and decisions
about NaN payloads, checked failures, casts, and later propagation. Dead pure
definition elimination can use a very narrow exhaustive whitelist while
exercising the more important structural deletion and compaction path.

### Start with whole-program reachability

Reachability is the best next larger optimization, but its root model spans
entry/export policy, function values, dispatch, static lifecycle, ownership,
arrays, generated operations, and program-level definitions. It should consume
a proven selectable pipeline rather than define that pipeline incidentally.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A pass bypasses the final-MIR seal | Pipeline-owned capability; no seal constructor or raw mutable program exposure |
| A no-op pass causes unnecessary verification cost | Explicit unchanged outcome retains the existing seal |
| Pass order becomes nondeterministic | Profiles expand to immutable schedules; occurrence identity is positional |
| Repeated passes become ambiguous | Stable name plus schedule position and occurrence number |
| Selection changes source diagnostics | All diagnostics and lifecycle planning precede the optimizer; parity gates |
| New MIR variants are silently treated as pure | Exhaustive no-wildcard eligibility match |
| Dead values remain referenced by proof metadata | Value-use census reuses the exhaustive identity traversal |
| Instruction deletion leaves a dangling declaration | Instruction and `MirValue` deletion are paired in one transaction |
| Cascading dead trees are only partly removed | Deterministic fixed point with bounded progress |
| Reporting confuses visited and changed callables | Separate processed and explicit changed counts |
| Dumps expose malformed intermediate MIR | Inspect only initial and successfully resealed checkpoints |
| A premature analysis manager becomes architectural debt | Pass-local analysis and unconditional invalidation first |
| Default optimization destabilizes broad behavior | `none` parity, pass disabling, full goldens, determinism, MSRV, and native equivalence before activation |

## Effort and recommended delivery order

Overall effort is **medium to large**. Most mechanisms already exist, but
selection, pass-attributed errors, observation, request policy, and production
parity cross several ownership boundaries.

| Delivery slice | Relative effort | Primary result |
|---|---|---|
| Typed registry, identities, profiles, and schedule resolution | Small to medium | Deterministic selectable policy |
| Request and CLI configuration | Medium | User-visible none/default and disabling |
| Production verified runner and structured failures | Medium | Safe multi-pass execution |
| Per-pass measurements and clarified aggregate metrics | Medium | Observable work without logging |
| Verified checkpoint inspection | Small to medium | Deterministic optimized MIR dumps |
| Value-use census facade | Small to medium | Reusable exhaustive canary analysis |
| Dead-pure canary and broad parity hardening | Medium to large | First real production optimization |

The implementation roadmap preserves this dependency order:

1. settle registry, profile, schedule, and configuration types;
2. productionize the verified runner with no transformation registered;
3. add pass-attributed failures and exact schedule tests;
4. add structured per-pass measurement and verified inspection checkpoints;
5. expose the narrow exhaustive value-use census; and
6. finish by registering, enabling, and hardening the dead-pure-definition
   elimination canary.

The roadmap should not close until `default` contains the canary and `none`
proves the exact verification-only baseline. Whole-program reachability should
receive its own subsequent design and roadmap.

## Confirmation and promotion

MOP1 through MOP12 were confirmed together on 2026-08-30 because the canary's
soundness and usefulness depend on the registry, capability, verification,
selection, measurement, and traversal decisions as one boundary. The durable
contract is promoted into the living compiler phase, driver, and reporting
documents linked from this proposal's status. The implementation roadmap
divides delivery into reviewable tasks, and its dedicated discoveries record
keeps larger follow-ups outside active scope.

The confirmed decisions are:

- a static typed registry rather than runtime plugins;
- `none` and `default` profiles, with `default` eventually containing the
  canary;
- request and CLI profile selection plus pass disabling;
- exact schedules restricted initially to crate-private tests and tools;
- verified input and immediate reverification after every changed occurrence;
- no preservation declarations or analysis manager in the first framework;
- structured pass measurements and verified dump checkpoints;
- a narrow exhaustive dead-pure whitelist; and
- fixed-point instruction and value-declaration deletion as the final canary.

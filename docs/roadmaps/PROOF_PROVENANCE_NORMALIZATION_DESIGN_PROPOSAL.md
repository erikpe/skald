# Proof-Provenance Normalization Design Proposal

Status: draft design proposal; PPN1 through PPN14 await review. This proposal
resolves the architecture question recorded as
[proof-coupled logical CFG remains intentionally opaque](LOCAL_FINAL_MIR_SIMPLIFICATION_DISCOVERIES.md#proof-coupled-logical-cfg-remains-intentionally-opaque).

Skald's final MIR currently carries two kinds of information in one verified
product: executable operations consumed by the backend and provenance used to
prove that lowering preserved path-sensitive language rules. The latter names
exact blocks, values, and storage. That is valuable while verification still
needs the evidence, but it also makes already-consumed proof structure an
accidental permanent root of the executable CFG.

This proposal introduces a mandatory, one-way normalization boundary after
the last proof-sensitive verifier. It consumes path-condition and logical-
expression provenance, lowers their remaining runtime carrier operations to
ordinary executable MIR, and seals a distinct post-proof product. Later CFG
passes can then reason about executable reachability without having to update
source-proof records whose semantic job is complete.

The purpose is broader than deleting a few metadata records. The boundary
should provide a maintainable home for future CFG simplification, checked-
protocol normalization, inlining, and other transformations that are awkward
while final MIR simultaneously serves as executable program and lowering
certificate.

The language contract does not change. Compilation remains permanently
whole-world and resulting programs remain single threaded. Evaluation order,
checked failure, aliases, sequential mutation, allocation, ownership,
destruction, static activation and shutdown, diagnostics, panic locations,
and runtime traces remain observable exactly as before.

## Intended outcome

The design should provide:

- an explicit distinction between proof-rich verified final MIR and
  post-proof verified executable MIR;
- one exhaustive inventory of proof-bearing records and their last semantic
  consumers;
- one mandatory normalization transaction, independent of optimization
  profile and pass selection;
- ordinary executable loads in place of path-condition reads;
- no path-condition or logical-expression identities in the normalized
  product;
- a post-proof verifier with a deliberately stated trust boundary;
- stage-aware pass registration and inspection, so a pass cannot silently run
  against the wrong MIR contract;
- backend acceptance of only the normalized sealed product;
- reusable atomic CFG rewriting which protects permanent semantic roots but
  no longer protects consumed proof roots;
- deterministic normalization measurements and dumps;
- unchanged native behavior for `none`, default, and explicitly selected pass
  schedules; and
- one conservative production canary that demonstrates proof-only CFG roots
  can actually disappear before broader CFG work is attempted.

## Current architecture and evidence

### One seal currently has two responsibilities

Today, `VerifiedFinalMirProgram` proves ordinary MIR structure, path and logical
shapes, optional/array/shared ownership, storage lifetime, synthesized static
lifecycle, and target-independent reachability. Every changed optimization
invalidates that seal and reruns the complete verifier. The backend accepts
only the resealed product.

This is a strong boundary for transformations which preserve proof-rich MIR.
It becomes restrictive once a transformation wants to replace proof-shaped
topology with simpler but equivalent executable topology: complete
verification still expects the original proof vocabulary and exact block
relationships.

### Path conditions combine proof identity with an executable carrier

`MirPathCondition` records a parent condition, boolean activation storage,
active and inactive predecessor blocks, and their merge. `MirPathCondition`
values refer to both that proof identity and the activation storage.

The distinction already exists operationally. The backend ignores the
condition identity and lowers a path-condition value exactly as a load from
its activation storage. The condition identity, parent relation, and exact
predecessor/merge topology are consumed by verification, not execution.

`MirStorageKind::PathCondition` similarly tells proof-rich verification that
one boolean home is the canonical carrier for a condition. After proof
consumption it has the same executable role as other compiler-owned scalar
storage.

### Logical-expression records are non-executable provenance

`MirLogicalExpression` records the complete selected short-circuit lowering:
split, selection, right-hand entry and exit, short path, join, result storage,
and intermediate values. Its own model documentation explicitly states that
the record does not execute. The backend consumes only the branches, stores,
loads, and jumps which the record verifies.

### Proof metadata is a current CFG root

Callable-local CFG analysis protects every block named by a path-condition or
logical-expression record. Conservative CFG cleanup therefore retains blocks
which ordinary entry reachability no longer reaches. This is intentional:
deleting the block while retaining its proof record would leave invalid
identities, and deleting a subset of the record would discard evidence still
required by the current verifier.

That conservative rule blocks more than unreachable deletion. Empty-block
forwarding, block merging, jump threading, and short-circuit CFG
simplification all change predecessor or join topology named by the proof
records.

### The evidence has multiple final consumers

Path metadata is not merely a pretty representation of `and` and `or`.
`PathStates` uses it to partition dataflow facts, and the optional
initialization, array ownership, shared ownership, cleanup, and storage-
lifetime verifiers consume those partitions. Removing path metadata and then
running the existing complete verifier would lose information used to accept
valid lowered programs.

Consequently, normalization must happen only after all proof-sensitive
verification has succeeded. It cannot be an early cleanup pass or an
alternative implementation of the same complete verifier.

### Whole-world and single-threaded assumptions

Whole-world compilation lets the pipeline place one mandatory boundary after
all definitions and lifecycle regions have been verified. There is no
separate object or dynamically loaded module whose unconsumed proof needs to
be joined later.

Single-threaded execution means replacing a canonical condition read by the
same ordinary storage load introduces no concurrency concern. It does not,
however, make arbitrary storage loads stable or aliases harmless. The
normalization is an exact representation change, not general load
propagation.

## Comparison with Niflheim

Niflheim separates semantic optimization from a backend IR whose verifier and
CFG analysis are already expressed in executable terms. Its backend CFG pass
can fold branches, forward empty jump blocks, and remove entry-unreachable
blocks to a fixed point because backend blocks are not retained by a
path-sensitive source-proof table. Verification runs after every backend
pass.

The useful precedent is the phase separation: semantic evidence is consumed
before a lower executable representation receives broad CFG cleanup. Skald
should not copy Niflheim by introducing another full IR or by moving this work
into the target backend. Skald's final MIR remains the target-independent
optimization representation; it instead needs two verified stages of that
same representation with a named one-way transition between their contracts.

## Proof inventory and classification

The normalization owner must keep this inventory exhaustive. Adding a new MIR
record or operation which names proof topology must update the classification
and maintenance tests.

| MIR information | Classification at this boundary | Last or continuing consumer | Normalized treatment |
|---|---|---|---|
| `MirLogicalExpression` | Consumable proof provenance | Proof-rich logical-expression verifier | Remove the record after successful proof verification |
| `MirPathCondition` identity, parent, predecessors, merge, and span | Consumable proof provenance | Path-condition verifier and path-sensitive verifier dataflow | Remove the record after all proof-sensitive verifiers succeed |
| Condition identity on `MirRvalueKind::PathCondition` | Consumable proof reference | Proof-rich instruction, logical, and path-state verification | Remove by rewriting the rvalue |
| Activation storage read in `MirRvalueKind::PathCondition` | Permanent executable behavior | Backend load lowering and storage lifetime | Rewrite to `MirRvalueKind::Load(MirPlace::base(activation))` with identical result, type, and span |
| `MirStorageKind::PathCondition` classification | Consumable proof classification over permanent storage | Proof-rich path-condition verification | Reclassify the boolean storage as `ScalarSpill`; do not delete it during normalization |
| Ordinary stores selecting activation state | Permanent executable behavior | Backend and later dataflow/CFG passes | Retain unchanged; later storage optimization may prove them dead |
| Optional guard identities and begin/end protocol | Permanent verified executable protocol | Optional, lifetime, and backend lowering | Retain; this proposal does not normalize optional protocols |
| Checked integer/cast/array/ownership terminators | Permanent executable failure protocol | Verifier, optimizer, and backend | Retain until a separate protocol-specific normalization proves a replacement |
| Final-write and cell-write authorizations | Permanent verifier-visible operation authority | Write and lifecycle verification | Retain with their executable instruction; they do not root CFG blocks |
| Static publication attachments and their endpoint blocks | Permanent lifecycle semantics | Static-lifecycle verification, reachability, and backend | Retain as protected roots |
| Static-lifecycle proof and activation authority | Permanent whole-program authority | Reachability and backend retained-domain planning | Retain; it is not callable-local consumed provenance |
| Reachability graph and closure | Recomputable seal-bound analysis | Whole-world passes, verification, and backend | Recompute for every changed normalized product |
| Dense callable-local identities | Recomputable representation invariant | Rewriter, verifier, dumps, and backend | Compact atomically after normalization and every later rewrite |

“Consumable” means that the information has completed its semantic role and
has an exact executable replacement where one is needed. It does not mean
that a pass may delete it before proof-rich verification or preserve only a
convenient subset.

## Proposed representation boundary

### Two sealed products over one MIR model

Do not fork the entire MIR model. Use two wrappers with private constructors:

- rename the current proof-bearing wrapper to `VerifiedProofMirProgram`; and
- retain the established `VerifiedFinalMirProgram` name for the normalized
  post-proof product returned by the public pipeline and accepted by the
  backend.

The normalized wrapper contains a `MirProgram`, fresh target-independent
reachability facts, and a private consumed-proof authority created only by the
normalization owner. External code and ordinary rewrite helpers cannot forge
either seal.

Using a second wrapper rather than a second IR keeps instructions, identities,
dumps, and target-independent analyses shared. The type distinction makes the
stage visible at every pass and backend boundary and avoids a boolean
`proofs_normalized` flag whose invalid combinations would spread throughout
the compiler.

Public pipeline callers and the backend therefore retain the
`VerifiedFinalMirProgram` result name. Rename the proof-rich verifier to
`verify_proof_mir`; keep `verify_final_mir` only as a verify-and-normalize
convenience returning the backend-ready product if a public raw-MIR sealing
entry point is still required. No API named “final” may return the
proof-bearing intermediate product.

### Normalized executable invariant

A sealed executable product must satisfy all of the following:

- every definition passed complete proof-rich verification immediately before
  normalization;
- `path_conditions` and `logical_expressions` are empty in every callable;
- no rvalue, instruction, terminator, attachment, or retained record refers to
  a `PathConditionId`;
- no storage declaration has `MirStorageKind::PathCondition`;
- each former path read is an ordinary base-place load from the same boolean
  activation storage;
- every remaining local and global identity is dense or satisfies the
  established sparse-definition contract;
- every remaining executable reference resolves and has the expected owner
  and type;
- static-publication and static-lifecycle authority remain valid;
- reachable executable definitions remain present; and
- reachability facts are computed from this exact normalized program.

The empty tables remain in the shared MIR model initially so proof-rich and
normalized products can use the same structures and rewriting infrastructure.
Removing the variants or fields physically is a later cleanup decision after
the producer and verifier boundary is stable.

## Mandatory normalization transaction

Normalization is a compiler phase transition, not a selectable optimization
pass. It runs exactly once for every successful compilation, including the
`none` profile.

For each verified definition, the transaction must:

1. inventory every path condition, logical expression, activation storage,
   and path-condition read;
2. reject inconsistent ownership or references even though normal input was
   already verified, so the transaction remains defensive and testable;
3. replace every path-condition rvalue by an ordinary base-place load while
   preserving the assignment, `ValueId`, result type, and source span;
4. change each owned path-condition activation declaration to
   `MirStorageKind::ScalarSpill`, preserving its `StorageId`, type, name,
   scope, and lifetime operations;
5. delete all logical-expression and path-condition records together;
6. commit through the dense rewriting owner so no stale local identity can be
   published;
7. exhaustively prove that no consumed-proof identity or classification
   remains; and
8. run normalized verification and recompute reachability before sealing the
   result.

The transaction does not initially delete activation storage, selection
stores, result carriers, blocks, or values. Keeping normalization mechanical
separates proof consumption from liveness decisions. Ordinary post-proof
passes may remove entities when their own executable analyses prove them
unnecessary.

If any step fails, the pipeline reports a normalization-stage error and
publishes neither a partially normalized program nor a normalized seal.

## Verification and trust model

### Full proof verification happens before consumption

Every proof-rich input is checked by the existing complete final-MIR verifier.
All path-sensitive language-safety analyses run there. A pass which creates or
changes proof-bearing structure must remain before normalization and must
return to this verifier.

### Normalized verification has a narrower responsibility

The post-proof verifier does not attempt to reconstruct erased path
predicates or rerun analyses whose evidence was intentionally consumed. It
checks the normalized executable invariant, including:

- exhaustive absence of consumed proof references;
- dense local identities and valid global references;
- CFG successors, entries, and permanent protected roots;
- definitions, uses, types, places, storage lifetimes, and surviving protocol
  shapes which can be checked without erased predicates;
- static publication and lifecycle consistency;
- target-independent dependency extraction and reachable-definition
  completeness; and
- backend preconditions which are independent of source-proof provenance.

Existing verifier code should be divided into shared structural checks,
proof-rich checks, and normalized checks. Do not copy the full verifier and
let two implementations drift.

The private consumed-proof authority says only that the source program was
fully verified before the exact normalization transaction. It does not prove
that an arbitrary later rewrite is semantics preserving. As with every
optimizer, transformation correctness remains part of the pass contract;
normalized verification catches malformed products but is not an equivalence
oracle.

### Post-proof mutation is stage restricted

Introduce a post-proof rewrite capability rather than giving existing
proof-rich callbacks a flag. It may initially expose:

- ordinary terminator replacement;
- edge redirection;
- deletion of executable-entry-unreachable blocks and their owned values;
- deletion of now-unused ordinary values; and
- the existing atomic dense commit.

It must continue to protect body entry, static-publication endpoints, and any
future permanently semantic attachment. It cannot introduce path-condition or
logical records, change lifecycle authority, manufacture checked operations,
or expose raw mutable `MirProgram` access.

Broader operations such as empty-block forwarding, block merging, or jump
threading should extend this capability only when their exact predecessor,
storage-lifetime, checked-failure, ownership, and trace preconditions are
reviewed. The normalization design removes an architectural blocker; it does
not declare every CFG rewrite sound.

## Pipeline placement and selection

The pipeline becomes two typed pass regions separated by the mandatory
normalizer:

```text
raw final MIR
  -> complete proof-rich verification
  -> proof-rich selectable passes, with complete reverification
  -> mandatory proof-provenance normalization
  -> normalized executable verification
  -> post-proof selectable passes, with normalized reverification
  -> backend
```

Pass descriptors gain a closed stage classification. Schedule resolution must
reject a pass occurrence in the wrong region before execution. Profiles and
stable-name exclusions continue to select optimization passes; they do not
select, repeat, or disable normalization.

Use a closed `MirPassStage::{ProofRich, Final}` descriptor field. Pass listing
includes the stage so users can understand placement without treating the
mandatory boundary as a selectable pass.

The initial migration should keep primitive folding, algebraic
simplification, checked-integer folding, dead-pure elimination, and the current
proof-aware conservative CFG occurrence in the proof-rich region. Add one
post-proof CFG canary after normalization, then run whole-world reachability
last against the normalized product. This ordering lets newly unreachable
call sites disappear before final program-level retention.

Register the canary under the stable name
`post-proof-unreachable-block-elimination`. It is deliberately limited to
deleting blocks unreachable from the
callable's executable entries and permanent roots, together with block-owned
values through the existing dense transaction. It may fold an ordinary
constant branch if that operation is shared cleanly with the existing pass.
It does not initially forward empty blocks, merge blocks, thread jumps, delete
storage, or simplify checked protocols.

`none` changes from “verification only” to “verification plus mandatory
normalization.” That is an intentional compiler representation-contract
change, not an enabled optimization. With every selectable pass disabled,
native behavior and emitted machine operations must remain equivalent; the
backend currently lowers a path-condition read and the replacement ordinary
load identically. Proof-rich and normalized dumps are expected to differ.

## Inspection, reporting, and determinism

Inspection must make the phase boundary visible. Replace a checkpoint API
which assumes one seal type with a stage-tagged borrowed view:

- proof-rich input and after-pass checkpoints expose
  `VerifiedProofMirProgram`;
- a named `after-proof-normalization` checkpoint exposes
  `VerifiedFinalMirProgram`; and
- post-proof after-pass and final checkpoints expose the executable product.

The view must remain read only and must not let a consumer retain a borrow or
detach reachability facts. Dump filenames and display labels include the
stage so two different contracts are never both called merely `final`.

Normalization reports deterministic structural counts:

- path-condition records consumed;
- logical-expression records consumed;
- path reads lowered;
- activation storage declarations reclassified;
- callables changed; and
- proof-protected blocks released to executable reachability.

These are phase measurements, not pass-occurrence measurements, because the
normalizer is mandatory and non-selectable. The post-proof canary separately
reports its ordinary pass measurements, including removed blocks and values.

The compiler must produce byte-for-byte deterministic normalized MIR dumps,
measurements, pass ordering, dense identity maps, reachability dumps, and
assembly across repeated and independent-process runs.

## Language, diagnostic, and runtime contract

This proposal changes no source semantics and adds no language feature or
attribute. It is triggered automatically after successful proof-rich final-
MIR verification; source code cannot request, suppress, or observe the
boundary.

In particular:

- short-circuit right operands still execute exactly when selected;
- sequential mutations through aliases remain observable;
- checked failures still occur at the same dynamic operation with the same
  reason and source location;
- ownership and destruction order remain unchanged;
- inactive and active static lifecycle behavior remains governed by the
  existing reachability-gated contract;
- runtime stack and panic traces retain source callable/span identities; and
- compile-time diagnostics and their order remain independent of optimization
  profile.

Whole-world and single-threaded execution justify one complete phase boundary
and remove concurrency from the carrier-load equivalence. They do not justify
deleting executable effects or weakening alias and lifecycle rules.

## Scope and non-goals

- Do not add another general-purpose MIR or an SSA representation.
- Do not physically remove proof-rich model variants in the first delivery.
- Do not recompute source proof after normalization.
- Do not make normalization selectable or target specific.
- Do not fold dynamic loads, infer immutable memory, or add alias/effect
  analysis.
- Do not normalize optional, checked cast, array, shared-ownership, cleanup,
  or checked integer protocols in this work.
- Do not delete activation/result storage as part of normalization.
- Do not implement empty-block forwarding, block merging, jump threading,
  logical CFG simplification, loop canonicalization, or inlining yet.
- Do not weaken static-publication, lifecycle, reachable-definition, or
  backend verification.
- Do not make post-proof verification a silent subset selected by individual
  passes.
- Do not rely on backend machine-artifact pruning to repair malformed MIR.

## Verification and test strategy

### Classification and normalization tests

- exhaustive maintenance tests for every MIR identity-bearing site and every
  storage/rvalue variant;
- nested `and`/`or` path conditions, including parent relationships;
- path-sensitive optional initialization, array ownership, shared ownership,
  cleanup, and storage-lifetime examples accepted before consumption;
- exact rewrite of path reads to ordinary loads with preserved identity, type,
  place, and span;
- exact reclassification of only condition activation storage;
- zero path/logical records and zero remaining condition references;
- atomic failure on malformed ownership or unexpected proof references; and
- deterministic dense commit without incidental reordering.

### Seal and verifier tests

- compile-fail tests proving neither seal nor consumed-proof authority is
  forgeable;
- proof-rich verification rejecting malformed source-proof topology;
- normalized verification rejecting leaked records, variants, storage kinds,
  invalid edges, missing definitions, and stale lifecycle authority;
- normalized verification not pretending to rerun erased path-state proofs;
- unchanged proof-rich seals when pre-normalization passes make no change; and
- fresh normalized reachability facts after normalization and every changed
  post-proof pass.

### Pipeline and canary tests

- exact stage classification for every registered pass;
- rejection of wrong-stage schedules;
- mandatory normalization under `none`, default, and all exclusions;
- one normalization checkpoint in deterministic order;
- deletion of a formerly proof-protected but executable-unreachable logical
  region by the post-proof canary;
- preservation of body entry, static publication, shutdown, checked-failure,
  and other permanent regions;
- final whole-world retention observing call sites removed by the canary; and
- deterministic measurements for repeated pass occurrences.

### Semantic and repository gates

- native equivalence across logical, optional, arrays, shared ownership,
  cleanup, casts, checked integers, statics, panic, and runtime-trace suites;
- optimization-off assembly equivalence for the representation-only
  normalization where the backend already emitted the same load;
- golden proof-rich and normalized MIR fixtures;
- full root Makefile quality gate, golden/native suites, documentation link
  checks, formatter and linter checks, and supported MSRV gate for Rust or
  manifest changes.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Proof is consumed before its last verifier | One centralized inventory, typed phase order, and tests whose acceptance requires path-sensitive facts |
| A path read changes runtime behavior | Mechanical rewrite to the same activation storage; backend parity and native tests |
| A remaining reference dangles after record deletion | Exhaustive identity traversal and normalized zero-proof invariant before sealing |
| The post-proof verifier gives a false impression of reproving source safety | Explicit authority semantics and separate names for proof-rich and normalized verification |
| A later pass performs an unsafe rewrite which the normalized verifier cannot detect | Stage-specific narrow mutation capability, reviewed pass contracts, immediate structural verification, and semantic equivalence tests |
| Static lifecycle blocks become ordinary dead CFG | Keep publication endpoints and lifecycle authority as permanent roots |
| Two verifier implementations drift | Factor shared structural owners; isolate only proof-rich and normalized-specific checks |
| `none` ceases to be a useful reference | Keep all optimization passes disabled, expose both stage dumps, and require representation-only assembly/native parity |
| A new proof-bearing MIR variant bypasses normalization | Closed exhaustive classification and compile-time/test maintenance points |
| The boundary turns into a second optimizer | Normalizer performs only exact representation conversion; liveness remains in named selectable passes |
| Global reachability observes stale pre-normalization facts | Recompute facts when sealing normalized MIR and keep whole-world retention last |
| Public APIs become ambiguous | Use explicit product and checkpoint names; migrate callers in one roadmap slice |

## Alternatives considered

### Teach every CFG pass to rewrite proof records

This retains one verifier but spreads knowledge of path parents, logical
selection, ownership dataflow, and exact joins through every optimization.
Each new CFG transform would need to preserve a source-lowering certificate
which no longer has a semantic consumer. The coupling and test burden grow
with every pass.

### Keep proof records as permanent CFG roots

This is the safe current behavior, but it leaves the named optimization
barrier intact and makes consumed source provenance dictate executable shape
indefinitely.

### Delete records and continue using the current verifier

The current verifier uses path predicates to establish valid optional,
ownership, cleanup, and lifetime states. Erasure would either reject valid
programs or tempt the implementation to weaken those checks globally.

### Lower directly from proof-rich MIR into a backend CFG optimizer

That could recover some machine-level cleanup but would not enable
target-independent reachability, protocol, inlining, or CFG transformations.
It would also duplicate the solution for every backend.

### Introduce a wholly separate normalized optimization IR

A new IR could encode cleaner invariants but would duplicate most final-MIR
instructions, types, identities, dumps, verifiers, and lowering. The current
problem requires a different seal and contract, not yet a different data
model. Scalar SSA or a target LIR should be justified separately by their own
optimization needs.

### Make proof normalization an optional pass

Profiles would then produce two different backend input contracts, passes
would require conditional scheduling, and disabling optimization would retain
the very coupling the architecture is meant to remove. A mandatory phase
transition is simpler and testable independently of optimization policy.

## Effort and recommended delivery order

Overall effort is **large**. The mechanical record conversion is modest; the
substantial work is splitting verification and pipeline products without
weakening proof-sensitive safety or duplicating owners.

| Delivery slice | Relative effort | Primary result |
|---|---|---|
| Exhaustive proof inventory and normalized invariant | Medium | One reviewable classification maintenance point |
| Two seals and shared/proof-rich/normalized verifier ownership | Large | Explicit trust boundary without verifier duplication |
| Atomic proof-provenance normalizer | Medium to large | Exact one-way executable representation conversion |
| Stage-aware registry, runner, inspection, and failure model | Large | Passes and tools cannot confuse the two contracts |
| Backend and reachability migration | Medium to large | Only normalized sealed MIR reaches target lowering |
| Post-proof CFG canary and dense cleanup | Medium | First demonstrated removal of proof-only roots |
| Parity, native, determinism, and documentation hardening | Medium to large | Safe mandatory activation under every profile |

The eventual implementation roadmap should preserve this order:

1. freeze the inventory, stage names, seal responsibilities, and `none`
   semantics;
2. factor verification into shared, proof-rich, and normalized owners before
   changing accepted products;
3. add the normalized seal and atomic normalizer with exhaustive zero-proof
   checks;
4. make scheduling, errors, measurements, and inspection explicitly
   stage-aware;
5. migrate reachability and the backend to normalized input;
6. add the entry-unreachable post-proof CFG canary without broader topology
   rewrites;
7. prove semantic, assembly, inspection, and determinism parity; and
8. promote the durable phase, driver, backend, reporting, and testing
   contracts before enabling broader CFG candidates.

Empty-block forwarding, block merging, jump threading, short-circuit
simplification, checked-protocol normalization, and inlining remain separate
catalog candidates. They should consume this foundation rather than silently
expand its first implementation roadmap.

## Proposed decisions

### PPN1 — Consume proof only after complete proof-rich verification

Path and logical evidence remains mandatory until every path-sensitive
verifier has succeeded. Normalization is one way and cannot run earlier.

### PPN2 — Use two clearly named seals over one MIR model

Rename the current proof-bearing product to `VerifiedProofMirProgram` and
retain `VerifiedFinalMirProgram` for the normalized backend product. Do not
fork the MIR instruction model or leave a “final” API returning an
intermediate proof-rich product.

### PPN3 — Classify records exhaustively

Maintain one closed inventory distinguishing permanent semantics, consumable
proof, and recomputable analysis. Unknown identity-bearing records are a
normalization error, not implicitly disposable metadata.

### PPN4 — Erase logical and path-condition provenance atomically

Remove all `MirLogicalExpression` and `MirPathCondition` records together,
after rewriting every remaining executable carrier use.

### PPN5 — Lower condition reads to ordinary storage loads

Replace each path-condition rvalue with a base-place load of the same
activation storage while retaining its assignment, value identity, type, and
span.

### PPN6 — Reclassify, but do not delete, activation storage

Change `MirStorageKind::PathCondition` to `ScalarSpill`. Storage, stores,
lifetime operations, and blocks remain until a later ordinary optimization
proves them dead.

### PPN7 — Make normalization mandatory and non-selectable

Run it exactly once for every profile, including `none`. Pass listing,
selection, exclusion, and repetition apply only to optimization passes.

### PPN8 — Give normalized verification an explicit narrower contract

Share structural checks, verify the zero-proof executable invariant and all
surviving semantics, and do not claim to reconstruct consumed path proofs.

### PPN9 — Restrict post-proof mutation by type and capability

Only post-proof pass descriptors and rewrite capabilities accept the
normalized seal. Permanent lifecycle roots stay protected, and capability
growth follows reviewed transformation families.

### PPN10 — Recompute reachability for normalized products

Bind fresh dependency and closure facts to the normalized seal and every
changed post-proof result. Run selectable whole-world retention after the
initial post-proof CFG cleanup.

### PPN11 — Make inspection and failures stage explicit

Expose proof-rich and executable checkpoint variants plus a named
normalization checkpoint and failure stage. Never present both products under
one ambiguous borrowed type.

### PPN12 — Validate the boundary with one conservative CFG canary

Register `post-proof-unreachable-block-elimination`, initially deleting only
executable-entry-unreachable blocks and their owned values while retaining
permanent roots. Defer forwarding, merging, threading, storage deletion, and
protocol simplification.

### PPN13 — Preserve all language and runtime behavior

The work is a compiler representation and verification-boundary change only.
Whole-world and single-threaded guarantees do not weaken mutation, alias,
failure, ownership, or lifecycle semantics.

### PPN14 — Promote broader CFG work separately

FMC-08 through FMC-15 and checked-protocol normalization require their own
evidence and reviewed designs after this boundary is implemented and measured.

## Confirmation and promotion

PPN1 through PPN14 should freeze together. The classification, conversion,
seal, verifier, pipeline, backend, and `none` decisions form one trust
boundary; confirming only record deletion would leave the unsafe and awkward
parts implicit.

After confirmation, create a dedicated implementation roadmap and discoveries
record. Promote the durable contracts into compiler phase, driver, backend,
reporting, and testing documentation as their implementation slices land.
Advance FMC-07 and the cross-cutting proof-normalization catalog entry from
**Draft design** to **Proposed** only when this proposal is frozen.

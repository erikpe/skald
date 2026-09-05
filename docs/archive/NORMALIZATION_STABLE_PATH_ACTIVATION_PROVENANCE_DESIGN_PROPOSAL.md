# Normalization-Stable Path-Activation Provenance Design Proposal

Status: frozen design; NSP1 through NSP14 were accepted on 2026-09-05,
implemented, and promoted into the living compiler contracts. The completed
[normalization-stable path-activation provenance roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md)
records delivery.

This proposal resolves the remaining storage-classification limitation from
the completed
[proof-provenance normalization work](PROOF_PROVENANCE_NORMALIZATION_ROADMAP.md):
after path proof is consumed, a path activation currently becomes an ordinary
`ScalarSpill`, so normalized MIR cannot distinguish it from other compiler-
owned scalar homes.

The primary purpose is to strengthen the normalized final-MIR contract before
broader storage transformations are introduced. The current one-way seal makes
the representation sound, but the normalized definite-initialization verifier
must exempt every `ScalarSpill` because some of them are former path
activations whose initialization was proved only by consumed path provenance.
A stable, final-only storage classification can narrow that exemption to the
exact carriers which require it and give future passes an explicit semantic
barrier or target.

This is representation and verification work, not the dead-carrier
optimization itself. It does not delete storage, stores, loads, lifetime
operations, blocks, or values, and it does not reconstruct or retain consumed
path-condition identities.

## Intended outcome

The design should provide:

- a distinct `MirStorageKind::NormalizedPathActivation` for the executable
  boolean carrier left after path proof is consumed;
- explicit phase legality: `PathCondition` belongs only to proof-rich MIR and
  `NormalizedPathActivation` belongs only to normalized MIR;
- normalizer-exclusive conversion between those two classifications;
- ordinary definite-initialization checking for genuine `ScalarSpill`
  storage under both verifier contracts;
- a narrowly documented consumed-proof exception only for normalized path
  activations;
- exhaustive classification in the MIR model, verifier, rewriter, dump, and
  backend-facing storage vocabulary;
- unchanged `StorageId`, executable operations, spans, evaluation order,
  lifetime behavior, ABI, frame layout, assembly, and native behavior;
- a stable semantic hook for later dead condition-carrier cleanup or other
  reviewed storage transformations; and
- no new language, runtime, target, or user-facing optimization-selection
  contract.

## Non-goals

This proposal does not:

- retain `MirPathCondition`, `PathConditionId`, parent relations, predecessor
  identities, or logical-expression records after normalization;
- reconstruct path-sensitive definite-initialization dataflow in normalized
  MIR;
- generalize constant propagation to arbitrary storage;
- classify every `ScalarSpill` by its lowering-site history;
- add dead-store, dead-storage, store-to-load, scalar-promotion, alias,
  lifetime-shortening, or frame-slot optimization;
- change the proof-rich verifier's acceptance of currently valid producer MIR;
- broaden any existing final-stage mutation capability;
- add an optional normalization profile or pass; or
- change Skald's whole-world, single-threaded, failure, ownership, cleanup,
  static-lifecycle, or runtime-trace semantics.

## Current architecture and evidence

### Proof-rich path activation has an exact classification

Lowering creates a boolean `MirStorageKind::PathCondition` declaration for
each canonical path activation. `MirPathCondition` records name that storage
together with the condition identity, parent, active and inactive
predecessors, and merge. `MirRvalueKind::PathCondition` reads carry both the
proof identity and the executable activation storage.

Complete proof-rich verification uses those records to establish correlated
path-sensitive initialization, ownership, cleanup, and lifetime facts. It also
runs ordinary scalar definite-initialization checks over compiler-owned
`ScalarSpill` storage.

### Normalization erases proof but retains execution

The mandatory normalizer validates a complete immutable inventory and then:

- rewrites each path-condition read into an ordinary load from the same base
  storage;
- changes the activation storage kind from `PathCondition` to `ScalarSpill`;
- removes path-condition and logical-expression records; and
- seals the result only after normalized verification succeeds.

The storage identity, stores, loads, lifetime markers, blocks, values, types,
and spans survive unchanged. This exact conversion is profile-independent and
is composed atomically with the optional constant-left logical selection plan.

### The current normalized verifier has an intentionally broad exception

Ordinary primitive definite-initialization analysis cannot prove every former
path activation after the path relation has been erased. Consequently,
normalized verification currently excludes all `ScalarSpill` declarations
from that analysis. Complete proof-rich verification and the private
`ConsumedProofAuthority` make this sound for the current pipeline, while
surviving checked-protocol verifiers continue checking their exact carrier
shapes.

The exception is nevertheless broader than its reason. A genuine scalar spill
and a former path activation have the same storage kind, so the normalized
verifier cannot continue the ordinary spill check for one while trusting
consumed path proof for the other. A future storage pass would also have no
stable way to distinguish a path-owned carrier from an ordinary compiler
spill.

### Existing final-stage passes do not require the distinction

Post-proof unreachable-block deletion, empty-block forwarding, basic-block
merging, dead-pure-definition elimination, and whole-world definition
retention do not synthesize or move storage accesses. Their narrow mutation
surfaces and semantic tests preserve the consumed-proof assumption.

The convergent local constant solver does not close this gap. It certifies
checked-protocol `ScalarSpill` carriers against proof-rich MIR and discards all
certificates at the next seal. It neither consumes normalized activation
storage nor publishes persistent carrier provenance.

The immediate benefit of this proposal is therefore verifier precision and a
more explicit stage contract. The later optimization benefit is that dead
normalized condition-carrier cleanup and future storage passes can classify
the carrier without names, lowering history, erased proof records, or stale
identities.

### Whole-world and single-threaded assumptions

Whole-world compilation ensures that every path activation is lowered,
verified, normalized, optimized, and emitted in one closed pipeline. No
separately compiled body can introduce an unknown storage classification.

Single-threaded generated programs avoid concurrent observation of activation
stores. They do not make an uninitialized load valid, permit arbitrary load
movement, or weaken sequential alias and lifetime rules. The proposal remains
an exact representation refinement rather than an optimization justified by
concurrency assumptions.

## Niflheim comparison

Niflheim does not expose an equivalent Skald path-condition carrier whose
source-proof identity is consumed while its executable storage survives. Its
useful precedent remains phase separation: optimization-facing executable IR
keeps the classifications needed by its own verifier instead of relying on
erased source-analysis history.

Skald should apply that principle within its existing MIR rather than copy a
different IR model. The two-seal boundary, `StorageId` domain, dense rewriting,
and backend-ready final MIR remain authoritative.

## Proposed decisions

| Decision | Question | Proposed answer | Status |
|---|---|---|---|
| [NSP1](#nsp1--add-one-dedicated-normalized-storage-kind) | How is the origin represented? | Add `MirStorageKind::NormalizedPathActivation` | **Frozen** |
| [NSP2](#nsp2--retain-a-classification-not-consumed-proof) | What survives normalization? | Only a storage-role classification; no path or logical identity | **Frozen** |
| [NSP3](#nsp3--make-the-normalizer-the-only-producer) | Who may create the kind? | Only the validated mandatory normalizer | **Frozen** |
| [NSP4](#nsp4--make-storage-phase-legality-explicit) | Where is each form legal? | `PathCondition` only before the boundary; normalized activation only after it | **Frozen** |
| [NSP5](#nsp5--narrow-the-definite-initialization-exception) | What changes in scalar initialization? | Verify ordinary spills in both stages; exempt only normalized activations | **Frozen** |
| [NSP6](#nsp6--validate-the-surviving-role-structurally) | What can normalized MIR still prove? | Exact type/source/phase shape, while initialization relies on consumed authority | **Frozen** |
| [NSP7](#nsp7--preserve-identities-and-executable-shape) | Does normalization otherwise change? | No; preserve every executable operation and identity | **Frozen** |
| [NSP8](#nsp8--keep-rewriting-phase-aware-and-exhaustive) | How do rewrites treat the kind? | Preserve it by default; storage-mutating capabilities must handle it explicitly | **Frozen** |
| [NSP9](#nsp9--provide-one-semantic-query-surface) | How do analyses identify it? | Query the storage kind through a narrow MIR helper; never infer names or shapes | **Frozen** |
| [NSP10](#nsp10--keep-backend-representation-identical) | Does target lowering change? | Treat it as the same boolean stack home as the former scalar spill | **Frozen** |
| [NSP11](#nsp11--make-observation-deliberate-and-deterministic) | What changes in dumps and reports? | A distinct final-MIR storage label; existing normalization count remains | **Frozen** |
| [NSP12](#nsp12--leave-dead-carrier-deletion-separate) | Is FMM-13 included? | No; this supplies its provenance prerequisite only | **Frozen** |
| [NSP13](#nsp13--make-no-language-runtime-or-selection-change) | Does observable behavior change? | No | **Frozen** |
| [NSP14](#nsp14--retain-facade-oriented-ownership) | Where does implementation live? | MIR model/contract, normalizer, verifier, dump, and existing backend classifiers | **Frozen** |

## NSP1 — Add one dedicated normalized storage kind

Add the unit variant:

```rust
MirStorageKind::NormalizedPathActivation
```

It means: “compiler-owned boolean activation storage whose path-sensitive
initialization was established by the proof-rich verifier and whose proof
record was consumed by the mandatory normalizer.” It is not a source binding,
general temporary, checked-protocol carrier, or arbitrary scalar spill.

A dedicated variant matches existing MIR practice: `PathCondition`,
`OptionalUnwrap`, `ArrayPosition`, and other protocol-specific homes already
use storage kinds to encode semantic handling. It avoids adding an orthogonal
optional origin field whose combinations with every other storage kind would
need validation.

Do not change `ScalarSpill` into a payload-bearing generalized origin enum in
this proposal. Most current spills need no persistent lowering history, and no
second consumer yet demonstrates a stable shared origin taxonomy. A future
roadmap may generalize storage protocol ownership if effect, alias, or scalar-
promotion work finds multiple recurring classifications.

## NSP2 — Retain a classification, not consumed proof

`NormalizedPathActivation` carries no `PathConditionId`, logical-expression
identity, predecessor, merge, parent, span, or source-expression reference.
Those identities remain consumed proof and must still be absent from final
MIR.

The new variant records only the continuing semantic role needed by the
normalized verifier and later transformations. Because it contains no local
identity, dense block/value/storage rewriting cannot make it stale, and a
block merge does not require updating it.

The final seal's private `ConsumedProofAuthority` remains the evidence that
the exact input was completely proof-verified and passed through the complete
normalization transaction. The storage kind complements that authority; it
does not replace or expose the authority.

## NSP3 — Make the normalizer the only producer

Source lowering and preliminary/final MIR construction continue producing
`PathCondition`. No lowering helper, optimization pass, importer, test fixture,
or public constructor should create `NormalizedPathActivation` as ordinary
proof-rich MIR.

The normalizer may create the new kind only while applying a plan whose
inventory already proved that:

- the storage belongs to the same callable as one exact path condition;
- the declaration is `PathCondition`, has type `bool`, and has no source
  binding;
- no second path condition owns it;
- every path read resolves to the same activation; and
- the proof-rich program has already passed complete verification.

The conversion remains in the same unpublished atomic transaction as path-read
rewriting and proof-record deletion. An inventory or rewrite failure publishes
neither a partially reclassified program nor a consumed-proof token.

## NSP4 — Make storage phase legality explicit

Storage classification needs an exhaustive phase-legality decision in addition
to the existing proof-disposition decision:

| Storage kind | Proof-rich contract | Normalized contract | Proof disposition |
|---|---|---|---|
| `PathCondition` | Required for canonical path activation | Rejected | Executable carrier with consumable proof |
| `NormalizedPathActivation` | Rejected | Accepted only under consumed-proof sealing | Permanent executable classification |
| `ScalarSpill` | Accepted and checked | Accepted and checked | Permanent executable storage |

Implement this through one closed verifier-owned classification, not scattered
ad-hoc conditions. The existing exhaustive storage-kind maintenance tests must
cover both proof disposition and phase legality, so a later storage kind cannot
silently become legal in both stages.

The crate-private structural normalized checker may validate synthetic test
fixtures containing the new kind, but public final-seal construction still
requires the unforgeable normalization authority. Proof-rich verification of
raw MIR must reject a normalized-only activation before it could be used to
bypass path proof.

## NSP5 — Narrow the definite-initialization exception

Remove the stage-wide `verify_scalar_spills` switch. Ordinary primitive
definite-initialization analysis should include `ScalarSpill` under both
proof-rich and normalized contracts.

It should exclude only `NormalizedPathActivation`, because that exact class is
the one whose valid initialization relation depends on consumed path proof.
`PathCondition` continues through the proof-rich path-sensitive verification
and its existing storage checks.

This produces an immediate invariant improvement: a malformed or future final-
stage rewrite which leaves an ordinary scalar spill load uninitialized is once
again rejected by normalized verification. The consumed-proof exception is no
longer inherited accidentally by checked carriers and unrelated compiler
spills.

## NSP6 — Validate the surviving role structurally

Normalized verification must check every `NormalizedPathActivation`
declaration that remains:

- its type is exactly `bool`;
- it has no source binding;
- it appears only under the normalized contract;
- it contains no proof identity or proof-bearing rvalue reference;
- all storage references are callable-local and structurally valid; and
- ordinary storage lifetime and place validity rules which remain meaningful
  without path proof still apply.

Do not run ordinary intersection-based definite-initialization over this class
and claim to reproduce the consumed path proof. That analysis rejected valid
short-circuit and conditional-cleanup MIR because the necessary correlation is
no longer represented.

The final seal instead states two facts honestly: structural normalized checks
passed, and the exact ancestry of these marked carriers was verified before
the normalizer consumed the proof. Later passes remain responsible for
semantic equivalence and may not synthesize or move marked accesses through a
capability which does not explicitly authorize them.

## NSP7 — Preserve identities and executable shape

Normalization changes only the storage-kind discriminant for each validated
activation. It preserves:

- the exact `StorageId`, declaration order, name, type, and span;
- every store, load, `StorageLive`, and `StorageDead` operation;
- every `ValueId`, `BlockId`, instruction index, and terminator;
- evaluation and failure order;
- cleanup, ownership, and static-lifecycle behavior; and
- target-independent reachability and backend-visible execution.

The existing path-read rewrite remains exact. No new value, instruction,
block, declaration, or persistent record is inserted.

## NSP8 — Keep rewriting phase-aware and exhaustive

Identity traversal and dense remapping treat the new unit variant as an
identity-free storage declaration. Callable import preserves it when operating
on normalized MIR, just as it preserves other compiler-owned storage kinds;
phase verification remains responsible for rejecting it in proof-rich output.

Existing final-stage CFG-only capabilities may retain, move with a merged
block, or delete blocks containing marked storage operations only under their
already reviewed semantic rules. They do not gain permission to synthesize,
move, combine, or retarget individual activation loads and stores.

Any future storage-edit capability must give `NormalizedPathActivation` an
explicit disposition. The conservative default is rejection. Dead storage may
be deleted only by a separately reviewed pass after proving there is no
material read, write, lifetime, attachment, ownership, failure, or backend
dependency.

## NSP9 — Provide one semantic query surface

Analyses and passes should identify normalized activations from the storage
classification, optionally through a concise verifier/MIR helper where that
avoids duplicate matches. They must not infer ownership from:

- generated names such as `condition` or `spill`;
- boolean type alone;
- a store/load topology resembling current lowering;
- source spans;
- declaration position; or
- a remembered pre-normalization `StorageId` set outside the sealed product.

No analysis manager or cross-seal cache is introduced. The kind lives in MIR
and therefore follows the exact current program through ordinary immutable
queries and verified rewrites.

## NSP10 — Keep backend representation identical

The backend treats `NormalizedPathActivation` as an ordinary addressable
boolean stack home with the same size, alignment, frame-slot allocation, load,
store, and lifetime handling currently used after reclassification to
`ScalarSpill`.

It remains illegal for the backend to receive `PathCondition` storage or a
path-condition rvalue. The new kind is accepted only through
`VerifiedFinalMirProgram`; it does not create a new ABI category, register
class, relocation, runtime symbol, or machine instruction.

Assembly and native behavior must remain byte-for-byte unchanged when no dump
or report output is requested. Any backend match which distinguishes storage
kinds must classify the new variant explicitly rather than rely on a wildcard.

## NSP11 — Make observation deliberate and deterministic

Proof-rich MIR dumps continue spelling the carrier `path-condition`.
Normalized/final MIR dumps spell the surviving role
`normalized-path-activation` with a matching generated-source placeholder.
This intentional dump change exposes the stronger final contract instead of
hiding it behind `scalar-spill`.

The existing normalization metric “activation storage declarations
reclassified” remains accurate and keeps its public label and ordering. No new
pass occurrence, duration, profile, exclusion, or driver option is added.
Deterministic dump and report tests pin both stage spellings and repeated-run
order.

## NSP12 — Leave dead-carrier deletion separate

This proposal does not implement the catalog's dead normalized condition-
carrier storage cleanup candidate. After this foundation, that candidate can
move from “foundation needed” to a concrete follow-up whose analysis can target
`NormalizedPathActivation` directly.

The later pass must independently specify:

- what constitutes a material load, store, lifetime, attachment, and backend
  use;
- whether it deletes only declarations or complete store/load/lifetime
  protocols;
- its position relative to post-proof unreachable deletion, CFG
  canonicalization, dead-pure cleanup, and frame planning;
- atomic dense rewriting and stale-plan handling; and
- optimization-off, lifecycle, runtime-trace, and native equivalence.

Keeping deletion separate prevents the mandatory normalizer from becoming an
optimizer and preserves `none` as a representation-only reference profile.

## NSP13 — Make no language, runtime, or selection change

No Skald program can observe a MIR storage-kind discriminant. The language
still evaluates the same condition expressions and short-circuit paths, and
the generated program remains whole-world and single threaded.

The change does not alter source acceptance, diagnostics, panic locations,
evaluation order, aliases, mutation, allocation, ownership, destruction,
static activation/shutdown, runtime traces, ABI, or target support. Mandatory
normalization still runs exactly once under `default`, `none`, every exclusion
set, and an all-disabled schedule.

## NSP14 — Retain facade-oriented ownership

Implementation responsibility remains with existing owners:

- `mir::model` owns the storage-kind vocabulary and documentation;
- `mir::verify::contract` owns proof disposition and phase legality;
- `passes::pipeline::normalization` owns the sole conversion and statistics;
- `mir::verify::scalar_initialization` owns ordinary spill dataflow and the
  narrowed exception;
- existing general body/lifetime/place verifiers own structural validity;
- `mir::dump` owns the stable textual spelling;
- MIR import/rewrite owners preserve and exhaustively classify the kind; and
- the backend owns identical frame and machine treatment.

Do not introduce a new general provenance subsystem, public metadata API, or
parallel verifier. Add a small helper or submodule only where it centralizes a
repeated phase-classification responsibility.

## Normalization and verification sequence

The complete intended transaction is:

```text
proof-rich MirProgram
  -> complete proof-rich verification
  -> immutable normalization/optional-transition plan
  -> optional logical edge/result rewrites
  -> PathCondition rvalue -> ordinary Load
  -> PathCondition storage -> NormalizedPathActivation
  -> erase path/logical proof records
  -> normalized structural verification
       - reject proof-rich forms
       - accept only structurally valid normalized activations
       - verify ordinary ScalarSpill initialization again
  -> recompute reachability
  -> VerifiedFinalMirProgram + ConsumedProofAuthority
```

After any changed final-stage pass, normalized resealing repeats the structural
checks and ordinary spill initialization. The consumed authority travels with
the invalidated final product, while the explicit storage kind tells the
verifier which exact declarations still depend on that authority.

## Error ownership

Failures remain compiler-internal structured MIR verification or normalization
errors:

- proof-rich MIR containing `NormalizedPathActivation` is a phase-legality
  violation;
- normalized MIR retaining `PathCondition` remains a proof-bearing-storage
  violation;
- a normalized activation with a source binding or non-`bool` type is a
  storage-contract violation;
- an orphan or duplicate proof-rich activation remains a normalization
  inventory failure;
- an ordinary uninitialized `ScalarSpill` in normalized MIR is again a
  definite-initialization error; and
- a stale or partially applicable conversion remains an atomic rewrite
  failure.

Do not expose a new source diagnostic for these invalid compiler states. Error
ordering remains callable and storage identity order.

## Testing strategy

### Exhaustive model and contract tests

- Add the new variant to every exhaustive storage-kind classification test.
- Pin proof disposition and proof-rich/normalized legality independently.
- Reject a normalized-only activation injected into proof-rich MIR.
- Reject a proof-rich `PathCondition` declaration in normalized MIR.
- Reject wrong type, source binding, foreign identity, and malformed storage
  references for the new kind.
- Ensure future storage variants require an explicit phase decision.

### Normalization tests

- Prove exact `PathCondition -> NormalizedPathActivation` conversion for
  functions, methods, initializers, destructors, and static initializers.
- Preserve storage/value/block identities, declaration order, spans, stores,
  loads, and lifetime operations exactly.
- Cover nested and parented logical expressions, multiple activations,
  constant-left selection plans, empty proof inventories, stale plans, and
  atomic rollback.
- Pin stable statistics and distinct proof/final dump labels.

### Verifier tests

- Keep all valid normalized short-circuit, conditional cleanup, optional,
  ownership, and lifetime fixtures accepted.
- Demonstrate that an uninitialized ordinary `ScalarSpill` is rejected under
  the normalized contract.
- Demonstrate that the corresponding marked normalized activation remains
  accepted only through consumed-proof ancestry.
- Recheck normalized resealing after every current final-stage CFG and
  definition-retention transformation.

### Backend and source-to-native tests

- Prove identical frame size, offsets, assembly, and native behavior for
  representative activation carriers before and after the implementation.
- Cover default, `none`, logical folding disabled, CFG passes disabled, all
  passes disabled, methods, lifecycle bodies, static initialization/shutdown,
  selected and skipped failures, ownership cleanup, and runtime traces.
- Retain deterministic final-MIR dumps across processes.

### Repository gates

Implementation should run focused compiler tests while changing each owner,
then `make check`. The closing task should additionally run
`make golden-determinism-test`, `make golden-release-test`, `make msrv-check`,
and `make robustness-long` from an artifact-free snapshot because a public MIR
enum, both verifier contracts, and backend input classification change.

## Migration direction

A later implementation roadmap should proceed in this order:

1. freeze the phase-legality and storage-role contract with exhaustive tests;
2. add the MIR variant and migrate generic model/rewrite/dump/backend matches;
3. make the normalizer produce it atomically and update normalization evidence;
4. narrow scalar definite-initialization and add normalized structural checks;
5. revalidate every current final-stage transformation and source/native
   profile matrix; and
6. promote living documentation, update the candidate catalog, and archive the
   resolved discovery.

The dead-carrier optimization should receive its own design or roadmap only
after this foundation is implemented and measured. It must not be folded into
the representation migration merely because it becomes easier to express.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| The new kind is treated as retained proof | Classify it as permanent executable storage with separate normalized-only phase legality |
| Raw proof-rich MIR forges consumed ancestry | Reject the kind under the proof-rich verifier and keep final sealing authority private |
| Normalized verification claims to recreate path proof | Continue the explicit consumed-proof exception only for the marked kind |
| Ordinary spills remain accidentally unchecked | Remove the broad stage switch and regression-test malformed normalized spills |
| A final-stage pass silently edits activation accesses | Keep current CFG capabilities narrow and require an explicit disposition in any future storage capability |
| The normalizer partially changes classifications | Inventory first and commit conversion, read rewriting, and proof deletion atomically |
| Backend output changes | Give the new kind identical scalar frame/lowering treatment and compare assembly/native output |
| A generalized provenance abstraction grows without consumers | Add one dedicated variant now; generalize only after another concrete owner appears |
| Dumps become confusing | Use distinct stable proof-rich and normalized labels and document the phase transition |
| Dead-carrier deletion leaks into mandatory normalization | Keep FMM-13 independently selectable and out of this proposal |

## Alternatives considered

### Keep the current `ScalarSpill` reclassification

This remains sound while the one-way seal and narrow final-stage capabilities
hold, but it keeps the normalized verifier's broad spill exception and leaves
future storage passes without an explicit semantic distinction. It preserves
the discovery rather than resolving it.

### Add `origin: Option<MirStorageOrigin>` to every `MirStorage`

An orthogonal field permits invalid combinations with source locals,
parameters, array storage, owners, aliases, and every other kind. Verifying the
cross-product would add more complexity than the one demonstrated distinction
requires.

### Change `ScalarSpill` into `ScalarSpill(MirScalarSpillOrigin)`

This is more disciplined than a free optional field, but it still asks every
existing spill producer and consumer to choose from a generalized provenance
taxonomy before there is a second stable persistent origin requirement. A
dedicated variant is smaller and consistent with existing protocol kinds.

### Retain `PathCondition` after normalization

Its name and current proof disposition imply that path proof is still present.
Keeping it would weaken the normalized zero-proof invariant and make final
passes wonder whether path records remain authoritative.

### Retain a path-condition identity on the normalized storage

The path record has been deleted, and its block/value relationships may be
rewritten immediately afterward. Retaining its identity would recreate stale
pre-normalization coupling and force every CFG pass to remap consumed proof.

### Infer origin from generated names or store/load shape

Names are diagnostic text, and topology changes under CFG simplification.
Neither is a semantic contract. Such inference would be brittle,
non-exhaustive, and hostile to future lowering changes.

### Delete path activation storage during normalization

Some activations remain executable. Proving a carrier dead is an optimization
requiring use, lifetime, attachment, and CFG analysis. Making it mandatory
would conflate proof consumption with optimization and make `none` cease to be
a representation-only reference.

### Run ordinary definite initialization for normalized activations

The erased path relation is exactly what allowed the proof-rich verifier to
establish valid correlated accesses. A less precise normalized dataflow has
already rejected valid programs. Pretending it is authoritative would either
change accepted behavior or require reconstructing the proof this boundary was
designed to consume.

## Effort and expected value

Overall effort is **medium**. The semantic change is narrow, but
`MirStorageKind` is intentionally exhaustive across model, verification,
rewriting, dumping, tests, and backend classification.

| Work | Relative effort | Durable value |
|---|---|---|
| Phase-legality classification | Small to medium | Prevents proof/final storage forms from crossing the wrong seal |
| MIR-kind migration | Medium | Stable explicit carrier role with compile-time maintenance points |
| Atomic normalizer conversion | Small | Preserves the existing transaction while retaining useful classification |
| Definite-initialization hardening | Medium | Removes the broad normalized `ScalarSpill` exemption |
| Dump/backend migration | Small to medium | Explicit observability with unchanged target behavior |
| Cross-profile and source/native tests | Medium | Proves the representation refinement is behavior-neutral |

The direct runtime value is zero because this proposal performs no
optimization. Its immediate correctness and maintainability value is medium;
its enabling value becomes high before any final-stage storage, spill,
promotion, or dead-store transformation.

## Review questions

The proposed decisions intentionally answer the representation questions, but
review should explicitly confirm:

1. Is `NormalizedPathActivation` the accepted stable name and dump spelling?
2. Should proof-rich verification reject the final-only kind even in synthetic
   compiler fixtures? This proposal says yes.
3. Should ordinary `ScalarSpill` definite initialization run under both
   contracts once the distinction exists? This proposal says yes.
4. Should normalized activations retain only a unit classification rather than
   path identity or generalized origin metadata? This proposal says yes.
5. Should dead normalized carrier cleanup remain a separate selectable pass?
   This proposal says yes.

## Freeze and promotion

NSP1 through NSP14 were accepted together on 2026-09-05. This record is frozen
in `docs/archive/`; the final-only storage role and verifier direction are
promoted into the living compiler phase, backend, and testing contracts. The
[completed roadmap](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_ROADMAP.md)
records delivery, its resolved companion
[discoveries record](NORMALIZATION_STABLE_PATH_ACTIVATION_PROVENANCE_DISCOVERIES.md)
records that no out-of-scope follow-up was found, and the optimization catalog
continues to keep FMM-13 dead-carrier deletion separate from this prerequisite.

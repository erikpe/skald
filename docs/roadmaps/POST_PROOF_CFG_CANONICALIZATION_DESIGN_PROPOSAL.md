# Post-Proof CFG Canonicalization Design Proposal

Status: draft design proposal. PCG1 through PCG14 are proposed together and
have not been confirmed or promoted into an implementation roadmap.

This proposal defines the next conservative final-MIR control-flow layer for
Skald. It covers the optimization catalog's FMC-08 empty-block forwarding and
FMC-09 basic-block merging candidates in the
[final-MIR CFG graph](OPTIMIZATION_CANDIDATE_CATALOG.md#final-mir-checked-operation-and-control-flow-graph).
Both transformations operate only on the normalized
`VerifiedFinalMirProgram`, after proof provenance has been consumed and before
target-independent whole-world definition retention.

The purpose is broader than removing individual jumps. The work should turn
the post-proof CFG canary into a reusable, still narrow canonicalization
boundary that later CFG analyses can build on without exposing raw mutable MIR
or weakening lifecycle, failure, ownership, trace, and determinism contracts.

The language contract does not change. Compilation remains permanently
whole-world and resulting programs remain single threaded. Evaluation order,
checked failure, aliases, sequential mutation, allocation, ownership,
destruction, static activation and shutdown, diagnostics, panic locations,
runtime traces, and ABI behavior remain observable exactly as before.

## Intended outcome

The design should provide:

- one deterministic normalized CFG snapshot with explicit predecessor edges,
  successor edges, entry identity, and permanent semantic roots;
- a separately selectable `post-proof-empty-block-forwarding` pass;
- a separately selectable `post-proof-basic-block-merging` pass;
- exact structural eligibility rules that do not rely on failed output
  verification to discover unsafe candidates;
- transitive forwarding of instruction-free jump chains without looping on
  empty cycles;
- deterministic merging of maximal single-entry linear block chains;
- preservation of every surviving instruction, terminator, value, storage,
  span, and executable effect in its original execution order;
- explicit barriers around static-publication endpoints and any future
  permanent block attachment;
- atomic sparse editing and deterministic dense commit through the existing
  callable-local rewrite owner;
- normalized verification and fresh seal-bound reachability after every
  changed pass occurrence;
- independently meaningful selection, disabling, metrics, checkpoints, and
  dumps for the two transformations; and
- native and runtime-trace equivalence under `none`, default, and selective
  schedules.

The proposal does not make final MIR generally mutable. It adds two reviewed
operations to the existing final-stage CFG capability and leaves every other
instruction, terminator, storage, protocol, and lifecycle rewrite inaccessible
to these passes.

## Current architecture and evidence

### Normalized final MIR is now the correct boundary

The mandatory proof-provenance normalizer consumes path-condition and logical-
expression records after complete proof-rich verification. The resulting
`VerifiedFinalMirProgram` contains executable MIR, permanent static-publication
attachments, and reachability facts bound to that exact normalized product.

Final-stage passes receive a concrete capability distinct from the proof-rich
capability. A change consumes the seal, commits one complete program rewrite,
runs normalized verification, recomputes target-independent reachability, and
reseals the product before another pass or the backend can observe it. The
`none` profile still crosses the mandatory normalization boundary while
selecting no optimization pass.

This removes the former reason that forwarding or merging would have had to
rewrite source-proof topology. Continuing checked, optional, array, ownership,
storage-lifetime, and lifecycle protocols remain executable semantics rather
than disposable proof.

### The current final capability proves deletion but not canonicalization

The implemented `MirFinalCfgEdit` exposes only:

- a normalized callable-local CFG snapshot; and
- exact removal of blocks and transient values classified unreachable in that
  still-current snapshot.

The `post-proof-unreachable-block-elimination` canary therefore demonstrates
safe normalized CFG deletion without exposing instructions, terminators,
storage, proof records, or lifecycle authority. Empty-block forwarding needs
controlled edge redirection, while block merging additionally needs ordered
instruction transfer and terminator replacement. Neither operation is
currently available through the final-stage capability.

The general sparse `MirCallableEdit` already has the underlying structural
primitives: inspect blocks, redirect executable edges, rewrite instruction
lists and terminators, remove blocks, and commit with exhaustive identity
mapping. The new passes should not receive that general editor directly.
Instead, a final-CFG owner should validate reviewed candidates and perform only
the corresponding compound edits.

### Existing CFG facts need predecessor identity and multiplicity

`MirLocalCfgFacts` currently records block order, successors, block-owned value
definitions, entry reachability, permanent roots, and unreachable sets. That
is sufficient for deletion but not for proving that a merge successor has one
incoming executable edge.

The shared CFG query should add deterministic predecessor-edge facts. Edge
multiplicity matters: an ordinary branch with both arms targeting one block is
two successor occurrences, not permission to treat the branch as an
unconditional predecessor. Edge records should retain source block and stable
successor position or an equivalent exhaustive role. They are short-lived
analysis facts, not persistent MIR identities.

The query must continue deriving successors from the same exhaustive
terminator vocabulary used by verification and identity rewriting. A new
terminator variant must fail maintenance tests until its successor roles are
classified.

### Static publication makes some block identities semantic

Callable body entry is an execution root and may be retained while its
terminator or following linear region is simplified, but it must never be
deleted. Static initializers additionally name an initialization-exit block
and cleanup-entry block. Those blocks describe the boundary between startup
publication and shutdown cleanup, not merely reachability hints.

The initial canonicalization passes therefore treat every permanent attachment
as a hard mutation barrier:

- an attached block cannot be forwarded away or removed as a merge successor;
- an attached block's terminator cannot be redirected by forwarding; and
- an attached block cannot be the retained predecessor in a merge.

This is intentionally stricter than proving that one particular edge rewrite
would happen to preserve today's lifecycle verifier. A later lifecycle-aware
design may relax the barrier by updating or re-proving attachment semantics.

### Block-local values make linear merging tractable

MIR `ValueId` definitions and uses are block-local. A verified successor's
instructions and terminator therefore use only values defined earlier in that
same successor. When a single-predecessor successor is appended after all
instructions of its goto predecessor, those definitions and uses remain in
order and become block-local to the retained predecessor together.

No value declaration needs to be created, deleted, or substituted. Storage
operations remain in exactly the same sequence. This makes simple linear
merging possible without SSA, phi nodes, block parameters, alias analysis, or
storage propagation.

### Block and operation spans have different obligations

Every moved instruction and the successor terminator retain their exact source
spans. The retained predecessor keeps its basic-block span. The removed
successor's block span and the eliminated goto terminator's span disappear
from optimized MIR dumps because their structural entities disappear.

Current runtime tracing and failure attribution observe executable operation,
call, cleanup, termination, and failure spans rather than entry into an empty
basic block or execution of an unconditional goto. The passes may rely on that
current contract, but maintenance tests must make any future observable block-
entry or goto event force a design review.

### Whole-world and single-threaded assumptions

Whole-world compilation means all callable entries and static-publication
attachments are available when final CFG facts are built; no later module can
introduce another entry edge into a removed block. Single-threaded execution
means no concurrent control transfer or mutation can interleave with the
preserved instruction sequence.

Those guarantees do not justify moving an instruction across another
instruction, suppressing failure, changing destructor timing, or assuming
aliases are distinct. This proposal only removes empty transfers or fuses a
known adjacent linear sequence.

## Comparison with Niflheim

Niflheim's backend CFG simplifier forwards non-entry instruction-free jump
blocks, folds redundant branches, and removes unreachable blocks to a fixed
point. It resolves transitive forwarding targets, retains empty jump cycles,
preserves blocks carrying merge-copy instructions, and reruns backend IR
verification after mutation.

The useful precedents are deterministic transitive resolution, explicit cycle
handling, verified mutation, and a strict definition of an empty block.
Skald should not copy Niflheim's pass boundary directly:

- Skald performs this work in target-independent normalized final MIR rather
  than a target-private backend IR;
- Skald recompacts dense callable-local block identities after deletion;
- Skald has permanent static-publication block attachments;
- Skald retains continuing checked, optional, array, ownership, and cleanup
  protocols in this MIR; and
- Skald's selectable-pass architecture benefits from measuring forwarding and
  merging independently.

Niflheim currently does not provide the proposed instruction-bearing linear
block merge. Its implementation is evidence for conservative forwarding, not
authority for Skald's merge legality.

## Shared normalized CFG facts

The existing callable-local CFG analysis remains the single structural owner.
It should be extended rather than shadowed inside either pass.

For each current block, the immutable snapshot should expose:

- the block identity and current dense order;
- ordered successor edge occurrences;
- ordered predecessor edge occurrences;
- whether the block is the body entry;
- every permanent attachment role naming the block;
- instruction count;
- a closed terminator-shape classification sufficient to identify an
  unconditional goto and its target;
- transient values defined by the block; and
- existing entry, permanent-root, reachable, and unreachable closures.

Predecessor ordering is source block order followed by successor occurrence
order. Duplicate edges remain duplicate facts. The snapshot contains no raw
mutable references and is invalid after the first structural edit.

Proof-rich CFG consumers may reuse predecessor and edge facts, but the
canonicalization candidate queries accept only normalized facts. Consumed
proof roots remain an error in the normalized query.

## Empty-block forwarding

### Exact candidate

A block is forwardable only when all of the following hold in one current
normalized CFG snapshot:

1. it is not the callable body entry;
2. it has no permanent attachment;
3. its instruction list is empty;
4. its terminator is exactly `MirTerminator::Goto` to a distinct block;
5. the forwarding chain reaches a live non-forwardable block rather than a
   self-loop or cycle; and
6. no incoming edge originates in a permanent-attachment block whose
   terminator is a mutation barrier.

“Empty” means zero MIR instructions. A block containing a storage lifetime
operation, cleanup, retain/release, trace-affecting operation, assignment, or
any other instruction is not empty even if a later analysis might consider
that instruction redundant.

The target may be the body entry or a permanent root because neither is
deleted or mutated by the operation. Every incoming executable successor
occurrence is redirected to the resolved target while retaining its original
terminator kind, operands, checked/failure role, and span.

### Transitive chains and cycles

The pass resolves every eligible chain to its first non-forwardable target in
one occurrence. Candidate discovery and target resolution use current block
order and deterministic maps. A chain entering an empty self-loop or a cycle
has no valid exit target and is retained unchanged. The first implementation
does not choose an arbitrary representative for a cycle.

All candidate mappings are computed before mutation. The final capability
then rechecks the complete snapshot, redirects incoming edges to resolved
targets, and removes the exact candidate blocks. Empty blocks define no
transient values, so forwarding removes no `ValueId` declarations.

Forwarding may cause two arms of a branch to name the same block. This pass
does not rewrite the branch to a goto: ordinary or protocol-specific branch
simplification is a separate semantic transformation. The resulting MIR must
still satisfy normalized verification.

### Independent behavior

The pass must be idempotent and useful when block merging is disabled. One
occurrence removes all transitively resolvable empty jump blocks. Disabling
the post-proof unreachable-block canary must not make forwarding invalid;
incoming edges from unreachable blocks are either redirected too or act as an
explicit eligibility barrier, never left referring to a removed block.

## Basic-block merging

### Exact candidate

An ordered pair `(predecessor, successor)` is mergeable only when all of the
following hold in one current normalized CFG snapshot:

1. `predecessor` terminates with exactly one unconditional `Goto` edge to
   `successor`;
2. that goto is the successor's only incoming executable edge occurrence in
   the complete live callable, including unreachable regions;
3. the blocks are distinct;
4. `successor` is not the body entry;
5. neither block has a permanent attachment;
6. the successor does not refer to itself from its terminator, a condition
   already excluded by the single-incoming-edge rule when edge multiplicity is
   counted correctly; and
7. moving the successor's instruction list and terminator requires no value,
   storage, proof, lifecycle, or protocol rewrite beyond block compaction.

The predecessor may be the body entry because its identity is retained. A
branch predecessor is not mergeable even if both branch arms name the same
successor. A block reached by one predecessor block through multiple edge
roles is not mergeable.

### Compound rewrite

For an eligible pair, the final capability performs one compound edit:

1. append the successor's instructions after the predecessor's instructions;
2. replace the predecessor's goto with the successor's exact terminator;
3. retain every moved instruction and terminator span unchanged;
4. retain all `ValueId` and `StorageId` declarations and references;
5. remove the now-unreferenced successor block; and
6. let atomic dense commit remap every surviving block reference.

The predecessor's existing instructions execute first, followed by the
successor's instructions and terminator, exactly matching the original path.
The removed goto has no executable effect under the current language and trace
contract.

### Maximal linear chains

One occurrence repeatedly selects the first eligible pair in current block
order, applies the merge, rebuilds the short-lived CFG snapshot, and continues
until no eligible pair remains. Every successful step deletes one block, so
the process terminates without a general optimizer fixed-point manager.

Recomputation is deliberate. Merging changes predecessor relationships and
instruction ownership, making a precomputed overlapping merge plan difficult
to validate and unnecessary at current MIR sizes. The whole callable still
commits atomically; an error at any step publishes no partial program.

The pass must also be idempotent and useful when empty-block forwarding is
disabled.

## Final-stage mutation capability

The pipeline should retain `MirFinalCfgEdit` as the sole mutation surface for
these passes. It should gain reviewed compound operations equivalent to:

- apply a complete empty-forwarding plan authorized by exact normalized CFG
  facts; and
- merge one exact linear predecessor/successor pair authorized by a current
  normalized CFG snapshot.

The exact Rust API may differ, but these properties are mandatory:

- the capability never exposes `&mut MirProgram`, `&mut MirCallableEdit`, raw
  sparse slots, or unrestricted instruction/terminator mutation;
- candidate selection is visible and unit-testable rather than buried in a
  generic editor;
- the capability independently checks entry and permanent-root barriers;
- stale snapshots produce structured rewrite failures;
- a removed block cannot retain an ordinary, protocol, entry, or attachment
  reference;
- every change uses the existing atomic program rewrite coordinator;
- changed outcomes invalidate the final seal and all local-ID, instruction-
  position, CFG, and reachability facts; and
- the runner performs normalized verification and fresh reachability before
  the next pass.

Underlying generic edit operations remain available to their existing
compiler-private owners. Registration of these passes is not permission for a
future final-stage pass to call those operations directly.

## Pass registration, selection, and default schedule

Register two production passes with stable final-stage descriptors:

| Stable name | Stage | Responsibility |
|---|---|---|
| `post-proof-empty-block-forwarding` | `Final` | Redirect executable edges through transitive instruction-free goto blocks and remove those blocks |
| `post-proof-basic-block-merging` | `Final` | Fuse maximal eligible single-incoming goto chains while preserving operation order |

They are independently listed and independently disabled. There is no umbrella
`cfg-canonicalization` selection name, hidden implication, or requirement that
one pass be selected with the other.

The proposed default final-stage suffix is:

```text
post-proof-unreachable-block-elimination
post-proof-empty-block-forwarding
post-proof-basic-block-merging
whole-world-reachability
```

The existing unreachable pass runs first so normal compilation does not spend
canonicalization work on disposable regions. Empty forwarding then exposes
direct linear edges, and merging reduces maximal chains. Because forwarding
removes every eligible non-root empty goto block and merging a non-empty
successor cannot create one, the reviewed combination needs no repeated
occurrence. Cycle barriers and permanent roots remain deliberately
non-canonical rather than triggering schedule iteration.

Internal exact schedules and per-pass disabling must nevertheless prove every
combination. If implementation evidence demonstrates a real alternating case
under the frozen eligibility rules, the design must be amended rather than
silently adding an implementation-local cross-pass loop.

Whole-world reachability remains last so removed blocks and their call sites
can reduce the retained executable-definition graph. Neither new pass removes
definitions itself.

## Identity, ordering, and analysis lifetime

Surviving storage and value declarations retain their semantic entities.
Empty forwarding removes no values. Block merging moves definitions but
removes no value declaration. Removed blocks create sparse gaps only inside the
private transaction; commit deterministically recompacts block identities and
rewrites all surviving references through the existing exhaustive mapper.

Instruction positions and CFG edges remain observations, not identities.
Every structural change invalidates:

- the normalized CFG snapshot;
- predecessor and successor edge facts;
- value-definition site observations;
- instruction-position facts;
- target-independent reachability; and
- any pass-local candidate plan.

The empty-forwarding pass applies one complete prevalidated resolved mapping.
The merging pass deliberately recomputes facts after each deletion.
No facts survive the callable transaction or final reseal.

Traversal and tie-breaking use program definition order, callable block order,
and successor occurrence order. Hash-map iteration, filesystem order, pointer
identity, or compiler worker completion must not influence a candidate,
representative, emitted order, measurement, or dump.

## Language, failure, lifecycle, and trace contract

Both transformations are semantic equivalences under the current language:

- no instruction or effectful terminator is deleted;
- instruction evaluation order is unchanged;
- no operation crosses another operation;
- checked success and failure edges retain their roles and spans;
- panic and hard-termination behavior is unchanged;
- storage-live/dead, initialization, cleanup, ownership, retain/release, and
  destruction operations remain in the same dynamic order;
- permissive aliases and mutable shared pointees observe the same writes and
  reads;
- static initialization, publication, reverse shutdown, and active-field
  selection remain unchanged;
- function, method, indirect, virtual, and interface call behavior is
  unchanged;
- runtime-trace events retain the same executable spans and ordering;
- external ABI and target legality are unchanged; and
- compilation diagnostics and source acceptance remain optimization
  independent.

The proposal permits optimized MIR dumps and assembly labels/jumps to differ.
It does not permit native output, exit status, panic text, trace rows, or source
locations to differ.

## Observation and measurements

Use the existing pass reporting and checkpoint model. Each occurrence reports
processed and changed callables plus deterministic pass-owned counts.

Empty-block forwarding should report at least:

- removed forwarding blocks;
- redirected successor occurrences;
- retained cyclic/self-loop candidates; and
- retained candidates blocked by permanent attachments.

Basic-block merging should report at least:

- merged block pairs;
- moved instructions;
- removed blocks; and
- retained candidates blocked by multiple incoming edges or permanent
  attachments.

The structural rewrite report remains authoritative for retained, inserted,
and removed MIR entities. Pass metrics explain why a transformation occurred
or was refused; they must not duplicate every generic commit count under a
second meaning.

Pass listing exposes each stable name, final stage, and concise description.
Trace-level reporting and inspection checkpoints preserve exact schedule
position and occurrence number. Dumps after each changed pass must be
deterministic across processes.

## Scope and non-goals

This proposal includes only empty-block forwarding and basic-block merging on
normalized final MIR. It explicitly excludes:

- proof-rich CFG forwarding or merging;
- branch-to-goto folding created by target convergence;
- jump threading through conditional blocks;
- duplication or tail duplication of instructions;
- critical-edge splitting;
- short-circuit logical CFG reconstruction;
- checked-operation or optional-protocol normalization;
- loop discovery, loop canonicalization, peeling, or unrolling;
- storage deletion, load/store forwarding, dead-store elimination, or scalar
  promotion;
- scalar-spill provenance changes;
- value substitution, common-subexpression elimination, or SSA;
- callable inlining, cloning, specialization, or devirtualization;
- global analysis preservation declarations or a fixed-point pass manager;
- target-specific block layout, branch inversion, or peepholes;
- changes to static-publication attachments;
- language-contract changes; and
- repository CI.

The existing open scalar-spill provenance discovery is therefore not a
dependency: neither pass creates, deletes, combines, or reasons through
storage. It becomes a prerequisite only for a later storage-aware final-stage
transformation.

## Verification and test strategy

### CFG fact and maintenance tests

- Verify deterministic predecessor and successor occurrence order for goto,
  ordinary branch, every checked/protocol terminator, loops, duplicate branch
  targets, returns, panic, and terminate.
- Verify entry and every permanent attachment role are classified exactly.
- Verify proof-rich facts may retain consumed proof roots while normalized
  facts reject them.
- Add exhaustive maintenance coverage so a new terminator or permanent block
  attachment requires an explicit edge/root decision.
- Verify malformed foreign, deleted, duplicate, or missing block references
  return structured errors.

### Empty-forwarding tests

- Forward one empty block and a transitive chain.
- Redirect multiple incoming successor occurrences without changing their
  terminator kinds, operands, roles, or spans.
- Preserve body entry, self-loops, empty cycles, instruction-bearing blocks,
  permanent roots, and candidates reached from a permanent-root terminator.
- Cover candidates and barriers in functions, methods, static initializers,
  ordinary branches, checked failure/success edges, optional/array protocols,
  loops, cleanup regions, and entry-unreachable regions.
- Prove exact candidate-plan stale-snapshot rejection, atomic failure, dense
  compaction, deterministic dumps, and idempotence.

### Block-merging tests

- Merge a single linear pair and a maximal chain.
- Retain the body-entry block while merging its eligible successor.
- Preserve predecessor instruction order, successor instruction order,
  successor terminator kind/span, all value/storage identities, and every
  operation span.
- Reject a branch predecessor, duplicate incoming edges, multiple predecessor
  blocks, self-referential successors with another incoming edge, entry
  successor, either permanent-root endpoint, and stale facts. Cover valid
  two-block cycles which contract to one self-loop without changing operation
  order.
- Cover storage lifetime, cleanup, ownership, checked, optional, array, loop,
  return, panic, terminate, and static-initializer shapes.
- Prove atomic rollback, dense block compaction, deterministic maximal-chain
  selection, and idempotence.

### Pipeline and selection tests

- Validate both registry identities, stable names, descriptions, and `Final`
  stage.
- Freeze the exact default schedule and pass-list output.
- Cover `none`, each pass alone, each pass disabled from default, both passes
  disabled, the post-proof unreachable pass disabled, and repeated internal
  occurrences.
- Confirm an unchanged occurrence preserves the seal without reverification.
- Confirm a changed occurrence runs one normalized verification and recomputes
  reachability before the next pass.
- Confirm exact stage-bearing checkpoints, failure attribution, processed and
  changed callable counts, and pass-owned measurements.

### Semantic and repository gates

- Add source-level golden fixtures whose post-normalization MIR contains
  forwarding chains, single-entry linear chains, multiple-predecessor joins,
  loops, cleanup, checked failures, and static publication boundaries.
- Compare `none`, default, and selective variants for native output, exit
  status, panic text, and runtime traces.
- Assert smaller normalized MIR or assembly where a fixture is deliberately
  productive, without making target-specific instruction shape the semantic
  test oracle.
- Run focused rewrite, verifier, pipeline, driver, reporting, backend, and
  runtime-trace suites.
- Run `make check`, independent-process golden determinism, release-mode golden
  tests, documentation link/index validation, formatting, linting, and the
  supported Rust MSRV from an artifact-free snapshot before roadmap closure.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A deleted block remains referenced by an unusual terminator role | Derive and redirect exhaustive edge occurrences through the shared identity traversal, then normalized-verify the atomic result |
| A static lifecycle endpoint is structurally changed | Treat every permanent attachment and its terminator as an initial hard barrier |
| Duplicate branch edges are mistaken for one predecessor | Preserve edge multiplicity rather than only predecessor-block sets |
| Empty cycles cause nontermination or arbitrary representative choice | Resolve only chains ending at a non-forwardable live block; retain cycles unchanged |
| Merging changes instruction or failure order | Require a goto predecessor and single incoming edge, append exact instruction sequences, and retain the successor terminator unchanged |
| Moved block-local values become invalid | Move definitions and all same-block uses together; create, delete, and substitute no value |
| One pass depends secretly on the other | Require independent idempotence and exercise every selective schedule |
| Analysis facts are reused after mutation | Use exact snapshots, reject stale plans, and recompute after every merge |
| Dense recompaction makes output nondeterministic | Use existing sparse transactions, explicit block order, exhaustive mapping, and cross-process dump tests |
| Output verification becomes the eligibility algorithm | Encode every candidate and barrier before mutation; verification remains the final trust boundary |
| The pass grows into general CFG optimization | Keep branch folding, threading, duplication, protocols, loops, and storage changes as separately reviewed candidates |

## Alternatives considered

### One monolithic post-proof CFG simplifier

Rejected for the initial delivery. A single pass would make forwarding and
merging impossible to select and measure independently, and would encourage
unreviewed branch or protocol rewrites to accumulate behind one broad name.
Shared CFG facts and capability operations still prevent code duplication.

### Extend the unreachable-block canary

Rejected. Unreachable deletion consumes a reachability classification;
forwarding rewrites incoming edges, and merging moves executable instructions.
Keeping the canary narrow preserves its role as the smallest proof-normalized
deletion test and gives new behavior its own selection and metrics.

### Run the transformations before proof normalization

Rejected. Proof-rich path and logical records name exact blocks and topology.
Teaching these passes to rewrite consumed proof would recreate the coupling the
mandatory normalization boundary was introduced to remove.

### Expose the general callable editor to final passes

Rejected. The existing editor can mutate storage, values, instructions,
terminators, proof records, guards, and block order. A final pass should receive
only the compound operation its reviewed semantic proof authorizes.

### Forward or merge permanent attachment blocks and rewrite attachments

Deferred. Static-publication endpoints describe startup/shutdown semantics and
currently justify independent roots. Rewriting them needs a lifecycle-aware
proof and should not be smuggled into generic CFG cleanup.

### Treat one predecessor block as one incoming edge

Rejected. A branch can have two successor occurrences naming the same block.
Merging through that branch would also perform an unreviewed branch fold and
could suppress a semantic control operation.

### Preserve removed dense block ordinals

Rejected. Dense callable-local MIR identities intentionally recompact after
atomic rewriting. Sparse stable holes are private transaction state, not a new
published final-MIR contract.

### Add general jump threading now

Deferred. Threading conditional blocks needs path predicates, duplication and
code-size policy, cleanup and ownership joins, and more extensive protocol
reasoning. Empty forwarding and linear merging establish useful shared facts
without taking on those obligations.

### Implement only in the target backend

Rejected. These transformations are target independent and should reduce the
input to whole-world reachability, target legality, frame planning, and every
backend. Machine CFG layout and target branch decisions remain backend work.

## Effort and recommended delivery shape

Estimated effort is **medium to large**. The transformations are individually
small, but their reusable predecessor facts, narrow final capability,
permanent-root barriers, selection surface, observation, and semantic matrix
need staged review.

A later roadmap should likely separate:

1. deterministic predecessor-edge facts and exhaustive maintenance tests;
2. final-CFG candidate vocabulary and guarded compound edit operations;
3. independently selectable empty-block forwarding;
4. independently selectable basic-block merging;
5. default scheduling, listing, reporting, inspection, and selective profiles;
6. golden/native/runtime-trace equivalence and measured productive fixtures;
   and
7. ownership audit, documentation promotion, full clean-snapshot gates, and
   archival.

Do not combine this roadmap with callable effect summaries. The CFG work needs
no alias or effect lattice and should provide a cleaner final MIR on which a
later whole-program analysis can operate.

## Proposed decisions

### PCG1 — Operate only on normalized final MIR

Both passes are `Final` stage consumers of `VerifiedFinalMirProgram`. They do
not run against proof-rich MIR or participate in mandatory normalization.

### PCG2 — Register two independently selectable passes

Use `post-proof-empty-block-forwarding` and
`post-proof-basic-block-merging`, with separate identities, descriptors,
implementations, metrics, and exclusions.

### PCG3 — Extend the shared CFG snapshot with exact predecessor edges

Record deterministic predecessor and successor occurrences with multiplicity
and exhaustive terminator-role maintenance coverage. Do not introduce
persistent edge identities or a global analysis manager.

### PCG4 — Treat permanent attachments as hard mutation barriers

Never delete an attached block, redirect its terminator, or merge through it.
Body entry may be retained and rewritten but never removed.

### PCG5 — Define empty forwarding structurally

Forward only non-entry, unattached, zero-instruction blocks ending in a goto to
a distinct block. Preserve every incoming terminator's kind, operands, role,
and span.

### PCG6 — Resolve transitive forwarding without choosing cycle representatives

Map complete forwarding chains to the first non-forwardable target in one pass
occurrence. Retain self-loops and empty cycles unchanged.

### PCG7 — Merge only exact single-incoming goto pairs

Require a goto predecessor, one total incoming edge occurrence, a non-entry
and unattached successor, and no permanent attachment on either endpoint.

### PCG8 — Preserve operations, values, storage, order, and executable spans

Append the successor's instructions, transfer its exact terminator, delete
only the structural goto and successor block, and create, delete, substitute,
or reorder no executable value or storage operation.

### PCG9 — Keep final mutation capability narrow and atomic

Expose reviewed forwarding and merging operations through `MirFinalCfgEdit`,
validate exact snapshots, commit through dense rewriting, and publish no
partial result on failure.

### PCG10 — Make each pass independently convergent

Forward all resolvable empty chains in one forwarding occurrence. Merge
eligible linear chains by deterministic selection and fresh snapshots until
no pair remains. Require idempotence without a pipeline-wide fixed-point
manager.

### PCG11 — Run unreachable deletion, forwarding, merging, then reachability

Add the two passes to the default final suffix in that order, after the current
post-proof unreachable canary and before whole-world definition retention.

### PCG12 — Separate semantic metrics from generic rewrite accounting

Report forwarding, redirects, cycles, merges, moved instructions, and explicit
barriers through pass-owned metrics while retaining the structural commit
summary as the entity-change authority.

### PCG13 — Preserve the current language and runtime contract

Whole-world and single-threaded execution justify complete root knowledge, not
weaker evaluation, failure, alias, ownership, lifecycle, diagnostic, trace, or
ABI guarantees.

### PCG14 — Defer broader CFG and storage transformations

Do not add branch folding, jump threading, instruction duplication,
short-circuit or checked-protocol rewrites, loops, storage deletion,
scalar-spill propagation, SSA, or target layout under this design.

## Confirmation and promotion

Freezing this proposal requires PCG1 through PCG14 to be accepted together.
After confirmation:

- change the status to a frozen decision record with the confirmation date;
- create a PR-sized implementation roadmap and a separate discoveries file;
- update the roadmap index and candidate catalog from **Draft design** to
  **Proposed**;
- promote implemented behavior into living compiler, driver, reporting,
  backend, and testing documentation as each roadmap task lands; and
- archive the design and roadmap only after all tasks and artifact-free quality
  gates are complete.

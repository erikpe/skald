# Dense Callable-Local MIR Identity Rewriting Design Proposal

Status: frozen design proposal. DMI1 through DMI12 were confirmed together on
2026-08-30 and promoted into the
[compiler phase and IR contract](../compiler/PHASES_AND_IR.md#frozen-dense-callable-local-mir-identity-rewriting-direction).
The planned
[implementation roadmap](DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md) owns
delivery; this document preserves the reviewed decisions.

This proposal removes the editing constraint created by Skald's dense,
index-coupled callable-local MIR identities. It keeps dense final MIR as the
compact verified and backend-facing representation, while introducing a
private rewrite transaction in which passes can insert, replace, redirect, and
delete callable-local entities without renumbering the rest of the callable
after every operation.

The primary purpose is to enable future target-independent optimization. The
same change should also establish one exhaustive identity traversal, remove
ad-hoc table surgery from future passes, cover static initializer bodies and
their publication metadata uniformly, and give malformed transformations one
deterministic internal-error boundary.

The language contract does not change. Evaluation order, checked failures,
panic behavior, deterministic destruction, aliasing, ownership, and mutable
access retain their current meaning. Whole-world compilation and a
single-threaded resulting program are permanent assumptions. They simplify
closed-world analysis and exclude runtime concurrency concerns, but they do
not justify incomplete reference rewriting or nondeterministic compiler
output.

## Intended outcome

The redesign should provide:

- stable callable-local identities while one pass edits a callable;
- explicit deletion without shifting unrelated identities during the edit;
- deterministic dense `StorageId`, `ValueId`, `BlockId`, and
  `PathConditionId` values when the transaction commits;
- coordinated rewriting of every executable and metadata reference to those
  identities;
- correct handling of optional-guard identities and static-initializer
  publication blocks, which are easy to miss if only `MirBody` is traversed;
- one pass-independent API for allocation, replacement, edge redirection,
  deletion, import, and compaction;
- no production `iter_mut` escape hatch for definition tables;
- no malformed intermediate callable exposed as `MirProgram` or sealed final
  MIR;
- deterministic errors when a retained node still references a removed,
  undeclared, or foreign identity;
- explicit invalidation of analysis results keyed by pre-commit local IDs;
- compatibility with future per-callable compiler parallelism without global
  identity allocators; and
- a testable foundation for dead-definition removal, CFG simplification,
  inlining, specialization, and proof-metadata cleanup.

## Current architecture and evidence

`StorageId`, `ValueId`, `BlockId`, `PathConditionId`, and `OptionalGuardId`
contain a `CallableId` plus a `usize` index. Initial lowering allocates most of
them from the current vector length. This produces compact, deterministic MIR
efficiently.

Committed MIR gives four of those indices a second meaning: they are direct
positions in callable-owned vectors. Lookup indexes `storage`, `values`,
`blocks`, or `path_conditions`, then checks that the entry contains the same
identity. Verification requires every entry to have the matching dense index.
Verifier dataflow also sizes block-indexed state from `blocks.len()` and uses
`BlockId::index()` directly.

That representation is a good read and analysis format but an awkward edit
format. Removing storage entry `s3`, for example, shifts every following
storage and requires every storage-bearing place, projection, instruction,
terminator, signature attachment, path condition, and logical-expression
record to be updated in one operation. Blocks have the same issue, with the
additional requirement to update body entry, successors, path-condition
predecessors and merges, logical provenance, and static-initializer
publication boundaries.

The complete callable-local identity surface is broader than the three IDs
originally highlighted in the discovery record:

| Identity or sequence | Declaration owner | Important reference owners | Commit treatment |
|---|---|---|---|
| `StorageId` | callable `storage` table | receiver, parameters, return owner, places and projections, instructions, terminators, path activations, logical metadata | compact and remap |
| `ValueId` | callable `values` table | rvalues, arguments, calls, instructions, terminators, logical metadata | compact and remap |
| `BlockId` | `MirBody::blocks` | body entry, all successor forms, path metadata, logical metadata, static publication | compact and remap |
| `PathConditionId` | `MirBody::path_conditions` | parent relation, path-conditioned rvalues, logical metadata | compact and remap in parent-before-child order |
| `OptionalGuardId` | paired begin/end operations, without a declaration table | optional-view terminators and end instructions | collect, validate, and canonically remap |
| logical-expression record | `MirBody::logical_expressions` | no separate record ID; references the four dense identity kinds | retain or delete explicitly, then preserve relative order |

`BindingId` values retained as `MirStorage::source` are different. They are
source-semantic provenance, not MIR table positions, and remain stable for an
existing callable. Program-level function, member, class, field, static-field,
type, module, lifecycle, and declaration identities likewise remain outside
callable compaction.

The same executable shape currently appears in ordinary function definitions,
member definitions, and final static-initializer bodies. `MirDefinitionRef`
provides a useful read-only common view, but production mutation is deliberately
absent from the function, member, and lifecycle-initializer containers. A
rewrite boundary therefore needs an owned common adapter and narrow
take-and-replace integration rather than public mutable table iteration.

The final-MIR pipeline now has a real seal:
`verify_final_mir` constructs `VerifiedFinalMirProgram`, and backend input
accepts only that verified product. A rewrite transaction must invalidate that
seal and may not recreate it. Only central ordinary and static-lifecycle
realization verification may seal rewritten MIR again.

## Comparison with Niflheim

Niflheim provides useful pass-organization precedents but makes a different
local-identity tradeoff.

Its semantic and backend optimization pipelines use named pass descriptors,
ordered immutable program-to-program transforms, shared analysis helpers,
per-pass reporting, and backend verification before and after every registered
pass. Its backend optimizer currently includes dead-pure-definition
elimination, trivial-copy elimination, constant folding, algebraic
simplification, and CFG simplification. The semantic pipeline additionally
performs copy propagation, flow-sensitive type narrowing, interface-call
devirtualization, redundant-cast elimination, dead-store and dead-statement
elimination, and unreachable pruning.

Niflheim backend registers, blocks, and instructions have owner-qualified
ordinal IDs, but callable storage is tuple based and verifier lookup builds
dictionaries. The verifier requires ownership and uniqueness, not contiguous
ordinals. Consequently, CFG simplification can filter unreachable blocks and
dead-definition elimination can filter registers without renumbering surviving
references. Deterministic renderers sort by ordinal when required.

Skald should reuse the organizational lessons—named transformations,
central verification, functional ownership transfer, shared traversal and
analysis helpers, and deterministic pass order—but should not change all
committed MIR lookup to sparse maps merely to gain editability. Skald has more
callable-local ownership and proof metadata, and its verifier and backend
already benefit from dense indexing. Permanent sparse final IDs would spread
hash or tree lookup through hot analyses, leave avoidable holes in dumps and
tables, and make downstream code handle two storage strategies forever.

Niflheim's simpler backend CFG passes can rewrite just the relevant
terminators because much less metadata refers to blocks. Skald needs a central
exhaustive remapper: a locally correct terminator rewrite can still leave a
path condition, logical-expression record, or static publication boundary
pointing at a removed block.

This proposal does not otherwise design Skald's pass registry, optimization
levels, pass selection syntax, or analysis manager. Those should build on the
rewrite boundary in a separate proposal.

## Constraints and non-goals

The proposal assumes and preserves these constraints:

1. Every compiled program is a closed world. Rewriting can inspect every
   executable definition and every statically represented caller or callee.
2. The generated program is single threaded. This removes source-level races,
   atomics, and synchronization from optimization semantics, but compiler
   implementation threads may still process independent callables in the
   future.
3. Committed MIR remains deterministic, dense, directly indexable, and valid
   under the existing ordinary MIR verifier.
4. Program-level semantic identities are never renumbered by a callable-local
   rewrite.
5. The rewrite boundary enforces reference and table integrity. The ordinary
   MIR verifier remains authoritative for types, definition-before-use,
   ownership, liveness, cleanup, path semantics, and all other semantic
   invariants.
6. The static-lifecycle baseline authority remains immutable. Rewriting
   executable MIR never edits proof authority to make a transformation pass.
7. Proof-provenance normalization, SSA or block parameters, alias and effect
   analysis, a virtual-register backend, whole-program callable pruning, and
   individual optimization algorithms remain separate work.
8. No stable external MIR serialization format or plugin ABI is introduced.
9. No persistent MIR instruction identity is introduced. Instruction
   positions are local to one immutable block snapshot or one editor
   operation.
10. Initial HIR-to-MIR lowering remains append-oriented and does not need to
    use the optimization editor.

## Design principles

1. **Use different representations for editing and consumption.** Dense final
   MIR is valuable; requiring density after every edit is not.
2. **Rewrite a complete callable package.** Body tables, signature storage,
   owner-specific attachments, and metadata commit together.
3. **Make deletion explicit.** The infrastructure reports dangling references
   instead of guessing whether to delete, redirect, or substitute their users.
4. **Centralize exhaustive traversal.** A new MIR reference form must create a
   compile-time maintenance point in the mapper.
5. **Preserve semantic identity.** Only callable-local representational IDs are
   compacted.
6. **Keep intermediate invalidity private.** Sparse slots and tombstones never
   masquerade as a `MirProgram`.
7. **Commit deterministically.** Output numbering depends only on explicit
   editor order and pass decisions, never hash iteration, addresses, or worker
   scheduling.
8. **Do not hide semantic repair in compaction.** Commit checks references;
   verification checks meaning.
9. **Return facts, not side effects.** A commit returns its program, maps, and
   change summary; it does not log, render, verify, or mutate global state.
10. **Keep the boundary usable without duplicating MIR.** The editor reuses
    existing MIR node types behind private sparse tables instead of maintaining
    a second mirrored instruction and terminator hierarchy.

## Vocabulary and commit invariant

This proposal uses the following terms:

- **committed MIR** — ordinary `MirProgram` shape with dense callable-local
  tables and no tombstones;
- **callable package** — one function, member, or static-initializer executable
  definition together with its signature and owner-specific attachments;
- **edit slot** — a stable callable-owned local ID used to address an entity
  during one rewrite transaction;
- **tombstone** — a deleted table slot retained until commit so other edit-slot
  indices do not shift;
- **live order** — the explicit deterministic order in which retained and new
  entities will be emitted;
- **identity map** — the complete old-edit-slot to new-committed-ID mapping for
  one identity kind;
- **rehoming** — copying selected entities from one callable into freshly
  allocated destination slots while substituting all source-local references;
  and
- **attachment** — a callable-local reference outside the four declaration
  tables, such as receiver, parameter, return storage, body entry, or static
  publication block.

For identity kind `K`, callable `C`, and live sequence `L[K]`, commit constructs
a total map over retained slots:

```text
map[K](old_id_at L[K][i]) = K(C, i)
```

It then establishes:

```text
every retained declaration ID equals its output table position
every retained callable-local reference maps to one retained declaration
every mapped identity is owned by C
every required attachment maps successfully
every live-order entry occurs exactly once
no tombstone is emitted
```

Optional guards have no committed declaration table. Opening an editor scans
their references into a private guard registry so allocation and deletion are
still explicit during the transaction. Commit rejects a reference to a
deleted, unknown, or foreign guard, sorts the live guard slots
deterministically, and maps them densely to `OptionalGuardId(C, 0..n)`. The
ordinary verifier remains responsible for proving valid begin/end pairing and
path-sensitive nesting.

Passing these checks does not prove that each value still has exactly one
definition, that a redirected CFG satisfies ownership joins, or that retained
logical provenance describes the new CFG. Those are semantic verifier duties.

## Decision register

| ID | Question | Confirmed direction | State |
|---|---|---|---|
| [DMI1](#dmi1--keep-committed-mir-dense) | Does final MIR become sparse? | No; dense committed tables remain the only verified and backend-facing form | **Confirmed** |
| [DMI2](#dmi2--use-a-private-sparse-callable-edit-transaction) | How are identities stable during edits? | Callable-owned sparse slots and tombstones inside a private owned transaction | **Confirmed** |
| [DMI3](#dmi3--compact-atomically-in-explicit-deterministic-order) | When and how are IDs renumbered? | Once at commit, from explicit live order, with complete maps | **Confirmed** |
| [DMI4](#dmi4--centralize-exhaustive-callable-local-id-traversal) | How are all references updated? | One model-wide exhaustive visitor/remapper including attachments and metadata | **Confirmed** |
| [DMI5](#dmi5--reject-dangling-or-foreign-references-without-guessing) | What happens to references to deleted nodes? | Commit fails deterministically; the pass must explicitly delete, redirect, or substitute | **Confirmed** |
| [DMI6](#dmi6--expose-narrow-edit-operations-not-mutable-definition-tables) | What may passes mutate? | An editor facade with typed allocation, lookup, replacement, deletion, and functional instruction editing | **Confirmed** |
| [DMI7](#dmi7--treat-path-logical-guard-and-publication-metadata-as-first-class) | How is non-instruction metadata handled? | It participates in the same transaction; no automatic semantic repair | **Confirmed** |
| [DMI8](#dmi8--adapt-all-executable-definition-kinds-without-restructuring-public-mir) | How are function, member, and initializer bodies unified? | Private owned adapters and narrow container take/replace APIs | **Confirmed** |
| [DMI9](#dmi9--provide-explicit-rehoming-for-future-inlining) | How may entities cross callable ownership? | Allocate destination slots first, then copy through explicit substitutions and complete maps | **Confirmed** |
| [DMI10](#dmi10--invalidate-local-id-keyed-analysis-at-commit) | What happens to analyses and edit IDs? | They are transaction-scoped; commit reports maps and changes but no durable cache remains valid implicitly | **Confirmed** |
| [DMI11](#dmi11--integrate-through-the-final-mir-seal-and-central-verification) | How does rewriting interact with verification? | Only the pass pipeline may invalidate the seal; commit returns raw MIR and central verification reseals it | **Confirmed** |
| [DMI12](#dmi12--deliver-by-reference-census-and-adversarial-rewrite-tests) | How is exhaustiveness demonstrated? | Inventory, no-op parity, gap compaction, deletion/insertion, malformed, all-definition-kind, and determinism tests | **Confirmed** |

## DMI1 — Keep committed MIR dense

The verified `MirProgram` representation should retain direct vector lookup and
the current invariant that declaration ID indices equal table positions.
Verifier dataflow, dumps, and backend lowering can continue to use compact
arrays without an additional lookup abstraction.

This decision distinguishes a representation's consumption strengths from its
editing weaknesses. Dense indexing is not itself the defect. The defect is
exposing only that representation to structural transformations.

A permanent sparse-ID model was considered and rejected. It would make simple
deletion cheap, as in Niflheim backend IR, but would require map lookup or
parallel index construction in every Skald consumer, preserve arbitrary holes
indefinitely, and turn one optimizer concern into a permanent whole-compiler
cost. Generational arenas in committed MIR have the same issue and add stale
generation handling that is unnecessary for immutable phase products.

## DMI2 — Use a private sparse callable edit transaction

Opening a callable for rewrite moves its common tables and attachments into an
owned `MirCallableEdit`. Storage, values, blocks, and path conditions are held
in slot tables equivalent to `Vec<Option<T>>`. Existing IDs continue to name
their original slots. New entities receive monotonically appended slots, and
deletion replaces an entry with a tombstone.

The transaction is not a `MirProgram`, is not accepted by verification or a
backend, and remains private to the compiler crate. Reusing the existing MIR
node types avoids a mirrored editable instruction hierarchy that would need to
track every future operation. Inside the transaction, an ID still means a
callable-owned slot; the only relaxed property is contiguity of live slots.

The editor owns an explicit block-order sequence separate from block slots.
Deleting a block removes it from that order. Creating a block requires an
explicit append, before, or after position, so layout-affecting transformations
do not depend on arena allocation details. Storage, values, and path
conditions default to retained slot order followed by allocation order.
Path-condition creation requires its parent, if any, to have been allocated
first.

Logical-expression records use an ordered tombstone-capable sequence because
they have no separate identity. Optional guards use a private registry seeded
from all existing guard references; the registry is not emitted as a new MIR
table. Instruction sequences remain block-owned. There is no persistent
`InstructionId`: passes either functionally rewrite one block's instruction
vector or plan positional edits against an immutable block snapshot and apply
them in a defined order. This avoids adding another public identity merely to
manage vector mutation.

The editor contains no global allocator, shared mutable registry, or worker
index. Independent callable transactions could therefore run on compiler
workers later and still produce the same local IDs. That is compiler
implementation freedom; the generated program's single-threaded contract is
unchanged.

## DMI3 — Compact atomically in explicit deterministic order

Commit consumes the transaction. It first validates each live-order sequence,
constructs complete maps, validates and maps every reference, and only then
constructs an owned dense callable package. A partially compacted definition
is never installed into a `MirProgram`.

The canonical policies are:

- storage and values: surviving original slot order, followed by new slot
  allocation order;
- blocks: the editor's explicit block order;
- path conditions: surviving original order followed by new creation order,
  with parent-before-child validation;
- optional guards: surviving guard indices in ascending order; and
- logical-expression records: surviving explicit record order.

Passes may request a different block order through the editor because block
layout is a legitimate transformation decision. No equivalent arbitrary
storage or value reordering API is initially exposed. If a later measured
optimization needs it, it can be added without changing the commit invariant.

The commit result contains the rebuilt callable, the five local identity maps,
and a change summary. Maps cover retained pre-commit slots only; querying a
deleted slot is an error rather than returning `None` at arbitrary call sites.
New committed identities can be queried from the editor handle that created
them.

Compaction is not a semantic transformation. It does not fold branches,
redirect missing targets, remove unused declarations, infer metadata deletion,
or reorder for reachability. Those choices belong to the pass.

## DMI4 — Centralize exhaustive callable-local ID traversal

One private MIR rewrite owner should traverse all callable-local identity
fields. It covers:

- function and member receiver, parameter, and return-storage attachments;
- static-initializer publication entry and exit blocks;
- storage, value, block, and path-condition declaration IDs;
- body entry and every terminator successor form;
- every instruction, rvalue, argument, place base, and projection;
- path-condition parents, activations, predecessor blocks, and merge blocks;
- logical-expression result, operand, condition, split, selection, short-path,
  right-side, exit, and join identities; and
- optional-view begin/end guard identities.

Traversal should be expressed as a reusable visitor/remapper over mutable ID
references. Collection, ownership validation, rehoming, and compaction use the
same walk rather than maintaining independent lists of reference sites.
Enum matches and struct destructuring in the walk must be exhaustive and must
not use wildcard variants or `..` for identity-bearing structures. Adding an
operation variant or an identity-bearing field should therefore force a
compile-time review of the traversal.

The mapper belongs under a `mir::rewrite` facade, not in individual
optimization passes. Read-only analysis helpers may share low-level operand
classification later, but a pass may not copy the remapping match and become a
second authority.

This decision also creates a maintainability rule: any new callable-local ID
kind or reference site must update the identity inventory, mapper, round-trip
fixture, and malformed-reference coverage in the same change.

## DMI5 — Reject dangling or foreign references without guessing

If a retained node references a tombstoned slot, an undeclared slot, or an ID
owned by another callable, commit returns `MirRewriteError`. The error records
the callable, identity kind and value, and a deterministic structural site
such as header, publication, block, terminator, instruction position, path
condition, or logical record.

The editor does not automatically:

- redirect users of a removed value to another value;
- forward predecessors of a removed block;
- remove a path condition when one of its blocks disappears;
- delete logical metadata because its selected expression changed;
- remove storage liveness operations with a storage declaration; or
- drop an instruction merely because its result declaration was deleted.

Each choice can change semantics and must be explicit in the pass. Higher-level
helpers may perform coordinated operations—for example, value substitution
followed by definition removal—but those helpers are ordinary tested rewrite
algorithms built on the same checks.

Internal rewrite errors are compiler failures, not source diagnostics. Their
ordering and text should nevertheless be deterministic so tests and reports
identify the first broken reference reliably.

## DMI6 — Expose narrow edit operations, not mutable definition tables

Production passes should receive `MirCallableEdit`, not `&mut Vec<MirStorage>`
or mutable access to program definition containers. The initial facade should
provide operations in these groups:

- typed lookup and iteration over live storage, values, blocks, path
  conditions, and logical records;
- allocation of storage, values, blocks, path conditions, and optional guards;
- explicit block-order insertion and edge redirection;
- same-type value-use substitution and place/storage substitution where a
  caller supplies the required semantic proof;
- functional instruction-list rewrite for one block;
- explicit removal of declarations, blocks, path conditions, logical records,
  and paired guard uses; and
- commit, returning maps and a structured change summary.

Convenience helpers must state their preconditions. `replace_value_uses`, for
example, checks callable ownership and equal MIR type but cannot claim that
the replacement dominates every use; a dominance-aware pass must establish
that fact. `redirect_edges` updates executable successor references selected by
the caller but does not silently rewrite path or logical provenance.

The model's existing public fields need not all become private in this project.
The important production boundary is that executable definition containers do
not gain general `iter_mut` access and that future passes use the editor rather
than splice dense vectors directly. Test-only corruptors may retain narrow
mutation access for verifier tests.

## DMI7 — Treat path, logical, guard, and publication metadata as first class

Path conditions and logical-expression records are currently executable-proof
metadata tied to lowering CFG shape. This proposal does not normalize or
remove that coupling; it ensures a transformation cannot overlook it.

A pass that changes the represented shape has three explicit choices:

1. retain the metadata and update all referenced identities;
2. rebuild it to describe the new verified shape; or
3. delete the obsolete record and any dedicated activation storage and
   operations when the MIR verifier permits its absence.

Commit checks only declaration/reference integrity. Final MIR verification
still proves parent ordering, activation ownership, selection predecessors,
merge structure, logical-expression correspondence, optional-guard pairing,
and path-sensitive nesting. This separation prevents the generic compactor
from accumulating knowledge of every optimization's semantic intent.

Optional guards are included even though they have no table and are not
currently required to be dense. Canonical remapping keeps optimized dumps
compact and prevents imported guard pairs from colliding with destination
pairs.

`MirStaticPublication` is an attachment of a static-initializer callable, not
ordinary lifecycle-plan data. Both of its block references are remapped with
the body. Lifecycle activation and shutdown region order, baseline authority,
and stable static identities are not callable-local and remain unchanged.

The larger separation of proof provenance from executable MIR remains the
fourth optimization-architecture discovery. This boundary makes incremental
normalization possible later without requiring every earlier CFG pass to own
ad-hoc ID surgery.

## DMI8 — Adapt all executable definition kinds without restructuring public MIR

The implementation should introduce a private owned callable-package enum with
function, member, and static-initializer variants. Each variant separates:

- stable semantic owner data;
- common editable storage, value, and body data; and
- callable-specific attachments.

Opening and committing use one implementation for the common data and small
exhaustive adapters for attachments. This removes the need for three copies of
compaction while preserving the existing public MIR field layout.

A broad refactor that embeds a new public `MirCallableBody` inside every
definition was considered. It would remove some repeated fields and lookup
methods, but it would also create large unrelated churn across lowering,
verification, dumps, tests, lifecycle synthesis, and backend code. Private
owned adapters achieve the optimization boundary with less migration risk.
The public common-body refactor can be reconsidered independently if repeated
model logic remains after this work.

`MirProgram` gains narrow crate-private ownership-transfer support that can
extract executable definitions in deterministic container order and rebuild
the same containers only after every requested edit commits. Static
initializers remain in their lifecycle activation order; program-level function
and member identities and table positions do not change.

## DMI9 — Provide explicit rehoming for future inlining

Ordinary compaction never changes the callable owner embedded in an ID.
Inlining and specialization need a separate rehoming operation because copied
callee-local IDs are invalid in the caller.

The importer should operate in two phases:

1. allocate all required destination storage, values, blocks, path conditions,
   and guards and construct complete source-to-destination maps; then
2. clone selected MIR through the exhaustive mapper, applying explicit
   substitutions for receiver, parameters, return destination, entry, exits,
   and any other boundary value.

No raw callee-local reference may survive import. A reference outside the
selected clone set must have an explicit substitution or causes a rewrite
error. Program-level callable, type, field, method, static, and declaration IDs
are copied unchanged because they name closed-world semantic entities rather
than callee-local representation slots.

`MirStorage::source` needs an explicit policy. Existing caller storage retains
its `BindingId`. Imported callee locals may not retain a foreign callable's
binding provenance; an inliner must materialize compiler-owned storage with an
appropriate kind and no source binding, or provide a future dedicated inline
provenance representation. The generic importer must not silently forge a
caller binding.

This project supplies the rehoming primitive and adversarial tests, not a
production inliner. Call-site splitting, argument evaluation, ownership
transfer, cleanup, return merging, recursion budgets, and profitability remain
inliner design work.

## DMI10 — Invalidate local-ID-keyed analysis at commit

An analysis result keyed by a pre-commit `StorageId`, `ValueId`, `BlockId`,
`PathConditionId`, optional guard, or instruction position is not implicitly
valid after compaction. Editor handles and instruction positions are scoped by
convention and API ownership to one transaction.

Commit returns identity maps for three limited purposes:

- remapping callable attachments inside the same atomic commit;
- testing and deterministic reporting of what the transaction changed; and
- explicit handoff to an immediately adjacent owner that declares how its data
  is remapped.

The maps do not make arbitrary cached analyses valid. A later analysis manager
must either recompute an analysis, prove it preserved and remap all of its
keys, or use a pass-specific update operation. There is no global analysis
cache or revision counter in this proposal.

The change summary should contain already-known counts—inserted, removed,
replaced, or redirected entities—without scanning committed MIR again. It is a
pass result suitable for the existing structured-reporting owner; the rewriter
itself emits no text.

Whole-world analyses may plan edits from one immutable verified snapshot and
then apply per-callable transactions. They must not retain borrowed node
references while committing. Because program-level semantic IDs remain
stable, call-graph and declaration facts can be reconstructed or selectively
updated without callable renumbering.

## DMI11 — Integrate through the final-MIR seal and central verification

Only the target-independent pass pipeline should be able to consume a
`VerifiedFinalMirProgram` into raw executable MIR for transformation. The
invalidation method remains crate-private. `MirCallableEdit::commit` returns
raw MIR and never constructs a verified wrapper.

Once transformations are registered, the safe pipeline shape is:

```text
raw final MIR
  -> ordinary + lifecycle-realization verification
  -> sealed verified final MIR
  -> pipeline-private seal invalidation
  -> one or more owned rewrite transactions
  -> dense raw final MIR
  -> ordinary + lifecycle-realization verification
  -> sealed verified final MIR
  -> backend
```

The initial verification prevents an optimization from relying on malformed
producer input. The final verification proves all semantic MIR invariants and
the monotone static-lifecycle realization relation after transformation. Debug
and test configurations should support verification after each transforming
pass to localize defects; whether release builds do that is a later pass-pipeline
policy. Reporting counters must record the executions that actually occurred.
With the current empty pipeline, the existing single final verification remains
sufficient.

The rewrite boundary does not classify static-lifecycle effects. A pass that
changes static access, reachability, lifecycle operations, or possible callees
invalidates the seal and relies on final realization verification. No editor
API exposes mutable baseline authority.

Backend-specific rewrites remain behind target legality and use backend-owned
identities. This proposal is for target-independent final MIR only.

## DMI12 — Deliver by reference census and adversarial rewrite tests

Implementation should begin with an explicit census test module listing every
current model family that may contain callable-local IDs. The central mapper is
then added before any editor operation uses it.

Required test groups are:

1. **No-op parity:** opening and committing every executable callable from a
   representative source corpus produces equal MIR and an identical dump.
2. **Artificial-gap round trip:** a test-only rekeying operation moves live
   entities into sparse slots, updates all references through the mapper, and
   proves commit returns the original dense MIR. This exercises reference
   sites without requiring each entity to be semantically dead.
3. **Deletion and insertion:** focused fixtures remove and add values, storage,
   blocks, path conditions, logical records, and optional guards, then pass the
   ordinary and lifecycle verifiers.
4. **All callable kinds:** ordinary functions, instance and static members,
   lifecycle members, and static initializers all use the same common commit
   path. Static publication references must change when their blocks move.
5. **Malformed edits:** retained references to deleted or foreign storage,
   values, blocks, path conditions, and guards fail with deterministic rewrite
   errors. Missing or duplicate live-order entries and invalid attachments are
   rejected.
6. **Rehoming:** a synthetic cross-callable clone proves every local identity
   receives the destination owner and that missing boundary substitutions are
   rejected.
7. **Semantic handoff:** structurally committable but semantically invalid
   edits are rejected by `verify_final_mir`, demonstrating that commit does not
   counterfeit the verifier seal.
8. **Determinism:** repeated and independent-process runs produce identical
   committed IDs, MIR dumps, rewrite errors, and change statistics.

The implementation is not complete merely because a `remove_block` helper
exists. It is complete when the common transaction covers the entire identity
surface, no production pass needs direct dense-vector surgery, and the tests
prove successful compaction and failure behavior. The first optimization pass
should be planned separately and must use this boundary; likely early consumers
are dead-pure-definition cleanup and conservative unreachable-block pruning.

## Frozen module ownership direction

The exact file split remains implementation detail, but the ownership should
follow Skald's facade-oriented Rust organization:

```text
mir/
  rewrite/
    mod.rs          supported crate-private facade
    edit.rs         sparse callable transaction and typed operations
    map.rs          exhaustive local-ID visitor/remapper
    commit.rs       validation, deterministic maps, and dense rebuild
    import.rs       cross-callable rehoming
    error.rs        structured internal rewrite failures
    tests.rs        focused unit and census tests

passes/
  pipeline.rs       seal invalidation, pass ownership, verification policy
```

Program-container ownership transfer remains with the MIR model owner and is
exposed only as narrowly as the rewrite facade requires. Individual
optimization passes should live under a later pass facade and consume the
supported editor operations; they should not import commit internals.

## Rejected alternatives

### Make committed MIR permanently sparse

This copies Niflheim's useful editing property but gives up Skald's compact
direct lookup throughout verification, analysis, and backend lowering. It also
retains holes in stable dumps and makes every consumer choose or build an
index. The private transaction obtains the same editing benefit without that
permanent cost.

### Renumber immediately after every edit

This preserves dense MIR continuously but makes a sequence of deletions
quadratic and exposes each pass to repeated whole-callable remapping. It also
creates many intermediate opportunities to miss metadata. One atomic commit is
simpler and safer.

### Give each pass its own old-to-new maps

The model has too many reference forms for duplicated traversal to remain
sound. A new MIR operation could be supported by lowering and backend code
while an older pass silently fails to remap one field. One exhaustive owner is
a required maintenance boundary.

### Introduce a second fully mirrored optimization MIR

A separate SSA or generic optimization IR may eventually be justified, but
mirroring today's ownership-rich instruction, terminator, place, path,
optional, array, I/O, and lifecycle vocabulary solely for deletions would be a
large permanent maintenance burden. Sparse tables around existing nodes are a
smaller foundation and do not foreclose a later IR.

### Parameterize every MIR node over an identity family

Generic dense/edit identity types would provide stronger type separation, but
would push type parameters through almost the entire MIR model, verifier,
dumper, lifecycle analysis, and backend. The private transaction already
prevents sparse shape from crossing the public phase boundary. The added type
complexity is not justified initially.

### Automatically cascade deletion

Deleting all users of a removed node sounds convenient but is an optimization
algorithm, not neutral infrastructure. Effects, ownership, cleanup, path
metadata, and control flow make cascading semantics context dependent. Helpers
may coordinate well-defined cases; commit itself remains conservative.

### Add persistent instruction IDs now

Niflheim's backend IR has instruction IDs, but Skald currently identifies
scalar definitions through `ValueId` and effecting operations through their
block positions. Adding another committed identity expands the rewrite surface
without solving a demonstrated requirement. Functional per-block instruction
editing is sufficient for this foundation.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A reference form is omitted | One exhaustive mapper, no wildcard identity-bearing matches, census and artificial-gap tests |
| A pass mistakes a tombstoned ID for a valid declaration | Editor lookups return typed errors/options; commit rejects all dangling references |
| Compaction changes semantics while appearing structural | Commit performs no implicit repair; central final verification remains mandatory |
| Static initializer publication points to old blocks | Publication is a required callable attachment in the mapper and dedicated tests |
| Path or logical metadata becomes shape-inconsistent | Identity remap is automatic; rebuild/deletion is explicit; semantic verifier checks final correspondence |
| Cross-callable cloning leaks foreign IDs | Two-phase rehoming with total maps and required boundary substitutions |
| Output depends on pass hash iteration or compiler workers | Explicit live order, ordered maps/sets, callable-local allocation, determinism tests |
| Future analyses silently retain stale local IDs | Transaction-scoped handles, explicit commit maps, default invalidation policy |
| The common-body cleanup causes excessive migration churn | Use private adapters; defer public model restructuring |
| Verification cost grows with a pass pipeline | Keep one required input and final check, support per-pass debug checks, measure before changing policy |

## Delivery ownership

The
[dense callable-local MIR identity rewriting roadmap](DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md)
owns implementation in this dependency order:

1. freeze the callable-local identity inventory and mapper contract;
2. implement exhaustive collection/remapping with no-op and artificial-gap
   tests;
3. implement sparse storage/value/block/path/logical edit tables and atomic
   commit;
4. add function, member, and static-initializer adapters plus program-level
   ownership transfer;
5. add narrow substitution, edge, instruction, deletion, and allocation
   helpers;
6. add optional-guard canonicalization and cross-callable rehoming;
7. integrate seal invalidation, structured changes, and central verification;
   and
8. complete malformed, determinism, corpus, and repository-wide validation.

That roadmap should remain infrastructure-focused. The first production
optimization and the general selectable pass registry should receive their own
explicit tasks or proposals so optimization policy does not become hidden
inside identity compaction.

## Frozen decision summary

The confirmed design establishes that:

- committed MIR should stay dense while edit transactions become sparse;
- the transaction boundary is one complete callable package, not just
  `MirBody`;
- the identity inventory covers storage, values, blocks, path conditions,
  optional guards, logical records, and static publication attachments;
- central exhaustive remapping is preferable to pass-local traversal;
- dangling references should fail rather than cascade implicitly;
- public MIR layout should not be broadly refactored as part of this work;
- cross-callable rehoming belongs in the foundation but production inlining
  does not;
- local-ID-keyed analyses are invalidated by default at commit;
- only central verification may recreate `VerifiedFinalMirProgram`; and
- implementation completion requires adversarial compaction and all-callable
  coverage, not merely an editor type.

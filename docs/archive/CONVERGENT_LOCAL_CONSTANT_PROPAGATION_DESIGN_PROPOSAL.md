# Convergent Local Constant Propagation Design Proposal

Status: frozen decision record. CLP1 through CLP18 were confirmed together on
2026-09-05 and delivered by the completed
[convergent local constant propagation roadmap](CONVERGENT_LOCAL_CONSTANT_PROPAGATION_ROADMAP.md).

This proposal defines expression-complete constant propagation for Skald's
existing target-independent proof-rich final-MIR simplification passes. Within
one callable and the explicitly supported integer and boolean domain, constant
results should propagate through primitive assignments, canonical private
scalar carriers, successful checked integer protocols, and constant-left
short-circuit logical selection regardless of the depth of the source
expression or lowered MIR dependency graph.

The purpose is not merely to implement the former narrow scalar-spill
provenance candidate. The work should establish one reusable, terminating
callable-local constant solver, separate semantic evaluation from structural
protocol recognition, and let multiple independently selectable passes consume
the same kind of solution without repeatedly mutating and reverifying the
whole program until no pass changes it.

The design builds on the implemented
[local final-MIR simplification](LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md)
and
[checked integer constant protocol simplification](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_ROADMAP.md).
It resolves the remaining nested-protocol limitation recorded in the
[checked-integer discoveries](CHECKED_INTEGER_CONSTANT_PROTOCOL_SIMPLIFICATION_DISCOVERIES.md#nested-successful-protocols-do-not-feed-enclosing-scalar-carriers),
but intentionally chooses a broader architectural boundary than a one-off
carrier substitution rule.

The language contract does not change. Compilation remains permanently
whole-world and resulting programs remain single threaded. Evaluation order,
checked failure, sequential mutation, aliases, allocation, ownership,
destruction, static activation and shutdown, diagnostics, panic locations,
runtime traces, and target-independent arithmetic semantics remain observable
exactly as before.

## Intended outcome

The design should provide:

- a callable-local constant dependency graph over existing dense `ValueId` and
  eligible `StorageId` identities;
- one monotonic worklist solver which reaches a unique stable solution without
  a configured expression-depth or iteration limit;
- an explicit completeness guarantee for the closed supported operation and
  carrier domain;
- reusable exact primitive and checked-integer evaluators rather than a second
  arithmetic implementation;
- structural checked-protocol observations that do not require their operands
  to be literal constant assignments;
- structural logical-expression observations with exact constant-left
  short-circuit selection;
- narrowly certified private scalar-carrier facts with exhaustive access,
  unique-write, dominance, type, and protocol-ownership checks;
- separate rewrite plans for ordinary primitive assignments, successful
  checked integer protocols, and selected short-circuit logical CFG;
- retention of the existing independently selectable
  `primitive-constant-folding` and
  `checked-integer-constant-folding` pass names;
- one new independently selectable `constant-short-circuit-folding` pass at a
  narrow proof-consuming transition stage;
- one immutable analysis followed by at most one atomic program rewrite per
  selected pass occurrence;
- atomic composition of selected logical CFG rewriting with mandatory proof
  normalization, without retaining logical provenance in normalized MIR;
- no hidden pass repetition, generic optimizer fixed point, or persistent
  cross-seal analysis cache;
- deterministic facts, plans, measurements, dumps, and dense commit;
- preservation of source spans and surviving MIR identities; and
- focused and source-to-native evidence for arbitrarily deep supported
  expressions, static failure barriers, selection independence, idempotence,
  and optimization-off parity.

“Expression-complete” is deliberately scoped. It means that every result in a
finite callable-local dependency graph is derived when every semantically
evaluated leaf is a supported constant and every intervening edge is one of
the supported primitive, carrier, successful checked-protocol, or
constant-selected logical relations. A short-circuited right operand is not a
semantically evaluated leaf. This does not mean compile-time evaluation of
arbitrary Skald programs.

## Current architecture and limiting behavior

### Straight-line primitive folding is already depth independent

`PrimitiveConstantFacts` scans assignments in instruction order inside one
basic block. A newly folded result is inserted immediately, so a straight-line
chain of supported primitive operations already folds to arbitrary depth in
one pass occurrence. Facts are cleared at every block because `ValueId` values
are block-local and current folding deliberately does not reason about
storage.

The limitation is therefore not expression-tree recursion. Checked operations
lower into multiple blocks and communicate through `ScalarSpill` storage.
Those storage boundaries disconnect the existing block-local facts.

### Checked-protocol discovery currently owns constant provenance

The checked-integer observer recognizes an exact verifier-owned check,
success, failure, store, join, and reload topology. It currently also requires
each operand carrier's unique store source to be an assignment which evaluates
as a constant without consulting any previously derived facts.

That fused responsibility makes an exact literal acceptable but rejects a
constant reloaded from an earlier successful checked protocol. Structural
protocol validity and the semantic value flowing into the protocol are
different concerns and should have different owners.

### Checked folding plans one immutable snapshot

`CheckedIntegerFoldPlan` discovers every candidate from the current verified
MIR and then rewrites all selected candidates through one atomic program
transaction. This is a sound and useful transaction boundary. It also means
that rewriting an inner protocol cannot expose a new outer candidate during
the same occurrence when discovery itself cannot see through the retained
carrier.

Repeating the same pass does not solve the underlying problem: after a checked
rewrite, the result is still stored and reloaded through its result carrier.
Adding a fixed number of primitive/provenance/checked occurrences would only
move the maximum supported nesting depth.

### Short-circuit logic crosses the proof-normalization boundary

`&&` and `||` are not eager boolean rvalues. Lowering records each
`MirLogicalExpression` as an exact split, activation selection, right-hand
region, short path, result carrier, and join. The right operand is absent from
execution on the short path, and path-sensitive lifetime, initialization,
ownership, and cleanup verification consume the record before normalization.

This representation makes all four constant-left rules semantically exact:

```text
false && rhs -> false       rhs is not evaluated
true  && rhs -> rhs         rhs is evaluated exactly once
true  || rhs -> true        rhs is not evaluated
false || rhs -> rhs         rhs is evaluated exactly once
```

The right operand does not need to be effect-free. On a short path it was
never executed; on a selected-right path its complete existing region must be
retained and executed once. The left operand is always evaluated and may not
be removed merely because its value selects a known path.

The current proof-rich CFG pass cannot rewrite this structure because its
output must pass proof-rich verification against the unchanged exact logical
record. The current final-stage passes cannot reliably rediscover the same
semantic relationship because mandatory normalization has deleted the record
and reclassified its activation carrier. A safe selectable rewrite therefore
needs a narrow transition owner which reads already verified proof provenance
and produces normalized executable MIR in the same atomic transaction that
consumes it.

### The pipeline deliberately has no generic fixed-point manager

The selectable pipeline gives every occurrence one verified input seal and
one atomic rewrite opportunity. A changed occurrence commits dense MIR and is
immediately reverified before any later occurrence. Explicit schedule
repetition is useful when distinct transformations predictably expose one
another's work, but it is not an appropriate representation of unbounded
expression depth.

A general pipeline fixed-point group would also broaden selection,
occurrence-numbering, checkpoint, measurement, failure-attribution, and
verification contracts. This proposal does not require that larger facility.

Supporting optional logical selection does require one smaller pipeline
extension: a proof-consuming transition stage between ordinary proof-rich
passes and final passes. It is a representation boundary, not a convergence
mechanism.

### Existing representation and rewrite foundations are sufficient

MIR already provides dense callable-local values and storage identities,
exact checked-operation topology, block and instruction locations, local CFG
facts, value-use traversal, and atomic sparse-to-dense rewriting. The primitive
and checked-integer evaluators already define the required target-independent
semantics.

No SSA conversion, block parameter, persistent instruction identity, new MIR
rvalue, or new storage kind is required. The missing representations are a
transient analysis graph and a narrow typed transition capability, not a new
executable IR or retained logical-provenance table.

## Scope and non-goals

- The solver runs only on verified proof-rich final MIR and only within one
  callable.
- The supported constants remain `i64`, `u64`, `u8`, and `bool` initially.
- The supported ordinary producers remain the operation families already
  accepted by the exact primitive evaluator.
- The supported checked producers remain integer division, integer remainder,
  and integer shifts with the exact successful semantics already implemented.
- The supported logical producer is an exactly verified `MirLogicalExpression`
  whose left result is constant. The selected right result may remain dynamic.
- Constant facts may cross only certified compiler-owned scalar carriers
  participating in recognized checked protocols, plus an exact logical result
  relation supplied by a verified logical record.
- A statically failing checked operation produces no result fact and remains
  executable, with its original failure reason, location, and timing.
- A right operand skipped by a constant-left logical selection is not analyzed
  as a required result dependency and may contain calls, mutation, allocation,
  ownership work, or a checked failure without blocking the short result.
- A right operand selected by constant-left logic remains evaluated exactly
  once with all of its existing effects and failure behavior.
- The solver does not interpret calls, callable addresses, arbitrary loads,
  source locals, parameters, aliases, fields, arrays, shared pointees,
  ownership operations, statics, optional protocols, type tests, or checked
  floating conversions.
- Floating arithmetic, comparisons, and numeric conversions remain excluded
  until Skald has a deterministic target-independent IEEE evaluator.
- The solver is not SCCP, general copy propagation, store-to-load forwarding,
  global value numbering, common-subexpression elimination, range analysis,
  symbolic algebra, or compile-time function evaluation.
- General right-constant identities such as `lhs && false` or `lhs || true`
  are excluded when the left result is not constant. Discarding an evaluated
  left operand would require an independently proven total-and-effect-free
  classification; “side-effect-free” alone does not exclude failure.
- No source diagnostic is added, removed, or reordered.
- No operation is reordered, and no evaluated operand producer is deleted by
  a constant-folding or logical-selection pass.
- No cross-pass analysis cache, preservation declaration, or general analysis
  manager is introduced.
- The proposal does not remove scalar storage declarations or lifetime
  operations. Existing cleanup passes retain independent ownership of
  deletion.
- The logical transition rewrites selection edges and, where the selected
  result is constant, its protocol-owned result load. It leaves newly
  unreachable blocks and newly dead ordinary definitions to independently
  selected cleanup passes.

Whole-world compilation ensures that no later-loaded code can introduce an
unknown executable dependency. It does not make this local solver
interprocedural. Single-threaded execution removes concurrent writes, but not
sequential mutation or aliases; those remain barriers outside the certified
private-carrier subset.

## Constant dependency graph

### Graph identities

The graph is an immutable view of one verified callable and contains two
fact-bearing node kinds:

- `Value(ValueId)` for a transient scalar result; and
- `Carrier(StorageId)` for one eligible compiler-owned scalar carrier.

Checked protocols are structural producers connecting operand facts to their
result assignment and result carrier. Logical records are conditional
producers connecting a left fact and, only when selected, a right fact to the
logical selected-result value. They may have private analysis IDs or stable
discovery positions, but they do not become persistent MIR identities.

Dense `ValueId` and `StorageId` tables should back the facts where practical.
Reverse dependency lists let a newly solved node enqueue only its direct
consumers. Graph construction follows definition, block, instruction, and
operand order so diagnostics and observations are deterministic even though
the final solution must not depend on queue order.

### Value producers

A value node may receive a constant from exactly one reviewed producer:

1. a supported literal assignment;
2. a supported primitive rvalue whose operand value nodes are constant;
3. an exact base-place load from a certified carrier at a site dominated by
   that carrier's unique store; or
4. the result assignment of a recognized checked integer protocol whose two
   operands solve to an exact successful result; or
5. the selected-result value of a recognized logical expression according to
   the constant-left short-circuit transfer rules.

The graph records unsupported assignments as barriers rather than trying to
infer their behavior. A `ValueId` still has exactly one MIR definition; the
solver does not merge multiple alternative definitions.

### Certified carrier relation

A `StorageId` may carry a constant only when all of the following are proven
from the current verified callable:

1. its declaration is `MirStorageKind::ScalarSpill` with an exactly supported
   primitive type;
2. it is named as an operand or result carrier by a structurally recognized
   checked integer protocol;
3. every relevant access is an exact base-place access classified by one
   exhaustive storage-access census;
4. it has exactly one ordinary store in the callable;
5. the store has no write authorization or final-write authorization;
6. the stored value has the same exact MIR type;
7. the store dominates each load site through which a fact is propagated;
8. the storage is not exposed through an alias, call argument by reference,
   projection, attachment, ownership operation, or unclassified future role;
9. surviving lifetime operations are consistent with the recognized protocol
   and do not permit a read outside the stored lifetime; and
10. the carrier belongs to the same callable and verified seal as the graph.

Eligibility is deliberately protocol-owned. A generic `ScalarSpill` is not
constant merely because a scan happens to find one store. In particular, the
solver must not consume normalized former path-condition carriers or silently
become general storage propagation.

The analysis may represent the proof as a carrier certificate containing the
declaration, unique store site, source value, exact eligible load sites, and
type. It is transient and invalid after any rewrite.

### Structural protocol observations

Checked-integer discovery should be split into two layers:

- a structural observation proving the exact check/success/failure/join
  topology, carrier declarations, predecessor relationships, protected-root
  exclusions, private operand loads, result store, and result reload; and
- constant solving which supplies operand facts and classifies the operation
  as successful, statically failing, or unresolved.

The structural observation must not require an operand store's source
assignment to be a literal constant. It should expose the operand and result
carrier relationships needed by the graph while retaining all current
topology and ownership checks.

This separation also keeps future consumers from copying the checked topology
matcher merely to obtain carrier provenance.

### Structural logical observations and transfer

The graph should inventory every verified `MirLogicalExpression` together
with its matching path condition. The observation names the operation, left
result, right result, selected result, activation and result storage, split,
selection predecessors, right entry/exit, short block, and join. It remains an
immutable view of the current proof-rich seal.

When the left result becomes constant, the solver publishes a deterministic
logical selection immediately:

| Operation | Constant left | Selected path | Result fact |
|---|---:|---|---|
| `&&` | `false` | Short | `false`, without requiring a right fact |
| `&&` | `true` | Right | The right fact when one becomes available |
| `||` | `true` | Short | `true`, without requiring a right fact |
| `||` | `false` | Right | The right fact when one becomes available |

`SelectedRight` is useful even when the right result is not constant: it
authorizes control-flow selection while preserving the complete right-hand
evaluation. `SelectedShort` authorizes making the existing right-hand region
unreachable because language evaluation would never enter it.

Logical result storage deliberately does not receive a generic carrier
certificate. Its two alternative stores violate the unique-write carrier
rule, and their equivalence is proven by the logical record and selected path
rather than ordinary storage analysis. The solver publishes a fact directly
for the record's exact `selected_result` value. Nested logical records then
compose through ordinary value dependencies without recursive mutation or a
depth limit.

## Fact lattice and convergence

### Monotonic facts

Each eligible node starts `Unknown` and may transition at most once to
`Constant(PrimitiveConstant)`. Unsupported, conflicting, or insufficiently
proven nodes remain unknown; an implementation may retain an internal barrier
classification for explanations, but a barrier never becomes optimization
authority.

Each logical record separately starts `Unselected` and may transition at most
once to `SelectedShort(constant)` or `SelectedRight`. A selected-right record
does not imply that its result is constant; it merely fixes which executable
path must be retained. If its right-result value later becomes constant, the
logical selected-result value may then transition to that same constant.

The exact type is part of `PrimitiveConstant`. A derived constant is accepted
only when it equals the MIR result or storage type selected by verification.
Two derivations for one node are not expected in verified MIR. If graph
construction nevertheless creates inconsistent derivations, analysis returns
a structured internal failure rather than choosing one by traversal order.

### Worklist algorithm

The solver should:

1. build and validate the complete supported dependency graph;
2. seed supported literal values;
3. enqueue direct consumers whenever a node first becomes constant;
4. evaluate an eager consumer only when all of its required operands are
   constant;
5. publish an exact constant only for a supported primitive operation,
   certified carrier transfer, statically successful checked protocol, or
   selected logical result;
6. evaluate a logical producer as soon as its left operand is constant,
   without waiting for a right operand which the selected short path skips;
   and
7. stop when the worklist is empty.

This is a dataflow fixed point, not a pass-pipeline fixed point. It performs no
MIR mutation while solving, does not reverify intermediate programs, and does
not rerun a selected pass. Cycles with no constant seed simply remain unknown.

Every node acquires at most one constant fact, every logical record acquires at
most one selection, and each dependency edge needs to react only to newly
available operands. The intended complexity is linear in the supported nodes,
logical records, and edges apart from existing CFG/dominance queries. There is
no configurable depth, wave, fuel, or pass-occurrence limit.

### Completeness guarantee

For the closed supported domain, the solver must derive a node's constant if:

- the node is reachable through the constructed dependency graph from
  supported literal seeds;
- every producer on that dependency subgraph is supported and type-correct;
- every carrier edge has a valid carrier certificate;
- every semantically evaluated checked producer evaluates successfully; and
- every traversed logical producer has a constant left result and either
  selects its fixed short result or has a constant selected right result.

This guarantee applies regardless of dependency depth and must be tested with
generated or programmatically constructed chains much deeper than ordinary
source fixtures. It does not promise a fact when a dependency crosses an
excluded semantic boundary.

### Static checked failures

The exact checked evaluator has three semantic outcomes:

- `Success(constant)` publishes the checked result;
- `StaticFailure(reason)` publishes no result and records a retained failure
  observation; and
- `Unsupported` publishes no result.

A static failure is a hard propagation barrier along that result path. Earlier
independent or operand computations may still fold, but the check, success and
failure topology remains unchanged. The optimizer must not turn the failure
into a compile-time diagnostic, unconditional termination, or fabricated
constant.

A statically failing operation inside a logically skipped right-hand region is
not on the selected result path and therefore does not block a fixed short
result. The later logical CFG rewrite makes that region unreachable exactly as
unoptimized short-circuit evaluation would. If the right-hand path is selected,
the failure remains a barrier and executes at its original location.

## Consumers and rewrite plans

### One solution, separate selectable transformations

Three pass registrations remain independently selectable:

| Stable pass name | Solution consumed | Mutation owned |
|---|---|---|
| `primitive-constant-folding` | Constant facts for supported ordinary assignments | Replace eligible non-literal primitive rvalues with exact constants while preserving assignments |
| `checked-integer-constant-folding` | Successful constant facts plus structural observations for checked integer protocols | Rewrite complete eligible checked protocols through the existing checked transaction |
| `constant-short-circuit-folding` | Constant-left logical selections, optional selected-result facts, and exact logical proof records | Select the existing short or right CFG path while consuming logical proof during mandatory normalization |

Each selected occurrence builds a fresh solution from its own current verified
seal. The three passes do not share a cached object across an intervening rewrite
and do not call one another. This preserves independent selection and failure
attribution while giving all transformations depth-independent reasoning.

It is sound for primitive folding to consume a constant derived through a
checked protocol even when checked-protocol folding is disabled: the original
checked evaluation remains in place, and a fact is published only when that
protocol is proven to succeed. Operand evaluation and all unrelated effects
remain present.

It is likewise sound for checked folding to consume constants derived through
ordinary primitive assignments even when primitive folding is disabled: the
ordinary operations remain in place and compute the same exact values at
runtime.

Both proof-rich folding passes may consume logical selected-result facts even
when logical CFG folding is disabled. The verified logical protocol continues
to implement the same selection at runtime. Conversely, the logical pass may
select a path using a left fact derived from primitive or checked operations
without rewriting those producers.

### Primitive rewrite plan

The primitive consumer records every supported non-literal ordinary
assignment for which the solution supplies an exact constant. It replaces the
rvalue kind in place, preserving result `ValueId`, declared type, instruction
position, source span, block identity, and every use.

The plan may include assignments in different blocks when their facts cross
only certified carrier edges. It does not replace the checked result
assignment in isolation; complete checked topology belongs to the checked
consumer.

### Checked-protocol rewrite plan

The checked consumer records every structurally eligible protocol whose solver
outcome is successful. All such candidates are discovered against the same
immutable callable snapshot and applied through one atomic sparse callable
transaction.

Before mutation, the transaction revalidates the complete plan's local
identities, topology, carrier certificates, and solved constants against the
unchanged source snapshot. After that validation succeeds, deterministic
application may rewrite multiple dependent protocols without treating an
earlier edit in the same unpublished transaction as stale input for a later
candidate.

Protocol rewrites retain their current semantic granularity: preserve operand
evaluation, replace the checked success result with the exact constant,
remove only protocol-private operand-load values, redirect the checked
terminator to success, retain result storage/reload structure, and leave later
cleanup to independently selected passes.

This plan-level validation replaces the current assumption that every operand
carrier source is itself a constant assignment. It does not weaken the exact
shape checks or use output verification as candidate discovery.

### Logical transition rewrite plan

The logical consumer records every verified logical expression whose solver
selection is known. Its plan is validated against the immutable proof-rich
snapshot and is applied only as part of the proof-rich-to-final normalization
transaction.

For `false && rhs` and `true || rhs`, the plan:

1. redirects the logical split to the predecessor which stores the inactive
   path condition;
2. redirects the selection branch to the existing short block;
3. preserves evaluation of the left operand and the activation store and
   lifetime protocol needed by the selected path; and
4. replaces the exact protocol-owned selected-result load with the fixed
   boolean constant when the combined transaction can do so without changing
   its identity or span.

The right-hand region is then unreachable. The transition does not need a
purity proof for that region because the source semantics never evaluate it.
It does not eagerly delete that region; independently selectable post-proof
unreachable and dead-definition passes retain deletion ownership.

For `true && rhs` and `false || rhs`, the plan instead redirects the split to
the active predecessor and the selection branch to `right_entry`. It retains
the complete right-hand region and evaluates it exactly once. If the solver
also derives a constant for the right result, the exact selected-result load
may be replaced by that constant; otherwise only control flow is selected.

Nested logical plans are ordered by their stable proof-record positions and
validated together. A plan may rely on another record's solved semantic result
without relying on an earlier syntactic edit. All selected edits and mandatory
normalization are committed once, or no final MIR is published.

### Atomicity and seal lifetime

Solving and planning borrow one verified proof-rich product. An ordinary
proof-rich occurrence with no changes retains its seal. A changed primitive or
checked occurrence consumes the seal, performs at most one whole-program
rewrite commit, recompacts identities deterministically, and returns through
proof-rich verification.

The logical transition occurrence is different only because its successful
output cannot be proof-rich MIR: rewriting the logical shape invalidates the
proof record it consumes. It validates its complete optional plan first, then
performs the logical edits and mandatory proof normalization as one unpublished
transaction, and publishes only a verified final-MIR seal. If the logical pass
is absent or makes no selection, the same mandatory normalizer runs without
optional edits.

No graph, carrier certificate, protocol observation, instruction position, or
rewrite candidate survives either kind of commit. Each later occurrence
rebuilds its analysis from the newly sealed product.

## Pipeline composition and selection

The two existing stable pass names remain unchanged and the registry gains
`constant-short-circuit-folding`. The default pipeline may retain its current
proof-rich prefix and add one transition occurrence at the representation
boundary:

```text
dead-pure-definition-elimination
primitive-constant-folding
primitive-algebraic-simplification
primitive-constant-folding
checked-integer-constant-folding
dead-pure-definition-elimination
conservative-cfg-cleanup
dead-pure-definition-elimination
constant-short-circuit-folding
-- mandatory proof normalization, composed with the selected transition --
post-proof-unreachable-block-elimination
empty-block-forwarding
basic-block-merging
whole-world-reachability
```

The second primitive occurrence remains useful because algebraic
simplification can create a constant from a non-constant expression, such as
a reviewed annihilator or identity result. It is not an expression-depth
workaround. Each primitive or checked occurrence is independently complete
for its supported dependency graph as observed at that seal.

No additional primitive occurrence is required after checked folding. Before
the checked rewrite, the preceding primitive occurrence can already reason
through every statically successful checked protocol. Checked folding then
removes protocol overhead, and existing dead-pure/CFG cleanup consumes the
resulting structural redundancy.

### Proof-consuming transition stage

`MirPassStage` gains `ProofTransition` between `ProofRich` and `Final`.
Initially the scheduler accepts zero or one transition occurrence and rejects
repetition or placement outside that boundary. This is not a third generally
mutable MIR representation: the transition callback receives a narrow typed
capability which can inspect verified proof-rich MIR, submit an optional
logical selection plan, and invoke the one mandatory normalizer. It cannot
publish raw unnormalized or partially normalized MIR.

When the transition pass is selected, the pipeline emits its ordinary
occurrence report and a transition-labelled verified checkpoint, followed by
the established `after-proof-normalization` checkpoint for the same verified
final product. There is no observable checkpoint between optional logical
rewriting and proof normalization. With the pass disabled, only mandatory
normalization and its existing checkpoint remain. Core normalization metrics
remain distinct from optional logical-fold metrics.

Disabling any stable name removes only that transformation. Other proof-rich
passes may still use semantic facts about operations they do not rewrite.
Disabling `constant-short-circuit-folding` preserves every logical CFG while
mandatory normalization still consumes its proof metadata exactly as today.
The `none` profile runs no selectable occurrence, including no logical fold,
but still runs mandatory normalization and must preserve optimization-off
parity.

## Modular implementation ownership

The substantial new analysis should live behind one private recursive facade,
for example:

```text
passes/pipeline/optimizations/local_constant/
├── mod.rs
├── graph.rs
├── carrier.rs
├── logical.rs
├── solve.rs
└── tests.rs
```

The exact file split may follow implementation evidence, but responsibilities
must remain distinct:

- the facade exposes only immutable solution queries needed by sibling pass
  coordinators;
- graph construction owns dependency and reverse-dependency discovery;
- carrier analysis owns exhaustive storage access and certificate rules;
- the solver owns the lattice, worklist, termination, and completeness tests;
- primitive and checked evaluators remain the single owners of arithmetic
  semantics;
- checked-protocol discovery owns structural topology;
- logical observation owns the verified record-to-transfer relationship;
- primitive, checked, and logical pass modules own their respective rewrite plans,
  selection, measurements, and capability use; and
- the transition coordinator owns atomic composition of an optional logical
  plan with the mandatory normalizer; and
- generic MIR rewriting remains the only owner of sparse editing and dense
  commit.

Submodules remain private and are selectively re-exported through the
optimization facade. The solver must not gain driver, reporting, backend, CLI,
or filesystem dependencies.

The existing block-local `PrimitiveConstantFacts` should either become a thin
consumer of the shared solution or be retired once no distinct responsibility
remains. The implementation must not retain two independently evolving
constant-propagation engines.

## Identity, ordering, and observable semantics

- Analysis nodes are transient and never appear in MIR dumps, ABI, or public
  compiler APIs.
- Surviving `ValueId`, `StorageId`, and `BlockId` identities retain existing
  dense rewrite behavior.
- Primitive replacement preserves the original assignment and span.
- Checked replacement preserves the current checked-pass span contract and
  does not relocate operand evaluation.
- Logical selection preserves left evaluation, selects exactly one existing
  path, preserves selected-right evaluation exactly once, and preserves the
  selected-result identity and span when replacing its load.
- Candidate and metric order follows callable, block, instruction, and operand
  order, never hash iteration or worklist accident.
- Solver queue order may affect evaluation timing inside the compiler but not
  the final fact set, plan order, dump, measurement, or emitted program.
- Static failure reasons and runtime trace locations remain unchanged.
- No new externally observable allocation, destruction, I/O, call, or storage
  event is introduced or removed by the solver itself.

## Observation and diagnostics

Existing transformation measurements remain authoritative: primitive fold
families, checked quotient/remainder/shift folds, removed protocol loads, and
retained static failures. Structural commit statistics continue to own total
entity changes.

Add only stable measurements that explain the new capability, preferably:

- primitive folds whose proof crossed at least one certified carrier;
- checked protocols whose operands required propagated rather than direct
  literal facts;
- constant-left `&&` and `||` selections split by short versus right path;
- logical selected-result loads replaced by constants; and
- the maximum dependency depth among materialized folds, if it can be defined
  independently of worklist order.

Mandatory proof-normalization statistics remain separate. A logical fold may
make blocks unreachable, but later cleanup reports their deletion; the logical
pass reports only the path selection and direct selected-result replacement it
owns.

Do not report queue pops, internal waves, hash sizes, or solver iteration
counts as semantic pass metrics. They are implementation details and would
make a correct algorithmic refactor appear to change optimization behavior.

Unsupported or ineligible nodes are ordinary conservative outcomes. Invalid
identities, inconsistent types, contradictory derivations, stale plans, or a
broken supposedly canonical protocol are structured internal pass failures
attributed through the existing occurrence boundary.

Verified before/after checkpoints remain the detailed observation mechanism.
A deterministic compiler-test-only fact dump may be added if unit queries are
insufficient, but ordinary reports must not embed the complete graph.

Logical-plan validation or application failures are attributed to the
`constant-short-circuit-folding` occurrence. Failures in the unchanged core
normalization rules remain attributed to mandatory proof normalization. Final
seal verification failure identifies the combined transition transaction and
publishes no partial program.

## Verification and test strategy

### Graph and carrier tests

- literal, primitive, carrier-store, carrier-load, and checked-result edges;
- exact logical-record inventory and left, right, selected-result, path, and
  block relationships;
- exhaustive storage access-role classification;
- unique-write and exact-type requirements;
- same-block instruction order and cross-block dominance;
- read-before-store, multiple-store, projection, alias, authorization,
  attachment, lifetime, and unclassified-role rejection;
- protocol-owned `ScalarSpill` acceptance and unrelated spill rejection;
- malformed local identities reported as structured errors; and
- deterministic graph construction independent of map implementation.

### Solver tests

- straight-line primitive chains at depths 0, 1, and well beyond ordinary
  source nesting;
- alternating primitive, carrier, division/remainder, and shift chains;
- multiple independent inner protocols feeding one outer operation;
- fan-out and fan-in dependencies;
- cycles with and without usable constant seeds;
- unsupported leaves and partial constants remaining unknown;
- exact `i64`, `u64`, and `u8` wrapping/canonicalization boundaries;
- statically failing division, remainder, and shift barriers;
- all four constant-left logical selections, including a skipped unsupported
  or statically failing right operand and a selected dynamic right operand;
- nested logical expressions feeding primitive and checked producers and one
  another at depths beyond ordinary source nesting;
- a proof that every node transitions at most once;
- a result independent of deterministic worklist seed order in test-only
  permutations; and
- no Rust call-stack dependence on expression depth.

### Rewrite-consumer tests

- `((8 / 2) + (7 % 3)) / 2` becomes one final constant result while all three
  successful checked protocols are eligible in one checked occurrence;
- `(1 << 2u) << 1u` folds both checked protocols without a fixed-depth limit;
- generated alternating expressions at large depths fold completely;
- primitive-only selection folds ordinary nodes while retaining checked CFG;
- checked-only selection folds checked protocols while retaining ordinary
  operand computation;
- logical-only selection chooses the exact short or right path while retaining
  primitive and checked producer syntax;
- `false && rhs` and `true || rhs` make an effectful or failing RHS unreachable
  without evaluating or requiring a fact for it;
- `true && rhs` and `false || rhs` preserve every RHS effect and failure exactly
  once;
- disabling all three stable names preserves the unfurled expression;
- a failing inner checked operation prevents propagation through that result
  while preserving exact failure behavior;
- independent foldable work before or beside a failure still folds;
- spans, result values, evaluation order, carrier lifetime, dense identities,
  and protocol-private load deletion remain exact;
- multiple dependent checked candidates apply atomically and deterministically;
- multiple nested logical selections compose atomically with mandatory
  normalization and publish no proof-invalid intermediate MIR;
- a stale or inconsistent plan publishes no partial MIR; and
- a second identical selected occurrence is idempotent.

### Pipeline and source-to-native tests

- the two existing pass names remain stable and the registry lists
  `constant-short-circuit-folding` at `ProofTransition`;
- stage validation accepts zero or one boundary transition occurrence and
  rejects repeats or misplaced transition/final/proof-rich passes;
- occurrence numbering, transition and normalization checkpoint labels, and
  failure attribution are deterministic;
- the default schedule contains no new repetition justified by nesting depth;
- `none`, explicit logical exclusion, and all-disabled proof-rich/final MIR
  parity, with mandatory normalization still present;
- debug and release compiler builds produce identical folding decisions;
- repeated independent processes produce identical MIR and measurements;
- native stdout, stderr, exit status, panic text/location, and runtime trace
  match optimization-off behavior for success, selected-right failure, and
  skipped-right failure/effect cases; and
- full compiler, CLI, golden, native, documentation, formatting, lint, and
  supported-toolchain gates pass.

## Frozen decision register

| Decision | Question | Frozen decision | Status |
|---|---|---|---|
| [CLP1](#clp1--make-the-supported-domain-expression-complete) | What depth is supported? | Every finite supported dependency graph, with no configured depth or wave limit | **Frozen** |
| [CLP2](#clp2--solve-facts-before-mutating-mir) | How is convergence reached? | One monotonic callable-local worklist analysis followed by a rewrite plan | **Frozen** |
| [CLP3](#clp3--keep-transformations-independently-selectable) | What is registered? | Retain primitive and checked names and add a separate constant-left logical pass | **Frozen** |
| [CLP4](#clp4--share-semantic-facts-across-pass-boundaries-not-seals) | How do the passes cooperate? | Rebuild the same analysis kind per occurrence; never cache facts across a rewrite seal | **Frozen** |
| [CLP5](#clp5--separate-protocol-shape-from-constant-provenance) | Who recognizes checked topology? | A structural observer independent of constant sources, consumed by the solver and rewriter | **Frozen** |
| [CLP6](#clp6--certify-only-protocol-owned-private-scalar-carriers) | Which storage propagates constants? | Exact checked-protocol `ScalarSpill` carriers satisfying exhaustive access and dominance rules | **Frozen** |
| [CLP7](#clp7--reuse-the-exact-existing-evaluators) | Who owns arithmetic? | Existing primitive and checked-integer evaluators remain authoritative | **Frozen** |
| [CLP8](#clp8--treat-static-failure-as-a-result-barrier) | How are failing checks handled? | Publish no result fact and retain the original runtime protocol and failure | **Frozen** |
| [CLP9](#clp9--materialize-each-semantic-family-separately) | What does each pass rewrite? | Primitive assignments, checked protocols, and logical selection each retain a separate owner | **Frozen** |
| [CLP10](#clp10--validate-one-complete-plan-before-atomic-mutation) | How are dependent checked candidates committed? | Revalidate the full source-snapshot plan, then apply it in one unpublished transaction | **Frozen** |
| [CLP11](#clp11--retain-the-proof-schedule-and-add-one-transition-occurrence) | Does scheduling express depth? | No; retain proof order, then run at most one logical transition at normalization | **Frozen** |
| [CLP12](#clp12--add-no-persistent-mir-provenance) | Does MIR representation change? | No new executable IR, storage kind, persistent node, or cross-seal identity | **Frozen** |
| [CLP13](#clp13--keep-the-solver-private-and-facade-oriented) | Where does implementation live? | One private recursive analysis module with narrow immutable queries for three consumers | **Frozen** |
| [CLP14](#clp14--observe-materialized-capability-not-worklist-mechanics) | What is reported? | Stable transformation/provenance counts, not internal iterations | **Frozen** |
| [CLP15](#clp15--make-no-language-or-runtime-contract-change) | Which semantic assumptions change? | None; whole-world and single-threaded guarantees do not weaken barriers | **Frozen** |
| [CLP16](#clp16--support-all-four-constant-left-short-circuit-rules) | Which logical identities are supported? | Select short or right from a constant left operand without requiring RHS purity | **Frozen** |
| [CLP17](#clp17--introduce-a-narrow-proof-consuming-transition-stage) | Where can logical CFG change safely? | Between proof-rich and final seals through a typed single-occurrence transition | **Frozen** |
| [CLP18](#clp18--compose-logical-rewriting-atomically-with-mandatory-normalization) | How is proof metadata consumed? | Validate once and publish only the combined verified final-MIR result | **Frozen** |

## CLP1 — Make the supported domain expression complete

Guarantee constant discovery for every finite dependency graph composed only
of supported primitive operations, certified checked-protocol carriers, and
successful checked integer operations, plus verified constant-selected logical
relations. Do not encode a maximum nesting depth, number of propagation waves,
or number of selected pass occurrences.

**Rationale:** source nesting depth is not a semantic boundary. Once an
operation family is admitted, truncating it at an implementation-selected
depth is surprising and makes optimization results depend on schedule length.

## CLP2 — Solve facts before mutating MIR

Build the complete graph, compute its monotonic solution with a dependency
worklist, and only then construct a mutation plan. Do not discover one wave,
commit it, reverify, and rerun until unchanged.

**Rationale:** analysis convergence is cheap, local, and independent of dense
identity rewriting. Repeated whole-program commits would add avoidable
verification cost and turn an analysis property into pipeline policy.

## CLP3 — Keep transformations independently selectable

Retain `primitive-constant-folding` and
`checked-integer-constant-folding`, and add
`constant-short-circuit-folding`. All may reason through the complete
supported graph, but each materializes only its own transformation family.

**Rationale:** primitive replacement, checked-CFG rewriting, and logical path
selection have different risk, stages, measurements, and debugging value. A
monolithic replacement pass would weaken selection without being necessary for
complete facts.

## CLP4 — Share semantic facts across pass boundaries, not seals

Define one reusable solution API, but rebuild it in every occurrence that
needs it. Never retain it after an atomic rewrite or expose a global analysis
cache.

**Rationale:** the current seal invalidation rule is simple and trustworthy.
The graph is linear-sized and cheap enough to recompute until broader analysis
cost provides evidence for a cache manager.

## CLP5 — Separate protocol shape from constant provenance

Make checked-protocol discovery report exact structural relationships without
requiring literal operands. Let the constant solver decide which structurally
valid protocols have constant, successful inputs.

**Rationale:** topology and value provenance are reusable independent facts.
Keeping them fused caused the current nested-expression boundary.

## CLP6 — Certify only protocol-owned private scalar carriers

Propagate through `ScalarSpill` only when it participates in a recognized
checked protocol and satisfies the full carrier-certificate rules. Reject
generic spills and every ambiguous access.

**Rationale:** this gives the required cross-block bridge without claiming
general load/store or alias analysis. Single-threaded execution alone is not
enough to make arbitrary storage constant.

## CLP7 — Reuse the exact existing evaluators

Keep `PrimitiveConstant`, primitive rvalue evaluation, floor division,
divisor-sign remainder, shift widths/directions, and byte canonicalization in
their existing semantic owners. The solver supplies operands and consumes
their closed outcomes.

**Rationale:** dataflow must not become a second arithmetic specification.

## CLP8 — Treat static failure as a result barrier

Do not publish a result for division/remainder by zero, an invalid shift
count, or any other evaluator-reported static failure. Retain the original
runtime protocol and allow independent prior computations to fold.

**Rationale:** folding successful expressions does not authorize changing
failure timing, diagnostics, panic attribution, or trace behavior.

## CLP9 — Materialize each semantic family separately

The primitive pass replaces eligible ordinary rvalues in place. The checked
pass rewrites complete checked protocols. The logical pass selects complete
short-circuit protocols during normalization. None deletes evaluated operand
producers or takes ownership of downstream cleanup.

**Rationale:** this preserves existing atomic semantic boundaries and keeps
dead work, CFG, and whole-world retention independently selectable.

## CLP10 — Validate one complete plan before atomic mutation

For multiple dependent checked candidates, validate every candidate and its
carrier certificate against the original verified snapshot before applying
any edit. Apply the validated non-conflicting plan in stable order through one
callable transaction and one dense commit.

**Rationale:** later candidates may depend semantically on earlier candidates
without needing their already-mutated syntax. Plan-level validation prevents
the first unpublished edit from making valid later snapshot evidence appear
stale.

## CLP11 — Retain the proof schedule and add one transition occurrence

Do not add repeated occurrences to accommodate expression depth. Keep the
second primitive occurrence only because algebraic simplification is a
separate transformation capable of exposing new constants. Add the logical
pass once at the proof-normalization boundary.

**Rationale:** pass scheduling should describe semantic composition, not serve
as fuel for an incomplete local analysis.

## CLP12 — Add no persistent MIR provenance

Represent graph nodes, carrier certificates, protocol outcomes, and logical
selections only as seal-local analysis data. Do not add a new MIR storage kind,
retained source provenance, or normalized logical record.

**Rationale:** the existing MIR and dense editor can express every resulting
program. Persistent provenance would enlarge verification and normalization
contracts without an executable need.

## CLP13 — Keep the solver private and facade oriented

Place graph construction, carrier certification, logical observation, and
solving under one private recursive module. Expose narrow immutable solution
queries to the three pass coordinators and keep arithmetic, protocol rewrite,
normalization, and dense commit in their current owners.

**Rationale:** this creates one discoverable analysis owner without growing an
oversized pass file or widening compiler APIs prematurely.

## CLP14 — Observe materialized capability, not worklist mechanics

Retain existing fold and rewrite measurements and add only stable counts that
show carrier-enabled materialized results. Do not expose iterations or queue
operations as optimization behavior.

**Rationale:** traversal strategy may change without changing the solved facts
or optimized program.

## CLP15 — Make no language or runtime contract change

Preserve all current semantics and treat whole-world/single-threaded execution
only as environmental guarantees. Do not infer immutable memory, absent
aliases, unobservable failures, or movable destruction from them.

**Rationale:** the complete supported graph can be proven using compiler-owned
private carriers and exact operations without weakening Skald's language.

## CLP16 — Support all four constant-left short-circuit rules

Support `false && rhs -> false`, `true && rhs -> rhs`,
`true || rhs -> true`, and `false || rhs -> rhs` whenever the left result is
solved as a boolean constant. Never require the right region to be pure merely
to select the path. Preserve left evaluation and preserve selected-right
evaluation exactly once.

**Rationale:** these are control-flow selection rules, not eager boolean
algebra. Their safety follows from the verified short-circuit protocol and the
constant left result; requiring RHS purity would reject valid effectful and
failing examples which the language already conditionally evaluates.

## CLP17 — Introduce a narrow proof-consuming transition stage

Add `ProofTransition` between proof-rich and final pass stages, initially
allowing only zero or one occurrence of `constant-short-circuit-folding` at
that boundary. Give it a typed capability to consume verified logical records
and invoke normalization, not general access to publish a third MIR form.

**Rationale:** proof-rich verification requires the exact logical record and
shape, while normalized MIR no longer retains enough semantic provenance to
rediscover the relationship safely. A narrow transition expresses the true
ownership boundary without turning normalization into an unselectable
optimizer.

## CLP18 — Compose logical rewriting atomically with mandatory normalization

Validate all selected logical plans against one proof-rich snapshot, apply
them together with mandatory proof normalization in one unpublished rewrite,
and publish only verified final MIR. Run the unchanged mandatory normalizer
when the selectable logical pass is absent.

**Rationale:** no intermediate representation can honestly be verified as
proof-rich after its proof shape changes. Atomic composition avoids persistent
logical provenance and partial publication while preserving optimization-off
and `none` behavior.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A carrier is treated as immutable despite another access | Exhaustive access census, unique store, exact base place, protocol ownership, dominance, and unknown-role rejection |
| Queue order changes optimized output | Monotonic single-assignment facts, deterministic plan sorting, and permuted-worklist tests |
| A deep expression overflows the compiler stack | Iterative graph construction/worklist processing for dependency traversal and generated deep-chain tests |
| A checked failure is optimized through | Publish result facts only for exact evaluator success; retain and test static failures |
| A skipped failing or effectful RHS blocks a fixed short result | Make logical dependencies conditional on the selected path and test unsupported, effectful, and failing RHS regions |
| Logical selection drops or duplicates evaluated work | Preserve left evaluation and select the existing right region exactly once; verify source-to-native effects, failures, and traces |
| Dependent checked rewrites invalidate later candidates | Validate the complete plan before mutation and apply it within one unpublished transaction |
| Primitive, checked, and logical passes drift semantically | Shared solution, existing arithmetic evaluators, and one logical transfer owner |
| Shared analysis undermines independent selection | Each pass builds facts independently and materializes only its own rewrite family |
| Logical CFG is rewritten before or after its proof is authoritative | Permit it only in the typed transition and compose it atomically with normalization |
| Optional optimization changes mandatory normalization policy | Keep core normalization rules and metrics unchanged; supply only a separately selected, separately reported logical plan |
| Rebuilding the solution costs compile time | Linear dependency work, no intermediate verification, and later measurement before considering seal-scoped caching |
| The solver expands into unsound general memory propagation | Accept only structurally recognized checked-protocol carriers and reject generic storage |
| Existing fixed schedule becomes mistaken for a depth bound | Explicit completeness tests and documentation distinguish algebraic repetition from solver convergence |

## Alternatives considered

### Add only the narrow carrier-provenance substitution

This would bridge one lowered store/load boundary but would leave propagation
and checked-protocol discovery organized as successive snapshots. It would
solve the current fixture without defining why arbitrary alternating depth is
complete. The broader solver subsumes the safe carrier proof while giving it a
clear consumer and convergence model.

### Repeat the whole pass until it reports unchanged

This eventually handles some nests but performs repeated mutation, dense
commit, complete proof verification, reachability rebuilding, measurements,
and checkpoints. It also cannot help while the rewritten checked result still
crosses an unrecognized carrier. Pass repetition is rejected as the expression
depth mechanism.

### Add a generic pipeline fixed-point group

This would be useful only if several unrelated transformation families need a
shared convergence policy. For this problem it needlessly changes occurrence,
selection, disabling, inspection, metrics, verification, and failure
contracts. A local monotonic solver is the smaller and more precise owner.

### Replace all consumers with one monolithic constant pass

One pass could solve and materialize everything, but users would lose the
ability to isolate ordinary rvalue replacement, checked CFG rewriting, and
logical selection. Shared analysis does not require shared mutation authority,
so separate registrations remain preferable.

### Mutate while walking the expression or CFG

Recursive or instruction-order mutation makes correctness depend on traversal
order, complicates dependent protocol rewrites, and risks deep Rust recursion.
An immutable graph followed by a stable plan separates proof from mutation.

### Introduce scalar SSA or full SCCP

SSA would make broader cross-block constant propagation natural, but requires
promotion, phi/block arguments, alias and lifetime decisions, verifier and
backend changes, and a substantially larger roadmap. The checked-protocol
carrier subset can be solved exactly with current MIR.

### Add persistent protocol provenance to MIR storage

Persistent provenance may later be justified for normalized final-stage
storage transformations. This pass operates before proof normalization and can
derive exact protocol ownership structurally from the current seal, so changing
the MIR storage model now would be unnecessary coupling.

### Rewrite logical CFG in an ordinary proof-rich pass

Changing logical edges while retaining their exact proof record creates a
shape the proof-rich verifier must reject. Deleting or weakening the record
before its final semantic consumer would erase required ownership and lifetime
evidence. The proof-consuming transition is the first honest rewrite point.

### Rediscover short-circuit relationships after normalization

Normalized path-condition loads and ordinary branches do not uniquely encode
the source logical protocol. Shape matching there would either be fragile or
require new retained provenance. Consuming the verified record at the boundary
is more exact and leaves final MIR simpler.

### Fold logical expressions unconditionally in mandatory normalization

This would be mechanically convenient but would make an optimization part of
every profile, including `none`, and would erase independent selection and
measurement. The mandatory normalizer may execute an optional validated plan,
but creation of that plan remains a selectable pass decision.

## Effort and likely delivery shape

Overall effort is **large**, although no individual piece requires a new IR.
The largest correctness work is exhaustive carrier access classification,
dependency-complete solving, atomic validation of multiple dependent checked
rewrites, and the typed proof-to-final transition.

| Delivery concern | Relative effort | Durable result |
|---|---|---|
| Structural protocol/value-provenance separation | Medium | Reusable checked topology independent of literals |
| Logical observation and conditional transfer | Medium | Exact arbitrary-depth short-circuit facts without eager RHS requirements |
| Exhaustive storage access census and carrier certificate | Medium to large | Sound narrow storage bridge |
| Dependency graph and monotonic worklist solver | Large | Arbitrary-depth constant facts with explicit completeness |
| Primitive pass migration | Medium | Ordinary folds consume shared facts without duplicate engines |
| Checked plan and transaction migration | Large | Multiple dependent protocols fold from one snapshot |
| Proof-transition stage and normalizer composition | Large | Selectable proof-aware logical CFG rewriting without retained provenance |
| Logical plan and transaction | Large | All four constant-left rules with exact evaluation behavior |
| Selection, measurements, and schedule proof | Medium | Stable modular user-facing behavior |
| Deep, failure, determinism, native, and regression coverage | Medium to large | Evidence for completeness and semantic parity |

The implementation roadmap settles contracts and test fixtures before
restructuring current pass owners, then delivers structural protocol facts,
carrier certification, logical observations, the solver, proof-rich consumers,
dependent checked transactions, the transition capability and logical
consumer, pipeline observation, and broad hardening in that order.

## Freeze and promotion

CLP1 through CLP18 are frozen as one bundle. The completeness guarantee,
narrow carrier boundary, static-failure behavior, all four constant-left
rules, separate selectable consumers, proof-consuming transition, plan-level
atomicity, and rejection of pipeline-level repetition jointly define what
“proper” constant propagation means.

The durable direction is promoted into the compiler phase, driver, reporting,
and testing contracts. The optimization catalog records the work as
**Implemented**, the completed roadmap records delivery, and the archived
discoveries record preserves resolved implementation evidence.

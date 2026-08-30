# Target-Independent Whole-World Reachability Design Proposal

Status: proposed; WWR1 through WWR12 require confirmation before an
implementation roadmap is created.

This proposal moves semantic reachability ahead of target lowering and makes
it reusable whole-program compiler infrastructure rather than logic hidden in
one pruning pass. It builds on Skald's verified final-MIR seal, monotone static-
lifecycle certificate, dense callable-local rewrite transactions, and
selectable final-MIR pass pipeline.

The first production client will conservatively remove unreachable executable
definitions. The larger objective is a deterministic program-dependency and
reachability boundary that later devirtualization, inlining, effect analysis,
specialization, metadata pruning, call-graph inspection, and compile-time
planning can query without rebuilding subtly different notions of possible
targets.

The language contract does not change. Every program remains permanently
compiled as one closed world, and the resulting program remains single
threaded. Evaluation order, checked failure, panic behavior, allocation,
deterministic destruction, aliasing, ownership, and mutable access through
shared pointees retain their current meaning. Reachability may remove compiler
work and executable artifacts, but it may not change source acceptance,
diagnostics, or observable startup, execution, and shutdown behavior.

## Intended outcome

The design should provide:

- one target-independent inventory of executable dependency nodes and typed
  edges over final MIR;
- an explicit, independently testable whole-program root contract;
- deterministic possible-target expansion for direct, static, virtual,
  interface, indirect, ownership, optional, array, copy, and destruction work;
- immutable reachability facts bound to one verified final-MIR product;
- concise queries for reachable callables, possible callees, recursion,
  dispatch use, and runtime-materialized semantic entities;
- one exhaustive maintenance point when MIR gains a new executable operation;
- a clear distinction between dense semantic declarations and the executable
  definitions retained for the current final program;
- stable program-level identities when executable definitions are removed;
- one atomic program-retention capability separate from callable-local dense
  rewriting;
- central verification that independently proves every reachable executable
  target still has a valid definition;
- backend consumption of the verified retained domain before target legality,
  layout, frame planning, instruction selection, and trace planning;
- continued target-private pruning for artifacts that exist only after target
  lowering;
- deterministic dumps and structured measurements without pass-owned logging;
  and
- one selectable `whole-world-reachability` pass that initially prunes only
  executable definitions, leaving broader declaration and metadata compaction
  to later clients of the same facts.

## Current architecture and evidence

Skald already has the right phase location. Static lifecycle synthesis creates
final `MirProgram`; the selectable pass runner verifies it, runs target-
independent passes, immediately reverifies every changed result, and hands
only `VerifiedFinalMirProgram` to the backend. The implemented dead-pure-
definition elimination pass proves callable-local deletion and dense commit,
but the pass capability currently offers only a callback over each
`MirCallableEdit`. It cannot remove a whole function or member definition.

`MirProgram` deliberately separates declarations and definitions:

- function declarations are dense by `FunctionId`;
- function definitions use sparse slots aligned with those declarations;
- member declarations remain inside dense class-owned tables;
- member definitions use a `BTreeMap<CallableId, _>`; and
- static initializer bodies are owned by the lifecycle coordinator.

This is already close to the desired retained representation. The awkward
part is contractual rather than physical: current final verification requires
every internal function, initializer, user copy operation, and destructor to
have a definition, and interface verification requires every conformance
implementation to have one. Backend dispatch planning similarly assumes that
every selected declaration has an executable body even if no reachable call
or runtime operation can select it.

Removing dense declarations would force a global identity rewrite through
types, classes, fields, conformances, virtual families, lifecycle authority,
function types, bodies, diagnostics, and dumps. That is unnecessary for the
first useful result. The safer boundary is to keep semantic identities and
declarations stable, permit unreachable definitions to be absent from
verified optimized final MIR, and make reachability-aware verification and
backend planning distinguish a declared operation from a retained executable
operation.

Skald also already contains most dependency semantics, but under narrower
owners:

- static-effect extraction expands direct, virtual, interface, and indirect
  calls and follows implicit copy, assignment, array, ownership, and
  destruction operations;
- function-value analysis inventories callable-address operations by exact
  function type;
- virtual-family and interface-conformance tables describe the closed set of
  dynamic method implementations;
- lifecycle synthesis owns explicit static initialization and reverse-
  shutdown regions; and
- the backend walks target-generated symbol references from exported assembly
  roots after every MIR definition has already been lowered.

Reachability must consolidate the reusable target-resolution responsibility
without making static-effect evidence or backend symbols its abstraction. A
static-field effect graph, a possible-callee graph, and a machine-symbol graph
answer different questions even where they share edge discovery.

## Relationship to completed foundations

The proposal depends on three completed designs:

- the
  [static-lifecycle certificate](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md)
  allows effect-removing final-MIR transformations to realize a subset of
  immutable baseline lifecycle authority;
- the
  [dense callable-local MIR rewriting boundary](../archive/DENSE_MIR_IDENTITY_REWRITING_DESIGN_PROPOSAL.md)
  provides safe local editing and deterministic recompaction without changing
  program-level semantic identities; and
- the
  [selectable final-MIR optimization pipeline](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md)
  provides typed registration, deterministic scheduling, verified ownership
  transfer, pass inspection, and structured accounting.

Whole-world reachability uses those owners rather than replacing them. It adds
a program-level analysis and retention boundary alongside callable-local
rewriting. Any changed product still returns through ordinary final-MIR and
static-lifecycle realization verification before another pass or backend may
observe it.

## Comparison with Niflheim

Niflheim runs semantic unreachable pruning at the end of its default semantic
optimization sequence. Its reachability analysis starts at `main`, walks
functions, classes, interfaces, methods, types, static fields, constructors,
and expression-level dispatch, then reconstructs modules containing only the
reachable declarations. The analysis result is separate enough to be tested,
and running the pruning pass after folding, propagation, devirtualization, and
dead-statement cleanup lets earlier transformations remove edges first.

Useful lessons for Skald are:

- reachability belongs before backend lowering;
- roots and traversal should use canonical semantic identities;
- possible dispatch targets must be part of the analysis contract;
- analysis should be independently callable rather than inseparable from
  filtering; and
- reachability is most effective late in a schedule and may legitimately
  occur more than once as other passes remove edges.

Skald should not copy Niflheim's monolithic semantic walker or module
reconstruction. Skald's final MIR has explicit ownership and lifecycle work,
verified static startup and shutdown, capture-free function values, dense
program metadata, and a backend seal. Skald also needs possible-target queries
for future passes, not only four sets consumed immediately by one filter.
Passes must return measurements rather than log, and central final-MIR
verification must independently reject a retained program with a missing
reachable definition.

## Fixed assumptions and non-goals

- Every compilation is a closed world. There are no unknown future Skald
  definitions, dynamic modules, or separately compiled extensions.
- The generated program is single threaded. No source thread, signal handler,
  or asynchronous source callback introduces an additional semantic root.
  Compiler implementation parallelism remains allowed if results are
  deterministic.
- The current entry function is the only source-defined exported entry point.
  External function declarations are imports, not roots for internal Skald
  definitions.
- Current verification forbids transporting function values through external
  calls. The initial indirect-call model therefore has no opaque external
  source of internal callable addresses.
- All program-level static activation and reverse shutdown remain observable
  program behavior. An unread static is not dead merely because no ordinary
  callable reads it.
- Initial implementation does not change eager static initialization, export
  policy, dispatch ABI, object layout, runtime type identity, or language
  visibility.
- Initial pruning removes executable definitions only. It does not renumber or
  delete modules, declarations, classes, interfaces, fields, static fields,
  function types, array types, optional types, optional-box types, literal IDs,
  virtual-family IDs, or lifecycle authority.
- The design does not add local CFG reachability, SCCP, devirtualization,
  inlining, effect inference, alias analysis, escape analysis, or SSA.
- The design does not make a global persistent analysis manager. Facts belong
  to one verified MIR product and are discarded on transformation.
- Target-generated helper and data reachability remains a backend
  responsibility.
- Optimization-off mode retains its exact MIR and source-diagnostic behavior.
  Computing verification-owned reachability facts does not itself prune the
  program.
- No dynamic optimization plugin ABI or serialized reachability format is
  introduced.

## Design principles

### Separate discovery, closure, and retention

Dependency extraction describes possible execution. Root collection describes
what the program contract requires. Closure computes facts. Retention consumes
facts through a narrow ownership boundary. Keeping these responsibilities
separate lets later passes query the analysis without also deleting anything.

### Make executable edges explicit

Future passes should not reinterpret every `MirInstruction` independently to
answer which callables may run. A new executable MIR operation must encounter
one exhaustive extractor and focused tests.

### Preserve semantic identity, remove physical work

Unreachable declarations may remain as semantic metadata while their bodies
are absent. Stable IDs are more valuable than compacting global tables during
the first reachability implementation. Backend work should be driven by the
verified retained domain, not by declaration-table length.

### Verify the semantic consequence, not the pass's claim

The pass may propose removing definitions. Central verification recomputes
roots and closure from the resulting program metadata and retained bodies and
rejects every missing reachable executable target. A pass-generated reachable
set is never accepted as proof merely because it came from the registered
pass.

### Keep over-approximation visible

Conservative target expansion is safe but may retain excess work. The analysis
should expose edge kinds and target sets so later precision improvements can be
measured and reviewed without changing what “reachable” means.

### Reuse mechanisms, not unrelated evidence

Static-effect analysis and reachability should share exhaustive target and
implicit-lifecycle dependency extraction. Reachability should not retain
static-access witnesses or phases it does not need, and static-lifecycle
authority should not become a general call-graph cache.

## Vocabulary and invariant

This proposal uses these terms:

- **execution node:** a callable or compiler-defined implicit lifecycle
  operation that can cause executable work;
- **dependency edge:** one conservative may-execute or must-retain relation
  from one execution node to another;
- **root:** an execution obligation imposed directly by the whole-program
  contract;
- **reachable node:** a node in the transitive closure of the roots;
- **reachable callable:** a callable execution node in that closure;
- **possible target set:** the deterministic closed-world callable set for one
  call or lifecycle operation;
- **retained definition:** an executable body physically present in optimized
  final MIR;
- **declared callable:** a stable semantic identity and signature, whether or
  not an unreachable optimized body remains;
- **runtime entity:** semantic metadata or data required to lower reachable
  work, such as a class dispatch family, array lifecycle description, static
  slot, function type, or literal backing; and
- **target artifact:** a symbol, helper, table, trace record, panic message, or
  data object introduced or finalized during backend lowering.

Let `roots(P)` be the roots derived from final MIR program `P`, `edges(P, n)`
the conservative dependencies of execution node `n`, `closure(P)` their least
fixed point, and `definitions(P)` the set of executable bodies physically
present in `P`.

Final-MIR verification establishes:

```text
roots(P) subset-of closure(P)

for every edge source reachable in P:
    every semantically possible executable target is represented in closure(P)

reachable_callables(P) subset-of definitions(P)

every definition physically present in P:
    has a matching declaration and independently valid body

static_effects(P, lifecycle_root)
    subset-of baseline_lifecycle_authority[lifecycle_root]
```

`definitions(P)` may be a strict superset of `reachable_callables(P)`. This is
what permits optimization-off final MIR and intermediate schedules to retain
dead definitions safely. The reachability pass makes those sets equal for the
definition kinds it owns, subject to explicitly documented conservative
retention.

## Decision register

| Decision | Question | Proposed decision | Status |
|---|---|---|---|
| [WWR1](#wwr1--introduce-one-reusable-final-mir-reachability-product) | What is the durable product? | Immutable seal-scoped graph and reachability facts, separate from pruning | **Proposed** |
| [WWR2](#wwr2--separate-root-policy-from-dependency-extraction) | Who defines liveness? | Explicit target-independent root collector plus exhaustive dependency extractor | **Proposed** |
| [WWR3](#wwr3--model-callables-and-implicit-lifecycle-work-as-execution-nodes) | What participates in closure? | Typed execution nodes for callables and implicit class/array lifecycle work | **Proposed** |
| [WWR4](#wwr4--use-entry-and-complete-static-lifecycle-as-initial-roots) | What are today's roots? | Entry plus all coordinator activation and shutdown obligations | **Proposed** |
| [WWR5](#wwr5--centralize-conservative-possible-target-expansion) | How are dynamic targets found? | Shared deterministic direct, virtual, interface, indirect, and lifecycle expansion | **Proposed** |
| [WWR6](#wwr6--bind-analysis-facts-to-the-verified-final-mir-seal) | How long do facts live? | Compute during final verification, expose read-only, invalidate on every change | **Proposed** |
| [WWR7](#wwr7--keep-global-identities-stable-and-permit-sparse-executable-definitions) | What is physically removed? | Definitions only; declarations and global IDs remain stable initially | **Proposed** |
| [WWR8](#wwr8--add-a-narrow-atomic-program-retention-capability) | How may a pass prune? | A separate capability filters complete definition containers atomically | **Proposed** |
| [WWR9](#wwr9--make-final-verification-prove-reachable-definition-completeness) | How is pruning checked? | Recompute closure and require bodies for every reachable internal target | **Proposed** |
| [WWR10](#wwr10--make-backend-planning-consume-the-verified-retained-domain) | How does backend work shrink safely? | Lower retained definitions and only required dispatch/runtime metadata | **Proposed** |
| [WWR11](#wwr11--keep-machine-artifact-retention-as-a-target-safety-net) | Does backend pruning remain? | Yes; it owns dependencies introduced after MIR | **Proposed** |
| [WWR12](#wwr12--ship-one-conservative-selectable-pruning-client) | How is the foundation proven? | Register, observe, harden, then enable definition-only whole-world pruning | **Proposed** |

## WWR1 — Introduce one reusable final-MIR reachability product

Add a crate-private target-independent analysis facade whose primary product
is conceptually `MirReachabilityAnalysis`. Its concrete representation remains
private, but it provides deterministic queries for:

- roots and their reasons;
- reachable execution nodes and callables;
- outgoing possible executable targets by node and edge kind;
- address-taken targets by exact function type;
- used virtual families and interface requirements;
- runtime entities referenced by reachable work;
- whether a declaration has a retained definition; and
- stable counts suitable for pipeline measurements and debugging.

The product is not a filtered `MirProgram` and owns no transformation. It must
be independently testable on verified final MIR and useful to consumers that
never remove definitions. Initial likely consumers are final verification, the
whole-world pruning pass, backend planning, analysis dumps, and later
devirtualization or inlining heuristics.

The analysis should retain canonical sorted vectors or equivalent deterministic
private storage. Public behavior may not depend on hash iteration, pointer
identity, filesystem order, compiler worker completion, or target selection.

No general analysis cache is introduced. Reachability is substantial enough
and sufficiently central to justify one named product; that does not imply an
open-ended analysis-manager framework.

## WWR2 — Separate root policy from dependency extraction

Use three cohesive owners:

1. **root collection** derives obligations imposed without following MIR body
   edges;
2. **dependency extraction** enumerates executable and runtime-entity edges
   from declarations, metadata, and bodies; and
3. **closure solving** computes deterministic least-fixed-point facts and
   optional witnesses.

This separation is essential for future uses. A call-graph client may need all
possible targets without applying the program root policy. A diagnostic dump
may need to explain why a node is reachable. A later library or embedded entry
contract may add a new root kind without changing instruction traversal.

Root and edge kinds are closed enums with exhaustive matches. Unknown roots or
MIR operation variants are not silently ignored. Extraction failures are
structured internal compiler errors attributed to reachability analysis, not
source diagnostics.

The existing static-effect extractor should be refactored only where it
duplicates target or implicit-lifecycle selection. Static-field access
evidence, phases, witnesses, and lifecycle-authority logic remain under the
static-lifecycle owner.

## WWR3 — Model callables and implicit lifecycle work as execution nodes

The graph cannot contain only callable bodies. MIR instructions may request
compiler-defined work whose eventual executable dependencies come from class,
optional, shared-owner, or array lifecycle metadata.

Use typed execution nodes for at least:

- `CallableId`;
- class copy construction;
- class copy assignment;
- complete class finalization;
- array default construction;
- array copy;
- array assignment; and
- array destruction.

These are the same semantic categories already used by static-effect analysis,
but the general dependency owner should use reachability-neutral vocabulary.
The implementation may share or migrate the existing stable node identity;
it should not maintain two independently exhaustive lifecycle-node enums.
Compatibility aliases or focused re-exports may preserve the static-
lifecycle proof API while ownership is clarified.

Edges carry an exhaustive kind such as direct call, static call, callable
address retention, indirect call, virtual dispatch, interface dispatch,
initializer, copy, assignment, user destructor, field/base finalizer, shared
finalizer, optional lifecycle, or array lifecycle. Source spans may be retained
for deterministic explanations, but are evidence rather than node identity.

Runtime-entity references are tracked alongside executable edges rather than
pretending that a literal or dispatch table is callable. Initial entity facts
should cover what backend planning needs to avoid rebuilding semantic
reachability. Broader table deletion remains later work.

## WWR4 — Use entry and complete static lifecycle as initial roots

The initial root contract is deliberately small and exact:

- `MirProgram::entry_function` is a callable root;
- every explicit static initializer selected by the lifecycle coordinator is
  a root through its activation region;
- every zero-default static activation remains a runtime obligation even when
  it has no callable body;
- every static shutdown cleanup is a root through its exact class, optional,
  shared, or array lifecycle operation; and
- any compiler-declared source export added in the future must become an
  explicit root kind before it can be omitted from entry-only reachability.

External declarations and intrinsics are leaf dependencies when reached by a
call. They are not internal executable roots and have no Skald body to retain.
Current MIR exposes no other internal export contract.

All static lifecycle is rooted because startup and deterministic reverse
shutdown are observable. Reachability may remove callees absent from those
paths, but it may not decide that an unread static initializer or destructor
is irrelevant. Changing this rule would be a language or lifecycle-contract
proposal, not an optimizer implementation detail.

Single-threaded execution means there are no concurrent roots. Whole-world
compilation means this list is complete under the current language contract.

## WWR5 — Centralize conservative possible-target expansion

The shared target resolver follows these initial rules:

| Operation | Possible target rule |
|---|---|
| direct function or static method call | the exact internal target, or an external/intrinsic leaf |
| direct instance method call | the exact selected method |
| virtual method call | every member of the verified virtual family that can occupy the selected slot; initially the full family is acceptable |
| interface call | every verified conformance implementation for the selected interface requirement |
| callable-address formation | the exact addressed internal callable is retained as code/data identity |
| indirect call | address-taken internal targets with the exact verified function type |
| initializer/copy/assignment/destruction operation | the exact user body plus recursively selected implicit lifecycle nodes |
| optional/shared/array lifecycle operation | targets selected by the canonical MIR type and lifecycle metadata |

Function values require one coupled monotone fixed point. Collecting every
address in the entire unpruned program and treating it as an indirect candidate
would let an unreachable address formation keep unrelated bodies alive. The
solver should instead record address formations as their containing execution
nodes become reachable. A reachable indirect-call signature activates all
known reachable formations of that exact type; a newly reached formation
activates its target when that signature is already in use. Forming an exact
callable address retains that target even if no indirect call is subsequently
observed, because the address itself is runtime data.

Scanning all structurally present blocks of a reachable callable is the
initial conservative rule. A later unreachable-block pass can physically
remove dead CFG regions and thereby narrow the next reachability occurrence.

Virtual and interface expansion may initially over-approximate all declared
closed-world implementations. The resolver boundary must permit later rapid-
type-analysis or exact-origin refinement without changing callers. Any
refinement must remain monotone and independently verified.

## WWR6 — Bind analysis facts to the verified final-MIR seal

Central final verification computes reachability after structural declaration
and body checking has made the program safe to inspect. A successful
`VerifiedFinalMirProgram` retains the immutable analysis facts beside its MIR
program and exposes them through a crate-private read-only query.

This gives all selected passes in one occurrence a coherent input pair:

```text
verified final MIR = program + reachability facts derived from exactly program
```

Any changed pass outcome invalidates both. Immediate central reverification
rebuilds facts for the changed dense program before the next occurrence,
inspection checkpoint, or backend. An unchanged outcome preserves the same
seal and facts without recomputation.

This follows the pipeline's existing conservative analysis-lifetime rule. No
pass declares reachability preservation, mutates facts, or carries call-site
facts across local-ID compaction. A later measured design may add a narrowly
proven preservation path, but it is not part of this proposal.

The analysis itself remains target independent. Target selection, ABI,
registers, machine symbols, trace encodings, and object offsets cannot affect
its result.

## WWR7 — Keep global identities stable and permit sparse executable definitions

Optimized final MIR keeps all program-level declarations and their current
identities initially. It may omit an unreachable executable body while
retaining the corresponding declaration and metadata.

Concretely:

- an unreachable internal function retains its dense `FunctionId` declaration
  and an empty sparse definition slot;
- an unreachable initializer, copy operation, destructor, or method retains
  its class-owned declaration and may be absent from the member-definition
  map;
- static initializer bodies remain because every current static activation is
  rooted;
- virtual families and interface conformances continue to name declared
  methods even when an unreachable implementation body is absent; and
- no surviving `CallableId`, class/type ID, source span, or lifecycle-authority
  identity is renumbered.

This formalizes a useful distinction already latent in the data model:
declaration completeness describes the semantic closed world; definition
completeness describes executable work retained in one optimized product.

The ordinary verifier must continue validating every body that is present.
It must not require a body merely because a declaration exists. Reachability-
aware completeness verification owns the stronger requirement for every
callable that can actually execute.

Later metadata-pruning work may introduce sparse or rewritten declaration
containers under its own design. It is not required to gain the main backend
and code-size benefit here.

## WWR8 — Add a narrow atomic program retention capability

Add a program-level retention owner beside `mir::rewrite`. It consumes a
verified program and its seal-bound reachability facts, filters executable
definition containers, and publishes a complete `MirProgram` only after all
retention checks succeed.

The first capability is intentionally narrow, conceptually:

```text
retain_reachable_definitions(verified_final_mir)
    -> unchanged verified product
     | changed raw retained program + deterministic summary
     | structured retention error
```

It does not expose mutable declaration tables, arbitrary ID deletion, or
in-place callbacks. It preserves function-table holes, member-map key order,
static coordinator ownership, and every retained body byte-for-byte. There is
no callable-local recompaction because whole definitions are either retained
or removed.

The operation returns counts by callable kind and the stable IDs removed so
tests and inspection can explain its result. Pass code owns policy and
measurements; the retention owner owns container integrity. It does not log,
verify, reseal, render dumps, or modify lifecycle authority.

Future program-level transformations may reuse the ownership-transfer pattern,
but should not turn this facade into unrestricted mutable access. Declaration
compaction, call-target narrowing, and dispatch metadata rewriting each need
their own explicit invariants.

## WWR9 — Make final verification prove reachable-definition completeness

Final verification is extended in two layers:

1. structural verification validates all declarations, metadata, retained
   definition ownership, and every retained body without assuming that every
   declaration has a body; and
2. reachability completeness recomputes roots and closure, then requires a
   retained definition for every reachable internal executable callable.

This independently catches:

- a missing entry definition;
- a missing direct, virtual, interface, or indirect target;
- a missing initializer, user copy, assignment, or destructor body;
- a missing dependency selected through optional, shared, or array lifecycle;
- a retained callable-address target without code identity; and
- a lifecycle root whose reachable execution no longer exists.

Preliminary MIR keeps its existing producer-completeness rules. Only verified
final MIR gains the declaration/retained-definition distinction. This prevents
an incomplete lowering product from using reachability to excuse a producer
bug before lifecycle planning and synthesis.

Static-lifecycle realization then runs over the same retained program. Removal
may reduce realized effect sets but cannot add facts beyond immutable baseline
authority. Required lifecycle-root coverage remains exact.

Errors remain deterministic internal MIR verification failures. Optimization
does not create or suppress source diagnostics.

## WWR10 — Make backend planning consume the verified retained domain

Backend input receives the verified final MIR together with its seal-bound
reachability facts. Target lowering must stop rediscovering semantic liveness
from the declaration inventory.

Initially the backend should:

- run target legality, frame planning, runtime-trace activation planning, and
  instruction selection only for retained executable definitions;
- require dispatch entries only for virtual families and interface
  requirements that reachable MIR can select;
- permit unreachable declaration-only methods to have no body;
- materialize class, array, optional-box, literal, and static metadata when
  the target-independent runtime-entity facts require it; and
- report a backend error if it cannot lower a target-independent entity marked
  required by a verified product.

The backend may remain conservative and retain extra metadata during the first
implementation. It may not demand executable definitions outside the verified
reachable callable set merely because a dense declaration or unused dispatch
slot exists.

This is the phase-placement payoff: unreachable definitions no longer enter
target legality, layout, frame planning, instruction selection, or trace
planning.

## WWR11 — Keep machine-artifact retention as a target safety net

The existing x86-64 SysV artifact walk remains after lowering. It owns
relationships that target-independent MIR cannot represent, including:

- entry wrappers and target ABI shims;
- generated array, ownership, optional-box, and finalization helpers;
- target symbol spellings;
- concrete dispatch-table and literal-backing symbols;
- runtime-trace strings, contexts, and locations;
- panic-message symbols; and
- any later target-specific helper introduced during instruction selection.

Target-independent reachability reduces what reaches the backend. Target-
private retention proves that the emitted assembly contains only artifacts
reachable from actual exported machine symbols after all target-generated
edges exist. Neither layer substitutes for the other.

Cross-layer tests should demonstrate that an internal MIR definition removed
before lowering never appears in target planning, while a helper generated for
a retained definition survives machine-artifact pruning exactly when its
machine symbol is referenced.

## WWR12 — Ship one conservative selectable pruning client

Register one pass with stable name `whole-world-reachability`. It consumes the
verified analysis product and invokes only the atomic reachable-definition
retention capability.

The first pass:

- removes unreachable ordinary function and member definitions;
- retains all current static initializer bodies;
- does not rewrite retained bodies;
- does not compact global declarations or metadata;
- reports examined, reachable, removed, and conservatively retained counts by
  callable/edge category;
- returns unchanged without reverification when every definition is already
  reachable; and
- returns changed MIR through immediate central final verification otherwise.

The implementation may register the pass before enabling it in `default`, but
the roadmap should not close until broad parity, deterministic dumps,
pass-disable behavior, native equivalence, lifecycle coverage, and backend
work reduction justify default activation. `none` remains an empty schedule.

The initial supported order should place dead-pure-definition elimination
before whole-world reachability. Later schedules may repeat reachability after
CFG cleanup, devirtualization, inlining, or other edge-removing passes. The
registry and occurrence model already support repetition.

## Analysis and query boundary

The reusable facade should be narrow enough to remain coherent and rich enough
to prevent immediate reimplementation. Conceptually, consumers need:

```text
analysis.roots()
analysis.is_reachable(node)
analysis.reachable_callables()
analysis.possible_targets(execution_site_or_node)
analysis.used_virtual_families()
analysis.used_interface_requirements()
analysis.required_runtime_entities()
analysis.explain(node)
```

Exact Rust names and storage are implementation details. Queries return
borrowed deterministic data and cannot mutate MIR or facts. Explanation may be
a canonical first predecessor/root witness; it is a debugging aid, not a
correctness certificate or stable user diagnostic.

Call-site-specific identity is awkward because MIR instructions have no
persistent IDs. The initial product should therefore key durable facts by
execution node and typed target category, using block/instruction positions
only inside an immutable verified snapshot or dump. It must not introduce
persistent instruction identities solely for this analysis.

## Ownership and module direction

The intended organization is facade-oriented:

- a target-independent dependency-analysis owner under `passes`, separate from
  static lifecycle and the selectable pipeline runner;
- focused root, extraction, target-resolution, closure, model, and dump
  submodules behind one crate-private facade;
- a `mir` program-retention facade responsible only for atomic definition
  container ownership transfer;
- pipeline execution integration that binds facts to
  `VerifiedFinalMirProgram` and exposes the narrow retention capability;
- one small optimization module that selects reachable-definition retention;
- final-verifier integration for sparse definition completeness; and
- backend integration consuming only the public sealed query surface, never
  analysis internals.

The existing `passes::graph` algorithms may be reused for canonical SCC or
closure support. Static-lifecycle extraction should consume shared target
resolution rather than reaching into the reachability solver, avoiding a
dependency cycle between correctness analysis and optimization policy.

## Determinism and observation

Reachability is observable through:

- the existing per-pass occurrence record;
- deterministic pass measurements;
- verified before/after/final MIR checkpoints;
- a focused deterministic reachability dump for compiler tests and debugging;
  and
- aggregate final-MIR and backend-work statistics already owned by reporting.

Passes and analysis helpers do not log. Reports should not embed the complete
graph or dumps. Candidate measurements include:

- root count;
- reachable execution-node and callable counts;
- direct, virtual, interface, indirect, and lifecycle edge counts;
- reachable function-value signatures and address targets;
- removed function/member definition counts; and
- retained definitions exceeding the computed closure, if any.

Counts must distinguish declarations, physically present definitions, and
reachable callables. Conflating them would hide both pruning effectiveness and
malformed retained products.

## Verification and test strategy

### Focused analysis tests

- entry-only direct and transitive call chains;
- unreachable acyclic definitions and mutually recursive components;
- self recursion and deterministic SCC/closure behavior;
- direct, static, instance, virtual, and interface calls;
- exact callable-address retention and indirect calls by function type;
- address formations present only in unreachable callables;
- initializer, user copy, copy assignment, destructor, optional, shared, and
  array lifecycle dependencies;
- static activation and reverse-shutdown roots;
- external and intrinsic leaves;
- deterministic roots, edges, candidate sets, and witnesses across repeated
  and independent-process runs; and
- exhaustive maintenance tests for every MIR instruction, terminator, and
  implicit lifecycle variant that can select executable work.

### Retention and verification tests

- stable global IDs and declaration tables after definition removal;
- preserved sparse function slots and deterministic member-map order;
- byte-for-byte unchanged retained definitions;
- atomic failure without a partially published program;
- acceptance of absent unreachable definitions in final MIR;
- rejection of absent entry, direct, virtual, interface, indirect, copy,
  destruction, array, shared, and static-lifecycle targets;
- continued rejection of incomplete preliminary MIR;
- lifecycle realization as a subset of baseline authority after pruning; and
- unchanged outcomes that preserve the original seal without another
  verification execution.

### Pipeline and backend tests

- pass registration, listing, profile selection, disabling, exact schedules,
  and repeated occurrence behavior;
- deterministic measurements and checkpoint dumps;
- optimization-off exact final-MIR parity;
- default versus explicitly disabled MIR/assembly differences only where dead
  definitions exist;
- backend legality, layout, frame, instruction-selection, and trace planning
  not visiting pruned definitions;
- virtual/interface metadata remaining valid when unused declared methods have
  no retained body;
- target-generated helper survival through final artifact retention; and
- native equivalence for scalar, object, dispatch, function-value, static,
  optional, shared, array, panic, and runtime-trace corpora.

### Repository gates

The implementation roadmap should require focused Rust tests while each owner
lands, then the root Makefile quality gate, golden and native suites,
determinism checks, documentation-link checks, and the supported MSRV gate when
Rust code or manifests change. No repository CI is introduced.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| A new MIR operation silently omits an executable dependency | Exhaustive extraction matches and variant-focused maintenance tests |
| Root policy accidentally drops observable static work | Root every coordinator activation and shutdown region; lifecycle parity tests |
| An unreachable address formation retains unrelated indirect targets | Reachability-scoped function-value fixed point rather than whole-program candidate seeding |
| Dynamic dispatch pruning misses an implementation | Start with full verified family/conformance expansion; narrow only under a later proof |
| Declaration metadata still assumes every body exists | Explicit final-MIR declaration/retained-definition contract and reachability-aware verifier/backend |
| A pass removes a reachable definition | Central closure recomputation and reachable-definition completeness verification |
| Analysis facts outlive changed MIR | Store facts in the final-MIR seal; unconditional invalidation on every changed occurrence |
| Static-effect and reachability extractors drift | Share target/lifecycle dependency selection, retain separate effect evidence and solvers |
| Global ID compaction spreads through the compiler | Preserve declarations and program-level identities in the initial implementation |
| Backend artifact pruning is removed too early | Keep it as the mandatory final target-generated dependency safety net |
| Conservative expansion hides poor precision | Typed edge counts, target queries, dumps, and explicit over-retention measurements |
| Reachability becomes an accidental general analysis manager | One named immutable product with fixed ownership and no pluggable cache protocol |

## Alternatives considered

### Keep only backend machine-artifact pruning

This preserves emitted-size cleanup but keeps dead definitions in target
legality, layouts, frames, instruction selection, traces, and generated helper
planning. It also provides no target-independent call graph for future passes.

### Put reachability inside the backend

That would duplicate semantic target selection for every backend and make
devirtualization, inlining, and effect analysis depend on target code. Skald's
whole-world guarantee exists before target selection and should be used there.

### Let the pass delete dense declarations and compact all IDs immediately

This promises smaller metadata but turns the first implementation into a
global semantic-identity rewrite across nearly every MIR table and proof. It
adds risk without being necessary to stop lowering unreachable bodies.

### Retain all methods whenever any class is reachable

This is safe and may be used as a temporary rollout fallback, but it makes
method pruning ineffective for common object-heavy programs. The durable
contract should instead retain methods selected by reachable direct or dynamic
operations while allowing conservative full-family target expansion.

### Trust the pass's reachable set as a certificate

A bug in the root or edge walker would then become silent miscompilation.
Central verification must independently recompute closure on the resulting
program and check body availability, just as it independently checks static-
lifecycle realization.

### Reuse the static-effect analysis product unchanged

It contains useful edges but is shaped around static-field facts, phases,
witnesses, and preliminary lifecycle planning. Making every future call-graph
consumer depend on static-effect summaries would blur ownership and retain
unnecessary evidence. Share extraction mechanisms and stable semantic node
identity, not the complete result type.

### Compute reachability only inside the pruning pass

That would make final verification and backend planning rebuild their own
versions and deny later passes a supported query. The analysis is the
foundation; pruning is one client.

## Effort and recommended delivery order

Overall effort is **large**. The graph algorithm is modest; the work lies in
making target selection exhaustive, separating declaration and retained-body
validity, integrating the verified seal, and teaching backend dispatch/runtime
metadata planning not to demand unreachable bodies.

| Delivery slice | Relative effort | Primary result |
|---|---|---|
| Execution-node, edge, root, and target-resolution contract | Medium | One reviewable semantic dependency vocabulary |
| Deterministic graph extraction, closure, queries, and dump | Medium to large | Reusable whole-program reachability analysis |
| Final-seal and verification integration | Medium to large | Facts tied to verified MIR and independently checked sparse definitions |
| Atomic program-level definition retention | Medium | Safe stable-ID body removal |
| Backend retained-domain integration | Large | Dead MIR avoids target planning while machine pruning remains |
| Selectable pass, measurements, parity, and default activation | Medium to large | First production client proving the full boundary |

The implementation roadmap should preserve this dependency order:

1. settle execution nodes, root reasons, target expansion, and reusable query
   ownership;
2. extract and test the immutable dependency graph without changing products;
3. compute deterministic root closure and bind it to central final
   verification;
4. redefine final-only definition completeness while preserving preliminary
   producer completeness;
5. add atomic stable-ID definition retention;
6. make backend planning consume verified retained-domain facts;
7. register the definition-only pruning pass and prove selection,
   measurements, dumps, and repeated schedules; and
8. enable it by default only after full semantic, native, determinism, and
   optimization-off gates pass.

Declaration/metadata compaction, rapid-type-analysis precision, call-site
points-to analysis, reachability-preservation declarations, and broader
interprocedural optimization remain discoveries for later work rather than
scope silently absorbed by this roadmap.

## Confirmation and promotion

If confirmed, WWR1 through WWR12 should freeze together. Root completeness,
possible-target expansion, sparse-definition validity, verification, and
backend consumption form one correctness boundary; confirming only the
pruning pass would leave its proof obligations undefined.

After confirmation, promote the durable direction into the compiler phase,
MIR verification, backend, and optimization-selection contracts, then create
an implementation roadmap plus a dedicated discoveries record. The roadmap
should end with the registered and hardened production pass, but its primary
success criterion is the reusable verified reachability foundation described
here.

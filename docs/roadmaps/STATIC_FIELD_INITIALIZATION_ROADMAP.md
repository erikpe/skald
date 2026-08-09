# Static Field Initialization and Shutdown Roadmap

Status: in progress; the combined design record is frozen, SI0 through SI6 are
complete, and SI7 is next.

This roadmap extends the implemented zero-default static-field profile with
eager declaration initializers and deterministic normal-return shutdown. The
result permits primitives, exact inline objects, optional values, shared
owners, strings, and arrays to be constructed once before the selected entry
function and cleaned once after it returns, without adding lazy initialization,
garbage collection, or source-visible module lifecycle blocks.

The design investigation compared Skald's current receiver-free static places,
stored-value initialization, verified cleanup, always-generated host entry
wrapper, and canonical whole-program identities with Niflheim's implemented
static-variable pipeline. Niflheim's synthesized program initializer and entry
wrapper are useful precedents. Its constant-only expressions, nullable
reference slots, GC root registration, and lack of deterministic static
destruction are not suitable Skald lifecycle semantics.

## Design status and rationale

This document intentionally serves as both the frozen design record and the
implementation roadmap. No separate design proposal preceded it. SI0 completed
the investigation and froze the language, lifecycle, analysis, verification,
backend, and exclusion decisions before executable implementation work begins.
The scope, lifecycle model, dependency graph contract, and rationale below are
therefore the authoritative planned design; they do not claim that the living
compiler already implements it.

| Concern | Frozen direction | Rationale and rejected alternatives |
|---|---|---|
| Expression surface | Permit ordinary typed initialization expressions | A constant-only subset would be simpler but too weak for constructor arguments, allocation, arrays, and useful shared singletons. |
| Startup mechanism | Generate one eager program initializer called by the existing host wrapper | Lazy first-use state complicates access and threading; linker constructors make order target- and toolchain-dependent. |
| Dependency safety | Conservatively analyze all reachable static effects and reject cycles at compile time | Direct-syntax ordering misses deep calls and cleanup; runtime guards add ordinary-access overhead and defer errors until execution. |
| Analysis boundary | Analyze preliminary target-independent MIR before optimization and backend lowering | HIR does not yet make every temporary cleanup and lifecycle edge explicit; backend analysis is too late and target-dependent. |
| Lifetime ordering | Use one field dependency relation and exact-reverse shutdown | Independent startup and shutdown orders accept more programs but weaken the predictable nested-lifetime model. |
| Verification | Carry a checkable effect and ordering certificate in MIR | Trusting analysis metadata would weaken the backend verification boundary; emitted per-slot guard state is unnecessary for valid closed programs. |
| Shutdown and failure | Clean statics after normal entry return; retain non-unwinding abrupt termination | Process-lifetime retention leaks deterministic resource finalization, while exceptional partial-startup cleanup requires unwinding semantics Skald does not have. |
| Runtime boundary | Add private generated code and storage but no public runtime service or ABI change | Dependency resolution, lifecycle state, and ordering are compiler-owned whole-program semantics. |

During implementation, a discovery that invalidates one of these frozen
directions must revise this design record before dependent roadmap work
continues. Each implemented contract moves into the appropriate living
language or compiler document in the task that makes it executable. At
roadmap completion, those living documents become authoritative for current
behavior and the archived roadmap remains the durable rationale and delivery
record.

## Scope and invariants

- Add `static name: T = expression;` and
  `private static name: T = expression;` without changing the identity,
  namespace, inheritance, qualification, or privacy rules of existing static
  fields.
- Keep initializer-free declarations restricted to the existing complete
  all-zero value set. An explicit initializer permits every ordinary stored
  field type: primitives, exact classes, supported inline optionals, shared
  owners, optional shared owners, and inline arrays. `unit`, bare interfaces,
  and bare `Obj` views remain invalid storage.
- Treat the declaration expression as direct initialization of previously
  uninitialized program storage through the existing
  `HirStoredValueInitialization` family. It is not assignment to a zero value,
  does not require copy assignment, and retains ordinary construction, copy,
  adoption, optional publication, array, overload, evaluation-order, and
  full-expression rules.
- Give a declaration initializer the lexical privacy of its declaring class,
  but no receiver, parameters, local declarations, or `self`. The expression
  may use ordinary literals, operators, calls, constructors, allocations,
  array construction, casts, and explicitly qualified static fields.
- Eagerly establish static state after the runtime ABI marker succeeds and
  before the selected Skald entry function begins. Do not use ELF
  `.init_array`, linker constructors, or host-specific initialization order.
- Define one target-independent activation plan over every static field.
  Activating an initializer-free zero-default field runs no Skald code, but its
  position remains part of the plan because the field's eventual value cleanup
  may depend on another static. Evaluate explicit initializers one complete
  full expression at a time at their planned positions.
- Compute dependency order from whole-program typed effects rather than only
  direct syntax. For each explicit field, include every static read, write,
  borrow, or other access that may occur while evaluating its expression,
  inside any transitively called function or lifecycle body, during implicit
  copy/construction, and while cleaning its full-expression temporaries.
- Include static uses that may occur while destroying a field's final value in
  the same dependency relation. If field `X` may access field `Y` while `X` is
  initialized or destroyed, `Y` must become live before `X` and outlive it.
- Use that one relation for both activation and exact-reverse shutdown. This
  deliberately rejects a program whose startup and teardown constraints could
  be satisfied only by two different, non-reverse orders; preserving one
  nested lifetime order keeps construction, ownership, and destruction
  semantics predictable.
- Conservatively include every possible virtual/interface dispatch target,
  every possible dynamic finalizer for shared and array-contained owners, and
  recursive call strongly connected components. A path that cannot be proved
  free of a static use counts as using that static.
- Reject self-dependencies and dependency cycles during target-independent MIR
  analysis, before final MIR verification or backend selection, with a
  diagnostic that shows a useful initializer/destructor and call-chain
  witness. Do not defer a valid Skald program's first bad access to runtime.
- Topologically order all fields by the dependency graph, using existing
  canonical declaration order as the deterministic tie-breaker for unrelated
  fields: canonical module-path order, class source order, then direct static
  declaration order. Import traversal, provider option order, selected entry,
  and inherited aliases do not change the result.
- Ordinary code reached during startup may read, borrow, or replace a field
  proved live by the plan. Normal access never initializes a slot. Whole-
  program effect validation guarantees that valid Skald code cannot reach the
  slot currently being initialized or any not-yet-live slot.
- Publish an explicitly initialized slot as live immediately after its stored
  value completes and before cleanup of later source temporaries at that full-
  expression boundary. Begin the next initializer only after that boundary is
  complete.
- After the entry function performs its own ordinary return cleanup, preserve
  its `i64` result and end every static lifetime in exact reverse activation
  order, including initializer-free fields that may have acquired owning
  contents during program execution.
- Semantically transition a live slot to `destroying` before invoking any
  destructor, shared release, optional cleanup, or array release, then to
  `dead` after the operation. Effect validation rejects a teardown plan that
  could access the current or an already-ended slot; fields whose teardown has
  not begun remain live and usable by later-field destructors.
- Apply shutdown to every static field, including initializer-free fields that
  acquired owning contents through later replacement. This deliberately
  replaces the current process-lifetime-retention rule on normal entry return.
  Primitive slots have no value cleanup but still make the same observable
  live-to-dead transition.
- Keep panic and other abrupt termination non-unwinding. A failed initializer,
  panic in user `main`, panic during static destruction, allocation failure,
  or foreign process termination does not run remaining static cleanup.
- Keep lifecycle order, effect evidence, and semantic state transitions
  explicit and verified above the backend. The x86-64 backend mechanically
  emits private value slots, lifecycle functions, and wrapper calls; ordinary
  static access requires no per-access lifecycle guard in a valid program.
- Add no public runtime symbol or service and no runtime dependency resolver.
  The public runtime ABI need not change unless implementation evidence
  exposes a missing invariant.
- Exclude lazy or first-use initialization, thread safety, atomics, initializer
  blocks, deinitializer blocks, user-selected priorities, dependency
  source-selected priorities, separate compilation, dynamic loading, external
  or exported storage, top-level globals, exceptional unwinding, cleanup after
  `exit`/signals, foreign re-entry into generated `main`, foreign concurrency
  during program lifecycle, and compile-time object serialization.

## Lifecycle model

Each static declaration has one hidden state with these semantic values:

```text
uninitialized -> initializing -> live -> destroying -> dead
```

Initializer-free fields skip `initializing` and become `live` at their planned
activation point. An explicit initializer enters `initializing` before its
expression begins and enters `live` only after its destination is completely
initialized. No ordinary access changes state. Shutdown accepts only `live`,
sets `destroying` before value cleanup, and sets `dead` after cleanup.

The state is a semantic verification model, not necessarily emitted target
storage. It is not part of the field's source type, object layout, value slot,
public ABI, or reflection surface. MIR must preserve the transitions and
distinguish lifecycle-owned uninitialized destination access from ordinary
static access. Since whole-program effect validation proves that ordinary
access occurs only while the target is live, a backend need not emit one state
byte or check on every access.

## Dependency graph contract

Lifecycle planning uses two related graphs with different nodes and different
reasons for finding strongly connected components:

1. The callable-effect graph has one node for every ordinary MIR body and
   analyzable implicit lifecycle operation. An edge `A -> B` means executing
   `A` may execute `B`. Direct static-place uses seed each node. Condensing
   recursive callable components and propagating seed sets over the resulting
   DAG produces conservative transitive static effects with witness paths.
2. The static-lifetime graph has one node for every static declaration. An
   edge `T -> F` means `T` must be activated before `F` and therefore is
   destroyed after `F`. Such an edge is added when initialization of `F` may
   access `T`, or destruction of the eventual value in `F` may access `T`.
   This rule applies equally to explicit and initializer-free fields.

An access by initializer `F` to `F` before publication is an invalid self-edge;
an access during cleanup explicitly proven to occur after publication is
valid. Access by `F`'s destruction to `F` is invalid because its state is
already `destroying`. Every other reached static access adds the ordinary
dependency edge above.

The planner rejects self-edges and every multi-node static-lifetime strongly
connected component. A diagnostic selects one cycle deterministically by
canonical field identity and stable outgoing-edge order. Each reported field
edge retains its root phase, implicit or callable path, final static access,
and source spans, so a dependency cycle is not reported as an unexplained list
of declarations. Topological sorting operates on the acyclic lifetime graph
with canonical identity as its ready-node tie-breaker.

The analysis result is a checkable certificate, not an unchecked optimizer
annotation. Final MIR verification confirms that each body summary contains
all of its direct static uses and the summaries of every possible callee or
implicit lifecycle target. For each initializer and destruction root, it then
requires every summarized static effect to have the corresponding lifetime
edge and verifies that edge against the activation order. The verifier need
not recompute a least fixed point or choose cycle witnesses, but it must reject
an omitted effect, target, field, edge, or ordering constraint before a backend
consumes MIR.

For example:

```ska
class Log {
    static enabled: bool;
    static destination: shared Sink = new Sink("events.log");
    static buffer: u8[] = u8[](4096u);

    init() {}
}
```

With no additional dependency edges, canonical order makes `Log.enabled` live
first with `false`; `Log.destination` then
allocates and initializes its owner; `Log.buffer` allocates last. Normal
shutdown releases `buffer`, then `destination`, then ends `enabled`. If code
called while constructing `destination` may access `Log.destination`,
compilation fails with the transitive self-dependency path. If it may access
`Log.buffer`, the dependency graph instead moves `buffer` before `destination`
and shutdown reverses that order. Access to `Log.enabled` likewise keeps or
moves that slot before `destination`.

## Progress

- [x] SI0 — Freeze the eager static lifecycle profile
- [x] SI1 — Retain and resolve declaration initializers
- [x] SI2 — Type-check direct stored-value initialization
- [x] SI3 — Lower analyzable static lifecycle bodies to preliminary MIR
- [x] SI4 — Infer transitive static effects
- [x] SI5 — Plan and diagnose static lifetimes
- [x] SI6 — Define and verify lifecycle MIR
- [ ] SI7 — Synthesize the lifecycle coordinator
- [ ] SI8 — Execute eager startup
- [ ] SI9 — Execute reverse normal-return shutdown
- [ ] SI10 — Harden and publish initialized static fields

## PR-sized implementation sequence

### SI0 — Freeze the eager static lifecycle profile

**Purpose:** Resolve source, ordering, dependency, ownership, failure, and
shutdown decisions before changing representations.

- [x] Audit the implemented static declaration, place, storage, and entry
      wrapper pipeline.
- [x] Audit ordinary stored-value initialization and deterministic cleanup for
      every supported owning category.
- [x] Compare Niflheim's generated initializer and entry wrapper while
      separating its constant-expression and GC assumptions from Skald.
- [x] Select eager dependency-ordered initialization, reverse shutdown, and
      whole-program transitive static-effect validation.
- [x] Preserve the useful unrestricted expression surface while requiring a
      conservative proof across calls, dispatch, implicit lifecycle, and
      recursive call components.
- [x] Record exclusions and the intentional change from process retention to
      normal-return static cleanup.

**Tests:** Design review against `STATIC_FIELDS.md`, `CLASSES_AND_LIFECYCLE.md`,
`FUNCTIONS_AND_CONTROL_FLOW.md`, `SHARED_OWNERSHIP.md`, `ARRAYS.md`,
`ERRORS.md`, `PHASES_AND_IR.md`, `BACKEND.md`, and `RUNTIME_ABI.md`; inspect
Niflheim's `STATIC_CLASS_VARIABLES_IMPLEMENTATION_PLAN.md`, generated program
initializer, entry wrapper, and root tests.

**Exit criteria:** Later tasks require no new language-level decision about
syntax, type availability, order, dependency failure, publication, cleanup,
or abrupt termination.

### SI1 — Retain and resolve declaration initializers

**Purpose:** Carry the optional source expression to stable, owner-aware
resolved IR without changing executable behavior yet.

- [x] Parse optional `= expression` before the static declaration semicolon,
      retain exact `=` and expression spans, and preserve contextual `static`
      recovery and existing initializer-free syntax.
- [x] Extend `StaticFieldDecl`, `ResolvedStaticFieldDeclaration`, exact AST and
      resolved dumps, and deterministic equality/fixture coverage.
- [x] Add a callable-like compiler identity for each explicit static
      initializer, derived from its canonical `StaticFieldId`, so binding,
      temporary, block, and storage IDs never borrow an unrelated function or
      lifecycle member identity.
- [x] Resolve the expression only after all program declarations, imports,
      class hierarchy, members, overload candidates, string language items,
      and static identities are available.
- [x] Resolve with the declaring class as lexical privacy owner but without a
      receiver or base-initialization capability. Reject `self`, `super`, bare
      member lookup, statements, and other receiver-only forms through normal
      resolution paths.
- [x] Retain canonical `StaticFieldId` uses, resolved call targets, dynamic
      dispatch families, and source spans needed by later effect analysis.
- [x] Preserve source order and inherited aliases without creating additional
      initializer work or storage.

**Tests:** Focused syntax and resolution tests for every declaration form,
malformed `=`, privacy, imports and selective imports, inherited aliases,
forward declarations, strings, stable identities, exact dumps, and
cross-process determinism.

**Exit criteria:** Every syntactically valid explicit initializer is retained
once under its declaring static identity and either resolves completely or
produces deterministic source diagnostics; later phases perform no name or
order reconstruction.

### SI2 — Type-check direct stored-value initialization

**Purpose:** Reuse ordinary initialization semantics for program-owned
destinations and settle the complete explicit-initializer type matrix in HIR.

- [x] Split declaration validation into initializer-free zero-default
      capability and explicit stored-value initialization capability.
- [x] Accept explicit initialization for the same stored types as ordinary
      instance fields, while retaining focused rejection of `unit`, bare
      interfaces, bare `Obj`, aliases, and unsupported forms.
- [x] Generalize `CallableChecker` only as needed for a receiver-free,
      parameter-free static-initializer context with declaring-class privacy
      and ordinary expression checking.
- [x] Route the destination and expression through
      `check_stored_value_initialization` so primitives, direct exact-object
      production, selected copy construction, inline optionals, shared
      transfer/adoption, optional shared owners, strings, and arrays keep one
      semantic owner.
- [x] Retain one `HirStoredValueInitialization` and its full-expression cleanup
      metadata on the static declaration or a dedicated HIR lifecycle table;
      do not encode assignment to a default value.
- [x] Retain source spans for direct static accesses, calls, selected
      constructors/copies, temporary ownership, and implicit cleanup so later
      effect analysis can produce call-chain diagnostics.
- [x] Extend HIR dumps and diagnostics with initializer identity, destination
      type, selected overload/copy operation, and lifecycle order.

**Tests:** Focused type-check tests for all primitive and owning categories,
constructor overloads and privacy, direct production versus copy, unavailable
copy, shared and optional transfer, arrays and element lists, strings,
evaluation once, invalid stored types, exact HIR dumps, and deterministic
diagnostics.

**Exit criteria:** HIR completely describes each accepted static destination,
its typed direct initialization, and full-expression ownership. It retains
the selected semantic operations needed for MIR lowering, but does not attempt
to duplicate MIR's explicit temporary-cleanup or lifecycle edge inventory;
invalid source cannot reach lifecycle analysis.

### SI3 — Lower analyzable static lifecycle bodies to preliminary MIR

**Purpose:** Establish an explicit, target-independent analysis input in which
implicit calls, temporaries, and cleanup cannot be missed by a second partial
interpretation of HIR.

- [x] Split MIR construction into a preliminary program product and the final
      planned `MirProgram`, or otherwise make it impossible for a backend to
      consume lifecycle-unplanned MIR.
- [x] Lower one independently identified MIR body for each explicit static
      initializer through the ordinary stored-value lowering machinery,
      including evaluation order, temporaries, adoption, optional wrapping,
      array construction, and full-expression cleanup.
- [x] Represent the static destination, the completion/publication boundary,
      and any cleanup that follows publication explicitly enough for the
      analysis to distinguish pre-publication dependencies from legal
      post-publication uses of the newly live field.
- [x] Retain all ordinary callable bodies plus virtual families, interface
      conformance selections, destruction plans, array element lifecycle, and
      source spans in the preliminary product.
- [x] Give every implicit lifecycle operation an analyzable target or a
      conservative finite target set; do not rely on backend-only destructor
      or shared-release knowledge.
- [x] Add a structural preliminary verifier for identities, types, targets,
      ownership metadata, and control flow that is meaningful before a global
      lifecycle plan exists.

**Tests:** Exact preliminary MIR for every stored-value category; implicit
constructor, copy, temporary-cleanup, shared-release, optional, and array
operations; publication-boundary placement; virtual/interface metadata; and
rejection of malformed preliminary programs.

**Exit criteria:** Static lifecycle analysis receives one structurally valid
MIR-level closed-world program in which all executable operations relevant to
static effects are explicit, and no target backend can observe the unplanned
product.

**Implementation result:** `PreliminaryMirProgram` privately owns the ordinary
program, canonical field-mode inventory, and one static-initializer body per
explicit declaration. Each body has a single CFG publication edge before
full-expression cleanup. Ordinary and initializer bodies share verifier and
dump machinery, while preliminary-only static destination rules and finite
shared lifecycle target expansion preserve the final MIR/backend trust
boundary. The complete stored-value matrix, strings, named static copies,
publication ordering, closed-world lifecycle metadata, malformed products,
public API composition, and cross-process dumps are covered. `make check`,
`make golden-determinism-test`, and `git diff --check` pass.

### SI4 — Infer transitive static effects

**Purpose:** Prove at compile time that every static use reached during startup
or shutdown targets a value live at that point, including uses hidden by deep
call stacks and implicit lifecycle operations.

- [x] Add a responsibility-oriented `passes::static_lifecycle` analysis after
      preliminary MIR lowering and before the ordinary verified MIR pipeline;
      keep it target-independent and before optimization.
- [x] Build one whole-program callable effect graph from MIR, with every place
      rooted at a static field and its source span as a seed effect.
- [x] Add edges for direct and static calls, selected initializers and copy
      constructors, user destructors, compiler-generated complete finalizers,
      temporary cleanup, optional and array lifecycle, string language-item
      calls, and other implicit executable operations.
- [x] Expand virtual/interface calls to every possible linked implementation
      and shared-owner cleanup to every compatible dynamic finalizer that the
      closed program can produce.
- [x] Condense recursive callable components and propagate effects over the
      resulting DAG. Retain a minimum-edge witness for each transitive static
      effect, with equal-length paths ordered by stable target identity, edge
      kind, and source span.
- [x] Keep access kind and lifecycle phase in effect evidence where they
      improve diagnostics, while treating every read, write, borrow, or other
      ordinary access as requiring the target field to be live.
- [x] Verify effect extraction exhaustively against the MIR instruction and
      terminator inventory so adding a new executable operation cannot silently
      bypass lifecycle analysis.

**Tests:** Direct, multi-hop, recursive, mutually recursive, virtual,
interface, constructor, copy, string, temporary-destructor, shared-finalizer,
optional, and array effects; conservative unreachable-branch effects; stable
witness selection; complete instruction-inventory coverage; and cross-process
determinism.

**Exit criteria:** Every callable and lifecycle body has one conservative,
deterministic summary of the static fields it may access, including a source-
facing witness for each reported effect.

**Implementation result:** `passes::static_lifecycle::infer_static_effects`
builds one closed-world graph over ordinary/static-initializer bodies plus
explicit compiler-generated copy, complete-finalizer, and array-lifecycle
nodes. Exhaustive instruction, terminator, rvalue, nested lifecycle, and
destruction matches collect static-rooted evidence and implicit edges;
virtual/interface dispatch and shared cleanup expand through linked target
sets. An iterative SCC condensation and DAG propagation computes conservative
field sets without call-stack recursion, while deterministic breadth-first
witness selection retains minimum-edge source-facing paths, access kinds, and
initializer publication phase. The driver runs inference after preliminary
verification, and stable dumps plus focused and cross-process tests cover deep
calls, recursion, dispatch, construction/copy/assignment, strings, temporary
and optional cleanup, shared finalizers, arrays, unreachable branches, and
witness ordering. `make check`, `make golden-determinism-test`, and
`git diff --check` pass.

### SI5 — Plan and diagnose static lifetimes

**Purpose:** Convert transitive effects into one deterministic activation and
shutdown plan, or reject the program with actionable cycle diagnostics.

- [x] Build the static-lifetime graph over every field. Add `T -> F` when
      initialization or eventual-value destruction of `F` may access `T`,
      including destruction of an initializer-free field whose owning value
      can be replaced during ordinary execution.
- [x] Reject initializer or destructor self-dependencies and dependency cycles
      by finding static-lifetime strongly connected components separately from
      callable recursion; label declarations, direct static uses, and the
      transitive call/lifecycle path.
- [x] Produce one deterministic topological activation plan using canonical
      declaration identity as the tie-breaker; produce shutdown as its exact
      reverse.
- [x] Verify phase-sensitive publication: accesses before a field publishes
      are dependencies, while full-expression cleanup proven to occur after
      publication may use that field.
- [x] Retain one evidence record per lifetime edge with root field, startup or
      shutdown phase, call/lifecycle witness, target static access, and source
      spans. Select a stable representative cycle when an SCC contains several
      possible cycles or parallel evidence paths.
- [x] Attach effect summaries, dependency evidence, and the completed lifecycle
      plan to a dedicated planned-MIR product so final MIR verification and
      backends never repeat call-graph or dependency inference.
- [x] Report lifecycle failures as ordinary source diagnostics from the driver,
      separately from malformed-MIR verification errors.

**Tests:** Initialization-only and destruction-only dependencies; destruction
dependencies from replaced initializer-free owning fields; independent-order
tie-breaking; valid deep dependencies; self and multi-field SCCs; mixed
startup/teardown cycles that would require non-reverse orders; overlapping
cycles and parallel evidence paths; separation from recursive callable SCCs;
phase-sensitive publication; exact plan and evidence dumps; stable cycle
diagnostics; and cross-process determinism.

**Exit criteria:** The planned-MIR product contains one deterministic activation
and reverse-shutdown plan over every static field, and valid Skald code has a
conservative proof that no startup or shutdown path can access a not-live
static field.

**Implementation result:** `passes::static_lifecycle::plan_static_lifetimes`
builds a canonical static-declaration graph from initializer summaries and
type-derived eventual-value destruction roots, including initializer-free
replaceable owning slots. It retains one stable startup/shutdown evidence
record per `T -> F` edge, distinguishes lifecycle destination publication from
ordinary access, rejects self-edges and deterministic representative SCC
cycles with `STA001`/`STA002` source diagnostics, and uses canonical identity
for topological tie-breaking and exact-reverse shutdown. `PlannedMirProgram`
privately owns preliminary MIR together with effect summaries, dependencies,
and the completed plan so later phases cannot repeat inference. Effect
summaries now retain distinct access/phase witnesses when destination
initialization and ordinary access reach the same field. Focused graph,
publication, destruction, recursion, mixed-cycle, overlapping-cycle, dump,
driver, public-API, and cross-process determinism tests cover the phase.
`make check`, `make golden-determinism-test`, and `git diff --check` pass.

### SI6 — Define and verify lifecycle MIR

**Purpose:** Give the planned lifecycle and its effect certificate an explicit,
target-independent MIR schema with a verifier-owned trust boundary.

- [x] Add dedicated MIR program-lifecycle definitions or tables with stable
      identities, typed storage, values, blocks, and dumps; do not masquerade
      them as source functions or receiver members.
- [x] Widen `MirStaticFieldDeclaration` from the zero-default subset to the
      complete checked HIR storage matrix and retain initialization mode and
      lifecycle order.
- [x] Represent `begin initialization`, `publish live`, `begin destruction`,
      and `finish destruction` as explicit MIR semantics tied to one
      `StaticFieldId`.
- [x] Distinguish lifecycle-owned uninitialized destination access from
      ordinary static-place access without exposing an uninitialized form to
      ordinary source lowering.
- [x] Represent per-body direct effects, conservative transitive summaries,
      possible dynamic targets, lifetime edges, and plan indices as a
      checkable certificate rather than backend-owned metadata.
- [x] Extend structural, place, and type verification for program-owned roots,
      lifecycle identities, destination types, phase partitions, and complete
      field coverage.
- [x] Verify certificate soundness by scanning every instruction and
      terminator for direct effects and possible callees, requiring each
      transitive summary to be a conservative superset, and checking every
      lifetime edge against the planned order.
- [x] Keep least-fixed-point computation, topological sorting, and diagnostic
      witness selection in the analysis pass; verification checks soundness
      without silently repairing or trusting an incomplete certificate.

**Tests:** Hand-built valid planned MIR; mutation tests for missing direct
effects, call targets, dynamic targets, summary closure, lifetime edges,
fields, phase partitions, and order constraints; mistyped or foreign
identities; exact dumps; and verifier determinism.

**Exit criteria:** The lifecycle MIR schema can carry the complete plan and a
verifier can establish its soundness without target layout, backend inference,
or runtime access guards.

**Completed:** `PlannedMirProgram` now owns explicit MIR lifecycle definitions,
field-derived initializer identities with typed storage/value/block bodies,
activation and shutdown transitions, declaration plan indices, and a
certificate containing direct effects, conservative summaries, possible
targets, and evidenced lifetime edges. `verify_planned_mir` first applies the
ordinary preliminary-MIR structural/type checks, then exhaustively re-extracts
direct effects and call targets and checks certificate coverage, summary
closure, evidence, field coverage, phase partitions, dependency order, and
exact-reverse shutdown. The unpublished destination has a distinct MIR place
root that ordinary lowering cannot construct and backends cannot consume.
Mutation, exact-dump, public-API, driver-boundary, and determinism tests cover
the new trust boundary. SCC solving, topological sorting, and witness selection
remain solely in the analysis pass. `make check`,
`make golden-determinism-test`, and `git diff --check` pass.

### SI7 — Synthesize the lifecycle coordinator

**Purpose:** Turn the verified plan and preliminary initializer bodies into the
single final MIR program consumed by ordinary passes and every backend.

- [ ] Move each already-lowered preliminary initializer body into its planned
      program-owned lifecycle region without reinterpreting HIR or changing
      expression evaluation and cleanup order.
- [ ] Emit zero-default activation at its planned position, including no-op
      value work for primitive slots, and wrap explicit bodies with begin and
      publish transitions at the checked completion boundary.
- [ ] Preserve post-publication full-expression cleanup before beginning the
      next field and reject control flow that bypasses publication or cleanup.
- [ ] Synthesize reverse-order destruction with begin and finish transitions
      around ordinary complete-object, optional, shared-owner, and array
      cleanup operations.
- [ ] Extend ownership, array, cleanup, lifetime, and control-flow verification
      for the synthesized coordinator, including exact coverage, unique legal
      transitions, and destination non-escape.
- [ ] Verify every ordinary static access in a lifecycle region through the
      checked effect certificate and plan; emit no runtime access guard.
- [ ] Run the ordinary target-independent MIR pipeline only after synthesis
      produces the final, fully verified `MirProgram`.

**Tests:** Exact synthesized MIR for every storage and cleanup category;
publication followed by temporary cleanup; zero-default positions; reverse
shutdown; mutation tests for missing, duplicated, reordered, bypassed, or
illegal transitions; initializer destination escape; ownership violations;
and deterministic dumps.

**Exit criteria:** Final verified MIR alone proves complete static activation,
publication, ordinary-access validity, and reverse destruction, and no backend
can observe preliminary or merely planned MIR.

### SI8 — Execute eager startup

**Purpose:** Make every initialized field available in the statically proven
dependency order before user entry.

- [ ] Extend x86-64 static planning for the complete stored type matrix while
      keeping one private aligned value slot per declaration and leaving
      instance layout, dispatch tables, callable ABI, and source visibility
      unchanged.
- [ ] Lower verified lifecycle transitions mechanically without adding a
      state load and branch to ordinary static accesses.
- [ ] Lower the program initializer as an ordinary private generated function
      so constructors, calls, allocations, ownership, arrays, and cleanup use
      existing instruction selection.
- [ ] Call the initializer from the existing host `main` wrapper after the ABI
      marker and before the selected Skald entry callable.
- [ ] Preserve deterministic symbols, section choice, alignment, relocations,
      literal data, panic pools, dumps, and assembly across compiler processes.
- [ ] Retain structured backend errors for malformed MIR instead of silently
      fabricating states, slots, or lifecycle order.

**Tests:** Focused backend tests for slot layout and symbols,
primitive/object/optional/shared/array initialization, initializer side
effects, dependency ordering, entry ordering, absence of per-access guards,
assembler acceptance, native startup order, exact assembly, and malformed
MIR.

**Exit criteria:** Successful startup publishes every declaration exactly once
before user entry in the checked dependency order, with no ordinary static-
access runtime overhead.

### SI9 — Execute reverse normal-return shutdown

**Purpose:** End program-owned resources deterministically while preserving
the selected entry result and existing abrupt-termination behavior.

- [ ] Lower the verified program finalizer through ordinary complete-object,
      optional, shared-owner, and array cleanup operations in exact reverse
      activation order.
- [ ] Preserve the verified semantic `destroying` and `dead` transitions in
      MIR order; keep not-yet-destroyed dependencies live during later-field
      destructors without requiring emitted per-slot state bytes.
- [ ] Include initializer-free optional, optional-shared, and array fields so
      contents acquired by replacement during execution are cleaned normally.
- [ ] Spill the selected entry's `i64` result in the host wrapper, call the
      generated finalizer after the entry callable returns, restore the result,
      and preserve System V stack alignment and callee-clobber rules.
- [ ] Keep panic, initializer failure, entry panic, destructor panic, signals,
      and foreign process termination non-unwinding with no remaining-static
      cleanup attempt.
- [ ] Cover destructor reads of still-live dependencies, compile-time
      rejection of current or already-dead dependencies, shared last-owner
      destruction, reverse array element cleanup, and inherited complete-
      object destruction.
- [ ] Confirm the feature needs no public runtime ABI change; if evidence
      contradicts that assumption, stop this task and revise the runtime
      contract before changing C symbols.

**Tests:** MIR and backend shutdown order, wrapper result preservation,
primitive no-op transitions, inline and inherited destructor effects,
optional replacement, shared release counts, arrays, dependency diagnostics,
panic/no-unwind boundaries, assembly and native execution, and deterministic
goldens.

**Exit criteria:** Normal entry return destroys the current contents of every
static field once in the frozen reverse order and returns the original `i64`;
abrupt termination retains the documented no-unwind behavior.

### SI10 — Harden and publish initialized static fields

**Purpose:** Audit the complete lifecycle matrix, update authoritative living
contracts, and remove migration-only assumptions.

- [ ] Add end-to-end success goldens for primitive constants, strings, exact
      inline objects, constructor arguments, shared singletons, optional
      caches, empty/nonempty arrays, imports, inherited aliases, privacy,
      side-effect order, dependencies, replacement, and reverse destruction.
- [ ] Add compile-failure goldens for malformed syntax, invalid storage,
      direct and transitive dependency cycles, destructor dependency cycles,
      conservative dynamic-dispatch effects, copy or overload failure,
      private access, and wrong-kind uses.
- [ ] Audit source-reachable assertions, effect-graph coverage, generated
      symbol privacy, phase determinism, normal-return cleanup, and artifact-
      free repeated compilation.
- [ ] Update the implemented grammar, static-field language contract, classes
      and lifecycle, functions and evaluation order, shared ownership, arrays,
      errors, status matrix, phases and IR, backend, runtime ABI, testing,
      debugging, README summaries, and a focused sample where useful.
- [ ] Remove the zero-default-only title and process-retention language only
      when the new behavior is executable across the supported target.
- [ ] Run `make check`, `make msrv-check`, `make robustness-long`,
      `make golden-determinism-test`, and `git diff --check`.
- [ ] Close and archive this roadmap only after every task, test plan, and exit
      criterion is complete.
- [ ] Preserve this combined design and delivery record in the archive while
      making living language and compiler documents the sole authority for
      implemented behavior.

**Tests:** Focused suites above followed by the full repository and extended
quality gates.

**Exit criteria:** Initialized static fields and deterministic shutdown are
fully specified, verified, executable, documented, deterministic, and free of
the old process-retention and zero-default-only implementation assumptions.

## Ordering and dependencies

The source and identity contract comes first because type checking cannot own
expressions whose callable-local IDs or privacy context are ambiguous. Typed
stored-value selection comes next so later phases never repeat overload, copy,
optional, shared, or array decisions. Preliminary MIR then exposes calls,
temporaries, cleanup, dispatch sets, and lifecycle operations in the compiler's
most complete target-independent form. Whole-program MIR effects settle
dependencies before final lifecycle state and verification. Startup execution
establishes valid owning values before shutdown can be tested; shutdown then
composes existing cleanup and finalizer machinery.
Broad goldens and living documentation come last, when the complete observable
contract is executable.

SI1 and SI2 must land sequentially. SI3 needs completely selected typed calls
and lifecycle operations before it can expose sound analysis input. SI4 scans
that preliminary MIR rather than duplicating HIR lowering, and SI5 converts its
summaries into a checked plan. SI6 defines and verifies the certificate schema
without reconstructing the graph; SI7 consumes the plan to synthesize final
MIR. SI8 and SI9 share backend slot metadata and the entry wrapper, so shutdown
depends on startup rather than developing a parallel wrapper. No task depends
on a runtime ABI revision, exception support, threading, or a module-system
redesign.

## Required quality gates

- Focused syntax, resolution, type-check, HIR, MIR, verifier, backend, CLI, and
  native golden tests in the task that changes each owner.
- Exact AST, resolved, HIR, MIR, assembly, diagnostics, stdout, stderr, and exit
  observations where relevant.
- `make check` before every task handoff that changes executable behavior.
- `make msrv-check` when Rust targets or supported syntax change and at
  roadmap closeout.
- `make robustness-long` after syntax/recovery changes and at closeout.
- `make golden-determinism-test` for lifecycle-order and cross-process output
  changes and at closeout.
- `git diff --check` for every handoff.
- No CI configuration; the repository Makefile remains the automation
  interface.

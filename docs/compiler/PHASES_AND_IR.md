# Compiler Phases and Intermediate Representations

Status: authoritative for current compiler phase inputs, products, invariants,
verification boundaries, deterministic dumps, and phase-facing public paths.
Explicitly marked frozen extensions define representation invariants selected
for implementation but not yet present in current phase products.
Source-visible meaning remains owned by the
[language documentation](../language/README.md). Shared ownership's
cross-phase invariants are specified separately in the
[shared-ownership compiler and runtime contract](SHARED_OWNERSHIP.md).
The optional-value phase and IR additions are specified in the
[optional-values compiler contract](OPTIONAL_VALUES.md). Optional tokens,
source-shaped AST nodes, and flat resolved identities are implemented;
primitive and exact-class optional owning locals, fields, internal
parameters/results, and temporaries additionally have typed HIR, verified MIR,
conditional lifecycle operations, dynamically guarded checked payload views,
and executable backend lowering.
Capture-free function types, exact callable references, trivial scalar
storage, receiverless indirect calls, verified address provenance,
exact-signature static effects, and retention are implemented across these
same phase products under the
[function-value compiler contract](FUNCTION_VALUES.md).

## Pipeline contract

The target-independent compiler path is:

| Responsibility | Public entry | Product |
|---|---|---|
| Source ownership | `source::SourceDatabase` | source IDs, files, text, spans, line locations |
| Lexing | `lexer::lex` | `LexOutput`: tokens and diagnostics |
| Parsing | `syntax::parse` | `ParseOutput`: source-shaped AST and diagnostics |
| Resolution | `resolve::resolve`, `resolve::resolve_module_graph` | `ResolveOutput`: resolved program and diagnostics |
| Type checking | `typeck::type_check` | `TypeCheckOutput`: diagnostics and optional typed HIR |
| Preliminary MIR lowering | `mir::lower_preliminary_hir` | closed-world `PreliminaryMirProgram` with unplanned static lifecycle bodies |
| Preliminary MIR verification | `mir::verify_preliminary_mir` | opaque, read-only `VerifiedPreliminaryMirProgram` required by static-lifecycle analysis |
| Static effect inference | `passes::static_lifecycle::infer_static_effects` | deterministic direct and transitive static effects from sealed `VerifiedPreliminaryMirProgram`, with witnesses for every callable and implicit lifecycle operation |
| Static lifecycle planning | `passes::static_lifecycle::plan_static_lifetimes` | consumes sealed preliminary MIR and returns `PlannedMirProgram` with a planning-only analysis report plus compact authority and deterministic activation/reverse-shutdown order |
| Planned MIR verification | `passes::static_lifecycle::verify_planned_mir` | opaque `VerifiedPlannedMirProgram` after exact authority issuance verification |
| Static lifecycle synthesis | `passes::static_lifecycle::synthesize_static_lifecycle` | final `MirProgram` with program-owned activation, publication, and reverse-destruction regions |
| Ordinary MIR lowering | `mir::lower_hir` | target-independent `MirProgram` when no explicit static lifecycle work exists |
| MIR passes | `passes::run_mir_pipeline` | read-only `VerifiedFinalMirProgram` or structured `MirPipelineError` |

`driver::compile_request_to_assembly` composes provider normalization,
reachable graph loading, these phases, target selection, and backend emission.
`driver::compile_source_to_assembly` is its in-memory singleton convenience
surface. Their observed counterparts emit typed phase and compilation-total
events without entering the products. All forms stop after any source phase
that produced an error. Successful
type checking always produces HIR; failed type checking
produces no HIR. HIR lowering represents every typed operation directly in
target-independent MIR. The
[backend and target contract](BACKEND.md)
defines how verified MIR is checked and realized for a selected target; driver
behavior is separate from the target-independent phase model and is defined by
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md).

Phase products are request-owned values. The compiler has no global source,
diagnostic, identity, or IR registry.

The frozen [structured reporting contract](REPORTING.md) observes this
pipeline through a request-scoped interface without entering phase products or
changing their ownership. Direct public phase paths remain independent from
reporting; observation belongs to driver composition. Details metrics inspect
stable product tables and executable MIR shape only after the observer requests
them, while already-known loader and pass execution counts remain sidecars of
their owning orchestration boundaries.

`HirForIn` carries one exhaustive execution plan. `Protocol` retains the
selected iterable receiver, state, optional result, item lifecycle, and call
plans. `PrimitiveRange` is selected only for an immediately consumed canonical
`u8`, `u64`, or `i64` structural range source and retains ordered endpoints,
minimal exact loop evidence, comparison/increment operations, and the item
epoch. Preliminary-MIR lowering
erases the latter to current/end scalar storage, one less-than branch, one
same-typed increment before source body entry, ordinary jumps, and explicit
cleanup. No range aggregate, optional, protocol call, range MIR operation, or
range identity survives that boundary; subsequent static-effect, lifecycle,
pipeline, verification, and backend phases remain source-loop agnostic.

The frozen [generic-class specialization contract](GENERIC_CLASSES.md) inserts
a template-resolution and deterministic closed-specialization responsibility
between syntax/module declaration collection and the ordinary resolved
program. Its defining trust boundary is that every published ordinary class,
type, member, and body is closed. `ResolvedClassDeclaration`, HIR, MIR,
verification, and backends do not gain an unresolved parameter variant or a
runtime type-argument protocol. The source-shaped syntax nodes, template
semantics, deterministic closed identities, declarations, and complete
ordinary resolved bodies are implemented and explicitly gated before typed
HIR; lifecycle and every later phase consume only those closed ordinary
products.

The implemented [generic-interface specialization contract](GENERIC_INTERFACES.md)
extends the same pre-HIR layer. Declaration collection now exposes stable
interface-template, template-requirement, and owner-correct type-parameter
identities as resolution-only products, and module declarations distinguish
them from ordinary interfaces and class templates. One coordinated
class/interface worklist closes every requested application into ordinary
`InterfaceId` and `InterfaceRequirementId` values before executable body
resolution. Exact claims and bounds, interface aliases, shared owners,
optionals, arrays, casts, type tests, and structural calls then reuse their
ordinary resolved and type-checking paths. Ordinary resolved executable
declarations, HIR, MIR, verification, and backends remain free of
parameter-bearing interface terms, dictionaries, or runtime type arguments;
successful HIR construction explicitly asserts that every executable class
claim is ordinary. Preliminary and final MIR verification, static effects,
complete-object metadata, exact witness calls, ownership cleanup, and native
x86-64 execution now consume these closed identities unchanged.

Resolved IR, typed HIR, and MIR carry the same validated
`module::ProgramModuleTable`: dense `ModuleProvenance` in `ModuleId` order plus
the selected entry module. Every top-level function, class, and interface
declaration carries its owning `ModuleId`; members derive their module through
the enclosing class or interface. These additions preserve the existing flat
whole-program declaration and definition tables. Lower phases use typed
identities and never repeat module-path or source-name lookup.

String literals retain deterministic decoded-data identities from resolution
through HIR. MIR adds program-local immutable literal-data declarations, a
distinct immortal static shared-owner producer, and identity-selected complete
`Str` descriptor publication. Verification proves the data, array, and field
identities and consumes the temporary backing owner before ordinary class
lifecycle continues; target offsets and static section layout remain backend
responsibilities.

`resolve::resolve(&CompilationUnit)` remains a single-source adapter. It
synthesizes one request-local logical `main` module around the AST's `SourceId`
and otherwise uses the normal program resolver.
`resolve::resolve_module_graph(&ModuleGraph)` collects all reachable parsed
modules in canonical logical-path order, allocates declarations and members in
source order, and produces the same flat resolved program. Per-module indexes
retain visibility and expose only directly owned public declarations. The
selected entry module alone supplies the prospective `main`; other declarations
named `main` are ordinary functions.

Graph resolution gives each module its own ordinary top-level declarations and
a separate namespace of direct module bindings. An unaliased module import
binds its complete canonical path; an alias binds one identifier. One
current-module lookup service resolves qualified signatures, hierarchy uses,
interface claims, calls, construction, allocation, casts, and type tests to
the target module's directly owned public declaration identity. Exact direct
imports are required: absolute, descendant, and transitive paths do not create
bindings. Resolved dumps retain local binding spelling plus canonical module
ownership; HIR and lower phases contain only selected dense identities.

The `module` facade provides validated exact-case `ModulePath` values,
request-local module provenance vocabulary, and distinct `ModuleId`,
`ProviderId`, and `PackageId` identities. The `driver` facade exposes a typed
`CompilationRequest` containing entry, root, standard-library, target,
artifact, working-directory, and installed-standard-library inputs. Requests
can expand their active ordinary and standard-library roots into explicit
provider configurations. The `module` facade normalizes and coalesces those
roots, assigns deterministic request-local provider/package identities, and
resolves one exact logical path to missing, unique, ambiguous, or structured
filesystem failure. It can then select a logical or positional entry, create
an isolated outside-root singleton when required, acquire and parse only the
reachable import closure, assign dense module/source identities in canonical
logical-path order, accept cyclic multi-module edges while rejecting direct
self-imports, and return an inspectable `ModuleGraph`.
Discovery caches source text before canonical final parsing, so recursive
discovery order does not determine final identities. The graph resolver
preserves that canonical module order when allocating all semantic identities.
The request pipeline consumes this graph directly; the source-text convenience
entry remains isolated from filesystem discovery.

The lexer and parser recognize module punctuation, imports, top-level
visibility, and qualified declaration spellings. The AST
retains unresolved path components and all diagnostic-relevant separator and
introducer spans without choosing a module binding or declaration leaf.
The source-text convenience resolver emits `RES023` when an import or
qualified name requires the module/root context that only a
`CompilationRequest` supplies.
Request graph resolution constructs direct module and selective ordinary
bindings and resolves their uses before lower phases.

The parser also recognizes contextual, bodyless top-level `intrinsic fn`
declarations as a distinct AST shape. Resolution retains intrinsic linkage
separately from Skald definitions and external links, validates that the only
recognized identity is the exact public `std::error::panic` signature, and
resolves every ordinary import/qualification spelling to its `FunctionId`.
Declaration metadata can pass through HIR and MIR without a definition or
foreign symbol. A call statement becomes a terminating `HirPanic` and then a
no-successor `MirTerminator::Panic`; using it in expression position emits
`TYP041`. MIR verification independently rejects any residual direct call to
intrinsic metadata.

The [standard I/O compiler contract](IO.md) reserves five additional canonical
intrinsics under `std::io`. The closed registry, exact declaration checks,
array-alias checking, dedicated typed HIR, and verified target-independent MIR
are implemented. MIR preserves exact byte-array places, access, backing
anchors, length-inclusive checked range offsets, and one exact `i64` result.
The x86-64 target mechanically extracts verified byte pointers and lengths,
marshals scalar arguments and signed results, and calls the exact version-9
runtime symbol selected by the MIR operation. Operation selection never
depends on source spelling after resolution.

Static fields follow the same phase ownership. Syntax retains a distinct
class-member shape, modifier spans, and an optional initializer expression
with its exact `=` and expression spans. Resolution allocates dense class-owned
field identities in source order and callable-like initializer identities
derived from them. After the complete program namespace, hierarchy, overload
candidates, and string language items exist, a delayed pass resolves each
initializer in a receiver-free context owned lexically by the declaring class.
Resolved IR retains selected declarations, calls, dispatch families, and
source spans without reconstructing names or order later.

For initializer-free declarations, type checking validates the complete
all-zero live-value set and emits identity-based typed places for all accepted
operations. For explicit declarations, it separately validates ordinary
stored-value capability and checks the expression in a receiver-free,
parameter-free callable context owned by the declaring class. HIR retains one
field-derived initializer identity, destination type, and
`HirStoredValueInitialization`, including selected construction, copy, owner
transfer, optional or array behavior and full-expression ownership metadata.
It is direct initialization of uninitialized storage rather than assignment.

`PreliminaryMirProgram` privately owns the ordinary closed-world program and
one independently identified body per explicit initializer. Each body uses the
ordinary target-directed lowering paths for calls, construction, copy,
optionals, shared owners, strings, arrays, temporaries, and cleanup. One CFG
edge separates destination completion from post-publication full-expression
cleanup. The preliminary product retains ordinary bodies, virtual families,
interface conformances, destruction plans, array lifecycle metadata, and
source spans. Shared-owner views expand through one canonical API to a finite
set of compatible class or array lifecycle implementations.

The structural preliminary verifier checks the ordinary program and every
initializer body's identities, types, selected targets, ownership metadata,
control flow, exact destination, and publication boundary without assuming a
global activation order. Success consumes raw `PreliminaryMirProgram` and
returns the opaque, read-only `VerifiedPreliminaryMirProgram` seal. Static
effect inference and lifecycle planning require this seal, so their internal
dependency adapter may remain infallible without accepting malformed raw MIR.

`passes::static_lifecycle::infer_static_effects` then scans every instruction,
terminator, static-rooted place, direct or dynamic call, selected initializer
or copy operation, complete finalizer, shared release, optional cleanup, and
array lifecycle operation. Its exhaustive enum matches make a new MIR
operation a compile-time maintenance point. Virtual and interface calls and
shared-owner finalizers expand to closed-world target sets. Compiler-generated
copy, complete-finalizer, and array operations have distinct graph nodes, so
their backend-realized calls do not disappear behind user body identities.
Callable-address operations are separately inventoried by exact
`FunctionTypeId` and `CallableId` without adding an effect. Each receiverless
indirect call expands to every matching address-taken target through an
`IndirectCall` edge. The inventory retains first reference spans in the
planning report; it is neither executable proof nor a callable-retention
contract.
The private `passes::static_lifecycle::analysis` owner contains graph
extraction, candidate inventories, direct evidence, solved summaries,
recursive-component results, normalized-root closure, witness selection, and
their deterministic comparison helpers. The public
`passes::static_lifecycle` facade exposes only supported analysis, planning,
synthesis, and verification APIs.
The pass condenses recursive components, propagates field sets over the
component DAG, and retains minimum-call-edge, deterministically tied witnesses
for each field in every node summary. Distinct access-kind and root-phase
witnesses remain separate, so a lifecycle destination write cannot hide an
ordinary access to the same field. Direct evidence keeps source span and
initializer publication phase; a transitive initializer witness carries the
phase of its first call or lifecycle edge. The stable
`passes::static_lifecycle::dump_static_effects` renderer exposes this product.

`passes::static_lifecycle::plan_static_lifetimes` runs inference once and
builds a second graph over every canonical static declaration. An edge from
`T` to `F` records that initialization or eventual-value destruction of `F`
may access `T`. Destruction roots come from the stored type even for
initializer-free optional, shared, and array slots, because ordinary execution
may replace their values. Each edge retains its startup or shutdown root,
declaration and access spans, access kind, and call/lifecycle witness.
Iterative strongly connected component analysis remains separate from
callable recursion and reports deterministic `STA001` self-dependency or
`STA002` cycle source diagnostics. An acyclic graph is topologically ordered
with canonical field identity as the ready-node tie-breaker, and shutdown is
always exposed as a derived reverse iterator over activation.

An initializer's own destination write is lifecycle-owned rather than a
self-dependency. Other pre-publication accesses to that field are invalid;
cleanup proven to start after publication may use the newly live field.
Accesses to other fields in either region remain ordinary dependencies.

`PlannedMirProgram` privately owns preliminary MIR, one canonical definition
table sorted by stable static-field identity, one activation-order vector,
compact baseline authority, and a `StaticLifecyclePlanningReport`. Shutdown,
positions, required dependency pairs, source-rich dependency evidence, and
planned transition views are derived rather than stored. The report owns
direct effects, conservative targets and summaries, exact-signature
candidates, recursive-component count, spans, and witnesses from which
inspection can reconstruct the same evidence deterministically.
`verify_planned_mir` independently re-extracts normalized root facts, requires
exact authority, derives dependency pairs from authority and definitions, and
checks canonical definition and activation coverage plus dependency order.
Stable `dump_planned_mir` and `dump_static_lifetime_plan` render derived
positions, reverse shutdown, dependencies, and transitions without adding
mirrors to the product. The private ownership boundary prevents final MIR
passes and backends from consuming analysis evidence or unplanned initializer
bodies.

`verify_planned_mir` consumes the draft planned product and is the only public
constructor of `VerifiedPlannedMirProgram`. `synthesize_static_lifecycle`
accepts only that sealed product, moves every
initializer body unchanged into the planned activation order, and produces the
only final `MirProgram` used by the ordinary MIR pipeline. The planning report
is dropped at this boundary. Synthesis constructs structured regions directly
from definitions, activation order, initializer publication metadata, and
stored types; it does not build or retain parallel flat transition vectors.
Final MIR retains the canonical planned data plus compact baseline authority.
A zero-default field has one direct activation-to-live transition at its
planned position. An explicit field has begin and publish transitions, with
publication fixed to the checked CFG edge before its preserved
post-publication full-expression cleanup. The coordinator also owns
exact-reverse destruction regions whose begin and finish transitions surround
type-selected no-op, complete-object, optional-class, shared-owner,
optional-shared, or array cleanup semantics.

`verify_synthesized_mir` independently checks final definitions, exact region
coverage and order, unique legal transitions, initializer publication and
cleanup control flow, ordinary ownership/array/lifetime rules, lifecycle
destination non-escape, and the final root-effect realization using only final
MIR. Coordinator verification walks the same structured regions consumed by
the backend and checks them directly against definitions and activation order;
there is no mirrored transition-vector equality check. Realization verification
re-derives closed-world targets and normalized effects, requires exact
contractual lifecycle-root coverage and a subset of baseline authority, then
checks realized dependencies against the frozen activation order.
`passes::verify_final_mir` owns this combined verifier, then derives
target-independent whole-world reachability from the structurally and
lifecycle-valid program. It constructs one opaque, read-only
`VerifiedFinalMirProgram` that owns both exact MIR and the facts derived from
it. `run_mir_pipeline` calls that boundary initially and after every changed
target-independent transformation. `BackendInput` accepts only the sealed
result and does not repeat target-independent verification.
No runtime access guard is represented; certified ordinary static accesses are
valid because their targets are earlier in activation and later in shutdown.

Preliminary and planned products remain unavailable to ordinary passes and
backends. The driver reports lifetime graph failures as ordinary source
diagnostics after preliminary verification, independently of malformed-MIR
verification errors. A valid explicit initializer reaches final verified MIR,
ordinary x86-64 instruction selection, and the private dependency-ordered
program initializer called before entry. The backend also lowers the verified
reverse destruction regions into one private program finalizer, using existing
complete-object, optional, shared-owner, and array lifecycle helpers. The host
wrapper preserves the entry result across that normal-return call. The
source-visible lifetime rule is owned by the [static-field
contract](../language/STATIC_FIELDS.md#initialization-and-lifetime).

### Frozen static-lifecycle certificate direction

The implemented design keeps lifecycle planning before optimization and uses a
root-effect authority relation rather than exact cross-phase analysis-shape
equality. Its complete rationale and delivery history are preserved in the
[frozen design record](../archive/STATIC_LIFECYCLE_CERTIFICATE_DESIGN_PROPOSAL.md).

Planning now issues a compact MIR-owned baseline authority computed by a
checker-oriented normalized root-effect analysis over the raw extracted graph.
It inventories roots from static definitions and stored types, walks direct and
implicit closed-world edges without trusting solved summaries, and preserves
target field, access kind, propagated root phase, and lifecycle-owned status.
The authority has private construction, canonical sorted unique storage, and a
public read-only inspection API.

`verify_planned_mir` independently extracts preliminary MIR, recomputes the
normalized root facts, and requires exact authority equality. It derives
required dependency pairs from the authority and lifecycle definitions,
and checks every pair against activation order. The solved graph is not a
second proof oracle. `STA001` and `STA002` diagnostics still use its
deterministically selected evidence during planning. Planned dumps identify the
compact proof as `StaticLifecycleBaselineAuthority` and render the separate
planning report's analysis and witnesses.

`verify_synthesized_mir` is the distinct final realization checker. It extracts
the actual final program and moved initializer bodies, re-derives virtual,
interface, indirect, direct, and implicit lifecycle targets, and computes the
normalized facts for every root required by the final definitions. Required
root coverage remains exact, while each realized fact set need only be a
subset of its immutable baseline authority. It independently derives realized
dependency pairs and checks them against the frozen activation order. Direct
effects, call edges, source spans, witnesses, node inventory, and address-taken
candidate inventory are no longer compared across the final boundary.

Solved analysis, direct graph shape, candidate inventory, recursive-component
metrics, access spans, and witness inputs live only in
`StaticLifecyclePlanningReport`; source-rich dependencies are reconstructed
from that report when inspected. Synthesis drops the sidecar. Final
`MirStaticLifecycleProof` owns only immutable baseline authority, so analysis
evidence cannot constrain graph reshaping or reach backend-consumable MIR.
The MIR-owned lifecycle schema is separately divided into compact proof,
canonical plan, structured coordinator, and phase-product modules behind
the existing `mir` facade. Stable lifecycle-root identities and normalized
authority remain MIR values because they cross planning, optimization,
verification, and backend boundaries; source-rich analysis evidence does not.

For every explicit static initializer and implicit class or array lifecycle
operation used to activate or destroy a static field, preliminary analysis
issues an exact baseline authority. One authorized fact consists of target
static field, access kind, root phase, and whether the access is the
lifecycle-owned unpublished initializer destination. Source span, witness
path, directness, intermediate callable, call-edge kind, and target-set shape
are evidence or analysis details rather than fact identity.

Let `effects(P, r)` be the conservative closed-world normalized facts reachable
from lifecycle root `r` in MIR product `P`, and let `B[r]` be its immutable
baseline authority. Planned-MIR issuance must establish:

```text
effects(preliminary_mir, r) = B[r]
```

Final synthesis and effect-changing MIR transformations must establish:

```text
effects(final_mir, r) subset-of B[r]
```

Final verification also derives the dependencies of the realized facts and
checks them directly against the frozen activation order, including existing
self-access, publication-phase, and lifecycle-destination rules. This accepts
effect removal, target narrowing, and inlining even when their direct graph
shape changes, while rejecting a newly reachable field/access/phase fact.
Optimization never replans lifecycle order or changes source diagnostics.

Future target-independent MIR passes must declare one of two lifecycle
classes in their own implementation and tests:

- A lifecycle-effect-preserving pass proves that it cannot change static
  accesses, reachable control flow, lifecycle operations, or possible call
  targets. It may preserve an existing seal only through an API that encodes
  that proof; no such production pass needs that API today.
- A lifecycle-effect-changing pass works on raw `MirProgram`, invalidates any
  earlier final seal, and returns to `verify_final_mir`. This includes effect
  removal, dead-code removal, devirtualization or other target narrowing,
  inlining, and call-graph reshaping even when the expected effect set only
  shrinks.

The current pipeline has two registered production transformations and no
shared analysis cache. Its immutable registry makes dead-pure-definition
elimination and whole-world reachability discoverable, while the supported
`default` profile executes dead-pure-definition elimination followed by
whole-world reachability;
`none` records one final verification execution and returns the unmodified
sealed product required by every backend. A changed default product is
immediately reverified.
Whole-world compilation makes target re-derivation finite, and single-threaded
generated execution requires no synchronization or runtime lifecycle guard at
this boundary.

Target expansion is re-derived from each MIR product. Whole-world compilation
makes virtual, interface, exact-signature function-value, copy, finalization,
cleanup, and array-lifecycle target sets finite. Function-value candidates are
an analysis input; callable retention is owned by the whole-world reachability
boundary below rather than the lifecycle certificate. No unknown
external Skald target may be assumed effect-free.
Single-threaded generated execution requires no runtime guards, atomics, lazy
initialization, or synchronization state.

The baseline authority is planner-owned and opaque to optimization passes.
Planned issuance and final realization use distinct verifier entry points, and
the backend accepts only a final product checked for ordinary MIR structure and
lifecycle realization. Passes that may change static access, control-flow
reachability, lifecycle operations, or possible callees invalidate the derived
realization and cause central reanalysis; all pipelines verify once at the
backend boundary regardless of pass declarations.

The promoted schema direction has one canonical representation for each
durable fact:

- one lifecycle definition per certified active static field;
- one activation-order vector, with shutdown and position indices derived;
- one immutable sorted active-field authority;
- one immutable baseline authority map keyed by lifecycle root; and
- structured activation and destruction regions as the executable coordinator
  form, with flat transitions available only as derived dump views.

Required dependency pairs derive from authority and definitions. Direct effect
graphs, target inventories, SCC metrics, solved per-callable summaries, source
witnesses, and diagnostic paths remain pass-owned analysis or reporting data
rather than executable MIR certificate identity. The migration preserves
complete declaration identity, initialization modes and types, publication
dominance, destination non-escape, exact-reverse destruction over the
certified set, deterministic dumps, and the existing `STA001` and `STA002`
diagnostic behavior.

### Frozen reachability-gated static lifecycle direction

Status: **semantic cutover implemented**. The compiler computes and
independently re-solves the exact activation closure at the accepted
preliminary-MIR boundary. Its proof, planner, synthesis, dumps, and verifiers
carry the exact active subset, and central final verification rejects reachable
accesses outside that certified subset. The source-visible contract is owned by
[Static Fields](../language/STATIC_FIELDS.md#frozen-reachability-gated-activation-direction),
the complete decisions by the
[frozen design record](../archive/REACHABILITY_GATED_STATIC_LIFECYCLE_DESIGN_PROPOSAL.md),
and delivery by the
[completed roadmap](../archive/REACHABILITY_GATED_STATIC_LIFECYCLE_ROADMAP.md).

The accepted phase boundary inserts one mandatory target-independent static-
activation analysis after preliminary MIR is structurally verified and before
static-lifecycle planning. It is language semantics, not a registered final-
MIR pass. `none`, `default`, pass exclusions, target selection, and future
compiler-internal parallelism must all observe the same result. Preliminary MIR
remains definition-complete: inactive declarations and initializer bodies are
still lowered and checked before the activation boundary.

The analysis reuses the shared target-independent execution-dependency
extractor and adds exhaustive ordinary static-place access records. It computes
one iterative least fixed point over activation-reachable execution nodes and
active `StaticFieldId`s. The selected entry roots execution; reached reads,
writes, replacements, and borrows activate fields; each newly active field adds
its explicit initializer and eventual-destruction lifecycle execution. The
field's own lifecycle-owned unpublished destination is not an activation edge.
All structural blocks and the current full virtual-family, interface-
conformance, exact-function-type, copy, assignment, destruction, optional,
shared-owner, and array target rules are part of the frozen semantic analysis.

One immutable analysis product owns the canonically sorted active set,
activation edges, conservative targets, counts, and canonical first triggers
and witnesses. Planning reports and dumps may retain explanations, but proof
identity remains compact. Planned-MIR verification re-extracts preliminary MIR
and re-solves activation without trusting the planning report, solved
summaries, witness paths, or planner-issued compact set. It requires the report
and lifecycle certificate to equal the independently recomputed field set
before issuing the verified phase product.

Planned and final lifecycle MIR contain definitions, order, initializer bodies,
activation regions, destruction regions, and root authority for exactly the
active subset. Program-level declarations, field IDs, classes, types, layouts,
and preliminary bodies are not compacted. Shutdown and positions remain
derived from the one activation-order vector. `STA001` and `STA002` are computed
only over the active dependency graph, while every ordinary source and
preliminary-MIR diagnostic remains definition-complete.

Final verification independently checks exact coordinator coverage and
monotone realization of active baseline authority. Canonical whole-world
reachability now retains the exact static-place accesses from reachable
execution nodes together with their selecting root and dependency explanation.
Central verification independently reconstructs the program roots and requires
every such access to target the lifecycle certificate's active authority, every
active field's storage to remain reachable, and every activation or shutdown
root to remain present. Final MIR containing any static declaration must retain
a lifecycle coordinator and activation authority, including when the certified
active set is empty, so deleting the complete certificate cannot masquerade as
an empty lifecycle. A physically retained but unreachable body may mention
an inactive declaration and is still fully structurally verified. Any changed
pass invalidates and rebuilds final MIR, reachability facts, static-access facts,
and lifecycle realization together; a pass that makes an inactive access
reachable, loses active lifecycle work, or adds an unauthorized active-root
effect fails central verification.

Optimization may remove every surviving ordinary access to an already-active
field, narrow executable targets, or delete unreachable bodies without
replanning its lifetime. This deliberately freezes source-side initializer and
destructor effects before selectable optimization. Whole-world compilation
makes the closure finite, and single-threaded generated execution needs no
runtime initialized-state flag, access guard, atomics, locking, or lazy-
initialization protocol.

The activation analysis, lifecycle planner, final reachability verifier, and
backend remain separate owners behind concise facades. Shared extraction must
not fork into a second call/lifecycle walker. Activation dumps are deterministic
inspection products; structured reports receive only typed already-known
counts; neither becomes certificate identity or pass logging.

The migration boundary has the following durable ownership map:

| Concern | Owner |
|---|---|
| Neutral execution identities, possible targets, direct static-place records, implicit lifecycle expansion, and final executable closure | `passes::reachability` |
| Complete propagated preliminary static effects and source-rich lifecycle evidence | `passes::static_lifecycle::analysis` |
| Entry-rooted field-activation policy, canonical reasons, witnesses, and counts | `passes::static_lifecycle::activation` |
| Dependency graph, diagnostics, activation order, and planning report | `passes::static_lifecycle::plan` |
| Active coordinator construction | `passes::static_lifecycle::synthesize` |
| Certificate issuance and final realization checks | `passes::static_lifecycle::verify` plus central final-MIR verification |
| Phase sequencing and typed observation adaptation | `driver` and the MIR pipeline boundary |
| Private slots, initializer/finalizer lowering, and generated-symbol retention | the selected backend |

The private activation owner now contains the immutable vocabulary, coupled
deterministic solver, borrowed queries, canonical witnesses and first triggers,
per-cause conservative target counts, and a focused deterministic dump. It
consumes the same extracted execution dependencies, direct static accesses,
scoped callable-address formations, indirect-call sites, entry policy, and
static cleanup target resolver as target-independent reachability. Static-
lifecycle planning extracts those facts once and computes the semantic result.
Planned verification separately extracts and solves them again and rejects a
mismatching report or compact certificate. The compact final proof stores that
exact authority beside only the lifecycle roots required by it. Definitions,
dependency order,
derived shutdown/transitions, moved initializer bodies, and coordinator regions
must exactly cover the authority; declarations and preliminary initializer
bodies remain complete and keep stable IDs. Internal empty and sparse fixtures
exercise this boundary. Source-rich triggers, witnesses, edges, target counts,
and summary counts remain untrusted planning-report data; they are not
certificate authority or public observation state.

### Dense callable-local MIR identity rewriting

The implemented target-independent structural editing boundary follows the
static-lifecycle certificate foundation. Its rationale and delivery history are
preserved in the archived
[design record](../archive/DENSE_MIR_IDENTITY_REWRITING_DESIGN_PROPOSAL.md) and
[roadmap](../archive/DENSE_MIR_IDENTITY_REWRITING_ROADMAP.md). The current
pipeline still has no production MIR transformation.

Committed MIR remains dense. `StorageId`, `ValueId`, `BlockId`, and
`PathConditionId` continue to contain their callable owner and direct vector
position, and verified consumers may continue to index callable tables without
building sparse maps. Program-level semantic identities and existing
source-semantic `BindingId` provenance are not renumbered.

The implemented private edit state moves one callable's storage, values,
blocks, and path conditions into stable sparse slots with tombstones. Blocks
retain their instruction vectors and have an explicit order independent from
allocation; new blocks require append, before, or after placement. Logical
records use ordered tombstones, path-condition creation requires an already
allocated earlier parent, and a private registry discovers optional guards
through the exhaustive traversal without adding a committed guard table.
Allocation is callable-local and monotonic, so deletion never renumbers a live
edit slot. This state is not a `MirBody` or `MirProgram` and has no path into
ordinary verification, static-lifecycle analysis, dumping, or a backend.

The implemented common-state commit is atomic and deterministic. Storage and
values retain surviving slot order followed by allocation order; blocks use
explicit editor order; path conditions retain parent-before-child order;
optional guards are canonicalized
from live guard slots; and logical records retain explicit relative order.
Commit constructs complete old-slot-to-new-ID maps, validates every live-order
entry and reference, rewrites declarations and attachments, and emits dense
tables whose IDs equal their positions. A deleted, unknown, duplicate,
missing-order, or foreign reference is a deterministic internal rewrite error
with its structural site. Commit consumes the private transaction and returns
the dense common callable state, five typed identity maps, and structured
retained/inserted/removed counts without logging, rendering, verifying, or
updating an external analysis. Callable header and static-publication
attachments join this atomic boundary through the definition adapters.
Compaction never guesses value substitution, edge forwarding, cascading
deletion, proof-metadata repair, or any other semantic transformation.

The implemented private `mir::rewrite` identity traversal is authoritative for
callable-local identity references. One shared structural kernel produces an
immutable observer and a mutable remapper, so read-only analyses borrow MIR
directly without maintaining a competing identity inventory or cloning a
callable. It covers declarations, receiver,
parameters, return storage, body entry, instructions, rvalues, arguments,
places, projections, all terminators, path-condition structure,
logical-expression provenance, optional-view guards, and static-initializer
publication blocks. It provides deterministic structural sites and a shared
callable-owner validator without changing production MIR. Identity-bearing
matches do not use wildcard variants or partial struct patterns, so a new
reference form forces compiler review. Each new callable-local identity or
reference form must update this traversal and its census coverage in the same
change; individual passes may not maintain competing remapping inventories.

Every future production transformation of valid final MIR must enter through
the supported `mir::rewrite` facade and its atomic program coordinator. Direct
dense-vector construction remains appropriate for initial append-oriented
lowering, while narrowly named test-only mutation helpers remain appropriate
for deliberately malformed verifier and backend fixtures; neither is a valid
optimization boundary.

Function, member, and static-initializer definitions retain their public MIR
shape. The implemented private owned adapters share common transaction and
commit logic while remapping each definition kind's attachments. Atomic
program rewriting consumes sparse function slots, callable-keyed members, and
lifecycle-ordered initializer CFGs through narrow crate-private ownership
transfer, commits every requested edit, and only then restores the containers.
Function holes, member identities, initializer activation order, and the
entire lifecycle plan, proof, activation, and shutdown coordinator remain
unchanged. The model gains no production `iter_mut` escape hatch. Initial
HIR-to-MIR lowering remains append-oriented and does not use the optimization
editor.

The implemented `mir::rewrite` facade publishes only crate-private program
rewriting, typed results/errors, block placement, logical-record handles, and
the callable editor. The editor provides live typed lookup and iteration,
monotonic allocation, explicit deletion, functional instruction-list and
terminator replacement, same-type value-use substitution, same-type storage
reference substitution, executable-edge redirection, and explicit block
ordering. Instruction positions exist only inside a borrowed rewrite snapshot
and cannot be retained as committed identities. Value substitution preserves
definition sites and checks callable ownership and exact MIR type, but callers
remain responsible for dominance and semantic equivalence. Storage
substitution does not rewrite callable header attachments, and edge
redirection does not rewrite body entry, publication, path, or logical
provenance.

These helpers state structural preconditions but do not claim semantic facts
such as effect freedom, ownership equivalence, or cleanup safety. They never
infer cascading deletion. Path conditions, logical records, optional-guard
pairs, storage liveness, and related proof operations must be explicitly
rebuilt, retained, or deleted by the transformation. Atomic dense commit then
rejects dangling or foreign structure, while `verify_final_mir` remains the
authority for dominance, lifetime, ownership, proof-metadata, and lifecycle
meaning.

Future inlining and specialization can use the implemented distinct two-phase
rehoming primitive. An owned immutable source snapshot preserves common MIR
without retaining program borrows. An import request names each selected
storage declaration and its explicit source-free destination kind, value,
block, path condition, logical record, and optional guard. It also supplies
typed storage, value, block, path, and guard substitutions for receiver,
parameter, result, entry, exit, or other references outside the selected
region. The importer validates the complete selection and substitutions,
allocates every destination identity before cloning executable nodes through
the exhaustive mapper, and publishes the cloned destination transaction only
if the whole import succeeds.

Every imported local identity is owned by the destination callable; no raw
source-local reference may survive. Program-level callable, type, field,
static, declaration, and lifecycle identities pass through unchanged.
Source-semantic `BindingId` values are never copied: source snapshots reject a
binding owned by a third callable, and selected storage is materialized with
`source: None` and the caller's explicit source-free storage kind. Complete
source-to-destination maps include both fresh allocations and explicit
boundary substitutions. Rehoming does not split call sites, transfer
arguments, merge returns, design cleanup, limit recursion, choose
profitability, or otherwise define an inliner.

All analyses keyed by pre-commit local IDs or instruction positions are
invalidated by default. Commit maps support attachments, tests, reporting, and
an immediately adjacent owner that explicitly updates its data; they do not
silently preserve arbitrary caches. Change counts are pass-owned structured
results, and the editor performs no logging or global mutation. Callable-local
allocation and explicit order keep output deterministic even if independent
compiler work is parallelized later. The generated Skald program remains
single threaded.

Only the target-independent pass pipeline may invalidate a
`VerifiedFinalMirProgram` for transformation. Its private ownership bridge
consumes the seal directly into the supported atomic whole-program rewrite
coordinator. A successful rewrite yields raw dense MIR plus callable-scoped
commit maps and change summaries; it cannot recreate the seal. The
transforming coordinator verifies raw input before invoking the rewrite,
distinguishes input-verification, structural-rewrite, and output-verification
failures, and returns rewritten output through `verify_final_mir`. That central
boundary remains authoritative for ordinary MIR semantics and immutable
static-lifecycle baseline realization. Exact internal schedules exercise the
same production multi-pass runner without registering a production
transformation.

Pipeline accounting records verification and pass executions at the point they
occur. Structurally successful commits contribute already-known processed and
changed callable counts plus retained/inserted/removed entity counts; the
editor emits no report text. Pass-owned integer counters retain deterministic
first-owner and first-counter order. The driver renders aggregate counts at
details level and typed occurrence records at trace level. The `none` schedule
retains byte-for-byte MIR, one final verification, zero pass executions, and
no pass-finished events; the default schedule runs nine pass occurrences in
the exact repeated order documented below.

This direction adds no SSA form, persistent instruction identity, public
common callable-body restructuring, dynamic pass registry, optimization-level
CLI, broader optimization suite, proof-provenance normalization, alias/effect
analysis, or backend virtual-register layer. Those remain separate decisions
that can consume the implemented pipeline boundary.

### Selectable final-MIR optimization pipeline

The target-independent optimizer now has a deterministic selection-policy
foundation over final MIR. Its frozen complete design is preserved in the
[decision record](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_DESIGN_PROPOSAL.md),
and its delivery is recorded by the
[completed implementation roadmap](../archive/SELECTABLE_FINAL_MIR_OPTIMIZATION_PIPELINE_ROADMAP.md).
The typed registry and profiles, schedule occurrence model, exclusions, exact
compiler-internal test schedule resolver, request/CLI selection, verified
execution, structured failures and measurements, inspection checkpoints, and
shared value-use census are implemented. Every compiler adapter resolves its
profile before provider or source work and passes that schedule to the same
MIR pipeline.

One compiler-owned immutable registry couples each entry's typed identity,
unique stable lowercase kebab-case name, description, implementation-declared
identity, and transformation entry point. Deterministic validation rejects
duplicate identities or names, invalid names, empty descriptions, and
mismatched implementation identity before schedule selection. The production
registry contains `checked-integer-constant-folding`,
`dead-pure-definition-elimination`,
`primitive-constant-folding`, `primitive-algebraic-simplification`,
`conservative-cfg-cleanup`, and `whole-world-reachability`. Its validated
descriptors are exposed in stable-name order for the public read-only query
and the input-free `--list-mir-passes` CLI command; discovery therefore reads
the same metadata used by schedule resolution. The `none` profile
expands to an empty explicit ordered schedule. `default` contains the exact
nine-occurrence optimization schedule documented below. Disabling all
pass names selected by `default`, including duplicate disabling, produces the
same schedule as `none`.

A resolved schedule may deliberately repeat a pass, and every occurrence is
identified by its resolved schedule position, pass identity, and that pass's
zero-based occurrence number. Stable-name exclusions remove every matching
occurrence, duplicate exclusions are idempotent, and unknown names plus the
complete known-name inventory are sorted lexically. Filesystem order, module
discovery, registry order, map iteration, or compiler worker completion never
selects execution order. Exact schedules are a crate-private input for tests
and compiler tools. The command line selects profiles and exclusions,
not arbitrary pass order.

The transforming runner first calls central final-MIR verification, including
immutable static-lifecycle realization and target-independent reachability.
Every occurrence then receives read-only access to that verified program-plus-
facts product and one pipeline-owned capability to consume the seal through
the atomic whole-program rewrite coordinator. An unchanged outcome retains the
same seal and facts and adds no verification execution. A changed outcome
invalidates program and facts together, yields raw dense MIR, rewrite maps,
change summaries, and explicit changed-callable pass data, and immediately
calls central verification before any later pass, inspection checkpoint, or
backend can observe it. Input-verification, pass execution, structural-
rewrite, and output-verification failures identify the exact pass name,
identity, schedule position, and occurrence where applicable, then stop
without exposing a partial or later product.

Passes cannot construct seals, mutate dense definition tables directly,
change lifecycle authority, emit diagnostics, log, write files, render dumps,
or depend on driver, reporting, target, or another pass's private analysis.
Analyses are pass-local by default, and every change invalidates data keyed by
pre-commit local identities or instruction positions. The first framework has
no preservation declarations or global analysis manager. Whole-program
analysis may inspect all verified definitions, but edits still commit through
the single atomic program coordinator.

The callable editor and dense verified definitions provide one read-only
value-use census backed by that same exhaustive identity traversal. Both
borrow their MIR directly; no private callable or edit snapshot is built for
analysis. The census records every live value declaration
in value-index order, distinguishes the unique instruction definition site
from actual uses, and counts uses in rvalues, calls, arguments, terminators,
logical proof records, and every other value-bearing site owned by the shared
traversal.
Definition positions and declarations are not uses. Foreign, unknown, deleted,
and duplicate definition identities produce structured rewrite errors. A
census describes only the MIR state from which it was computed: any rewrite
invalidates it, and fixed-point passes must recompute before their next wave.
This is deliberately not liveness, dominance, effect analysis, alias analysis,
or a cached analysis manager.

The runner now returns deterministic aggregates and, when trace observation is
requested, one ordered typed record for every attempted occurrence. Records
distinguish unchanged, changed, and failed outcomes; identify repetitions by
pass identity, stable name, schedule position, and occurrence number; and keep
unavailable failure measurements absent. Pass timers and record allocation are
skipped below trace detail. Reports contain compact typed metrics and events
rather than MIR text, distinguish processed from actually changed callables,
and treat elapsed durations as observations rather than deterministic
products.

Optional pipeline inspection is a request-local service separate from semantic
compilation requests and report observers. It receives only borrowed
`VerifiedFinalMirProgram` checkpoints at `input`, after every successfully
completed occurrence, and `final`. After-pass labels use
`after-<schedule-position>-<stable-pass-name>-<occurrence-number>`, so repeated
passes cannot collide. Changed MIR is centrally resealed before inspection;
pass, rewrite, or output-verification failure emits no failed after-checkpoint
and no final checkpoint. The ordinary path passes no inspector and therefore
constructs no checkpoint label strings, dumps, collections, or report events.
The inspected entry point may invoke phase-owned `mir::dump_mir`, request the
checkpoint's deterministic seal-bound reachability dump, or collect in-memory
facts, but filesystem retention and CLI dump policy remain separate.

The framework's production canary removes only an unused
`MirInstruction::Assign` whose rvalue is an integer, byte, binary64-bit, or
boolean constant; an exact unary or binary primitive operation; a primitive
comparison; or a non-checked primitive cast. Eligibility is an exhaustive
no-wildcard classification. Calls, loads, callable addresses, path conditions,
checked division, checked shifts, checked binary64-to-integer conversion, type
tests, optional-presence operations, array length, and every non-assignment
result producer remain ineligible.

For each executable function, method kind, lifecycle member, and static
initializer, the canary computes value uses through the exhaustive MIR
identity traversal, deletes unused eligible assignments and their matching
value declarations in stable waves to a fixed point, and commits the callable
once. It performs no CFG, storage, metadata, ownership, lifecycle, folding,
replacement, or reordering edit. The canary runs at the first, sixth, and
eighth positions of `default`, with whole-world reachability last;
`none` preserves the exact verification-only path, selective disabling
provides parity, and every changed product passes ordinary and
lifecycle-realization verification.

This boundary adds no dynamic pass ABI, target-specific pass, numerical
optimization level, SSA, proof-provenance normalization, general alias/effect
analysis, devirtualization, inlining, CFG cleanup, register allocation, or
target LIR. Permanent
whole-world compilation and single-threaded generated programs make later
analyses more tractable, but neither assumption weakens verification,
determinism, evaluation-order, checked-failure, allocation, ownership, alias,
or destruction requirements.

### Local final-MIR simplification

The implemented target-independent local-simplification layer follows the
[frozen design](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_DESIGN_PROPOSAL.md);
its delivery and validation history are preserved in the
[completed roadmap](../archive/LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md).
Exact primitive evaluation, block-local constant facts, exhaustive value-use
classification, and the independently selectable `primitive-constant-folding`
and `primitive-algebraic-simplification` passes are implemented. Proof-aware
local CFG facts and the independently selectable `conservative-cfg-cleanup`
pass are also implemented. The exact repeated default schedule is active;
broad semantic and determinism coverage exercises it under debug and release
compiler builds, repeated independent processes, selectable exclusions, and
the exact `none` reference profile.

The local-simplification layer consists of three independently selectable
production passes under the existing verified pipeline:

- `primitive-constant-folding` evaluates a closed exact family of
  block-local integer and boolean primitive operations;
- `primitive-algebraic-simplification` applies the reviewed integer/boolean
  identity catalog and atomically forwards safe result uses; and
- `conservative-cfg-cleanup` folds eligible ordinary boolean branches and
  removes unprotected unreachable blocks and their transient values.

One compact measurement fixture reduces 3 executable definitions, 5 blocks,
14 instructions, and 14 transient values to 2 definitions, 3 blocks, 3
instructions, and 3 values. A standard-library-backed golden additionally
exercises scalar extrema, proof-protected logical CFG, function values, static
startup/shutdown, destruction, and a call target exposed to whole-world
pruning. Pass-owned counters attribute the local changes; they are structural
evidence and do not imply a timing threshold or a general workload ranking.

Constant semantics have one optimizer-private typed owner. Initial
folding includes explicit wrapping `i64`, `u64`, and `u8` add, subtract, and
multiply; wrapping `i64` negation; integer bitwise operations and complement;
boolean not; integer and boolean comparisons; identity casts; integer width
conversions; integer-to-boolean zero testing; and canonical boolean-to-integer
conversion. `u8` results are explicitly canonicalized. Host arithmetic whose
debug/release behavior differs is not a valid evaluator.

Floating operations and conversions, division, remainder, shifts, checked
conversions, loads, calls, path conditions, callable addresses, type/optional
queries, array length, ownership operations, and failure-bearing protocols are
outside the initial evaluator. In particular, checked integer and shift
operations cannot be folded by replacing only their success rvalue because
verification relates them to exact predecessor diamonds.

Implemented scalar facts are instruction-ordered and reset at every block. A
constant or constant-result algebraic rewrite preserves the assignment's
result identity, declared type, instruction position, and source span. MIR has
no copy rvalue;
an algebraic identity that returns an existing operand instead proves exact
type, earlier same-block definition, and every use role, then substitutes uses
and deletes the obsolete assignment and declaration in one atomic callable
transaction.

An implemented read-only use-site query enumerates each selected transient
value use in deterministic structural and operand order. Forwarding is
permitted only through explicitly classified ordinary executable scalar uses
that follow the definition in the same block. Path/logical proof metadata,
dedicated checked terminators, proof-coupled success operations, lifecycle or
ownership state, callable attachments, I/O, and unknown future roles are
barriers. The query shares exhaustive immutable identity traversal and compact
definition/use census ownership with rewriting but remains narrower than the
general substitution mapper. Its result is a snapshot and must be recomputed
after any rewrite.

CFG cleanup rewrites only ordinary `Branch` terminators whose condition is a
preceding block-local constant or whose targets are identical. It preserves
the terminator span and never rewrites a dedicated checked or multiway
terminator. Local block reachability starts from body entry, every callable
lifecycle/publication attachment, and every block named by path-condition,
logical-expression, or other proof metadata. Protected proof regions remain
even when ordinary entry reachability no longer reaches them.

An unprotected unreachable block is removed together with every transient
value defined by its instructions. Storage declarations, path conditions,
logical records, guards, and attachments remain. Empty-block forwarding,
block merging, jump threading, proof-record normalization, checked-diamond
simplification, storage propagation, alias/effect analysis, SSA, and target
optimization remain later decisions.

The exact `default` schedule is:

```text
dead-pure-definition-elimination
primitive-constant-folding
primitive-algebraic-simplification
primitive-constant-folding
checked-integer-constant-folding
dead-pure-definition-elimination
conservative-cfg-cleanup
dead-pure-definition-elimination
whole-world-reachability
```

`none` remains empty, stable-name exclusion removes every repeated occurrence,
and whole-world retention remains last so it can observe calls, callable-
address formations, and other executable dependencies removed by CFG cleanup.
Every occurrence continues to consume verified MIR, preserve an unchanged seal
when possible, atomically commit changes, invalidate all final-MIR-derived
facts, and immediately reverify before any later pass or backend observes the
product. Preliminary-MIR static activation and baseline lifecycle authority
remain immutable; final verification rechecks realization against them rather
than replanning activation.

### Checked-integer constant protocol simplification

The checked-integer occurrence consumes constants exposed by the preceding
primitive folds. For an eligible division, remainder, or shift protocol it
preserves operand evaluation, replaces the checked success operation with its
exact constant, removes the two protocol-private load values, and turns the
dedicated check into an ordinary successor edge. The following dead-pure and
CFG occurrences can then remove redundant scalar work and the unreachable
failure region. Disabling `checked-integer-constant-folding` retains the
checked protocol; disabling CFG cleanup retains its now-unreachable failure
block. Static failures and insufficiently proven protocols remain unchanged.

Eligibility is deliberately narrower than general constant propagation. Both
operands must reach the checked terminator through distinct canonical
`ScalarSpill` carriers, each with one exact dominating constant store. The
observer requires the verifier-owned check, success, failure, result-store,
join, and reload topology and rejects any block protected by logical,
path-condition, lifecycle, or static-publication metadata. The rewrite
revalidates the complete snapshot against live sparse edit state before one
atomic dense commit. It preserves result identity and source spans, retains
carrier storage and lifecycle work, and never turns a static failure into a
compile-time diagnostic or changes failure timing.

The pass implements exact Skald floor quotient and divisor-sign remainder,
including the defined signed-minimum pair, wrapping left shift, arithmetic
signed right shift, logical unsigned right shift, and canonical byte results.
It does not fold a dynamic operation with only a known-safe divisor or count,
checked floating-to-integer conversion, floating arithmetic, casts or type
tests, optional or array checks, calls, loads, ownership operations, or
target-specific instructions. Nested checked results are not propagated
through their retained scalar carriers by this pass.

### Current execution-dependency vocabulary

MIR now owns one neutral `MirExecutionNode` identity for callables, class copy
construction, class copy assignment, complete class finalization, and array
default/copy/assignment/destruction. Static-lifecycle correctness analysis and
whole-world reachability consume that identity and its neutral
`MirClassLifecycleOperation` and `MirArrayLifecycleOperation` taxonomies
directly, so the two analyses cannot acquire independently drifting lifecycle-
node vocabularies.

The crate-private `passes::reachability` facade defines typed dependency
targets and edge kinds, whole-program root targets and reasons, runtime-entity
references, semantic-declaration references, and physically retained callable-
definition references. Executable nodes, declarations, retained bodies,
runtime metadata, external/intrinsic leaves, and backend artifacts therefore
remain distinct roles even when they refer to related semantic identities.
Canonical comparison keys define deterministic node, edge-kind, root-reason,
and source-span order without exposing future graph storage.

The contract has one read-only extraction implementation over borrowed
ordinary functions, member definitions, and preliminary or final static
initializer bodies. It records deterministic direct, static, instance,
virtual-family, interface-conformance, exact callable-address, and indirect-
signature dependencies; recursively expands canonical class, optional,
shared-owner, and array lifecycle plans; and preserves external and intrinsic
calls as typed leaves. The same traversal records every direct static-place
read, write, replacement, borrow, initialization, and destruction with its
containing execution node, target field, structural phase, source span, and a
typed ordinary versus lifecycle-owned-destination classification. All
structurally present blocks contribute evidence; extraction performs no local
CFG pruning. Invalid field and lifecycle-destination identities return the
same structured extraction failure channel as invalid dependency targets.

Callable-address formations retain their containing execution node, exact
function type, target, and span so later closure can scope candidates without
rescanning MIR. A private shared function-value coupling worklist accepts
reached execution-node events, maintains the exact-function-type candidate and
indirect-site fixed point independently of discovery order, and returns newly
selected indirect execution edges in canonical order. Final reachability and
semantic static activation consume those common edges while retaining separate
roots, witnesses, errors, and result models. Shared optional and owner
lifecycle resolvers also drive exact reverse-shutdown root expansion, so
dependency and root policy do not maintain separate cleanup walks.

The lifecycle dependency facade owns the common dependency vocabulary, static
cleanup dispatch, and class-before-array extraction order. Private cohesive
owners implement class copy and finalization, recursive optional/shared-owner
expansion, and array default/copy/assignment/destruction respectively. The
facade remains the only path used by body extraction, static activation, and
reverse-shutdown roots.

Static-effect analysis adapts the shared static-access records, targets, and
lifecycle edges into its private summaries. It retains ownership of propagated
witnesses, authority, diagnostics, dumps, and solved effects, but owns no MIR
body scanner. The superseded static-effect static-place, call, function-value,
and lifecycle walkers have been removed.

On complete final MIR, the same facade collects typed roots for the
internal entry, every static activation, and every reverse-shutdown cleanup,
then computes an iterative deterministic least fixed point. Reached address
formations populate only their exact function-type candidate set; reached
indirect sites and newly discovered candidates are coupled until stable, and
every reached formation independently retains its addressed internal callable.
All structurally retained blocks are scanned. Immutable private sorted storage
backs borrowed queries for roots, nodes, outgoing edges, callable definitions,
function-value candidates, dispatch use, runtime entities, stable counts, and
canonical first-witness explanations. A separate target-independent dump is
available to focused compiler tests and tools.

Central verification now runs this analysis only after ordinary structure and
static-lifecycle realization succeed, and binds its immutable facts to the
exact `VerifiedFinalMirProgram`. A crate-private borrowed query lets pass
capabilities and later backend work inspect those facts without exposing their
construction or mutation publicly. Unchanged pass outcomes preserve them;
changed outcomes discard them with the old program and rebuild a coherent
product before any later pass, checkpoint, or backend boundary. Existing
verification-execution accounting covers this complete boundary; there is no
global cache or preservation protocol. Checkpoint labels and ordinary MIR dump
bytes remain unchanged, while focused compiler tests may separately inspect
the reachability dump.

Final-MIR structural verification now distinguishes declarations from retained
executable definitions. It validates every physically present function,
member, and static-initializer body against its declaration and all ordinary
body invariants, but an internal declaration need not have a body merely
because it remains in the closed-world semantic inventory. Preliminary MIR
uses a separate producer-completeness mode and still requires every internal
function and every declared lifecycle/member body.

The central final boundary owns the stronger executable-completeness proof.
After structural and static-lifecycle verification, it independently derives
the exact root closure and requires a retained body for every reachable
internal callable. Missing reachable definitions produce canonical callable-
attributed verification errors naming the final selecting root or dependency
category. Virtual families, interface conformances, and other stable metadata
may therefore name a bodyless method only when no reachable operation selects
it. Every retained body remains fully verified even when unreachable, and
static activation, reverse shutdown, and immutable lifecycle baseline
authority remain enforced with unrelated sparse definitions.

MIR now also owns a private program-level definition-retention facade separate
from callable-local identity rewriting. The pipeline capability asks this
facade to prepare an opaque plan from the reachability facts sealed with the
exact verified input; callers cannot provide a retained-ID set or predicate.
Preparation borrows MIR, validates that every current static initializer is
rooted, and computes canonical removed IDs plus examined, retained, and removed
counts for functions, static initializers, initializers, copy constructors,
copy assignments, destructors, and methods. An error therefore occurs before
any definition container is consumed.

An unchanged plan returns the original verified product without another
verification execution. A changed plan alone may invalidate the seal, move
retained function and member bodies into rebuilt containers, preserve existing
function holes and member-key order, and publish complete raw MIR. The pass
runner immediately sends that product through central final verification and
rebuilds coherent reachability facts before a later pass or checkpoint can see
it. Retention never rewrites a body, static initializer, declaration, global
identity, metadata table, source span, coordinator region, or lifecycle proof.
The registered whole-world reachability pass is the sole production client.
It reads already-derived counts, invokes this capability once, and adds no
second MIR traversal or broader mutation authority.

Whole-world reachability runs after the canary in the supported `default`
profile and remains independently disableable. Backend planning consumes the
physical retained-definition domain and requires bodies only for reachable
dispatch selections.
Any new MIR operation that can select executable work, or new implicit
lifecycle operation, must update the exhaustive dependency extraction and its
focused coverage in the same change.

### Target-independent whole-world reachability

The confirmed
[whole-world reachability design](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
and its
[completed roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
define the implemented reusable final-MIR analysis and retention boundary.

Final MIR exposes one target-independent execution-dependency vocabulary
covering ordinary callables plus implicit class copy, assignment, complete
finalization, and array default/copy/assignment/destruction work. Root
collection, dependency extraction, possible-target resolution, closure
solving, program retention, verification, and backend consumption remain
separate owners. Static-effect analysis shares exhaustive direct, virtual,
interface, function-value, ownership, optional, array, copy, and destruction
target selection without turning static-access phases or witnesses into the
general reachability product.

The current whole-program root contract is the identity-selected entry plus
every static coordinator activation and reverse-shutdown obligation. Static
startup and deterministic shutdown remain observable even when no ordinary
callable reads the field. External and intrinsic declarations are reached
leaves, not internal roots. Whole-world compilation makes this root and target
inventory finite; single-threaded generated execution introduces no concurrent
source roots, atomics, synchronization edges, or asynchronous callbacks.

Direct and static calls retain exact targets. Virtual calls initially expand
to the full verified family, interface calls to every verified implementation
of the selected requirement, and implicit lifecycle operations through their
canonical MIR plans. Function-value reachability uses a coupled monotone fixed
point: callable-address formations become candidates only when their containing
execution node is reachable, exact signatures select indirect targets, and
forming an exact address retains the addressed callable even without a later
indirect call. All structurally present blocks of a reachable callable are
conservatively scanned until a separate CFG pass removes dead regions.

Central final verification binds immutable deterministic reachability facts to
exactly one `VerifiedFinalMirProgram`. These facts include exact static-place
accesses from reachable execution nodes and canonical selecting explanations.
Unchanged pass outcomes preserve that seal and its facts. Every changed outcome
invalidates both and rebuilds them before another occurrence, inspection
checkpoint, or backend can observe the program. The product provides
crate-private read-only root, reachable-node, callable-target, static-access,
dispatch-use, runtime-entity, and explanation queries without introducing a
global analysis manager or preservation declarations.

Preliminary MIR remains definition-complete. Optimized final MIR may retain a
dense semantic declaration while omitting its unreachable function or member
body. Program-level IDs, declarations, classes, interfaces, fields, type
tables, virtual families, lifecycle authority, literals, and spans remain
stable. Final verification validates every body physically present,
independently recomputes root closure, and requires a body for every reachable
internal target selected by calls, callable addresses, dispatch, copy,
assignment, destruction, ownership, optionals, arrays, or static lifecycle.
The lifecycle realization set must still be a subset of immutable baseline
authority.

One private atomic program-retention capability filters sparse function
definition slots and callable-keyed member definitions using facts bound to
the consumed seal. It does not expose mutable declaration tables, compact
global identities, rewrite retained bodies, mutate lifecycle authority, log,
verify, or render. The registered `whole-world-reachability` pass consumes this
capability and removes executable definitions only; declaration and metadata
compaction, rapid-type analysis, points-to refinement, devirtualization,
inlining, and broader interprocedural analysis remain later work.

The existing final-MIR runner remains authoritative. Changed retention re-
enters ordinary and lifecycle-realization verification immediately, and the
backend continues to accept only the resealed product. Optimization-off mode
still retains exact complete MIR and source diagnostics; computing seal-owned
facts alone performs no pruning.

The optional-values contract assigns each decision to these same phase owners.
Syntax preserves source shape and resolution assigns recursive, bottom-up
interned optional identities whose payloads may name earlier optional or array
identities. Type checking owns a canonical ID-indexed HIR description of every
optional payload's storage, representation, lifecycle, checked access, and
argument/result/static/array-element plans. For optional owning values, it selects explicit
absent or present initialization, copy, assignment, overload injection,
field/call boundaries, presence, primitive extraction, checked class payload
views, and optional shared copy/adopt/move/release and secured unwrap. MIR
lowering deterministically preserves the canonical optional identity table and
its payload storage, representation, lifecycle, checked-access, and boundary
plans. Specialized scalar, aggregate, and shared-owner instructions remain
only where their runtime work differs. MIR owns
initialized places, caller-owned argument/result
aggregates, explicit unwrap success/failure control flow, begin/end guard
operations, and guarded-mutation checks. Verification proves compatible
operations, definite wrapper initialization, balanced compatible guards,
anchor ordering, isolation of the zero niche from ordinary owners, and
identical initialized optional state across CFG joins. Inline optional
container aliases use ordinary indirect MIR places plus exact optional types.
Checked optional-array payload aliases additionally use a guarded payload
projection and an ordinary array-backing anchor for the complete immediate
call. Shared optional boxes have canonical resolved, HIR, and MIR
target identities, typed construction/ownership, verified
unpublished-payload and owner lifetimes, explicit exact optional-pointee
access, and explicit polymorphic object-box views. Exact and polymorphic local
boxes execute on x86-64 through descriptor, shared-owner, recursive optional,
guard, dispatch, cast, stored-value, array, static-lifecycle, and internal call
lowering. External optional signatures remain deliberately unsupported and
optional-reference shapes remain syntax diagnostics.

Optional definite-initialization verification keeps one private state model
behind the existing optional-verifier facade. A propagation owner computes
path-sensitive fixed-point entry states and condition convergence; a checking
owner replays each block to emit instruction and terminator diagnostics; and a
state owner encapsulates storage epochs, ownership transfer, entry seeding, and
recursive initialization of optional fields in completed class storage. This
division is internal: diagnostic text, ordering, MIR contracts, and the
separate immediate-consumer guard analysis are unchanged.

### Shared optional box phase boundary

The `Shared<Optional<P>>` implementation extends shared targets with exact
optional allocation identities and polymorphic optional-object view
identities. Resolution preserves target `?`, `shared?` shorthand, grouping,
and allocation spans while interning deterministic optional and box-view
identities. Object-only, array-only, optional-place, and generic owner
consumers query explicit target capabilities rather than assuming every
non-array owner is an object.

HIR owns typed optional-box allocation and local owner transfer. It records
exact allocation target, static owner view, destination-directed
initialization/copy/transfer plan, source-before-allocation order, owner
provenance, publication boundary, and diagnostic spans. Published box wrappers
are immutable, so HIR never represents a whole-pointee assignment or mutable
whole-wrapper alias. Exact pointee places retain stable, copied-place, or
adopted-producer owner provenance; non-stable owners receive a hidden
full-expression anchor. Object-box views retain a static
class/interface/`Obj` view separately from the exact dynamic allocation class.
Presence observations through interface and `Obj` views remain box operations
rather than synthesizing invalid standalone optional identities.

MIR implements a distinct optional-box allocation origin and makes allocate,
initialize the exact `SharedAllocationPayload`, publish, and adopt separate
verified transitions. After publication, `SharedPointee(owner)` permits no
pre-publication observation and addresses exact optional operations only while
its owner is verified live. A checked polymorphic unwrap begins one
optional-box guard for each traversed layer and exposes
`OptionalBoxPayload { owner, target }` as the complete object subject; matching
ends precede anchor cleanup in reverse order. Owner copy, secure replacement,
field/static storage, internal calls/results, temporary cleanup, and final
release reuse the ordinary shared-owner state machine. Existing
optional initialization instructions complete the canonical wrapper without a
parallel box-payload instruction family. MIR verification ties each view,
presence query, cast, and dispatch origin to a live compatible owner, balanced
guards, a static box target, and exact dynamic descriptor metadata. The x86-64
legality pass accepts both exact and polymorphic box targets with
verified addressable metadata.

The initial x86-64 realization keeps one-word owners and the 16-byte shared
header, uses deterministic exact optional-box descriptors/finalizers, and
places the canonical optional payload at target-layout offset 16. An object
box descriptor also retains exact dynamic class and view membership. The
outer `(shared P?)?` zero niche remains separate from the allocation's inner
optional state. All work remains compiler-owned and adds no runtime ABI
version or public symbol.

Exact object-box descriptors clone the exact class's virtual and interface
dispatch entries while replacing only finalization with the recursive box
finalizer. Static base/interface/`Obj` views and successful checked owner casts
therefore retain the original allocation and dispatch exactly like ordinary
shared objects. Copying a polymorphic wrapper into an eligible exact inline
class optional remains target-directed and deliberately slices; interface and
`Obj` inline optional destinations remain invalid.

## Produced exact-class alias representation

The source-visible
[produced read-only alias contract](../language/ALIASES_AND_OWNERSHIP.md#implemented-produced-read-only-alias-arguments)
is implemented through syntax, type checking, HIR, verified MIR, and native
x86-64 execution. Syntax and resolution reuse the ordinary producer path;
later phases classify the source, prove its hidden lifetime, and consume the
ordinary object-view representation. The extension uses the existing phase
owners and object-view pipeline rather than introducing a reference-valued
expression or a second alias representation:

- Syntax retains the producer as an ordinary call argument expression. No AST
  node, grammar form, binding mode, or lifetime annotation is added.
- Resolution selects ordinary construction, literal, direct/static/method/
  interface-call, grouping, and checked-cast identities exactly as it does in
  other object-producing contexts. It does not decide alias eligibility or
  materialize storage.
- Type checking recognizes an exact-class producer only after the selected
  parameter requires read-only alias access. It applies the existing exact,
  ancestor, implemented-interface, and `Obj` view relation and rejects the
  same implicit downcasts and unrelated targets. A `mut ref` parameter still
  requires an existing mutable place. Initializer applicability observes this
  same static binding relation without evaluating a producer, allocating
  identities speculatively, or emitting candidate-local duplicate
  diagnostics; final checking represents the selected source once.
- Typed HIR records one `HirObjectView` whose source is the existing
  `HirViewSource::Produced` producer form, whose origin retains the exact
  dynamic class and complete-object identity, whose static base projections
  retain an ancestor target without slicing, whose target retains the selected
  class/interface/`Obj` view, and whose access is read-only. Grouping changes
  only spans. A checked cast retains its ordinary static or runtime selection
  and bounded carrier instead of being flattened into an existing-place view.
- HIR-to-MIR lowering allocates one caller-owned exact-class `Temporary` at
  the argument's source position, lowers the producer directly into it, and
  uses that same complete place as the alias-view source. It emits no
  copy-construction step. Registration occurs only after successful
  completion. The temporary remains live through later arguments and the
  outer call, then joins the enclosing full-expression plan and is destroyed
  once in reverse completion order. Selected-path lowering creates no
  storage, view, or cleanup for a skipped producer, and any checked-view
  carrier ends before its owning producer temporary.
- MIR verification proves storage lifetime, initialization before view use,
  exact complete-object origin, compatible read-only access, registration,
  use through the consuming call, checked-carrier dependency order, and one
  correctly ordered cleanup. A mutable produced view, premature or missing
  cleanup, duplicate destruction, or mismatched origin is malformed MIR.
- Backends consume the ordinary verified `MirArgument::View` representation.
  The existing internal alias calling convention is unchanged, external
  object and alias signatures remain invalid, and no runtime service or ABI
  version change is introduced.

The native backend consumes that verified view through the unchanged
three-component internal object-alias convention. Deterministic HIR, MIR,
assembly, stdout, and destruction-trace coverage establishes the complete
compiler boundary without a produced-source target branch or runtime service.

Type-check diagnostics preserve the distinction between provenance and type:
an incompatible exact-class producer reports a type mismatch with producer
and parameter context, while a compatible producer selected for `mut ref`
reports that an existing mutable place is required. Excluded primitive,
optional, array, raw-shared-owner, and implicit-shared-dereference sources keep
their family-specific diagnostics. Call checking continues through later
arguments in source order.

## Produced primitive read-only alias representation

The [produced primitive alias
contract](../language/ALIASES_AND_OWNERSHIP.md#implemented-produced-primitive-read-only-alias-arguments)
is implemented for ordinary internal calls and initializers without adding
reference-valued expressions or changing the alias ABI:

- Type checking preserves exact compatible bindings, static fields, and their
  groupings as direct `HirPrimitivePlace` arguments. Any other successfully
  checked exact primitive expression may satisfy only a read-only `ref` and is
  retained as one `HirCallArgument::ProducedPrimitiveAlias` expression.
- HIR-to-MIR lowering evaluates that expression once at its ordinary
  left-to-right argument position, stores the value once in dedicated
  `PrimitiveAlias` scalar storage, and passes its base place as the ordinary
  alias argument. The full-expression tracker emits `StorageLive` before the
  store and one reverse-ordered `StorageDead` after the call result is secured.
- MIR verification requires one matching primitive declaration, lifetime
  start, initialization, read-only call or initializer borrow, and lifetime
  end in order. It rejects mutable borrowing, loads, additional uses, writes,
  escape, early or duplicate end, missing initialization, and wrong type.
- Direct, static, method, interface, indirect, and initializer call forms all
  consume the same HIR call-argument capability. Backends need no dedicated
  branch because verified storage places already use the internal alias ABI.

## Produced exact-class method-receiver representation

The source-visible
[produced receiver contract](../language/FUNCTIONS_AND_CONTROL_FLOW.md#produced-exact-class-method-receivers)
is implemented through verified MIR. The resolver and
type checker admit eligible exact-class producers for read-only dot-method
receivers while retaining the existing-place requirement for mutable methods.

Implementation reuses the ordinary exact-class producer, object-view, and
full-expression lifetime pipeline:

- syntax keeps the existing postfix member and call nodes;
- resolution records the selected producer once, its exact class, source span,
  and any canonical inherited or interface selection, rather than inventing a
  source binding;
- type checking admits that provenance only for a read-only instance method,
  preserves the exact complete-object origin and dynamic class, and emits one
  explicit produced object view in typed HIR;
- HIR-to-MIR lowering constructs the producer exactly once in one caller-owned
  `Temporary` before explicit arguments, registers cleanup only after complete
  construction, and uses the ordinary non-owning receiver view for the call;
- direct and virtual `MirMethodReceiver` values retain the access granted by
  HIR separately from their backing place. This matters for produced objects:
  their compiler-owned storage is mutable while their callable view is
  read-only. A `MirViewProvenance` marker distinguishes that direct produced
  view from ordinary place, cast, and anchored views. Interface receivers
  retain the same access and provenance distinction in `MirObjectView`;
- result securing or transfer completes before full-expression cleanup, and
  checked or bounded carriers end before the owning receiver temporary; and
- MIR verification proves initialization, read-only access, receiver and
  complete-object compatibility, selected-path liveness, result ordering, and
  exactly one reverse-ordered cleanup.

Typed member-receiver provenance uses one exhaustive checked carrier, and HIR
exposes one `HirObjectReceiver` enum for method and field receivers:

- `Place` retains an ordinary selected place and complete-object origin;
- `Checked` retains the checked-cast carrier and its inspection path;
- `View` carries one `HirObjectView` for shared-backed, optional-backed, and
  produced receivers; and
- `ArrayElement` keeps checked addressing and later projections explicit.

Object-to-interface conversion matches that carrier exhaustively and produces
the existing discriminated interface receiver. Access queries, control-effect
discovery, HIR dumps, field and call lowering, and test mutation helpers also
match the enum rather than coordinating optional fields. Existing shared
anchors, optional guards, array anchors, origins, dispatch, cleanup, and dump
vocabulary are unchanged. An optional inspection place preserves historical
dump paths for existing views but is never executable provenance; a produced
`View` leaves it absent and therefore needs no fake binding. A third
independent `produced_view` field is not permitted.

Direct, inherited, virtual, interface, and closed-generic calls all lower
through their existing selected method targets and receiver origins. The
backend receives an ordinary verified receiver place backed by explicit
temporary storage. No target-specific producer branch, internal or external
calling-convention change, runtime service, or runtime ABI-version change is
introduced. Resolved, HIR, and MIR dumps expose provenance deterministically;
MIR method-call dumps include the receiver's granted access before its exact
or forwarded origin.

## Produced-object field-read representation

The source-visible
[produced-object field-read contract](../language/FUNCTIONS_AND_CONTROL_FLOW.md#produced-object-field-reads)
is implemented, with its complete field-category surface lowered
through existing typed carriers. A selected final field uses ordinary
`ResolvedFieldAccessExpr`;
its receiver retains the
producer exactly once in `ResolvedObjectReceiver::Produced`, preserves
`exact_class`, tracks the terminal `class`, and orders canonical inherited-base
and intermediate `ObjectProjection::Field` entries. Type checking lowers
primitive reads to ordinary `HirFieldPlace` reads over one read-only produced
`View` without an inspection place. Exact inline-class endpoints retain the
same view for method, alias, checked-view, and copy consumers. MIR materializes
the producer once, projects the selected field, records that field subobject
as the exact complete origin, and reuses ordinary load, call, alias, and copy
operations. Optional storage, array receivers and sources, shared places and
transfers, optional-box views, aliases, and owning results retain those same
typed categories while their field place carries the produced view. Member
assignments retain the same receiver provenance so type checking rejects
mutation through its read-only access.

Implementation is constrained to extend the existing member-receiver pipeline
rather than create another provenance family:

- syntax retains the existing postfix member nodes and assignment shapes;
- resolution retains the eligible producer once in
  `ResolvedObjectReceiver::Produced`, append canonical base and field
  projections, and continue to reject unsupported root families with their
  existing diagnostics;
- type checking represents a read as an ordinary `HirFieldPlace` whose
  receiver is one read-only `HirObjectReceiver::View` backed by
  `HirViewSource::Produced`, with no inspection place or fake binding;
- class-typed endpoints remain projected object consumers rather than scalar
  expressions, and optional, array, shared-owner, and optional-owner endpoints
  reuse their existing typed source categories;
- HIR-to-MIR lowering materializes the producer exactly once through the
  existing full-expression object-temporary helper, then reuse ordinary field
  projection, load, copy, transfer, anchor, guard, and call operations; and
- verification proves initialization, read-only access, exact complete
  origin, projection validity, selected-path liveness, result securing before
  cleanup, and exactly one reverse-ordered destruction.

The produced root is internally writable only for initialization and
destruction. No source store, mutable receiver, or mutable alias may use it.
The complete root remains live through its immediate field consumer and the
enclosing full expression; subordinate checked views and anchors end first,
while independent owning results are secured before root cleanup. Control-
effect discovery must include both production and the final field consumer so
logical paths and earlier scalar spills retain their current ordering.

Resolved, HIR, and MIR dumps expose the existing produced-view provenance and
canonical projection order deterministically. The implementation adds no
new HIR receiver variant, MIR storage kind, backend operation, calling-
convention component, runtime call, or runtime ABI version.

## Primitive binding reassignment boundary

The source contract for
[primitive binding reassignment](../language/FUNCTIONS_AND_CONTROL_FLOW.md#primitive-binding-reassignment)
extends the existing pipeline without adding a new place family:

- Syntax already retains an identifier or grouped identifier followed by `=`
  in the existing assignment-shaped AST node, including the equality span,
  source expression, and complete statement span. Parsing chooses no binding
  identity, type, mutability, or semantic assignment category.
- Resolution recognizes this meaning only when lexical lookup selects a
  primitive `BindingId::Local(LocalId)` or `BindingId::Parameter(ParameterId)`.
  It emits a dedicated `ResolvedPrimitiveBindingAssignment` containing that
  destination `BindingId`, equality span, resolved source, and statement span.
  Grouping does not alter lookup, and destination lookup completes before
  resolving the source.
- Type checking requires the source expression to have exactly the
  destination binding's declared type and accepts only `i64`, `u64`, `u8`,
  `f64`, or `bool`. HIR uses a dedicated
  `HirPrimitiveBindingAssignment` containing the destination `BindingId`, one
  typed source expression, and the statement span. The operation type remains
  available from the binding table and `HirExpression::ty`; it is not
  duplicated in the statement where the two copies could drift.
- MIR lowering evaluates the HIR source once, emits the existing
  `MirStore` to the binding's already allocated local or parameter storage,
  and then emits the ordinary full-expression boundary. No initialization,
  liveness, ownership, or cleanup registration changes.
- MIR verification already requires a scalar, exactly typed, mutable store
  destination and a defined value operand. Backends consume the verified
  store mechanically. The x86-64 target already handles canonical integer,
  byte, boolean, and floating stores, so this feature adds no layout, ABI,
  runtime, or target-specific semantic rule.

Assignment remains a statement with no HIR expression result. Alias
parameters, invalid roots, compound and chained assignment, destructuring,
and every existing non-primitive assignment family remain outside this
boundary.

## Implemented primitive operator boundary

Primitive integer bitwise and shift operations, comparisons, and casts plus eager
boolean negation and equality have
[source contract](../language/TYPES_AND_VALUES.md#implemented-primitive-comparisons-boolean-negation-and-integer-casts)
and are current products through native x86-64 execution. Bitwise operations
have their focused contract under
[implemented integer bitwise and shift operators](../language/TYPES_AND_VALUES.md#implemented-integer-bitwise-and-shift-operators).
The pipeline responsibilities are:

- Lexing recognizes eager bitwise, logical, and comparison punctuation by
  longest match and keeps prefix `!` distinct from postfix unwrap by position.
  Syntax retains each operator, unary or binary operand shape and span, or one
  primitive cast target and operand. It assigns no numeric meaning or target
  behavior.
- Resolution preserves bitwise, comparison, and logical-negation shape.
  Primitive casts preserve their primitive target without declaration lookup,
  while nominal and shared object-cast targets continue through existing
  identity lookup; lower phases never disambiguate cast kinds from source
  text.
- Type checking is the sole owner of operation selection. It requires matching
  `i64`, `u64`, `u8`, or `f64` comparison operands, admits `bool` only for
  equality and inequality, selects logical negation only for `bool`, selects
  exact-width complement, AND, OR, or XOR only for matching integers, or
  selects any of the twenty-five primitive cast pairs. Unsupported operations
  and implicit conversions are rejected before HIR.
- Typed HIR records the selected primitive comparison predicate and operand
  kind, exact-width bitwise operation, or exact boolean logical-negation
  operation. Primitive casts use the cohesive primitive-wide HIR operation
  described below, carrying exact source, target, semantic class, and failure
  capability. Type checking constructs both pure and checked classes. Neither
  representation retains a backend condition code, register width, or
  spelling-based signedness choice.
- MIR lowering evaluates eager operands left to right and every unary or
  binary operand exactly once. Bitwise operations become same-type pure scalar
  rvalues, comparisons and negation become typed boolean-producing rvalues;
  non-failing casts become ordinary pure rvalues with no trap, call,
  allocation, cleanup, or exceptional control-flow edge.
- MIR verification proves matching comparison operand definitions and types,
  rejects boolean ordering, and requires exact boolean negation operands and
  results. Bitwise verification proves its closed integer matrix, while
  pure-cast verification proves all twenty-two non-failing pairs with exact
  source and result types. Checked-cast verification proves each matching
  range diamond, success-only conversion, result join, and terminal failure
  edge. Both operation families retain the existing
  block-local value, definition-before-use, and deterministic-error
  invariants.
- Each backend receives already selected signedness and width through verified
  MIR. The x86-64 target realizes exact-width complement, AND, OR, and XOR,
  signed `i64` ordering, unsigned `u64`/`u8` ordering, IEEE unordered `f64`
  comparison, and boolean negation/equality with canonical results. It realizes integer
  casts through canonical scalar loads and stores: same-width bits are
  preserved, narrowing retains the low byte, and `u8` widening zero-extends.
  Selection does not infer semantics from source spelling or expose target
  registers to MIR.

These operations add no ownership or lifetime rule and no public runtime ABI.
Checked, saturating, implicit, mixed-type, and user-defined operations remain
outside this boundary. Implemented short-circuit logic uses the structured
boundary below rather than this eager scalar boundary.

## Implemented checked-shift source boundary

Lexing and parsing recognize longest-match `<<` and `>>` in one
left-associative tier between additive expressions and `&`. Resolution retains
source direction and spans. Type checking alone requires an exact integer left
operand and exact `u64` count, then constructs typed HIR with the selected
direction and `i64`, `u64`, or `u8` left kind. The result is the left type and
right-shift flavor is derived target-independently.

Lowering evaluates and secures the left operand, then evaluates and secures
the count. A dedicated MIR terminator compares the secured count with the
operation width. Its success block alone reloads both carriers, performs the
matching shift, stores the result carrier, and joins; its failure block
directly terminates with `shift count out of range`. The result reload starts
the join. Successful full-expression cleanup remains at its ordinary enclosing
boundary, while the non-returning failure edge retains the existing
non-unwinding panic rule.

MIR verification ties the exact scalar-spill carriers, operation flavor,
success-only shift, result join, dominance, and terminal failure reason into
one checked diamond. An unchecked shift or an alternate predecessor into its
success block is malformed MIR. Deterministic dumps expose
`shift-count-check`, `shl`, `sar`, or `shr` with exact types and width; they do
not expose target registers.

## Implemented primitive operator representation

The
[implemented primitive operator profile](../language/TYPES_AND_VALUES.md#implemented-primitive-operator-profile)
uses one complete target-independent representation boundary across every
source-to-native phase product.

Lexing and syntax retain exact operator spellings, source order, operand
shapes, grouping, and operator/operand spans. Longest-match tokenization keeps
multi-character punctuation intact. Resolution preserves operator identity
without selecting target instructions, signedness, widths, or conversions.

Type checking is the sole owner of primitive operation selection. It:

- requires the exact operand matrix from the language contract;
- inserts no cast, promotion, narrowing, truthiness conversion, or
  expected-type literal reinterpretation;
- selects a result type and one exact semantic operation;
- records wrapping width and signedness where relevant;
- distinguishes integer from binary64 division;
- distinguishes arithmetic and logical right shift;
- records whether an eager operation can reach a compiler-known failure; and
- retains `is` as the existing specialized type or presence test rather than
  an equality operation.

Explicit primitive casts remain separate HIR operations that complete before
operator selection. Operator HIR cannot observe cast provenance.

Typed HIR represents eager unary and binary operations as exact typed values.
Each operation retains its operand type, result type, semantic flavor, source
span, and failure capability without encoding a backend opcode. Boolean `&&`
and `||` remain structured short-circuit operations in HIR so a skipped right
operand is absent from abstract execution rather than marked as an eager value
whose effects may later be discarded.

The internal HIR and MIR models include checked integer division and remainder
operations for exact `i64`, `u64`, and `u8`. They retain quotient versus
remainder, floor signed quotient, divisor-signed remainder, the defined
signed-minimum pair result, and the distinct zero-divisor reason. Both are
classified as control-affecting. HIR-to-MIR lowering evaluates and secures the
dividend and divisor in source order, then emits an explicit divisor-check
diamond whose success edge alone performs the operation and initializes its
result carrier. MIR verification proves that relationship and its exact typed
carriers before accepting the operation. Source `/` and `%` construct these
operations after exact same-integer type selection. The x86-64 target executes
verified operations with explicit
zero and signed-overflow guards, native unsigned or signed division, and the
required signed floor correction.

Source type checking selects exact `f64 / f64` as an ordinary non-failing
binary64 HIR operation while preserving the separate checked integer family.
It reuses ordinary eager binary operand lowering, including securing the left
value when a control-affecting right operand changes blocks, and introduces no
semantic check, failure edge, or termination reason. MIR verification requires
exact `f64` operands and result before the x86-64 backend may consume it.

The HIR and MIR comparison models carry an explicit `f64`
operand flavor for all six predicates. Each comparison remains one eager,
pure, non-failing scalar rvalue with exact `f64` operands and a canonical
`bool` result; dumps retain the predicate as `eq.f64`, `ne.f64`, `lt.f64`,
`le.f64`, `gt.f64`, or `ge.f64`. MIR verification rejects operand and result
type mismatches before target lowering. Source type checking selects this
flavor only for two exact `f64` operands and rejects mixed primitive types and
boolean ordering before HIR.

MIR lowers eager primitive operations to target-independent scalar operations.
It preserves:

- wrapping `i64`, `u64`, and canonical `u8` arithmetic;
- floor signed division and divisor-sign remainder;
- the non-failing signed-minimum division/remainder pair;
- exact bitwise width and signed or unsigned right-shift flavor;
- checked shift-count and integer zero-divisor failure reasons;
- IEEE binary64 operation and unordered comparison flavor; and
- canonical `bool` and `u8` results.

Structured short-circuit HIR lowers to ordinary MIR branches and jumps with one
selected canonical `bool` result. A selection diamond records whether the
right operand runs before entering that operand, which lets recursively nested
logical expressions inherit an already selected parent path. The short and
right paths write one explicit target-independent result carrier and reload it
only after their result join. MIR has no eager logical scalar operation whose
lowering may evaluate both operands.

The selected path owns only temporaries, checked views, guards, and anchors
that it actually establishes. Every completed full-expression temporary
remains live to the enclosing boundary. A join before that boundary must either
represent path-dependent lifetime and conditional cleanup explicitly or keep
affected continuations distinct until their lifetime states are compatible.
Consumer-bounded optional payload views retain their immediate-consumer
lifetime; they are not promoted to full-expression temporaries.

The source pipeline recognizes longest-match `&&` and `||`, preserves their
precedence and grouping in distinct AST and resolved nodes, and checks both
operands in source order for exact `bool`. Successful selection constructs the
structured logical HIR directly, so every ordinary expression consumer reaches
the same MIR lowering and verification path described here.

MIR implements the path-dependent representation. A callable body
may declare deterministic path-condition identities. Each identity names
canonical compiler-owned `bool` activation storage, an optional earlier parent
condition, distinct active and inactive predecessors, and their exact merge
block. The
predecessors store `true` and `false` respectively before ordinary jumps to
the merge. A dedicated target-independent value reads the activation storage;
ordinary MIR branches consume that value.

Path-sensitive storage verification retains separate, explicitly selected
alternatives across such a declared merge. A resource live only in one
alternative cannot be used or ended on another. Branching on the matching
path-condition value selects the corresponding verifier alternative.
Alternatives with equal resource state are stored as one compact predicate
cube: each selected condition may be active, inactive, or either selected
value. "Either" remains distinct from a condition absent from the current
epoch, so parent selection and missing-condition diagnostics retain their exact
meaning. A later loop or malformed-CFG edge that changes only a concrete
subset splits that subset, applies the verifier domain's ordinary conflict
merge, and leaves unrelated dimensions compact.
Ending the activation storage epoch requires all conditional storage state to
have converged, ends the predicate fact, and permits the same static identity
to begin a later loop epoch. Child conditions can be selected and read only
inside an active parent alternative. Undeclared ordinary joins retain their
existing exact-state checks.

The full-expression owner records resource completion as one ordered sequence
of unconditional or path-conditioned registrations. At the enclosing
boundary it walks that sequence in reverse and surrounds each conditional
cleanup or storage death with a local path-condition decision. An ancestor is
tested before a child activation can be read; independent sibling conditions
are tested separately and can therefore both select cleanup. Each decision
reconverges immediately after its action instead of cloning the later
continuation. Scalar result carriers and later unconditional resources are
established before this cleanup graph, remain live through it, and are ended
only after selected-path resource state is compatible. Child activation
epochs end inside their active parent path, followed by root activations.

Storage-lifetime, inline-object cleanup, optional definite-initialization,
shared-owner, checked-view, array-owner, and array-anchor verification retain
distinct states for the declared path alternatives. Conditional cleanup must
destroy or release exactly the selected temporaries in reverse completion
order, leave the skipped alternative untouched, consume selected optional and
aggregate ownership state, and converge before activation storage dies.
Cleanup history for storage whose epoch has ended may differ between
alternatives; live objects and owners, checked views, outstanding cleanup,
array backings and anchors, argument ownership, and temporary order may not.
Backend lowering sees only the resulting ordinary loads, branches, cleanup
instructions, lifetime markers, and jumps.

Dedicated exact-`bool` logical HIR and its primitive/call-capable MIR lowering
are implemented for internal construction. Logical metadata retains the
operation, selection path, right and short regions, result carrier, and join so
verification can reject malformed control-flow shapes while the backend stays
generic. Every block in the right-only region must have no incoming edge from
outside that selected region. Right-operand completions inherit the selected
logical condition; left completions retain the enclosing condition. Internal
operands may complete inline objects, class or shared optionals, ordinary
shared owners, checked shared-backed places, arrays, and array anchors.
Consumer-bounded optional views end before their operand publishes its scalar
result, while primitive unwrap copies and secured optional-owner unwrap retain
their existing lifetime categories. Compiler-known failures remain terminal
inside the selected operand and are unreachable from the short path.
Conditional and loop conditions consume selected cleanup and activation
storage before either successor or the next loop epoch; returns secure their
scalar before cleanup. The accepted source surface reaches this representation
through every currently valid exact-`bool` operand and expression consumer.

The parser caps nested logical operations before semantic phases. Within that
budget, a flat chain produces linear logical diamonds and sibling condition
state. Right nesting produces linear diamonds plus quadratic local cleanup
decisions because each selected lifetime must test its ancestor chain. These
decisions reconverge immediately and never clone an arbitrary later
continuation.

MIR verification rejects:

- operand or result types outside the operator matrix;
- a non-`u64` shift count;
- operation flavors inconsistent with signedness or width;
- noncanonical `bool` or `u8` production;
- an eager or multiply evaluated logical right operand;
- use of a logical result without a defined selected path;
- cleanup of a skipped-path temporary or loss of a completed-path temporary;
- joins with unrepresented incompatible lifetime state;
- a source-reachable divide or shift fault without its semantic failure
  reason; and
- a compiler-known failure edge with an ordinary successor.

The static-termination representation includes the executable checked-shift,
integer-division-by-zero, and integer-remainder-by-zero reasons. Each has
deterministic MIR vocabulary, participates in verified checked control flow,
and selects its exact static message from the
[language panic catalog](../language/ERRORS.md#frozen-panic-design).

Constant folding and every later transformation use the same wrapping,
division, remainder, shift, NaN, panic, evaluation, ownership, and
short-circuit rules as unoptimized execution. Algebraic identities are invalid
when they remove required effects, change NaN results, suppress a panic, or
alter temporary completion and cleanup.

Generated target code mechanically realizes verified MIR. Exact Rust enum
names, module organization, basic-block numbering, temporary-storage
selection, branch shape, instruction sequence, and optimization algorithm
remain private. The operator profile adds no public runtime ABI entry point
beyond the existing common panic reporter.

## Implemented interface-based operator representation

The implemented [operator-protocol lowering contract](OPERATOR_OVERLOADING.md)
extends semantic selection while preserving the existing phase boundary.
Syntax and resolution retain source operator shape and evidence. Semantic
selection chooses either an exact existing primitive operation or one unique
canonical `std::ops` application from the static left operand or its declared
generic bounds. Expected result types, implicit conversion, and specificity
ranking do not participate.

Definition-site generic selection records the canonical template requirement
and structural operand/result terms. Specialization closes that record to an
ordinary class witness or compiler-owned primitive operation without
reselection. Class realizations become existing HIR interface calls; primitive
realizations become existing HIR primitive operations. No unresolved operator
protocol reaches completed HIR, and MIR gains no overloaded-operator node,
dispatcher, effect model, backend lookup, or runtime service.

Produced primitive RHS expressions rely on the implemented caller-owned
read-only scalar alias temporary. Lowering otherwise reuses existing receiver,
argument, result, effect, panic, anchor, and reverse full-expression cleanup
plans. MIR verification proves only the resulting ordinary call or primitive
operation and rejects injected unresolved or mismatched realization evidence.

This representation and its verifier hardening are implemented. The
[implemented primitive representation](#implemented-primitive-operator-representation)
continues to own every primitive realization.

## Frozen complete primitive cast representation

The
[complete explicit primitive cast matrix](../language/TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix)
has a cohesive source-to-native path for all twenty-five cells. Type checking
selects them into typed HIR. Lowering represents twenty-two as pure MIR and
the three checked `f64`-to-integer cells as the verified control flow described
below; x86-64 executes every form inline.

Lexing recognizes all five primitive type keywords. Syntax uses one
primitive-cast node retaining the exact `i64`, `u64`, `u8`, `f64`, or `bool`
target, target span, right-associative unary operand, complete span, grouping,
and common nesting budget. Every primitive keyword in cast-target position
selects this form without declaration lookup. Nominal and `shared` object-cast
targets remain separate syntax and resolution paths; lower phases never
redisambiguate a cast from source text.

Resolution preserves the primitive target and resolved operand without
selecting conversion behavior. Typed HIR has the cohesive complete-matrix
operation described below. Type checking accepts exactly the complete
twenty-five-pair matrix, inserts no implicit use of a cast at another typed
boundary, and constructs typed HIR carrying:

- exact source and target primitive types;
- one target-independent semantic class: identity, integer bit conversion,
  conversion to `bool`, conversion to `f64`, conversion from `bool` to an
  integer, or checked `f64`-to-integer;
- whether conversion may terminate; and
- the source span needed by deterministic diagnostics and failure lowering.

The semantic class is derived once when typed HIR is constructed. HIR does not
encode an x86 instruction, register class, host-language conversion, runtime
helper, or constant-folding decision. Same-type casts remain represented semantic
operations rather than being erased before typed inspection. Identity `f64`
casts preserve the complete binary64 datum; other conversions preserve the
exact source-visible rules rather than storage coincidences.

HIR-to-MIR lowering evaluates every pure cast operand exactly once. Identity,
integer-to-integer, boolean/numeric, numeric/boolean, and integer-to-`f64`
casts become ordinary pure primitive-cast rvalues carrying exact source,
target, and selected semantics. They add no block, call, failure edge,
allocation, ownership action, or cleanup obligation.

The three checked `f64`-to-integer cells are structurally different. Lowering
secures the evaluated `f64` operand in canonical scalar storage, then emits an
explicit primitive-cast range-check diamond. The check's success edge alone
performs truncation toward zero, initializes one exact target-typed result
carrier, and reaches the join. Its failure edge terminates with the distinct
primitive-cast-out-of-range reason from the
[language catalog](../language/ERRORS.md#frozen-panic-design). The join reloads
the result as an ordinary block-local scalar value. No conversion rvalue may
be reachable except through its matching successful check.

The range check is defined over the mathematical truncated result, not merely
the original floating value or a target instruction's sentinel result. It
rejects NaN and infinities before conversion and accepts a finite negative
fraction greater than `-1.0` for an unsigned target because truncation produces
zero. A valid constant source follows the same path semantics as a runtime
value: optimization may replace a successful conversion with its exact
constant result or replace a known failure with the same terminal reason, but
must not change it into a literal-range diagnostic, remove an observable
failure, or make behavior depend on optimization settings.

Control-effect discovery classifies checked `f64`-to-integer conversion as
control-affecting regardless of operand purity. Earlier scalar values that
must survive its block change are spilled through the existing canonical
storage boundary. Full-expression temporaries, optional guards, checked
views, shared anchors, and other ownership state established before the check
retain the ordinary rule for unrecoverable termination: success reaches their
normal cleanup boundary, while failure does not return and guarantees no
remaining source-level cleanup.

For ordinary primitive-cast rvalues, MIR verification proves:

- a legal source/target pair and selected semantic class;
- exact operand, result-carrier, and rvalue types;
- block-local definition-before-use for every pure cast;
- rejection of a checked semantic class when it is encoded as an ordinary
  primitive-cast rvalue;
- one secured `f64` source, matching check, success-only conversion, unique
  result initialization, result join, and terminal catalog reason for every
  checked cast; and
- deterministic error accumulation and dumps under malformed-MIR mutations.

HIR and MIR dumps expose exact source and target types without target registers
or helper names. Syntax, resolution, HIR, and MIR public facades expose
cohesive primitive types and casts with no parallel integer-only cast model.
Source, HIR, and MIR fixtures cover the complete matrix through native
execution, including checked success and failure paths.

The canonical private `std::f64` intrinsics reuse the primitive operation
pipeline without changing explicit-cast syntax. Type checking replaces
`_to_bits(f64) -> u64` and `_from_bits(u64) -> f64` calls with a distinct
`bit_reinterpretation` HIR semantic class. It is valid only for the exact
`f64`/`u64` pair in either direction, is pure and non-terminating, and remains
distinguishable from the numeric conversions selected by `(u64) value` and
`(f64) bits`. MIR carries the same source type, result type, operand, and
semantic class as an ordinary scalar rvalue. Verification rejects any other
type pair or mismatched result and proves definition-before-use. Intrinsic
declaration metadata may remain for deterministic whole-program identity, but
no intrinsic call survives typed HIR.

Focused validation covers all twenty-five pairs through
syntax, resolution, type checking, HIR, MIR, verification, target legality,
assembly, and native observation. Boundary coverage includes integer extrema,
values around `2^53`, both floating zeroes, subnormals, infinities, multiple
NaNs, every integer target boundary and adjacent binary64 value, negative
fractions near zero, exactly-once evaluation, nested checked casts, surrounding
control effects, deterministic phase products, and optimization parity. The
current MIR pass pipeline performs verification without transformations and
is tested to preserve both pure and checked casts exactly.

## Sources and diagnostics

`SourceDatabase` owns source text and assigns `SourceId` values. `Span` and
`TextRange` use UTF-8 byte offsets into an owning source. Human-facing lines
and columns are one-based; columns count Unicode scalar values, with rendering
policy kept outside the source model.

Diagnostics are structured data containing severity, code, labels, and notes.
Phases accumulate them rather than printing. Rendering is a separate,
deterministic operation over diagnostics and their source database, so tests
and tools can inspect structure without parsing display text.

Source errors are expected compiler results, not Rust panics. Recovery may
produce more than one diagnostic within a phase; the complete driver does not
send an erroneous phase product into the next semantic phase.

## Lexer

The lexer owns token formation, trivia handling, and preservation of source
spellings. It does not perform name lookup, choose semantic types, or convert
numeric payloads into checked language values.

Integer token kinds carry their radix structurally alongside the preserved
source spelling. Implemented integer forms carry either decimal or hexadecimal
radix; later phases consume that classification rather than rediscovering a
prefix from text. Floating literals remain a distinct radix-free kind.

Single-quoted byte literals have a dedicated token and lexical diagnostic
family. Their scanner validates that the spelling decodes to exactly one byte,
recovers through a closing quote or at a physical newline, and shares only
delimiter-independent hexadecimal decoding with strings.

Syntax and resolved IR preserve an integer literal's complete spelling and
explicit radix for source-facing dumps and diagnostics. Type checking removes
the known prefix and suffix and converts the magnitude exactly once to the
existing typed HIR integer constant. No hexadecimal distinction survives into
MIR, verification, code generation, or the runtime.

The source AST and resolved IR retain a distinct byte-literal node containing
the decoded `u8` and complete span; their dumps render the byte as two lowercase
hexadecimal digits. Type checking immediately lowers that node to the existing
typed HIR `u8` constant, so no byte-literal source distinction reaches MIR,
verification, code generation, layout, ABI, or runtime behavior.

`LexOutput` contains tokens and diagnostics together. Invalid characters and
malformed numeric, byte, and string spellings are diagnosed while retaining a
recoverable token stream. Token kinds and accepted lexical forms follow the
[grammar authority](../language/GRAMMAR.md).

## Syntax

The recovering parser produces an unresolved, source-oriented AST. Nodes keep
spans and spellings needed by later diagnostics, including exact private field
and method modifier spans, but contain no selected declaration identities,
inferred types, access decisions, or target details.

Grammar nesting uses a shared finite budget. Exceeding it is a source
diagnostic with recovery rather than unbounded recursion. The precise accepted
source shape, recursive nesting limit, and separate logical-expression limit
are owned by the
[implemented grammar](../language/GRAMMAR.md).

Optional AST nodes retain separate payload, `shared`, `?`, `!`, `is`, and
presence-target spans. `none`, presence tests, and unwrap are distinct
expression nodes; malformed and reserved optional type combinations recover
without entering later phases.

## Resolution and identities

Resolution is the only compiler phase that selects declarations from source
names. It first collects program and member declarations, then resolves
callable bodies, which permits forward references without making lower phases
name-dependent.

The resolved program replaces successful name uses with typed identities for
functions, classes, interfaces, interface requirements, members, callables,
parameters, locals, and bindings.
Its module table and explicit top-level owners preserve source-module
provenance independently of those declaration IDs. Changing only the selected
entry in a table does not reorder table entries or declaration identities.
Optional direct class bases likewise carry `ClassId` rather than source
spelling. Callable-owned identities also scope later local MIR identities.
Declaration tables retain deterministic identity order, and later phases
select entries by identity rather than by source spelling.

A reachable canonical `std::ops` module additionally produces one atomic
`ResolvedOperatorLanguageItem`. Its fixed canonical table records all
seventeen exact generic-interface templates, structural parameter identities,
and requirement identities after whole-bundle validation. Ordinary imports or
direct entry selection alone create reachability; primitive operator tokens
create no module edge. Malformed bundles publish no partial product, while
valid declarations remain ordinary generic interfaces for explicit claims,
bounds, views, and calls. Class and generic-bound operator selection consumes
this table by identity; completed HIR retains only the resulting ordinary
interface call or existing primitive operation.

External declarations additionally retain their source `FunctionId` while
referencing a dense compilation-wide `ExternalLinkId`. Resolution allocates
links in exact symbol order, groups every compatible declaration of that
symbol, and reports incompatible ABI signatures before HIR. One immutable
external-link table owns the native symbol and ordered declaration membership
through resolved IR, HIR, and MIR. Verification checks the table and
declarations bidirectionally; the backend reads the symbol only from the
verified link entry.

Recursive array types use dense `ArrayTypeId` values backed by one canonical
resolved table in deterministic first-use order. Each entry records its exact
resolved element type, so nested arrays and grouped element ownership remain
name-independent without recursively embedding owned type trees in phase
values. Ordinary and optional shared targets can name an exact array identity;
arrays remain outside class hierarchy, interface conformance, `Obj`, casts,
and type tests. Resolved construction, projection, and array-assignment nodes
retain their source structure.

The separately frozen
[structural indexing and slicing contract](INDEXING_AND_SLICING.md) has made
the source AST's bracket vocabulary type-neutral. Resolution retains the array
representation above for true arrays and normalizes eligible class and
interface index and slice operations to ordinary calls before HIR.
Independently omitted slice bounds become typed optional arguments through
ordinary call checking; no synthetic length call or repeated receiver
evaluation is introduced. Class calls retain ordinary direct or virtual
selection and interface calls retain ordinary witness selection. The
extension adds no structural HIR, MIR, verifier, backend, or runtime operation.
Primitive and owning result and replacement families, checked receiver
anchors, static effects, self-aliasing arguments, and reverse cleanup are
verified through the existing ordinary-call phase products.

Type checking lowers array declarations, owning locals/fields/signatures,
inline and shared construction, exact element lifecycle capabilities,
projection, replacement, slices, aliases, and named-copy versus
produced-adoption provenance into deterministic HIR. Recursive class/array
capability analysis terminates at a fixed point, while array backing remains
excluded from finite inline-containment edges.

All supported array HIR lowers to verified target-independent MIR. Canonical
array declarations and explicit storage roles describe ownership without
choosing a descriptor layout. Generated array loops, checked allocation,
signed position normalization, projections, slice checks, publication,
adoption, replacement, element lifecycle, cleanup, and anchors remain explicit
through the verifier boundary. The x86-64 backend executes empty and
dynamically sized inline and shared-outer arrays containing primitives,
optionals, exact classes, recursively nested inline arrays, and ordinary or
optional shared owners of exact classes and arrays. This includes length,
checked element access with signed negative-relative indices, named deep copy,
produced-backing adoption, arbitrary-length replacement, class fields,
internal owning calls/results, exact shared defaults, secure shared-element
replacement, and deterministic element cleanup. Its legality pass
accepts the complete verified array operation profile before instruction
selection.

Explicit array element-list construction preserves one additional
ordered construction mode from syntax through resolution. The explicit type
resolves to its canonical `ArrayTypeId` before element checking; syntax and
resolved IR retain both braces, comma spans, and every element expression in
source order. Type checking replaces those expressions with exact
destination-directed initialization plans for primitive, class, optional,
nested-array, shared-owner, and optional-owner elements. Those plans retain
initializer and copy identities, named versus produced provenance, access,
and ownership operations rather than leaving lower phases to infer them.

Primitive element plans now lower to `AllocateElements`, an exact constant
count and zero initialized prefix, followed by ordered `InitializeElement`
operations and complete inline or shared publication. Allocation and its
failure edge precede every element effect. Verification proves exact primitive
types, unique increasing positions, prefix completion, backing consumption,
and enclosing full-expression lifetime; no default live elements or
assignment loop is introduced. The x86-64 backend reuses checked allocation,
layout-specific primitive stores, publication, and release, while the runtime
remains unaware of the list and prefix.

Exact-class plans reuse the existing destination-directed `Initialize`,
object-result `Call`, `StringInitialize`, and `CopyConstruct` operations with
the current prefix slot as their complete destination. A focused
`CompleteElement` operation advances the prefix only after one exact slot
construction has completed. Verification carries that completion fact across
CFG edges and rejects missing, duplicate, wrong-slot, post-publication, or
premature advancement. Existing full-expression cleanup owns grouped produced
sources, and existing class-array release destroys published elements in
reverse order. Inline optionals reuse their ordinary absent/present operations;
class payloads progress through absent-wrapper initialization, completed
payload construction, presence publication, and only then outer-prefix
advancement. Nested inline-array plans recursively deep-copy named sources or
consume produced descriptors through exact-identity `Adopt` operations before
`CompleteElement` advances the outer prefix. The ordinary recursive array copy,
anchor, adoption, and reverse-release machinery remains authoritative. Shared
owner plans reuse typed shared temporaries: named sources retain into the
temporary, produced sources adopt into it, and slot initialization consumes it
before prefix advancement. Optional shared plans reuse their generic-place
zero-niche initialization and conditional owner transfer. Array-prefix state
and shared-owner state verify independently, including nested shared-array
owners. The detailed representation boundary is in
[the array compiler contract](ARRAYS.md#element-list-representation).

Optional types use deterministic interned identities rather than recursively
wrapping the general type enum at every use. Resolved expressions retain
explicit absence, presence-test, and unwrap nodes. Canonical semantic dumps use
`T?` and `(shared T)?` independently of source trivia; syntax dumps retain
`shared? T` shorthand provenance. Shared optional boxes add a static
box-view identity only where class/interface/`Obj` views carry information
absent from the exact optional allocation identity.

Resolved programs contain one canonical class hierarchy keyed by `ClassId`.
It validates cycles, traverses direct-to-root chains, answers subtype and
nearest inherited-member queries, and preserves each selected field or
method's declaring owner. Virtual roots allocate deterministic family and slot
identities; each explicit override records that family, its root, and the
nearest overridden declaration. Inherited collision checks, override
resolution, and finite-containment analysis consume the canonical hierarchy
instead of rebuilding ancestry from declarations. Interface calls likewise
select one requirement identity during resolution; later phases do not repeat
requirement-name lookup.

Resolved field and method declarations retain source member visibility.
Ordinary lookup first selects the nearest identity without filtering the
inherited namespace, then one centralized check compares that identity's
declaring `ClassId` with the callable's lexical class owner. This gives
unknown-member and inaccessible-member diagnostics deterministic precedence
before member-kind, receiver-access, or type checking. Private methods are
excluded from virtual families and interface conformance. Once access is
authorized, visibility is deliberately erased: HIR, MIR, verification, layout,
lifecycle lowering, and target code operate on the same field and method
identities as public members.

Resolved ordinary initializer declarations likewise retain per-overload
visibility and the exact modifier span. Type checking first performs ordinary
applicability and unique-most-specific selection, then applies the same exact
declaring-class owner comparison to the selected `InitializerId`. Direct,
shared-allocation, base, and class-element array-default construction all use
that centralized check. No-match and ambiguity stop before access checking.
Authorized HIR deliberately erases initializer visibility along with field
and method visibility.

## Private cell field representation

Status: **implemented contract**. Declaration metadata, typed write
authorization, lifecycle and alias composition, specialization, dispatch,
independent MIR verification, and target lowering are implemented. The source
meaning is defined by
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md#private-cell-field-direction).

Syntax retains one explicit `cell` modifier and exact span on an ordinary
private instance field. Resolution preserves the field's existing dense
`FieldId`, declaring `ClassId`, visibility, name, type, declaration order, and
span plus one cell marker. Generic template collection and closed
specialization preserve and substitute that same declaration property. HIR
and MIR field declarations retain the modifier span, and MIR verification
rejects structurally invalid evidence. The
marker does not create another field identity, wrapper type, member category,
layout edge, or lifecycle slot.

Unlike ordinary visibility, the durable cell capability must remain available
after source access is authorized because typed and MIR trust boundaries need
to verify the exceptional write. HIR and MIR field declarations therefore
retain cell metadata while continuing to erase private visibility after
resolution. Deterministic syntax and resolved dumps show `private cell`; HIR
dumps expose a selected cell-write authorization without changing the
receiver's read-only access. MIR dumps expose the exact authorization beside
the ordinary assignment instruction as `cell-write <FieldId>`.

Verified cell assignments retain the ordinary path-state protocols around
them. Exact-class and optional replacement uses the current field lifecycle
plan, shared-pointee calls keep their hidden owner anchors, inline-array
element calls keep detached-backing anchors, and checked optional payload
views remain guarded until the call ends. Read-only methods seed initialized
receiver optionals for both checked views and guarded mutation checks; cell
authorization does not stand in for, weaken, or bypass those proofs.

Type checking authorizes assignment through a read-only object place only
when all of these facts hold:

1. the destination ends at one selected field identity;
2. that field carries the cell marker;
3. the current callable's lexical class owner is the field's exact declaring
   class; and
4. the operation replaces that complete field rather than mutating a nested
   projection or forwarding it as mutable.

Existing name selection and declaring-class privacy run first and retain their
diagnostic precedence. Initializer writes remain direct initialization through
their existing incomplete-receiver rules. A genuinely mutable root retains
ordinary access independently of the marker.

Typed HIR represents a cell write as an ordinary type-directed assignment plus
this explicit field-write authorization:

```text
HirFieldWriteAuthorization = Mutable | DeclaringClassCell
```

Complete replacement destinations carry this authorization on their
`HirFieldPlace`, whose existing `field` member is the exact authorized
endpoint. Reads and initialization destinations carry none. The same
typed field place is nested in scalar stores, exact-class copy assignment,
optional writes, shared-owner replacement, and array replacement, keeping the
access decision centralized. It does not upgrade
`HirObjectReceiver`, `HirObjectPlace`, or a complete projection path to
mutable, because that would incorrectly authorize mutable methods, nested
fields or elements, optional payload mutation, shared-pointee mutation granted
only by cell, and `mut ref` arguments.

MIR retains corresponding authorization on scalar stores, exact-class copy
assignment, primitive/class/aggregate/shared-owner optional assignment,
shared-owner replacement, and array replacement:

```text
MirCellWriteAuthorization { field: FieldId }
```

Ordinary mutable writes carry none. Preliminary and final MIR verification
independently prove the declared marker, exact endpoint, enclosing callable
owner, read-only receiver access, field type, assignment family, place
liveness, optional guards, shared and array anchors, ownership transitions,
and cleanup. Forged authorization on an ordinary or static field, nested
destination, wrong class body, initialization operation, mutable destination,
malformed origin, or mismatched assignment family is invalid MIR. The former
`MIR001` executable gate is removed because authorized writes now cross both
verified boundaries.

The operation otherwise reuses ordinary assignment lowering. Existing scalar,
copy-assignment, optional, shared-owner, and array instructions continue to
own evaluation order, self-assignment, retain/adopt/release, displaced-value
destruction, detached backing, presence transitions, failure, and
full-expression cleanup. An active optional payload guard prevents cell
replacement before invalidation; shared-owner and array-backing anchors keep
old aliased storage alive exactly as they do for replacement through another
mutable path. No raw store may bypass these plans.

At the sealed final-MIR boundary, verification consumes the authorization
proof. Backend target selection then uses the ordinary field address and
assignment machinery; the evidence produces no target instruction. Cell metadata changes
no field offset, object size,
alignment, calling convention, dispatch table, symbol family, runtime call,
public C API, or runtime ABI version. It carries no atomic, volatile,
synchronization, thread-local, or runtime borrow semantics.

The compiler-known `std::str::Str` descriptor composes the implemented cell
permission with intrinsic literal construction: its fourth exact field is a
private-cell `u64?` hash cache, and `StringInitialize` publishes it absent
without adding a runtime or target-level cell operation.

## Final field representation

Status: **implemented contract**. Source semantics are defined by
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md#final-fields)
and [Static Fields](../language/STATIC_FIELDS.md#final-static-fields).

Syntax retains one exact `final` modifier span on an ordinary instance or
static field declaration. Resolution preserves the declaration's existing
dense `FieldId` or `StaticFieldId`, declaring class, visibility, name, type,
order, initializer identity where applicable, and source span plus one final
marker. Generic template collection and closed specialization preserve that
property while allocating ordinary specialized identities. Finality creates
no wrapper type, member category, containment edge, layout slot, virtual
family, lifecycle slot, or runtime state.

The canonical source order is `private final static`. Parser lookahead keeps
`private`, `final`, `static`, and `cell` contextual while diagnosing
reordered, duplicated, incomplete, and cross-category combinations. A final
instance field has no declaration initializer. A final static requires an
explicit initializer and cannot use the zero-default static path. AST,
resolved IR, HIR, preliminary MIR, and final MIR dumps expose the marker;
declaration verification rejects malformed spans, incompatible cell metadata,
and zero-default final statics.

Final statics pass through preliminary inventory, effect-certified lifecycle
planning, coordinator synthesis, and final MIR verification before reaching
the backend. Final-bearing complete values use the ordinary assignment backend
path after exact user or synthesized evidence crosses both MIR verification
boundaries. Construction, copy construction, assignment, reads, destruction,
and shallow mutation require no feature gate.

Construction is direct initialization of incomplete storage. Ordinary
and copy constructors use the current exact-once direct-field analysis, and
synthesized copy construction retains final fields in declaration order.
Those operations do not require post-construction write authorization.

Type checking owns one centralized complete-object field-write decision.
Privacy is resolved first. Declaration finality then takes precedence over
ordinary mutable receiver access and the separate declaring-class cell
capability. Every independent final-slot replacement reports `TYP043`,
including writes from the declaring class, methods, static methods, helpers,
destructors, and derived bodies. Reads retain ordinary access, and finality is
shallow: selecting mutable state beneath a final field does not replace the
final slot.

For complete objects, typed HIR must distinguish at least these write reasons:

```text
FieldWriteAuthorization = Mutable
                        | DeclaringClassCell
                        | DeclaringClassFinalAssignment(CopyAssignmentId)
```

The concrete Rust representation remains implementation-private. An ordinary
mutable field write uses `Mutable`; an exact private-cell write retains its
existing authorization. Final-assignment authorization is created only for a
direct endpoint in the exact declaring class's selected user copy-assignment
body. It does not flow through helper calls, base projections, or nested
projections, does not clear declaration metadata, and does not broadly upgrade
a receiver, object place, or class body.

Synthesized copy assignment carries equivalent exact evidence on every final
field step in its selected capability plan. User-defined assignment remains a
general mutable body and may contain control flow, calls, zero or repeated
final writes, and arbitrary supported source expressions. Whole-object
assignment keeps its existing destination, source evaluation, operation
selection, self-assignment, and live-lifetime rules whether or not the class
contains final fields.

HIR-to-MIR lowering preserves final-assignment evidence on every affected
scalar, exact-class, optional, shared-owner, and array replacement carrier.
Preliminary and final MIR verification independently prove the declared
marker, direct endpoint, exact lifecycle owner, assignment family, field type,
place liveness, initialization state, optional guards, ownership transitions,
anchors, and cleanup. Forged evidence in a method, destructor, helper, derived
assignment, nested field, inherited field, construction operation, ordinary
mutable write, or mismatched lifecycle is invalid MIR. Synthesized plans must
prove the same ownership and field-order facts rather than relying on source
absence.

A final static declaration retains its explicit initializer through the
existing preliminary lifecycle definition, effect analysis, deterministic
plan, compact proof, coordinator synthesis, and final verification. The planned
schema must distinguish explicit final publication from an ordinary mutable
static and reject zero-default initialization or any later source root write.
Normal reverse shutdown remains a lifecycle cleanup, not an assignment.

After final verification, target lowering uses existing field/static address,
copy, ownership, optional, array, publication, and cleanup machinery. Final
metadata changes no size, alignment, offset, register classification, symbol
family, runtime call, public C API, or runtime ABI version. Backend code must
not infer authorization from source names or reconstruct it after verification.

Rebinding-capable field/static aliases are checked at a shared
type-checking boundary and independently rejected by MIR verification when
forged; shallow class, array, optional-payload, and shared-pointee mutation
continues through existing access and anchor rules. Standard-library primitive
boxes use ordinary public final fields and receive no compiler exception.

Resolved IR remains source-oriented: it records selected declarations and
object paths, but does not decide final expression types, access validity,
copy capability, storage, evaluation lowering, or ABI placement.

Executable shared type syntax resolves to an explicit class, interface, `Obj`,
array, exact optional, or optional object-view target.
Allocation syntax resolves to an exact concrete `ClassId` and retains ordinary
arguments or the explicit copy source as the existing distinct construction
modes. These facts cross resolution without a feature gate. Type checking owns
their semantic compatibility and the current lower-phase gate.

Explicit shared dereference resolves to a dedicated node containing the
resolved owner expression, class/interface/`Obj` target, source `*`-versus-`->`
provenance, and exact operator and expression spans. The source AST keeps `.`
and `->` distinct; resolution normalizes `->member` to one typed dereference
followed by member selection without synthesizing a source `*` span or
duplicating receiver evaluation. Resolution rejects raw shared member
receivers; other object-place consumers reach type checking as owner
expressions only so that it can issue the corresponding explicit-dereference
diagnostic. No lower phase manufactures a pointee place from a raw handle.

Ordinary construction and copy construction have type-distinct identities
through every semantic phase. `InitializerId` names only an ordinary `init`
candidate; `CopyConstructorId` names the separate copy lifecycle slot, and
both have corresponding `CallableId` variants for parameters, locals, bodies,
verification, and backend symbols. Syntax represents `copy` as a dedicated
class member, and resolution validates its exact-class read-only source
directly into the copy slot. No phase infers lifecycle intent from an `init`
signature.

Ordinary direct construction and direct-base initialization select
initializers below name resolution. Resolution retains the target class
identity and source-ordered arguments, while that class owns the stable,
source-ordered candidate set. Type checking analyzes each argument once,
determines applicability from the existing argument-binding relation, selects
the unique most-specific static parameter-type sequence, checks that selected
initializer against the callable's lexical class owner, and records exactly
one authorized `InitializerId`. An inaccessible private selection does not
fall back to another candidate. HIR and MIR therefore contain neither
unresolved overload choice nor visibility. Both IRs store dense,
source-ordered initializer declaration and definition vectors, and MIR
lowers, verifies, dumps, and emits every entry.

The distinct `copy(ref source: T)` declaration is a separate lifecycle
capability rather than an initializer candidate. `T(copy source)` selects that
capability explicitly and records a target-directed checked exact-`T` source.
Ordinary `T(arguments)` never falls back to copy construction.

Syntax and resolved IR retain ordinary arguments and explicit-copy source as
different construction modes. HIR replaces the former with one selected
`InitializerId` and typed arguments, or the latter with one selected
copy-constructor operation and checked object source. This
destination-oriented representation is also used by resolved `new T(...)`;
later typed lowering does not need to inspect source expression shape.

## Typed HIR

Type checking validates the whole resolved program and constructs HIR only
when no type error remains. HIR records exact semantic types, receiver and
alias access, selected primitive and lifecycle operations, exact callable
targets, object places, construction destinations, copy choices, and
structured flow summaries.

Callable-body resolution and checking record a class-owned body's lexical
`ClassId` independently of whether that body has a receiver. A receiver, when
present, separately records its exact class and access. Resolved and HIR method
declarations use mutually exclusive instance and static kinds: instance kinds
carry receiver access and dispatch, while static kinds carry neither. Source
`static fn` and `private static fn` bodies therefore retain lexical class
ownership without `self` or receiver state. Initializers and lifecycle members
remain receiver-bearing.

HIR calls likewise distinguish receiver-bearing direct/virtual method calls
from receiverless static calls. A static scalar or object producer retains its
selected class-owned `MethodId` and explicit arguments but has no receiver
expression. Primitive, unit, class, shared, optional, optional-shared, and
array results continue through their existing typed result and ownership
forms.

Static inheritance crosses this boundary explicitly. HIR records selected base
initializers, complete lifecycle composition, identity-based base projections,
inherited field and direct-method selections, access-preserving class/`Obj`
alias views, and owning slices with exact target copy operations. It also
retains validated virtual-family declaration metadata. Method-call targets are
explicitly direct or virtual; virtual targets carry the family, slot, and
statically selected method. Receivers and alias views retain either an exact
complete place/dynamic class or a forwarded binding that carries runtime
complete-object and dynamic-class metadata. Destructor `self` origins also
record the declared dispatch limit.

Interfaces cross the typed boundary as declaration tables and deterministic
requirement-to-method maps for every effective class conformance. Interface
alias arguments retain their static interface target, access, and exact or
forwarded complete-object origin. Interface calls name both `InterfaceId` and
`InterfaceRequirementId`.

Type tests retain their class/interface/`Obj` target, selected non-owning
source view, and static-success, static-failure, or runtime classification.
Checked object casts retain an access-preserving result view and either a
static conversion or runtime check with explicit terminating failure.

Type checking derives both operations from one identity-based, closed-world
object-view relation. Exact inline objects resolve against their known dynamic
class; forwarded class, interface, and `Obj` views resolve against the declared
classes that can inhabit the source view. Checked-view selection then preserves
access, projects statically selected class targets, and records terminating
runtime failure.

The implemented [object-cast profile](../language/OBJECT_CASTS.md) uses an
expression-level checked-place operation. HIR retains the source view,
target identity, preserved access/origin, static or runtime classification,
post-cast projections, and immediate consumer target/access. Plain cast views
are bounded by their consuming full expression. Consumers include receivers,
alias arguments, field
access and mutation, and exact-class owning copy construction, assignment,
value arguments, and results. An owning HIR source wraps the checked view and
may add the ordinary exact-ancestor slice path; it does not introduce another
copy operation. Shared copy allocation consumes a target-directed
checked exact-class source in `new T(copy source)` while separately recording
allocation and selected copy construction; allocation is not an effect of a
cast node.

HIR preserves structured source control flow and source spans useful for
diagnostics. It does not contain byte offsets, registers, stack slots, calling
convention locations, or target symbols. Lower phases therefore consume
already checked semantic choices without reimplementing language policy.

Shared types cross this boundary as canonical class, interface, `Obj`, or array
targets, distinct from inline values and non-owning views. Shared value
consumers retain a named owner place or produced owner and explicitly select
copy or adopt. Ordinary `new C(arguments)` retains exact `C`, its selected
`InitializerId`, and typed source-ordered arguments. Shared locals, value
parameters, results, and fields use this vocabulary, including compatible
implicit up-views. Inline values and aliases do not implicitly manufacture an
owner, and external shared signatures remain invalid. Explicit copy allocation
records its checked source and selected exact-class copy operation separately
from ordinary initializer overloads. Shared casts record their source
provenance, static or runtime relation, target, and copy/adopt result ownership.
Box HIR follows that owner vocabulary while adding a distinct
optional-box allocation producer and checked optional-pointee place. It does
not encode wrapper mutation, synthesize a bare owning interface/`Obj` optional,
or lose the exact allocation class behind a polymorphic box view.

MIR lowering accepts compatible shared local initialization and assignment
from named owners and ordinary allocations. It also carries compatible shared
owners through internal function, initializer, method, and interface
parameters and results. Named sources copy, produced sources adopt, calls
consume caller argument owners, callees normally release parameter owners, and
shared returns escape through one dedicated result owner after cleanup.
Assignment secures an owning temporary before releasing its destination and
moves that temporary into the destination. Shared field initialization or
replacement similarly secures a temporary owner before installing it. MIR
distinguishes field-owner copying, initialization, replacement, synthesized
shared-field copy steps, and reverse-order shared-field destruction from
inline containment. The verifier checks field type, access, transfer
ownership, exact initialization on normal initializer returns, lifecycle
metadata, and control-flow agreement. Stable shared locals and value
parameters lower to explicit shared-pointee places and shared object origins
for inherited projection, mutable member access, virtual/interface dispatch,
and `is`. Type checking constructs every such borrowed place through one
checked shared-pointee operation. That operation preserves the source's
class/interface/`Obj` target, mutable access, complete-object origin,
projections, span, and stable-versus-anchored owner provenance; receivers,
aliases, casts, type tests, field access, and owning inline-copy consumers do
not rediscover those facts from expression shape or an expected type. The
verifier ties every such place to a live owner and compatible
header metadata. Shared-backed receivers and alias arguments classify stable,
copied-field, and adopted-produced provenance in HIR, then lower hidden owners
to the explicit `SharedAnchor` MIR storage role. Plain checked places use the
same source classification and anchor storage while retaining a distinct
checked-view carrier through their immediate receiver, alias, field, or
owning-inline-copy consumer. MIR verification tracks the carrier-to-owner
dependency and requires the checked view to end before anchor release.
Produced allocations retain exact dynamic provenance through shared up-views.
Copy allocation lowers the established source and any anchor before allocating,
then performs one exact copy construction before publication and adoption.
Explicit dereference is consumed at this HIR boundary and reuses these
same shared views, origins, checked carriers, and anchors; MIR has no parallel
explicit-dereference place or ownership operation.

## MIR

MIR is executable in shape and target-independent. It makes these concerns
explicit:

- callable declarations and executable definitions;
- addressable storage and semantically projected places;
- dense class-owned static declarations and always-live identity-based static
  place roots, including inline-optional state/payload projections and
  optional shared-owner slots, kept distinct from function-local storage
  carriers;
- canonical direct-base metadata and identity-based base projections;
- transient primitive values;
- source-ordered calls, argument modes, and access-restricted
  class/interface/`Obj` views;
- canonical virtual-family metadata, explicit receiver-bearing direct/virtual
  method targets, receiverless static method targets, and complete-object
  receiver origins;
- dense canonical function-type metadata, exact callable-address scalar
  producers, and receiverless indirect targets carrying a stabilized callee
  value and exact function type;
- interface declarations, effective class conformance maps, and explicit
  interface call targets;
- initialization, copying, assignment, and cleanup operations;
- checked-view sources for owning copy operations, with explicit bounded
  carrier lifetime across any runtime selection;
- destination-oriented ordinary and explicit-copy construction, with runtime
  failure before copy and one exact-class copy instruction on success;
- selected base copy steps, owning slices, and complete destruction plans;
- object-result destinations and full-expression temporary boundaries; and
- distinct unpublished shared-allocation storage plus explicit exact
  allocation, initialization, publication, produced-owner adoption,
  named-owner copy, temporary-to-local owner move, release, and ownership
  full-expression boundaries;
- projected shared-field copy, initialization, secure replacement, synthesized
  lifecycle steps, and reverse-order destruction-plan releases; and
- checked-place carriers with explicit shared-owner dependencies and
  full-expression-ordered view end before anchor release; and
- basic blocks with explicit return, jump, boolean-branch, checked-cast, and
  unrecoverable-failure terminators.

MIR is not SSA. State that crosses control-flow edges uses storage. Class
objects remain addressable places rather than transient scalar values. Field
and base projections carry semantic identities rather than target offsets.
Every class-owned MIR definition records its exact owner and an optional
receiver storage identity as separate facts. Verification requires the owner
to agree with the callable identity, requires an identified receiver to name
exactly one correctly owned receiver storage slot of the owning class, and
rejects receiver storage when the optional identity is absent. All MIR
method declarations use an instance-or-static kind matching their definitions.
Static calls lower to `MirCallTarget::Static(MethodId)` with no
`MirCallReceiver`; explicit arguments retain source order and use the ordinary
argument, destination, ownership, and cleanup machinery. Static methods are
rejected from virtual families and interface conformance maps. Source class
selection produces this receiverless target directly; resolution has already
selected inherited identity and enforced declaring-class privacy.
Static views retain their source place, target, access, and complete-object
origin; slices are exact target-class copy operations from a verified
base-projected source.

Function references lower to `MirType::Function(FunctionTypeId)` values formed
only by an exact `CallableId` address or a typed storage load. Indirect calls
evaluate their callee once before explicit left-to-right arguments and reuse
the ordinary argument, aggregate-destination, ownership, cleanup, loop, and
abrupt-control-flow lowering. A callee that must cross argument-created CFG is
secured in an ordinary scalar spill. Verification checks dense bottom-up type
metadata, exact eligible internal targets, definition availability, signatures,
receiver absence, complete call carriers, use-before-definition, and definite
non-null initialization of every loaded function slot. The x86-64 backend
currently rejects this verified MIR structurally before layout or selection.

HIR-to-MIR lowering owns deterministic allocation and emission order,
including base initialization, full-expression temporaries, view arguments,
and slices into locals, fields, arguments, return storage, and assignments.
Supported HIR may rely on producer invariants; arbitrary public HIR
construction is not a supported input contract.

Dynamic virtual calls lower to explicit MIR targets containing the canonical
family, stable slot, and statically selected declaration. Every method call
also carries its statically selected receiver place and either an exact
complete place/dynamic class or a forwarded metadata carrier. Scalar and
object results use the existing value or destination forms, so virtual calls
do not create a second call or cleanup pipeline.

Interface calls lower through the same call, argument, result, and cleanup
pipeline. Their MIR targets retain interface and requirement identities; their
receivers are explicit non-owning interface views with source, target, access,
and complete-object provenance. Conformance maps retain the effective
implementing method for each class and requirement. MIR deliberately contains
no backend witness layout, byte offset, or requirement slot.

Static type tests become boolean constants; runtime tests retain an explicit
source view and target identity. Runtime checked casts use dedicated indirect
carrier storage established only on the success edge and ended at the
full-expression boundary. The verifier checks legal static/runtime relations,
declared targets, view access and provenance, single definition, bounded
liveness, and the terminating failure edge.

Plain casts feed ordinary receiver, alias-argument, field access and mutation,
copy construction, copy assignment, value-argument, and result operations.
Runtime casts use explicit success and unrecoverable failure edges plus a
checked-view carrier ended at the consuming full-expression boundary. Static
casts from concrete places become verified view projections; forwarded static
sources use the same bounded carrier when a typed indirect home is required.
Scalar values that must survive a cast edge are explicitly spilled so MIR's
transient values remain block-local. The verifier checks target relation,
access, provenance, single definition, carrier liveness, failure termination,
and consumer compatibility. Shared-owner casts use explicit static
instructions or runtime success/failure terminators, with copy/adopt ownership
performed only on success and no allocation operation. Copy allocation
instead composes a target-directed checked source with explicit source `new`,
exact-class allocation, and the selected copy-constructor operation after the
check succeeds.

### Implemented standard I/O representation

The [standard I/O compiler contract](IO.md) assigns canonical
intrinsic validation to resolution and type checking, semantic access and
offset information to typed HIR, and executable read, write, open, close, and
standard-handle operations to MIR. MIR verification owns exact scalar and
array types, read-only versus mutable access, offset validity structure, and
the backing anchor that keeps each array range live across its host call.

These phase responsibilities and IR variants are implemented. Lowering
preserves left-to-right, exactly-once argument evaluation and exposes neither
a Skald array descriptor nor `Str` representation to the runtime. The x86-64
backend consumes the verified operations, checks each unsigned offset against
the array length, and passes one data pointer and remaining byte count to
runtime ABI version 9.

### Frozen panic and termination representation

The common reporting design is implemented. Typed HIR
represents an invocation of the validated canonical panic intrinsic as a
dedicated non-returning statement carrying one fully produced exact
`std::str::Str` value and source span. It does not retain an ordinary call
selected by the spelling `panic`.

MIR preserves two different semantic forms:

- explicit source panic, carrying its dynamic exact-`Str` message place; and
- compiler-known unrecoverable termination, carrying one distinct
  target-independent reason from the closed
  [language catalog](../language/ERRORS.md#frozen-panic-design).

Both forms have no successor. A static termination reason remains distinct
until instruction selection so verification, mutation tests, and dumps can
identify the failed rule without depending on message bytes or a target ABI.
Neither form is an exceptional edge and neither may join ordinary cleanup.
Target lowering owns deterministic used-message pooling after instruction
selection, descriptor extraction, and the sole public reporter call. Backend
ownership retains reference the same static pool directly for legal count
exhaustion; invalid handles and impossible count/header states retain separate
hard-trap edges.

Malformed public MIR and impossible states proven absent by verification do
not acquire a termination reason. They remain structured verifier errors
before target lowering, or hard compiler-defect traps if an invalid state is
somehow reached after the trust boundary.

### Runtime trace phase boundary

Runtime traces add no AST, resolved, HIR, or MIR operation. The source-aware
backend input, requested-only target metadata, and target-private activation
frames do not change any phase product or dump.
MIR continues to own the semantic spans already carried by definitions,
instructions, calls, blocks, `Panic`, and every `Terminate` reason; it must not
gain target trace-record identities, rendered paths, TLS operations, or stack
frame offsets.

Backend emission receives one explicit input containing the final verified
`MirProgram`, read-only `SourceDatabase` access when tracing is enabled, and
the selected trace policy. It also consumes the program's existing
semantic declaration and module provenance needed to form source-callable
names and provider-relative display paths. `SourceDatabase` resolves used span
starts to checked one-based `u64` line and Unicode-scalar column values only
when target lowering requests a location. Omitted emission has no source
access and performs no trace-only lookup. Request compilation and the
in-memory singleton adapter enable tracing by default.

The target boundary decides which source definitions contribute frames and
which existing spans require location records. Source functions, methods,
ordinary initializers, explicit static-field initializer bodies, and
source-authored copy, assignment, and destruction bodies are eligible.
Generated wrappers, coordinators, lifecycle/array/ownership/finalization
helpers, and target thunks are not. Before an omitted helper or runtime
operation can report, its source caller's current location is the initiating
MIR operation span.

For a function-value call, replacement uses the indirect call expression's
span before the stabilized target is loaded. The selected target then enters
its ordinary source activation frame, so a panic reports the exact target's
semantic top-level, static-method, or closed-generic name followed by the
indirect caller at its call site.

Location replacement is required before every source or external call, before
every generated helper call representing a source operation, before a runtime
operation permitted to report, and on each taken explicit-panic or static
termination edge. The x86-64 target implements that rule for source calls,
central reporter edges, generated array/ownership/lifecycle helpers,
allocation, and inline ownership overflow. Omitted helpers inherit the
caller's established operation without gaining a synthetic MIR span or visible
context. Failure-only placement means a successful checked operation does not
execute a trace update. A later target-private dataflow optimization may remove
a replacement already established on every incoming path, but correctness
never depends on that optimization.

The public verifier continues to validate target-independent spans and control
flow only. Trace-frame layout, metadata interning, update placement, and TLS
sequences are target legality and lowering responsibilities described by the
[backend contract](BACKEND.md#runtime-trace-target-boundary).

## While-loop representation

The source behavior of `while`, `break`, and `continue` is specified in
[Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits).
Source `while` is represented end to end. Callable-local `LoopId`, composable
HIR control effects, and structured `HirWhile` are current representations.
MIR storage lifetime epochs and cycle-safe verification are implemented:
every stateful verifier uses deterministic finite forward dataflow, checks
disconnected cyclic components, and resets per-epoch ownership and
initialization facts at storage lifetime boundaries. HIR-to-MIR lowering emits
the generic graph described below. Source `break` and `continue` are
represented end to end.

The contract fixes which phase owns each decision and the invariants visible
across phase boundaries. It does not fix private Rust organization, concrete
container types, exact instruction names, or basic-block numbering.

### Resolution and structured HIR

Resolution assigns every source loop one deterministic callable-local loop
identity in source order. A resolved `while` retains its condition, body,
identity, and source spans. Resolved `break` and `continue` statements carry
the identity of the nearest lexically enclosing loop. Lower phases do not
recover an exit target from source names or a nesting-depth count. Future
labels, if separately frozen, may resolve to the same identity model without
changing lower phases.

Typed HIR retains a structured loop operation containing:

- the resolved loop identity;
- the exact-`bool` condition;
- the typed body;
- the source span; and
- one composable control-effect summary.

The control-effect summary distinguishes these conceptual outcomes:

```text
FallThrough
Return
Diverge
Break(loop)
Continue(loop)
```

It represents a set of possible outcomes rather than forcing a block or
conditional to have only one. Statement-sequence composition sends only
fallthrough paths into the next statement and preserves the other effects.
Conditional arms combine their outcome sets. A loop consumes break and
continue effects targeting itself: its breaks contribute to loop fallthrough,
while its continues contribute to another condition test. Function exits,
divergence, and exits targeting an outer loop propagate. Every `while` also
retains the source contract's conservative condition-false fallthrough.

Different effects remain distinguishable until the structured operation that
owns them consumes them. Current HIR names the outcome set
`HirControlEffects`; its concrete storage remains private. The type checker
uses the same composition for existing blocks, conditionals, returns, and
panic, preserving the established callable-completeness diagnostics.

MIR cleanup planning exposes an opaque retained lexical-scope depth. Planning
an edge to that depth is non-consuming and returns cleanup and storage-dead
work for precisely the exited scopes. The lowering loop context binds a
`LoopId` to exit and latch block targets plus separate retained depths for
`break` and `continue`. `while` supplies one shared depth; general iteration
lets `continue` retain its outer receiver/state scope while `break` exits it.

### Repeatable MIR storage lifetimes

MIR keeps one stable storage identity and declaration for each source local or
compiler temporary. A storage whose dynamic lifetime can repeat has explicit
target-independent lifetime epochs with operations equivalent to:

```text
storage-live storage
...
cleanup, release, move, or transfer as required
storage-dead storage
```

Current MIR spells these operations `StorageLive` and `StorageDead`; their
required meaning is:

- an inert compiler-temporary declaration left behind after structural
  optimization has no dynamic epoch and no storage uses;
- initialization, use, projection, and cleanup require live storage;
- beginning another epoch while storage is live is invalid;
- ending an epoch while storage is dead is invalid;
- initialized owned contents must be destroyed, released, moved, or
  transferred before the epoch ends;
- ending an epoch clears all per-lifetime initialization, field, ownership,
  move, release, optional, checked-view, anchor, and temporary state associated
  with that storage;
- beginning a later epoch starts from the storage declaration's uninitialized
  state;
- loop-body locals and reusable condition or body temporaries are dead at the
  loop header and after the loop exit;
- storage declared outside the loop may remain live across its backedge; and
- parameters, receiver storage, and hidden result storage may use documented
  entry and exit conventions instead of source-emitted epoch operations.

Primitive storage participates even though it has no destructor. Cleanup
instructions alone cannot define lifetime epochs because primitive, moved, and
transferred storage may have no ordinary cleanup. Inline objects and their
owned fields, optionals, arrays, shared owners, checked views, anchors, and
full-expression temporaries all use the same epoch boundary rather than
receiving loop-specific reset exceptions.

Lifetime operations carry no target layout, stack-slot, or register decision.
Frame planning may map repeated epochs to one physical home, and a later phase
may erase operations after every analysis that consumes them.

The implemented callable-entry convention treats receiver, parameter,
alias-parameter, and hidden result storage as implicitly live for the complete
callable body. Those storage categories do not receive explicit lifetime
operations. Source locals begin an explicit lexical epoch immediately before
their initializer and end it after any required cleanup on each ordinary scope
exit. Compiler-owned arguments, spills, temporaries, checked views, shared
allocations and anchors, optional unwraps, and array storage use explicit
full-expression epochs unless lowering deliberately extends a result carrier
through scope-exit cleanup. Every ordinary end emits ownership cleanup before
the corresponding `StorageDead`.

### Generic CFG lowering and cleanup edges

HIR-to-MIR lowering represents source loops with ordinary basic blocks,
boolean branches, and jumps. It does not introduce a source-specific loop
terminator and does not reuse the generated array-loop terminator, whose array
storage and lifecycle invariants are unrelated to source control flow.

Current lowering applies this representation to source `while`, `for-in`,
`break`, and `continue`.
It allocates the loop regions before emitting their edges, evaluates and
finishes the condition full expression in the header, lowers the body as a
child lexical scope, routes normal completion through a latch, and selects the
exit as the continuation regardless of literal condition. When every body
path transfers elsewhere, lowering omits the unreachable latch rather than
inventing an edge with incompatible enclosing-storage state. The
representation is verified through the MIR pass boundary and native backend
for zero, repeated, nested, returning, breaking, continuing, and
ownership-heavy cases.

The implemented [general-iteration compiler contract](ITERATION.md) extends this
boundary with a dedicated source, resolved, and typed-HIR `for-in` form. HIR
retains exact canonical interface and requirement identities, `Item`, `State`,
the iterable expression, a loop-duration read-only receiver plan, state and
optional-result lifecycle, the item binding, body, loop identity, spans, and
control effects. HIR-to-MIR lowering expands that structure into ordinary
interface calls, mutable state aliases, optional presence and payload
operations, storage epochs, cleanup, branches, jumps, and the existing
loop-context destinations. No dedicated iteration operation reaches MIR,
verification, or a backend. The dedicated source, resolved, and typed-HIR
forms and the complete receiver and stored-value MIR/native matrices are
implemented.

Its canonical protocol foundation is already present before executable body
resolution: typed module-edge evidence identifies the canonical dependency,
and `ResolvedProgram` may retain validated `Iterable` template, `Item` and
`State` parameter, and `iter_state` and `iter_next` requirement identities.
Explicit canonical imports, direct canonical-module compilation, and `for-in`
syntax supply that evidence. Resolved loops additionally retain exact closed
interface and requirement identities, `Item`, `State`, loop/local IDs, and the
body. These resolution-only identities do not add an operation to current HIR
or MIR.

The basic `while` lowering form has these semantic regions:

```text
preheader -> condition-entry
condition true -> body-entry
condition false -> exit
body fallthrough or continue -> latch -> condition-entry
break -> exit
return -> function exit
```

The condition may expand into additional blocks for checked operations. Its
final successful path preserves the boolean result and completes
full-expression cleanup before branching. A reachable dedicated latch gives
normal body completion and every cleaned continue edge one continuation
destination. A unique exit joins condition-false and cleaned break edges.

Lowering tracks break and continue destinations and a retained
lexical cleanup depth for every active loop. Before transferring control,
normal body completion, `break`, and `continue` emit the source-defined
cleanup for every exited scope. Planning is depth-oriented
and does not consume lexical cleanup state, so multiple outgoing edges can
receive the same required sequence. `return` retains all-scope cleanup, and
panic retains its non-unwinding terminator.

After targeted exits become ordinary CFG edges, MIR need not retain the source
loop identity for correctness. Optional loop metadata may later support
diagnostics, debugging, or optimization hints, but analyses must remain able
to recognize loops in generic CFG.

The named regions and their edges are an initial lowering and deterministic
dump invariant, not a promise of exact block IDs. Checked expression lowering
may add blocks, and valid transformations may split, merge, redirect, or
remove blocks while preserving the verified semantics.

### Cyclic verification and transformation invariants

Every MIR dataflow domain must reach a finite fixpoint over cyclic CFG. Its
state describes the current possible lifetime and ownership state, not whether
an operation happened during some historical iteration.

Verification requires:

- compatible live storage, initialization, ownership, field, optional,
  checked-view, anchor, and full-expression state at joins and backedges;
- completed body-local and temporary epochs before the latch and loop exit;
- live outer storage to retain compatible state across the backedge;
- valid live/dead transitions and use only within a live epoch;
- exactly-once cleanup, release, move, or transfer before an owned epoch ends;
- structural checking of every block even when unreachable; and
- deterministic structured-error ordering independent of worklist visitation.

The MIR pass pipeline consumes raw final MIR and returns sealed verified MIR.
Transformations never repair a producer invariant or establish correctness
required for unoptimized execution. Source acceptance, type diagnostics, and
definite-return diagnostics are determined before MIR optimization.

Transformations preserve condition evaluation frequency and source ordering.
Destruction, retain/release, allocation, panic, checked failure, lifetime
boundaries, and full-expression cleanup remain effects unless a narrower
analysis proves a particular transformation safe. Dominator, natural-loop,
liveness, invariant-motion, and induction analyses derive loop structure from
generic CFG rather than source-only nodes.

Current MIR may continue using mutable storage for loop-carried source values;
this extension does not require SSA or phi nodes. If a later optimization
boundary introduces SSA, it may derive header phi nodes without changing
resolved or HIR loop meaning.

### Determinism and private implementation freedom

Resolved and HIR loop identities are allocated deterministically in callable
source order. Resolved and HIR dumps retain structured loops and targeted
effects. MIR dumps expose lifetime epochs, cleanup order, and generic control
edges in deterministic initial-lowering order. A future optimized dump belongs
to its named pass stage and need not preserve unoptimized block numbering.

This contract deliberately does not freeze:

- Rust module or file layout;
- public or private Rust type and helper names;
- the concrete control-effect collection;
- the numeric representation of loop or block identities;
- exact lifetime-operation spelling;
- worklist, dominator, or loop-analysis algorithms;
- optional source-loop metadata after MIR lowering;
- stack-slot, register, or frame assignment; or
- a future optimization IR boundary.

## Verification and passes

`mir::verify_mir` checks the structural and type invariants required before
target lowering, including:

- module-table density and path uniqueness, selected-entry ownership, known
  top-level module owners, semantic identity ownership, declaration-table
  density, and declaration/definition agreement;
- callable signatures, receiver and argument modes, and external exclusions;
- method declaration/definition kind agreement, static receiver absence,
  static-call target kind, and static exclusion from virtual/interface maps;
- storage, value, place, projection, and operation types;
- hierarchy acyclicity, direct-base paths, view targets/access, and selected
  base lifecycle operations;
- virtual-family density, membership, signature/access agreement, call
  selection, receiver compatibility, and complete-object provenance;
- interface density, conformance and requirement/method agreement, view
  provenance, access, non-ownership, signatures, and receiver liveness;
- definition-before-use and valid block targets;
- construction, copy, result-destination, temporary, and cleanup liveness;
- exact shared-allocation publication order, compatible owner storage,
  copy/adopt/release liveness, normal-exit release, and identical ownership
  state at control-flow joins;
- branch, return, and terminator consistency on every block; and
- access and ownership requirements for reads, writes, calls, and cleanup.

Verification returns ordered structured errors. It currently runs at two
deliberate boundaries:

1. after HIR lowering in debug builds, identifying producer defects close to
   their source;
2. unconditionally at the input of `passes::run_mir_pipeline`, with immediate
   reverification after each changed occurrence, constructing the only sealed
   final MIR accepted by backend input.

The backend does not repeat target-independent verification. Under the
[selectable pipeline](#selectable-final-mir-optimization-pipeline),
a non-empty pipeline first verifies its input, retains the seal after an
unchanged occurrence, and privately invalidates and immediately rebuilds the
seal after every changed occurrence. Per-changed-pass verification localizes
transformation defects before another pass or backend can inspect the result.

Target-specific legality and structured backend failures are defined by the
[backend and target contract](BACKEND.md#input-and-legality-boundary).

The supported MIR profiles share the registry, request selection,
verified runner, per-occurrence reporting, inspection checkpoints, and
value-use analysis. `none` verifies without transforming; `default` runs the
dead-pure canary exactly once to a conservative fixed point and then runs
whole-world reachability exactly once. Every
transformation has explicit ordering and returns changed MIR through the same
verifier boundary.
Compiler correctness must not depend on an optimization pass being enabled.

The shared-ownership implementation preserves this division of
responsibility: HIR records owner provenance and anchor requirements, MIR
makes copy/adopt/release and anchor lifetimes explicit, and verification proves
their structural ownership invariants before a backend realizes them. Exact
future requirements are owned by
[Shared-Ownership Compiler and Runtime Contract](SHARED_OWNERSHIP.md#target-independent-phase-contract).

The path-sensitive shared verifier retains one private ownership state behind
its existing facade. A propagation owner schedules CFG alternatives, selects
conditional successors, converges path conditions, and reports incompatible
joins. A transition owner applies allocation, publication, adoption, transfer,
field, cast, call, return, and full-expression rules. A use-validation owner
checks shared pointees, dynamic provenance, static backing, and checked-view
dependencies. The state owner supplies entry state, storage-epoch reset, and
compatible-live-state comparison. This internal split changes neither MIR nor
diagnostic text, ordering, or suppression.

## Deterministic inspection

Every major phase product has one phase-owned textual renderer:

| Product | Renderer |
|---|---|
| Tokens | `lexer::dump_tokens` |
| AST | `syntax::dump_ast` |
| Resolved program | `resolve::dump_resolved` |
| Typed HIR | `hir::dump_hir` |
| MIR | `mir::dump_mir` |

The renderers share only low-level formatting primitives. Each phase owns its
dump vocabulary and ordering so one IR can evolve without a cross-phase
serialization abstraction. Dumps are debugging and regression formats, not a
stable interchange or persistence schema.

Stable identities, deterministic table/block order, and exact renderers are
tested both within phases and across independent compiler processes. The
three semantic IR renderers include selected-module metadata, modules in dense
identity order, and module ownership on top-level declarations. The
public dump paths let integration tests and temporary tools inspect the same
representation used by focused tests. Practical inspection steps are in
[Debugging the Compiler](../development/DEBUGGING.md).

The final-MIR pipeline exposes `run_mir_pipeline_inspected` with a
request-local `MirPipelineInspector`. Its callback receives a typed label and
only a borrowed verified final-MIR product. Checkpoint labels and `dump_mir`
bytes are deterministic across independent processes. The inspection surface
is neither a dump serializer nor a filesystem publication service.

Static-field dumps retain declaration identity and type in resolved IR and
HIR, and show the same identity on every MIR static root. Cross-process tests
compare those products and assembly both for a complete source pipeline and
for cyclic module graphs written and discovered in different orders.

## Trust and testing boundaries

The compiler trusts products created by successful earlier phases. Public
fields make products inspectable, but do not promise validation for arbitrary
construction or mutation. MIR alone has a public verifier because it is the
portable executable contract consumed by every backend.

Private `cfg(test)` helpers compose source through a named phase boundary and
assert only that preceding phases succeeded. Malformed-MIR fixtures and
mutation hooks remain crate-visible so verifier tests can violate invariants
without widening the production API. Cross-phase tests that use only public
facades live under `crates/skald-compiler/tests/`; complete source behavior
belongs in golden tests.

The crate-level [public API integration test](../../crates/skald-compiler/tests/public_api.rs)
compiles the intentional phase entries, products, dumps, verifier, pass
pipeline, target boundary, and driver paths together. This protects the
facades used by repository consumers without freezing private modules or every
field of an evolving IR schema.

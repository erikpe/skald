# Compiler Phases and Intermediate Representations

Status: authoritative for current compiler phase inputs, products, invariants,
verification boundaries, deterministic dumps, and phase-facing public paths.
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

## Pipeline contract

The target-independent compiler path is:

| Responsibility | Public entry | Product |
|---|---|---|
| Source ownership | `source::SourceDatabase` | source IDs, files, text, spans, line locations |
| Lexing | `lexer::lex` | `LexOutput`: tokens and diagnostics |
| Parsing | `syntax::parse` | `ParseOutput`: source-shaped AST and diagnostics |
| Resolution | `resolve::resolve`, `resolve::resolve_module_graph` | `ResolveOutput`: resolved program and diagnostics |
| Type checking | `typeck::type_check` | `TypeCheckOutput`: diagnostics and optional typed HIR |
| MIR lowering | `mir::lower_hir` | target-independent `MirProgram` |
| MIR passes | `passes::run_mir_pipeline` | verified `MirProgram` or verification errors |

`driver::compile_request_to_assembly` composes provider normalization,
reachable graph loading, these phases, target selection, and backend emission.
`driver::compile_source_to_assembly` is its in-memory singleton convenience
surface. Both stop after any source phase that produced an error. Successful
type checking always produces HIR; failed type checking
produces no HIR. HIR lowering represents every typed operation directly in
target-independent MIR. The
[backend and target contract](BACKEND.md)
defines how verified MIR is checked and realized for a selected target; driver
behavior is separate from the target-independent phase model and is defined by
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md).

Phase products are request-owned values. The compiler has no global source,
diagnostic, identity, or IR registry.

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
logical-path order, reject cycles, and return an inspectable `ModuleGraph`.
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

The optional-values contract assigns each decision to these same phase owners.
Syntax preserves source shape and resolution assigns non-recursive optional
target identities. For optional owning values, type checking selects explicit
absent or present initialization, copy, assignment, overload injection,
field/call boundaries, presence, primitive extraction, checked class payload
views, and optional shared copy/adopt/move/release and secured unwrap. MIR owns
initialized places, caller-owned argument/result
aggregates, explicit unwrap success/failure control flow, begin/end guard
operations, and guarded-mutation checks. Verification proves compatible
operations, definite wrapper initialization, balanced compatible guards,
anchor ordering, isolation of the zero niche from ordinary owners, and
identical initialized optional state across CFG joins. Inline optional
container aliases use ordinary indirect MIR places plus exact optional types;
reserved boxed, nested, and optional-reference shapes remain diagnosed before
HIR.

## Frozen primitive integer operation boundary

Primitive integer comparisons and casts have a
[frozen source contract](../language/TYPES_AND_VALUES.md#frozen-primitive-integer-comparisons-and-casts)
but are not current compiler phase products. Their implementation uses the
existing pipeline with these fixed responsibilities:

- Lexing recognizes comparison punctuation by longest match and otherwise
  preserves source spellings. Syntax retains each predicate, both operand
  shapes and spans, or one primitive cast target and operand. It assigns no
  numeric meaning or target behavior.
- Resolution preserves comparison and primitive-cast shape without performing
  declaration lookup for primitive keywords. Nominal and shared object-cast
  targets continue through their existing identity lookup; lower phases never
  disambiguate cast kinds from source text.
- Type checking is the sole owner of operation selection. It requires matching
  `i64`, `u64`, or `u8` comparison operands and records a `bool` result, or
  selects one of the nine valid integer source/target cast pairs. Unsupported
  and implicit conversions are rejected before HIR.
- Typed HIR records the selected comparison predicate and integer operand type,
  or both the integer cast source and target types. It does not retain a
  backend condition code, register width, or spelling-based signedness choice.
- MIR lowering evaluates comparison operands left to right and every operand
  exactly once. Comparisons become typed boolean-producing rvalues; integer
  casts become ordinary pure rvalues with no trap, call, allocation, cleanup,
  or exceptional control-flow edge.
- MIR verification proves matching comparison operand definitions and types,
  a `bool` result, and the closed integer cast matrix with exact source and
  result types. Both operation families retain the existing block-local value,
  definition-before-use, and deterministic-error invariants.
- Each backend first receives the already selected signedness and width through
  verified MIR. It realizes signed `i64` ordering, unsigned `u64`/`u8`
  ordering, canonical boolean results, bit preservation, truncation, identity,
  or zero extension without inferring semantics from source spelling or target
  register accidents.

These operations add no ownership or lifetime rule and no public runtime ABI.
Floating, boolean/numeric, checked, saturating, implicit, mixed-type, and
user-defined conversion remain outside this boundary.

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

`LexOutput` contains tokens and diagnostics together. Invalid characters and
malformed numeric spellings are diagnosed while retaining a recoverable token
stream. Token kinds and accepted lexical forms follow the
[grammar authority](../language/GRAMMAR.md).

## Syntax

The recovering parser produces an unresolved, source-oriented AST. Nodes keep
spans and spellings needed by later diagnostics, including exact private field
and method modifier spans, but contain no selected declaration identities,
inferred types, access decisions, or target details.

Grammar nesting uses a shared finite budget. Exceeding it is a source
diagnostic with recovery rather than unbounded recursion. The precise accepted
source shape and nesting limit are owned by the
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

Optional types use two flat, copyable resolved families rather than recursively
wrapping the general type enum: an inline primitive/exact-class payload target,
or an optional shared class/interface/`Obj` target. Resolved expressions retain
explicit absence, presence-test, and unwrap nodes. Canonical dumps use `T?` and
`shared? T` independently of source trivia.

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

Resolved IR remains source-oriented: it records selected declarations and
object paths, but does not decide final expression types, access validity,
copy capability, storage, evaluation lowering, or ABI placement.

Shared type syntax resolves to an explicit class, interface, or `Obj` target.
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
the unique most-specific static parameter-type sequence, and records exactly
one `InitializerId`. HIR and MIR therefore contain no unresolved overload
choice. Both IRs store dense, source-ordered initializer declaration and
definition vectors, and MIR lowers, verifies, dumps, and emits every entry.

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

Shared types cross this boundary as canonical class, interface, or `Obj`
targets, distinct from inline class values and non-owning views. Shared value
consumers retain a named owner place or produced owner and explicitly select
copy or adopt. Ordinary `new C(arguments)` retains exact `C`, its selected
`InitializerId`, and typed source-ordered arguments. Shared locals, value
parameters, results, and fields use this vocabulary, including compatible
implicit up-views. Inline values and aliases do not implicitly manufacture an
owner, and external shared signatures remain invalid. Explicit copy allocation
records its checked source and selected exact-class copy operation separately
from ordinary initializer overloads. Shared casts record their source
provenance, static or runtime relation, target, and copy/adopt result ownership.

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
- canonical direct-base metadata and identity-based base projections;
- transient primitive values;
- source-ordered calls, argument modes, and access-restricted
  class/interface/`Obj` views;
- canonical virtual-family metadata, explicit receiver-bearing direct/virtual
  method targets, receiverless static method targets, and complete-object
  receiver origins;
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

Verification returns ordered structured errors. It runs at three deliberate
boundaries:

1. after HIR lowering in debug builds, identifying producer defects close to
   their source;
2. unconditionally in `passes::run_mir_pipeline`, protecting the input and
   output of target-independent transformations; and
3. again inside backend emission, preventing malformed library-created or
   mutated MIR from reaching target legality and instruction selection.

Target-specific legality and structured backend failures are defined by the
[backend and target contract](BACKEND.md#input-and-legality-boundary).

The MIR pass pipeline currently verifies without transforming. Future
analyses and transformations must have explicit ordering and must return MIR
that satisfies the same verifier boundary. Compiler correctness must not
depend on an optimization pass being enabled.

The shared-ownership implementation preserves this division of
responsibility: HIR records owner provenance and anchor requirements, MIR
makes copy/adopt/release and anchor lifetimes explicit, and verification proves
their structural ownership invariants before a backend realizes them. Exact
future requirements are owned by
[Shared-Ownership Compiler and Runtime Contract](SHARED_OWNERSHIP.md#target-independent-phase-contract).

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

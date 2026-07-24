# Compiler Phases and Intermediate Representations

Status: authoritative for current compiler phase inputs, products, invariants,
verification boundaries, deterministic dumps, and phase-facing public paths.
Source-visible meaning remains owned by the
[language documentation](../language/README.md). The frozen additions required
for shared ownership are specified separately in the
[shared-ownership compiler and runtime contract](SHARED_OWNERSHIP.md). That
contract distinguishes the implemented typed-HIR vocabulary from the
remaining MIR, backend, and runtime work.

## Pipeline contract

The target-independent compiler path is:

| Responsibility | Public entry | Product |
|---|---|---|
| Source ownership | `source::SourceDatabase` | source IDs, files, text, spans, line locations |
| Lexing | `lexer::lex` | `LexOutput`: tokens and diagnostics |
| Parsing | `syntax::parse` | `ParseOutput`: source-shaped AST and diagnostics |
| Resolution | `resolve::resolve` | `ResolveOutput`: resolved program and diagnostics |
| Type checking | `typeck::type_check` | `TypeCheckOutput`: diagnostics and optional typed HIR |
| MIR lowering | `mir::lower_hir` | target-independent `MirProgram` or structured `HirLoweringError` |
| MIR passes | `passes::run_mir_pipeline` | verified `MirProgram` or verification errors |

`driver::compile_source_to_assembly` composes these phases with target
selection and backend emission. It stops after any source phase that produced
an error. Successful type checking always produces HIR; failed type checking
produces no HIR. HIR lowering currently represents every typed operation in
target-independent MIR. Its result type retains a structured lowering-error
boundary so future staged work need not turn a temporary phase gap into a
panic. The
[backend and target contract](BACKEND.md)
defines how verified MIR is checked and realized for a selected target; driver
behavior is separate from the target-independent phase model and is defined by
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md).

Phase products are request-owned values. The compiler has no global source,
diagnostic, identity, or IR registry.

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
spans and spellings needed by later diagnostics, but contain no selected
declaration identities, inferred types, access decisions, or target details.

Grammar nesting uses a shared finite budget. Exceeding it is a source
diagnostic with recovery rather than unbounded recursion. The precise accepted
source shape and nesting limit are owned by the
[implemented grammar](../language/GRAMMAR.md).

## Resolution and identities

Resolution is the only compiler phase that selects declarations from source
names. It first collects program and member declarations, then resolves
callable bodies, which permits forward references without making lower phases
name-dependent.

The resolved program replaces successful name uses with typed identities for
functions, classes, interfaces, interface requirements, members, callables,
parameters, locals, and bindings.
Optional direct class bases likewise carry `ClassId` rather than source
spelling. Callable-owned identities also scope later local MIR identities.
Declaration tables retain deterministic identity order, and later phases
select entries by identity rather than by source spelling.

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

Resolved IR remains source-oriented: it records selected declarations and
object paths, but does not decide final expression types, access validity,
copy capability, storage, evaluation lowering, or ABI placement.

Shared type syntax resolves to an explicit class, interface, or `Obj` target.
Allocation syntax resolves to an exact concrete `ClassId` and retains ordinary
arguments or the explicit copy source as the existing distinct construction
modes. These facts cross resolution without a feature gate. Type checking owns
their semantic compatibility and the current lower-phase gate.

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
copy operation. The later shared extension will consume a target-directed
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
owner, external shared signatures remain invalid, and explicit copy allocation
and shared-owner casts remain structured typed exclusions.

MIR lowering accepts exact-class shared local initialization and assignment
from named owners and ordinary allocations. It also carries same-target shared
owners through internal function, initializer, method, and interface
parameters and results. Named sources copy, produced sources adopt, calls
consume caller argument owners, callees normally release parameter owners, and
shared returns escape through one dedicated result owner after cleanup.
Assignment secures an owning temporary before releasing its destination and
moves that temporary into the destination. A structured
`HirLoweringError::UnsupportedSharedOwnership` gate remains for shared fields,
polymorphic transfers and views, casts, and anchors.

## MIR

MIR is executable in shape and target-independent. It makes these concerns
explicit:

- callable declarations and executable definitions;
- addressable storage and semantically projected places;
- canonical direct-base metadata and identity-based base projections;
- transient primitive values;
- source-ordered calls, argument modes, and access-restricted
  class/interface/`Obj` views;
- canonical virtual-family metadata, explicit direct/virtual method targets,
  and complete-object receiver origins;
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
  full-expression boundaries; and
- basic blocks with explicit return, jump, boolean-branch, checked-cast, and
  unrecoverable-failure terminators.

MIR is not SSA. State that crosses control-flow edges uses storage. Class
objects remain addressable places rather than transient scalar values. Field
and base projections carry semantic identities rather than target offsets.
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
and consumer compatibility. Shared-owner casts will add explicit copy/adopt
ownership operations but no allocation operation. Future copy allocation
instead composes a target-directed checked source with explicit source `new`,
exact-class allocation, and the selected copy-constructor operation after the
check succeeds.

## Verification and passes

`mir::verify_mir` checks the structural and type invariants required before
target lowering, including:

- identity ownership, table density, and declaration/definition agreement;
- callable signatures, receiver and argument modes, and external exclusions;
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

The frozen shared-ownership extension preserves this division of
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

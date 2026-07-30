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

Primitive integer comparisons and casts plus eager boolean negation and
equality have a
[source contract](../language/TYPES_AND_VALUES.md#implemented-primitive-comparisons-boolean-negation-and-integer-casts)
and are current products through native x86-64 execution.
The pipeline responsibilities are:

- Lexing recognizes comparison punctuation by longest match and keeps prefix
  `!` distinct from postfix unwrap by position. Syntax retains each predicate,
  unary or binary operand shape and span, or one primitive cast target and
  operand. It assigns no numeric meaning or target behavior.
- Resolution preserves comparison and logical-negation shape. Primitive casts
  preserve their primitive target without declaration lookup, while nominal
  and shared object-cast targets continue through existing identity lookup;
  lower phases never disambiguate cast kinds from source text.
- Type checking is the sole owner of operation selection. It requires matching
  `i64`, `u64`, or `u8` comparison operands, admits `bool` only for equality
  and inequality, selects logical negation only for `bool`, or selects one of
  the nine valid integer source/target cast pairs. Unsupported operations and
  implicit conversions are rejected before HIR.
- Typed HIR records the selected primitive comparison predicate and operand
  kind or the exact boolean logical-negation operation. Primitive casts record
  both integer source and target types.
  Neither representation retains a backend condition code, register width, or
  spelling-based signedness choice.
- MIR lowering evaluates comparison operands left to right and every unary or
  binary operand exactly once. Comparisons and negation become typed
  boolean-producing rvalues; integer
  casts become ordinary pure rvalues with no trap, call, allocation, cleanup,
  or exceptional control-flow edge.
- MIR verification proves matching comparison operand definitions and types,
  rejects boolean ordering, and requires exact boolean negation operands and
  results. Cast verification proves the closed integer matrix with
  exact source and result types. Both operation families retain the existing
  block-local value, definition-before-use, and deterministic-error
  invariants.
- Each backend receives already selected signedness and width through verified
  MIR. The x86-64 target realizes signed `i64` ordering, unsigned `u64`/`u8`
  ordering, and boolean negation/equality with canonical results. It realizes integer
  casts through canonical scalar loads and stores: same-width bits are
  preserved, narrowing retains the low byte, and `u8` widening zero-extends.
  Selection does not infer semantics from source spelling or expose target
  registers to MIR.

These operations add no ownership or lifetime rule and no public runtime ABI.
Floating comparisons, boolean/numeric operations, short-circuit logic,
checked, saturating, implicit, mixed-type, and user-defined conversion remain
outside this boundary.

## Frozen primitive operator representation

The
[frozen primitive operator profile](../language/TYPES_AND_VALUES.md#frozen-primitive-operator-profile)
selects a complete target-independent representation boundary without claiming
that the corresponding phase products are implemented.

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

Future explicit casts remain separate HIR operations that complete before
operator selection. Operator HIR cannot observe cast provenance.

Typed HIR represents eager unary and binary operations as exact typed values.
Each operation retains its operand type, result type, semantic flavor, source
span, and failure capability without encoding a backend opcode. Boolean `&&`
and `||` remain structured short-circuit operations in HIR so a skipped right
operand is absent from abstract execution rather than marked as an eager value
whose effects may later be discarded.

MIR lowers eager primitive operations to target-independent scalar operations.
It preserves:

- wrapping `i64`, `u64`, and canonical `u8` arithmetic;
- floor signed division and divisor-sign remainder;
- the non-failing signed-minimum division/remainder pair;
- exact bitwise width and signed or unsigned right-shift flavor;
- checked shift-count and integer zero-divisor failure reasons;
- IEEE binary64 operation and unordered comparison flavor; and
- canonical `bool` and `u8` results.

Short-circuit HIR lowers to ordinary MIR branches and jumps with one selected
canonical `bool` result. Because MIR transient values are block-local, a result
used after the branch crosses through explicit target-independent storage or
an equivalently verified future representation. MIR has no eager logical
scalar operation whose lowering may evaluate both operands.

The selected path owns only temporaries, checked views, guards, and anchors
that it actually establishes. Every completed full-expression temporary
remains live to the enclosing boundary. A join before that boundary must either
represent path-dependent lifetime and conditional cleanup explicitly or keep
affected continuations distinct until their lifetime states are compatible.
Consumer-bounded optional payload views retain their immediate-consumer
lifetime; they are not promoted to full-expression temporaries.

MIR verification rejects:

- operand or result types outside the frozen matrix;
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

The existing static-termination representation gains three distinct reasons:
integer division by zero, integer remainder by zero, and shift count out of
range. They remain distinct through verification and instruction selection and
select exact messages from the
[language panic catalog](../language/ERRORS.md#frozen-panic-design).

Constant folding and every later transformation use the same wrapping,
division, remainder, shift, NaN, panic, evaluation, ownership, and
short-circuit rules as unoptimized execution. Algebraic identities are invalid
when they remove required effects, change NaN results, suppress a panic, or
alter temporary completion and cleanup.

Generated target code mechanically realizes verified MIR. Exact Rust enum
names, module organization, basic-block numbering, temporary-storage
selection, branch shape, instruction sequence, and optimization algorithm
remain private. The frozen profile adds no public runtime ABI entry point
beyond the existing common panic reporter.

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

Resolved ordinary initializer declarations likewise retain per-overload
visibility and the exact modifier span. Type checking first performs ordinary
applicability and unique-most-specific selection, then applies the same exact
declaring-class owner comparison to the selected `InitializerId`. Direct,
shared-allocation, base, and class-element array-default construction all use
that centralized check. No-match and ambiguity stop before access checking.
Authorized HIR deliberately erases initializer visibility along with field
and method visibility.

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
`LoopId` to exit and latch block targets plus that retained depth. Current
`while` lowering installs this context around the body. `break` and
`continue` lowering query it to select identical exited-scope cleanup and,
respectively, the exit or latch target.

### Repeatable MIR storage lifetimes

MIR keeps one static storage identity and declaration for each source local or
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

Current lowering applies this representation to source `while`, `break`, and
`continue`.
It allocates the loop regions before emitting their edges, evaluates and
finishes the condition full expression in the header, lowers the body as a
child lexical scope, routes normal completion through a latch, and selects the
exit as the continuation regardless of literal condition. When every body
path transfers elsewhere, lowering omits the unreachable latch rather than
inventing an edge with incompatible enclosing-storage state. The
representation is verified through the MIR pass boundary and native backend
for zero, repeated, nested, returning, breaking, continuing, and
ownership-heavy cases.

The initial lowering form has these semantic regions:

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

The MIR pass pipeline consumes verified MIR and returns verified MIR.
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

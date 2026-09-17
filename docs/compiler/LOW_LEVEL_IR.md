# Low-Level Execution and Publication

Status: private checked declarations and the complete lowered draft vocabulary
are implemented. Production emission still uses the [current backend](BACKEND.md).
Independent graph/value-flow and full lowered callable verification are implemented.
Complete lowered-program inventory publication and parent-bound target
declarations are implemented. Selected graphs, edits, placement and native
consumption remain planned.

## Ownership and checking

`backend::plan` owns supplied declaration facts, structural checking and borrowed
immutable views. It has no MIR executable enums, frontend state, source database,
physical instruction types or frame offsets. The facts contain target/profile
and trace/artifact policies, dense layout/signature pools, typed callable/artifact
keys, executable source dispositions, active statics and stable dispatch slots.

Checking consumes the supplied facts and validates their structural consistency.
This is not production target planning or certification of a final-MIR projection.
The supplied executable/static domains and unused dispatch slots must eventually
come from the verified input's authority; the checker never recalculates semantic
reachability. Present source bodies may acquire an executable callable binding.
An absent source declaration remains inspectable but cannot acquire that binding.
Target thunks cannot enter the shared declaration catalog.

Layouts distinguish addressable objects, including size-zero objects, from
elided unit/metadata views. Sizes/alignment and extent arithmetic are checked.
Equal layout records do not automatically merge identities or helper semantics.
Signatures record execution scalar/address types, convention and logical roles.
Unit/nonreturning signatures have no result; scalar returns have one typed result;
aggregate returns have a matching address-valued destination and no scalar result.
Receiver triples and optional alias-origin pairs must be complete and address
typed. Component roles are independent of list position and physical ABI location.
Runtime services have checked logical shapes and conservative mandatory effects;
independent operation verification rechecks those contracts against stored operations.

The profile vocabulary includes x86-64 System V and AArch64 AAPCS64 fact shapes,
with 64-bit addresses and capability checks for binary64, indirect calls and
runtime tracing. This does not register an AArch64 backend or validate native
recipes. Concrete target capability projection and ABI/resource binding remain
target responsibilities.

Typed artifacts distinguish callables, data, runtime services, user externals and
trace TLS. Runtime/external declarations require matching convention signatures;
data requires an addressable layout. Omitted tracing rejects trace catalog/TLS
entries. Complete mode permits inactive static declarations; reachable mode
requires static declarations to belong to the supplied active domain.

## Identity and lookup

Dense declaration IDs name supplied pool entries. `PlanFacts::add_layout` and
`add_signature` allocate checked `usize` indices in explicit declaration order.
The planner must choose that order deterministically. Keyed callable/artifact
catalogs and dispatch slots iterate canonically regardless of request arrival.

After checking, `PlanView` supplies context-bound declaration handles and
`CallableBinding` supplies an executable owner. Lookup checks the same live
context and target/profile before accessing an entry. Two simultaneously live
plans with equal facts and numeric IDs remain different authorities. Identity is
compared privately through borrowed context references, without a global registry,
serialized token or public context constructor.

`backend::graph` supplies `OwnedArena`, reusing the compiler's dense ID table.
Lowered/selected block, value and object IDs are six distinct types. Allocation
and lookup check indices, context and callable ownership; iteration preserves
index order. IDs contain no stack home or physical location. Arena handles check
context/owner, not immutable snapshot freshness. Callable publication establishes
that separate authority; future consuming edits must reverify it.

All these interfaces remain backend-private. Context checking issues no callable
verification seal, completion receipt, complete-program authority or placement
result. Public backend input still requires verified final MIR.

## Lowered construction

`backend::lir` owns callable drafts, checked mutations and closed scalar/object
schemas. A callable stores an explicit entry, ordered signature inputs, blocks,
values and symbolic objects. Values record their scalar type, optional source
span and one definition site: entry input, block parameter or ordered instruction
result. Blocks and values can be reserved before their definitions; unresolved
reservations remain draft obligations. Entry may have any block index. Entry
inputs are allocated from the checked logical signature before instructions.

The builder accepts context-bound block/value/object handles and checks ownership
before recording compact IDs. It rejects duplicate definitions, mismatched result
and edge types, invalid entry shapes and writes after termination. Rejected
semantic writes do not install partial definitions. Forward edges retain ordered
arguments; branches to the same target remain separate edge occurrences. Loop
parameters and edge swaps do not overwrite an existing value definition.

Scalar schemas cover wrapping integer and binary64 arithmetic, floor integer
division, shifts, the shared neutral comparison predicates, all primitive cast
cells and explicit float/pointer bit conversions. Bytes and booleans use canonical
payload types; binary64 constants store exact bits, including signed zero and NaN
payloads. Address formation uses symbolic artifacts/objects, explicit byte
offsets and checked static strides, with no hidden runtime checks or frame offsets.
Loads/stores record scalar type, storage byte width and alignment independently.

Object records distinguish addressable size-zero storage from elided unit and
metadata views, which cannot form addresses or lifetime operations. Roles cover
semantic storage, aggregate results/temporaries and enabled-only trace records.
Static lifetime sites support ordered start/end markers, including repeated loop
executions; they do not prove source initialization, alias lifetimes or liveness.

Scalar checks have explicit success/failure edges and three finite relations:
nonzero divisor, count below integer width and finite truncated binary64 in an
integer range. Evidence names a check terminator or an exact constant value.
These records grant no proof by themselves. Callable verification checks operand
identity, constant validity and success-edge protection independently. The graph
checker handles cross-block use ordering and forward-edge reconciliation.
`finish` returns a draft even when
reservations or blocks remain incomplete; it creates no seal or emission authority.

Owner-private storage supports independently malformed test fixtures without
exposing an unchecked verified constructor. Arithmetic descriptors specify
meaning; native recipe equivalence is a separate target obligation.

## Structural verification and analysis

`backend::graph::check_graph` checks stage-independent structural descriptions.
The lowered adapter reads stored draft tables independently of builder history,
checks arena context/owner and referenced local IDs before converting them to
indices, and derives ordered uses/results from the closed operation vocabulary.
The shared checker reconciles every declared definition with its actual input,
parameter or result site and type. It rejects duplicate/unresolved definitions,
missing terminators, invalid entry shapes/predecessors and mismatched ordered
edge arguments. The separated instruction/terminator storage permits one final
terminator per block, with no instruction-after-terminator representation.

After structural checks succeed, one session builds CFG predecessors, entry
reachability and dominance. Instruction operands precede their results; edge
arguments are uses in the predecessor terminator, before successor parameters.
Parallel edges retain their distinct predecessor/slot occurrences. Critical edges,
loop parameters and simultaneous swaps/cycles remain legal.

Unreachable blocks still undergo structural and same-block ordering checks.
Entry-path dominance imposes no cross-block constraint on an unreachable use;
analysis queries involving unreachable blocks return unknown across blocks.
Making a block reachable requires a new check and ordinary dominance. Sessions
borrow the exact immutable draft, so mutation cannot coexist with continued use
of its analysis. No shared context clone, per-use CFG rebuild or global cache is
involved.

Failures carry stage, target/profile, machine callable, stable reason, local
position and a value origin span when available. Shared failures are ordered by
storage category and block/instruction/edge position, independent of traversal
order. An adapter rejects an unsafe reference before constructing the indexed
description. Graph success grants analysis only: scalar legality, guard protection,
call/artifact contracts, mandatory effects, memory extents and trace-path parity
must all pass the lowered verification checks before a phase seal or receipt can
be published. Native trace-path parity remains a target responsibility.
Synthetic selected payloads exercise the same algorithm over independent selected
storage. Descriptor consistency and target verification remain publication obligations.

## Calls, effects and tracing

Calls name a typed callable/runtime/external declaration or a secured
`CodeAddress(SignatureId)` value. Ordered arguments carry the exact logical roles
and types from the checked signature. Entry, call and return use that signature;
aggregate destinations and receiver/alias triples are inputs, with no fabricated
scalar result. Arguments and indirect targets remain value IDs across later calls.

The runtime catalog checks all nine service shapes against their fixed contracts.
Calls always retain a call barrier. Internal, external and indirect calls
conservatively read/write unknown memory and may allocate, free, report, hard-fail
and touch trace state; the closed runtime catalog supplies reviewed service effects.
Nonreporting/hard-defect attribution cannot narrow a reporting contract.

Every instruction and terminator records mandatory effects. Ordinary loads/stores
use object/static/unknown provenance derived from address formation and constant
offsets. Loaded pointers, alias inputs, dynamic offsets and unresolved merges
remain unknown; origin spans confer no alias authority. Optional summaries must
cover every mandatory effect; unknown reads/writes cover known regions, while a
known object cannot cover unknown memory. Pure operations have empty effects.
Verification recomputes provenance and mandatory effects independently before
publication, including forward address definitions. Supplied known provenance
must agree; unknown metadata can become known after checking. Known object and
static accesses must fit their declared extent and alignment; inactive statics
are rejected even in complete mode. Unknown addresses grant no bounds proof.

Call attribution distinguishes source operations, inherited boundaries, source
bodies entered from omitted helpers, nonreporting calls, hard defects and process
boundaries. Enabled trace plans bind local record ownership, frame eligibility,
context and permitted locations to the checked catalog. Ineligible helpers have
no local frame record. Explicit push/location-replacement/pop actions retain
ordering and associated instruction/terminal sites; attribution emits no action
implicitly. Omitted tracing rejects plans, actions and trace references, while a
compiler-defect span remains ordinary metadata. Verification checks record ownership, eligible entry push and return pop, and
immediate location-to-operation associations. Full path parity and physical
marshalling order remain native lowering/selection obligations.

`ReportFailure` contains its checked nonreturning panic call and a typed failure
message artifact with exact byte length. The shared failure catalog is also used
by current native emission, preventing a second message list. Arbitrary
nonreturning calls retain an explicit terminal call without inventing a reason.
Hard trap is a separate terminal with no reporter. These terminals have no
cleanup/unwind edges; target selection later exposes the defensive trap if a
nonreturning callee violates its contract. Explicit data initializers and inventory closure are checked during program
publication. Production discovery and native lowering remain future responsibilities.

The shared-release fixture uses ordinary loads/stores and branches for immortal,
ordinary and last-owner paths, then an indirect finalizer call and free of the
original header value. Enabled helper attribution and omitted tracing exercise
the same graph. It is representation evidence, not native finalizer equivalence.

## Immutable callable publication

`verify_callable` consumes a draft. It first establishes safe structural access,
then independently checks stored scalar/cast schemas, domains, object layouts,
memory, effects, signatures/services, trace associations and typed references.
Construction and verification share borrowed local schema rules; verification
rebinds stored IDs through checked arenas and recomputes derived facts rather
than trusting builder history. Graph success alone cannot publish a callable.

Guard evidence must name the exact secured operand and relation. Removing the
check's success edge must prevent access to the guarded operation, both from
entry when reachable and from the check itself. The latter checks dead regions
and rejects disconnected evidence. A reload cannot reuse an earlier value's
check. Constant evidence reads the actual defining constant: nonzero divisors,
counts strictly below width, and finite mathematically truncated floats within
the target integer range. NaNs, infinities and exclusive upper endpoints fail.

Only successful verification constructs `VerifiedCallable` and its
`CompletionReceipt`. The body exposes an immutable draft view. A receipt carries
its live context/callable owner, checked typed dependencies and a private unique
snapshot witness; equal bodies or equal live plans do not share authority.
Cloning a receipt preserves that exact witness without retaining the body.
These are genuine lowered callable receipts, not complete-program closure or
selected-stage authority. Program inventory reconciliation checks these against the chosen completion records.

Failures are ordered by local storage position and stable reason, with stage,
target/profile, source or generated callable identity and available origin spans.
Private conversion preserves the public `BackendError` shape, leaves generated
identities out of the source-callable field, and distinguishes capability/size
rejection from verifier defects. It introduces no production phase event.

This verifier checks supplied low-level execution, not the final-MIR projection,
source initialization or alias/lifetime correctness. Dynamic memory bounds,
physical ABI, native recipe equivalence and full trace-path parity remain with
their upstream or target owners. No native emitter consumes these products yet.

## Program inventories and target declarations

`ProgramBuilder` starts from the immutable checked declaration catalog. All
required source/generated bodies remain construction roots, including unused
complete-mode helpers; absent sources cannot be requested or built. `next`
chooses canonically. Discovery can request an already reserved helper, including
one currently building, without recursively constructing it. `begin` rejects
reentry and completed definitions. A verified state contains its receipt, so
completion state cannot diverge from its witness.

`complete` checks the chosen verified body and supplied receipt against the live
parent and exact snapshot before recording completion. Bodies may then be
released: bookkeeping retains receipts and typed dependencies rather than every
predecessor graph. Receipts certify completion; they do not reconstruct released
input bodies or implement the native streaming schedule. `finish` consumes construction state and checks all required
bodies and data definitions before producing `VerifiedProgram`. Consumers use
`require_input` to reconcile a chosen input witness; receipts from replacements,
other callables, other live contexts or other targets cannot substitute. Future
consuming edits must invalidate and republish program authority.

Data definitions explicitly contain byte, zero-fill and typed address
initializers. Their checked total width must exactly match the declared extent.
References must name the declared category and executable domain. Data addends
must fit the referenced extent (including a one-past address); code/TLS addends
are zero. Forward and cyclic data references require prior declarations, not
initializer construction order. Intrinsic failure bytes must match the shared
message catalog. Active statics require definitions in both artifact policies;
inactive complete-mode static declarations remain inspectable and cannot be
initialized or referenced. Runtime/external declarations and authorized null
dispatch slots require no fabricated body.

`TargetDeclarations` borrows the exact finalized parent inventory. It can declare
target thunks with existing logical signatures and constant data with existing
layouts, then define that data and freeze a `TargetExtension`. It cannot replace
parent declarations, introduce source/helper bodies or trace policy, or allocate
new layout/signature facts. New facts require replanning. Catalogs and initializer
iteration are canonical; the extension checks its parent publication witness,
separate from equality of declarations or callable receipts.

This freeze certifies declarations and data only. Selected drafts use frozen
extensions; a frozen extension creates no selected seal, placement result or
native emission authority. The
supplied catalog remains the inventory authority: these checks do not rediscover
semantic reachability, build real helpers or certify production MIR projection.

## Selected construction and target descriptions

`backend::selected` supplies independent selected draft arenas over the shared
block/value/object ID machinery. A selection context borrows a frozen target
extension and owns its resource catalog and symbolic ABI slot shapes. Checked
handles additionally carry a fresh selection scope: equal declarations and
numeric IDs in two contexts do not confer shared ownership. Source drafts require
an exact receipt from the finalized lower program; declared target thunks have
no fabricated lower input. Explicit stage maps preserve value/object origins and
record block provenance without copying lower definition sites.

Targets implement `Payload::describe` over their concrete opcodes. The immutable
view derives ordered operands and results from opcode fields and borrows ties,
clobbers, mandatory effects, typed artifact references, objects and ABI component
bindings. Transient owned operand descriptions are permitted; independent
editable use/def lists are absent. Shared structural analysis consumes this view
without matching target enums. Terminal payloads expose their successor count,
and graph storage contains every ordered edge and argument list.

Operand constraints distinguish legal resource views with a memory alternative,
a fixed view and a symbolic ABI slot. Representations have nonzero widths and
separate bits, float, data address and signature-qualified code address kinds.
Context-bound representation checks enforce address widths, capabilities and code signatures.
The draft records checked entry/result bindings. ABI binding construction preserves
exact logical component order, including
hidden destination and receiver roles, and checks representation/resource/slot
shapes against the parent facts. Slots identify components within incoming,
outgoing or result areas; they have no physical frame offsets.

The event order is **early uses, early clobbers, early definitions, late uses,
late clobbers, late definitions**. Call clobbers precede new results, including
results in the same fixed resource. Ties relate operand slots with distinct value
IDs and leave later input uses intact. Resource views name overlap units
independently of bank membership. Reservations conservatively exclude overlapping
views from allocation; fixed ABI bindings may still name reserved resources.
Partial preservation describes a unit footprint, so a narrow view can survive
while an overlapping wide view does not.

Atomic bundles prevent insertion inside flag-sensitive sequences. Bounded
recipes declare a positive step bound and explicit resource scratch requirements;
all effects and successors remain in the enclosing description and graph. The
synthetic tests exercise two-address and three-address descriptions, fixed call
results, early clobbers, flag branches and hidden ABI components. They establish
structural contracts, not native instruction completeness or ABI preservation.
Selected drafts grant no seal or placement authority. Independent shared/target
descriptor verification and immutable selected publication are the next step;
real target opcodes, ABI catalogs and physical realization remain future work.

## Regression ownership

Declaration tests are colocated under the plan owner; arena tests under the
arena owner; lowered tests under the draft owner, grouped by graph, scalar and
memory/call/trace contracts. They challenge foreign live contexts, wrong
targets/owners, bounds and overflow, signature roles/results, sparse body
dispositions, artifact categories, trace omission and stable domain/dispatch
handling. Public compile-fail examples protect private paths.

The maintained phase-boundary test also guards these narrower core scopes:
frontend/MIR execution and pass inputs, source lookup and physical x86 owners
cannot enter the declaration/arena/lowered/selected core. Private APIs without native
consumers temporarily have item-scoped non-test lint allowances. Their removal
obligations
are tracked in the [model roadmap](../roadmaps/LOW_LEVEL_IR_MODEL_ROADMAP.md);
the ordinary test build retains dead-code/import checking.

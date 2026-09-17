# Low-Level Execution Drafts

Status: private checked declarations and the complete lowered draft vocabulary
are implemented. Production emission still uses the [current backend](BACKEND.md).
Independent graph/value-flow verification is implemented. Full callable
verification/publication, selected graphs, edits, placement and native consumption
remain planned.

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
independent operation verification remains planned.

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
context/owner, not immutable snapshot freshness: later phase publication must
establish that separate authority, and edits must reverify it.

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
These records grant no proof: operand identity, constant validity and success-edge
protection still require independent verification. The independent graph checker
handles cross-block use ordering and forward-edge reconciliation. Guard/domain checks, object extents and independent effect/provenance
recomputation remain publication obligations. `finish` returns a draft even when
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
must all pass separate checks before a phase seal or receipt can be published.
Synthetic structural fixtures exercise the same algorithm for selected shapes;
selected payload storage and its target-specific verification remain planned.

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
Verification must recompute these facts independently before publication.

Call attribution distinguishes source operations, inherited boundaries, source
bodies entered from omitted helpers, nonreporting calls, hard defects and process
boundaries. Enabled trace plans bind local record ownership, frame eligibility,
context and permitted locations to the checked catalog. Ineligible helpers have
no local frame record. Explicit push/location-replacement/pop actions retain
ordering and associated instruction/terminal sites; attribution emits no action
implicitly. Omitted tracing rejects plans, actions and trace references, while a
compiler-defect span remains ordinary metadata. Association/path parity and
physical marshalling order still require independent/native checks.

`ReportFailure` contains its checked nonreturning panic call and a typed failure
message artifact with exact byte length. The shared failure catalog is also used
by current native emission, preventing a second message list. Arbitrary
nonreturning calls retain an explicit terminal call without inventing a reason.
Hard trap is a separate terminal with no reporter. These terminals have no
cleanup/unwind edges; target selection later exposes the defensive trap if a
nonreturning callee violates its contract. Data initializer/inventory publication
and native lowering remain future responsibilities.

The shared-release fixture uses ordinary loads/stores and branches for immortal,
ordinary and last-owner paths, then an indirect finalizer call and free of the
original header value. Enabled helper attribution and omitted tracing exercise
the same graph. It is representation evidence, not native finalizer equivalence.

## Regression ownership

Declaration tests are colocated under the plan owner; arena tests under the
arena owner; lowered tests under the draft owner, grouped by graph, scalar and
memory/call/trace contracts. They challenge foreign live contexts, wrong
targets/owners, bounds and overflow, signature roles/results, sparse body
dispositions, artifact categories, trace omission and stable domain/dispatch
handling. Public compile-fail examples protect private paths.

The maintained phase-boundary test also guards these narrower core scopes:
frontend/MIR execution and pass inputs, source lookup and physical x86 owners
cannot enter the declaration/arena/lowered core. Private APIs without native
consumers temporarily have item-scoped non-test lint allowances. Their removal
obligations
are tracked in the [model roadmap](../roadmaps/LOW_LEVEL_IR_MODEL_ROADMAP.md);
the ordinary test build retains dead-code/import checking.

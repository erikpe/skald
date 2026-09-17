# Low-Level Execution Drafts

Status: private checked declarations and lowered scalar/object drafts are
implemented. Production emission still uses the [current backend](BACKEND.md).
Calls/effects/traces, independent verification/publication, selected graphs,
edits, placement and native consumption remain planned.

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
Service effect summaries and independent operation verification remain planned.

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
protection still require independent verification. The builder also leaves
cross-block use ordering, forward-edge reconciliation, object extents and effects
to that verification. `finish` returns a draft even when reservations or blocks
remain incomplete; it creates no seal or emission authority.

Owner-private storage supports independently malformed test fixtures without
exposing an unchecked verified constructor. Arithmetic descriptors specify
meaning; native recipe equivalence is a separate target obligation.

## Regression ownership

Declaration tests are colocated under the plan owner; arena tests under the
arena owner; lowered tests under the draft owner, grouped by graph, scalar and
memory contracts. They challenge foreign live contexts, wrong targets/owners, bounds and overflow, signature roles/results, sparse body
dispositions, artifact categories, trace omission and stable domain/dispatch
handling. Public compile-fail examples protect private paths.

The maintained phase-boundary test also guards these narrower core scopes:
frontend/MIR execution and pass inputs, source lookup and physical x86 owners
cannot enter the declaration/arena/lowered core. Private APIs without native
consumers temporarily have item-scoped non-test lint allowances. Their removal obligations
are tracked in the [model roadmap](../roadmaps/LOW_LEVEL_IR_MODEL_ROADMAP.md);
the ordinary test build retains dead-code/import checking.

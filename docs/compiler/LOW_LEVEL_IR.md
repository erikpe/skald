# Low-Level Execution and Publication

Status: private checked declarations and the complete lowered draft vocabulary
are implemented. Production emission still uses the [current backend](BACKEND.md).
Independent graph/value-flow and full lowered callable verification are implemented.
Complete lowered-program inventory publication, plan-bound target catalogs,
selected graphs with exact parent reconciliation and consuming edits are implemented. Verified
products support immutable visitors and private deterministic text inspection.
The [x86 native contracts](X86_NATIVE_CONTRACTS.md) implement resource facts and
component classification. Private whole-program pilot admission and final-MIR
fact projection, ordinary scalar/CFG lowering and guarded numeric lowering are
implemented, including lower program closure, calls/tracing and generated entry.
Private native scalar/numeric selection and joint checking are implemented with
signature-local ABI slot shapes. General native calls/traces, placement and
physical realization remain planned.

## Shared final-MIR lowering

The private `backend::pilot` adapter constructs ordinary scalar bodies with the
standard lowered builder and publishes them through full callable verification.
Its streaming worklist registers exact body receipts and closes the complete
lowered program and frozen data inventory. Calls, enabled/omitted tracing and
generated entry use checked signatures and attribution. Consumer failure prevents
publication, and unsupported programs reject during whole-program admission.

Every retained MIR block is preserved, including unreachable blocks and repeated
successor occurrences. MIR computed values are block-local, so the adapter needs
no block parameters or cross-block value transport. Semantic locals and parameters
remain symbolic memory objects, independently of computed values and later
placement homes. Logical entry inputs are stored in their parameter objects;
loads/stores use frozen semantic layouts and builder-derived effects. Object
addresses are cached within each block. Explicit source lifetime markers name one
lexical storage site whose dynamic epochs may repeat in loops. Unit storage has
no scalar access or lifetime operation.

Source value and storage origins survive reservation. Primitive comparison
predicates retain operand types; floating constants retain raw bits and callable
addresses retain canonical code signatures. Lowering queries only admitted MIR
and frozen plan facts, without selecting registers, frame offsets or instructions.

### Guarded numeric lowering

Checked MIR diamonds secure operands in temporary memory carriers. The adapter
moves the success block's secured divisor/count/float load into its check block,
keeping that source value's identity and origin. Both the scalar-check terminator
and the operation use this exact immutable value; the original success load is
not emitted again. Other source loads and result stores retain their order and
semantic objects. These values may cross the exclusive success edge without
block parameters; this does not promote ordinary locals or change their memory
representation. Full publication checks the success-edge protection independently.

The closed numeric operations are the recipe associations consumed by selection:

| Lowered association | Required native semantics |
| --- | --- |
| `Divide`, operand type and quotient/remainder result, with nonzero evidence | Signed floor quotient and divisor-sign remainder; minimum/-1 gives minimum or zero. Selection expands the overflow case and floor correction before constrained division. Unsigned and byte results preserve their widths. |
| `Shift`, direction/type and below-width evidence | Check the complete original U64 count against 8 or 64 before any native count narrowing; signed right shift is arithmetic, unsigned right shift is logical. |
| `Convert::TruncateFloat`, target and finite/truncated-range evidence | Reject NaN/infinity and truncated values outside the target range. Negative fractions above -1 truncate to unsigned zero. Conversion and unsigned correction occur only after the guard. |
| Other typed `Convert` cells | Integer bit conversion and byte canonicalization, canonical boolean predicates/results, correctly rounded integer-to-float conversion, and exact float-bit reinterpretation retain distinct semantics. |
| `ReportFailure`, exact message and source attribution | Existing panic service with typed message address/length, reporting effects and source span; selection supplies the defensive trap. Enabled failure-location trace actions are owned with trace lowering. |

Native recipes reconcile these operations and their guard associations against
verified parent snapshots. Overflow, rounding, unsigned conversion and floor
corrections are explicit selection obligations, never repairs performed by frame
realization or rendering. The private numeric fixture oracle tests lowered
control/data relationships against boundary expectations; native equivalence
still requires the target recipe and execution probes.

## Ownership and checking

`backend::plan` owns supplied declaration facts, structural checking and borrowed
immutable views. It has no MIR executable enums, frontend state, source database,
physical instruction types or frame offsets. The facts contain target/profile
and trace/artifact policies, dense layout/signature pools, typed callable/artifact
keys, executable source dispositions, active statics and stable dispatch slots.

Checking consumes the supplied facts and validates their structural consistency.
The checker never recalculates semantic reachability. The private
`backend::pilot` planner obtains executable and static domains from verified final
MIR and certified retention services before freezing a checked plan. Supplied-fact
fixtures remain available for independent model tests. Present source bodies may acquire an executable callable binding.
An absent source declaration remains inspectable but cannot acquire that binding.
Target thunks cannot enter the shared declaration catalog.

The private planner checks every physically retained body and its storage,
places, signatures, calls and terminators against the scalar pilot whitelist.
Requesting reachable artifact emission does not remove unsupported bodies.
Receiverless static methods are eligible; receiver-bearing bodies, aliases,
aggregate/lifecycle operations, I/O, string panic and statics reject explicitly.
Complete artifact emission also rejects declared families requiring unsupported
generated lifecycle/metadata roots. Reachable emission uses certified runtime
obligations; unused declarations do not acquire executable authority.

The admitted product borrows the exact inspected MIR snapshot for shared lowering
and owns immutable checked facts. It preserves absent source declarations,
canonical higher-order function signature IDs, distinct semantic layout IDs,
external and runtime service declarations, entry and failure-message data.
Unused aggregate/alias declarations remain inspectable; they do not grant pilot
support. Empty lifecycle coordinators are absent. Selection and later phases use
plan views rather than the admitted product's MIR access.

Existing checked x86 layout and trace services have narrow projection adapters.
Enabled trace planning freezes owned byte strings, context/location records and
source-span mappings in canonical order; target symbols and source databases do
not escape the adapter. `TraceBytes` keys distinguish byte backing from activation
records. Omitted tracing returns empty metadata before any source lookup and
creates no trace/TLS declarations. Later artifact closure retains the used subset.
The public backend remains on legacy emission; admission creates no executable
lowered, selected or physical body and never falls back after a private failure.

The private shared adapter can close the complete admitted lower inventory. It
lowers scalar direct/indirect calls and C externs against frozen logical signatures,
with ordered conservative effects; normalized binary64 bit intrinsics remain
ordinary conversions. Source bodies in enabled mode own a two-word shadow frame
(previous frame and current location), initialized with their frozen definition
location before parameter stores. Calls update the frame immediately before the
call; checked numeric failures update only on their failure path before reporting.
Scalar result values and source stores precede the final return-frame pop.
The generated process entry owns no source frame: it calls the runtime ABI marker,
then language main and returns main's exact scalar result. Admission excludes
statics, so startup/shutdown coordinators remain absent rather than acquiring
synthetic empty bodies.

`lower_program` streams each verified body to a consumer and retains exact
completion receipts. It materializes all declared failure and enabled trace
bytes/context/location data with checked relocations, then publishes the lower
inventory witness. Trace TLS has a frozen declaration; its physical zero storage
and relocation recipe belong to later target catalog/emission work. Consumer
failure cannot publish closure. This proves shared lowering, without granting
selected, physical or executable authority; the public backend remains legacy.

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
that separate authority; consuming edits must reverify it.

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
context, an eligible frame's required initial location, and permitted locations to
the checked catalog. The initial location must belong to that permitted set.
Ineligible helpers have no local frame record. Explicit push/location-replacement/pop actions retain
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
publication. Native discovery and physical lowering remain future responsibilities.

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
input bodies. `finish` consumes construction state and checks all required
bodies and data definitions before producing `VerifiedProgram`. Consumers use
`require_input` to reconcile a chosen input witness; receipts from replacements,
other callables, other live contexts or other targets cannot substitute. Consuming
edits invalidate and republish program authority.

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

`TargetDeclarations` borrows the immutable checked plan, not a finalized lower
inventory. It declares target thunks with existing logical signatures and constant
data with existing layouts, defines that data and consumes itself to freeze a
`TargetCatalog`. It cannot replace parent declarations, introduce source/helper
bodies or trace policy, or allocate new layout/signature facts. New facts require
replanning. Catalog and initializer iteration remain canonical.

The catalog certifies declarations/data and binds the exact plan, profile and
trace policy. It grants no complete-program or native emission authority. Source
selection requires an actual `VerifiedCallable`; a detached receipt cannot start
construction. The selected callable retains that body's witness, so its storage
may be released after registering downstream completion. Local checking confirms
plan and owner; final program closure confirms the chosen executable snapshot.
Discovery-pass receipts cannot certify fresh executable-pass bodies.

`SelectedProgramBuilder` reserves required plan bodies and frozen target thunks.
`finish` consumes its state and requires a finalized `VerifiedProgram` from the
same plan. It checks all required selected completions and reconciles every source
input witness with the lower program's chosen receipt. Same IDs or equal body
contents do not establish snapshot equality. The resulting selected program
borrows its exact finalized parent authority. Wrong-parent, stale/replaced-input,
missing-definition, duplicate and foreign-context completions fail.

These APIs permit callable-at-a-time construction before lower-program closure;
production discovery/orchestration and physical closure remain planned. The
supplied catalog remains declaration authority: checks do not rediscover semantic
reachability, build real helpers or certify production MIR projection.

## Selected construction and target descriptions

`backend::selected` supplies independent selected draft arenas over the shared
block/value/object ID machinery. A selection context borrows a frozen target
catalog and owns its resource catalog and symbolic ABI slot shapes keyed by
checked signature identity. Entry and return slots use the owner signature; call
slots use the called signature. One context therefore admits different types at
slot zero in different boundaries while checking each area, index and type
strictly. Checked
handles additionally carry a fresh selection scope: equal declarations and
numeric IDs in two contexts do not confer shared ownership. Source drafts require
an actual verified lower callable with the same plan and owner; declared target
thunks have no fabricated lower input. Explicit stage maps preserve value/object origins and
record block provenance without copying lower definition sites.

Targets implement `Payload::describe` over their concrete opcodes. The immutable
view derives ordered operands and results from opcode fields and borrows ties,
clobbers, mandatory effects, typed artifact references, objects and ABI component
bindings. Transient owned operand descriptions are permitted; independent
editable use/def lists are absent. Shared structural analysis consumes this view
without matching target enums. Terminal payloads expose their flow kind and successor count,
and graph storage contains every ordered edge and argument list.

Target payload immutability and descriptor totality are private implementation
contracts: shared borrows cannot mutate opcode/descriptor facts, mutable aliases
cannot survive publication, and malformed drafts must yield verification errors
rather than descriptor indexing panics. Concrete target review and independent
recipe tests must enforce these properties; the generic trait bound cannot.

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
Selected drafts grant no seal or placement authority. `verify_selected` composes
shared context, graph, representation, resource, tie/timing, effect/reference,
ABI and bounded-recipe checks with a required target verifier. Calls retain their
logical signature and attribution; a secured indirect target is a separate early
use. Entry/call/return bindings preserve exact component order. Trace effects
require enabled policy and an explicit TLS dependency. Symbolic objects are
checked for layout, role and signature-qualified ABI-area extent.

The x86 target privately selects ordinary scalar lower graphs into native virtual
instructions. Its immutable opcode fields derive operands, legal register views,
ties, finite scratch bundles, effects and references. Independent native checking
also validates fields, the exact resource catalog, canonical entry/return ABI,
code-address signatures, pointer widths and proven object memory bounds. Branch
edges carrying parameters pass through distinct forwarding blocks, so transfers
occur after the flag-sensitive branch. Lower value/object/block maps and recipe
sites survive selection and consuming edits; edits require fresh joint verification.
Values have no selected stack homes. Constrained numeric operations, calls, traces,
placement and physical execution remain separate implementation obligations.

Selected static memory effects contribute typed dependencies to receipts even
when the opcode has no explicit static artifact operand.

Only successful shared and target checks publish an immutable
`VerifiedSelectedCallable`. Its receipt binds the live selection context, exact
selected snapshot, checked references and chosen lower input; generated thunks
have no fictitious lower input. Complete selected publication closes required
source bodies and target thunks and reconciles those exact input witnesses with
the finalized lower program. Bodies may be released after local completion.
Lower completion receipts retain immutable trace plans and site-to-location
associations so target checking can validate frame records, initial locations and
attributed calls after executable lower storage has been released.

Targets must independently check opcode completeness, mandatory effects and
references, real resource footprints and target-specific control-flow rules.
There is no default accepting verifier. Synthetic tests demonstrate guard and
correction graphs, simultaneous edges, secured indirect calls, inherited helper
attribution, partial resource preservation and receipt binding. They prove the
shared interface is usable across two target shapes, not physical preservation.
The private x86 pilot implements concrete opcodes and its ABI catalog. Placement
and frame realization remain future work.

### Selected numeric and call-effect checking

The x86 owner publishes concrete numeric cells and visible correction CFG under
joint shared/native checking; see [native numeric publication](X86_NATIVE_CONTRACTS.md#concrete-numeric-publication).
Descriptions expose fixed divide pairs, nonaliasing divisors and tied CL shifts.
Zero-code semantic associations remain independently checked metadata and grant
no execution or placement authority themselves.

Immutable selected visitors tie borrowed facts to the inspected snapshot's
lifetime. Consumers may collect those borrows for a single verification or test
session without cloning opcode payloads; consuming edits still invalidate all
previous receipts and analysis. Builder handle lookup validates arena ownership.

Trace-state effects on calls preserve the callee's possible trace observation as
a barrier, including omitted tracing. They do not imply a caller TLS reference.
An explicit non-call trace-state access requires enabled policy and a declared
TLS artifact. Native reporter terminals retain the canonical runtime service's
memory-read, call, report, trace-state and hard-trap effects and exact ABI/data
associations. General native calls, entry and explicit TLS/trace memory selection
are implemented; see [call and trace publication](X86_NATIVE_CONTRACTS.md#concrete-call-and-trace-publication).
Shared indirect descriptors accept the target-defined early or late use timing;
the native verifier requires a secured late target in R11. Physical execution
remains planned.

## Consuming edits and snapshot analyses

Both stages have private editor owners. Backend orchestration opens an edit by
consuming the complete program and its exact chosen callable through `edit`.
The result is an unpublished editor and a program builder missing that callable's
completion. Other chosen receipts and data definitions remain intact. Selected
program edits retain the exact finalized lower parent; even another program
publication with identical chosen callable receipts cannot replace that bound
authority. A callable can enter an editor directly only inside its owning phase,
before program
admission. No public constructor or mutable published-draft accessor exists.

Editors replace instruction or terminal uses, redirect individual edge occurrences
with simultaneous argument lists, split blocks, and rebuild compact arenas in
explicit value/block/object order. Partial remaps fail on any surviving reference
to a deleted ID; they never substitute identity mappings. Remaps retain their
source receipt and reject a different snapshot. Numeric IDs may change: use the
returned mappings rather than old handles after rebuilding. Origins stay with
preserved records. Lowered rebuilding covers calls, scalar evidence, trace plans
and sites, object effects and edge arguments. Splitting relocates moved scalar
checks and instruction/terminal trace associations.

Selected targets implement `EditablePayload` by rewriting concrete opcode fields,
including definitions and symbolic objects, and relocating their own indexed
metadata. Shared code rewrites stored edges and stage-origin maps; it never edits
a description or matches physical opcodes. Target callbacks have no accepting
default. Splits require an explicit target transfer payload.

Editor drafts are unverified working state. Rebuild/finish regenerates definition
sites; lowered edits discard cached address claims and conservatively widen
required effects without dropping existing barriers. Publication recomputes
provenance, guards and references. Replacing a use preserves existing evidence so
verification can reject an invalidated guard. `finish` consumes the editor and
runs the full owning verifier, including both verification layers for selected
code. Success produces a fresh receipt; stale completions cannot certify it.
Reclosed programs reconcile their exact chosen snapshots, and selected programs
also expose `require_input` for that check.

A verified callable's `analysis` returns the shared read-only graph session
borrowing its exact immutable draft. Its lifetime prevents consumption while the
view is still used. Callable, program and analysis authority is non-cloneable;
receipts may be cloned but retain their original snapshot identity. There is no
global cache, generic pass manager, placement stub or production pipeline change.

## Immutable inspection and text dumps

Each verified stage owns `visit` and `dump`. Lowered visitors borrow typed value,
object and block records, including their ordered instructions and terminals.
Selected visitors expose entry/ABI facts, resource banks/views, explicit stage
origin maps, representations, definition/origin facts, block parameters, opcode
payloads and edge occurrences. All records are
immutable borrows; visitors grant no publication, mutable model, MIR or source
lookup authority.

Text starts with a schema version, stage, verified status and canonical callable
key, then target/profile, trace/artifact policy and checked declarations. Dense
arena order and ordered maps define output order. Graphs print entry, inputs,
objects, definitions, origins, instructions/results, terminals and edge slots.
Lowered output preserves binary64 constants as 16 hexadecimal bit digits, memory
widths/alignments, guard evidence, effects and call attribution. Selected output
also prints representations, symbolic ABI bindings/areas, resource footprints,
reservations, ties, early/late operand events, clobbers and bounded bundles.
`InspectPayload::fmt_opcode` is a required target-owned formatter for opcode
choices and immediate data absent from structural descriptions. Implementations
must be total on diagnostic drafts and format deterministic scalar facts, never
live context identity or host pointers. Shared rendering uses `describe` for
structural facts rather than target enums.

`dump_inventory` prints canonical completion keys and typed dependencies without
reconstructing released bodies or serializing receipt/context witnesses. A draft's
separate `dump_draft` prints `status=unverified-draft` and marks unresolved
reservations; it does not follow potentially invalid references or certify the
result. Writers receive streaming text and errors propagate normally. These are
private debugging APIs, not a stable external wire format or importer.

Production phase events, public dump adapters and requested metrics remain
pending until the native pipeline executes these phases. Current CLI dumps still
observe the existing production compiler. No synthetic stage event, new CLI
switch or native performance claim follows from model inspection.

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
are tracked in the active [program handoff](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#retained-model-artifacts-and-removal-owners);
the ordinary test build retains dead-code/import checking.

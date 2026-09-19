# Low-Level IR Model, Construction, and Verification Design Proposal

Status: accepted, frozen and promoted LA02 design, 2026-09-17. Accepted by the
user after review of the draft committed at `10b5de75`; source assessment is
against `027af94f`, the phase-preparation closing change. The executable model
is implemented and the child workstream is archived. The streaming publication
amendment accepted on 2026-09-18 below is implemented by the active native
phase roadmap; the completed model roadmap validates the original schedule.
The [implementation roadmap](LOW_LEVEL_IR_MODEL_ROADMAP.md) records cumulative
review and validation; the [active program handoff](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#common-model-readiness)
owns native counterpart and retained-artifact obligations. Changes to these
contracts require an explicit design amendment
before dependent implementation.

Parent: [Low-Level Compiler Architecture](../roadmaps/LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
Inherited authority: [frozen phase design](LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md).
Preparation input: [migration handoff](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#preparation-handoff-and-next-designs).
Program implementation baseline remains `495debd3`; a child implementation
baseline must be recorded immediately before its first implementation task.

## Purpose and completion boundary

Give shared lowering and target selection a concrete executable representation
with identities, construction APIs, publication checks and deterministic
inspection. A value denotes a computation, an object denotes addressable
memory, and a physical location denotes where placement makes a value available.
Those three concepts must stay separate.

LA02 delivers the lowered model and verifier, shared selected-graph/operand
contracts exercised with a synthetic target, builder/edit APIs, immutable
inspection and phase-owned dumps. Its tests must construct and verify meaningful
graphs, including calls and guarded arithmetic; declarations without consumers
are insufficient. It does not deliver production MIR-to-LIR lowering or an
executable native pilot. Those begin in LA03 and expand in LA04.

LA03 owns actual x86 selected opcodes, register resources, ABI locations,
transfers, stack placement/checking, frames, physical legalization and native
emission. LA04 owns complete language/helper construction. LA05 owns production
adoption, observations and legacy removal; LA06 owns allocation. No allocator,
semantic SSA conversion, local promotion, scheduling, general alias analysis,
new runtime service, public mutable LIR API or AArch64 backend is introduced here.

## Current source and inherited constraints

The current [backend facade](../../crates/skald-compiler/src/backend/mod.rs)
accepts only verified final MIR, with source access available only for enabled
runtime tracing. Preserve its public target/input/emission contract. Planning
may inspect verified declarations and narrow retention queries; new LIR
consumers cannot query MIR to recover executable meaning.

Three existing owners need different representations:

| Current owner | Coupling | Proposed representation boundary |
| --- | --- | --- |
| [MIR local IDs](../../crates/skald-compiler/src/mir/model/ids.rs) and [frame planning](../../crates/skald-compiler/src/backend/x86_64_sysv/frame.rs) | Semantic storages/values are assigned fixed homes before selection | New callable-owned machine value/object IDs; no home in lowered LIR |
| [Physical machine model](../../crates/skald-compiler/src/backend/x86_64_sysv/machine.rs) | Physical operands and string labels obscure virtual dataflow/CFG | Separate selected payload with explicit virtual operands and graph edges |
| [ABI classification](../../crates/skald-compiler/src/backend/x86_64_sysv/abi.rs) and [call attribution](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/call/emission.rs) | Physical locations are classified early; attribution is audited then discarded | Logical component signatures plus retained call/trace metadata, followed by target ABI assignment |

The frozen contract selects layout-specialized shared lowering, separate verified
phase products, single-definition values, block parameters, explicit memory and
complete lifecycle expansion before lowered publication. This proposal refines
those decisions; it does not replace them. The continuing
[coverage inventory](../roadmaps/LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md) owns exhaustive
MIR/helper migration accounting rather than a second inventory in this document.

## Accepted decisions

| Question | Accepted decision |
| --- | --- |
| Shared storage | Typed dense vectors with explicit block order and entry; no hash-map iteration defines execution or dump order |
| IDs | Distinct program, callable, local and stage IDs; MIR IDs appear only in origins/planning keys |
| Values | One definition per value, exact types and definition sites; block parameters carry joins; no mutable virtual-register assignment |
| Objects | Symbolic size/alignment/role and lifetime metadata, independent of value placement; no frame displacement |
| Operations | Closed lowered executable enums with precisely defined scalar operations, addresses, memory, calls, lifetime markers and trace actions |
| Checked scalar domains | Three finite scalar-check relations with success-edge or directly verified constant evidence, tied to exact value IDs; no general proof language |
| CFG | Explicit edge occurrences and simultaneous argument lists; introduced target branches are graph nodes, never private labels |
| Effects | Structural operations derive mandatory effects; call services supply conservative checked summaries; annotations cannot declare a store or unknown call pure |
| Calls | Checked logical component signatures; direct/indirect target identity and attribution retained; no implicit aggregate or ownership work |
| Generated inventories | Immutable declaration facts, deterministic worklists, staged inventory finalization; no mutation of a context already used by a seal |
| Selected interface | Separate target payload over shared CFG/value utilities; a narrow operand/resource/effect view, without a universal target opcode enum |
| Authority | Verifier-owned seals; editing consumes the product; analyses and future placement borrow exact immutable snapshots |
| Inspection | Canonical text dump and read-only visitor views; no public importer, JSON round trip or CLI switch without a production consumer |

Names and record fields below specify responsibilities and invariants. The
roadmap may arrange cohesive Rust files differently; it must not weaken these
contracts or replace them with one permissive stage enum.

## Context, identities, and storage

### Immutable planning and stage contexts

A `CheckedPlan` contains an immutable target-profile key (architecture, ABI,
data layout and enabled capabilities), trace policy, logical signatures,
checked object/layout facts, source-callable declarations and shared helper/data
declaration catalog. It also carries the permitted executable-body domain,
certified static domain and stable dispatch facts projected from `BackendInput`.
Planning and shared MIR-to-LIR lowering receive verified MIR; only enabled trace
planning receives source lookup. Models, graph utilities and selected consumers
receive narrow checked facts. A callable's origin is attribution, not authority
to retrieve an absent MIR body.

The LIR-facing plan view supplies execution facts and declarations without
physical registers, target instructions, relocations or frame offsets. Existing
semantic IDs may identify a layout/helper request, but no LIR opcode needs a
nominal type query to execute. LA03 supplies actual target facts and capability
checking; LA02 tests this boundary with checked fixture plans.

Use borrowed immutable contexts, not a global context registry. Verified products
retain their context reference and consumers require that same live context;
identity comparison is private and never serialized. Rust lifetimes alone do
not distinguish two concurrently alive plans: publication/consumption also checks
context identity and target profile. Structurally equal plans from separate
compilations do not confer interchangeable authority.

The finalized lowered inventory accompanies the plan at complete-program
publication. Selection uses a separately finalized extension for target thunks,
constant data and resource/ABI descriptors. The extension retains its parent
plan/inventory identity; it cannot overwrite their layouts, signatures or IDs.
Selected callables bind to this selection context as well as their target.
The accepted streaming amendment below refines the extension's parent binding:
declaration freeze binds the checked plan; exact finalized-parent inventory
binding is required at selected-program closure, not at callable construction.

Each publication also creates a private snapshot witness. A completion receipt
binds that witness, context, callable, stage and verified reference requirements,
not just the callable's numeric ID. A derived stage retains its input witness
without retaining the input body. Program finalization reconciles receipts with
the actual chosen body/derivation records; a receipt from an earlier edit cannot
certify its replacement. Editing/replacing a body consumes the corresponding
complete-program authority and requires fresh receipts/publication. These small
immutable witnesses are private verification bookkeeping, never dump/cache IDs
or mutable globally indexed certificates.

### Identity domains

| Identity | Domain and purpose |
| --- | --- |
| `LirCallableId` | Source callable, shared helper, program coordinator, entry wrapper or target thunk, distinguished by typed keys |
| `ArtifactId` | Typed callable/data/runtime/external identity within one inventory; category checked at each use |
| `SignatureId`, `LayoutId` | Immutable checked declaration facts in the plan |
| `LoweredBlockId`, `LoweredValueId`, `LoweredObjectId` | Callable owner plus private arena index |
| `SelectedBlockId`, `SelectedValueId`, `SelectedObjectId` | A separate selected domain; no implicit conversion from lowered IDs |
| Instruction/check/edge locations | Callable, stage, block and ordinal; an edge occurrence distinguishes two edges to the same successor |
| Trace location/context IDs | Enabled-only declaration inventory, not arbitrary source spans interpreted by selection |

Indices use checked `usize` allocation, matching current repository arenas and
avoiding an unrelated new 32-bit program-size limit. Constructors stay inside
the owning builder. Validate ownership and bounds before indexing; malformed
internal products return a structured verification failure rather than panic
through indexing. Program IDs are interpreted through their bound context;
numeric equality is never a freshness test.

Source and generated keys are separate: a helper is not a fabricated semantic
`CallableId`. Shared helper keys enumerate the current array lifecycle families,
raw-address copy wrappers, retain/release, complete class/optional-box finalizers
and program lifecycle. Their checked shape/signature key distinguishes layout
specializations. Target thunk keys are owned by the target inventory; shared
code sees typed declarations rather than x86-specific key variants.

Blocks contain parameter IDs, ordered instruction records and exactly one
terminator. The callable owns value/object tables and an explicit entry ID;
entry need not have index zero. Values record type, definition site (entry input,
block parameter or instruction result) and optional origin. Instructions own
ordered result IDs. Types, results and definition records must agree exactly.

IDs are stable within an immutable product, not promised across edits or stages.
An edit may rebuild compact arenas; it returns explicit remaps for preserved
origins/references. Deleted IDs cannot remain referenced. Selection allocates
fresh IDs, including its correction blocks and temporaries, and records origin
links without treating an old MIR/LIR index as the new graph's identity.

## Lowered values, operations, and memory

### Execution types and canonical forms

The initial scalar vocabulary is `I64`, `U64`, `U8`, `Bool`, `F64`, `DataAddress`
and `CodeAddress(SignatureId)`. Pointer width comes from the checked target plan;
addresses are not assumed to be x86 integers. Integer signedness and float bits
are explicit. No `unit`, class, array, optional, interface or `Obj` virtual value
exists: their executable components are scalar/address values and memory.

`U8` always denotes the low eight bits with a canonical zero-extended scalar
representation; `Bool` is exactly zero or one. Constants, loads, arithmetic,
conversions, call inputs/results and edge arguments obey the same contract.
Canonicalization must not disappear because placement eliminates a store.
Float constants store binary64 bits, preserving negative zero and NaN payloads;
no host floating formatting defines constant identity.

Data addresses can carry null where runtime representations require it.
Code addresses carry a checked logical signature; dynamic dispatch/finalizer
loads name that signature explicitly. Null/invalid target handling is expanded
by shared lowering under the existing semantic contract; the type does not
silently add a source-language null check or claim to prove runtime non-nullness.
Address/integer conversions are explicit pointer-width operations needed by
runtime representations; they do not grant pointer alias or ownership facts.

### Closed instruction vocabulary

| Family | Operations / required meaning |
| --- | --- |
| Constants | Typed integer/boolean/binary64/address-null constants; exact bits |
| Integer arithmetic | Negate, add, subtract, multiply and complement/and/or/xor; wrapping at the declared width, including canonical `U8` results |
| Floating arithmetic | Negate, add, subtract, multiply and divide; existing binary64 semantics without reassociation or fast-math permission |
| Comparisons | Six primitive predicates, signed/unsigned interpretation from operand type, existing NaN behavior; canonical `Bool`; boolean/address equality/inequality and logical not |
| Division | Quotient or remainder for `I64`/`U64`/`U8`; signed floor semantics, including the current `MIN / -1` result; matching nonzero-divisor evidence |
| Shifts | Left, arithmetic right or logical right; width 8 or 64, `U64` count and matching below-width evidence |
| Conversions | Identity, integer-bit conversion, to/from boolean, integer/boolean to binary64, binary64/`U64` bit reinterpretation, guarded binary64-to-integer truncation |
| Addresses | Symbol/object address, byte offset, scaled index and explicit pointer-bit conversion; checked sizes/strides, no concrete frame displacement or hidden bounds/lifecycle check |
| Memory | Scalar load/store with representation, byte width and alignment; explicit pointer source and mandatory effects |
| Calls | Direct or signature-typed indirect target, component inputs/results, checked service effects and attribution |
| Lifetimes | Start/end of a symbolic object's recorded lifetime site; ordering metadata, no value definition or implicit cleanup |
| Traces | Push frame, replace location and pop frame using enabled-only symbolic records; ordered trace-state effects |

Reuse the existing neutral
[comparison predicate descriptor](../../crates/skald-compiler/src/primitive_comparison.rs).
Do not make HIR or MIR execution enums the low-level model's API. Cast/check
descriptors specify their source/target cells directly, with semantic expectations
from the existing [primitive cast contract](../language/TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix).
Identity casts may become value reuse in lowering; signed floor correction and
checked float conversion remain precisely defined operations for target recipes.
Their resulting target CFG must be explicit before selected publication.

Signed quotient rounds toward negative infinity and remainder follows
`dividend - quotient * divisor`, with the current overflow pair explicitly
returning `MIN` and zero for quotient/remainder. Integer bit conversions preserve
same-width bits, zero-extend `U8` or narrow to its low byte. Boolean conversion
tests nonzero: either floating zero is false and NaN is true; conversion from
boolean produces zero/one. Binary64 equality/ordering with NaN is false and
inequality is true. No fast-math or integer-overflow trap is implied by these
operations. The existing type/value contract remains authoritative for all cast
cells and floating environment behavior.
Address equality compares pointer-width bits of matching address types; code
addresses additionally require the same signature. No implicit address ordering,
dereference or ownership comparison is introduced.

Addresses compute target pointer-width bits; byte-offset/index operations do not
implicitly check bounds, allocate, dereference or report. Shared lowering expands
the verified checks before forming an access. Runtime headers, array descriptors,
optional state, dispatch tables and ownership graphs use these ordinary operations.
No `ReleaseShared`, `DestroyObject`, `ConstructArray`, opaque legacy fragment,
target callback or arbitrary assembly string is legal in a lowered instruction.

### Guarded scalar operations

Retain three finite check relations in `LoweredTerminator::ScalarCheck`:

- `NonZeroDivisor(integer_type, divisor)`;
- `ShiftCountBelowWidth(U64 count, width)`;
- `FiniteTruncatedF64InIntegerRange(source, integer_target)`.

The terminator has explicit success/failure edges. A guarded instruction names
the check location and exact operand/relation. Publication verifies the success
edge protects the instruction: removing that edge makes the instruction
unreachable from entry, and the check itself precedes the use. Reject a bypass,
operand substitution, wrong width/target, failure-path use or stale remap. This
evidence concerns immutable virtual values, not mutable storage names. Guard
references add verification obligations, not machine-register uses.
For an unreachable guarded region, also check paths rooted at the check itself;
a failure edge reaching the operation cannot pass merely because entry cannot
reach either block. A check must reach its associated guarded operation through
success, not serve as arbitrary evidence for a disconnected instruction.

Lowering must secure the checked value and reuse it on success, including where
current MIR checks and operations name storage carriers. A later reload is a new
value and needs its own justified check; no alias assumption equates two loads.
The float relation means finite with mathematical truncation toward zero in the
target's range, preserving valid fractions near a boundary. It is not simply an
inclusive comparison of the original float with the integer endpoints.

`ScalarDomainEvidence` is either a matching success-edge check or an exact
constant satisfying the same finite relation. The verifier checks constant bits
and operand identity itself; there is no unchecked "known safe" annotation.
This permits constant helper shifts and already-folded checked operations without
inserting redundant failure CFG. Test zero/nonzero divisors, 7/8 and 63/64 shift
counts, finite fractional float boundaries and NaN/infinities with independent
oracles. Propagation of general arithmetic facts is outside scope.

Checks select scalar predicates and branches; shared lowering chooses the
failure block and exact reporting attribution. Selection consumes their evidence
while expanding physical operations and any additional correction CFG. There
is no cross-phase certificate imported from MIR and no general theorem prover.
Native recipe tests still establish that selected arithmetic implements the
declared relation and failure contract.

### Symbolic objects and lifetimes

An object declaration has checked size/alignment, role, semantic origin when
available, and lifetime disposition. Alignment is nonzero and a power of two;
size/offset arithmetic is checked independently of target encoding limits.
Roles initially cover semantic storage, aggregate result/temporary and trace
record. Borrowed/receiver/alias addresses are input values, not fictional local
object allocations. Target ABI areas and placement-created value/spill/save or
scratch objects belong to later owners and cannot masquerade as semantic locals.

Lowered loads/stores specify memory width separately from scalar type. A byte
field load can define canonical `U8`/`Bool`; a parameter or carrier object can
retain a larger checked storage representation. Object size comes from planning,
not from assuming every value needs eight bytes. Origins map all current MIR
storage roles to an explicit disposition; proof-only storage remains forbidden.

Addressable zero-size objects retain an object identity and explicit size-zero
disposition; this does not promise distinct physical addresses or silently add
padding. Elided unit and metadata-only views produce no fictitious payload or
call result. Target realization must preserve existing address/layout behavior.
Initial placement never reuses semantic object storage across lifetime epochs.

Lifetime markers record the original initialization/lifetime boundaries, including
repeated loop executions of one static lifetime site. The structural verifier
checks referenced objects/sites, not source definite initialization or all alias
lifetimes. Final MIR remains that authority. A pointer's last use and an object's
semantic lifetime are different facts; neither marker establishes value liveness.

### Effects and address provenance

Mandatory effect classes are object/static/unknown memory reads and writes,
call, allocation, free, reporting termination, hard trap and trace-state access.
An unknown memory effect may alias addressable locals and statics; it is never
disjoint merely because another operand has a known object ID.

Compute known address provenance from object/symbol formation and preserved
offset operations. Loaded pointers, external/alias inputs and unsupported merges
are unknown unless all incoming provenances agree. Optional ownership/origin
annotations provide attribution only. Loads/stores derive their minimum effects
from this provenance; user-supplied summaries may widen, never narrow them.
Calls obtain effects from a checked declaration/service catalog; default internal,
indirect and external calls conservatively read/write unknown memory and may
allocate, free and report unless a reviewed contract is stronger. Every call
retains an ordered call barrier; a narrower memory summary alone is not purity.
Runtime alloc/free/panic and I/O
have explicit service records; no string name guesses their behavior.

No memory motion, purity inference or general alias analysis is delivered.
Preserve instruction order, call/destruction order and trace ordering. Placement
may add transfers to its own value homes, but cannot move an operation past an
observable effect or reuse a live semantic object. A missing effect declaration
is a verifier defect, not permission to optimize.

## CFG and value-flow rules

Terminators are `Jump(Edge)`, `Branch(Bool, true_edge, false_edge)`, the finite
`ScalarCheck`, `Return(component_values)`, `ReportFailure(reporter_call, reason)`
and `HardTrap`. A reported terminator explicitly contains its nonreturning call
target/signature, message components and attribution; selection exposes the call,
clobbers and defensive trap if a supposedly nonreturning reporter returns.
No panic/unwind cleanup successor exists. Arbitrary nonreturning service calls
use the same explicit terminal-call structure, without inventing a panic reason.

Entry inputs match the checked signature's input components and are defined
once before entry instructions. Entry has no predecessor or edge parameters;
an iteration jumps to a separate loop header. Other blocks have ordered typed
parameters. An edge's arguments are uses at the predecessor terminator, with
exact arity and type agreement. Parameters define fresh values on block entry;
edge arguments transfer simultaneously, including swaps and cyclic copies.

Two edges may share a successor and pass different argument lists; edge occurrence
identity, rather than predecessor/successor pairs alone, distinguishes them.
Critical edges are legal. LA03 explicitly splits them or schedules edge-local
transfers before placement; a split forwards parameters/arguments and must be
reverified. No allocator restriction silently changes the LIR graph contract.

Every value has exactly one definition. Uses must name a live table entry in the
same callable/stage, with exact type and result-definition agreement. Verify
same-block ordering and entry-rooted dominance; edge uses occur before successor
parameters are defined. A loop-carried value enters through a block parameter,
not an instruction overwriting a prior virtual ID. A selected two-address tie
also constrains two *different* use/result IDs to a location; it is not a second
definition of the input value.

Unreachable blocks may remain for complete-mode inspection. Verify their IDs,
types, definitions, edges and local order too. Entry-path dominance is vacuous
for unreachable uses; analysis APIs return unknown for cross-block dominance
queries involving them, so transformations cannot exploit that vacuity. Any edit
that makes such a block reachable must pass ordinary dominance/check verification.
Keeping these blocks does not authorize a missing semantic body or imply a second
whole-program reachability analysis. No automatic block pruning changes artifact
or trace retention in this workstream.

## Calls, traces, and artifact inventories

### Logical signatures and calls

A checked `Signature` records convention identity, ordered input components,
ordered result components, return behavior and conservative service effects.
Each component has a scalar/address type and role: explicit parameter index,
aggregate address, result destination, receiver static/complete/metadata address,
alias address/origin, or runtime parameter. Role identity is independent of
its list position and eventual ABI location.

Unit has no result component. An aggregate result has a caller-owned destination
input and no fictitious scalar return; an owning handle has its explicit scalar
address result. Receiver and object-origin triples remain distinct components.
Direct targets reference a typed callable/runtime/external declaration. Indirect
targets are values with the same `CodeAddress(SignatureId)` as the call; no
virtual/interface call opcode survives lowered publication. Selection sees the
already resolved metadata/table access and indirect target.

Call inputs are previously computed values in source evaluation order. Lowering
secures earlier arguments, indirect targets and ownership anchors before later
argument effects; a call opcode cannot reread a MIR place to reconstruct them.
Entry, call and return all consume the same logical signature. Selection later
adds a checked physical ABI binding for each component; role-based destinations
allow the frozen AArch64 result witness without pretending it is a leading scalar.

Retain call attribution with the six current meanings: source operation,
inherited operation, source body entered from an omitted helper, nonreporting,
hard-defect-only and process boundary. Calls identify the symbolic source
operation/location or inherited boundary. That metadata does not grant call
purity or replace an ordered trace action. Helper/destructor native witnesses
remain the oracle for visible frame and allocation/destruction behavior.

### Trace actions and failures

For enabled tracing, a callable's checked trace plan declares frame eligibility,
symbolic record and permitted location IDs. Push, location replacement and pop
actions use those IDs; publication checks policy, ownership and record/action
consistency. Reporting/call-site updates remain ordered immediately before the
associated operation after marshalling. Generated helpers inherit attribution;
source-authored bodies retain their own frames.

Omitted tracing has no trace record/action/location inventory, source access,
metadata or TLS reference. Reject trace actions/artifact references under omitted
policy even if a caller claims they will be dead. A source `Span` retained for
compiler defect attribution does not authorize source-text lookup.

The lowered verifier checks local action references and required associations;
full path-sensitive trace parity belongs to LA03/LA04 recipe/native tests.
Selection expands every trace action into explicit selected instructions,
temporaries and trace-state effects. No emitter-only scratch register is allowed.
Reported failure and hard defect remain different terminal operations with exact
existing messages; neither acquires an unwind or extra destruction path.

### Inventories and generated work

Typed artifact references distinguish callable bodies, data/table/literal/static/
trace objects, runtime functions, TLS data and user externals. An address/reference
includes any checked byte addend; target relocation kind is selected later.
Data declarations describe checked size/alignment and byte/zero/address initializers
with explicit dependencies. No assembly-string parsing determines dependencies.

Use staged publication rather than a mutable universal program context:

1. Planning freezes source and shared helper/data declaration facts, including
   allowed helper shape/signature keys. The existing complete-mode helper families
   remain roots of the construction inventory where current behavior requires
   them; demand-only generation is not a hidden optimization in this proposal.
2. Shared lowering reserves declared helper keys before constructing their bodies.
   A canonical worklist tracks declared, building and verified states; recursive
   requests refer to the reserved declaration. It never recursively constructs
   an already-building body or invents a body for an absent source definition.
3. Each callable can be verified against the immutable plan while other declared
   helper bodies are pending. Its references/signatures must already be declared.
   Complete lowered-program publication finalizes the inventory and requires
   verifier-produced completion receipts for every required generated definition.
   A receipt certifies completion without retaining all intermediate body payloads.
4. Target selection freezes a separate extension of target-specific declarations
   and operand/resource/ABI descriptors before selected publication. Thunks use
   already checked logical signature shapes; new layout/signature facts require
   explicit replanning, never mutation of a sealed parent context. Target worklist
   closure and target verification apply to thunks just as to selected source bodies.

Unknown keys, unresolved required bodies, duplicate/conflicting definitions,
category-mismatched references and cross-context receipts are defects. External
declarations need no body. Verified-unused dispatch slots have explicit null
dispositions; dense declaration presence alone is not body authority.

Allocate/order generated keys canonically by family/shape/signature, pool bytes
deterministically and never use hash arrival order or pointers in emitted identity.
LA03 must demonstrate its target-inventory discovery/freeze strategy without
requiring every intermediate body to remain resident. Frozen declarations and
small completion receipts support streaming one callable at a time. Complete
and reachable artifact modes retain their current semantics; final physical
references may add target dependencies but cannot resurrect removed MIR bodies.

## Accepted streaming publication amendment

Accepted 2026-09-18 with the [frozen native target design](TARGET_SELECTION_PHYSICAL_REALIZATION_DESIGN_PROPOSAL.md#declaration-freeze-and-callable-streaming).
The [native phase roadmap](TARGET_SELECTION_PHYSICAL_REALIZATION_ROADMAP.md)
implements this change. This amendment supersedes any reading of staged
publication above that requires a finalized lower program before source
selection begins; it does not weaken complete-program authority.

- Freeze a separate target declaration catalog against the exact immutable
  checked plan, target/profile, trace policy and permitted declaration domains.
  The catalog cannot mutate layout/signature pools or authorize absent bodies.
  It grants selection context, never complete-program authority.
- Discovery may lower and verify one callable at a time to collect canonical
  target requests. Discard discovery receipts; the executable pass creates new
  verified snapshots and must derive from those actual bodies.
- During the executable pass, register each lower receipt, select from that
  verified callable, retain its exact input witness in the selected receipt,
  and release predecessor bodies after registering downstream derivations.
- Consume program worklists at closure. Reconcile selected inputs with the exact
  chosen receipts of the finalized lower program, then reconcile physical
  inputs with the finalized selected program. Bind complete products to their
  exact parent authority and close all required thunks/data and typed dependencies.
- Replacing an input or changing a catalog invalidates affected downstream
  authority. Wrong-parent, stale-snapshot, missing-definition and premature
  publication rejection remain mandatory; matching IDs or plan identity alone
  cannot establish derivation equality.

The native phase roadmap replaces the finalized-parent `TargetExtension` with
a plan-bound `TargetCatalog`; selected-program closure borrows the exact finalized
lower parent and checks its chosen witnesses. The original invariant coverage is
retained. Neither the amendment nor a catalog seal authorizes emission from
partially closed programs. Production streaming orchestration remains downstream.

## Selected graph and target-facing structural interface

Share graph traversal, identity validation, parameter/edge rules and dominance
algorithms; keep lowered and selected instruction/terminator types separate.
A private selected graph is parameterized by a concrete target payload. Dispatch
chooses the target at orchestration; shared verification/placement does not match
on x86 register or opcode enums.

The target adapter provides a borrowed structural description derived from the
opcode, not an independently editable use/def list:

| Description | Shared consumer requirement |
| --- | --- |
| Values/results | Ordered virtual uses/defs, representation width/kind and exact definition site; selected-only bit-width temporaries are legal through checked descriptors |
| Operands | Slot identity, use/definition role, early/late timing, legal register bank/resource-view set, register/memory alternatives, fixed register or symbolic ABI slot |
| Ties | Explicit relation between use/result operand slots; separate value IDs; validation of compatible widths/classes |
| Clobbers | Register-unit footprints and timing, including call/service/flag effects; preservation can depend on resource view/width |
| Entry/call/return bindings | Checked logical component-to-physical-location descriptions, including hidden destinations and indirect target operands |
| Memory/effects and references | Mandatory effects, ordered barriers, typed artifact dependencies and symbolic object uses |
| Bundles / late recipes | Atomic flag-sensitive sequences or bounded physical recipes; explicit scratch/effects, no hidden CFG/call/failure edge |

The structural record consists of ordered `OperandDesc` slots, `TieDesc` slot
pairs, `ClobberDesc` footprints/timing, mandatory effect/reference views and an
optional atomic/bounded-recipe descriptor. An operand constraint is a checked
resource-view set with register/memory permissions, a fixed resource view, or a
symbolic ABI slot. IDs resolve through the immutable selection context. A payload
cannot provide inconsistent duplicate slot numbers or arbitrary undeclared masks.

Timing order is early uses, early clobbers, early definitions, late uses, late
clobbers, late definitions. Ordinary inputs default to early use and results to
late definition. At a call, clobbers discard old values before result definitions
establish new ones, even when they share a resource. A tie describes reuse at
those operand points; it does not erase an input's later uses. LA03 must confirm
this ordering against its recipes, including early-clobber and indirect-target
cases, before its concrete target adapter is frozen. The future checker must
interpret the same ordering as the placer.

Resource descriptors model banks, views, overlapping units, reservations and
preserved footprints independently. Sharing a bank does not establish overlap;
different banks do not automatically exclude it. A fixed x86 byte view overlaps
its enclosing register; the frozen AArch64 partial-preservation witness can be
represented without declaring a whole wide value preserved. Scalar foundation
descriptors may reserve overlapping resources conservatively.

Flag-dependent compare/branch sequences initially use atomic target bundles.
The descriptor exposes every operand, clobber and effect; transfers cannot be
inserted inside the bundle. Hidden branches are forbidden: terminal bundles name
their actual graph edges. Explicit flag values can be a later reviewed extension;
ordinary `Bool` values cannot conceal a physical flags dependency.

LA02 validates the structural schema, references, value flow and descriptor
consistency with synthetic payloads. LA03 defines the real resource catalog,
opcode descriptions, legal timing/ties, ABI slot shapes and target verifier.
Selected publication requires both structural and target checks. Descriptions
are not proof that every real instruction effect/clobber was declared; native
assembler/ABI/adversarial recipe tests remain independent requirements.

Symbolic selected objects distinguish semantic objects, trace records and ABI
areas. Placement later adds separate value-home/spill/save/transfer-scratch
requirements. Physical offsets are absent. A future `CheckedPlacement` borrows
the exact immutable `VerifiedSelectedCallable` that it describes; locations are
per operand/program point, not one permanent register per value. Reordering or
editing that callable cannot leave a usable placement result. LA03 owns transfer
semantics/checking and realization; LA02 introduces no placeholder placer.

## Construction, editing, and publication

`LoweredCallableBuilder` reserves blocks/parameters, declares objects, defines
typed entry inputs, appends instructions/results and terminates blocks. Forward
block references are legal in a draft. Only publication can establish that every
reserved block/value/object/check is defined and consistent. Attempting to append
after termination or define a value twice is a construction error, not a panic.

The normal builder offers no opaque instruction escape hatch. Private malformed
fixtures may bypass checked append helpers to exercise the verifier's independent
checks. Selected builders offer the same structural operations over fresh selected
IDs and the target payload; they can introduce blocks and simultaneous edges.

Publication consumes draft state and returns either an immutable verified product
or ordered structured failures. Wrapper fields/constructors are private to their
verification owner; backend orchestration and inspection cannot forge a seal.
Published access is read-only, with no mutable dereference, interior-mutable payload
or public constructor. Complete-program seals also check finalized inventories
and completion receipts; a single valid callable is not a whole-program seal.

Editing consumes the verified product and returns a draft editor. Initial APIs
support operand replacement, edge redirection/argument replacement, block splitting
and explicit rebuild/remapping. Each edit updates all affected definition/check/
origin records before republishing. No downstream use of an old seal or cached
analysis survives mutation. Avoid implementing a generic pass manager, arbitrary
plugin mutation API or unused optimization pipeline.

CFG/reachability/dominance facts are computed once per verification session;
errors are emitted in canonical block/instruction/edge order. Stage-local analysis
views borrow their exact immutable callable. IDs or semantic origin equality are
insufficient to reuse results. No global analysis cache or clone of declaration
context per callable is needed.

## Verification and errors

| Check layer | Required rejection |
| --- | --- |
| Context/plan | Wrong live context, target/profile/trace mismatch, undeclared signature/layout/artifact, forbidden execution domain |
| Storage/definitions | Owner/range mismatch, unresolved reservation, duplicate definitions, result/definition/type disagreement, invalid entry/input/object disposition |
| CFG | Missing/multiple terminator, instruction after termination, absent or foreign target, entry predecessor, wrong edge occurrence/argument type or arity |
| Dataflow | Same-block use before definition, reachable nondominating use, predecessor use of successor-only parameter, invalid remap |
| Scalar operations | Illegal type/cast cell, noncanonical constant, wrong arithmetic/check relation, missing/bypassed/substituted success evidence |
| Memory/effects | Invalid width/alignment/provenance/reference, narrowed mandatory effect, missing service contract; known static access outside its permitted domain |
| Calls/traces | Signature/component/target disagreement, fictitious unit result, undeclared attribution/location/record, omitted-policy trace reference/action |
| Selected structure | Invalid resource/representation/ABI/recipe reference, malformed operand timing/tie/definition/clobber description, hidden target CFG or leftover lowered opcode |
| Program inventory | Unfinished worklist, unresolved required definition, conflicting artifact, illegal absent-body reference, cross-context completion receipt |

Known in-object constant accesses are checked against object extent/alignment;
unknown pointers do not imply statically verified bounds or alignment. Dynamic
address safety, initialization, lifecycle policy and selected semantic recipes
remain upstream/native-test obligations. The verifier must distinguish what it
checks structurally from what it inherits; it is not a second semantic verifier.

Private error records carry stage, stable reason code, target, machine callable,
semantic origin/span when available and block/instruction/operand/edge location.
Choose stable structured reasons rather than testing incidental prose alone.
Verification failure is a compiler invariant defect, never a runtime trap,
source error, optimization warning or silent fallback to legacy selection.
Target capability/size/encoding rejection remains separately categorized.

Retain the public `BackendError` shape initially: orchestration converts private
failures into its existing target/callable/message presentation and preserves
driver failure categories. A generated callable has its own private identity;
do not fake a semantic callable merely to fill the optional public field.
Origin may fill that field when valid, with generated identity in the rendered
message. A later public structured inspection/error extension requires its own
tests and living documentation update; it is not forced by private LIR storage.

## Inspection, dumps, and Rust ownership

Implement a canonical text renderer for verified lowered graphs and the synthetic
selected structural view. Header includes schema version, stage, target/profile
and canonical callable key. Print entry, ordered inputs/objects, blocks in stable
ID order, parameters, result IDs/types, opcodes, operands, edges, effects, guards,
attribution and artifact references. Print float bits and explicit memory widths;
never host pointers, context tokens, timings, unordered maps or mutable state.

Draft defect rendering is a separate best-effort facility with unresolved entries
marked; it cannot be mistaken for a verified checkpoint. Read-only visitors expose
the same phase-owned facts without returning a mutable model or MIR. No full JSON
schema, importer, persisted LIR cache or stable external wire format is required
for this workstream. Add structured export only with an actual maintained consumer.

Observation remains request-local under the
[reporting contract](../compiler/REPORTING.md). Proposed private stage identities
are planning, lowering, lowered verification, selection, selected verification,
placement/checking, realization/physical verification and artifact closure.
Observers can stream immutable checkpoints and requested-only counts. LA03/LA05
wire real execution/events and any public adapters; LA02 does not emit fabricated
phase events or add CLI stop/dump switches for a pipeline that does not execute.

Keep code private within `skald-compiler::backend`, organized by cohesive concern:

```text
backend/
  plan/       checked immutable fact views and declaration inventories
  graph/      stage-independent graph traversal/dominance and identity utilities
  lir/        lowered model, builder/editor, verifier, dump, local tests
  machine/    selected structural model/descriptions, verifier/view, local tests
```

This is an ownership sketch, not a mandate to create empty modules. `mod.rs`
files provide concise private facades and selective re-exports. Real target
payloads remain with their target owner; MIR expansion lives in a later `lower/`
owner. Shared graph/model/verification cannot import frontend state, MIR execution
enums or physical x86 types. Existing nominal identity and `source::Span` metadata
are permitted without source-database lookup. Extend maintained boundary checks
for these narrower scopes rather than adding scanner exceptions or a new checker.

## Worked acceptance cases

### Live input and a destructive target tie

Lowered inputs `a: I64`, `b: I64` define `sum = add a, b`; a later call still
uses `a`. Each has its own value ID, with no object/home allocation implied.
The synthetic x86-like descriptor ties the add's output to its first input
location while preserving different input/result IDs. A three-address descriptor
permits distinct locations. Both pass structural verification and dump distinctly.
LA03 checks that placement preserves `a` and rejects a corrupt tied assignment;
LA02 does not claim its descriptor tests prove physical preservation.

### Division diamond and join

An explicit nonzero-divisor check branches to failure or a success block defining
floor quotient/remainder from the secured operands. A join receives results as
block arguments, and later uses of the original operands remain visible. Negative
fixtures substitute a divisor reload, bypass the success edge, use a failure-only
definition or change an edge argument type. Synthetic selection adds correction
blocks and remaps every use; dangling old IDs or private-label joins fail.
Positive constant evidence and deliberately invalid constant/check evidence
exercise both bounded domain-discharge paths without borrowing MIR proofs.
LA03 supplies real `MIN / -1`, zero, signed-floor and fixed-register native recipes.

### Loop edges and simultaneous transfers

A header parameter `iteration` receives an entry value and a backedge's freshly
defined `next`; an edge may also swap two other parameters. Verify use positions,
types and simultaneous semantics, including two edges to one successor and a
critical edge. A semantic MIR loop local remains an object load/store unless a
separate promotion design proves otherwise. LA03 owns cycle-breaking transfers.

### Hidden destination, receiver triple, and mixed pressure

The logical signature contains destination, receiver static/complete/metadata
addresses, integer and floating arguments as separate roles. Scalar results are
empty for the aggregate return. A signature-typed indirect target is another use,
not an argument reloaded during marshalling. Fixtures exhaust synthetic integer
and floating banks independently and retain component roles for the frozen x86
and AArch64 ABI witnesses. LA03 chooses/checks real locations; LA04 carries the
existing aggregate-pressure native golden through full method/interface lowering.

### Shared release and trace omission

A synthetic release graph loads count/header metadata, branches through immortal,
ordinary and last-owner paths, calls the finalizer and frees the **original header
value** afterward. Opaque release nodes are impossible in the lowered schema.
Effects and use-after-call dataflow are inspectable without accessing MIR. Enabled
trace fixtures carry inherited helper attribution; omitted fixtures reject all
trace-only inventory/actions. These fixtures verify representation, while LA04
retains the private destructive-finalizer probe as independent native evidence.

### Resource views and target extensions

Synthetic catalogs cover overlapping byte/full views, three-address operations,
reserved resources, partial-width preservation and an invalid fixed/tied pair.
A target thunk extension can reference its parent declarations and be verified;
it cannot replace an existing signature or use a receipt from another context.
These are structural portability witnesses, not target registration, AArch64
assembly acceptance or an allocator-library choice.

## Validation, cost, and migration

Keep malformed model/builder/verifier fixtures colocated with their owners.
Public facade/privacy and cross-phase contracts use crate integration or
compile-fail tests; real source-to-observation behavior stays in existing feature
goldens. Reuse the handoff's named witnesses rather than copying backend runners
or introducing a second corpus. Add independent-process text-dump determinism,
same-input/different-live-context rejection, read-only/seal privacy and edit/reseal
fixtures. Synthetic targets are test-only and cannot emit unchecked production code.

The roadmap runs `make check` and `make msrv-check` for the new Rust targets/APIs,
with focused owner tests per task. Extended native/determinism/release gates become
mandatory when production behavior changes in later workstreams. Documentation
describes only implemented APIs/checkpoints as they land; drafting this proposal
requires link/index and whitespace checks, not a new full compiler benchmark.

The [foundation measurement procedure](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
and pre-migration record at `9e3cebb1` remain authoritative. All baseline inputs
are complete; eleven short compile timing gates remain inconclusive. The closing
stack-extraction guard changed the harness fingerprint: future old/new pairs must
recapture the preserved compiler and candidate with one compatible harness.
Do not edit historical records, grant cost clearance or require allocation to
make the foundation acceptable. LA02 should avoid per-instruction heap metadata
where typed vectors/tables suffice, repeated declaration clones and retaining all
phase bodies for dumps; actual foundation cost acceptance belongs to LA05.

Production continues through the current backend throughout LA02. Its implementation
must nevertheless leave useful verified models/consumers and exercised APIs,
not abandoned prototypes. LA03 introduces complete-callable pilots that require
the new path in tests, with explicit eligibility and no hidden fallback. Migration
bridges belong in the child/program ledger with introduction commit, removal owner
and disposition. Local commits between tasks do not discharge those obligations.
Each roadmap closes with a cumulative diff review; LA05 also reviews the whole
foundation from `495debd3` and removes legacy selection.

## Joint review and promotion checkpoint

The common contracts above are accepted and frozen. The table records their
required LA03 counterparts; acceptance does not claim that real target recipes,
ABI mappings or resource catalogs have been designed or verified. LI01 records
the common schema agreement against the inherited walkthroughs, and LI07/LI08
exercise it with synthetic targets before shared selected publication. LA03 must
settle and validate its concrete counterparts before their implementation; they
cannot be deferred to emission.

| Joint question | LA02 frozen contract | LA03 required agreement/evidence |
| --- | --- | --- |
| Logical shape versus physical ABI | Role-based checked signatures; shared entry/call/return shape; typed indirect target | Component-to-location plan, incoming/outgoing slot descriptors and resource timing for scalar, hidden-result, receiver/origin and mixed-pressure witnesses |
| Selected representation and operands | Fresh stage IDs; target representation/resource descriptor IDs; opcode-derived operand/tie/clobber view and specified early/late event ordering | Representative x86 payload shapes, width/canonicalization obligations, fixed/tied/early-clobber timing and complete call clobbers; matching synthetic three-address/overlap witness. Full target opcode schema is frozen in LA03 |
| Flag and edge constraints | Atomic flag bundles, explicit terminal edges, legal critical edges and edge occurrences | No insertion inside bundles; split/transfer policy with swap/cycle cases and preserved indirect targets |
| Inventories and publication | Immutable checked declaration catalog, finalized lower inventory and separate target extension; per-callable completion receipts | Concrete target request/freeze strategy and streaming ownership; thunks pass target verification without new mutable layout/signature facts |
| Symbolic objects and scratch | Distinct semantic/trace/ABI objects; placement-created requirements later; recipe/scratch descriptors, no offsets | ABI slot shapes, declared scratch lifetimes and bounded frame legalization contract sufficient for baseline placement/checking |
| Inspection and errors | Private read-only views, canonical text, stage/local structured defects; existing public facade retained | Selected/placement/physical dump integration and failure conversion without MIR queries, fabricated observations or generated semantic IDs |

The user's acceptance promotes the common model contract. Concrete LA03
algorithm/native decisions follow under its own design; they cannot enter shared
APIs as x86 defaults. If implementation review finds that a witness cannot be
represented, stop the affected dependent task, record the mismatch and amend
the owning design explicitly. Do not invent an unchecked adapter or weaken a seal
to pass the checkpoint. This applies equally to conflicts with the frozen phase
design.

The [LA02 roadmap](LOW_LEVEL_IR_MODEL_ROADMAP.md) orders context/IDs, lowered
construction, verification/publication, selected structural contracts,
editing/inspection and cumulative closure. Closure requires all model consumers
and negative fixtures, repository/toolchain gates, accurate indexes and an updated
program handoff. Native pilot, complete migration and allocation remain pending;
no measurement go/no-go can turn unimplemented phases into delivered architecture.

## Alternatives

| Alternative | Reason not selected |
| --- | --- |
| Reuse MIR values/storage or physical machine enum | Carries semantic homes/IDs or fixed physical choices across a different CFG and preserves the existing coupling |
| One enum with a stage flag | Makes forbidden payloads and unchecked phase transitions representable in every consumer |
| Mutable virtual registers or phi assignments | Conflicts with the frozen single-definition/block-argument contract and complicates independent value-flow checking |
| Fully general proof/effect/alias framework | Expands scope beyond the three guarded scalar domains and conservative ordering needed for migration |
| Shared enum of all target opcodes/registers | Couples target-independent storage/verification to every target's instruction set |
| Allocator-specific IR/dependency now | Lets a future algorithm determine foundation contracts before baseline placement exists |
| Public LIR importer/JSON schema/plugin API | Adds compatibility and mutation authority without a maintained consumer |

Sibling Niflheim's
`docs/BACKEND_IR_SPEC.md` informed deterministic callable-owned IDs, explicit CFG
and independent verifier fixtures. Its target-neutral, multiple-definition IR,
high-level object/array nodes and GC safety model are not copied: Skald's frozen
layout-specialized executable expansion and value/effect contracts are authoritative.

# Target Selection, Checked Placement, and Physical Realization Design Proposal

Status: accepted, frozen and promoted LA03 design, 2026-09-18. Accepted by the
user after review of the draft committed at `8834bcd6`.
Implementation: [native phase roadmap](TARGET_SELECTION_PHYSICAL_REALIZATION_ROADMAP.md),
in progress; streaming publication authority is implemented, native consumption
remains pending.
Source assessment: `f59d3fff`, the committed executable-model closing change.
Parent: [Low-Level Compiler Architecture](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
Inherited contracts: [frozen phase design](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md)
and [frozen model design](../archive/LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md).
Preparation: [completed-model handoff](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#common-model-readiness),
[target obligations](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#target-implementation-obligations)
and [retained artifacts](LOW_LEVEL_COMPILER_MIGRATION_COVERAGE.md#retained-model-artifacts-and-removal-owners).
Program implementation baseline remains `495debd3`. Record the child baseline
immediately before implementation; this document does not start implementation.

## Purpose and completion boundary

Make the new phase architecture execute real x86-64 programs. Establish concrete
target selection, operand-point placement, independent placement checking,
symbolic frames, bounded realization, physical verification and typed artifact
closure. A deterministic stack placer is the first client of these interfaces;
a future register allocator replaces that client without replacing selection,
checking or realization.

The workstream includes the narrow production-fact projection and shared
MIR-to-LIR construction necessary for a complete scalar/control-flow/call pilot.
A hand-built selected graph alone is insufficient. The pilot remains private and
explicitly required by tests; ordinary production emission stays on the existing
backend. Full language/helper migration belongs to LA04, default adoption and
legacy removal to LA05, and register allocation to LA06.

No semantic SSA conversion, local promotion, optimizing allocator, scheduling,
new runtime ABI, registered AArch64 backend, object-file writer, public LIR API
or new CLI migration switch is included. Synthetic portability tests remain
permanent test-only consumers.

## Present implementation and reuse boundaries

The completed model provides checked supplied facts, separate stage arenas,
closed lowered operations, shared graph checking, scalar-domain verification,
immutable callable/program receipts, consuming edits and deterministic inspection.
It does not yet project executable facts from final MIR or execute native phases.
Selected publication already requires both shared checks and a `TargetVerifier`;
these interfaces are the starting point, not prototypes to bypass.

| Existing owner | Reuse / change boundary |
| --- | --- |
| [Public backend facade](../../crates/skald-compiler/src/backend/mod.rs) | Keep verified final-MIR input, target registry, trace-source isolation and public errors unchanged |
| [x86 ABI](../../crates/skald-compiler/src/backend/x86_64_sysv/abi.rs) | Preserve current scalar and component ordering; introduce classification over checked logical signatures, without MIR types in the new target consumer |
| [Frame planning](../../crates/skald-compiler/src/backend/x86_64_sysv/frame.rs) | Preserve relevant layout/address behavior; new frames consume symbolic objects and placement requirements, never MIR storage/value IDs |
| [Current physical instructions](../../crates/skald-compiler/src/backend/x86_64_sysv/machine.rs) and [renderer](../../crates/skald-compiler/src/backend/x86_64_sysv/emit.rs) | Reuse small legal instruction-formatting rules where useful; new verified physical storage keeps typed references and explicit CFG, rather than accepting legacy assembly as proof |
| [Division](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/integer_division.rs), casts and call selectors | Use as behavioral references and native test oracles; do not invoke their MIR/frame-dependent selection from the new pipeline |
| Shared failure catalog and runtime ABI | Preserve exact messages, marker, services, layouts and trace contract; no new support service to conceal an incomplete recipe |

The old backend remains a separate implementation during the pilot. Extract only
leaf rules whose inputs already obey the new phase boundary. Do not build a
universal instruction enum, duplicate module runners or introduce frontend
queries into target selection, placement or realization.

## Accepted decisions

| Question | Accepted decision |
| --- | --- |
| Native target | Linux x86-64 SysV, ELF relocations and existing GNU Intel-syntax assembly interface |
| Selection | Concrete immutable x86 payloads over the implemented shared selected graph; all semantic correction CFG precedes publication |
| Placement | Strategy-independent operand-point assignments and explicit transfers; deterministic private value homes for baseline placement |
| Checking | Separate checker reconstructs requirements and value availability from the exact verified selected input; it does not trust a placer's success flag |
| Frames | Symbolic requirements first, checked offsets after placement; conservative semantic-object lifetimes, frame pointer and fixed outgoing area |
| Late recipes | Finite straight-line target expansions with declared scratch, clobbers and bounds; no hidden semantic CFG, helper or failure discovery |
| Streaming | Discovery pass, plan-bound target freeze and callable streaming; reconcile exact lower/selected/physical receipts at final closure, under the explicit model amendment below |
| Integration | Private explicit whole-program pilot entry; no per-callable fallback and no default production switch |
| Portability | Shared location/transfer/checking contracts parameterized by target facts; x86 physical instructions remain target-owned |

These refine inherited contracts. A mismatch with the frozen model requires an
explicit owning-design amendment before dependent implementation, not a permissive
verifier or an unchecked adapter.

## Pilot admission and shared lowering

Admission is a whole-program check against the physically retained executable
MIR, its certified domains and the requested policies. Produce either an admitted
pilot plan, an explicit unsupported-pilot reason, or an ordinary target/invariant
error. Test callers must require admission; an error after admission never invokes
legacy lowering. A future production selector is a separate adoption decision.

The source pilot includes payload primitives (`I64`, `U64`, `U8`, `Bool`, `F64`),
unit, receiverless function pointers and their scalar signatures; ordinary scalar
local memory, constants, callable addresses, scalar loads/stores, arithmetic,
comparisons, shifts, the complete primitive cast matrix, division/remainder,
direct/secured-indirect scalar calls, scalar C externals, jumps/branches/checks,
returns and intrinsic reported failures/hard traps. Include loops and joins;
semantic locals remain objects. Lowered code addresses carry signatures and
scalar checks secure the exact loaded operand that the success operation uses.

Admission excludes class/interface/object/optional/array/shared lifecycle,
receiver/aggregate/alias-origin source signatures, array/I/O/process intrinsics,
user string panic construction, active or referenced static fields and any other
helper family not implemented by the pilot. Inspect signature and place/rvalue
forms as well as instruction variants. Never silently drop an excluded retained
body, generated root or dependency to make the program admissible.

Projection may inspect final-MIR declarations, existing checked layout/dispatch
providers and narrow retention services. It freezes complete signature/layout
pools, executable body dispositions, service declarations and typed data facts.
Source lookup is confined to enabled trace metadata planning. Lowering uses that
immutable plan and final MIR; selection onward has no executable MIR query.
Proof-only MIR variants remain impossible at the sealed boundary and absent
from the lowered schema. Map every admitted form to the coverage inventory.

The process wrapper preserves the existing entry signature, runtime marker,
startup/normal-shutdown ordering and result behavior. With no statics, lifecycle
coordinators have explicit empty/absent dispositions consistent with the current
protocol; do not invent language entry parameters or bypass the marker.

Both enabled and omitted tracing are pilot requirements. Shared lowering emits
ordered trace actions and exact source/helper attribution. Selection expands TLS,
record accesses and updates into ordinary target operations with explicit effects,
values and scratch. Omitted tracing has no source lookup, trace declarations or
TLS dependency. Full lifecycle trace-path parity remains LA04.

Native phase fixtures separately exercise checked logical hidden destinations,
receiver/origin triples and seven-integer/nine-floating argument pressure. These
can use explicit lowered inputs and a C probe; they are not a claim that excluded
source-level class/interface construction has migrated. Existing aggregate and
shared-release source witnesses remain full-migration oracles.

## Declaration freeze and callable streaming

Planning inventories are immutable. Target selection cannot add signatures or
layouts to a sealed plan. The inherited model at `f59d3fff` made `TargetExtension` borrow a
finalized `VerifiedProgram`; selected drafts had to match its chosen receipts.
This is sound, but finalization requires every lower callable to complete before
any source selection can begin. Receipts do not retain bodies. A second lowering
pass creates fresh snapshots and cannot reuse first-pass authority. Keeping all
lowered bodies until selection would defeat the requested callable streaming.

**Accepted explicit model amendment (2026-09-18):** freeze target declarations against the
immutable checked plan and its permitted body/domain declarations, while deferring
exact complete-program receipt reconciliation to program closure. Retain genuine
per-callable verification and exact derivation witnesses. This changes when the
parent inventory must be complete, not what complete-program authority proves.
The [owning frozen model](../archive/LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#accepted-streaming-publication-amendment)
records this accepted amendment. The active roadmap implements it with a
plan-bound `TargetCatalog` and exact finalized-parent reconciliation at selected
program closure, preserving invariant coverage. Native orchestration remains pending.

The executable schedule is:

1. Project final-MIR facts and reserve canonical source/generated/data keys and
   all signature/layout pools. Collect target declarations with a discovery pass
   that lowers/verifies one callable at a time and releases its body. Its receipts
   are exploratory and discarded; they do not certify the executable second pass.
2. Freeze a target declaration catalog bound to the exact `CheckedPlan`, target,
   trace policy and allowed declaration domains. It cannot overwrite parent
   facts or authorize an absent semantic body. Discover all constant/thunk
   requirements with the same pure request rules used by selection.
3. Lower/verify one final callable; register its genuine receipt with the lower
   worklist. Select from that actual verified body and exact receipt against the
   frozen catalog; jointly verify, check placement and realize/verify physical
   code. Register each derivation before releasing the predecessor body.
4. Finalize the lower inventory. Selected-program finalization compares every
   collected source input receipt with that finalized inventory's chosen witness,
   closes frozen target thunks/data, and binds the exact finalized parent program.
   Physical-program finalization reconciles its selected inputs and dependencies
   with that selected program. Only complete closure authorizes a final artifact.

Catalog checking and completed-program checking have separate types and
owners. `SelectionContext` borrows the frozen catalog/resources; it grants no
complete-program seal. A source builder still requires an actual fully verified
lower input with the same plan/profile/owner. A selected receipt retains that
input witness even after its body is released. A finalization API must consume
worklist state and check exact inputs against the supplied finalized parent;
context equality or matching callable IDs cannot substitute for snapshot equality.
The final product borrows/retains that parent's authority. Future body replacement
consumes affected program authority and requires fresh downstream reconciliation.

A failed second pass, missing/late target request or replaced lower/selected input
leaves no whole-program seal or externally published partial assembly. Changing a
frozen catalog requires a new context and rerunning dependent selection, never
appending mutable declarations. Require tests that finalize the wrong parent,
replace a body after local completion, mismatch derivation receipts, omit a thunk
or attempt to emit before closure. Adapt the current tests as contracts change;
retain all their invariant coverage rather than merely deleting old rejection
cases. If implementation reveals an unrepresentable invariant, stop dependent
work and amend the owning design explicitly; do not hide all-program body
retention behind a pilot-only bound or vague spool.

Physical callable bodies may be rendered into a private temporary fragment store
once individually verified. Retain typed dependencies, derivation receipts and
canonical fragment keys for final closure/retention. Fragments are storage, not
input to a verifier or symbol parser. Publish only the final checked artifact;
Clean up failed stores and account for errors. This bounds resident executable
bodies while preserving the existing final `String` assembly result. No general
LIR serializer, importer, snapshot reconstruction or persisted cache is needed.

Target requests include relocation-bearing constants, literal/failure bytes and
any ABI thunk. Canonical typed keys and sorted sets determine identities; arrival
order and pointers never determine symbols. For the pilot, prefer direct
ABI-compatible calls and reject unnecessary thunk requests explicitly. Every
implemented thunk has a normal selected body and required target verification.
All referenced data/static/trace/service artifacts appear in receipts, including
static dependencies from memory effects without explicit opcode artifact operands.
Production request collection itself cannot ask selection to recover meaning
from MIR: shared lowering produces requests; target discovery examines verified
lowered operations through narrow immutable interfaces.

## Concrete selected x86 contracts

A target-owned opcode enum stores execution operands and immediates. Implement
`Payload`, `EditablePayload` and `InspectPayload` exhaustively. Descriptions derive
uses/definitions, constraints, ties, early/late events, clobbers, effects, typed
references, ABI bindings and flow from those fields. There is no independent
editable use/def list, assembly callback, mutable alias or source database.
Descriptions must be total for malformed drafts; target errors are structured.

| Selected family | Required visible contract |
| --- | --- |
| Integer/byte/boolean operations | Width/canonicalization, destructive use/result tie where required, legal register/memory alternatives and flag clobbers |
| Floating arithmetic/conversion | Exact binary64 semantics, integer/SIMD bank restrictions, explicit intermediate values and unordered comparison behavior |
| Addresses and memory | Pointer-width inputs, width/alignment, symbolic object/artifact/addend, legal addressing scales and memory effects |
| Division and shifts | Fixed resources and count constraints; explicit zero/overflow/floor correction branches and guarded conversions; no hidden labels |
| Calls and returns | Role-based incoming/outgoing/result binding, secured indirect target, complete caller clobbers, return behavior and retained attribution |
| Checks, branches, traps and tracing | Actual graph edges, atomic flag-sensitive bundles, explicit reporting calls/defensive traps and trace-state/TLS dependencies |

The proposed closed pilot payload vocabulary is below. Names are illustrative;
fields and legal cells are contracts. Ordered stored edges supply successors;
no payload contains arbitrary labels or raw symbol strings.

| Payload | Execution fields / native obligation |
| --- | --- |
| `Constant`, `CopyBits`, `ExtendBits`, `NarrowBits` | Explicit bit pattern or typed source/result; source/result widths and signed/zero extension; byte/boolean definitions meet their canonical form |
| `IntUnary`, `IntBinary` | Negate/complement and add/subtract/multiply/and/or/xor; width, source operands and fresh result; destructive timing/tie and flag footprint |
| `FloatUnary`, `FloatBinary` | Negate and add/subtract/multiply/divide on F64; explicit inputs/result and SIMD constraints; no fast-math permission |
| `CompareBool`, `BranchCompare` | Concrete signed/unsigned/float/pointer predicate, inputs and boolean result or explicit successors; compare/set or compare/branch atomic bundle, including unordered float handling |
| `SignExtendDividend`, `UnsignedDividendHigh`, `DividePair` | Explicit high/low dividend, divisor, quotient/remainder values; fixed RAX/RDX resources and nonaliasing divisor; overflow and floor corrections are separate graph operations |
| `Shift` | Left/arithmetic-right/logical-right, width, value/count/result; immediate or CL constraint; checked full count secured before any narrowing to a native count view |
| `ConvertScalar`, `ReinterpretBits` | Precisely enumerated native integer/F64 conversion or bit-copy cells; unsigned/range corrections use separate CFG/values, never a hidden high-level cast callback |
| `Address`, `Load`, `Store` | Explicit base/index/scale or symbolic object/artifact/addend; destination, memory width and mandatory effects; memory addressing legality checked independently |
| `Call`, `Return`, `Jump`, `HardTrap` | Direct typed artifact or separate secured indirect operand, logical signature/attribution, ordered ABI uses/results and complete clobbers; returns/jumps expose stored transfers |
| `TlsAddress` | Typed enabled-only TLS artifact and result; target relocation sequence with declared footprint; subsequent trace loads/stores remain explicit |

`UnsignedDividendHigh` defines the zero high half; it is not an implicit divide
side effect. Selecting byte arithmetic or boolean materialization may use bounded
straight-line canonicalization bundles, with the complete resource/flag footprint.
A reporting call followed by a trap may be an explicit terminal bundle with both
facts exposed. No opaque `Trace`, `Release`, semantic `Cast` or legacy fragment
survives selected publication. Native recipe review must establish concrete legal
cells and descriptor fields for these payloads before the enum is frozen.

Expand signed-floor correction and `MIN / -1`, unsigned-to/from-float correction
and other branching recipes into selected blocks and fresh IDs. Reconcile origins
explicitly. A selected guarded operation must retain enough concrete operands and
check/recipe association for the target verifier to reject substituted/bypassed
checks. Such checking and adversarial native tests establish different properties;
structural descriptions alone do not prove arithmetic equivalence.

Compare/branch and compare/materialized-boolean sequences use atomic bundles.
There are no ordinary virtual flags values initially. A bundle cannot conceal a
call, failure edge or temporary, and placement cannot insert inside it. Reserve
flag resources conservatively; a later explicit-flags design is separate.

The catalog covers legal GPR and SIMD resources, byte/full overlapping views,
stack/frame reservations and call-preserved/clobbered footprints. Exclude high
byte forms initially. Conservative whole-register overlap is acceptable; bank
membership is never an alias rule. Baseline scratch uses declared caller-saved
resources; future allocation is not permanently denied all such registers.
Recipe scratch and operand assignments must exclude simultaneous live conflicts,
including an indirect target during marshalling.

`TargetVerifier` independently matches concrete opcodes, expected descriptor
facts, fixed/tied legality, call signatures/clobbers and recipe/CFG relationships.
There is no accepting default or success based solely on calling `describe` twice.
Review immutable payload types and test deliberately corrupted descriptions and
recipes. Keep native assembler/C ABI probes as independent evidence.

## Logical components and target ABI plans

Classify checked `SignatureFact` components, not MIR types. One target ABI plan
supplies entry, call and return bindings. Preserve Skald's existing internal
component ordering and primitive-only external ABI. Hidden destinations,
receivers and alias origins retain their logical roles even when source pilot
admission excludes them.

For x86, preserve the existing six integer/eight floating argument banks,
independent exhaustion, component-ordered eight-byte stack slots, integer/SIMD
results and aligned outgoing areas. A logical hidden destination consumes the
existing first integer argument position. Do not silently adopt C aggregate
classification for Skald internal calls. Bind incoming, outgoing and result areas
separately; symbolic slot indices are not byte offsets.

Call marshalling is a simultaneous transfer problem after argument evaluation.
Secure the indirect target before any assignment can overwrite its resource.
Store call results to their required post-call locations before subsequent cleanup
or trace operations can clobber them. A nonreturning reporter has an explicit
call barrier and defensive trap, without an unwind path.

The existing supported subset and the maintained
[AMD64 psABI](https://gitlab.com/x86-psABIs/x86-64-ABI/-/blob/master/x86-64-ABI/low-level-sys-info.tex)
are distinct authorities: preserve Skald compatibility while checking platform
obligations. Freeze supported external cells and reject unsupported variadic,
vector or aggregate C forms rather than implementing a partial classifier.

## Placement result and baseline strategy

Proposed private `PlacementDraft` borrows the exact immutable
`VerifiedSelectedCallable` and selection context. It stores ordered assignments
at entry, block parameters and each descriptor operand event, plus explicit
transfers before/after instructions and at individual edge occurrences. Locations
are resource views, ABI slots or placement-owned symbolic value storage; semantic
objects remain a different ID domain. No permanent value-to-register requirement
enters the shared selected model.

Placement requirements distinguish value homes/spills, callee saves, transfer
scratch and ABI areas from semantic/trace objects. Each requirement has checked
representation, extent, alignment and lifetime/use bounds. A transfer names its
logical value and source/destination representations; bit-preserving float moves
are different from numerical conversions.

Baseline placement allocates distinct deterministic homes for values and block
parameters, without slot reuse, rematerialization or object-lifetime reuse. Entry
bindings are copied to homes. Instruction inputs load into legal declared
resources; tied outputs reuse the required location without overwriting an
input's home. Results return to homes. Calls marshal from homes, and loop edges
perform simultaneous parameter assignments. Homes preserve values across calls;
callee-preserved working registers are unnecessary for this strategy.

The interface nevertheless supports register locations across instructions,
splitting, spills and save requirements. Test the checker with small hand-built
register placements as well as the baseline producer; otherwise future allocator
support is only an unexercised claim. No optimizing register producer ships here.

Edge plans distinguish two edges to the same successor. Selection normalizes
multi-successor parameter-transfer edges into explicit forwarding blocks before
publication, so baseline transfers execute on the taken path after flag bundles.
Shared LIR still permits critical edges; the target policy is explicit. Rewrites
require fresh selected publication before placement. Test forwarding parameter
and origin maps, swaps and cycles, including mixed register/memory locations.

Resolve parallel assignments deterministically: emit destinations no remaining
source needs, and break cycles with a declared representation-compatible scratch
slot/resource. Memory-to-memory movement uses declared target scratch. Check the
resolved sequence against the original simultaneous assignment independently;
no emitter-only temporary register or undocumented stack push is allowed.

## Independent placement checking and authority

Only the checker creates private `CheckedPlacement`; realization cannot accept
a draft, placer's flag or list of moves. The seal borrows the exact selected
snapshot, so consuming edits cannot leave a usable placement. Recheck after any
change to assignments, transfers or requirements.

The checker reads the selected graph and target facts independently of producer
choice. It verifies complete/unique operand assignments, legal widths/banks/views,
reservations, ABI roles/slots, ties and event timing. It interprets transfer plans
and clobbers in the frozen event order, checking that every use sees its declared
virtual value and every definition establishes its result. Scratch cannot alias a
simultaneously required value; a live input must survive a destructive tie.

Value availability needs a finite CFG analysis, not a single straight-line scan.
Use abstract location contents with simultaneous edge-parameter rebinding and
conservative joins; account for loop epochs, kills, overlapping writes, calls
and width-dependent preservation. Do not reuse the placer's availability map as
proof. Check edge arguments before successor parameter identities replace them,
and reject missing/stale copies or a transfer on the wrong edge occurrence.

Freeze the concrete checker state/join/iteration rules with worked loops and
counterexamples before implementation. A shared transfer interpreter may define
move semantics; planning heuristics and checker acceptance remain separate.
Bound work by finite selected storage/location facts. No path sampling or native
execution substitutes for checking all reachable uses. Unreachable structure is
validated without permitting a transformation to exploit unknown dominance.

## Frames, bounded realization, and physical publication

`FrameRequirements` combines symbolic semantic/trace/ABI objects and checked
placement-created storage. `FramePlan` assigns deterministic checked offsets,
keeps incoming stack arguments separate, reserves one maximum outgoing call area,
preserves a frame pointer and uses no red zone or semantic-object slot reuse.
Callee-save requirements are supported and tested with manual placements, even
though the baseline producer uses caller-saved working resources. Save/restore
footprints obey width-sensitive target facts.

Target realization takes selected code, `CheckedPlacement` and its checked frame
plan. Resolve symbolic addresses and transfers, then expand only declared finite
straight-line recipes. All scratch/clobbers are accounted for before placement;
no late semantic calls, traps, branching corrections or extra allocation appear.
Deterministic edge forwarding and prologue/epilogue code have explicit provenance
and obligations; they are not arbitrary hidden selected CFG.

Preserve current x86 representability limits initially: checked frame sizes and
stack displacements must fit the supported profile. Test legal boundary offsets
and structured rejection beyond the limit. An unsupported size is a target error,
not wrapped arithmetic or fallback. Target recipes can materialize encodable
addresses with declared scratch where supported. The synthetic AArch64 profile
must exercise a narrow displacement requiring a bounded address-materialization
recipe; it must not inherit x86's displacement range as a shared rule.

Proposed `PhysicalDraft` stores concrete target instructions, ordered blocks,
explicit branches/calls, frame realization and typed relocation/artifact records.
It has no MIR ID, virtual operand, symbolic frame offset or opaque assembly text.
A target physical checker publishes `VerifiedPhysicalCallable` only after checking
instruction forms, register/width legality, displacement/immediate bounds, edge
closure, entry/return stack state, call alignment, save/restore obligations and
that expansion introduces only declared transfers/recipes and dependencies.
Check frame-state joins/loops rather than balancing a linear instruction listing.

Physical verification does not prove general instruction equivalence. Recipe
checks, transfer value-flow checking and native semantic oracles remain required.
Typed physical dependencies may conservatively retain selected references or add
frozen target data; they cannot create missing semantic bodies, extend trace
policy or replace declarations. Complete physical-program closure reconciles
per-callable receipts and data before deterministic assembly rendering. Reachable
artifact retention traverses typed code/data references, never assembly strings.
Legacy symbol parsing stays on the legacy path until its removal.

## Ownership, errors, and observation

Use cohesive private owners under the backend facade: shared final-MIR lowering,
shared placement model/checking/transfer interpretation, and target-owned x86
selection/resources/ABI, frame realization, physical verification and formatting.
These are responsibility boundaries, not empty modules or mandatory crates.
Shared checking consumes narrow target descriptors/transfer semantics without
matching x86 registers. No target phase can query source state to recover meaning.

Defects identify stage, target profile, machine callable, source origin where
available and block/instruction/operand/edge location. Keep capability/size
rejection separate from compiler invariant failure, and preserve public
`BackendError`/driver categories. Generated identities do not fabricate semantic
callables. Failure after admission is visible and terminal.

Extend phase-owned immutable inspection to placements, requirements, frames and
physical code. Private pilot tests consume actual checkpoints in execution order;
no fabricated phase events. Public reporting/CLI adapters and default-pipeline
observation parity remain LA05. Quiet execution does not collect whole-program
dumps or counts. Omitted tracing never acquires source access through inspection.

## Portability walkthroughs

| Witness | x86 pilot | Synthetic Linux AArch64 obligation |
| --- | --- | --- |
| Live add input after tied result and call | Destructive tie, preserved home, fixed call resources | Three-address result with different legal resource set |
| Division/shift/conversion | Explicit correction/check CFG and fixed resources | Different recipe/resource constraints; no x86 opcode in shared checker |
| Hidden result and mixed pressure | Existing internal destination/receiver role mapping, independent banks and stack overflow | Result destination in `x8`, distinct argument/result roles and platform reservations |
| Parallel transfers | Stack swaps, register cycles, critical-edge forwarding | Same checker/interpreter with different scratch constraints |
| Preservation and calls | GPR overlap and complete call clobbers; callee-save manual placement | Link-register preservation, reserved platform role and low-64-bit versus wider SIMD preservation |
| Frames and symbols/TLS | Checked supported displacement range, typed RIP-relative/TLS recipes | Narrow frame displacement legalization and different symbol/TLS realization rules |

The result-register, link-register and partial-preservation cases follow the
[official AAPCS64 contract](https://github.com/ARM-software/abi-aa/blob/main/aapcs64/aapcs64.rst).
They guide shared representation; no AArch64 assembler, runtime or registered
target is claimed. A target-specific opcode printer is not a portability test
unless the shared placement checker consumes its differing constraints.

## Validation and completion evidence

Reuse the handoff's native witnesses and existing test plumbing. Keep malformed
payload/placement/frame/physical cases colocated; public facade/phase dependency
contracts stay in crate integration tests, and source observations in goldens.

| Layer | Required positive and independent negative evidence |
| --- | --- |
| Admission/projection/lowering | Source pilot through verified lowered publication; every whitelist cell and representative excluded place/signature/helper; sparse bodies never reconstructed |
| Selection | Runtime-loaded scalar operands, signed floor/minimum/zero boundaries, shifts, every cast cell, NaNs/negative zero; corrupt descriptor, guard, call clobber and hidden CFG rejection |
| ABI/calls | C probes in both directions, integer/float bank exhaustion, stabilized indirect target, hidden destination/receiver phase fixtures, nonreturning reporters |
| Placement | Baseline and manual register placements, live destructive input, early clobber, overlapping views, partial preservation, call result timing; dropped/wrong-edge/cyclic copy and stale-snapshot rejection |
| Frame/physical | Alignment, stack-state joins/loops, callee saves, large-offset limits/recipes, invalid instruction/relocation and undeclared scratch rejection; actual assembler acceptance |
| Whole pilot | Explicit new-path success under default/minimum MIR optimization, enabled/omitted tracing, complete/reachable artifacts and quiet/observed private execution; independent expected results and subprocess determinism |

Direct synthetic scalar fixtures do not clear source lowering; legacy source
success does not clear new placement. Tests must prove the new entry was used,
with no fallback. Full aggregate/destructive-finalizer source parity remains
explicitly pending. Do not multiply every witness across every mode blindly;
record which dimensions each selection establishes and the remaining gaps.

Implementation runs focused owner checks, `make check` and serial
`make msrv-check`; native-path changes also require the applicable release and
full-determinism gates in `make check-long`. Final gates use an artifact-free
snapshot containing closing fixups. Golden runs in one checkout remain serial
until the indexed [artifact ownership discovery](LOW_LEVEL_COMPILER_ARCHITECTURE_DISCOVERIES.md)
is resolved.

The [foundation measurement protocol](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md)
and preserved compiler `9e3cebb1` stay authoritative. Pilot timings/code/frame
counts are exploratory; a scalar subset cannot qualify foundation adoption.
Do not change historical records or clear the eleven inconclusive timing gates.
LA05 recaptures compatible full-corpus old/new pairs and owns cost acceptance.

## Accepted decisions and implementation checkpoints

The streaming amendment is accepted and recorded in the owning frozen model.
Scope, ownership, pilot admission and the baseline strategy are frozen. Concrete
algorithm and instruction contracts are mandatory deliverables before their
consumers, rather than unresolved promotion decisions:

| Contract | Required checkpoint in the implementation roadmap |
| --- | --- |
| Streaming authority | Replace the finalized-parent construction schedule while retaining exact snapshot reconciliation and complete-program publication checks |
| Placement checker | Specify finite state, overlap/width kills, loop rebinding and joins; establish live-tie, call and parallel-copy counterexamples before implementing the checker |
| Native selected schema | Walk division, checked float, indirect call and both trace modes through actual opcodes, events, resources and ABI before implementing dependent recipes |
| Frame/physical verification | Record exact size/encoding limits, stack-state rules, scratch bounds and scaffolding provenance before realization |

A checkpoint passes only when its documented contract and executable witnesses
agree with this frozen design. On a mismatch, stop dependent work, record the
specific gap and explicitly amend the owning design or split a focused proposal.
Do not weaken checks, publish provisional seals or silently widen pilot admission.

Complete-mode inactive-static fallback on the legacy path also needs explicit
reconciliation before full migration: the model rejects inactive static accesses,
including unreachable blocks. The source pilot excludes statics, so it cannot
settle that contract by promoting fields or dropping bodies. LA04 must decide the
owning construction/domain treatment and amend an inherited contract if needed.

The roadmap must track committed lint allowances inherited from the handoff,
new pilot entry/admission gates, extracted legacy adapters and any exploratory
storage. Remove allowances per actual consumer, with remaining full-surface
obligations transferred to LA04. The private pilot entry retires or becomes the
sole production entry at LA05; no permanent alternative backend route remains.

Closure reviews the cumulative child diff and relevant foundation history,
including manually committed earlier tasks. It verifies all seals and negative
witnesses, removes expired scaffolding, updates coverage with actual native
readiness and leaves full migration/adoption/allocation pending. Commit ownership
stays with the user.

## Alternatives

| Alternative | Reason not proposed |
| --- | --- |
| Run legacy MIR selectors from realization | Preserves phase coupling and hides register/CFG requirements from checking |
| Add allocation to current physical instruction stream | Loses virtual value flow and lacks reusable target-independent placement contracts |
| Implement an allocator first | Makes an unfinished physical foundation depend on an optimization algorithm |
| Encode stack homes in selected opcodes | Prevents per-point register assignment and couples selection to the baseline strategy |
| Let realization discover scratch or correction CFG | Invalidates checked placement and makes recipes impossible to audit beforehand |
| Keep all predecessor bodies by default | Contradicts bounded callable processing; ownership needs a deliberate reviewed solution |
| Add a public CLI selector now | Creates rollout/compatibility surface before a complete new backend exists |

Sibling Niflheim's explicit backend IR boundary informs the ownership split.
Its multiple-definition register model and GC-oriented semantics do not replace
Skald's single-definition, explicit lifecycle and independently sealed stages.

## Accepted numeric reporter effect clarification

Numeric selection consumes the existing shared reporting terminals before general
call/trace selection. Their native atomic recipe is the canonical panic call and
defensive UD2, with exact message/length/signature/attribution and full caller
clobbers. It retains the runtime service's mandatory unknown-read, call, report,
trace-state and hard-trap effects. No numeric correction hides in this recipe.

The shared selected effect checker distinguishes a call trace-state barrier from
an explicit caller TLS operation. Omitted mode does not request source/TLS
artifacts, but a call still preserves its callee's possible trace observation.
Non-call trace-state effects continue to require enabled tracing and TLS
references. This closes a checking gap without introducing a new representation,
public interface or tracing policy; general native call/trace recipes remain due.

## Accepted trace-fact retention clarification

Native trace verification must survive callable streaming. Lower completion
receipts therefore retain the immutable trace plan and site-to-location
associations from verified lower actions. They retain no executable body or
source database. Native checking resolves the record through selected object
origins and compares initial/replacement locations and call attribution against
these frozen facts. Consuming edits retain exact snapshot authority and remap
selected origins; lower republication derives fresh trace facts. This clarifies
how independent checking observes the already accepted trace obligations after
lower body release; it grants no placement or emission authority.

# x86 Native Resource and Component ABI Contracts

Status: immutable target resource facts and checked-signature component
classification and the complete pilot selected vocabulary, including calls and
trace memory operations, and independent placement checking are implemented
privately. Deterministic baseline placement marshals operands and ABI values
through independently checked explicit transfers. Symbolic frame planning,
typed physical realization, independent verification and exact final program
closure are implemented in the private pilot. Production still uses the
[existing backend](BACKEND.md).
Shared [placement drafts and checking](PLACEMENT_CHECKING.md) enforce exact-input
binding, target constraints, preservation and finite CFG availability.
These contracts refine the [shared low-level model](LOW_LEVEL_IR.md) and
[frozen target design](../archive/TARGET_SELECTION_PHYSICAL_REALIZATION_DESIGN_PROPOSAL.md).

## Resource authority

`backend::x86_64_sysv::native` owns facts without MIR types. `NativeResources`
constructs a checked catalog and exposes immutable queries. It describes all
sixteen GPRs at low 8/16/32/64-bit widths and sixteen SIMD registers at 64/128-bit
widths. SIMD widths do not admit vector source operations or external signatures.
Each physical register has one conservative overlap unit. Different banks are
not an alias rule: actual units determine overlap.

Every write kills the old whole-register token conservatively. Hardware 32-bit
GPR writes zero the upper half; 8/16-bit writes do not establish a fresh full-width
value. Selected canonicalization must expose extension when a full-width value
is needed. The whole-register model deliberately makes no partial preservation
promise. This is compatible with SysV's lack of call-preserved SIMD registers;
shared checking must retain its independent partial-preservation portability tests.

RSP and RBP views are reserved at every width, including aliases. RAX, RCX, RDX,
RSI, RDI and R8–R11 plus all SIMD units are caller clobbers. RBX, R12–R15 and the
reserved stack/frame units are preserved across a returning call; frame planning
still saves/restores any used allocatable preserved register. A call's stack
movement and alignment are separate physical-state obligations.

Flags have a clobber-only unit, no value bank/view and no virtual definitions.
Calls clobber it. Flag-consuming compare/set or compare/branch sequences are
atomic bundles. High-byte AH/BH/CH/DH forms are absent. Low-byte SPL/BPL/SIL/DIL
require REX; extended GPRs require REX at every width and 64-bit operations require
REX.W. Width and concrete instruction encoding checks remain mandatory later.

## Component classification

Classification looks up a signature through its checked plan and rejects a
non-x86/SysV target. Bindings retain the signature's original component order;
physical assignment follows logical roles: hidden result destination, receiver
static/complete/metadata, then parameters by index with each alias address followed
by complete/metadata origins. Runtime parameters use their logical index. This
preserves the existing native ABI even if checked declarations list roles in a
different order. Components keep their roles after assignment.

Six integer argument positions are RDI, RSI, RDX, RCX, R8, R9; eight floating
positions are XMM0–XMM7. Banks exhaust independently. Spilled components receive
successive eight-byte symbolic slots in physical component order. Incoming and
outgoing bindings name different areas using the same slot indices, never RBP
or RSP byte offsets. The outgoing area rounds to sixteen bytes and rejects
arithmetic overflow or a rounded size greater than the existing signed 32-bit
frame/address limit. [Checked frame planning](FRAME_PLANNING.md) validates complete frame size,
alignment and address displacement bounds over exact checked placement.

Scalar integer/address results use the appropriate RAX view; binary64 uses XMM0.
Unit/aggregate/nonreturning signatures have no logical scalar result. Internal
aggregate return uses its hidden destination, without introducing a C aggregate
classification or a new pointer-return convention. Nonreturning calls retain
explicit call effects and a defensive trap in selection.

External C signatures accept only value parameters/results of I64, U64, U8,
Bool and F64, plus a unit result. Receivers, aliases, aggregate results, raw/code
address cells and nonreturning source external declarations reject explicitly.
Runtime signatures remain distinct and support their checked address/service
components. Variadic classification rejects explicitly for every convention;
there is no implied partial varargs support or vector argument-count protocol.
Narrow byte/boolean bindings do not certify canonical upper register contents;
marshaling and external bool-result normalization remain selection obligations.

## Ordinary scalar selection

The native selected owner exposes immutable opcode and origin queries. Concrete
fields derive ordered early uses and late definitions, legal register choices,
fixed return bindings, ties and mandatory effects/references. Ordinary choices
exclude RSP/RBP and have no memory alternative or value stack home. Placement
must separately preserve any used callee-saved registers.

Integer arithmetic and byte offsets tie the first input to the result and declare
late flags; complement preserves flags. Scalar floating arithmetic uses tied SIMD
operands. Integer comparisons, boolean negation and branch testing consume flags
inside atomic bundles. Binary64 comparisons use UCOMISD: equality, less and less
or equal explicitly exclude unordered; inequality includes unordered. Greater and
greater or equal use above predicates, which already exclude unordered.

| Bounded recipe | Maximum steps | Explicit scratch |
| --- | --- | --- |
| Raw binary64 constant: integer immediate then MOVQ | 2 | One 64-bit GPR |
| Binary64 sign flip: MOVQ, sign-mask immediate, XOR, MOVQ | 4 | Two 64-bit GPRs; late flags |
| Byte multiply: two zero extensions, 32-bit IMUL, low-byte move | 4 | Two 32-bit GPRs; late flags |
| Parity-sensitive comparison: UCOMISD, two SETcc, byte combination | 4 | One byte GPR; late flags |

Each branch edge carrying parameters gets its own empty-parameter forwarding
block with one outgoing jump carrying the original arguments. Parallel edge
occurrences remain distinct. Transfers cannot occur inside the atomic branch.
Recipe origins retain lower block, instruction ordinal or edge slot and source
span independently of selected block relocation.

Mandatory native verification checks actual fields and cached effects/references,
not agreement between two descriptions. It requires the canonical x86 catalog,
eight-byte little-endian pointer layout, exact entry/return classification,
signature-qualified code addresses and aligned, bounded object accesses proven
from concrete address definitions and incoming edges. Lifetime markers retain a
finite site count. Invalid representations, slots and recipe metadata yield
errors; describing malformed opcodes remains total. Static memory provenance,
pointer conversion/scaled-address recipes outside the pilot, and ABI thunks
reject explicitly. Production continues through the existing backend.

Incoming/outgoing/result slot shapes belong to each checked signature within one
selection context. The same slot index can therefore represent integer bits in
one boundary and binary64 in another without weakening validation or receipt
authority. Symbolic ABI-area objects also name their signature.

## Native opcode and event walkthroughs

The shared event order is early uses, early clobbers, early definitions, late uses,
late clobbers, late definitions. Transfers cannot be inserted inside an atomic
bundle. Concrete scalar, numeric, call and trace payloads implement the contracts
below, including the reporter terminal used by numeric failures.
Schema tests do not certify executable native parity.

| Recipe | Required operands/resources/events | Independent rejection witness |
| --- | --- | --- |
| Signed dividend setup | RAX64 input, RDX64 high-half definition; `cqo` preserves flags, high half is an explicit value | Wrong bank/width/fixed register or undeclared high-half result |
| Unsigned dividend setup | RAX64 input and explicit RDX64 zero result via flag-preserving MOV; an XOR zero idiom would require a different declared flag footprint | Implicit high half, wrong zero-extension footprint or undeclared flags |
| Signed/unsigned divide | Low/high/divisor late uses; low RAX64, high RDX64; quotient RAX64 and remainder RDX64 late definitions; flag clobber after uses; divisor excludes both overlapping units | Divisor alias, wrong fixed/tied binding, missing overflow/zero guards or stale live dividend |
| Variable shift | Validated full-width count narrows explicitly to CL8; value and CL are late uses, result is a tied late definition; flags clobber after uses | Narrowing before guard, count assigned outside CL, value/count overlap with different tokens, lost live tied input |
| Integer destructive operation | Inputs consumed before tied result definition, declared flag footprint and legal width; other live input cannot be destroyed | Omitted tie or preserving a live input only in an overwritten location |
| Checked F64/integer conversion | Concrete conversion cells expose banks, intermediate values and correction CFG; unsigned range/NaN/overflow checks precede conversion; no flags cross a bundle boundary | High-level cast hiding branches, wrong conversion width/cell or missing correction provenance |
| Direct call | Component ABI operands are late uses; all caller units/flags clobber after uses; scalar results are late definitions | Missing call clobber, wrong slot area, result stored only after clobbering cleanup |
| Indirect call | Separate code-address operand with exact logical signature is a late use; its location survives all preceding simultaneous marshaling/scratch transfers | Target overwritten by an argument or temporary, signature mismatch, hidden target operand |
| Enabled trace actions | Explicit TLS address and trace loads/stores/values, ordered attribution and effects; scratch/flags are declared; call-free local-exec ELF TLS recipe | Hidden trace helper call, undeclared TLS/scratch, location update after call or reporter, result lost during pop |
| Omitted tracing | No source lookup, trace/TLS declaration, location update or hidden frame action | Generated trace metadata request or explicit TLS access in omitted mode |
| Reporter/trap | Reporting call uses ordinary ABI/clobbers and exact failure attribution, then explicit defensive hard trap; no unwind edge | Reporter without call barrier/trap or unrecorded failure edge |

Division overflow/floor correction, checked float ranges and full-width count
validation are shared/selection CFG, never realization repairs. Calls evaluate
arguments before simultaneous ABI transfers; an indirect target stays available
through that entire transfer sequence. Trace pop follows result preservation;
failure locations update only on the failure path immediately before reporting.
A source frame has two pointer-width words (previous frame and current location);
its initial definition location is an explicit frozen lower fact. The generated
entry has no source frame and calls the ABI marker, optional initializer,
language main and optional finalizer in order, preserving main's result across
normal shutdown. With no lifecycle work, coordinators remain absent. Trace context records contain
name pointer/length and path pointer/length; location records contain context
pointer, line and column. These data are materialized during shared lower closure.
TLS relocation and its finite scratch/address recipe must be fully described
before physical realization; unsupported relocation forms reject rather than
calling a resolver implicitly.

Private [physical drafts](PHYSICAL_REALIZATION.md) implement these recipes.
Independent physical verification must discharge the remaining movement and
encoding requirements against actual immutable opcodes and independent
verification. A mismatch requires an explicit contract amendment before
consumers, not a permissive descriptor.

## Numeric correction walkthrough

For signed 64-bit division, the shared guard establishes a nonzero divisor and
the closed division descriptor specifies the minimum/-1 disposition; selection
handles that overflow case in explicit CFG
before `cqo`/`idiv`. The ordinary path has explicit RAX/RDX dividend inputs and
quotient/remainder results. Floor correction uses the nonzero remainder and sign
relationship in separate blocks, then merges fresh corrected values. Unsigned
`div` consumes an explicitly zero high half. Byte division widens to the declared
64-bit recipe and narrows/canonicalizes its explicit result; it never uses AH.

A variable shift validates the entire original count against the semantic width
before narrowing its unsigned 64-bit count to CL. SHL, SAR and SHR correspond
to left, signed-right and unsigned-right cells. A narrow native count never
substitutes for that proof, and RCX/CL overlap cannot hold different simultaneously
required value tokens.

F64-to-I64 uses `cvttsd2si` only after explicit finite/range checks for
[-2^63, 2^63). F64-to-U64 checks (-1, 2^64), branches at 2^63 and uses the signed
conversion either directly or after subtracting 2^63, restoring the high bit in
an explicit integer operation. F64-to-byte checks (-1, 256) before explicit narrowing; negative finite
fractions truncate to unsigned zero in both unsigned cells; boolean conversion follows its specified predicate/canonicalization,
never an unchecked integer-conversion shortcut. Signed integer-to-F64 uses
`cvtsi2sd`; unsigned values above signed range require the explicit half/low-bit
rounding recipe and doubling, or an equivalently validated explicit correction
CFG. MOVQ bit transfers between integer/SIMD banks are reinterpretation cells,
not numeric conversion. Each concrete recipe declares actual flags, intermediates,
constants and edges; unordered UCOMISD checks consume flags inside their atomic
comparison bundles. Native probes must later verify range endpoints, NaNs,
infinities and unsigned rounding independently of descriptor checking.

## Concrete numeric publication

Selected numeric cells expose COPY, byte zero extension and narrowing,
CVTSI2SD/CVTTSD2SI at signed 64-bit width, bank-crossing raw-bit MOVQ,
CQO or a flag-preserving explicit RDX64 zero move, DIV/IDIV pairs, and
SHL/SAR/SHR with CL8 or a fixed logical shift by one. Each cell has explicit
virtual inputs/results and register constraints. Correction arithmetic uses the
ordinary tied ALU cells. These cells require no implicit scratch or helper calls.

Selection expands each semantic diamond before publishing: signed division's
MIN/-1 branch and floor correction; unsigned integer-to-float rounding via
`((n >> 1) | (n & 1))` and multiplication by two; and unsigned float-to-integer
subtraction/high-bit restoration. Join parameters carry the final result. A
zero-code association marker records the original numeric operation and its
secured inputs/result. It is durable verification metadata, not a value operand
or placement event. Independent checking reconstructs actual definitions,
constants and protected branch arms instead of trusting that marker.

A check branch records its full-width source relation and tests a concretely
computed boolean. Verification requires its success edge to protect the raw cell
and setup/narrowing, validates exact constant evidence when there is no dynamic
check, and checks its failure path's reporting disposition. Signed divide also
requires the MIN/-1 exclusion before IDIV. Substituting the divisor, narrowed
count, conversion source, correction constant or merged result fails publication.
Consuming rebuilds remap metadata and operands; splits preserve lower origins
and relocate concrete overflow branches.

Numeric failure terminals use the canonical nonreturning panic signature, exact
message symbol/length and source attribution, late SysV argument uses and all
caller-unit/flag clobbers. The atomic recipe is exactly a direct reporter call
followed by UD2, without a hidden successor or scratch. It retains call, unknown
memory read, report, trace-observation and hard-trap effects. A call's trace-state
barrier describes callee observation; it does not request a generated TLS access.
Explicit caller trace operations still require enabled policy and TLS authority.
General calls expose the same ABI events and trace barrier rules.

Owner tests interpret concrete selected cells independently and compare against
primitive conversion and arithmetic references, including every cast cell,
NaN payloads, range cut points, full-count guards, floor correction and overflow.
This certifies selected recipe relationships; actual native placement, realization
and subprocess execution are still required before claiming native parity.

## Concrete call and trace publication

Calls store their typed direct target or separate indirect value, signature,
ordered component arguments/results, canonical ABI bindings and attribution.
Arguments and secured indirect targets are late uses; all caller units and flags
clobber next, followed by late result definitions. Indirect targets use R11;
placement must secure that value before simultaneous ABI transfers, exclude it
from transfer scratch, and preserve live inputs. Call result definitions precede
trace cleanup; placement captures them before subsequent cells. Entry uses the
same selector and preserves the marker/main protocol. Nonreturning calls are
atomic call/UD2 terminals; standalone hard traps have explicit terminal effects.

Enabled source frames use one sixteen-byte object aligned to sixteen bytes.
Push stores the previous TLS head and initial location, then publishes the record.
Location replacement stores to the second word immediately before its attributed
call; pop loads the previous head and restores TLS immediately before return.
`TlsAddress` expands to a call-free local-exec ELF sequence: load FS:0 into the
result GPR, then LEA that GPR with the typed TLS TPOFF relocation. Its bounded
recipe has two steps, no scratch and no flag clobber; reading FS:0 has an
explicit unknown-memory read effect. Trace loads/stores expose
pointer-width memory effects, trace-state effects and the TLS dependency.

The verifier reconstructs these memory sequences, operand relationships and CFG
frame balance. It compares records, initial locations and allowed replacements
with immutable parent trace facts retained in the lower completion receipt.
Each call/reporter location must match its frozen lower operation site;
consuming rebuilds resolve the record through remapped object origins. Omitted
tracing selects no TLS cells or source metadata. Callee trace-observation barriers
remain mandatory on calls under either policy.

Pure request collection reads the authenticated lower receipt. Immediate integer
and binary64 constants need no additional data artifact; failure/source bytes use
canonical shared keys. Selection requires frozen catalog authority and rejects
any concrete reference outside its discovered set. The ABI-compatible pilot
requires no thunks and rejects every thunk form explicitly. Whole-program selected
closure reconciles exact lower receipts after callable bodies are released.
These witnesses establish selected publication; physical marshaling, TLS encoding
and native execution are validated by their subsequent owners.

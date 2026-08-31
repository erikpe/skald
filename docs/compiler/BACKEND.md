# Backend and Target Contract

Status: authoritative for the current backend interface, supported target
registry, target legality, x86-64 System V realization, and generated assembly
boundary. Explicitly marked frozen additions define selected future target
boundaries without claiming current backend support.
Source-visible language semantics remain owned by the
[language documentation](../language/README.md); the runtime C interface is a
separate contract. The shared-handle/header layout, generated
reference-counting realization, and executable shared-field layout are owned by the
[shared-ownership compiler and runtime contract](SHARED_OWNERSHIP.md), not by
the current target profile below.
The [optional-values compiler contract](OPTIONAL_VALUES.md) separately owns
optional layout, ABI, guard, and trap realization. Verified primitive and
exact-class optional local, field, parameter, result, and temporary MIR is
legal backend input, including checked class payload views and optional shared
owners. Inline optional-container aliases are indirect optional places with
one address component and no object-origin metadata.
The [standard I/O compiler and runtime contract](IO.md) separately owns the
implemented byte-array operations and runtime-call boundary, including current
x86-64 backend input.

The implemented [function-value compiler contract](FUNCTION_VALUES.md) selects one
non-null eight-byte code pointer in the System V integer class, exact
position-independent symbol addresses, and receiverless register-indirect
calls through the complete internal ABI. Function values now reach this
boundary as verified target-independent MIR. The x86-64 backend lays out and
copies them as neutral scalars, materializes exact callable symbols, and loads
stabilized callees into `r11` only after hidden results, arguments, and trace
attribution have been prepared.

Verified eager static lifecycle MIR is current backend input. The x86-64
target emits one aligned, writable, target-private `.bss` slot per canonical
declaration and addresses it with identity-derived RIP-relative relocations.
It lowers explicit initializer bodies through ordinary frame planning,
instruction selection, calls, allocation, ownership, arrays, and cleanup, then
invokes them from one private program initializer in the verified activation
order. It lowers verified destruction regions through the same lifecycle
helpers and invokes one private program finalizer in exact reverse order after
normal entry return. Static slots add no object-layout, dispatch, callable-
ABI, external-ABI, or runtime-marker rule. Their source lifetime is owned by the
[language contract](../language/STATIC_FIELDS.md#initialization-and-lifetime).

## Backend interface and target registry

Backends consume an explicit `BackendInput`: an opaque, read-only
`VerifiedFinalMirProgram`, source lookup only when tracing is enabled, and a
typed runtime-trace policy. Only `passes::run_mir_pipeline` and
`passes::verify_final_mir` construct that sealed product after ordinary and
static-lifecycle realization verification and target-independent reachability
analysis. The product owns the reachability facts derived from its exact MIR,
although backend lowering does not consume them yet. The backend cannot accept
raw `MirProgram` and does not repeat target-independent verification. This
boundary does not expose AST, resolved IR, HIR, or type-checker state to a
backend. The public backend facade provides:

- `backend::Target`, the selected target identity;
- `backend::target_by_name`, which validates a user-facing target name;
- `backend::SUPPORTED_TARGET_NAMES` and `DEFAULT_TARGET_NAME`;
- `backend::BackendInput` and `backend::RuntimeTracePolicy`, whose constructors
  make source access unavailable on the omitted path; and
- `backend::emit_assembly`, which dispatches verified MIR to the selected
  target and returns textual assembly or a structured `BackendError`.

`x86_64-sysv` is the only supported target name and the default. It selects a
Linux x86-64 backend using the System V AMD64 ABI and GNU assembler text in
Intel syntax with `noprefix`. Other names are rejected rather than silently
selecting a fallback.

Target implementations remain private behind this facade. Adding a target
requires an explicit registry entry and a complete legality, layout, ABI,
lowering, emission, and test boundary; target conditionals do not belong in
frontend or target-independent IR code.

## Input and legality boundary

The x86-64 backend performs these steps in order:

1. reject verified MIR features not yet implemented by this target;
2. compute deterministic class dispatch tables from verified virtual families,
   interfaces, requirements, conformances, and classes;
3. compute checked primitive and class layouts;
4. check that every executable signature and called member can be represented
   by the target calling convention;
5. plan fixed stack frames and target addresses;
6. select target instructions into a private assembly model; and
7. emit deterministic GNU assembly text in Intel syntax with `noprefix`.

Raw or malformed MIR cannot construct `BackendInput`. Target-specific
failures—including recursive layout,
unrepresentable sizes, missing callable bodies needed by target lowering,
argument-area limits, frame limits, and displacement limits—also return
`BackendError`. An error identifies its target and, when applicable, the
callable being lowered.

The target accepts verified static single inheritance, base projections,
owning slices, and class/interface/`Obj` alias views from existing places or
caller-owned produced exact-class temporaries, plus virtual-family and
interface calls. Runtime class/interface tests compare the forwarded dynamic
class metadata identity against the verified target set. Checked object casts
use the same check, materialize a successful full-expression view, and report
the cast failure on the verified failure edge. Shared-owner casts use the same
metadata membership test, retain or transfer the source handle on success,
preserve the existing allocation header, and never call the allocator.

Verified primitive comparison and logical-negation rvalues lower inline. The
selector compares canonical full-register operands, chooses signed conditions
for `i64` and unsigned conditions for `u64` and `u8`, and uses equality
conditions for `bool`. Logical negation tests one canonical boolean operand.
Every condition is materialized into one byte and zero-extended before storing
a canonical `bool`. These operations add no target labels, runtime calls, or
ABI surface.

Verified primitive integer cast rvalues lower inline through the canonical
scalar load/store boundary. Same-width casts preserve all 64 bits, widening
from canonical `u8` zero-extends, and narrowing retains and canonicalizes the
low byte. The same representation crosses locals, fields, calls, returns,
temporaries, and later comparisons. Casts add no target labels, traps, runtime
calls, allocations, symbols, or ABI surface; malformed cast MIR remains a
verifier-boundary error.

Verified pure integer bitwise rvalues also lower inline. Complement, AND, OR,
and XOR preserve the complete `i64` or `u64` bit pattern and canonicalize the
low byte for `u8`; they add no labels, failure edge, runtime call, symbol, or
ABI surface. The accepted source path uses this same representation across
ordinary scalar consumers without a bitwise-specific calling convention.

Verified checked shifts lower from their explicit MIR diamond. The check path
loads the `u64` count, compares it unsigned against 64 or 8, and branches
before any target shift or load into the variable-count register. Only the
verified success block loads the count into `rcx` and emits `shl rax, cl`,
`sar rax, cl`, or `shr rax, cl`; `u8` results immediately pass through the
ordinary low-byte canonicalization boundary. The failure block selects the
exact `shift count out of range` static message through `ska_rt_panic`.
Existing message symbols keep their indices and the new message is appended at
target-private pool index 9. This adds no public symbol, calling convention,
frame category, or runtime ABI entry point. Source `<<` and `>>` select this
same verified representation through the ordinary frontend pipeline.

Verified checked integer division and remainder also lower from their explicit
MIR diamond. The check block branches on the secured divisor before the
success block places the dividend in `rax`, clears or sign-extends `rdx`, and
places the divisor in `rcx`. Unsigned operations use `div`; byte results pass
through the ordinary low-byte canonicalization boundary. Signed operations
guard `i64::MIN` with divisor `-1` before `idiv`, synthesize the defined result
for that pair, and otherwise correct a nonzero truncating remainder whose sign
differs from the divisor. The correction decrements the quotient and adds the
divisor to the remainder, producing floor quotient and divisor-sign remainder.
Failure selects the exact operation-specific static message. Existing message
symbols retain indices 0 through 9; division and remainder append indices 10
and 11. This adds no public symbol, calling convention, frame category, or
runtime ABI entry point. Source `/` and `%` select these operations through
the ordinary frontend pipeline.

Source-selected, verified binary64 division rvalues use the existing
two-register floating scalar path and emit `divsd` with source operand order
preserved. Floating zero, infinity, subnormal values, underflow, overflow, and
NaN remain ordinary binary64 outcomes: this path emits no zero guard, panic
message, runtime call, or additional control-flow block.

Verified floating comparisons load their exact operands in source order and
use scalar unordered binary64 comparison. Instruction selection
forms every predicate from the relation flags plus an explicit parity gate:
equality requires ordered-and-equal, inequality accepts unordered-or-not-equal,
and each ordering predicate requires ordered plus its requested relation. The
result is zero-extended into the ordinary canonical `bool` representation.
This path adds no semantic branch, runtime call, failure edge, or ABI rule.
Direct MIR fixtures and source-native goldens exercise the complete predicate
matrix, including NaN in either operand position, signed zero, and infinities.

Producer invariants established by the central final-MIR verifier may be
asserted inside later private steps. Arbitrary mutated MIR must be submitted
to `passes::verify_final_mir`; it cannot be used to construct backend input.
Target-specific legality failures remain structured `BackendError` values.

Inline optional owning values follow the implemented layout in
[Optional Values](OPTIONAL_VALUES.md#initial-x86-64-inline-layout): an
eight-byte state word precedes the payload at its required alignment. The
backend writes a present payload before publishing state, branches before
reading a copied or unwrapped payload, and lowers verified absent-access
failure to the common reporter. Exact-class payloads use the same state prefix with aligned
reserved class bytes and conditional lifecycle calls. State zero is absent,
one is present and unguarded, and greater values count active views. Begin,
end, overflow, and pinned-mutation checks lower inline without runtime helpers.
Fields use that layout recursively. Internal inline optional parameters/results
use the documented pointer aggregate convention. `(shared T)?` is one
integer-class word: zero is absent and a nonzero word is the existing canonical
shared handle. Calls pass it in registers or stack slots and return it in
`rax`; generated conditional retain/release paths branch around zero before
entering ordinary shared machinery.
Inline optional-container aliases use that same container address without
transferring ownership or scheduling callee cleanup.

Verified inline and shared-outer arrays are executable for primitive,
primitive-optional, exact-class, exact-class-optional, recursively nested
inline, and ordinary or optional shared-owner elements. The target accepts
empty/default-length construction,
immutable length, checked positive and one-time negative-relative indexing,
named deep copy, produced-backing adoption, arbitrary-length whole
replacement, class fields, internal value parameters/results, conditional
optional lifecycle, exact per-element shared defaults, secure shared-owner
replacement, and decreasing-index recursive cleanup. Invalid indices branch
to the verified terminating failure edge before indexed address selection.
Copied slices and checked equal-length slice assignment execute after verified
bounds and length failure edges; right-side slice temporaries provide snapshot
semantics for overlap. Whole-array and exact element aliases execute through
internal non-owning addresses. Inline backing accounts defer detached element
destruction, while shared aliases reuse secured strong-owner anchors.

Explicit array element-list construction adds no new target semantic
choice for any stored element category. Instruction selection receives
verified unpublished backing, a source-derived constant count, exact ordered
initialization, initialized-prefix advancement, and complete publication.
Primitive slots reuse scalar stores. Exact-class slots reuse the ordinary
aligned array-element place plus initializer, hidden object-result,
copy-constructor, and destructor calls. Inline optional slots reuse the
existing state word, payload offset, conditional copy/destruction, and
presence store; `CompleteElement` only increments the verified prefix after
the whole wrapper is live. Nested slots reuse recursive array cloning for named
sources, descriptor adoption for produced sources, and decreasing-index release
for cleanup. Shared-owner slots reuse one-word retain/adopt/store and release;
optional shared-owner slots add only the existing zero test around those owner
operations. Exact shared-array owners use the same generated nested array
finalization machinery, independently of outer array ownership. All categories
reuse checked allocation, inline/shared header layout, publication, and release
while preserving allocation-before-element failure order and enclosing
full-expression lifetime. They do not default-construct then assign,
aggregate-copy class bytes, recover source expressions, publish a partial
prefix, or introduce a new runtime service or descriptor layout.

Verified string literal data is pooled by exact decoded bytes in first
canonical identity order. The target emits one eight-aligned local object per
unique byte sequence in relocation-read-only data: immortal count, exact
shared-array metadata relocation, byte length, then bytes. Empty literals use
the same 24-byte header without a byte payload. Literal evaluation loads that
symbol directly and initializes the three identity-selected descriptor fields
through ordinary class layout; it performs no allocation or byte copy.

Generated shared retain/release recognizes `u64::MAX` as the verified immortal
sentinel and returns without storing, finalizing, or freeing. Dynamic retain
reports ownership-count exhaustion at `u64::MAX - 1` before it could collide
with the sentinel. MIR
verification remains the trust boundary that restricts static sentinel
publication; ordinary dynamic publication writes count one.

## Implemented primitive operator target boundary

The
[implemented operator representation](PHASES_AND_IR.md#implemented-primitive-operator-representation)
defines the contract for the complete primitive-operator input. A target
consumes already selected operation flavor, type, width, signedness, failure
capability, and control flow. It never reconstructs semantics from source
spelling or host-language arithmetic.

Target realization must:

- retain the low 64 or 8 result bits for wrapping integer arithmetic and
  canonicalize `u8`;
- implement signed floor division and divisor-sign remainder rather than
  exposing the target's truncation convention;
- handle `i64::MIN / -1` and `i64::MIN % -1` before any target divide
  instruction that could fault;
- branch to the verified integer zero-divisor reason before target division;
- check a `u64` shift count against the left width before any instruction that
  could mask the count;
- use arithmetic right shift for `i64` and logical right shift for `u64` and
  `u8`;
- implement IEEE binary64 division without turning floating zero into panic;
- materialize unordered NaN equality and ordering exactly, including `!=`
  being true and all ordering predicates being false when either operand is
  NaN; and
- materialize every comparison and logical result as canonical `bool`.

Short-circuit `&&` and `||` arrive as verified ordinary CFG with an explicit
selected result and path-correct lifetime operations. The backend does not
evaluate a skipped operand, invent logical eager instructions, merge
incompatible ownership state, or synthesize cleanup. Branch layout and
instruction selection may change only when evaluation, failure, ownership, and
cleanup remain equivalent.

The accepted source path uses this same boundary for arbitrary valid `bool`
operands and consumers, including external calls. Canonical source-level
results therefore cross the ordinary System V boolean call and return ABI
without a logical-operator-specific convention.

Zero-divisor and excessive-shift paths use the existing static-termination
pool and sole `ska_rt_panic` call. Raw divide faults, target count masking, and
undefined or build-mode-dependent host overflow are not valid lowering
strategies. Wrapping overflow, the signed-minimum division pair, and floating
division by zero do not report panic.

The implemented operator profile adds no target calling-convention rule, public
symbol, metadata object, or runtime entry point. Exact instruction sequences,
register choices, flag use, branch shapes, and constant-folding algorithms
remain private after these observable requirements are met.

## Frozen operator-protocol target boundary

The frozen [operator-protocol lowering contract](OPERATOR_OVERLOADING.md)
reaches a backend only as an existing verified primitive operation or ordinary
interface call. Primitive-bound specialization must generate the same target
semantics as direct primitive syntax. Class implementations use existing
witness metadata, receiver adjustment, internal call ABI, result ownership,
and cleanup.

No backend receives source punctuation, unresolved protocol identity,
candidate set, primitive conformance object, or operator-specific dispatch
operation. The feature adds no calling convention, layout rule, public symbol,
runtime entry point, or target-specific semantic selection. This boundary is
implemented and covered by reordered independent-process assembly, symbol,
runtime-reference, and native operator fixtures; release closure remains
staged.

## Frozen complete primitive cast target boundary

The frozen
[primitive cast representation](PHASES_AND_IR.md#frozen-complete-primitive-cast-representation)
defines legal target-independent MIR input. Source type checking admits all
twenty-five cells. MIR carries and verifies the twenty-two pure cells and the
three checked diamonds, which x86-64 executes inline with ordered finite/range
checks, success-only conversion, and the common static-message failure path. A
target consumes already selected source type, target type, semantic class, and
pure or checked control-flow shape. It never derives signedness from source
spelling or substitutes a host language's conversion rules.

Target realization must preserve these results:

- identity conversions return the unchanged value, with `f64` identity
  preserving every binary64 bit;
- integer-to-integer conversions retain the implemented modulo and target
  signedness behavior;
- `bool` converts exactly to integer zero or one and floating zero or one;
- integer-to-`bool` produces false exactly for zero;
- `f64`-to-`bool` produces false for positive and negative zero and true for
  every other value, including unordered NaNs;
- integer-to-`f64` uses source signedness and correctly rounds to nearest with
  ties to even; and
- checked `f64`-to-integer conversion truncates toward zero only on its
  verified success edge and produces the exact canonical target value.

The x86-64 implementation performs these operations in generated code. It may
use scalar moves, comparisons, SSE2 conversions, integer adjustment, or
branches, but no exact instruction sequence is frozen. In particular,
`u64`-to-`f64` and `f64`-to-`u64` must handle values on both sides of `2^63`
without treating the operand as signed or delegating to an unversioned C cast.
Floating unordered comparison must make every NaN true when converting to
`bool`. Canonical `bool` and `u8` stores remain the ordinary scalar boundary.

The implemented identity and boolean boundary uses scalar bit-preserving
moves, explicit integer zero tests, and ordered-plus-unordered floating zero
comparison. Both floating zeroes become `false`; every NaN becomes `true`.
Boolean-to-integer results remain canonical zero or one, and boolean-to-`f64`
produces exact binary64 zero or one inline. These operations add no helper
call, failure edge, public symbol, or ABI category.

Integer-to-`f64` conversion is also inline. Signed `i64` and canonical `u8`
use scalar signed conversion after the ordinary canonical integer load; every
`u8` value is therefore exact. Unsigned `u64` values through `i64::MAX` use
the same direct conversion. For the upper half of the `u64` domain, generated
code converts the half-sized sticky-bit value
`(value >> 1) | (value & 1)` and doubles the result. This avoids signed
reinterpretation while preserving round-to-nearest, ties-to-even results
through `u64::MAX`, without a helper call or ABI addition.

Checked `f64`-to-integer input arrives as verified control flow with one
secured source, a semantic range check, success-only conversion, result join,
and a terminal failure edge. The backend must reject NaN, infinities, and a
mathematical truncated result outside the exact target range before executing
any target conversion whose out-of-range result is undefined, indefinite, or
target-specific. It must accept negative finite fractions whose truncation is
unsigned zero. A target conversion sentinel, hardware exception, ambient C
undefined behavior, saturation, or modulo wrapping is not a legal substitute.

Failure selects the exact `floating-point cast out of range` static message
through the existing `ska_rt_panic` entry point. The feature adds no conversion
helper, public symbol, calling-convention category, metadata object, allocation,
or runtime ABI version. Pure casts add no failure edge or reporter reference.

Verified binary64 bit reinterpretation is also inline but remains semantically
separate from numeric casting. `f64` to `u64` moves the complete scalar payload
from the SSE register class to the integer register class; `u64` to `f64`
moves the same payload in the opposite direction. Neither operation rounds,
canonicalizes NaNs, changes signed zero, branches, allocates, or calls a
helper. The target accepts only the verified exact `f64`/`u64` pairs and adds
no public symbol or runtime ABI entry point.

Constant folding and target peepholes may replace a cast only with an exactly
equivalent canonical value or terminal reason. Assembly and behavior must be
identical in meaning across optimization settings, including at every
representability boundary and for negative fractions near zero. The current
pipeline has no transforming optimization or peephole pass; its explicit pass
boundary preserves primitive-cast MIR byte-for-byte after verification.

## While-loop target boundary

The
[loop representation contract](PHASES_AND_IR.md#while-loop-representation)
keeps all source loop meaning and cleanup planning target-independent.
Implemented source `while` loops lower to cyclic control flow using the
existing generic branch and jump forms. Repeatable lifetime epochs are
verified no-code MIR markers. The backend consumes only the resulting
verified generic MIR and has no source-loop-specific operation or state.

The target boundary requires:

- a condition to arrive as an ordinary verified boolean branch;
- normal repetition to arrive as an ordinary backward jump;
- `break` to arrive as an ordinary jump to its already selected exit target;
- `continue` to arrive as an ordinary jump to its already selected latch
  target;
- return and panic to retain their existing terminator forms;
- source-ordered condition cleanup and every exited-scope cleanup to be
  explicit before the corresponding edge;
- one static MIR storage declaration to receive one target storage home even
  when its verified dynamic lifetime repeats; and
- all ownership, optional, array, checked-view, anchor, and full-expression
  operations to retain their ordinary lowering on every path and iteration.

A backend does not inspect source loop identities, structured HIR effects,
lexical nesting, cleanup depths, or source `while`, `break`, and `continue`
nodes. It does not infer cleanup from a backedge, reconstruct a loop scope, or
decide whether a source loop falls through. The array-specific generated-loop
terminator remains separate and is not a source-loop representation.

MIR lifetime-epoch operations are verified target-independent facts and emit
no machine instruction by themselves. The target-independent pipeline may
erase them after every consuming analysis, or a backend dispatcher may accept
them as verified no-code markers. That private choice must not reset
ownership, synthesize cleanup, or make correctness depend on recognizing a
loop in target code.

The initial condition, body, latch, and exit regions need not retain their
unoptimized block numbering or arrangement after valid transformations.
Instruction selection realizes the remaining generic labels, branches, jumps,
storage accesses, calls, and lifecycle instructions. Target-private branch
relaxation, block layout, and register allocation may optimize that realization
without changing evaluation frequency, ordering, failure, lifetime, or cleanup
behavior.

Loops add no target calling-convention rule, public symbol, metadata object,
runtime entry point, hidden iteration counter, per-iteration frame allocation,
or process-entry behavior. The existing fixed-frame strategy may reuse one
physical home across verified non-overlapping lifetime epochs. Runtime ABI
compatibility remains owned by the
[unchanged loop ABI boundary](RUNTIME_ABI.md#loop-abi-boundary).

Focused target tests must cover deterministic forward and backward edges,
fixed-home loop-carried values, calls across backedges, nested CFG, assembler
acceptance, and observable per-iteration cleanup. Those tests prove mechanical
realization of verified MIR; they do not establish source legality or repair
invalid lifetime state.

### Implemented general-iteration target boundary

The implemented [general-iteration compiler contract](ITERATION.md) lowers
`for-in` before the backend boundary. A target receives only verified ordinary
interface calls, optional operations, storage lifetimes, cleanup, branches,
jumps, and cyclic CFG. It neither distinguishes `for-in` from `while` nor
implements protocol selection, state progression, termination, lexical loop
identity, or loop-duration ownership.

State and item values use their ordinary exact layouts and calling convention;
interface calls use existing witness metadata. The mechanism adds no required
allocation, target opcode, metadata format, or optimization guarantee.
Compiler-generated direct, inherited, specialized, and bound-selected loops
exercise this boundary through deterministic assembly, assembler acceptance,
and native execution tests.

Eligible immediate primitive ranges reach the same boundary without interface
or optional operations: they are already ordinary current/end scalar storage,
integer comparison and increment, lifetime markers, branches, and jumps. The
backend neither recognizes a range nor receives canonical range metadata. This
adds no target opcode, symbol, calling convention, runtime service, or ABI
revision; explicit, stored, class, generic-bound, view, inherited, and
lookalike ranges continue through the protocol path above.

## Implemented standard I/O target boundary

Standard I/O has five dedicated verified MIR operations which the x86-64
target lowers directly to the five exact version-9 symbols specified by the
[I/O contract](IO.md#implemented-runtime-abi-version-9). Array operands become
a backing byte address at the checked offset plus the remaining byte count;
neither an array descriptor, owner, nor `Str` value crosses into C. The
frame-resident backing anchor remains live through the call and ordinary
full-expression cleanup releases it afterward.

Offset validity must be established before pointer arithmetic and the call.
The runtime's signed `i64` result returns through the ordinary integer result
register and is stored canonically without interpretation. Empty descriptors
produce a null pointer and zero length without a header access; an offset equal
to length produces a valid zero-length end pointer. All calls fit the existing
integer argument registers, preserve the fixed frame's call alignment, and
select symbols from the MIR operation rather than source names.

## Panic and hard-trap boundary

The version-9 runtime reporter and explicit source-panic lowering are
implemented. Compiler-known optional, array, cast, checked-shift, and checked
integer-division failures use the same reporter while retaining distinct
target-independent MIR reasons.
Legal ownership-count exhaustion enters the same static-message pool from its
backend retain edge; corrupted ownership state remains a separate hard trap.

Instruction selection centrally lowers the explicit-panic and static
termination forms described in
[Phases and IR](PHASES_AND_IR.md#frozen-panic-and-termination-representation).
Explicit panic extracts the logical byte address and length from the verified
exact `std::str::Str` descriptor. Static termination selects the corresponding
bytes from one deterministic target-private pool, with each used message
emitted once in stable catalog order. The pool is derived from final selected
instructions so backend-owned ownership-overflow edges and MIR termination
reasons share one exact mechanism. Both call the sole public
[`ska_rt_panic`](RUNTIME_ABI.md#panic-reporting-abi) entry point. Array,
optional, cast, checked-shift, checked integer-division, string, and ownership
lowering must not grow private reporter calls or duplicate the authoritative
[message catalog](../language/ERRORS.md#frozen-panic-design).

The reporter is only for explicit source panic and compiler-known,
source-reachable failures. Impossible states remain defects: ownership-count
underflow; null, dangling, or otherwise invalid live handles; zero live counts;
missing or incompatible dynamic metadata or finalizers; double finalization;
impossible states after successful MIR verification; and violated private
lowering invariants. Generated code hard-traps such states with `ud2`.
Malformed public MIR must still fail structurally at verification instead of
being compiled into either a panic or a hard trap.

A violated public runtime ABI precondition follows the runtime's private hard
failure path. It never calls the user-facing reporter and never emits a
`panic:` record. Default compiler output enables the target metadata, frame
maintenance, source-call, central reporter, generated-helper, and lower-level
runtime attribution support below. Trace state applies only to source-level
panic reporting, not to hard traps.

## Runtime trace target boundary

Runtime-trace input and deterministic static metadata are implemented for
Linux x86-64 and enabled by default in ordinary builds. The version-9 runtime
owns the hidden TLS state and allocation-free renderer. Each traced source
callable receives one 16-byte linked trace
record inside its ordinary fixed native frame: an eight-byte pointer to the
previous record and an eight-byte pointer to immutable static location
metadata. The runtime owns one hidden C11 thread-local top pointer. Generated
code accesses that symbol directly with the local-exec TLS model and
`R_X86_64_TPOFF32`; it makes no C call and performs no allocation, capacity
check, or depth check while maintaining the trace.

The x86-64 target emits six instructions at callable entry to initialize and
publish the record and two instructions on every normal return to restore
`previous`. Source calls and taken central reporter edges use the exact
two-instruction RIP-relative address load and frame-home store. `r11` is
transient scratch for these sequences, not a reserved register. Lowering emits
the replacement after call arguments are marshalled and before an indirect
target is loaded into `r11`, so neither the target nor arguments are lost. A
future register allocator may use every general-purpose register outside
these short sequences.

The pop is unchecked generated code. A null `previous` is the valid outermost
state; a stale or corrupt link is a compiler/runtime defect. Frame-layout,
assembly, and nesting tests own the invariant rather than adding a comparison
and branch to every return. Panic does not unwind, so all published records
remain live while the reporter walks them.

The target-private requested-only metadata planner emits deterministic,
relocation-read-only metadata consisting of one context per used traced source
callable and one location per distinct used callable and span-start
line/column. Contexts hold length-delimited pointers
and lengths for semantic callable names and escaped module-provider-relative
paths. Positional sources outside a configured root use their configured
relative display spelling when available and otherwise may remain absolute.
Locations hold a context pointer plus `u64` line and column. Bytes and records
are interned and ordered by semantic identity and location rather than address
or hash traversal. No record is emitted for an unused location.

Eligible frames and update sites are fixed by the
[phase boundary](PHASES_AND_IR.md#runtime-trace-phase-boundary). The
bodyless panic intrinsic, process wrapper, generated static coordinator,
generated lifecycle/array/ownership/finalization helpers, runtime C frames,
and target thunks never push. Ordinary source-authored standard-library and
lifecycle bodies do push. Direct, static, virtual, interface, external,
initializer, copy, assignment, destruction, and other source calls record
their originating MIR operation. Taken dynamic-panic and static-termination
edges record the failing operation immediately before their reporter call,
while successful checks and hard traps execute no failure replacement. Source
operations record their location before entering generated array, ownership,
copy, destruction, finalization, anchoring, or allocation paths. Omitted
helpers retain that attribution across nested calls and push no artificial
frame; a source-authored body entered from such a helper pushes normally.
Inline ownership overflow replaces the location only on its taken failure
edge. Known non-reporting runtime calls, deallocation, process/static
coordination, and hard-defect paths perform no unnecessary replacement.

Trace emission is default-on. Omission removes the record homes, TLS
instructions and references, location replacements, metadata and strings, and
trace-only source lookup. Thus `--omit-runtime-trace` has zero target execution
or metadata cost. Linux AArch64 may later realize the same frame semantics
through target-specific ELF TLS access, but no AArch64 instruction sequence or
implementation scope is frozen here.

## Data layout

The x86-64 target layout is:

| MIR type | Size | Alignment |
|---|---:|---:|
| `i64` | 8 | 8 |
| `u64` | 8 | 8 |
| `f64` | 8 | 8 |
| `u8` | 1 | 1 |
| `bool` | 1 | 1 |
| primitive `T?` | 16 | 8 |
| `shared T` | 8 | 8 |
| `(shared T)?` | 8 | 8 |
| `shared P?` box owner | 8 | 8 |
| `(shared P?)?` optional box owner | 8 | 8 |
| inline `T[]` descriptor | 8 | 8 |
| `Obj` | no owning storage layout | no owning storage layout |
| `unit` | no storage layout | no storage layout |

An inline array uses zero as its allocation-free empty descriptor.
A nonzero descriptor points to one allocation with an eight-byte owner/anchor
count at offset zero, immutable eight-byte length at offset eight, and aligned
elements beginning at offset sixteen. Eight-byte primitives have stride eight;
`u8` and `bool` have stride one. Checked target layout computes header,
alignment, stride, maximum element count, and total bytes before allocation.
Primitive and optional element layouts, complete exact-class layouts, and
nested descriptor layouts determine the stride. Private initialization,
copy-element, whole-clone, destroy-element, and release helpers are emitted in
canonical `ArrayTypeId` order.

An inline root class lays out fields in declaration order. A derived class
first embeds its complete direct-base layout at offset zero, then lays out its
direct fields in declaration order. Each field begins at the next offset
satisfying that field's alignment; the complete size is rounded up to the
maximum base-or-field alignment. Empty root classes remain addressable with
size and alignment one, so a derived field after an empty base is padded as
required by its own alignment.

Class dependencies are laid out recursively from semantic `ClassId` and
`FieldId` metadata. MIR field and base projections remain target-independent
identities; target layout turns both into checked byte offsets. Recursive
inline layouts, undeclared dependencies, inconsistent base metadata,
arithmetic overflow, and layouts beyond the target's signed 32-bit addressing
limit are structured errors.

A shared field occupies one aligned machine word containing the canonical
allocation-header pointer. Field initialization and moves store that word,
copies retain before storing it, replacement secures the incoming owner before
releasing the old word, and cleanup releases fields in the verified
derived-to-base destruction order. Generated complete finalizers recurse
through inline fields and dynamically finalize shared pointees without adding
ownership policy to ordinary place-address computation.

These sizes and offsets are implementation contracts for the current target,
not source-language promises or a portable external object layout. External
declarations cannot currently contain class values.

## Scalar System V classification

Parameters are classified independently into integer and SSE classes:

- `i64`, `u64`, `u8`, `bool`, addresses, aliases, and internal object-home
  pointers use the integer class;
- `f64` uses the SSE class; and
- `unit` is payload-free and cannot be a parameter.

The integer argument registers are `rdi`, `rsi`, `rdx`, `rcx`, `r8`, and
`r9`. The SSE argument registers are `xmm0` through `xmm7`. The two register
sequences are consumed independently while preserving source argument order.
Arguments that exhaust their class share one source-ordered stack area with
eight-byte slots. The outgoing area is rounded to 16-byte alignment; incoming
stack arguments begin after the saved frame pointer and return address.

Integer results use `rax`, and `f64` results use `xmm0`. `unit` has no result
payload. The backend keeps `u8` values zero-extended after arithmetic, calls,
parameters, and returns. It similarly normalizes the low-byte result of an
external C-compatible `bool` call before storing it as a Skald value.

Argument-location and size calculations use checked arithmetic. A signature
or argument area that cannot be represented by the backend is rejected before
instruction selection.

## External C ABI

Restricted external functions use the x86-64 System V C ABI and their exact
declared source symbol. The supported mappings are:

| Skald type | C-compatible boundary type |
|---|---|
| `i64` | `int64_t` |
| `u64` | `uint64_t` |
| `u8` | `uint8_t` |
| `f64` | `double` |
| `bool` | `bool` / `_Bool` |
| `unit` result | `void` |

This is the external ABI contract for the current target. The compiler emits a
call to the declared symbol and no body for it. Exact source rules and the
trusted-ABI boundary are owned by
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md).
Object values, alias parameters, receivers, and object results are not
supported in external signatures.

External symbol selection and C compatibility are stable within this target
contract. The runtime's exported symbols, ABI version, and platform
requirements are defined by the [runtime ABI](RUNTIME_ABI.md).

## Internal calling convention

Skald-internal calls reuse the scalar System V classification but add
compiler-private address conventions for inline objects:

- a receiver carries three integer-class components in order: its statically
  selected address, complete-object address, and dynamic metadata address;
- a class, interface, or `Obj` alias carries the same three components. Its
  first address selects the static class subobject or complete-object identity;
  forwarding preserves the latter two components unchanged. Read-only and
  mutable access use the same representation;
- an exact-class, primitive-optional, or primitive-array value parameter is an
  address to caller-created aggregate storage whose ownership transfer was
  already selected in MIR; and
- an exact-class, primitive-optional, or primitive-array result uses a hidden
  destination address before the receiver and explicit arguments.

For an object-returning method, the hidden result destination precedes all
three receiver components. Each object alias's complete-object and metadata
components immediately follow its static address. Explicit scalar/value
arguments otherwise retain source order, SSE arguments retain their
independent sequence, and overflow components share one aligned stack area.

A read-only alias bound to a produced exact-class object reaches the backend
as the same ordinary `MirArgument::View` used for an existing place. Its
source is the selected static subobject in caller-owned temporary storage; its
origin supplies the exact complete-object and metadata addresses. The normal
three-component classifier and marshaler consume those fields without a
produced-value branch. Construction, liveness, reverse full-expression
cleanup, and any owning copy made by the callee are already explicit in
verified MIR and add no target layout or calling-convention rule.

Produced exact-class method receivers reach the backend through this same
boundary. Verified MIR already contains the
caller-owned temporary, its ordinary read-only receiver view, complete-object
origin, selected call target, and cleanup. The backend marshals the
existing three receiver components; it does not classify the source producer,
create ownership, or add a receiver-specific operation. Focused native tests
exercise this path through exact, inherited, virtual, interface, generic, and
register/stack-pressure calls, while assembly assertions retain the ordinary
private method symbols and receiver component sequence.

The implemented produced-object field-read surface introduces no backend
boundary. Verified MIR contains the same caller-owned temporary plus ordinary
field projections, scalar loads, copies, aliases, calls, owner operations,
anchors, and guards selected before target lowering. The backend must
not classify a produced field source, invent a field-read calling convention,
delay result securing, or emit a feature-specific symbol or runtime call. Its
only obligation is to consume the already verified ordinary MIR operations
and preserve their explicit order before full-expression cleanup.

The verified definition's optional receiver storage is the sole authority for
incoming receiver classification, spilling, frame homes, and object-origin
homes. Class ownership alone does not add receiver ABI components. Target
legality classifies outgoing method calls from the declared method kind and
lifecycle calls as receiver-bearing; MIR verification guarantees that this
agrees with definition receiver presence. A verified
`MirCallTarget::Static(MethodId)` selects the existing class-method symbol as a
direct `CallableId::Method` call and uses this same convention without receiver
components. Hidden result destinations, explicit argument classification,
stack overflow, ownership, and cleanup are unchanged. Source `static fn` and
`private static fn` declarations use this path directly.

These conventions are not a stable public object ABI. They may change with the
compiler as long as each generated caller and callee agree and source-visible
behavior remains unchanged. Metadata is never reconstructed from a
base-subobject address. The backend does not choose copying, ownership,
elision, cleanup, or evaluation order; it mechanically realizes operations and
destinations already present in verified MIR. Those choices are owned by
[functions and control flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md),
[classes and lifecycle](../language/CLASSES_AND_LIFECYCLE.md), and
[aliases and ownership](../language/ALIASES_AND_OWNERSHIP.md).

## Frames and places

The current backend gives every MIR storage entry and transient scalar value a
fixed stack home. Scalar values and pointer homes use eight-byte size and
alignment. Inline object locals and temporaries receive their complete checked
class layout. The complete frame is rounded to 16-byte alignment and uses
`rbp`-relative addressing. With the implemented runtime-trace extension, each
eligible traced callable adds exactly one 16-byte trace record before that
rounding; generated helpers add none, and omitted tracing leaves frame layout
unchanged.

Primitive, inline-optional (including optional-array), optional shared-owner,
and inline-array static roots do not
receive frame homes. A private target-data plan maps each declaration identity
to one aligned, writable, zero-filled local object using the ordinary target
type layout. Instruction selection materializes its address with RIP-relative
`lea` and applies the existing optional state, checked payload, and one-word
optional-owner and array descriptor operations, after which the same scalar,
aggregate, ownership, backing-anchor, projection, and alias-address machinery
used by other places applies. Generated backing and element helpers remain
ordinary array-type helpers; only the descriptor root uses static addressing.

Return destinations and owned class, inline-optional, or inline-array
parameters store an incoming pointer in a frame home. Receivers and aliases
additionally store complete-object and metadata homes for forwarding.
Definitions without receiver storage allocate and spill no receiver or
receiver-origin homes.
Projecting a MIR place loads the appropriate base when indirect, then
accumulates checked target base and field offsets.
Byte fields use byte-width loads and stores; wider primitive and address
values use their target-width operations.

Frame allocation and projected displacements must fit signed 32-bit x86-64
frame-relative addressing. Failure is a structured callable-specific backend
error rather than wrapped arithmetic or truncated offsets.

The stack-heavy strategy is replaceable. Register allocation or another
location strategy must preserve the MIR and ABI boundaries rather than
changing language semantics.

## Dynamic dispatch metadata and calls

The backend computes one metadata table per declared class. Every table has a
unique address even when it has no dispatch entries. Virtual entries follow
canonical `VirtualSlotId` order. Interface witness entries follow dense
`InterfaceId` and `InterfaceRequirementId` order, independently of names and
conformance-list order. Each applicable entry contains the effective method
for that class; unrelated entries contain zero. Missing executable bodies,
invalid MIR metadata, unrepresentable table displacements, and unsupported
external object signatures are rejected before instruction selection.

Tables are private read-only relocation data containing method symbols.
Entering a polymorphic call with an exact object supplies its statically known
class table. Forwarded receivers and aliases copy the incoming complete-object
and table addresses. A virtual call loads its family slot; an interface call
loads the witness entry for its requirement. Both pass the complete object as
the selected method receiver and call indirectly through the ordinary ABI.
This is valid for the current single-inheritance layout because every base
subobject begins at offset zero. Direct calls continue to pass their statically
selected place. Produced aliases use this same metadata path, so ancestor,
interface, and `Obj` views retain the derived object's dispatch and identity
without slicing or copying.

Runtime membership checks compare the forwarded table address with the
deterministic set of declared classes that provide the requested class or
interface view. They do not inspect object bytes or traverse the class graph at
runtime. Runtime object casts and forwarded static views use bounded temporary
view homes containing the selected address, complete-object address, and unchanged
metadata; concrete static sources project directly. Cast failure executes the
central non-returning reporter path. The backend does not call a runtime cast
helper, reconstruct metadata, allocate for a cast, retain for a plain place
cast, or permit a failure edge to continue.

Owning inline consumers lower the successful checked address through the
ordinary verified copy-construction or copy-assignment instruction. Any
selected ancestor path is already an explicit MIR base projection. Checked
view homes end only after the copy completes, and produced source temporaries
then follow the ordinary reverse full-expression cleanup plan. No additional
backend copy path or runtime service exists for cast sources.

The `new T(copy source)` copy-allocation path uses the same verified
checked-place relation through a target-directed copy context, not a cast-side
allocation. It takes its failure edge before calling the allocator for its
destination, keep the source anchor live through selected exact-`T` copy
construction, and write metadata for `T` rather than copying the source's
dynamic-class metadata.

## Instruction selection and cleanup realization

The private target assembly model represents registers, memory operands,
labels, calls, arithmetic, branches, prologue/epilogue operations, and the
implemented floating-point instructions. An exhaustive MIR-instruction
dispatcher delegates to responsibility-specific lowering for values, calls,
assignment, copying, cleanup, and terminators.

Instruction selection uses caller-saved scratch registers and does not rely on
unpreserved callee-saved scratch state. Control-flow blocks are emitted in
deterministic `BlockId` order, with explicit branches and one function
epilogue.

Copy, construction, object-result, full-expression, and cleanup ordering are
not rediscovered here. Verified MIR names the selected operation, target
place, and order. Copy construction and assignment lower the selected base
step before the user body or synthesized direct fields. Cleanup follows the
verified plan through derived bodies, recursively projected fields, and the
base chain. No path performs implicit allocation, deallocation, or aggregate
runtime copy.

The shared-ownership extension provides explicit verified ordinary
allocation, publication, adoption, named-owner copy, ownership move, and
release operations. Its one-word
handle, allocation header, dynamic finalizer, and internal ABI rules are fixed
in [Shared-Ownership Compiler and Runtime Contract](SHARED_OWNERSHIP.md#x86-64-representation).
The current backend executes compatible shared local initialization and
assignment from named owners and ordinary allocations, including checked
retain, secure-before-release replacement, dynamic complete destruction, and
exact-base deallocation. Internal shared parameters use one integer-class word
in the existing register/stack classifier, and shared results use `rax`
without the hidden destination reserved for inline class results. Functions,
initializers, methods, interface calls, recursion, and mixed argument pressure
all use this same call facade. Stable shared local/parameter pointee places
derive their complete payload at header offset 16 and dynamic metadata at
offset 8, then reuse ordinary field layout, virtual/interface dispatch, and
type-test machinery. Shared-owner casts execute from owner storage and shared
fields. Verified call anchors use the same one-word owner representation and
the existing retain/release lowering; they add no target ABI or C runtime
operation.

### Shared optional box target boundary

Exact primitive wrappers use a
checked 32-byte allocation (the 16-byte shared header followed by the 16-byte
tagged optional), one deterministic descriptor per exact box identity, and a
distinct no-op finalizer. Ordinary retain, move, secure replacement, last-owner
release, runtime tracing, and exact-base free paths operate unchanged.
Lifecycle-bearing wrappers use recursive finalizers; polymorphic object views
and immutable published-pointee access use the same verified target boundary.

The box implementation keeps the one-word owner and existing shared header:

| Offset | Box allocation field |
|---:|---|
| 0 | non-atomic `u64` strong count |
| 8 | exact optional-box descriptor pointer |
| 16 | target-layout placement of the canonical `P?` wrapper |

The backend data-layout owner computes payload size, alignment, total checked
allocation size, and addressability. Offset 16 is valid for the currently
supported optional alignments, but implementation must reject an incompatible
future alignment rather than silently misaligning the payload.

Optional-box descriptors and finalizers are emitted in deterministic exact
target order. A descriptor names the exact `OptionalTypeId`; an object-box
descriptor additionally retains exact dynamic class and view-membership data
for class/base/interface/`Obj` casts, tests, and dispatch. Last-owner release
passes the payload at offset 16 through the existing recursive optional
destruction plan before freeing the exact allocation base. Primitive targets
may share no-op finalizer code only when descriptor identity and output remain
deterministic.

Verified box construction lowers allocate, initialize the unpublished wrapper,
publish, and adopt in that order. Published pointee operations lower presence,
copy, checked unwrap, read-only alias access, and compatible object views; no
instruction selection exists for whole-wrapper assignment. Existing owner
anchors and optional guards compose around bounded access. `shared P?` remains
one integer-class word in internal arguments and results, and `(shared P?)?`
uses the existing zero niche. Allocation failure, optional unwrap failure,
guard overflow, and layout overflow retain their current boundaries; there is
no checked box-store failure and no public runtime ABI change.

## Final fields

Final instance and static fields reach target lowering only after independent
MIR verification of their declaration metadata and any exceptional
complete-value-assignment authorization. The backend uses ordinary field and
static addresses, lifecycle calls, copies, ownership transitions, publication,
and cleanup. Finality changes no layout, alignment, calling convention, symbol
family, target instruction, runtime call, or ABI version. The five
standard-library primitive boxes therefore expose public final payloads with
the same one-field layouts and calling conventions as their former private
ordinary payloads.

## Implemented generic-class specialization target boundary

The frozen [generic-class compiler contract](GENERIC_CLASSES.md) reaches the
backend only as ordinary closed exact classes. Each accepted application has a
distinct `ClassId`, fully substituted base and fields, concrete member and
lifecycle identities, and verified ordinary MIR. The backend receives no type
parameter, specialization request, generic dictionary, or runtime argument
list.

Layout, internal ABI classification, allocation metadata, finalizers,
dispatch tables, statics, calls, and cleanup therefore follow the existing
rules for the generated exact types. Distinct application identities remain
distinct even when their layouts or instructions happen to match. Private
symbols must encode enough canonical identity to avoid collisions and remain
deterministic; their exact spelling is not a source or compatibility contract.

Generic specialization adds no target instruction family or public runtime
call. The implementation verifies all concrete identities against the closed
MIR tables, classifies substituted signatures through the ordinary
register/stack/hidden-result rules, and emits layouts, dispatch, allocation,
finalization, statics, calls, and cleanup through the existing paths. Private
symbols encode the canonical semantic application plus the closed `ClassId`;
assembler and native tests cover equal-layout identities and cross-module
applications.

## Implemented generic-interface specialization target boundary

The implemented [generic-interface compiler contract](GENERIC_INTERFACES.md)
reaches the backend only through ordinary closed `InterfaceId` and
`InterfaceRequirementId` values. Each exact application uses the existing
interface view, complete-object metadata, witness lookup, receiver/result ABI,
checked cast, type-test, shared-owner, cleanup, and runtime-trace paths.

Distinct applications remain distinct metadata and witness entries even when
their substituted signatures coincide or one concrete method satisfies both.
Private names include sufficient canonical application identity to prevent
collisions. The backend receives no interface template, type argument vector,
dictionary, substitution request, or erased generic-interface representation,
and the feature adds no target instruction family or runtime call.

Backend, assembler, runtime-trace, and native tests cover distinct applications
sharing one method address, inherited overrides, bound-selected and structural
calls, produced shared results, exact casts/tests, checked failure, and the
unchanged version-9 ABI marker.

## Symbols and process entry

Internal callable and block symbols are derived deterministically from stable
compiler identities and use target-private local names. Their exact textual
spelling is a debugging detail, not a compatibility contract and not an input
to semantic lookup. Instance and static methods intentionally share the same
collision-proof class-owned method symbol family because their `MethodId`
identities are already distinct; static methods do not create a parallel
symbol namespace.

Static-field symbols are deterministic target-private object symbols derived
from canonical module, class, and field identities. Source visibility does not
export them, inherited or aliased selection does not duplicate them, and their
spelling remains a debugging detail rather than a source or ABI identity.
Each explicit initializer also receives one deterministic target-private
callable symbol derived from its canonical field identity. Fixed private
program-initializer and program-finalizer symbols coordinate startup and
shutdown. The backend consumes the same verified structured activation and
destruction regions checked at the final MIR boundary. Their transitions select
call order but emit no state slot, load, branch, or ordinary static-access
guard; zero-default activation emits no instruction.

External calls preserve the exact declared symbol. The backend also emits one
exported C-compatible `main` wrapper, which checks runtime ABI compatibility
through the current marker, calls the private program initializer when static
work exists, calls the identity-selected Skald entry function, preserves its
result across the private finalizer after normal return, and returns its low C
`int` result. Runtime marker ownership belongs to the
[runtime ABI](RUNTIME_ABI.md#version-and-link-compatibility) rather than the
internal symbol scheme.

The implemented [process-argument contract](../language/PROCESS.md) does not
change this wrapper or the parameterless internal entry call. The
`std::process` module reads the Linux host record through ordinary library I/O;
there is no backend argument-capture path or target ABI addition.

## Target-independent reachability boundary

The confirmed
[whole-world reachability design](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_DESIGN_PROPOSAL.md)
and
[completed roadmap](../archive/TARGET_INDEPENDENT_WHOLE_WORLD_REACHABILITY_ROADMAP.md)
move semantic definition retention ahead of target lowering. Backends accept
both complete and verified sparse final MIR and still perform the
machine-artifact retention described below.

`VerifiedFinalMirProgram` now owns verified reachability facts bound to its
exact final-MIR program. `BackendInput` projects only canonical required-
runtime-entity identities and used virtual-family/interface-requirement
queries; target code does not consume the analysis representation or pass
policy. Target legality, callable-signature checks, runtime-trace activation
planning, fixed frames, and instruction selection visit physically retained
executable definitions rather than assuming that every dense declaration has
a body. An absent final-MIR body is legal only because target-independent
verification independently proved its declaration unreachable. Class and
value layout remains declaration-driven; it does not walk callable bodies and
may conservatively cover unused semantic metadata.

Dispatch planning distinguishes complete semantic declarations from the
virtual families and interface requirements reachable MIR can select. It may
retain extra target metadata conservatively, but it may not demand an absent
unreachable method body solely because an unused dense slot or conformance
names the declaration. Dense ABI slot positions remain stable. A physically
present unused implementation remains available to complete-emission mode; an
absent implementation becomes null only in a verified-unused slot. Every
entry usable from a reachable call or implicit lifecycle operation still
selects a verified retained body. Required class, array, optional-box, literal,
static, and other runtime entities come from the target-independent retained-
domain query rather than a second backend-specific semantic reachability
walker. The x86-64 boundary validates those required identities before
planning and conservatively retains extra metadata where pruning is
unnecessary for sparse-body correctness.

This changes no ABI, layout identity, symbol spelling contract, export rule,
or target failure classification. Complete-emission diagnostics continue to
emit every body physically present in their verified input; they cannot
resurrect a body removed before lowering.

The target-private exported-symbol walk remains mandatory after instruction
selection. Entry wrappers, ABI shims, generated array/ownership/optional-box/
finalization helpers, concrete dispatch tables, literal backings, panic
messages, runtime-trace records, and any later target-generated dependency do
not all have target-independent MIR identities. Earlier MIR retention reduces
the input domain; machine-artifact retention proves the final emitted symbol
closure.

## Frozen reachability-gated static lifecycle boundary

Status: **frozen direction, not yet implemented**. Current backend input still
contains lifecycle work for every declared static. The future source semantics
are defined by
[Static Fields](../language/STATIC_FIELDS.md#frozen-reachability-gated-activation-direction),
and phase ownership is defined by
[Phases and IR](PHASES_AND_IR.md#frozen-reachability-gated-static-lifecycle-direction).

The backend will continue to accept only verified final MIR and will not infer,
narrow, or replan static activation. Its private program initializer and
finalizer will be generated solely from the certified active coordinator
regions, so no inactive initializer or eventual-value destruction may execute.
Target planning should use the verified active-static query for storage and
metadata where safe.

A first implementation may conservatively plan an addressable private slot for
an inactive declaration when a physically retained unreachable body still
mentions it. Such a slot has no source-visible lifetime, initializer, or
destructor. The existing target-generated symbol walk must remove it and all
initializer, helper, literal, trace, and metadata artifacts reachable only from
inactive work in ordinary emitted output.

This direction changes no public runtime service, ABI version, host wrapper,
entry/result protocol, object layout, field layout, calling convention,
relocation rule, or public symbol. Whole-world compilation makes active storage
known before target lowering, while single-threaded execution requires no
guard, once-state, synchronization, atomic operation, or thread-local variant.

## Assembly emission and artifact retention

The production driver requests closed-world artifact retention after target
instruction selection and before textual emission. Exported functions seed a
deterministic symbol graph; direct calls, callable addresses, static slots,
dispatch entries, literal metadata, panic messages, and runtime-trace records
form its edges. Unreachable functions and data are omitted together, while an
ordinary reference retains its complete transitive implementation. This
target-level boundary sees only explicit machine symbols and has no knowledge
of source sugar or language-item identities.

Direct backend consumers may instead request complete emission. That mode is
used for phase-owner diagnostics and tests which need to inspect lowering of
an otherwise uncalled verified MIR body. HIR and every MIR product remain
complete and deterministic in both modes; artifact retention does not mutate
or replace the verified program.

The target assembly model is rendered as deterministic GNU assembler text
beginning with `.intel_syntax noprefix`. Instructions use destination-first
operands, bare register names, bracketed memory operands, and explicit memory
widths where required. Function metadata, explicit local labels, and a
non-executable-stack note remain GNU/ELF directives. The generated text is
accepted by the system assembler in focused tests. Determinism is tested both
by repeated backend emission and independent compiler processes.

Textual assembly is a supported compiler artifact published through the
[driver](DRIVER_AND_ARTIFACTS.md), but exact internal symbol names, label
spellings, frame offsets, and instruction sequences may change between
compiler revisions. External symbols and behavior covered by the target ABI
remain the compatibility boundary.

Focused tests cover target registration, primitive, nested, and inherited
class layout, mixed register/stack classification, hidden destinations and
receivers, frame/place addressing, legality and structured failures,
instruction selection, assembler acceptance, call pressure, virtual and
interface dispatch, runtime type operations, and native execution. Golden
execution additionally covers deep base chains, static and polymorphic views,
slicing, object results, temporaries, and complete lifecycle order.

Private cell assignment adds no target operation or representation. After MIR
verification, it uses the same place address, load/store, type-directed copy or
assignment, cleanup, dispatch, and generic symbol machinery as assignment
through a mutable root. The cell evidence is a compiler trust-boundary fact;
it is not emitted metadata and does not alter field layout or callable ABI.

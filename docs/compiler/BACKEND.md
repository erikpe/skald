# Backend and Target Contract

Status: authoritative for the current backend interface, supported target
registry, target legality, x86-64 System V realization, and generated assembly
boundary. Source-visible language semantics remain owned by the
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

## Backend interface and target registry

Backends consume target-independent `MirProgram` values. They do not inspect
the AST, resolved IR, HIR, or type-checker state. The public backend facade
provides:

- `backend::Target`, the selected target identity;
- `backend::target_by_name`, which validates a user-facing target name;
- `backend::SUPPORTED_TARGET_NAMES` and `DEFAULT_TARGET_NAME`; and
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

1. run the public MIR verifier again at the backend trust boundary;
2. reject verified MIR features not yet implemented by this target;
3. compute deterministic class dispatch tables from verified virtual families,
   interfaces, requirements, conformances, and classes;
4. compute checked primitive and class layouts;
5. check that every executable signature and called member can be represented
   by the target calling convention;
6. plan fixed stack frames and target addresses;
7. select target instructions into a private assembly model; and
8. emit deterministic GNU assembly text in Intel syntax with `noprefix`.

Malformed MIR is returned as a backend error before target layout or
instruction selection. Target-specific failures—including recursive layout,
unrepresentable sizes, missing callable bodies needed by target lowering,
argument-area limits, frame limits, and displacement limits—also return
`BackendError`. An error identifies its target and, when applicable, the
callable being lowered.

The target accepts verified static single inheritance, base projections,
owning slices, class/interface/`Obj` alias views, virtual-family calls, and
interface calls. Runtime class/interface tests compare the forwarded dynamic
class metadata identity against the verified target set. Checked object casts
use the same check, materialize a successful full-expression view, and emit an
illegal-instruction trap on failure. Shared-owner casts use the same metadata
membership test, retain or transfer the source handle on success, preserve the
existing allocation header, and never call the allocator.

Producer invariants already established by MIR verification may be asserted
inside later private steps. Arbitrary mutated MIR is supported only through
the verifier and structured backend-error boundary, not as a valid lowering
input.

Inline optional owning values follow the implemented layout in
[Optional Values](OPTIONAL_VALUES.md#initial-x86-64-inline-layout): an
eight-byte state word precedes the payload at its required alignment. The
backend writes a present payload before publishing state, branches before
reading a copied or unwrapped payload, and lowers verified absent-access
failure to `ud2`. Exact-class payloads use the same state prefix with aligned
reserved class bytes and conditional lifecycle calls. State zero is absent,
one is present and unguarded, and greater values count active views. Begin,
end, overflow, and pinned-mutation checks lower inline without runtime helpers.
Fields use that layout recursively. Internal inline optional parameters/results
use the documented pointer aggregate convention. `shared? T` is one
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
semantics for overlap. Array aliases remain structured legality errors.

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
| `shared? T` | 8 | 8 |
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
`rbp`-relative addressing.

Return destinations and owned class, primitive-optional, or primitive-array
parameters store an incoming pointer in a frame home. Receivers and aliases
additionally store complete-object and metadata homes for forwarding.
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
selected place.

Runtime membership checks compare the forwarded table address with the
deterministic set of declared classes that provide the requested class or
interface view. They do not inspect object bytes or traverse the class graph at
runtime. Runtime object casts and forwarded static views use bounded temporary
view homes containing the selected address, complete-object address, and unchanged
metadata; concrete static sources project directly. Cast failure executes the
same non-returning `ud2` boundary. The backend does not call a runtime cast
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

## Symbols and process entry

Internal callable and block symbols are derived deterministically from stable
compiler identities and use target-private local names. Their exact textual
spelling is a debugging detail, not a compatibility contract and not an input
to semantic lookup.

External calls preserve the exact declared symbol. The backend also emits one
exported C-compatible `main` wrapper, which checks runtime ABI compatibility
through the current marker, calls the identity-selected Skald entry function,
and returns its low C `int` result. Runtime marker ownership belongs to the
[runtime ABI](RUNTIME_ABI.md#version-and-link-compatibility) rather than the
internal symbol scheme.

## Assembly emission and verification

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

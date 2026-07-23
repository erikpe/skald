# Backend and Target Contract

Status: authoritative for the current backend interface, supported target
registry, target legality, x86-64 System V realization, and generated assembly
boundary. Source-visible language semantics remain owned by the
[language documentation](../language/README.md); the runtime C interface is a
separate contract.

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
Linux x86-64 backend using the System V AMD64 ABI and GNU assembler syntax.
Other names are rejected rather than silently selecting a fallback.

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
8. emit deterministic GNU assembly text and metadata.

Malformed MIR is returned as a backend error before target layout or
instruction selection. Target-specific failures—including recursive layout,
unrepresentable sizes, missing callable bodies needed by target lowering,
argument-area limits, frame limits, and displacement limits—also return
`BackendError`. An error identifies its target and, when applicable, the
callable being lowered.

The target accepts verified static single inheritance, base projections,
owning slices, class/interface/`Obj` alias views, virtual-family calls, and
interface calls. Runtime class/interface tests compare the forwarded dynamic
class metadata identity against the verified target set. Checked narrowing
uses the same check, materializes the successful scoped view under the
ordinary alias ABI, and emits an illegal-instruction trap on failure.

Producer invariants already established by MIR verification may be asserted
inside later private steps. Arbitrary mutated MIR is supported only through
the verifier and structured backend-error boundary, not as a valid lowering
input.

## Data layout

The x86-64 target layout is:

| MIR type | Size | Alignment |
|---|---:|---:|
| `i64` | 8 | 8 |
| `u64` | 8 | 8 |
| `f64` | 8 | 8 |
| `u8` | 1 | 1 |
| `bool` | 1 | 1 |
| `Obj` | no owning storage layout | no owning storage layout |
| `unit` | no storage layout | no storage layout |

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

These sizes and offsets are implementation contracts for the current target,
not source-language promises or a portable external object layout. External
declarations cannot currently contain class values.

## Scalar System V classification

Parameters are classified independently into integer and SSE classes:

- `i64`, `u64`, `u8`, `bool`, addresses, aliases, and internal object-home
  pointers use the integer class;
- `f64` uses the SSE class; and
- `unit` is payload-free and cannot be a parameter.

The integer argument registers are `%rdi`, `%rsi`, `%rdx`, `%rcx`, `%r8`, and
`%r9`. The SSE argument registers are `%xmm0` through `%xmm7`. The two register
sequences are consumed independently while preserving source argument order.
Arguments that exhaust their class share one source-ordered stack area with
eight-byte slots. The outgoing area is rounded to 16-byte alignment; incoming
stack arguments begin after the saved frame pointer and return address.

Integer results use `%rax`, and `f64` results use `%xmm0`. `unit` has no result
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
- an exact-class value parameter is an address to caller-created parameter
  storage whose ownership transfer was already selected in MIR; and
- an exact-class result uses a hidden destination address before the receiver
  and explicit arguments.

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
`%rbp`-relative addressing.

Return destinations and owned class parameters store an incoming pointer in a
frame home. Receivers and aliases additionally store complete-object and
metadata homes for forwarding. Projecting a MIR place loads the appropriate
base when indirect, then accumulates checked target base and field offsets.
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
runtime. A successful narrowing stores the selected address, complete-object
address, and unchanged metadata address in scoped alias frame homes. Failure
executes `ud2`; it does not return or run remaining source cleanup.

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

The target assembly model is rendered as deterministic GNU/AT&T-style text
with function metadata, explicit local labels, and a non-executable-stack
note. The generated text is accepted by the system assembler in focused tests.
Determinism is tested both by repeated backend emission and independent
compiler processes.

Textual assembly is a supported compiler artifact published through the
[driver](DRIVER_AND_ARTIFACTS.md), but exact internal symbol names, label
spellings, frame offsets, and instruction sequences may change between
compiler revisions. External symbols and behavior covered by the target ABI
remain the compatibility boundary.

Focused tests cover target registration, primitive, nested, and inherited
class layout, mixed register/stack classification, hidden destinations and
receivers, frame/place addressing, legality and structured failures,
instruction selection, assembler acceptance, call pressure, and native
execution. Golden execution additionally covers deep base chains, padded and
empty bases, static views, slicing, object results, temporaries, and complete
lifecycle order.

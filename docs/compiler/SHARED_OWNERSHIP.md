# Shared-Ownership Compiler and Runtime Contract

Status: **frozen implementation design; typed HIR vocabulary implemented**.
This document is
authoritative for the planned target-independent ownership representation,
x86-64 allocation layout, generated reference-counting operations, dynamic
finalization, hidden anchors, and the minimal C allocation boundary. The
source-visible contract is [Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md).
Object cast legality and consuming contexts are owned by
[Object Casts](../language/OBJECT_CASTS.md).

Source AST and resolved IR retain shared targets, exact allocation class
identities, and ordinary-versus-copy allocation modes. Typed HIR has canonical
class/interface/`Obj` shared targets, named-place and produced-owner sources,
explicit copy/adopt transfers, and ordinary allocation with one selected
initializer. MIR lowering rejects this vocabulary with a structured error;
MIR and the backend still have no shared representation or operations, and the
current [runtime ABI](RUNTIME_ABI.md) remains version 4 with no allocation
functions. Explicit copy allocation and shared-owner casts remain typed
exclusions.
The completed
[constructor-semantics roadmap](../archive/CONSTRUCTOR_SEMANTICS_ROADMAP.md)
supplied overload-selected ordinary initialization, the distinct copy
constructor identity, and reusable ordinary-versus-copy construction modes.

## Responsibility split

The compiler owns all ownership policy:

- shared types, conversions, and source diagnostics;
- strong-owner provenance and copy-versus-adopt selection;
- retain, release, assignment, temporary, and cleanup insertion;
- borrow-anchor selection and lifetime;
- allocation-header and dynamic-metadata layout;
- complete most-derived finalizer generation and selection; and
- verification that every generated ownership operation is well formed.

The C runtime owns only checked byte allocation and deallocation behind a
versioned ABI. It does not own reference counts, object metadata, finalizer
dispatch, roots, tracing, safepoints, cycle detection, or borrow analysis.

## Target-independent phase contract

Resolution assigns the existing class, interface, field, callable, and
polymorphic identities. `shared T` carries a resolved static target, and
every `new C(...)` carries the exact concrete `ClassId` and either ordinary-
initializer-overload or explicit copy-allocation mode; lower phases never
recover these facts from a source name or argument shape. Type checking
selects the exact ordinary `InitializerId` using the same overload engine as
inline `C(arguments)`.

Typed HIR records:

- shared types and compatible class/interface/`Obj` views;
- the exact class and construction mode selected by every `new`;
- for `new T(copy source)`, the selected exact-class copy constructor,
  target-directed checked source, source provenance, and any full-expression
  anchor requirement;
- whether each owner use copies a named place or adopts a produced owner;
- shared local, parameter, result, field, and temporary lifetimes;
- pointee places with their static target, access, complete-object provenance,
  and dynamic-metadata provenance;
- the anchor requirement and source for every shared-backed receiver or alias
  argument; and
- the full-expression anchor requirement for every shared-backed checked place
  cast.

HIR remains target-independent. It contains no header byte offset, reference
count location, metadata slot, runtime symbol, register, or calling-convention
classification.

MIR makes every ownership effect executable and explicit. Its schema must
represent, by semantic operation rather than necessarily by these Rust names:

- lower ordinary `new C(arguments)` to allocation storage for exact `C` and
  invocation of its selected ordinary initializer;
- lower `new T(copy source)` as source evaluation and anchoring,
  target-directed checked-place selection, allocation storage for exact `T`,
  invocation of its selected copy constructor, and publication of the
  produced owner, in that order;
- create a strong owner by copy, transfer a produced owner by adopt, and end an
  owner by release;
- perform shared assignment in secure-incoming, release-old, store order;
- project a statically selected class/interface/`Obj` view from a shared
  handle while retaining complete-object and metadata provenance;
- create and end hidden call and checked-cast anchors;
- delimit shared full-expression temporaries and normal cleanup; and
- select a compiler-generated complete-object finalizer from dynamic metadata.

No backend may infer ownership operations from uses or insert an unverified
retain/release policy. Optimization may remove an operation only after MIR
represents it and only when ownership, destruction timing, and failure behavior
remain unchanged.

## MIR verification

The MIR verifier must reject a program unless all of the following hold:

- every shared storage entry has a compatible shared type and is initialized
  before use;
- every live shared storage entry or owning temporary accounts for one strong
  owner;
- a named-place value use copies, while a produced owner has exactly one adopt
  or release path;
- every normal lifetime end releases each owner exactly once;
- shared assignment secures the incoming owner before releasing the old owner;
- no place or view is used after its owner's verified lifetime;
- every shared pointee view carries a compatible static target, complete-object
  origin, and dynamic-metadata origin;
- every replaceable borrowed source has an explicit owning anchor covering the
  consuming call, copy, or other cast full expression;
- an inline subobject borrow is covered by the complete allocation's anchor;
- receiver and argument anchors are created in source evaluation order and
  remain live through the call;
- a checked cast view ends before its shared anchor is released on every
  normal full-expression exit;
- each ordinary allocation names a concrete constructible class and its
  selected ordinary initializer;
- each copy allocation originates from the exact source shape
  `new T(copy source)`, names concrete copy-constructible `T`, performs any
  target-directed dynamic check before allocating its destination, invokes
  the selected `T` copy constructor exactly once, and retains its checked
  source and anchor through completion;
- every allocation originates from source `new`; no cast, conversion, inline
  copy, owner copy, anchor, call, result, or assignment independently creates
  an allocation operation;
- each dynamic class identifies exactly one compatible complete finalizer; and
- shared values are absent from external signatures and static storage.

As with existing MIR, invalid public or mutated MIR fails verification before
target layout or code generation. Reference-count underflow and dangling
provenance are compiler defects that verification and focused lowering tests
must prevent, not recoverable machine states.

## x86-64 representation

A shared handle is one non-null integer-class machine word pointing to the
start of a backend-private allocation header. All static shared targets use the
same handle representation.

For the `x86_64-sysv` target, one allocation is:

| Offset | Field | Representation |
|---:|---|---|
| 0 | strong count | little-endian `u64` |
| 8 | dynamic metadata | non-null pointer to the complete dynamic class descriptor |
| 16 | complete-object payload | existing checked inline layout of the allocated exact class |

The payload begins at offset 16 and retains the existing class size, alignment,
base-prefix, and field rules. The current target's maximum object alignment is
eight, so the C allocator's alignment and the 16-byte header satisfy every
payload alignment. A future type requiring greater alignment must revise the
allocation contract rather than silently misalign the payload.

The header pointer is the canonical owned identity. The backend derives the
complete payload address by adding 16 and derives dynamic identity and dispatch
from the metadata field. Base and field projections are then applied to the
payload using existing checked target layout.

A shared field is one eight-byte, eight-aligned handle. Shared
class/interface/`Obj` parameters and results use one integer-class component;
the returned handle is in `rax`. These are compiler-private internal calling
conventions, not external C ABI.

Allocation size is checked as header size plus complete payload size. Overflow,
a size not representable by the runtime boundary, or an addressability
violation is a structured backend error when statically knowable. The runtime
never receives a zero byte count for a shared object.

## Dynamic class descriptors and finalizers

Every executable class descriptor has a unique address and includes one
compiler-generated complete finalizer entry in addition to its existing
virtual and interface dispatch information. Exact slot offsets remain backend
private, but descriptor identity continues to serve runtime type tests.

The allocation header always points to the descriptor for the exact class
named by `new`, for both ordinary and copy allocation, even when the handle's
static type is an ancestor, interface, or `Obj`. Copy allocation never derives
this descriptor from the source metadata. Upcasts copy only the header pointer
and never replace its metadata.

The complete finalizer accepts the complete payload address. It performs the
ordinary complete-object destruction sequence for its exact class:

1. the most-derived user destructor body, if any;
2. direct fields in reverse declaration order, recursively destroying inline
   class fields and releasing shared fields;
3. the direct base's complete destruction sequence; and
4. return to the generated last-release path.

The finalizer never frees the header and does not select itself from the
handle's static target. The last-release path calls it once through the dynamic
descriptor and then passes the original header pointer to the C deallocator
once.

## Generated strong-count operations

The compiler initializes a new allocation's count to one only after the header
and complete-object payload are ready to become the produced owner.

A generated retain:

1. loads the `u64` count;
2. terminates unsuccessfully if it is `u64::MAX`; and
3. stores the incremented count.

A generated release:

1. requires a non-null valid header and a positive count;
2. if the count is greater than one, stores the decremented count and returns;
3. if the count is one, marks it zero, loads the dynamic finalizer, and calls
   that finalizer on the complete payload; and
4. after the finalizer returns, calls the runtime deallocator on the original
   header.

Marking the last count zero makes reentrant misuse fail as a compiler defect.
Safe source cannot resurrect the object: non-owning aliases cannot be converted
into shared owners, and a last release cannot occur while another valid strong
owner exists.

Counts are non-atomic. The initial implementation provides no cross-thread
sharing contract. Overflow uses the same class of backend-owned unrecoverable
termination as a failed checked cast and guarantees neither diagnostic
text nor remaining cleanup.

## Hidden anchor lowering

Anchors use ordinary owning shared storage in HIR and MIR, marked hidden only
from source lookup and diagnostics. They follow the same copy, adopt, release,
overflow, and cleanup rules as source-visible owners.

For a call, lowering:

1. selects the receiver and materializes any required receiver anchor;
2. evaluates explicit arguments left to right, materializing each required
   anchor at its argument position;
3. performs the call while all anchors remain live;
4. secures the result; and
5. releases anchors and other full-expression temporaries in reverse
   completion order.

An existing shared local or value parameter does not need an extra owner for
an ordinary call because its storage cannot be rebound by the callee. A
replaceable field or nested replaceable place is copied into an anchor. A
produced owner is adopted by a temporary whose lifetime extends through the
call. A shared allocation's anchor covers every inline subobject within its
payload.

For a checked place cast from shared ownership, lowering evaluates the source
once and establishes any required owner before the dynamic check. An existing
local or value parameter already supplies a stable owner; a replaceable place
is copied; and a produced owner remains adopted by its temporary. The success
edge supplies a non-owning view to its consuming receiver, alias argument, or
inline copy. When the consumer is `new T(copy source)`, that same view and
owner remain live while exact `T` storage is allocated and its selected copy
constructor runs; the produced owner is secured before the view and anchor
end. A cast from an existing alias uses its verified outer lifetime and
creates no shared owner.

HIR need only record provenance and the required anchor category. MIR owns the
explicit storage and lifetime operations. Neither phase performs arbitrary
object-graph search or a general exclusivity borrow analysis.

## Minimal C runtime ABI

The first shared-ownership implementation requires an incompatible runtime ABI
revision from version 4 to version 5. In addition to carrying the renamed
version-5 marker and existing output functions, the public C header will add:

```c
void *ska_rt_alloc(uint64_t byte_count);
void ska_rt_free(void *allocation);
```

`ska_rt_alloc` requires a nonzero count representable by the host allocator.
It returns a suitably aligned non-null allocation of at least that many bytes.
If conversion to `size_t` is impossible or allocation fails, it terminates the
process unsuccessfully without returning or guaranteeing remaining Skald
cleanup.

`ska_rt_free` requires the exact non-null base pointer returned by one
successful `ska_rt_alloc` call that has not already been freed. It deallocates
that allocation exactly once. Violating the precondition is a compiler/runtime
defect.

The C implementation is deliberately a checked wrapper around `malloc` and a
wrapper around `free`. It does not know the header shape, initialize counts,
inspect metadata, invoke finalizers, or implement retain and release. Exact C
termination machinery remains private; the stable behavior is unsuccessful
non-returning failure.

ABI version 5 and these symbols are frozen for the shared-ownership
implementation, but are not claims about the currently shipped version-4
archive. The header, runtime implementation, every generated process-entry
marker, direct C harnesses, mismatch tests, and documentation must transition
together.

The runtime owns neither initializer nor copy-constructor selection nor
partially constructed object state. For copy allocation, the compiler
completes target-directed source selection before calling `ska_rt_alloc` for
the destination, then constructs the exact class named by `new`.

Dynamic cloning is outside this contract. The compiler does not derive a new
allocation class from source metadata or synthesize a clone path. A future
`clone()` convention or dedicated syntax requires a separate source and
lowering design.

## Safety argument and test obligations

The implementation preserves the source safety contract through four checked
boundaries:

- type checking prevents null construction, invalid conversions, escaping
  aliases, and unsupported external/static storage;
- HIR makes owner provenance and anchor requirements explicit;
- MIR makes ownership effects and lifetimes explicit and verifies them before
  every backend; and
- the backend realizes verified operations over one non-null handle/header
  representation while the runtime only allocates and frees bytes.

Focused implementation tests must cover named and produced values in every
local/field/parameter/result/assignment position; direct and indirect
self-assignment; cleanup order; dynamic finalization through every static
target; nested shared fields; strong cycles; call and checked-place cast
anchors; ordinary and copy allocation from inline, alias, produced, and
shared-backed sources; static slicing and checked downcast copies; unavailable
copy construction; shared-owner casts; receiver/argument order; cast failure
before destination allocation; overflow and allocation failure; malformed MIR;
ABI version mismatch; deterministic HIR/MIR dumps; assembly acceptance; and
native execution.

Strong cycles intentionally remain allocated, so leak detection must
distinguish that specified behavior from an owner lost by incorrect lowering.
No implementation roadmap is defined by this document.

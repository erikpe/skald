# Shared-Ownership Compiler and Runtime Contract

Status: **implemented through explicit exact-class copy allocation on x86-64**.
This document is
authoritative for the target-independent ownership representation,
x86-64 allocation layout, generated reference-counting operations, dynamic
finalization, hidden anchors, and the minimal C allocation boundary. The
source-visible contract is [Shared Ownership and Heap Allocation](../language/SHARED_OWNERSHIP.md).
Object cast legality and consuming contexts are owned by
[Object Casts](../language/OBJECT_CASTS.md).
The implemented
[optional-values compiler contract](OPTIONAL_VALUES.md) wraps ordinary shared
ownership as `(shared T)?`; it does not weaken the non-null handle invariants
defined here. Its zero niche is tested and branched around before existing
retain, release, metadata, dereference, anchor, cast, or finalization paths.
The optional-box extension, implemented through verified MIR and native
x86-64 realization, is specified in
[Optional Values](OPTIONAL_VALUES.md#shared-optional-box-representation)
and in the [box allocation contract](#shared-optional-box-allocation)
below.

Source AST and resolved IR retain shared targets, exact allocation class
identities, and ordinary-versus-copy allocation modes. Typed HIR has canonical
class/interface/`Obj` shared targets, named-place and produced-owner sources,
explicit copy/adopt transfers, and ordinary allocation with one selected
initializer. MIR implements compatible shared-owner semantics: allocation,
publication, adoption, named-owner copying, secure-before-release assignment,
full-expression temporary cleanup, and normal local release are distinct
verified operations. Shared call arguments, parameters, return owners, and
caller results use explicit ownership handoffs across every internal callable
kind. Shared fields use projected copy, initialization, secure replacement,
synthesized copy/assignment, and destruction-plan release operations in
verified MIR and execute as owning graph edges on x86-64. Compatible owner
up-views and stable local/parameter pointee places retain canonical header,
complete-payload, static-target, access, and metadata provenance through HIR
and MIR. Direct, virtual, and interface calls, inherited projections, mutable
pointee access, and `is` use those places on x86-64. Shared casts retain a
named owner or transfer a produced owner only on the statically guaranteed or
runtime-checked success path; failure terminates and no cast allocates or
copies payload. Plain checked-place casts classify stable, replaceable, and
produced shared sources, preserve exact produced-allocation provenance, and
use verified checked-view and owner-anchor lifetimes across every immediate
non-owning, inline-copy, and shared copy-allocation consumer. Explicit copy
allocation evaluates and secures its checked source before allocation, invokes
one selected exact-class copy operation, then publishes and adopts the owner.
The x86-64 backend executes the
defined handle, header, checked retain, one-word internal ABI, count-one
publication, recursively generated complete finalization, and last-owner
deallocation. [Runtime ABI version 9](RUNTIME_ABI.md) provides that allocation
boundary and the current common services without a box-specific entry point.
Ordinary
initializer overloads, the distinct copy-constructor
identity, and reusable ordinary-versus-copy construction modes are defined by
[Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md).

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

Type checking has one checked shared-pointee conversion for the boundary from
an explicitly dereferenced owner source to a non-owning object place. It consumes the existing
named-place or produced-owner classification and records a stable shared
origin or an anchored shared origin together with the target, access,
projections, source span, and anchor source. Class and interface receivers,
alias arguments, plain checked-place casts, type tests, field access, and
owning inline-copy consumers all use that operation. HIR therefore states the
owner-versus-borrowed-place distinction directly; MIR lowering does not infer
it from source expression shape or a consumer's expected type.

Resolution represents an explicit `*owner` or the dereference implied by
`owner->member` with one typed dereference node carrying its resolved shared
target, owner source, operator provenance, and spans. Direct fields,
class/interface receivers, inline projection, mutation, alias arguments,
plain casts, type tests, and every target-directed owning inline copy consumer
consume that node through the checked shared-pointee conversion above. The
same path covers `T(copy *owner)` and `new T(copy *owner)`. The explicit syntax
therefore changes no HIR ownership effect, MIR place, anchor state, backend
layout, or runtime operation. Resolver and type checking reject every raw
owner-to-pointee use before HIR; direct handle operations never enter this
conversion.

Shared-owner consumers do not accept this dereference node: owner
initialization, assignment, value arguments/results, up-views, and shared
casts operate on the handle. Resolution also rejects a dereference as a
whole-object assignment destination with a dedicated diagnostic; field
assignment through a dereferenced receiver remains an ordinary supported
projection.

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

The implemented owner state machine separates an allocation storage slot
from owner storage:

```text
new -> allocated -> initialized -> published -> adopted owner
                                                   |
                                                   v
                                                released
```

Only the publication transition creates count-one ownership. The allocation
slot remains compiler-owned construction provenance and cannot be used as a
place, view, receiver, or call argument. A named source is secured with
`SharedCopy`; a produced allocation is secured with `SharedAdopt`. A checked
optional unwrap is always secured into a fresh owning temporary. Direct local
initialization consumes that temporary with `SharedMove`; assignment first
secures either source into an owning temporary, releases the old local, and
then consumes the temporary with the same instruction. This makes checked
failure and direct or allocation-alias self-assignment safe without exposing
an empty owner. Any unconsumed owning temporaries are released in reverse
creation order before the explicit full-expression boundary.

The verifier requires every move source to be live and temporary, requires the
destination to be either fresh or just released, consumes the source exactly
once, and rejects a full-expression boundary with a live owning temporary.
Normal return requires every local owner to have been released. CFG joins
require identical shared allocation and owner states. These operations are
target-independent and carry no handle size, header offset, or runtime symbol.
Hidden call owners use the distinct `SharedAnchor` storage role, so dumps and
verification can distinguish them from general temporaries.

`MirPlaceBase::SharedPointee(owner)` is the target-independent root for a
borrowed payload place. Its owner must be a live stable local/value parameter
or `SharedAnchor` with a compatible shared target. Base and inline-field
identities remain semantic projections. `MirObjectOrigin::Shared` separately retains the
canonical owner, source static target, and access; its complete address and
metadata are deliberately derived only at the backend boundary. Verification
checks the target and projection relation, mutable access, origin agreement,
owner liveness at every use, and direct-versus-dynamic dispatch selection.
For a shared-backed checked cast, the verifier also records the dependency
from the checked-view carrier to that owner. Releasing the owner while the
carrier is live is invalid; `EndCheckedView` must precede `SharedRelease` on
every normal path. Exact dynamic provenance retained from an allocation is
checked against that allocation and may prove a cast static even after a
shared up-view.

At a call boundary, caller-owned argument storage is initialized by copying a
named owner or adopting a produced owner and is then consumed exactly once by
the call. The corresponding callee value parameter begins live, may be
replaced with the ordinary secure-before-release assignment sequence, and is
released by normal parameter cleanup. A named return is copied and a produced
return is adopted into the callee's dedicated return-owner storage before
local and parameter cleanup. `ReturnShared` then transfers that sole remaining
owner to caller-selected result storage. Verification rejects duplicate
argument transfer, an uncleaned parameter, an initialized result destination,
or a normal shared return with any owner other than its declared result still
live.

On x86-64, allocation storage and shared-owner storage each receive one
eight-byte stack home. `SharedAllocate` checks the exact class payload plus
the 16-byte header before calling `ska_rt_alloc`; initialization receives the
payload address through the existing receiver ABI; publication writes the
exact class descriptor and then count one; and adoption and move transfer the
header word without retaining it. Named-owner copy performs the checked
non-atomic `u64` retain. Release validates the header and positive count,
selects the complete finalizer from dynamic metadata on the one-to-zero
transition, and calls `ska_rt_free` only after finalization returns. The
verified copy operation uses the same checked retain lowering in source
programs. Assignment mechanically follows the verified copy-or-adopt, release,
move order.

An internal shared parameter is one integer-class word and follows the
existing source-ordered register/stack classifier alongside scalar and object
arguments. A shared result is returned directly in `rax`; unlike an inline
class result, it does not consume a hidden return-destination argument.
Incoming parameter spilling and outgoing result securing preserve that
distinction under receiver arguments, register exhaustion, recursion, and
indirect interface dispatch.

## MIR verification

Closed generic classes preserve ownership grouping before MIR lowering.
Substituting `T` into `shared T`, `(shared T)?`, and `shared T?` selects,
respectively, an ordinary non-null owner, an optional shared owner, and an
ordinary owner targeting a shared optional box. Exact class, base class,
interface, and `Obj` targets reuse their canonical shared identities and the
same retain, replacement, destruction, guard, and anchor plans as hand-written
closed code. The typed frontend does not introduce a generic owner kind;
generic native execution uses this same verified ownership path.

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
- shared values are absent from external signatures, and every non-optional
  shared static value is established by verified eager initialization before
  ordinary access.

The implemented [static-field profile](../language/STATIC_FIELDS.md) admits
initializer-free optional `(shared T)?` statics, whose zero state owns nothing,
and explicitly initialized ordinary or optional shared statics. Initializers
reuse ordinary adoption, copy, allocation, publication, and full-expression
cleanup. Present values use ordinary ownership transitions during execution.
Reverse normal-return shutdown releases the final current owner; last-owner
release uses the same dynamic complete finalizer and allocation-freeing path as
ordinary owners. Abrupt termination remains non-unwinding. The profile never
admits a zero or uninitialized ordinary `shared T` handle.

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

The current backend performs that derivation directly: a shared-pointee place
starts at header offset 16, while shared object origins load metadata at offset
8. Existing class layouts, dispatch descriptors, virtual slots, interface
witness entries, and metadata-membership tests are reused. No view, dispatch,
or type-test helper is added to the C runtime.

A shared field is one eight-byte, eight-aligned canonical header handle. It
follows the ordinary base-prefix and field-padding rules and therefore does not
change inline projection semantics. Shared class/interface/`Obj` parameters
and results use one integer-class component; the returned handle is in `rax`.
These are compiler-private internal calling conventions, not external C ABI.

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
handle's static target. Each nested shared-field release uses that field's
canonical header and dynamic descriptor. The last-release path preserves the
original header in an aligned private spill across the recursive finalizer
call, then passes that exact pointer to the C deallocator once.

## Generated strong-count operations

The compiler initializes a new allocation's count to one only after the header
and complete-object payload are ready to become the produced owner.

A generated retain:

1. loads the `u64` count;
2. returns without storing if it is the verified immortal sentinel
   `u64::MAX`;
3. reports `ownership count overflow` if it is `u64::MAX - 1`; and
4. otherwise stores the incremented count.

A generated release:

1. requires a non-null valid header and a positive count;
2. returns without storing if it is the verified immortal sentinel
   `u64::MAX`;
3. if the count is greater than one, stores the decremented count and returns;
4. if the count is one, marks it zero, loads the dynamic finalizer, and calls
   that finalizer on the complete payload; and
5. after the finalizer returns, calls the runtime deallocator on the original
   header.

Marking the last count zero makes reentrant misuse fail as a compiler defect.
Safe source cannot resurrect the object: non-owning aliases cannot be converted
into shared owners, and a last release cannot occur while another valid strong
owner exists.

Counts are non-atomic. The initial implementation provides no cross-thread
sharing contract.
The frozen [common reporting policy](../language/ERRORS.md#frozen-panic-design)
turns source-reachable count overflow into the shared reporter's ownership
reason. Underflow, invalid handles, zero live counts, incompatible metadata or
finalizers, and reentrant or double finalization remain compiler defects and
hard-trap through the
[backend defect boundary](BACKEND.md#panic-and-hard-trap-boundary).

### Frozen immortal-allocation extension

The implemented canonical-owner slice of the
[string compiler contract](STRINGS.md#immortal-shared-storage) reserves
`u64::MAX` for verified compiler-emitted program-lifetime allocations. Once
that producer exists, retain and release of a proven immortal handle are
successful no-ops, while retaining an ordinary dynamic count of
`u64::MAX - 1` terminates before colliding with the sentinel.

This is a general compiler-private shared-allocation state, not a source
ownership qualifier or string-specific header. Only verified static allocation
may publish it. The backend implements the sentinel-aware rules above;
ordinary dynamic publication writes one and cannot reach the reserved value
because retain reports exhaustion at `u64::MAX - 1`.

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

HIR records whether a borrow is forwarded, covered by stable owner storage, or
requires an anchor carrying a copied place or adopted producer. MIR owns the
explicit `SharedAnchor` storage and copy/adopt/release lifetime operations.
Neither phase performs arbitrary object-graph search or a general exclusivity
borrow analysis.

Within the structured short-circuit MIR representation, owner, allocation,
retain/release, checked-view, and hidden-anchor state remains separated by the
declared path condition until conditional full-expression cleanup converges.
The skipped alternative acquires and releases no owner. A selected checked
view must still end before its matching owner or anchor is released, including
when nested logical selections establish several owners before cleanup.

## Shared optional box allocation

The implemented
[compositional optional compiler contract](OPTIONAL_VALUES.md#compositional-optional-implementation)
normalizes `(shared T)?` and its `shared? T` shorthand as
`Optional<Shared<T>>`. Target-independent optional operations conditionally
invoke the ordinary copy, adopt, release, anchor, metadata, and finalization
operations defined here. The x86-64 zero-handle niche remains an optional
representation choice; zero never enters a plain-owner operation.

The `shared P?` extension introduces `Shared<Optional<P>>` as a distinct
allocation target while retaining the same non-null owner operations. The
compiler assigns canonical resolved, HIR, and MIR targets; selects local owner
copy/adoption/replacement and exact wrapper construction; and verifies the
complete lifetime. The x86-64 backend executes every eligible wrapper target,
object view, stored position, call boundary, and immutable pointee operation
through the ordinary header, count, and exact-base release path.

Each box allocation records a distinct optional-box origin, an exact canonical
optional payload target, and one descriptor/finalizer identity. Allocation
storage is unpublished while the existing optional initialization plan builds
the complete wrapper at its payload place. Publication creates count-one
ownership only after initialization, and ordinary adopt/copy/move/release
operations then manage the handle. Last release invokes the wrapper's exact
recursive optional destruction plan and frees the original header once.

The published optional wrapper is immutable. A box pointee supports presence,
owning copy, checked unwrap, and eligible read-only aliases, but not
whole-pointee assignment or mutable whole-wrapper aliases. Owner storage may
be securely replaced with a different compatible box handle without changing
the old allocation observed by other owners. Existing anchors keep the box
alive through non-owning use; existing optional guards protect bounded present
payload access.

Exact non-object boxes are invariant. An object box descriptor additionally
retains the exact concrete dynamic class while owners carry compatible static
class/base/interface/`Obj` box views. Owner casts, type tests, and dispatch
reuse the existing metadata relation without replacing or slicing the
allocation. The absence/presence state remains fixed, while an already-present
contained object remains shallowly mutable through its ordinary object view.

The x86-64 realization reuses the 16-byte header and places the canonical
optional payload at target-layout offset 16. The metadata word identifies a
deterministic exact box descriptor and finalizer; it is not forged as an
unrelated class or array descriptor. Optional box owners in calls remain one
integer-class word, and `(shared P?)?` reuses the existing zero niche. All
layout, metadata, guards, counting, finalization, and failure behavior remains
compiler-owned, so runtime ABI version 9 and its C surface do not change.

## Minimal C runtime ABI

Runtime ABI version 9 carries its version-specific marker, the common panic
reporter, the existing output functions, and these allocation operations:

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
allocation-failure termination machinery remains private; the stable current
behavior is unsuccessful non-returning failure. Valid host allocation failure
uses the version-9 [`ska_rt_panic`](RUNTIME_ABI.md#panic-reporting-abi) entry
point.
Invalid byte counts and violated deallocation preconditions remain runtime
contract defects rather than user panic.

ABI version 9 and these symbols are the current shared-ownership runtime
boundary. The header, runtime implementation, every generated process-entry
marker, direct C harnesses, mismatch tests, and documentation carry the same
version.

The runtime owns neither initializer nor copy-constructor selection nor
partially constructed object state. For copy allocation, the compiler
completes target-directed source selection before calling `ska_rt_alloc` for
the destination, then constructs the exact class named by `new`.

Dynamic cloning is outside this contract. The compiler does not derive a new
allocation class from source metadata or synthesize a clone path. A future
`clone()` convention or dedicated syntax requires a separate source and
lowering design.

The implemented [array compiler contract](ARRAYS.md) reuses this minimal allocation
boundary and secure owner machinery for exact non-polymorphic shared array
targets. Array lengths, element lifecycle, backing anchors, slice operations,
and finalizers remain compiler-generated rather than new C runtime
responsibilities. Shared-array ownership composes through the ordinary typed
owner operations and generated array lifecycle.

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

Implemented optional-owner lowering proves that zero represents only absent
`(shared T)?` storage and never reaches the non-null operations in this
contract. Present optional owners reuse the same copy, adopt, release,
metadata, finalization, and anchor rules. The canonical spelling
`(shared T)?` preserves this exact obligation.

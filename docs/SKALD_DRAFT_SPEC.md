# Skald Draft Language Specification

Status: exploratory draft.

This document defines the **Skald** programming language. Skald began as a design exploration derived from Niflheim, but it is a distinct language rather than a backwards-compatible revision. Its central departure is the memory model: Skald is not garbage collected. It uses deterministic object lifetimes, value semantics, built-in shared ownership, and call-scoped alias parameters.

The goal of this document is to describe the language itself, not its standard library.

---

## 1. Purpose and Scope

Skald is a learning-oriented, statically typed, compiled language. It should be practical for small personal projects while remaining simple enough that the compiler and runtime can be understood by one person.

The canonical filename suffix for Skald source code is `.ska`. The Skald compiler is named `skac`.

Primary goals:

- deterministic memory management instead of garbage collection;
- a small runtime;
- object-oriented programming with classes, methods, inheritance, and interfaces;
- explicit ownership and optionality;
- no raw pointers in safe code;
- no pointer arithmetic in safe code;
- no nullable non-optional values;
- efficient function calls over both inline objects and heap objects;
- enough continuity with Niflheim that existing design/compiler lessons remain useful.

Non-goals for the initial language:

- backwards compatibility with Niflheim;
- garbage collection;
- raw pointers in safe user code;
- general-purpose lifetime analysis / Rust-style borrow checking;
- generics;
- concurrency;
- closures with captured variables;
- standard library specification;
- unchecked undefined behavior in safe code.

Checked exceptions are part of the intended language design because they affect deterministic cleanup and code generation. A first compiler may still implement them after the non-exception core is working.

---

## 2. Design Summary

Skald distinguishes four important ways an object can be used:

```ska
fn takes_value(dog: Dog) -> unit;         // copies an inline Dog value
fn takes_ref(ref dog: Dog) -> unit;       // aliases a Dog read-only for this call
fn takes_mut(mut ref dog: Dog) -> unit;   // aliases a Dog mutably for this call
fn takes_shared(dog: shared Dog) -> unit; // copies a shared heap handle
```

Example call behavior:

```ska
var d: Dog = Dog();
var s: shared Dog = new Dog();

takes_value(d);       // copies Dog
takes_value(s);       // illegal unless an explicit pointee-copy form is later added

takes_ref(d);         // aliases inline Dog
takes_ref(s);         // aliases shared pointee

takes_mut(d);         // mutably aliases inline Dog
takes_mut(s);         // mutably aliases the shared pointee; s itself is not rebound

takes_shared(s);      // copies shared handle; increments reference count
takes_shared(d);      // illegal; inline Dog is not heap/shared
```

Key memory-model decisions:

- `Dog` is an inline value type.
- `shared Dog` is a non-null owning reference-counted heap handle.
- `ref name: Dog` and `mut ref name: Dog` are alias-binding forms; `Dog` remains the bound name's type.
- In the first implementation, alias bindings exist only for parameters and cannot be stored, returned, captured, assigned, or converted to `shared`.
- every alias-bound argument has a caller-owned anchor that keeps its storage alive for the complete call;
- ordinary instance `fn` methods have read-only receivers, while `mut fn` methods have mutable receivers;
- `mut ref` is mutable but not exclusive. Two mutable alias parameters may refer to the same object.
- Optional values are explicit using postfix `?`, for example `Dog?` or `shared Dog?`.
- Plain `Dog` and `shared Dog` are never null, and an alias binding must always designate a live `Dog` place.

---

## 3. Source Files, Modules, and Visibility

Skald uses a module-oriented source organization.

Skald source files use the canonical `.ska` suffix. The `skac` compiler accepts `.ska` source files as compilation inputs.

Supported declaration kinds:

- imports;
- classes;
- interfaces;
- top-level functions;
- external functions.

Module import forms:

```ska
import a.b;
import a.b as b;
import a.b as x.y;
import a.b as .;

export import a.b;
export import a.b as b;
export import a.b as x.y;
export import a.b as .;
```

Symbols are private to the defining module by default. `export` makes declarations or imported bindings visible to downstream modules.

Qualified names are explicit:

```ska
util.Counter
util.make_counter()
```

Unqualified names resolve local-first. If multiple imports provide the same unqualified name and there is no local declaration shadowing them, unqualified use is a compile-time ambiguity error.

### 3.1 Restricted Bootstrap External Functions

The compiler implements this deliberately narrow external declaration form:

```ska
extern fn external_name(value: i64) -> unit;
extern fn external_value(value: i64) -> i64;
```

It is a top-level declaration terminated by a semicolon and has no Skald body.
Parameter names are mandatory. The implemented profile permits by-value
`i64`, `u64`, `u8`, `f64`, and `bool` parameters and an `i64`, `u64`, `u8`,
`f64`, `bool`, or `unit` result.

The same restricted profile supports by-value `bool` parameters and results:

```ska
extern fn external_predicate(value: i64) -> bool;
extern fn external_bool_sink(value: bool) -> unit;
```

It does not permit alias parameters, `shared`, objects, arrays, optionals,
function values, variadic arguments, alternate link names, or user-selected
calling conventions. Supporting `bool` does not otherwise generalize the
foreign-function interface.

On Linux x86-64 System V, `u64`, `u8`, and `f64` correspond to C `uint64_t`,
`uint8_t`, and `double`. A supported target must represent C `double` as
IEEE-754 binary64 before it can implement Skald `f64`.

Defined and external functions share one non-overloaded top-level function
namespace. Any repeated name is a compile-time error, including two identical
external declarations or an external declaration and a Skald definition. An
external declaration named `main` is illegal and cannot satisfy the entry-point
requirement; the entry point remains a source-defined `fn main() -> i64`.

The external function's source identifier is its exact linker symbol. The
compiler does not add a module prefix or other mangling and this profile has no
source-level symbol override. Below resolution, the compiler represents the
selected declaration with a stable callable identity; later phases must not
repeat name lookup or recognize individual runtime functions by spelling.
Compiler-generated symbols for Skald definitions must use a target-private
spelling that cannot equal any valid external source identifier. This keeps
exact external symbols collision-free without reserving an ordinary Skald
identifier prefix for compiler use.

Calls use the selected target's C ABI. For the initial Linux x86-64 System V
target, Skald `i64` corresponds to C `int64_t`, Skald `bool` corresponds to C
`bool` (`_Bool`), Skald `u64` and `u8` correspond to C `uint64_t` and `uint8_t`,
Skald `f64` corresponds to compatible C `double`, and a Skald `unit` result
corresponds to C `void`. `unit` has no runtime payload.

`i64`, `u64`, `u8`, and `bool` use the System V integer class. `f64` uses the
SSE class. Scalar argument layout allocates the six integer registers and eight
SSE registers with independent counters; a mixed signature does not select an
XMM register from the argument's overall source position. An argument whose
class has exhausted its registers is passed in an eight-byte stack slot while
later arguments of the other class may still use registers. Stack placement
and call-site padding follow System V and preserve 16-byte call alignment.

`i64` and `u64` results use `%rax`, `u8` and `bool` results use their ABI result
byte, and `f64` results use `%xmm0`. Boolean arguments leave Skald as canonical
C false or true values. Incoming `u8` and boolean values are zero-extended or
normalized before general Skald use; unspecified upper result-register bits
are never part of a Skald value. Argument evaluation remains left to right and
is independent of ABI placement. The stage-0 compiler uses the same scalar
placement for Skald-defined and restricted external calls.

An external declaration is a trusted assertion about the definition supplied
at link time. `skac` checks Skald uses against the declared signature, and the
linker diagnoses a missing symbol, but the compiler cannot verify that a
supplied foreign definition has a compatible ABI type. An incompatible linked
definition is outside the language's safety and behavior guarantees.

This profile is sufficient to declare the bootstrap output functions in
Sections 13.1 through 13.3 as their value types become implemented. It does not
settle imports, export and visibility behavior, cross-module coalescing of ABI
declarations, separate compilation, ownership transfer, or the complete
foreign-function interface. Those remain specification gaps.

---

## 4. Types and Binding Modes

### 4.1 Primitive Types

Skald provides the following primitive value types:

- `i64`
- `u64`
- `u8`
- `bool`
- `f64`
- `unit`

Primitive types are always value types.

The floating-point type is named `f64` to state its width directly. It is an
IEEE-754 binary64 value and maps to C `double` only at compatible restricted
foreign-ABI boundaries; `double` is not a Skald type keyword.

`bool` is distinct from every integer type. Its only values are the literals
`false` and `true`; it does not acquire numeric truthiness merely because a
target may encode those values as zero and one. The initial C-series compiler
profile supports `bool` in parameters, results, initialized locals,
expressions, and calls. Physical storage width is target-defined. In
particular, the initial stack-heavy backend may use an eight-byte home without
making `bool` an eight-byte language type or an alias for `i64`.

Default values:

- `i64`, `u64`, and `u8` default to integer zero;
- `f64` defaults to positive zero (`0x0000000000000000`);
- `bool` defaults to `false`;
- `unit` has a single value.

These defaults apply only where a storage construct permits default
initialization. The currently implemented local-declaration grammar still
requires an initializer.

#### 4.1.1 Numeric Literals

The remaining-primitive slice uses this decimal literal grammar:

```text
decimal-digits = ASCII-digit+
exponent       = ("e" | "E") ["+" | "-"] decimal-digits

i64-literal = decimal-digits
u64-literal = decimal-digits "u"
u8-literal  = decimal-digits "u8"
f64-literal = decimal-digits "." decimal-digits [exponent]
            | decimal-digits exponent
```

An unsuffixed integer literal has type `i64`; expected type never changes it.
The suffixes are case-sensitive. `u` denotes `u64`, `u8` denotes `u8`, and
`u64` is not a valid suffix. A decimal point requires digits on both sides, so
`.5` and `1.` are invalid in this profile. Hexadecimal, octal, binary,
digit-separated, explicitly suffixed `f64`, infinity, and NaN literals are not
included. A leading `-` is always the unary operator rather than literal text.

Integer magnitudes are checked during type checking. `u64` accepts `0u`
through `18446744073709551615u`; `u8` accepts `0u8` through `255u8`. The
existing unsuffixed `i64` range and special unary-minus handling for `i64::MIN`
remain unchanged.

A decimal `f64` literal is converted to the nearest IEEE-754 binary64 value,
with ties resolved to the even significand. A literal that rounds to infinity
is a compile-time range error. Subnormal results and underflow to positive zero
are valid; unary negation may then produce negative zero. Semantic and
executable IR preserve the resulting raw 64-bit representation, and
deterministic dumps render exactly 16 lowercase hexadecimal digits rather than
host-formatted decimal text.

Numeric-looking malformed text is consumed as one invalid token where
possible. Bad suffixes, incomplete exponents, repeated decimal points, and
identifier tails should therefore produce one focused lexical diagnostic
rather than a misleading sequence of valid tokens.

### 4.2 Object Types

Class types are inline object types by default:

```ska
var dog: Dog = Dog();
```

An inline object has deterministic lifetime. It is constructed at initialization and destroyed when its storage lifetime ends. Assignment updates an already-live object and does not end its lifetime or invoke its `destroy` member.

The word "inline" describes language semantics, not a required physical stack layout. A compiler may place values in registers, stack slots, caller-provided return storage, or optimized-away storage as long as observable construction/destruction behavior is preserved.

### 4.3 Shared Types

`shared T` is a built-in owning heap handle:

```ska
var dog: shared Dog = new Dog();
```

Properties:

- `shared T` is non-null.
- `shared T` always points to a heap allocation owned by reference counting.
- the static target `T` may be a concrete class, a base class, `Obj`, or an interface; `new T(...)` still requires a concrete constructible class;
- Copying a `shared T` handle increments the reference count.
- Destroying or overwriting a `shared T` handle decrements the reference count.
- Every shared allocation records the complete dynamic class type with which it was constructed.
- Upcasts and interface conversions preserve the allocation identity, complete-object address, reference count, and dynamic type metadata.
- When the count reaches zero, the complete most-derived object is destroyed according to the recorded dynamic type, regardless of the static type of the releasing handle, and the original heap allocation is then freed exactly once.
- `shared T` is a fundamental language type, not an ordinary user-defined class.
- There are no raw pointer operations on `shared T`.

Reference cycles are allowed to leak in the initial language. A later version may introduce `weak T` or another cycle-breaking mechanism.

### 4.4 Universal Root Type

`Obj` is the universal root type for object hierarchies.

In Skald, `Obj` is usually meaningful through a polymorphic alias or shared handle:

```ska
fn describe(ref value: Obj) -> Str;
fn retain(value: shared Obj) -> unit;
```

Standalone inline variables of type `Obj` are not allowed initially:

```ska
var value: Obj = Dog(); // illegal
```

This avoids slicing arbitrary object values down to an empty or partial root object. Concrete object values should use their concrete class type, while polymorphic APIs should use read-only or mutable alias parameters of type `Obj`, or `shared Obj`.

### 4.5 Alias Binding Modes

Every variable or parameter name denotes a typed storage place. Skald separates the place's object type from the way the name is bound. A value parameter `name: T` owns local parameter storage initialized by copying the argument. A read-only alias parameter `ref name: T` and a mutable alias parameter `mut ref name: T` instead name an existing `T` place owned elsewhere.

The source place may be inline storage or the pointee of a `shared T` handle. This distinction is not observable through the alias: member access, virtual dispatch, and further calls operate on the same `T` object in either case. Inline versus shared ownership is a caller-side concern, not a parameter-type distinction. The callee receives no ownership-provenance tag and cannot test whether an alias originated from inline or shared storage.

Within the callee, an alias name otherwise participates as a `T` place. Supplying it to a value parameter copies the underlying `T`; supplying it to another compatible alias parameter forwards access to the same place. Neither operation creates a storable reference value.

`ref` and `mut ref` are binding modes, not type constructors or general reference value types. In the first implementation, explicit alias bindings are valid only on function, method, `init`, and interface-method parameters. The compiler must reject these modifiers in every local, field, element, static, return, or capture position. Locally declared alias bindings are reserved for a later stage and are described in Section 4.5.2.

Alias fields and alias returns are not permitted:

```ska
fn get_dog() -> ref Dog; // illegal: alias returns are not a language feature
class Kennel {
    ref dog: Dog;         // illegal: alias fields are not a language feature
}
```

`ref name: T`:

- provides read-only access to an existing object for the duration of the call;
- may bind to an inline `T`;
- may bind to the pointee of a `shared T`;
- may call read-only instance methods but not `mut fn` methods on the aliased object or its inline subobjects;
- cannot assign fields of the aliased object or pass the object or its inline subobjects as mutable alias arguments;
- cannot be assigned or rebound;
- cannot escape the call.

`mut ref name: T`:

- provides mutable access to an existing object for the duration of the call;
- may bind to a mutable inline `T`, including a `final` inline field reached through mutable containing-object access, because a mutable alias cannot replace the whole aliased object;
- may bind to the pointee of a `shared T` handle even when the handle is stored in a `final` field, because finality of the handle is shallow;
- may call both read-only `fn` methods and mutable `mut fn` methods;
- cannot be assigned or rebound;
- cannot escape the call;
- does not imply exclusive access.

All alias bindings, including the future local form, obey these invariants:

- an alias is initialized once and its identity cannot be rebound;
- an alias never owns the referenced object and cannot itself be copied as an ordinary value; the underlying `T` may still be copied when a value context requests it;
- an alias cannot be stored in a field, array element, static variable, heap object, or closure, and cannot be returned;
- an alias is confined to a statically apparent lexical or call scope and cannot escape that scope;
- the source place must remain alive and at a stable address for the entire alias scope, using a compiler-managed anchor when ownership alone does not guarantee this;
- mutable access cannot be obtained from a read-only source binding;
- mutable aliases are non-exclusive and may overlap other read-only or mutable aliases;
- conditionally alive storage, such as an optional payload, requires a dedicated scoped binding rule that prevents the payload from disappearing while aliased.

These restrictions make alias validity syntax-directed. They apply to parameter aliases in the first implementation and constrain the design of local aliases when those are added. Skald does not require general lifetime inference or an exclusivity-based borrow checker.

Example:

```ska
fn rename(mut ref dog: Dog, name: Str) -> unit {
    dog.name = name;
}

var d: Dog = Dog();
var s: shared Dog = new Dog();

rename(d, "Ada");
rename(s, "Turing");
```

Aliasing is allowed:

```ska
fn swap_names(mut ref a: Dog, mut ref b: Dog) -> unit {
    var tmp: Str = a.name;
    a.name = b.name;
    b.name = tmp;
}

var dog: Dog = Dog();
swap_names(dog, dog); // allowed; both parameters refer to the same object
```

This may produce surprising program behavior. It remains memory-safe because alias parameters cannot outlive the call and because the caller keeps the storage behind every alias alive until the call returns.

Read-only access is an access restriction, not a guarantee that the object remains observably unchanged. Another mutable alias parameter may mutate the same object during the call. Code using `ref name: T` simply cannot perform that mutation through that name.

#### 4.5.1 Borrow Anchors

Every argument bound to a `ref` or `mut ref` parameter has a **borrow anchor** owned by the caller. The anchor guarantees that the storage containing the aliased object remains alive for the complete dynamic execution of the call, including nested calls and exceptional cleanup. An alias parameter is still passed as a non-owning address; the anchor is caller-side state and is not part of the callee-visible binding.

Anchor selection is based on the source expression and its storage provenance:

- an inline local, inline value parameter, or inline static object is anchored by its existing storage;
- a pointee borrowed through a direct `shared T` local or `shared T` value parameter is anchored by that existing shared handle;
- a pointee borrowed from a replaceable shared place, such as a shared field, shared array element, or mutable shared static variable, is anchored by copying that handle into a hidden caller temporary;
- an inline field or base subobject reached through a shared object is anchored by a shared handle to the allocation that physically contains it;
- an inline array element is anchored by the array storage that physically contains it;
- an inline or shared temporary used as a borrowed argument has its lifetime extended until the call completes;
- forwarding an existing alias parameter to a nested call reuses the outer call's lifetime guarantee and does not create ownership from the alias.

A stable shared local is the common zero-overhead heap-object case:

```ska
var dog: shared Dog = new Dog();
inspect(dog); // dog itself keeps the pointee alive; no shared copy is required
```

The callee cannot rebind a shared local belonging to its caller. Rebinding some other shared handle to the same allocation cannot destroy the pointee while the caller's local handle remains alive.

A replaceable shared place requires a hidden shared copy because code executed by the call may reach and overwrite the original place through another alias:

```ska
inspect(owner.dog); // owner.dog has type shared Dog
```

Conceptually, but not as user-visible source syntax, the caller lowers this as:

```ska
var __borrow_guard: shared Dog = owner.dog;
inspect_raw_address_of_pointee(__borrow_guard);
// __borrow_guard is released after normal or exceptional call completion
```

The hidden copy performs an ordinary `shared` retain and release. It does not allocate another pointee.

If the aliased value is an inline field inside a shared object, the containing allocation is anchored instead:

```ska
class Owner {
    dog: Dog;
}

inspect(registry.current_owner.dog);
```

If `registry.current_owner` is a replaceable `shared Owner` place, the conceptual lowering is:

```ska
var __owner_guard: shared Owner = registry.current_owner;
inspect_raw_address_of_inline_field(__owner_guard, dog);
// __owner_guard is released after the call
```

The guard is a hidden `shared Owner` handle held in the caller's activation record or a register. It is not inserted into `registry`, stored beside `dog`, or found by walking the object graph. The compiler knows from the expression path that `dog` is physically contained in the `Owner` allocation. Even when the `shared Owner` handle is loaded from deep within a global structure, the compiler evaluates that lookup, copies or lifetime-extends the resulting handle, and then calculates the inline field address from the guarded allocation.

If a function or indexing operation returns a `shared Owner` value, the returned shared temporary itself may serve as the anchor:

```ska
inspect(registry.find_owner(id).dog);
```

Here the result of `find_owner` remains alive until `inspect` returns. No additional shared copy is required solely for borrowing if the returned temporary already owns the allocation.

The compiler establishes each required anchor as part of evaluating the corresponding argument, before later evaluation or user code can invalidate the source place. Hidden anchors are destroyed after the call in the ordinary cleanup order. Multiple borrows may use the same anchor; implementations may coalesce redundant hidden guards when doing so preserves observable retain, release, and destruction behavior.

Anchor selection is syntax-directed and local to expression lowering. It does not require a runtime ownership search, object-graph traversal, interprocedural lifetime inference, or general borrow checking. Safe code can maintain this property because aliases cannot be stored or returned and raw pointer construction is unavailable.

The initial language does not allow an alias to target a conditionally alive payload, such as the contained `T` inside a `T?`, if another alias could remove that payload during the call. A later presence-binding design may add such aliases together with rules that preserve the payload lifetime.

#### 4.5.2 Deferred Local Alias Bindings

Locally declared aliases are expected in a later language stage, but are not accepted by the first implementation. The reserved design direction is:

```ska
ref local_dog: Dog = existing_dog;
mut ref mutable_dog: Dog = existing_dog;
```

A local alias would use the same binding semantics as an alias parameter, except that its lifetime would be its statically apparent lexical scope rather than one call. In addition to the common invariants in Section 4.5, it must obey these restrictions:

- the declaration has exactly one initializer, and neither ordinary assignment nor control-flow merging can rebind the alias;
- an inline source place must have a storage scope that encloses the complete alias scope, and no operation may end or relocate that place while the alias exists;
- a source reached through shared ownership is protected by a compiler-managed shared anchor for the complete alias scope when the original handle is not itself guaranteed to remain available;
- a temporary source has its lifetime extended through the complete alias scope;
- optional payloads and any other conditionally alive subobjects cannot be directly aliased without a dedicated scoped binding construct;
- the alias remains unusable in fields, elements, statics, returns, captures, heap storage, and every other escaping position.

These rules permit an inline local alias to lower to an ordinary address with no allocation or reference-count operation. A local alias reached through shared ownership may additionally require a hidden retained handle, just as a call argument may require a borrow anchor. The compiler chooses the anchor from the initializer expression; it does not infer arbitrary lifetimes or search an object graph.

### 4.6 Optional Types

**Specification status:** provisional and intentionally incomplete. Optionals are deferred until after the first vertical compiler slice. This section reserves the `T?` and `none` design direction, but it does not yet define a usable optional feature.

Before optionals are implementation-ready, the specification must define presence testing and binding, extraction by copy or borrow, conversion from `T` to `T?`, type inference for `none`, copy/assignment/destruction behavior, nested optionals, subtype conversions, and the interaction between optional payload lifetime and borrowing. Examples in this section are design sketches rather than a complete normative contract.

Optionality is explicit and part of the type.

```ska
var dog: Dog? = maybe_get_dog();
var heap_dog: shared Dog? = maybe_get_shared_dog();
```

`T?` means either a `T` value or no value.

Plain non-optional values are never null:

```ska
var dog: Dog;             // always contains a Dog after definite initialization
var heap_dog: shared Dog; // always contains a valid shared handle
```

Optionality applies to the complete preceding type:

```ska
Dog?              // optional inline Dog
shared Dog?       // optional shared Dog handle
```

Because `ref` and `mut ref` are binding modes rather than type constructors, alias parameters whose object type is optional are written as:

```ska
fn inspect(ref dog: Dog?) -> unit;
fn rename_if_present(mut ref dog: Dog?) -> unit;
```

Here the parameter aliases a `Dog?` place: the optional container, rather than only its conditionally alive payload, is the object type of the binding. The alias itself is not optional and always designates that place. Since aliases are bindings rather than values, optional alias values are not part of the model.

Later optional presence-binding syntax may introduce a scoped alias to the contained `Dog`. Such an alias must obey the stability rules in Section 4.5 so that the payload cannot disappear during its scope.

The draft spelling for the empty optional value is:

```ska
none
```

Using a value of type `T?` requires explicit presence handling. The exact pattern-matching or unwrap syntax is deferred.

### 4.7 Array Types

**Specification status:** provisional and intentionally incomplete. Arrays, indexing, slicing, and array iteration are deferred until after the first vertical compiler slice. The syntax and properties below record the current design direction, not a complete implementation contract.

Before arrays are implementation-ready, the specification must finalize construction forms, copy and assignment semantics, element and slice mutation, destruction and partial-construction cleanup, nested-array behavior, element borrowing, iteration behavior, and the observable consequences of the chosen storage model.

Skald uses a built-in fixed-size array type constructor:

```ska
u8[]
i64[]
Dog[]
shared Dog[]
Dog[][]
```

Array construction:

```ska
var bytes: u8[] = u8[](1024);
var dogs: Dog[] = Dog[](8);
var heap_dogs: shared Dog?[] = shared Dog?[](8);
```

Default array construction is valid only when the element type has a default value:

```ska
var bytes: u8[] = u8[](1024);              // ok: u8 defaults to 0u8
var dogs: Dog[] = Dog[](8);                // ok if Dog is default-constructible
var maybe_dogs: Dog?[] = Dog?[](8);        // ok: elements default to none
var heap_dogs: shared Dog?[] = shared Dog?[](8); // ok: elements default to none
var required: shared Dog[] = shared Dog[](8);    // illegal: shared Dog has no default value
```

Array properties:

- size is fixed after construction;
- indexing is bounds-checked;
- slicing copies into a new array;
- nested arrays are jagged arrays;
- `T[]` is a built-in type constructor, not user-defined generics.
- array storage placement is an implementation detail.
- fixed-size array elements do not relocate during the array's lifetime;
- borrowing an inline element anchors the array storage for the call;
- borrowing the pointee of a replaceable `shared T` element copies that element handle into a hidden borrow anchor.

The language does not require arrays to be physically stack-allocated or heap-allocated. An implementation may choose direct inline storage, stack storage, heap-backed storage, or specialized variants based on element type, size, escape behavior, and whether the length is statically known. Observable construction, destruction, copying, indexing, and bounds-checking semantics must remain the same.

Default element initialization:

- primitive elements use primitive default values;
- inline object elements are default-constructed;
- non-optional `shared T` elements have no default value and therefore cannot be default array-constructed;
- optional elements default to no value.

Later versions may add explicit initialization forms for non-defaultable element types, such as initializer lists, fill constructors, or per-element generator syntax. For a later array-focused MVP slice, the current direction is that `shared Dog[](8)` is illegal and `shared Dog?[](8)` is legal.

### 4.8 Str

`Str` is the built-in immutable string type.

`Str` is a small inline value, not a garbage-collected reference. It is backed by immutable byte storage containing `u8` bytes. The language assigns no Unicode or text-normalization semantics initially; string contents are raw bytes.

Conceptual shape:

```ska
class Str {
    private storage: shared StrStorage;
    private start: u64;
    private length: u64;
}
```

`StrStorage` is a compiler/runtime-recognized storage object or equivalent internal representation. It is not required to be exposed as an ordinary public standard-library type.

Properties:

- `Str` is immutable.
- String bytes cannot be mutated through a `Str`.
- Copying a `Str` copies only the small descriptor/handle, not the bytes.
- Slicing a `Str` may share the same immutable backing storage.
- String manipulation should be implementable mostly in Skald code using `Str` methods and separate mutable builder/buffer types.
- A future `StrBuf` or byte-buffer type should provide mutable construction and editing, then produce an immutable `Str`.

String literals have type `Str`.

```ska
var greeting: Str = "hello";
```

A string literal evaluates to a `Str` value whose bytes are stored in compiler-emitted immutable static storage. The compiler must not lower each literal use by allocating and copying a fresh `u8[]` and running an ordinary `Str` constructor.

Acceptable implementation strategies include:

- emit static immutable bytes and create a small `Str` descriptor at each use;
- emit a static canonical `Str` descriptor and copy that descriptor at each use;
- use an internal immortal/static storage kind whose release operation is a no-op.

All strategies must preserve the same observable semantics:

- no per-use byte copy for literals;
- no per-use heap allocation for literal bytes;
- literal bytes are immutable;
- copying a literal `Str` has the same behavior as copying any other `Str`.

---

## 5. Declarations

### 5.1 Local Variables

Initial local declarations use the following syntax:

```ska
var name: Type = initializer;
```

`var` creates an owning value binding with its own storage. Initializing it from another inline value copies that value; initializing a `shared T` local copies the handle. It does not create an alias binding. The future local `ref` and `mut ref` forms are described separately in Section 4.5.2.

Examples:

```ska
var count: i64 = 0;
var dog: Dog = Dog();
var maybe_dog: Dog? = none;
var heap_dog: shared Dog = new Dog();
```

Ordinary statements and declarations use semicolon terminators.

Variables must be definitely initialized before use. Uninitialized values must not be observable.

### 5.2 Functions

Function declarations:

```ska
fn name(param1: Type, param2: Type) -> ReturnType {
    ...
}
```

Binding modifiers precede the parameter name; they are not written as part of `Type`.

Parameters may use value bindings, `shared` value types, and alias-binding modes:

```ska
fn copy_in(dog: Dog) -> unit;
fn borrow_in(ref dog: Dog) -> unit;
fn mutate_in(mut ref dog: Dog) -> unit;
fn share_in(dog: shared Dog) -> unit;
```

Parameter passing:

- `T` copies the argument into the callee.
- `shared T` copies the shared handle into the callee.
- `ref name: T` binds the parameter name as a call-scoped read-only alias.
- `mut ref name: T` binds the parameter name as a call-scoped mutable alias.

At a call site, both inline `T` storage and a `shared T` pointee can supply the place for an alias parameter. The callee declares only the access mode it needs; it does not provide separate overloads for inline and shared ownership.

Return values may be primitives, inline objects, optionals, arrays, function values, or `shared` handles. Alias bindings cannot be returned.

### 5.3 Function Values

Skald initially uses capture-free function values.

Type syntax:

```ska
fn(i64, i64) -> i64
fn(ref Dog) -> unit
fn(mut ref Dog) -> unit
```

In function-type syntax, `ref T` and `mut ref T` record the unnamed parameter binding mode. They do not construct reference types.

Function values may refer to:

- top-level functions;
- static class methods.

Out of scope initially:

- captured-variable closures;
- instance method values;
- interface method values;
- lambda literals.

Function types are invariant and require exact parameter and return types.

### 5.4 Classes

Class declarations:

```ska
class Dog extends Animal implements Named {
    name: Str;

    init(name: Str) {
        self.name = name;
    }

    virtual fn speak() -> unit {
        ...
    }
}
```

Classes support:

- fields;
- `init` declarations, including copy constructors;
- `assign` declarations for copy assignment;
- `destroy` declarations;
- instance methods;
- static methods;
- static variables;
- `private` members;
- `final` fields;
- single inheritance via `extends`;
- interface conformance via `implements`;
- explicitly declared virtual methods;
- explicit `override` for overridden methods.

`init`, `assign`, and `destroy` are contextual special-member introducers when used directly in a class body with their corresponding declaration syntax. They are not globally reserved identifiers and the special declarations do not introduce ordinary methods. Because ordinary methods require `fn`, the same spellings remain available to user code:

```ska
class Example {
    init() { ... }                    // special initialization member
    assign(ref other: Example) { ... } // special assignment member
    destroy { ... }                   // special destruction member

    fn init() -> unit { ... }         // ordinary method named init
    fn assign(value: i64) -> unit { ... }
    fn destroy() -> unit { ... }

    init_count: i64;                  // ordinary field
}
```

The words may likewise be used for locals, parameters, top-level functions, and other ordinary identifiers where the special class-member grammar is not being parsed.

#### 5.4.1 Instance-Method Receiver Mutability

Every instance method has an implicit receiver access mode. Ordinary `fn` methods have a read-only receiver by default. Methods that may mutate the receiver are declared with `mut fn`:

```ska
class Dog {
    name: Str;

    fn get_name() -> Str {
        return self.name;
    }

    mut fn rename(name: Str) -> unit {
        self.name = name;
    }
}
```

The modifier describes access through the implicit `self` receiver:

- in an ordinary instance `fn`, `self` is read-only;
- in a `mut fn`, `self` is mutable;
- when combined with existing member modifiers, `mut` immediately precedes `fn`, for example `private mut fn`, `virtual mut fn`, and `override mut fn`;
- `init`, copy assignment, and `destroy` members have an implicitly mutable `self` and do not use the `mut fn` spelling;
- static methods and top-level functions have no receiver, so `mut` is not valid on them;
- receiver mutability is not part of capture-free function-value type syntax because instance method values are out of scope initially.

In a read-only instance method, code cannot:

- assign an instance field of `self`;
- call a `mut fn` method on `self`;
- mutate an inline field, base subobject, or inline array element contained in `self`;
- pass `self` or any of those inline subobjects to a mutable alias parameter.

It may read and copy fields, call read-only methods, allocate objects, perform I/O, and modify separate objects or static state when otherwise permitted. Receiver read-only access is not a purity or side-effect annotation.

Read-only access is shallow across shared ownership. A read-only method cannot replace a `shared T` field of `self`, but receiver read-only access does not extend through the handle to the `T` pointee. The pointee remains mutable through a copied or otherwise available shared handle, even if runtime aliasing causes that pointee to be the same object as one reached through another read-only path:

```ska
class Owner {
    child: shared Dog;

    fn get_child() -> shared Dog {
        return self.child; // copies the handle; allowed
    }

    fn rename_child() -> unit {
        self.child.rename("Rex"); // allowed: mutates the separate Dog allocation
    }

    fn invalid_replace() -> unit {
        self.child = new Dog(); // illegal: mutates Owner storage
    }
}
```

Access through a `ref` parameter uses the same read-only receiver rules. Access through a `mut ref` parameter, a mutable inline object, or a `shared T` pointee may call both `fn` and `mut fn` methods. Restricting mutable access to read-only access is allowed and requires no runtime conversion; granting mutable access through an existing read-only path is illegal.

```ska
fn inspect(ref dog: Dog) -> Str {
    dog.rename("Rex");       // illegal: rename has a mutable receiver
    return dog.get_name();   // allowed
}

fn update(mut ref dog: Dog) -> unit {
    var old_name: Str = dog.get_name(); // allowed
    dog.rename("Rex");                  // allowed
}
```

The initial language has no separate `const T` type syntax. The compiler tracks receiver access mode during type checking, and binding an object through `ref name: T` restricts the available access rather than casting the object to a different type. This does not create a distinct runtime object representation, change object layout, or emit a runtime cast. Access mode propagates through inline fields and base subobjects because those values are physically part of the receiver.

`final` is independent of receiver mutability and is shallow. A final field can be initialized during construction but cannot later be reassigned as a whole. A final inline object field may still be changed through its own `mut fn` methods when reached through a mutable containing object, and a final `shared T` field may still be used to mutate its separately allocated pointee. Finality prevents whole-field reassignment; it does not recursively freeze the field's internal state or an object graph.

#### 5.4.2 Restricted Stage-0 Inline-Object Profile

**Implementation status:** this restricted profile is implemented end to end
for Linux x86-64. This subsection narrows the broader class model above for the
current compiler; it does not remove features from the eventual language.

The first implemented object profile contains nominal top-level classes,
primitive fields, one explicit initializer, local inline storage, direct field
access, and statically dispatched instance methods. Its canonical parser-facing
grammar is in [`grammar/README.md`](../grammar/README.md).

The profile has these declaration and name rules:

- `class`, `self`, and `mut` are keywords; `init` remains a contextual
  special-member introducer and an ordinary identifier elsewhere;
- classes and top-level functions share one non-overloaded declaration
  namespace, and declarations are collected before bodies are resolved;
- fields and ordinary methods share one non-overloaded ordinary-member
  namespace within their class;
- the special initializer occupies a separate slot, so `init() { ... }` may
  coexist with `fn init(...) -> ...` or a field named `init`; an ordinary field
  and method still cannot share that name with each other;
- all fields and methods are accessible in this profile because access control
  is deferred;
- `self` exists only in the current initializer or instance-method body, denotes
  the current object place, and cannot be shadowed;
- class identity is nominal, and a class declared later in the same source file
  may be selected after top-level declaration collection.

Every executable field has one of the primitive types `i64`, `u64`, `u8`,
`f64`, or `bool`, or an acyclic inline class type. Class-typed field
declarations, nominal resolution, HIR declaration metadata, containment-cycle
validation, direct construction, and initializer liveness from Section 5.4.4
are implemented. Nested scalar access, receivers, and alias arguments are
lowered through complete identity paths and execute against recursively laid
out x86-64 storage. Base classes, interfaces, static members, `final`, access
modifiers, virtual/override declarations, and `assign` remain rejected by the
current compiler. Section 5.4.5 defines the implemented restricted `destroy`
extension, and Section 5.4.6 freezes the staged object-value extension. Empty
classes are valid.

Every class declares exactly one explicit, non-overloaded `init`. It has an
implicit mutable `self`, takes only by-value primitive parameters, and returns
`unit` implicitly. No initializer is synthesized, including for an empty
class. Copy and delegating initializers are unavailable.

The initializer body is a straight-line sequence containing only direct field
initializations:

```ska
self.field = primitive_expression;
self.child = Child(arguments);
```

The expression may use primitive literals, initializer parameters, fields of
`self` that have already been initialized, grouping, implemented primitive
operators, and calls to top-level defined or external functions with supported
primitive signatures. A class-typed field requires ungrouped construction of
its exact class directly in that field's storage. It cannot use `self` as a
complete value or contain another object-valued expression. Initializer bodies
have no local declarations, nested blocks,
conditionals, effect-only call statements, or explicit return.

Every field is assigned exactly once before normal completion. Assignment order
need not equal declaration order, but a field cannot be read before its own
assignment. Unknown, duplicate, missing, and type-mismatched field assignments
are compile-time errors. An empty class therefore uses an explicit empty
`init() {}`.

Construction is permitted only as the complete initializer of a newly declared
local of the exact class type:

```ska
var counter: Counter = Counter(40);
```

The construction syntax is parsed as the same postfix call shape used by an
ordinary function call; resolution distinguishes a class from a function in
their shared namespace. A constructor expression is not a general value in
this profile: it cannot be passed, returned, grouped for another use, assigned
to an existing place, used as a receiver, or nested inside another expression.
Object locals may be declared in any already-supported lexical block.

Destination storage is reserved before arguments are evaluated but is not yet
a live object. Arguments evaluate completely from left to right. `init` begins
only afterward, its field assignments execute in source order, and normal
completion establishes the complete object's lifetime. Checked exceptions are
not available, so there is no recoverable failed-construction path. When the
local's lexical storage scope ends, its lifetime ends; primitive fields and the
absence of `destroy` make that event unobservable in this profile.

Fields may be read as primitive values and assigned through `self` or a local
inline object. Grouping around that receiver place is transparent. General
local assignment, compound/chained assignment, object assignment, and an
arbitrary expression on the left-hand side remain unavailable. Since object
fields are excluded, every valid source field place has one field projection.

An ordinary instance method has primitive by-value parameters and a primitive
or `unit` result. Methods are unique by name and dispatch directly. A read-only
`fn` method may read fields and call read-only methods on `self`; a `mut fn` may
also assign fields and call mutable methods. A local object provides mutable
receiver access and may call either kind. `init` cannot call an instance method
because the complete receiver is not live yet. Method bodies otherwise use the
already implemented primitive statements, expressions, calls, locals, and
conditionals.

Object value parameters, results, ordinary value arguments, and FFI types;
general object temporaries; copying, assignment, moves, slicing, and elision;
inheritance, interfaces, polymorphism, casts, and dynamic metadata; `shared`,
`new`, and borrow anchors are deferred beyond this profile. The restricted
alias-parameter extension is defined in the following subsection.

The profile adds these observable evaluation-order rules:

- a method receiver is evaluated before its explicit arguments;
- explicit arguments are evaluated left to right;
- a field receiver place is evaluated before its field is loaded;
- field assignment evaluates its receiver place, then its complete right-hand
  value, and only then performs the store;
- construction reserves its destination, evaluates arguments left to right,
  invokes `init`, and makes the destination live only after normal completion.

Section 5.4.5 extends these rules with implemented MIR cleanup planning for
owning locals on the currently supported normal exits and x86-64 execution of
verified destruction plans. General temporaries, other control-flow exits,
shared ownership, aliases requiring anchors, and checked exceptions remain
later work.

#### 5.4.3 Restricted Stage-0 Alias-Parameter Profile

**Implementation status:** this restricted profile is implemented end to end
on Linux x86-64. Resolved IR carries binding mode separately from nominal
class identity; alias names have stable parameter identities and may form
existing object-place bases. HIR carries explicit
value/read-only-alias/mutable-alias parameter modes and one source-ordered
sequence of value or place arguments. Type checking enforces exact class
identity, place eligibility, access capability, forwarding, and non-escaping
restrictions. Verified MIR uses indirect alias-parameter places, and the
backend passes one pointer per alias using the internal ABI described below.
This profile extends the restricted inline-object profile in Section 5.4.2. It
does not implement every alias source described by the broader model in
Section 4.5.

The parameter grammar added by this profile is:

```text
parameter       = value-parameter | alias-parameter
value-parameter = identifier ":" primitive-type
alias-parameter = ["mut"] "ref" identifier ":" class-name
```

`ref` and `mut ref` are parameter binding modes, not type constructors. The
bound name's type is the named class, and the mode is represented separately
from that type. `ref` is a keyword; `mut ref` is the only mutable spelling.
`ref mut`, repeated modifiers, and a binding mode in a local, field, result,
static, element, or capture position are invalid.

Alias parameters are accepted on internally defined top-level functions,
instance methods, and initializers. An external declaration cannot contain an
alias parameter. Ordinary by-value parameters retain the implemented
primitive-only restriction; a class name without `ref` is not an object value
parameter. This profile accepts only an exact concrete class as the designated
type. Primitive, `unit`, optional, array, `shared`, interface, and function
types, along with inheritance and implicit conversions, remain outside the
profile.

An argument for an alias parameter must be an existing, already-live inline
class place of the exact designated class. The supported place sources are:

- a directly constructed inline local;
- `self` in an instance method, subject to that method's receiver access;
- an existing alias parameter forwarded to another call;
- grouping around one of those places.

No other source expression is converted to an alias place. In particular,
construction does not create a borrowable temporary, and object fields, array
elements, static objects, optional payloads, and shared pointees are not yet
available alias sources. An initializer's destination is not live while its
body executes and its `self` cannot be passed as an alias. An initializer may
receive an alias parameter and may read its primitive fields while initializing
the new object's fields, subject to the existing straight-line initializer-body
rules.

Section 5.4.4 freezes the next profile's extension from object fields to alias
sources. Until that profile is implemented, the source forms in this section
remain the complete accepted alias-source set.

An initializer of the enclosing class with the broader copy-constructor
signature `init(ref other: T)` may therefore be written in this profile. It is
invoked only by the existing explicit direct-local construction form
`var copy: T = T(source);`. This does not enable implicit copy construction,
ordinary object value arguments, synthesized copying, assignment, or any other
general copy context.

Place access has two capabilities, mutable and read-only. Mutable access may be
restricted to read-only access without a runtime conversion; read-only access
cannot be promoted to mutable access.

- An inline local provides mutable access.
- A method's `self` provides its declared receiver access.
- A `ref` parameter provides read-only access: fields may be read, read-only
  methods may be called, and the place may be forwarded only to another `ref`
  parameter.
- A `mut ref` parameter provides mutable access: fields may be read or written,
  either receiver mode may be called, and the place may be forwarded to either
  alias mode.

The access restriction belongs to the binding, not to a different const class
type or runtime representation. It is shallow in the same way as receiver
mutability. This profile still has only primitive fields, so access does not
yet propagate through an inline object-field chain.

Aliases are deliberately non-exclusive. Multiple read-only or mutable alias
arguments may designate the same object, and the compiler performs no overlap
analysis. A read-only alias prevents mutation only through that binding; it
does not guarantee that another alias cannot mutate the object during the
call.

Alias arguments participate in the existing source evaluation order. A method
receiver is evaluated first and explicit arguments are then processed from
left to right in one sequence. Selecting one of the supported alias places has
no user-visible effect, but the compiler representation must not split value
and alias arguments into reorderable lists. Forwarding preserves the same
object address and does not create an alias value.

Every supported place is stable for the complete call: a local remains in its
declaring activation, `self` is kept alive by its caller, and a forwarded alias
inherits the enclosing call's guarantee. Consequently this profile needs no
allocation, ownership-provenance tag, retain/release operation, hidden borrow
anchor, graph search, lifetime inference, or exclusivity-based borrow checker.
The alias cannot escape because it is not a value and cannot be stored,
returned, captured, assigned, rebound, or converted to `shared`.

The target-independent compiler contract for this profile is:

- syntax, resolved IR, HIR, and MIR carry parameter binding mode explicitly and
  separately from the underlying nominal type;
- resolution assigns ordinary stable `ParameterId` identities and selects
  names, while type checking alone decides place eligibility, exact type, and
  access sufficiency;
- HIR and MIR keep value and alias arguments in one ordered sequence, with an
  alias argument represented as a typed place rather than an object value;
- a MIR alias parameter is an indirect place base whose incoming payload is an
  address, distinct from owning local object storage;
- field projection remains semantic through `FieldId`; no target offset or
  register enters MIR;
- MIR verification checks declaration/definition agreement, argument kind and
  type, place ownership and liveness, projection validity, access sufficiency,
  and the exclusion of aliases from external declarations and scalar value
  operations before a backend is invoked.

Local alias declarations, primitive alias parameters, object value parameters
and results, shared sources and borrow anchors, polymorphism, whole-object
replacement, and alias-bearing function values remain deferred.

#### 5.4.4 Frozen Class-Typed Inline-Field Profile

**Implementation status:** implemented; IOF0–IOF6 of the
[archived Class-Typed Inline Object Fields Roadmap](archive/INLINE_OBJECT_FIELDS_ROADMAP.md)
are complete. The compiler accepts and resolves class-typed field declarations,
records canonical HIR field types, rejects recursive containment before target
selection, represents nested object places as root bindings plus ordered
semantic field identities, and distinguishes direct subobject construction
from scalar stores while enforcing initializer liveness. The type checker
supports nested scalar fields, method receivers, and exact-class alias
arguments with one root-derived access capability. Verified MIR retains those
paths as semantic field identities, and the x86-64 backend resolves them with
checked target offsets for deterministic native execution. The parser-facing
extension is recorded in
[`grammar/README.md`](../grammar/README.md#frozen-staged-extension-class-typed-inline-fields).

This profile extends the restricted stage-0 object and alias profiles with
class-typed fields, direct construction into those fields, recursively
projected places, and alias arguments designating contained subobjects. It does
not add object values, copying, destruction, inheritance, or shared ownership.

##### Field declarations and containment

The field grammar for this profile is:

```text
field-declaration = identifier ":" field-type ";"
field-type        = primitive-type | class-name
```

`unit` remains invalid as a field type. A `class-name` must resolve to an exact
concrete class in the same compilation unit. It cannot name a function, and an
unknown name is an error. Top-level collection continues to precede field-type
resolution, so a field may name a class declared later in the source file.
Class identity remains nominal.

A class-typed field contains one complete inline subobject. It is not a
pointer, nullable handle, alias, or separately allocated value. Two fields of
the same class contain two distinct subobjects. An empty class used as a field
still has a nonzero addressable target extent.

The directed graph whose vertices are classes and whose edges are class-typed
fields must be acyclic. Both a direct field of the enclosing class type and an
indirect cycle are invalid independent of target layout:

```ska
class Direct {
    child: Direct; // invalid direct containment cycle
    init() { self.child = Direct(); }
}

class Left {
    right: Right;
    init() { self.right = Right(); }
}

class Right {
    left: Left; // invalid indirect containment cycle
    init() { self.left = Left(); }
}
```

Source-level cycle checking occurs after field types have stable `ClassId` and
`FieldId` identities but before HIR is accepted, MIR is lowered, or a target is
selected. Diagnostics emit one primary error for each recursive strongly
connected containment component. Components are ordered by the earliest class
declaration they contain. The displayed representative cycle begins with the
earliest declared participating class and follows a deterministic
field-declaration-order path back to that class. The diagnostic identifies
every field edge in that path. A backend must retain defensive cycle rejection
for malformed or hand-built IR, but a valid source program never relies on the
backend to establish containment legality.

Acyclic diamonds, repeated class field types, forward dependencies, and empty
subobjects are valid. Physical layout may compute dependencies in any order,
but language-visible field order remains source declaration order.

##### Direct field construction and liveness

The restricted initializer body remains a straight-line sequence of direct
field-initialization statements. The meaning of the right side depends on the
declared field type:

```ska
class Child {
    value: i64;
    init(value: i64) { self.value = value; }
}

class Parent {
    tag: i64;
    child: Child;

    init(tag: i64, child_value: i64) {
        self.tag = tag;                  // primitive initialization
        self.child = Child(child_value); // construction in field storage
    }
}
```

`self.field = primitive_expression;` initializes a primitive field exactly as
in Section 5.4.2. When `field` has class type `T`, the complete right side must
be the ungrouped direct construction `T(arguments)`. Its constructor class must
match the field's exact nominal type. The construction does not produce an
object value and the statement is not assignment to a live object.

The only new construction destination is a direct field of the current
initializer's `self`. Grouping around `self` is transparent, but a field
projection cannot precede the destination field. Construction into an existing
local, method receiver, alias parameter, already-live field, or deeper path is
invalid. Each nested class's own initializer remains solely responsible for
constructing that class's direct fields.

Every direct primitive or class field must be initialized exactly once before
the enclosing initializer completes. Source initialization order need not
match declaration order. Duplicate initialization is invalid at the second
statement, and each missing field is diagnosed at its declaration. A scalar
right side for a class field, a construction for a primitive field, a grouped
construction, and construction of the wrong class are distinct invalid forms
and must not fall back to ordinary object-value type checking.

Destination storage exists before constructor arguments are evaluated, but a
class field is not live while its arguments or nested initializer execute. It
becomes a complete live subobject only when the nested initializer returns
normally. The enclosing `self` becomes a live complete object only after all
of its direct fields are initialized and its initializer returns normally.

Before a direct class field is live, no path beginning with that field may be
read, used as a method receiver, or passed as an alias. After it becomes live,
later initializer expressions may:

- read its primitive fields at any nested depth;
- call a method on the completed subobject or one of its completed
  subobjects; and
- pass the completed subobject or one of its completed subobjects to an exact-
  class `ref` or `mut ref` parameter.

The same initializer expressions may call methods through already-live alias
parameters received from the caller. The incomplete enclosing `self` remains
invalid as a complete method receiver or alias argument. The initializer's
statement grammar is otherwise unchanged: local declarations, nested blocks,
conditionals, explicit returns, and effect-only call statements remain
unavailable. Construction remains unavailable as a nested expression or
ordinary call argument.

There are no checked exceptions or other recoverable construction failures in
this profile. Definite-initialization state changes only after a nested
initializer returns normally. Later destruction and exception work may attach
cleanup to these explicit completion points without changing when a subobject
becomes live.

##### Nested places, receivers, and aliases

An object place consists of one supported live root followed by zero or more
class-field projections. Its root is:

1. a directly constructed inline local;
2. a live method `self`;
3. an existing `ref` or `mut ref` parameter; or
4. grouping around one of those roots or a projected place.

A class-typed endpoint remains a place. It may be a method receiver or alias
argument, but it is not an expression value. Selecting a final primitive field
loads or stores a scalar according to the existing expression or assignment
context. These forms therefore become valid:

```ska
var value: i64 = outer.inner.value;
outer.inner.value = 42;
outer.inner.observe();
update(outer.inner);
```

The first form requires a primitive terminal field. The second requires
mutable access. The third selects a method of the terminal class. The fourth
requires an exact-class alias parameter. A class field remains invalid as a
scalar expression, return value, ordinary value argument, value local
initializer, or whole-object assignment source or destination.

The root binding determines access for the complete inline path:

- an inline local, mutable method `self`, or `mut ref` parameter provides
  mutable access to every contained subobject;
- read-only method `self` or a `ref` parameter provides read-only access to
  every contained subobject;
- read-only access permits primitive reads, read-only method calls, and `ref`
  arguments;
- mutable access additionally permits primitive writes, mutable method calls,
  and `mut ref` arguments.

This propagation is physical containment, not a runtime conversion, recursive
const type, or ownership operation. No path permits whole-object replacement
in this profile. Aliases remain non-exclusive; two arguments may designate the
same contained subobject or overlapping containing/subobject places.

An alias to an inline field remains call-scoped and non-owning. A local or
receiver root keeps its complete containing storage alive for the call. A
forwarded alias root inherits the enclosing call's guarantee. A completed
field borrowed while its parent initializer is active is kept alive by the
destination storage and initializer activation even though the complete parent
is not live yet. These forms require no allocation, provenance tag,
retain/release operation, hidden borrow anchor, graph search, lifetime
inference, or exclusivity checking.

##### Evaluation and phase boundaries

The profile extends the existing observable ordering rules as follows:

- an object-place root is selected before its field projections, and
  projections are selected from left to right;
- a nested method receiver is selected before its explicit arguments;
- all explicit call and constructor arguments are evaluated left to right in
  one sequence, including alias-place selection at its source position;
- primitive field assignment selects the complete destination place, evaluates
  the complete scalar right side, and then stores;
- class-field construction selects and reserves the destination place,
  evaluates arguments left to right, invokes the exact initializer, and marks
  the field live only after normal return.

Current place selection has no user-visible side effects, but IR must preserve
this order so later array elements, shared anchors, temporaries, and cleanup do
not require a semantic rewrite.

Resolution assigns every class and member identity and records the complete
projection path. HIR records the terminal class, root access capability, and
whether the operation is a scalar load/store, receiver call, alias argument,
or construction destination. MIR uses the existing storage base plus ordered
`Field(FieldId)` projections and `MirInitialize` into a destination place.
Class endpoints never become scalar MIR values. Target offsets, alignments,
frame locations, and address arithmetic remain backend-owned.

Required source diagnostics cover at least:

- unknown names and functions used as field types;
- direct and indirect recursive containment;
- a scalar, grouped constructor, or wrong-class constructor used for a class
  field;
- construction into a non-direct, foreign, or already-live destination;
- premature, duplicate, or missing field initialization;
- nested mutation or mutable aliasing through read-only access;
- exact-class alias mismatch; and
- every attempt to use a class-field endpoint as an ordinary object value or
  replace a live whole object.

Diagnostics and dumps use source/stable-identity order. Exact wording may
evolve, but an error must identify the invalid use and the declaration or
earlier initialization that establishes the violated rule; it must not depend
on hash-map iteration or target layout.

##### Boundary with later object-model slices

This profile establishes complete contained subobjects. The frozen destruction
profile in Section 5.4.5 now represents and executes `destroy` bodies,
recursive field plans, initialized-place cleanup state, and cleanup-aware
normal control-flow edges using the completion points preserved here. Failed-
construction and exceptional cleanup remain later work.

There is no implicit or synthesized copy construction or assignment. A user
may pass a field to an existing explicit alias parameter, including an
initializer declared as `init(ref other: T)`, but copying the underlying field
as a value remains unavailable. The staged profile in Section 5.4.6 freezes
later synthesized copy operations as composition of field capabilities rather
than untyped storage.

Inheritance will later add base-subobject dependencies and projections without
changing the rule that by-value containment must have finite layout. Shared
ownership will later anchor an inline field through the allocation that
contains it; this profile has only inline roots whose storage already encloses
the call. Checked exceptions will later destroy only subobjects whose
initializers completed, rather than treating the incomplete enclosing object
as live. None of those future rules changes the source forms or normal-return
liveness boundary frozen here.

#### 5.4.5 Frozen Local Deterministic-Destruction Profile

**Implementation status:** implemented and published by DD0–DD6 of the
[archived Deterministic Destruction Roadmap](archive/DETERMINISTIC_DESTRUCTION_ROADMAP.md).
The parser-facing extension is recorded in
[`grammar/README.md`](../grammar/README.md#restricted-extension-deterministic-destruction).

This profile narrows the broader destruction rules in Section 5.7 to the
compiler's current local-only inline-object model and normal control flow. It
adds an observable end to the lifetimes already established by direct local and
class-field construction. It does not add another construction form, an object
value, or an operation that ends a live object early.

##### Declaration and body contract

The only new source production is:

```text
destructor-declaration = "destroy" block
```

The contextual spelling `destroy` selects a special destruction member only
when it directly introduces that class-member form. The declaration has no
`fn`, name, modifiers, parameter list, result annotation, or semicolon. These
forms are therefore invalid special declarations:

```ska
destroy() {}
destroy -> unit {}
mut destroy {}
destroy;
```

The spelling remains an ordinary identifier everywhere that does not parse a
special member. A field `destroy: i64`, an ordinary `fn destroy() -> unit`, and
locals, parameters, or top-level functions named `destroy` remain valid. The
special member does not enter the ordinary field/method namespace and may
coexist with those declarations.

Each class has one optional destruction-member slot. A second special
`destroy { ... }` declaration in the same class is an error at the second
declaration, with the first declaration identified as the established member.
An absent declaration means an empty user body; it does not suppress recursive
field cleanup.

The destruction body has an implicit mutable `self`, no parameters, and an
implicit `unit` result. Its statement surface is the same as an ordinary
implemented `unit` method: primitive and directly constructed object locals,
nested blocks, conditionals, field assignments, unit-producing calls, method
calls, and `return;` are permitted. A value return is invalid. Falling through
the closing brace and executing `return;` have the same destruction-member
completion semantics.

The receiver is a complete live object for the entire user body. All its
completed inline fields remain live and may be read, mutated, used as receivers,
or passed as aliases according to the existing mutable-access rules. The body
may construct its own local objects, whose scopes are cleaned normally. It may
not construct into any already-live field of `self`, replace a whole object,
use an object as a value, explicitly invoke a special destruction member, or
explicitly end any object's lifetime. An ordinary method named `destroy` is a
separate directly callable method and has no lifecycle effect.

##### Lifetime registration and normal cleanup

Storage reservation does not register cleanup. An owning object local becomes
registered only after all constructor arguments have evaluated, its exact
initializer has returned normally, and the declaration has established the
complete local as live. Registration occurs before execution advances to the
next statement. Primitive locals, alias-parameter homes, method receivers, and
alias arguments are never registered as owning cleanup entries.

The implemented language has no checked exceptions, recoverable initializer
failure, object-producing return expression, or other path that can leave a
partially constructed local while continuing execution. Consequently this
profile emits no failed-construction cleanup. Later exception work will use the
existing per-field and complete-object liveness points to clean only completed
subobjects.

Each runtime activation maintains the semantic equivalent of an initialized-
owning-place stack per active lexical scope. The required observable order is:

1. normal fallthrough from a block destroys that block's registered locals in
   reverse registration order before entering its continuation;
2. only the selected conditional arm executes or registers locals, and its
   child scope is cleaned before control reaches the conditional join;
3. `return` first evaluates and preserves its primitive result, then cleans
   every exited scope from innermost to outermost, using reverse registration
   order within each scope, and only then transfers the preserved result;
4. implicit fallthrough from a `unit` callable cleans its body scope by the
   same rule before returning;
5. a `return;` inside a destruction body first cleans locals owned by that body
   and its nested scopes, then completes the user body and begins field cleanup
   for the object being destroyed.

Source order fixes registration order for declarations that execute in one
scope. Branches do not merge registrations from unexecuted arms. Current place
selection and primitive expressions create no owning temporary cleanup entry.
Calls must finish before cleanup continues, so any alias passed by a cleanup
body remains valid for that complete call and cannot escape it.

##### Complete-object and field order

Destroying one complete object performs these steps exactly once:

1. execute its user destruction body, if present;
2. after that body and all body-local cleanup complete, destroy class-typed
   fields in reverse source declaration order;
3. finish the object's lifetime without copying or deallocating its inline
   storage.

Primitive fields have no destruction step. An absent user body is an empty
first step. Each class-typed field is itself a complete object and recursively
uses the same body-then-reverse-fields procedure. Field destruction order is
declaration order reversed, independent of the order in which the initializer
constructed the fields. Acyclic containment guarantees that this recursion is
finite. Empty classes and classes containing only primitive fields still run a
declared user body exactly once.

No source code runs after field cleanup begins for an object, so a partially
destroyed receiver cannot be observed. Inline storage is not deallocated; its
lexical storage duration simply ends after cleanup.

##### Diagnostics and phase boundary

Required source diagnostic categories are:

- malformed destruction declaration, identifying the required
  `destroy { ... }` shape and the forbidden parameters, result, modifiers, or
  semicolon;
- duplicate destruction member, identifying both the second and first
  declarations;
- value return from a destruction body, identifying its implicit `unit`
  result;
- construction into an already-live receiver field or another unsupported
  object destination;
- attempted use of the special member as a callable or value; and
- attempted explicit early destruction or any retained object-value form.

Diagnostics use source order and stable identities. Exact prose may evolve,
but every category receives a stable diagnostic code at the phase that owns the
invalid construct. Parser recovery must retain later class members and top-
level declarations. No malformed supported source may reach a backend
assertion.

Resolution will assign the special member an owner-qualified lifecycle identity
rather than selecting it by the spelling `destroy` below that phase. HIR will
own source-level receiver and body legality. MIR will explicitly represent
cleanup operations, initialized owning places, and cleanup order on normal
control-flow edges. Recursive field order must be explicit in target-
independent IR or target-independent generated cleanup bodies. A backend may
resolve semantic places to offsets and emit calls, but it must not infer lexical
lifetime, registration state, or language destruction order.

##### Exclusions and extension boundary

This profile adds no object assignment, copy construction, move, object value
parameter/result, object temporary, return storage, elision, explicit destroy
statement, early lifetime end, or replacement through an alias. It adds no
exception edge, unwinding, cleanup pad, panic cleanup, or observable failed-
construction path. It also excludes loops and their exits, arrays, optionals,
statics, globals, inheritance and base subobjects, virtual destruction, dynamic
type metadata, `shared`, allocation, reference counting, deallocation, and
borrow anchors.

Those exclusions are semantic boundaries, not invitations for a backend to
ignore cleanup. Every normal fallthrough and `return` supported by the current
language must follow the order above. Later copy/value, inheritance, shared,
loop, and exception roadmaps must extend this initialized-place model rather
than redefining the normal local-object behavior frozen here.

#### 5.4.6 Frozen Staged Object-Value Profile

**Implementation status:** semantic contract frozen by OVS0 of the
[Object Value Semantics Roadmap](OBJECT_VALUE_SEMANTICS_ROADMAP.md). OVS0 does
not enable copy or object-value source forms. OVS1 parses and resolves copy
lifecycle declarations to stable identities. OVS2 type-checks their bodies and
records canonical user, ordered synthesized, or unavailable capabilities in
HIR. General object assignment, class value parameters/results, and other
object-producing expressions remain rejected until their later slices.

This profile narrows Sections 5.5, 5.6, and 6 to exact concrete inline classes,
normal control flow, and the already implemented alias and destruction model.
An object value is always realized in owned storage. Source and destination
places, construction state, and cleanup are explicit; no phase represents a
class object as a scalar value or copies its bytes without selecting a language
copy operation.

##### Lifecycle declarations and identities

The staged class model has three independent lifecycle slots:

1. exactly one explicit ordinary initializer, as in the current profile;
2. at most one copy constructor;
3. at most one copy assignment member.

The parser uses the existing initializer shape and the new assignment shape:

```text
initializer-declaration    = "init" "(" parameters? ")" block
copy-assignment-declaration = "assign" "(" "ref" identifier ":" class-name ")" block
```

An initializer is the class's copy constructor precisely when it has one
parameter, that parameter uses read-only `ref`, and its type is the exact
enclosing class. The parameter name is immaterial. Every other valid
initializer signature is ordinary. A class may therefore declare its one
ordinary initializer and one copy constructor with the same contextual `init`
spelling; they are not a general overload set. A copy constructor does not
satisfy the requirement for an ordinary initializer.

A copy assignment declaration has exactly one read-only alias parameter of the
exact enclosing class, an implicit mutable `self`, and an implicit `unit`
result. It has no `fn`, result annotation, receiver modifier, or semicolon.
There is no overloaded ordinary `assign` lifecycle form. Fields and ordinary
`fn` methods named `init` or `assign` remain in the ordinary member namespace
and may coexist with the corresponding lifecycle declarations.

A second declaration for the same lifecycle slot is diagnosed at the second
declaration and identifies the first. A malformed copy signature is diagnosed
as a malformed lifecycle declaration; it is not silently reclassified or used
to trigger synthesis. User lifecycle declarations receive stable owner-
qualified identities in source order. Synthesized operations receive a stable
class-owned semantic identity distinct from every source declaration. Lower
phases select those identities and never repeat selection from the spelling
`init` or `assign`.

The copy-constructor body follows the current straight-line initializer rules:
its destination `self` is uninitialized, each direct field must be initialized
exactly once, and the complete receiver becomes live only on normal completion.
Its source parameter is an already-live read-only object place. The copy
assignment body follows the statement surface of a mutable `unit` method. Both
`self` and the source are complete and live for the whole call; `return;` and
fallthrough complete the operation, while a value return is invalid. A custom
assignment body may update any permitted subset of fields and is not required
to resemble synthesized fieldwise assignment.

##### Copy capabilities and synthesis

Copy-construction and copy-assignment capability are computed independently:

- a valid user declaration provides that operation and completely replaces
  synthesis for it;
- otherwise the compiler synthesizes the operation exactly when every direct
  field supports the corresponding operation;
- every primitive field supports both operations and preserves its exact typed
  value, including the raw bits of `f64`;
- a class-typed field supports an operation when its exact field class does;
- an empty or primitive-only class therefore receives both synthesized
  operations when they are absent;
- a `destroy` declaration neither suppresses nor changes synthesis;
- an absent operation whose field requirements are not met is unavailable, or
  implicitly deleted, and every use receives a source diagnostic explaining
  the first deterministic capability path that failed.

The current acyclic primitive/inline-field profile has no syntax for explicitly
deleting an operation and no field kind that is intrinsically non-copyable, so
every otherwise valid current class can synthesize both operations. The
unavailable state is nevertheless part of the semantic model for future field
kinds and for structurally invalid IR; no backend may assume universal
copyability.

A synthesized copy constructor processes direct fields in declaration order.
It initializes a primitive field from the corresponding source field and copy-
constructs a class field into its destination storage. Each completed class
field becomes live at its operation's normal completion. A synthesized copy
assignment processes fields in the same declaration order, assigns primitives,
and invokes class-field copy assignment. The complete destination and all its
fields remain live throughout. These are ordered semantic operations, never a
target `memcpy`. A user operation contributes only the body the user wrote;
the compiler adds no implicit field copy before or after it.

##### Local initialization, assignment, and aliases

The first object source forms are:

- an already-live exact-class place rooted at an owning local, value parameter,
  method `self`, or `ref`/`mut ref` parameter, with any valid inline field
  projections;
- a fresh exact-class constructor expression `T(arguments)`;
- an exact-class result from an internally defined function or method.

Grouping preserves a place or produced object's meaning. An object-producing
expression is otherwise valid only where this profile explicitly requests an
object source: local initialization, object assignment, a matching value
argument, or an object return. It is not a scalar expression, an effect-only
statement, a primitive operand, a field value, a method receiver, or an alias
argument. Externally declared functions remain primitive-only.

Direct copy initialization has the source shape:

```ska
var copy: T = original;
```

The source must be readable, live, and exactly `T`. Storage for `copy` is
reserved first. The selected copy constructor receives the source as a
read-only alias and initializes that storage. `copy` becomes live and is
registered for cleanup only after normal completion. Copying from a place does
not create an intermediate object.

Object assignment has the statement shape:

```ska
destination = source;
```

The destination is evaluated first and must be a mutable, already-live owning
object local, owning value parameter, or class-typed field reached from a
mutable owning local or live mutable `self`. Assigning the complete method
receiver, rebinding an alias parameter, or replacing any object through an
alias-rooted path remains invalid. The source is then evaluated and must
produce or designate the exact destination class. The selected copy assignment
operation runs once; assignment does not destroy, reconstruct, unregister, or
reregister the destination.

An existing source place is passed directly as the operation's read-only
source alias. The language does not infer exclusive access: source and
destination may be the same place, and ordinary call-scoped aliases may also
reach either object. A user copy assignment is invoked even for self-assignment
and is responsible for the behavior of its own body. Synthesized assignment
runs its ordinary declaration-ordered field sequence even for self-assignment;
primitive self-stores preserve their values, while selected user field-
assignment members retain all their normal effects. The compiler inserts no
implicit identity guard. It may remove an operation only through an ordinary
proof that all observable behavior is unchanged, not merely because the two
places are equal.

##### Value parameters and arguments

An internal value parameter `name: T` owns independent exact-class storage in
the callee. It is mutable like an owning local and is distinct from `ref name:
T` and `mut ref name: T`, which retain their non-owning behavior. Object value
parameters remain forbidden on `extern fn` declarations.

Call evaluation remains left to right. For each object value argument, the
caller reserves the corresponding parameter destination and evaluates the
argument at that source position. An existing place is copy-constructed
directly into the parameter destination; a produced object follows the
temporary rules below: it is materialized first, the parameter is copy-
constructed from it, and it remains live through the call. Construction of
that parameter completes before the next argument is evaluated. The callee
body begins only after every argument and owned parameter has completed.

On every supported normal callee exit, the return result is established first,
then body temporaries and locals are cleaned according to their scopes, and
then owning value parameters are destroyed in reverse parameter order. Alias
parameters are never cleaned. The callee owns parameter cleanup even if the
backend uses caller-reserved memory to implement the internal ABI.

##### Object results and return storage

An internally defined function or method may have an exact concrete class
result. Every call supplies distinct uninitialized result storage. A return of
an existing exact-class place copy-constructs that result before any exited
scope is cleaned. A return of a produced exact-class object initializes the
same result according to the temporary and elision rules below. Wrong-class,
unreadable, dead, or missing returns are invalid. A `ref` or `mut ref` parameter
may be the source place; the result owns a copy and the alias itself does not
escape.

The result becomes live only when its selected initializer or copy constructor
completes. It is not an owning local or parameter of the callee and the callee
does not destroy it. Once established, ownership belongs to the caller, which
either uses the storage as a final destination or treats it as a materialized
temporary. Callee local and parameter cleanup therefore cannot invalidate the
result. HIR and MIR represent the destination and transfer explicitly; target
aggregate registers, hidden pointers, offsets, and frame placement remain
backend decisions.

##### Temporaries and full expressions

A fresh constructor expression and an internal object-returning call
materialize an owning temporary whenever they are not constructed directly
into an eligible final destination. Copying from an existing place does not.
Each temporary becomes live only when its construction completes and is then
destroyed exactly once in reverse completion order at the end of its full
expression.

The staged full-expression boundaries are:

- the complete initializer of one local declaration;
- the complete right side of one assignment statement;
- one effect-only call statement, including the call's complete argument list;
- one `return` expression.

Temporaries created while evaluating call arguments remain live through the
call and are destroyed after the call result has been secured. For `return`,
the result is initialized first, expression temporaries are then destroyed,
and lexical locals and value parameters are cleaned afterward using the
destruction profile. A newly initialized local is live and registered before
its initializer temporaries are destroyed. These rules extend the existing
initialized-place stack; they do not introduce moves or an implicit ownership
transfer between two places.

##### Permitted copy elision and observable effects

Copy elision is permitted in exactly two cases where the expression is an
ungrouped fresh constructor of the exact destination class:

1. `var value: T = T(arguments);`
2. `return T(arguments);` from a callable returning `T`.

In either case the non-elided abstract execution creates a temporary, copy-
constructs the destination from it, and destroys the temporary. A program must
have an accessible valid copy constructor for that execution even when the
copy is elided. Elision makes the source temporary and destination one object:
the constructor selected by `T(arguments)` runs once directly against
destination storage, the selected copy-constructor operation is omitted in its
entirety, and the omitted temporary's complete destruction sequence does not
run. This permission includes side effects in a user copy constructor,
recursively selected field copy operations, and the temporary's user/field
destructors.
No other user-visible operation is omitted by this permission.

The initial compiler policy is to elide every eligible occurrence. That policy
preserves the existing direct-to-destination construction behavior and must be
deterministic across compiler processes. The language permission remains
explicit so a later mode may choose non-elided execution without changing
validity. Grouped construction, construction used as an argument, assignment
to a live object, copy from an existing place, initialization from a function
result, and named-return optimization are not eligible. In particular,
assignment from `T(arguments)` constructs a temporary, invokes copy assignment,
then destroys the temporary at the statement boundary.

##### Diagnostics and extension boundary

Source diagnostics must distinguish malformed/duplicate lifecycle
declarations, unavailable copy capability, wrong-class or inaccessible
sources, invalid or dead destinations, read-only or alias-rooted replacement,
unsupported object contexts, missing object returns, and object-bearing
external signatures. Capability diagnostics follow declaration order through
class fields and identify both the use and the declaration path that makes an
operation unavailable. Diagnostics and every phase dump remain deterministic.

This profile adds no move or destructive transfer, relocation, slicing,
inheritance, base subobject, dynamic dispatch, interface, cast, shared
ownership, allocation, reference counting, borrow anchor, array, optional,
static/global object, closure capture, cross-module value, external object ABI,
exception edge, failed-copy cleanup, unwinding, or explicit early destruction.
It does not permit replacement through aliases. Future profiles must extend the
same explicit destination, initialized-place, result-storage, temporary, and
cleanup model rather than treating aggregate bytes as ownership.

### 5.5 Initialization Members

Section 5.4.6 is the implementation contract for the staged exact-class,
normal-flow compiler. The broader rules in this section also describe later
default initialization, inheritance, and constructor families that remain
outside that profile.

An `init` declaration defines a constructor that initializes object storage.

```ska
init(name: Str) {
    self.name = name;
}
```

Rules:

- a class may declare zero or more `init` members;
- if no applicable `init` member is declared, a default or compatibility constructor may be synthesized when all fields can be initialized;
- an `init` member with exactly one read-only alias parameter of the enclosing class type is its copy constructor, using `init(ref other: T)` syntax;
- subclass initialization must initialize the base subobject before subclass fields;
- `super(...)` is valid only within `init` in the initial language;
- if construction fails in a later exception-enabled language, only fully constructed subobjects are destroyed.

### 5.6 Copy Constructors and Copy Assignment

Section 5.4.6 freezes the initial executable subset, including mandatory
synthesis when field capabilities permit it and exact self-assignment behavior.
The rules below retain the broader direction for future field kinds,
inheritance, `final`, and `shared`.

Copy construction initializes a new object from an existing object of the same type.

```ska
init(ref other: Dog) {
    self.name = other.name;
}
```

Copy assignment updates the value of an already-initialized object from an existing object of the same type. The destination object remains alive throughout the operation.

```ska
assign(ref other: Dog) {
    self.name = other.name;
}
```

Rules:

- `init(ref other: T)` inside class `T` is the copy constructor for `T`;
- `assign(ref other: T)` inside class `T` is the copy assignment member for `T`;
- the parameter name is not significant, but the single parameter's binding mode and exact type are significant;
- a class may declare at most one copy constructor and at most one copy assignment member;
- copy constructors initialize uninitialized storage;
- copy assignment operates on an already-initialized object;
- copy construction initializes final fields like any other construction, but copy assignment cannot reassign a final field;
- user and synthesized copy assignment both execute for self-assignment;
  synthesized assignment runs its ordinary field sequence without an implicit
  identity guard, and every selected user member retains its normal effects;
- user-defined `init`, copy-construction `init`, `assign`, and `destroy` members may have side effects;
- the staged compiler synthesizes copy construction when every field supports
  copy construction; later inheritance extends that requirement to the base
  subobject;
- the staged compiler synthesizes copy assignment when every field supports
  copy assignment; later `final` and inheritance rules may make that operation
  unavailable;
- synthesized copy construction copies fields in declaration order;
- synthesized copy assignment assigns fields in declaration order; a future
  ownership-bearing field's own assignment operation remains responsible for
  its internal alias safety;
- synthesized `shared T` field copy increments the shared handle reference count;
- synthesized `shared T` field assignment must retain the new handle before releasing the old handle to handle self-assignment and aliasing safely.

### 5.7 Destruction Members

A `destroy` declaration defines the class-specific destruction body that runs deterministically when an object's lifetime ends.

Section 5.4.5 is the frozen implementation subset for local inline objects and
normal control flow. The inheritance, array, shared-ownership, dynamic-type,
and exceptional-cleanup rules below remain broader language design until their
dedicated profiles are frozen.

Syntax:

```ska
destroy {
    ...
}
```

Rules:

- each class may declare at most one `destroy` member;
- `destroy` takes no parameters and returns `unit` implicitly;
- `destroy` must not throw in the initial exception design; if an exception escapes it, the program terminates;
- destruction begins with the `destroy` body of the most-derived class;
- after a class's `destroy` body, its fields are destroyed in reverse declaration order;
- after that class's fields, its direct base subobject is destroyed using the same body-then-reverse-fields procedure;
- destruction continues through the base-class chain until the complete object has been destroyed;
- an absent user-declared `destroy` member is treated as an empty `destroy` body, so fields and the base subobject are still destroyed;
- destroying a `shared T` handle may trigger complete dynamic destruction of the pointee if it was the last owner;
- assigning to an inline object never invokes that object's `destroy` member or ends its lifetime, although its assignment member may release or destroy values owned by its fields.

For example, destruction of a heap-allocated `Dog extends Animal` occurs in this order:

1. `Dog` `destroy` body;
2. `Dog` fields in reverse declaration order;
3. `Animal` `destroy` body;
4. `Animal` fields in reverse declaration order;
5. the same procedure for any further base subobject;
6. deallocation of the original heap allocation after complete-object destruction finishes.

Inline objects use the same class/body/field/base order but do not perform heap deallocation. A sliced inline `Animal` value is an actual `Animal` object and therefore runs only the `Animal` destruction sequence. A shared allocation whose dynamic type is `Dog` always runs the complete `Dog` sequence, even when its last owner has static type `shared Animal`, `shared Obj`, or `shared Interface`.

Objects are destroyed:

- when an inline local goes out of scope;
- when an inline field's containing object is destroyed;
- when an array of inline objects is destroyed;
- when the last `shared` handle to a heap object is destroyed or overwritten.

---

## 6. Assignment, Copying, and Object Lifetime

Skald uses copy semantics by default. Move semantics are not part of the initial language.

Assignment to an inline object updates an already-initialized value:

```ska
var dog: Dog = Dog("A");
dog = Dog("B");
```

This invokes the class assignment rules and leaves `dog` containing the new value. Assignment does not end `dog`'s lifetime and does not run `Dog`'s `destroy` member. The assignment member may release or destroy old field values as part of updating them.

Copying:

- primitives copy by value;
- inline objects use `init(ref other: T)` or synthesized fieldwise copy construction;
- array copying follows the future array copy policy; deep element copy is the current direction but is not yet normative;
- `shared T` copies the handle and increments the reference count;
- copying `shared T` does not invoke `T`'s copy constructor;
- alias parameters cannot be copied as alias values.

Assignment:

- primitives assign by value;
- inline objects use `assign(ref other: T)` or synthesized fieldwise assignment;
- `shared T` assignment copies the new handle and releases the old handle;
- alias parameters are not assignable or rebindable.

Classes with ownership-sensitive fields follow a "rule of three" style:

- if a class defines `destroy`, it likely also needs explicit copy construction and copy assignment;
- if a class defines copy construction or copy assignment, it likely needs the other;
- the compiler may synthesize these operations when the base subobject and fields support them, subject to the final-field restriction on copy assignment above.

`init`, copy-construction `init`, `assign`, and `destroy` members are not assumed to be side-effect-free. The compiler may optimize them only when the language explicitly permits elision or when it can prove observable behavior is unchanged.

### 6.1 Optional Copy Elision

Skald permits, but does not require, copy elision in two cases involving a fresh inline object constructor expression of the exact destination type:

1. direct initialization of a new object;
2. returning a freshly constructed object from a function.

Direct-initialization example:

```ska
var dog: Dog = Dog("Rex");
```

Without elision, the constructor expression creates a temporary `Dog`, `dog` is copy-constructed from that temporary, and the temporary is then destroyed. With elision, the implementation constructs `Dog("Rex")` directly in `dog`'s storage and omits both the copy-constructor call and the destruction of the omitted temporary.

Return example:

```ska
fn make_dog() -> Dog {
    return Dog("Rex");
}
```

Without elision, `Dog("Rex")` creates a temporary, the function result is copy-constructed from it, and the temporary is destroyed. With elision, the implementation constructs `Dog("Rex")` directly in the function's result storage, which may be caller-provided storage.

Optional copy elision has the following rules:

- the implementation may choose independently for each eligible initialization or return whether to elide the copy;
- the program must still have an accessible, valid copy constructor for the non-elided operation;
- when elision occurs, the source temporary and destination are treated as one object whose lifetime is the destination's lifetime;
- the omitted copy-constructor and temporary-destructor calls do not occur, even if they would have had side effects;
- copy elision does not apply to assignment to an already-initialized object.

For the staged exact-class profile, Section 5.4.6 fixes the initial compiler's
policy to elide every eligible occurrence. This keeps compiler artifacts and
observable lifecycle effects deterministic while preserving the broader
language permission described here.

The permission to omit side-effectful copy construction and destruction is a specific language exception to the ordinary observable-behavior rule. Other constructor, assignment, and destructor calls may be removed only when the compiler proves that doing so does not change observable behavior.

Assignment to an already-initialized object remains assignment:

```ska
var dog: Dog = Dog("Old");
dog = Dog("New");
```

The constructor expression creates a source temporary, `dog.assign(...)` or synthesized copy assignment updates the existing `dog`, and the source temporary is then destroyed. This is semantically assignment, not destruction followed by construction, and it is not eligible for the optional copy-elision rule. The compiler may replace it with another implementation only when it proves that construction, assignment, destruction, and alias-visible behavior remain unchanged.

Move-only values are out of scope for the initial language.

### 6.2 Assignment to Parameters

Value parameters are local variables inside the callee:

```ska
fn f(dog1: Dog, ref dog2: Dog, mut ref dog3: Dog, dog4: shared Dog) -> unit {
    dog1 = Dog();      // ok: assigns to the local copy
    dog2 = Dog();      // illegal: ref parameter is not assignable
    dog3 = Dog();      // illegal: mut ref parameter is not assignable/rebindable
    dog4 = new Dog();  // ok: replaces local shared handle
}
```

Mutation through a `mut ref` binding is allowed:

```ska
fn rename(mut ref dog: Dog, name: Str) -> unit {
    dog.name = name;   // ok
}
```

Whole-object replacement through `mut ref` is not part of the initial language because it is visually ambiguous with rebinding the parameter.

---

## 7. Heap Allocation and Shared Ownership

Heap allocation is explicit:

```ska
var dog: shared Dog = new Dog("Rex");
```

`new T(args...)`:

- allocates storage for `T`;
- constructs a `T` in that storage;
- returns `shared T`;
- never returns null;
- panics or aborts on out-of-memory in the initial language.

Reassigning a `shared T` variable releases the old handle:

```ska
var dog: shared Dog = new Dog("A");
dog = new Dog("B");
```

If the old handle was the last owner, the old heap object is destroyed immediately unless a caller-side borrow anchor still owns it. In that case, replacement releases the original handle, while the anchor delays destruction until the anchored call completes.

Borrow anchors also prevent replacement through another alias during a call from leaving a dangling alias parameter. Reassigning a `shared` variable after the anchored call has returned cannot leave a dangling alias in user code because parameter aliases cannot escape the call.

---

## 8. Classes, Inheritance, and Polymorphism

### 8.1 Inline Values and Slicing

Assigning a derived inline value to a base inline variable slices:

```ska
var derived: Dog = Dog();
var base: Animal = derived;
```

`base` contains a copied `Animal` base subobject. It does not remain dynamically connected to `derived`.

### 8.2 Shared Upcasts

`shared Derived` may be implicitly upcast to `shared Base` when `Derived extends Base`:

```ska
var dog: shared Dog = new Dog();
var animal: shared Animal = dog;
```

This copies the shared handle. The underlying heap object remains a `Dog`. The converted handle preserves the complete-object address and the allocation's dynamic `Dog` metadata; it does not replace that metadata with `Animal` metadata.

If `animal` becomes the last owner, releasing it runs the complete `Dog` destruction sequence and then frees the original `Dog` allocation. The static type `Animal` controls which operations are available through the handle, but it never selects shared destruction.

### 8.3 Alias-Parameter Upcasts

An existing `Derived` object may supply the place for a read-only or mutable alias parameter of type `Base`:

```ska
fn speak(ref animal: Animal) -> unit {
    animal.speak();
}

var dog: Dog = Dog();
var heap_dog: shared Dog = new Dog();

speak(dog);
speak(heap_dog);
```

The alias refers to the original object or shared pointee. No slicing occurs for alias parameters.

### 8.4 Virtual Dispatch

Instance methods are non-virtual by default. Virtual dispatch is enabled only for methods explicitly declared `virtual`.

This follows the C++ direction: ordinary methods have direct-call semantics, while virtual methods opt into dynamic dispatch and per-object/type dispatch metadata.

Complete-object destruction of a shared allocation is separate from user-visible virtual method dispatch. Destructors do not use `virtual` syntax, and a base class does not need to opt into safe polymorphic destruction. The shared runtime always selects the compiler-generated complete-object destruction entry from the allocation's dynamic type metadata.

Example:

```ska
class Animal {
    virtual fn speak() -> unit {
        ...
    }

    fn debug_name() -> Str {
        ...
    }
}

class Dog extends Animal {
    override fn speak() -> unit {
        ...
    }
}
```

Rules:

- only methods declared `virtual` in a base class may be overridden;
- overriding requires explicit `override`;
- override compatibility is exact initially, including receiver mutability;
- private methods, static methods, and `init` members are not virtual;
- non-virtual method calls are statically resolved;
- virtual read-only `fn` calls through read-only aliases, mutable aliases, and `shared Base` handles dispatch according to the dynamic object type;
- virtual `mut fn` calls require mutable receiver access and therefore cannot be made through a read-only alias parameter of type `Base`;
- calls on sliced inline base values dispatch as the sliced base value.

---

## 9. Interfaces

Skald interfaces participate in the inline-value, shared-ownership, and alias-binding model described by this specification.

Interface declarations contain method signatures:

```ska
interface Hashable {
    fn hash_code() -> u64;
}

interface Named {
    fn get_name() -> Str;
    mut fn set_name(name: Str) -> unit;
}
```

Classes declare conformance:

```ska
class Key implements Hashable {
    fn hash_code() -> u64 {
        return 42u;
    }
}
```

Initial interface rules:

- interfaces contain method signatures only;
- no interface fields;
- no default method bodies;
- no interface inheritance initially;
- class conformance is checked statically;
- private methods do not satisfy interface requirements;
- interface `fn` signatures require read-only implementations and `mut fn` signatures require mutable implementations;
- method signature compatibility is exact, including receiver mutability.

Interface use should primarily happen through alias parameters and shared handles:

```ska
fn print_hash(ref value: Hashable) -> unit {
    var h: u64 = value.hash_code();
}

var key: Key = Key();
var heap_key: shared Key = new Key();

print_hash(key);
print_hash(heap_key);
```

Standalone inline variables of interface type are not allowed initially:

```ska
var value: Hashable = Key(); // illegal
```

Interface use should go through read-only or mutable alias parameters of the interface type, or through `shared Interface`. This avoids needing a general inline interface-object representation.

A read-only interface alias may call only read-only interface methods. A mutable interface alias may call both read-only and mutable interface methods. Interface dispatch does not change these access rules.

A `shared C` handle may be implicitly converted to `shared I` when class `C` implements interface `I`:

```ska
var heap_key: shared Key = new Key();
var hashable: shared Hashable = heap_key;
```

The interface conversion copies the same owning handle and preserves the complete-object address, reference count, allocation identity, and dynamic class metadata. If `hashable` is the final owner, release runs the complete dynamic `Key` destruction sequence before freeing the original allocation. Interface method tables participate in dispatch but do not select destruction.

---

## 10. Expressions and Statements

Skald supports the following expression and statement forms.

**Specification status for loops:** provisional and intentionally incomplete. Looping and iteration are deferred until after the first vertical compiler slice. The `while`, `for ... in`, `break`, and `continue` entries below reserve the current design direction, but do not yet form an implementation-ready contract.

Before loops are implementation-ready, the specification must define loop-variable scope, condition and collection evaluation order, cleanup on `break` and `continue`, targets in nested loops, mutation of a collection during iteration, whether produced elements are copied or borrowed, and the exact iterator protocol and lifetime rules. This does not make `if`, blocks, or `return` provisional.

Statements:

- block;
- local variable declaration;
- assignment;
- expression statement;
- `if` / `elif` / `else`;
- `while`;
- `for ... in`;
- `return`;
- `break`;
- `continue`;
- `init`-only `super(...)`.

Expressions:

- literals;
- local references;
- field access;
- static member access;
- function calls;
- method calls;
- construction expressions such as `T(...)`;
- `new` heap allocation;
- unary and binary operators;
- explicit casts;
- type tests with `is`;
- indexing and slicing;
- array construction.

### 10.1 Conditional Statements

**Implementation status:** implemented end to end, including nested native
behavior, exact diagnostics, return analysis, and repeated-process determinism
coverage.

The initial conditional form follows Niflheim's chained-arm spelling:

```ska
if (first_condition) {
    first_action();
}
elif (second_condition) {
    second_action();
}
else {
    fallback_action();
}
```

Its grammar is:

```text
if-statement = "if" "(" expression ")" block
               ("elif" "(" expression ")" block)*
               ["else" block]
```

There may be zero or more `elif` arms and at most one final `else` arm.
Parentheses around every condition and a block for every arm are mandatory.
`elif` is a distinct keyword and the only chained-arm spelling; `else if` is
not accepted as an alternative. The construct is a statement and does not
produce a value.

Every condition must have type exactly `bool`. There is no implicit numeric,
object, shared-handle, or optional truthiness. Conditions are evaluated from
left to right. Evaluation stops at the first condition producing `true`, only
that arm's block executes, and no later condition or arm is evaluated. If all
conditions produce `false`, the `else` block executes when present; otherwise
execution continues after the statement.

Each condition is resolved in the lexical scope containing the complete
conditional statement. Each arm block creates an independent child scope. A
name declared in one arm is not visible in another arm, in a later `elif`
condition, or after the conditional. Ordinary nested-block shadowing rules
apply inside each arm.

For definite-return analysis, a conditional definitely returns only if it has
an `else` arm and every `if`, `elif`, and `else` block definitely returns. The
rule composes through nested blocks and conditionals. A conditional without
`else`, or with any arm that can reach its closing brace, can continue with the
following statement. This analysis, rather than the parser or backend,
enforces the requirement that every reachable path through a non-`unit`
function returns a value.

The initial C-series conditional profile does not include `if` expressions,
`else if`, implicit truthiness, casts to or from `bool`, equality or ordering,
logical negation, `&&`, `||`, pattern matching, optional presence tests,
flow-sensitive type narrowing, loops, branch optimization, SSA, or phi nodes.
The broader language may specify some of these separately. In particular,
short-circuit logical operators require expression-level control flow and must
not be introduced as eager binary operations.

### 10.2 Returns and Call Statements

Function return syntax follows the declared result type:

- a non-`unit` function returns with `return expression;`, where the
  expression must have exactly the function's declared result type;
- a `unit` function returns with `return;` and cannot attach an expression;
- reaching the closing brace of a `unit` function is an implicit `return;`;
- every reachable path through a non-`unit` function must return a value, so
  reaching its closing brace is a compile-time error.

The implemented language supports expression statements only for calls whose
result is `unit`:

```ska
do_work();       // valid when do_work returns unit
value_call();    // invalid when value_call returns i64
1 + 2;           // invalid
```

Grouping parentheses do not change whether the outer operation is a call. This
restricted call-statement rule avoids accidental discarded values and is
narrower than the complete statement list above; broader expression statements
may be specified later.

### 10.3 Operators

The T-series primitive profile extends the currently implemented operator
surface without adding new operator tokens:

- binary `+`, `-`, and `*` require two operands of exactly the same numeric
  type and produce that type;
- no integer is implicitly widened, narrowed, or converted between signed and
  unsigned representation;
- `u64` addition, subtraction, and multiplication wrap modulo `2^64`;
- `u8` addition, subtraction, and multiplication wrap modulo `2^8`, and every
  result is canonicalized to `0..=255` before another operation, store, call,
  or return can observe it;
- `f64` addition, subtraction, multiplication, and unary negation follow
  IEEE-754 binary64 under the default round-to-nearest, ties-to-even
  environment, including signed zeroes, subnormals, infinities, and NaNs;
- unary minus remains valid for `i64`, becomes valid for `f64`, and is invalid
  for `u64` and `u8`.

The restricted external-function profile assumes foreign callees preserve the
default floating-point environment. NaN payload propagation from arithmetic is
not guaranteed, but an unchanged `f64` value retains its raw representation.
This profile does not alter the still-open `i64` overflow behavior.

The broader language design reserves division, remainder, exponentiation,
bitwise operations, and shifts for later slices. Their intended direction is
matching numeric operands, explicit signed/unsigned casts, integer-only
bitwise operations, and arithmetic versus logical right shift according to
signedness. Exact division, remainder, shift-failure, and overflow behavior
must be settled before those operators are implemented; they are not part of
the T-series contract.

### 10.4 Indexing, Slicing, and For-In

This subsection is a provisional sketch. Indexing and slicing depend on the deferred array design, and the structural iteration protocol is not yet normative.

Arrays support:

```ska
arr.len()
arr[index]
arr[index] = value
arr[start:end]
arr[start:end] = value
```

Indexing and slicing syntax may also be structural sugar over methods:

```ska
x[i]       // x.index_get(i)
x[i] = v   // x.index_set(i, v)
x[a:b]     // x.slice_get(a, b)
x[a:b] = v // x.slice_set(a, b, v)
```

Structural read operations require read-only receiver methods, while structural write operations require mutable receiver methods:

- `fn index_get(K) -> R`;
- `mut fn index_set(K, W) -> unit`;
- `fn slice_get(i64, i64) -> U`;
- `mut fn slice_set(i64, i64, U) -> unit`.

Consequently, indexing and slicing reads are available through both read-only and mutable receiver access. Index and slice assignment require mutable receiver access. For built-in arrays, the same rule means that an array reached as an inline subobject through a read-only alias cannot be modified.

`for ... in` uses the following structural iteration shape:

```ska
for item in collection {
    ...
}
```

Eligibility requires:

- `fn iter_len() -> u64`;
- `fn iter_get(i64) -> T`.

The collection expression is evaluated once, and the iteration length is snapshotted before the loop.

---

## 11. Casts, Type Tests, and Equality

Primitive casts are explicit. Casts involving `unit` are invalid. Other primitive casts use the following rules:

- `bool` converts to integer zero or one and to `f64` zero or one;
- integers convert to `bool` as false for zero and true for nonzero;
- `f64` converts to `bool` as false for positive or negative zero and true for every other value;
- integer-to-integer casts truncate to the target width and then interpret the resulting bits using the target signedness; these casts do not panic;
- integer-to-`f64` casts use the source signedness and may lose precision;
- `f64`-to-integer casts truncate toward zero and then range-check the result; NaN, infinity, and out-of-range values panic and abort.

These cast rules describe the intended broader primitive-type system. The
T-series remaining-primitive profile implements no primitive casts at all and
performs no contextual literal conversion or numeric promotion. Initializers,
arguments, returns, and operator operands must already have the exact required
type. Conditions likewise still require an expression already typed as
`bool`. Cast syntax, lowering, and failure behavior require a separate design.

Object casts:

- derived-to-base inline assignment slices;
- binding a derived place to a base-typed alias parameter is an implicit non-slicing upcast;
- binding a class place to an implemented-interface alias parameter is implicit;
- binding an interface alias to an `Obj` alias parameter is implicit;
- the corresponding derived-to-base, class-to-interface, and interface-to-`Obj` conversions of `shared` handles are implicit;
- downcasts are explicit and checked at runtime;
- interface casts are explicit and checked at runtime when not statically known;
- every conversion or checked cast of a shared handle preserves its ownership pointer, allocation identity, reference count, and complete dynamic class metadata.

`is` performs a runtime type/conformance test for inline objects, shared handles, and alias-bound receivers.

Equality:

- primitive equality is value equality;
- inline object equality is not implicit unless a later operator-overload or protocol rule is added;
- `shared T` equality compares object identity by default;
- object identity comparison through aliases may be provided explicitly later, but is not needed for the initial core;
- optional equality is defined only when the contained type has equality.

Primitive equality is likewise outside the initial C-series profile. Boolean
values in that profile are formed by literals, bindings, parameters, and call
results rather than comparison expressions.

---

## 12. Error Model

The initial language keeps unrecoverable runtime failures:

- failed checked casts;
- out-of-bounds array access;
- invalid primitive casts such as out-of-range `f64 -> i64`;
- explicit panic;
- out-of-memory;
- failure to complete a bootstrap runtime stdout write.

These failures are unrecoverable and terminate the process unsuccessfully,
normally through runtime panic/abort machinery, unless a future rule explicitly
maps an operation into checked exception handling. Unless an individual
operation says otherwise, the exact exit status or terminating signal is not a
language guarantee.

### 12.1 Checked Exceptions

**Specification status:** provisional and intentionally incomplete. Checked exceptions are part of the intended Skald design, but are deferred until after the first vertical compiler slice. The syntax and rules below constrain the eventual design; they do not yet define a usable exception feature.

Before checked exceptions are implementation-ready, the specification must define `throw` and rethrow syntax, exception-set typing and subtyping, catch-clause ordering and binding ownership, compatibility rules for functions, overrides, interfaces, and function values, cleanup after partially completed construction or copying, and the propagation ABI or lowering. `try`, `catch`, `throw`, and `throws` should therefore be treated as reserved design syntax for now.

Draft syntax:

```ska
class IoError extends Exception {
    message: Str;
}

fn read_file(ref path: Str) -> Str throws IoError {
    ...
}

fn main() -> i64 {
    try {
        var text: Str = read_file("input.txt");
        return 0;
    } catch err: IoError {
        return 1;
    }
}
```

Rules:

- functions that may throw checked exceptions must declare them with `throws`;
- callers must catch checked exceptions or declare that they also throw them;
- unchecked panic/abort conditions remain outside checked exception handling unless later redesigned;
- exception objects should be heap-owned, most likely as `shared Exception`-like values, so caught exceptions cannot dangle;
- catch clauses match by exception type, with ordinary subtype rules;
- `finally` is deferred unless a later design needs it.

Design constraints already implied by Skald:

- unwinding must run destructors for all fully constructed inline locals, fields, arrays, and shared handles;
- `destroy` members must not throw initially;
- throwing during destruction terminates the program;
- an `init` member that throws must destroy fully constructed subobjects but must not run `destroy` for the incomplete whole object;
- all compiler IR that can branch to exceptional control flow must preserve cleanup ordering.

Implementation may initially lower exceptions to an explicit hidden result/exception channel rather than native platform unwinding. This keeps the runtime smaller and makes destructor cleanup paths visible in the compiler.

---

## 13. Runtime Model

Skald is designed around a small runtime with no garbage collector.

Runtime responsibilities:

- heap allocation and deallocation;
- reference-count header management for `shared`;
- runtime type metadata for casts, virtual dispatch, interface dispatch, and complete-object destruction;
- panic/abort reporting;
- minimal support for arrays and immutable string literal storage;
- exception propagation support.

There is no garbage collector.

No root stack, tracing metadata for GC, safepoints, or write barriers are required for ordinary memory management.

Heap object layout must support:

- reference count;
- dynamic type metadata pointer or equivalent;
- complete object payload;
- alignment suitable for the target platform.

The initial single-inheritance ABI places the direct base subobject at offset zero. A shared class, `Obj`, or interface handle stores the address of the complete object payload, or an equivalent representation from which both the complete payload and reference-count header are recovered without consulting the handle's static type. Shared upcasts, interface conversions, and checked shared casts do not adjust or replace this ownership pointer.

Each concrete class's dynamic type metadata contains a compiler-generated complete-object destruction entry or equivalent operation. Given the complete object payload, this entry runs the most-derived `destroy` body, destroys fields and base subobjects in the language-defined order, and returns without freeing an adjusted base or interface view address. The shared runtime frees the original allocation only after this entry completes.

Reference-count operations:

- retain on `shared` copy;
- release on `shared` destruction/overwrite;
- when release reaches zero, load the allocation's dynamic type metadata and invoke its complete-object destruction entry;
- after complete-object destruction, free the original allocation exactly once.

The static type of the releasing handle is not an input to destruction. A release through `shared Derived`, `shared Base`, `shared Obj`, or `shared Interface` follows the same allocation header and therefore selects the same most-derived destruction entry. This dynamic destruction is mandatory for all shared allocations; `destroy` does not use `virtual`, and safe shared destruction does not depend on an opt-in declaration.

Borrow anchors do not require a runtime ownership search or a separate runtime ownership structure. A hidden shared anchor uses the same retain and release operations as any other shared copy. Direct inline storage, stable shared locals and parameters, forwarded alias parameters, and already-owning temporaries require no additional reference-count operation solely for the alias. Hidden anchors are compiler-managed caller temporaries and must participate in normal and exceptional cleanup.

Thread-safe reference counting is out of scope unless concurrency is added later.

### 13.1 Bootstrap `i64` Output

**Implementation status:** implemented by the stage-0 x86-64 compiler under
runtime ABI version 4, with exact source-to-stdout golden coverage.

Until strings and the standard I/O library exist, the runtime exposes one
low-level output operation:

```c
void ska_rt_println_i64(int64_t value);
```

Skald source accesses it through the ordinary external declaration mechanism:

```ska
extern fn ska_rt_println_i64(value: i64) -> unit;
```

One successful call writes the shortest ASCII decimal representation of
`value` to stdout followed by exactly one line-feed byte (`0x0a`). Zero is
written as `0`; negative values have one leading ASCII `-`; positive values
have no sign; and there is no padding, grouping, locale-specific digit or
separator, carriage return, or extra whitespace. The operation is defined for
every `i64`, including `-9223372036854775808` and
`9223372036854775807`.

The operation completes and checks the entire record before returning. A
detected formatting, write, or flush failure is an unrecoverable runtime error:
the process terminates unsuccessfully rather than returning normally or
exposing a partial-success result to Skald. The exact diagnostic text, exit
status, or terminating signal is not part of this ABI contract.

This function exists to bootstrap observable tests. It is not the final
user-facing I/O API; a future Skald standard library may wrap lower-level
runtime facilities with ordinary functions and richer error handling.

### 13.2 Bootstrap `bool` Output

**Implementation status:** implemented end to end. The compiler accepts the
declaration below as an ordinary restricted external function.

The runtime ABI exposes:

```c
#include <stdbool.h>

void ska_rt_println_bool(bool value);
```

Skald source accesses it through the same ordinary restricted external
declaration mechanism as integer output:

```ska
extern fn ska_rt_println_bool(value: bool) -> unit;
```

One successful call with `true` writes the four ASCII bytes `true` followed by
one line-feed byte (`0x0a`). One successful call with `false` writes the five
ASCII bytes `false` followed by one line-feed byte. It writes no sign,
capitalization, padding, carriage return, locale-dependent text, or other
whitespace. Consecutive calls produce consecutive complete records in call
order.

The function completes and checks the entire record before returning. A
detected write or flush failure is an unrecoverable runtime error and
terminates the process unsuccessfully under the same policy as
`ska_rt_println_i64`. The exact diagnostic, status, or terminating signal is
not guaranteed. The symbol is part of the current runtime ABI version 4.

This operation exists only for bootstrap observability. It does not introduce
formatting, recoverable I/O, or a final standard-library printing API, and no
compiler phase recognizes its name specially.

### 13.3 Bootstrap Remaining-Primitive Output

**Implementation status:** implemented end to end. All three symbols below are
part of runtime ABI version 4.

```c
#include <stdint.h>

void ska_rt_println_u64(uint64_t value);
void ska_rt_println_u8(uint8_t value);
void ska_rt_println_f64_bits(double value);
```

The corresponding ordinary restricted external declarations are:

```ska
extern fn ska_rt_println_u64(value: u64) -> unit;
extern fn ska_rt_println_u8(value: u8) -> unit;
extern fn ska_rt_println_f64_bits(value: f64) -> unit;
```

`ska_rt_println_u64` writes the shortest unsigned ASCII decimal representation
of the complete `u64` range followed by one LF. `ska_rt_println_u8` does the
same for the canonical value in `0..=255`. Neither writes a sign, leading
zeroes except for the single value `0`, padding, grouping, locale-dependent
characters, carriage return, or other whitespace.

`ska_rt_println_f64_bits` observes representation rather than formatting a
decimal number. It writes lowercase ASCII `0x`, exactly 16 lowercase
hexadecimal digits containing the received IEEE-754 binary64 bit pattern from
most-significant nibble to least-significant nibble, and one LF. Examples are
`0x0000000000000000` for positive zero, `0x8000000000000000` for negative
zero, and `0x3ff8000000000000` for `1.5`. The operation preserves and exposes
the received representation of infinities and NaNs; it does not canonicalize
them or promise how prior arithmetic chose a NaN payload.

Each call produces one complete record and uses the existing unrecoverable
detected-output-failure policy. The functions are locale-independent bootstrap
test facilities, are not final user-facing formatting APIs, and receive no
special recognition from the compiler.

### 13.4 Stage-0 Inline-Object Layout and Receiver ABI

**Implementation status:** implemented by the Linux x86-64 System V backend.

This subsection is a stage-0 compiler ABI contract, not a promise of stable
cross-module object ABI. Inline layout remains an implementation choice in the
general language, and object types remain unavailable through the restricted C
external-function profile.

The initial target lays out fields in declaration order. Each field begins at
the smallest offset satisfying its target alignment. Class alignment is the
maximum field alignment, or one for an empty class, and trailing padding rounds
class size up to that alignment. Empty classes have size one and alignment one
so every inline object has an addressable storage extent suitable for later
aliasing.

Primitive field size and alignment on this target are:

| Type | Size | Alignment |
|---|---:|---:|
| `i64` | 8 | 8 |
| `u64` | 8 | 8 |
| `f64` | 8 | 8 |
| `u8` | 1 | 1 |
| `bool` | 1 | 1 |

Under the frozen class-typed inline-field profile in Section 5.4.4, a class
field uses the recursively computed size and alignment of its exact class.
Class layout dependencies may be computed in any deterministic order, while
fields within each class remain laid out in declaration order. The semantic
containment graph must already be acyclic before target lowering. The backend
nevertheless rejects a recursive or incomplete MIR layout structurally.
Resolving a nested place adds each checked field offset in projection order;
all offset and extent arithmetic remains checked target-size arithmetic.

`bool` and `u8` fields retain their language widths even if the stack-heavy
backend uses wider homes for unrelated scalar temporaries. Layout computation
uses checked target-size arithmetic. A class whose size, alignment, field
metadata, or offset cannot be represented for the selected target is rejected
with a structured target diagnostic; the compiler must not wrap host integer
arithmetic or continue with a partial layout.

MIR identifies fields semantically and never contains the byte offsets from
this table. The x86-64 backend's target layout service is the sole authority
for converting a verified field projection into an address.

An initializer or instance method is an internal direct-call entry point whose
hidden first argument is the address of the existing complete object storage.
On System V this receiver is an integer-class argument placed before explicit
source arguments. It consumes the next integer argument location—`%rdi` when
available at the beginning of an ordinary call—but consumes no SSE location.
Explicit integer and SSE arguments then use the existing independent register
counters and stack-placement rules. A primitive result uses its existing ABI
location; `unit` has no result payload. `init` has an implicit `unit` result.

The hidden receiver is not a source parameter, cannot be accessed as a pointer
value, and does not make object types externally linkable. Internal initializer
and method symbols are formed deterministically from stable compiler-assigned
identities through the same collision-proof symbol authority used for Skald
definitions. Their exact textual spelling is not a language guarantee and is
never recovered from source names below resolution.

### 13.5 Stage-0 Alias-Parameter ABI

**Implementation status:** implemented by the Linux x86-64 System V backend
for the restricted profile in Section 5.4.3.

An internal `ref` or `mut ref` parameter is passed as one pointer to the
complete inline object storage. The pointer is integer-class, has the target's
machine-pointer size and alignment, and never copies the object's bytes. Both
alias modes have the same machine representation; their difference is enforced
statically through place access.

Alias parameters participate in the ordinary source-ordered System V argument
layout. The hidden receiver, when present, remains the first integer-class
argument. Each alias consumes the next integer register or shared stack
argument slot, while primitive integer and SSE arguments retain their
independent register counters and existing stack order.

The callee stores an incoming alias address in a pointer-sized frame home.
Access through that parameter loads the address and applies target-computed
field offsets. The caller materializes the address of the verified source
place directly into the assigned argument location. Forwarding an alias passes
the same object address.

This ABI is internal to the stage-0 compiler. Alias parameters remain forbidden
in external declarations and no cross-module object ABI stability is promised.
The restricted profile has no shared sources, so calls perform no retain,
release, hidden anchoring, or runtime ownership search.

---

## 14. Relationship to Niflheim

Skald originated in an exploratory draft called Niflheim2, which used the earlier Niflheim language and compiler as a design starting point. The memory model and several related semantics diverged enough that the project became a distinct language with a new name, compiler, source suffix, and repository. Niflheim remains historical context rather than a compatibility target or normative dependency of this specification.

Skald intentionally retains several ideas explored in Niflheim:

- statically typed compiled language;
- the primitive types `i64`, `u64`, `u8`, `bool`, `f64`, and `unit`;
- fixed-size arrays;
- modules/imports/exports;
- classes;
- single inheritance;
- interfaces;
- universal root type `Obj`;
- virtual dispatch support;
- static methods and static variables;
- structural indexing/slicing/iteration sugar;
- immutable byte-backed `Str`;
- function values without captures.

Skald intentionally changes or removes:

- garbage-collected references;
- nullable reference values by default;
- implicit virtual dispatch by default;
- implicit mutable receiver access for every instance method; ordinary `fn` receivers are read-only and mutation requires `mut fn`;
- ordinary reference-typed locals/fields/returns;
- GC root/safepoint semantics;
- null as the default value for reference-like types;
- absence of recoverable exceptions; Skald adds checked exceptions to the design.

Niflheim code should not be expected to compile as Skald code without substantial changes. The Niflheim repository may be consulted for historical implementation context, but Skald behavior is defined by Skald's own specification and documentation.

---

## 15. Specification Status and Open Design Questions

### 15.1 Deferred Language Areas

The following intended features are deliberately not specified well enough to implement yet:

- optionals, including presence binding, extraction, conversions, payload lifetime, and ownership behavior;
- arrays, including construction, element lifetime, copying, mutation, indexing, and slicing;
- loops and iteration, including `while`, `for ... in`, `break`, `continue`, and the iterator contract;
- checked exceptions, including throwing, catching, exception-set checking, cleanup, and lowering;
- locally declared alias bindings and scoped narrowing aliases; restricted
  call-scoped parameter aliases are implemented as described in Section 5.4.3.

Their existing sections preserve design direction and reserve likely syntax,
but are non-normative where they do not give a complete rule. These features
are outside the currently implemented language subset.

### 15.2 Other Major Underspecified Areas

The following are also substantial gaps. Each must be settled before the
corresponding language area is considered complete:

- **Lexical and grammatical definition:** the implemented primitive and
  restricted inline-object profile has an explicit lexical and grammatical
  contract in [`grammar/README.md`](../grammar/README.md), but the complete
  language still needs token and comment rules, additional literal families,
  later operator precedence and associativity, and rules for resolving
  syntactic ambiguities.
- **Name, type, and call resolution:** the implemented subset defines
  single-file function/class and lexical-local resolution in
  [`grammar/README.md`](../grammar/README.md), without overloading or implicit
  conversions. The complete language still needs cross-module references,
  declaration cycles, overload availability or prohibition, candidate
  selection, conversion ranking, and ambiguity diagnostics.
- **Primitive edge-case semantics:** the implemented subset defines literal
  ranges, `u64`/`u8` modular `+`, `-`, and `*`, and binary64 `f64` behavior for
  the same operator surface. Signed `i64` overflow, division or remainder by
  zero, shifts, explicit casts, comparisons, NaN behavior, decimal floating
  formatting, and future constant evaluation remain open. Every additional
  backend must separately validate its C ABI mapping, binary64 behavior,
  floating environment, and mixed-class argument placement.
- **Evaluation and cleanup ordering:** the implemented subset defines
  left-to-right operands/arguments plus receiver, field, and direct-
  construction order in [`grammar/README.md`](../grammar/README.md). The
  frozen local deterministic-destruction profile additionally defines cleanup
  for owning locals on implemented normal block, conditional, and return exits.
  The staged object-value profile freezes full-expression boundaries and
  temporary cleanup for its normal-flow source contexts. Those rules are not
  implemented yet, and the complete language still needs cleanup sequencing
  for loops, exceptions, and later control-flow forms.
- **Initialization rules:** the implemented inline-object profile defines
  straight-line definite initialization for primitive fields during direct
  local construction. The frozen class-typed inline-field profile additionally
  defines exact direct field construction, normal-return subobject liveness,
  nested access, and acyclic containment. Default initialization in other
  storage contexts, base-subobject ordering, branching or throwing
  initializers and partial-construction cleanup remain open. The staged object-
  value profile freezes exact-class copy/assignment synthesis and bodies for
  the current no-inheritance field model, but those rules are not implemented
  yet.
- **Static storage lifetime:** initialization and destruction order within and across modules, dependency cycles, and failure during static initialization.
- **Polymorphic narrowing through aliases:** checked downcasts and interface casts are named, but the scoped alias-binding form for using a successfully narrowed object is not yet defined. It must inherit access mode and remain within the source alias's lifetime.
- **Modules, build model, linkage, and foreign interfaces:** Section 3.1 defines the implemented single-file exact-symbol profile and its planned extension over all primitive value types. Source-to-module mapping, import discovery, exports, separate compilation, symbol visibility, cross-module external-declaration coalescing, other ABI types, alternate calling conventions, and ownership rules for foreign calls remain open.
- **Required library and runtime surface:** Sections 13.1 through 13.3 define only bootstrap scalar observation operations. The minimum facilities for general I/O, decimal floating formatting, dynamic storage or collections, diagnostics, and other practical programs are not yet identified. This is especially relevant to the eventual self-hosting compiler, even if it is outside the core language semantics.

The local normal-flow destruction contract in Section 5.4.5 is implemented.
Temporary, loop, failed-construction, and exceptional cleanup remain broader
ownership-model gaps that must be settled before their associated features are
implemented.

### 15.3 Open Design Questions

The following decisions are intentionally not finalized by this draft:

1. Should whole-object replacement through `mut ref` exist with explicit syntax?
2. Which explicit array initialization forms should be added for non-defaultable element types?
3. How much of the old Niflheim unsafe systems-layer proposal should exist in Skald, if any?
4. What is the exact checked-exception syntax and lowering strategy?

### 15.4 Resolved Decisions

Resolved decisions in this draft:

- the language is named Skald, its compiler is named `skac`, and source files use the `.ska` suffix;
- local declarations use `var name: Type` syntax;
- ordinary statements and declarations use semicolon terminators;
- lifecycle declarations use the contextual special-member introducers `init`, `assign`, and `destroy` without `fn`;
- those contextual words remain available as ordinary identifiers and special members do not occupy the ordinary method namespace;
- instance methods and special members use `self`, not `__self`, for the current object;
- virtual dispatch is opt-in with `virtual`;
- inline interface-typed variables are not allowed initially;
- `Obj` remains the universal root type, mainly for read-only and mutable alias parameters and `shared Obj`.
- default array construction is valid only for element types with default values;
- array physical storage placement is an implementation detail;
- `Str` is an immutable small inline value backed by immutable byte storage;
- string literals lower to `Str` values backed by compiler-emitted static immutable bytes.
- the implemented bootstrap external-function profile uses exact source identifiers as C-ABI linker symbols, accepts only by-value `i64`, `u64`, `u8`, `f64`, and `bool` parameters and `i64`, `u64`, `u8`, `f64`, `bool`, or `unit` results, and treats declarations as trusted ABI assertions;
- on Linux x86-64 System V, Skald `bool` maps to C `bool` (`_Bool`), leaves Skald as canonical false or true, and external boolean results are normalized from the ABI result byte;
- compiler-generated function symbols cannot collide with valid exact external identifiers and do not reserve an ordinary Skald identifier prefix;
- external declarations and Skald function definitions share one non-overloaded namespace, and `main` must be a Skald definition;
- `unit` functions use `return;` or implicit fallthrough, while non-`unit` functions must return a value on every reachable path;
- the first implemented expression-statement subset contains only unit-producing calls;
- `ska_rt_println_i64` writes the shortest ASCII signed decimal representation and one LF, and a detected incomplete output is unrecoverable;
- the current runtime ABI implements `ska_rt_println_bool`, which writes
  lowercase ASCII `true` or `false` and one LF, uses the same unrecoverable
  detected-output-failure policy, and remains an ordinary external function;
- `u64`, `u8`, and raw-bit binary64 `f64` are implemented end to end;
  `double` is not a Skald type keyword;
- decimal `u64` literals use suffix `u`, decimal `u8` literals use suffix `u8`, decimal-point or exponent literals are `f64`, and expected type never reinterprets a numeric literal;
- the implemented numeric profile has no implicit conversions, promotions, or
  primitive casts, and keeps `main` exactly `fn main() -> i64`;
- `u64` and `u8` `+`, `-`, and `*` wrap modulo their widths, while `f64` arithmetic follows binary64 under the default round-to-nearest, ties-to-even environment;
- System V integer and SSE argument registers are allocated independently for mixed scalar signatures, and every Skald-visible `u8` is canonical in `0..=255`;
- runtime ABI version 4 implements `u64` and `u8` decimal output plus exact raw-bit `f64` observation, all as ordinary external functions;
- conditionals use mandatory-parenthesized `if` and `elif` conditions, mandatory arm blocks, an optional final `else`, and do not accept `else if`;
- conditional arms are tested left to right until the first true condition, only the selected block executes, and every arm has an independent lexical child scope;
- a conditional definitely returns only when it has `else` and every arm definitely returns;
- the restricted stage-0 object profile uses nominal top-level classes with
  primitive fields, exactly one explicit initializer, direct construction only
  into exact-type locals, and direct non-virtual receiver methods;
- classes and functions share one top-level namespace, fields and ordinary
  methods share one non-overloaded per-class namespace, and the contextual
  special `init` member occupies a separate slot;
- restricted initializers are straight-line field-assignment sequences that
  assign every field exactly once and never read an uninitialized field;
- empty restricted classes are valid and have a one-byte addressable x86-64
  layout; other fields use declaration-order checked target layout;
- restricted method receivers evaluate before explicit arguments and lower as
  hidden first integer-class arguments, while MIR retains semantic places and
  field identities rather than target offsets;
- the frozen class-typed inline-field profile permits exact concrete class
  field types while retaining primitive-only value parameters/results and
  place-only object semantics;
- inline class containment must be acyclic, is rejected semantically before
  target selection, and is laid out recursively in declaration order with
  checked target arithmetic;
- a class field is constructed exactly once by
  `self.field = ExactClass(arguments);`, becomes live only after its nested
  initializer returns normally, and never materializes as an object value;
- completed class fields may form nested primitive field places, direct method
  receivers, and exact-class alias arguments, with access inherited unchanged
  from the root local, receiver, or alias binding;
- class-typed fields do not by themselves enable whole-object replacement,
  implicit copying, destruction, partial-construction cleanup, inheritance,
  shared ownership, or checked exceptions;
- the frozen local deterministic-destruction profile adds one optional
  contextual `destroy { ... }` member with a mutable complete `self`, an
  implicit `unit` result, and ordinary unit-method statements;
- successfully initialized owning object locals register at complete
  constructor return and clean up on normal exits from innermost scope outward,
  in reverse registration order within each scope;
- `return` evaluates and preserves its primitive result before cleanup, and a
  complete object runs its user destruction body before recursively destroying
  class fields in reverse declaration order;
- the local destruction profile does not add explicit early destruction,
  object values or copying, failed-construction or exceptional cleanup,
  inheritance, shared ownership, deallocation, arrays, or loop exits;
- the frozen staged object-value profile retains one required ordinary
  initializer and adds independent optional user slots for exact-class copy
  construction and copy assignment;
- copy construction uses `init(ref other: T)` and is recognized from the
  enclosing class and exact parameter signature;
- copy assignment uses `assign(ref other: T)` and is recognized from the
  enclosing class and exact parameter signature;
- absent copy operations are synthesized in declaration order when all direct
  fields support the corresponding operation; object bytes are never copied as
  an unselected shortcut;
- object parameters own caller-constructed copies, object results use explicit
  caller-provided uninitialized storage, and class objects remain places rather
  than scalar MIR values;
- materialized object temporaries clean up in reverse completion order at the
  frozen full-expression boundaries, before enclosing return cleanup;
- constructors, copy constructors, copy assignment members, and destructors may have side effects;
- direct initialization and returning an ungrouped fresh exact-class
  construction permit optional copy elision, and the initial compiler policy
  is to elide every eligible occurrence deterministically;
- permitted elision may omit the complete selected copy-constructor operation
  and temporary destruction even when they have side effects, but never changes
  assignment into construction;
- ordinary instance `fn` methods have read-only receivers and mutable instance methods use `mut fn`;
- receiver mutability is enforced statically, propagates through inline subobjects, and has no runtime representation;
- read-only receiver access and `final` fields are shallow across `shared` ownership;
- receiver mutability is part of exact virtual-override and interface-method compatibility;
- `ref name: T` and `mut ref name: T` are non-owning alias-binding modes, not reference value types;
- parameter aliases are the only alias bindings in the first implementation, while restricted lexical local aliases are reserved for a later stage;
- alias parameters accept both inline places and matching shared pointees without separate function variants;
- all aliases are non-rebindable and non-escaping, and future local aliases remain subject to the same syntax-directed lifetime restrictions;
- the frozen restricted alias-parameter profile initially accepts exact
  concrete class aliases over inline locals, method `self`, and forwarded alias
  parameters; primitive aliases, shared sources, polymorphism, and local alias
  declarations remain outside that profile;
- restricted alias calls keep value and place arguments in one source-ordered
  sequence, represent an alias parameter as an indirect MIR place base, and
  pass one integer-class object pointer for either access mode;
- the restricted profile permits explicit direct-local construction through
  `init(ref other: T)` but does not thereby enable implicit or synthesized copy
  contexts;
- every shared allocation retains its complete dynamic type metadata across base, `Obj`, and interface conversions;
- final shared release invokes the most-derived complete-object destruction entry and frees the original allocation exactly once;
- shared destruction is automatically dynamic and does not require `destroy` to be declared virtual;
- every alias-bound argument has a caller-owned anchor for the complete call;
- stable locals and parameters can serve as zero-overhead anchors, while replaceable shared places use hidden shared copies;
- borrowing an inline subobject reached through shared storage anchors the containing shared allocation;
- borrow-anchor selection is syntax-directed and never performs a runtime object-graph search.

The remaining open questions do not invalidate the core memory-model direction, but some must be resolved before their associated features become normative.

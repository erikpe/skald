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

The implemented first post-vertical-slice output extension defines this
deliberately narrow external declaration form:

```ska
extern fn external_name(value: i64) -> unit;
extern fn external_value(value: i64) -> i64;
```

It is a top-level declaration terminated by a semicolon and has no Skald body.
Parameter names are mandatory. The implemented C2 profile permits by-value
`i64` and `bool` parameters and an `i64`, `bool`, or `unit` result.

The C-series boolean extension adds by-value `bool` parameters and `bool`
results to that same restricted profile:

```ska
extern fn external_predicate(value: i64) -> bool;
extern fn external_bool_sink(value: bool) -> unit;
```

It does not permit alias parameters, `shared`, objects, arrays, optionals,
function values, variadic arguments, alternate link names, or user-selected
calling conventions. Supporting `bool` does not otherwise generalize the
foreign-function interface.

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
`bool` (`_Bool`), and a Skald `unit` result corresponds to C `void`. `unit` has
no runtime payload. Boolean arguments use the System V integer class and leave
Skald as canonical C false or true values. An external boolean result is read
from the ABI result byte and normalized to a canonical Skald `false` or `true`
before it becomes observable to Skald code; unspecified upper result-register
bits are not part of the value. Argument evaluation is left to right, as for a
call to a Skald-defined function.

An external declaration is a trusted assertion about the definition supplied
at link time. `skac` checks Skald uses against the declared signature, and the
linker diagnoses a missing symbol, but the compiler cannot verify that a
supplied foreign definition has a compatible ABI type. An incompatible linked
definition is outside the language's safety and behavior guarantees.

This profile is sufficient to declare the bootstrap output functions in
Sections 13.1 and 13.2. It does not settle imports, export and visibility
behavior, cross-module coalescing of ABI declarations, separate compilation,
ownership transfer, or the complete foreign-function interface. Those remain
specification gaps.

---

## 4. Types and Binding Modes

### 4.1 Primitive Types

Skald provides the following primitive value types:

- `i64`
- `u64`
- `u8`
- `bool`
- `double`
- `unit`

Primitive types are always value types.

`bool` is distinct from every integer type. Its only values are the literals
`false` and `true`; it does not acquire numeric truthiness merely because a
target may encode those values as zero and one. The initial C-series compiler
profile supports `bool` in parameters, results, initialized locals,
expressions, and calls. Physical storage width is target-defined. In
particular, the initial stack-heavy backend may use an eight-byte home without
making `bool` an eight-byte language type or an alias for `i64`.

Default values:

- numeric types default to zero;
- `bool` defaults to `false`;
- `unit` has a single value.

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

### 5.5 Initialization Members

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
- copy assignment should handle self-assignment correctly;
- user-defined `init`, copy-construction `init`, `assign`, and `destroy` members may have side effects;
- the compiler may synthesize copy construction when the base subobject and all fields support copy construction;
- the compiler may synthesize copy assignment only when the base subobject and all fields support copy assignment and no final field would need to be reassigned;
- synthesized copy construction copies fields in declaration order;
- synthesized copy assignment assigns fields in declaration order unless a more precise rule is needed for safety;
- synthesized `shared T` field copy increments the shared handle reference count;
- synthesized `shared T` field assignment must retain the new handle before releasing the old handle to handle self-assignment and aliasing safely.

### 5.7 Destruction Members

A `destroy` declaration defines the class-specific destruction body that runs deterministically when an object's lifetime ends.

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

**Implementation status:** implemented end-to-end by C5 in the stage-0
x86-64 compiler, including exact diagnostics and native behavior coverage.

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

- an `i64` or `bool` function returns with `return expression;`, where the
  expression must have exactly the function's declared result type;
- a `unit` function returns with `return;` and cannot attach an expression;
- reaching the closing brace of a `unit` function is an implicit `return;`;
- every reachable path through a non-`unit` function must return a value, so
  reaching its closing brace is a compile-time error.

The first post-vertical-slice implementation supports expression statements
only for calls whose result is `unit`:

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

Primitive operator rules:

- arithmetic operators (`+`, `-`, `*`, `/`, `%`) require matching numeric operand types;
- signed/unsigned mixing requires explicit casts;
- unary minus is valid for `i64` and `double`;
- exponentiation (`**`) is integer-only initially: the left operand is `i64`, `u64`, or `u8`, the right operand is `u64`, and the result has the left operand's type;
- bitwise operators are valid for `i64`, `u64`, and `u8` and require matching operand types;
- shift operators accept an `i64`, `u64`, or `u8` left operand and a `u64` right operand, and the result has the left operand's type;
- right shift is arithmetic for `i64` and logical for `u64` and `u8`;
- shift counts greater than or equal to the left operand's bit width panic and abort;
- primitive casts are explicit.

Signed integer division rounds toward negative infinity, and signed remainder has the divisor's sign.

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

- `bool` converts to integer zero or one and to `double` zero or one;
- integers convert to `bool` as false for zero and true for nonzero;
- `double` converts to `bool` as false for positive or negative zero and true for every other value;
- integer-to-integer casts truncate to the target width and then interpret the resulting bits using the target signedness; these casts do not panic;
- integer-to-`double` casts use the source signedness and may lose precision;
- `double`-to-integer casts truncate toward zero and then range-check the result; NaN, infinity, and out-of-range values panic and abort.

These cast rules describe the intended broader primitive-type system. The
initial C-series boolean and conditional profile does not implement casts to
or from `bool`; its conditions require an expression already typed as `bool`.

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
- invalid primitive casts such as out-of-range `double -> i64`;
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

**Implementation status:** implemented by the stage-0 x86-64 compiler,
introduced in runtime ABI version 2, and retained in ABI version 3, with exact
source-to-stdout golden coverage.

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

**Implementation status:** implemented end-to-end by C2. The runtime symbol was
introduced by C1 in ABI version 3; the compiler accepts the declaration below
as an ordinary restricted external function.

Runtime ABI version 3 exposes:

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
not guaranteed. Adding this public symbol changed
`SKALD_RUNTIME_ABI_VERSION` from 2 to 3.

This operation exists only for bootstrap observability. It does not introduce
formatting, recoverable I/O, or a final standard-library printing API, and no
compiler phase recognizes its name specially.

---

## 14. Relationship to Niflheim

Skald originated in an exploratory draft called Niflheim2, which used the earlier Niflheim language and compiler as a design starting point. The memory model and several related semantics diverged enough that the project became a distinct language with a new name, compiler, source suffix, and repository. Niflheim remains historical context rather than a compatibility target or normative dependency of this specification.

Skald intentionally retains several ideas explored in Niflheim:

- statically typed compiled language;
- the primitive types `i64`, `u64`, `u8`, `bool`, `double`, and `unit`;
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

### 15.1 Features Deferred Beyond the First Vertical Slice

The following intended features are deliberately not specified well enough to implement yet:

- optionals, including presence binding, extraction, conversions, payload lifetime, and ownership behavior;
- arrays, including construction, element lifetime, copying, mutation, indexing, and slicing;
- loops and iteration, including `while`, `for ... in`, `break`, `continue`, and the iterator contract;
- checked exceptions, including throwing, catching, exception-set checking, cleanup, and lowering;
- locally declared alias bindings and scoped narrowing aliases; parameter aliases are the only supported alias-binding form in the first implementation.

Their existing sections preserve design direction and reserve likely syntax, but are non-normative where they do not give a complete rule. A compiler may omit these features from the first vertical slice without being considered inconsistent with this draft.

### 15.2 Other Major Underspecified Areas

The following are also substantial gaps. They need not all be part of the first vertical slice, but each must be settled before the corresponding language area is considered complete:

- **Lexical and grammatical definition:** the implemented first vertical slice has an explicit lexical and grammatical contract in [`grammar/README.md`](../grammar/README.md), but the complete language still needs token and comment rules, literal spelling, operator precedence and associativity, and rules for resolving syntactic ambiguities.
- **Name, type, and call resolution:** the implemented first vertical slice defines single-file function and lexical-local resolution in [`grammar/README.md`](../grammar/README.md), without overloading or implicit conversions. The complete language still needs cross-module forward references, declaration cycles, overload availability or prohibition, candidate selection, implicit-conversion ranking, and generic diagnostics for ambiguous or invalid calls.
- **Primitive edge-case semantics:** the first vertical slice defines decimal `i64` literal range checking, including the unary-minus spelling of `i64::MIN`, in [`grammar/README.md`](../grammar/README.md). Runtime arithmetic overflow and underflow, division or remainder by zero, the signed minimum divided by negative one, floating-point conformance and exceptional values, and whether constant evaluation diagnoses or reproduces runtime failures remain open.
- **Evaluation and cleanup ordering:** the first vertical slice now defines left-to-right operand and argument evaluation in [`grammar/README.md`](../grammar/README.md). The complete language still needs receiver ordering, full-expression boundaries, temporary destruction order, and cleanup sequencing for every control-flow exit. These are prerequisites before destructors, shared-handle temporaries, or borrow anchors can be implemented reliably.
- **Initialization rules:** definite initialization, default initialization in every storage context, field and base initialization order, and exact rules for implicit or unavailable constructors, copy constructors, assignment members, and destructors.
- **Static storage lifetime:** initialization and destruction order within and across modules, dependency cycles, and failure during static initialization.
- **Polymorphic narrowing through aliases:** checked downcasts and interface casts are named, but the scoped alias-binding form for using a successfully narrowed object is not yet defined. It must inherit access mode and remain within the source alias's lifetime.
- **Modules, build model, linkage, and foreign interfaces:** Section 3.1 defines only the implemented single-file bootstrap profile of exact-symbol C-ABI declarations over `i64`, `bool`, and `unit`. Source-to-module mapping, import discovery, exports, separate compilation, symbol visibility, cross-module external-declaration coalescing, additional ABI types, and ownership rules for foreign calls remain open.
- **Required library and runtime surface:** Sections 13.1 and 13.2 define only the implemented bootstrap `i64` and `bool` line-output operations. The minimum facilities for general I/O, dynamic storage or collections, diagnostics, and other practical programs are not yet identified. This is especially relevant to the eventual self-hosting compiler, even if it is outside the core language semantics.

The most urgent of these for the ownership model is evaluation and cleanup ordering. A scalar-only first vertical slice can postpone much of it, but an implementation should settle it before adding user-defined inline objects, deterministic destruction, shared ownership, or anchored borrowing.

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
- the implemented bootstrap external-function profile uses exact source identifiers as C-ABI linker symbols, accepts only by-value `i64` and `bool` parameters and `i64`, `bool`, or `unit` results, and treats declarations as trusted ABI assertions;
- on Linux x86-64 System V, Skald `bool` maps to C `bool` (`_Bool`), leaves Skald as canonical false or true, and external boolean results are normalized from the ABI result byte;
- compiler-generated function symbols cannot collide with valid exact external identifiers and do not reserve an ordinary Skald identifier prefix;
- external declarations and Skald function definitions share one non-overloaded namespace, and `main` must be a Skald definition;
- `unit` functions use `return;` or implicit fallthrough, while non-`unit` functions must return a value on every reachable path;
- the first implemented expression-statement subset contains only unit-producing calls;
- `ska_rt_println_i64` writes the shortest ASCII signed decimal representation and one LF, and a detected incomplete output is unrecoverable;
- runtime ABI version 3 implements `ska_rt_println_bool`, which writes lowercase ASCII `true` or `false` and one LF, uses the same unrecoverable detected-output-failure policy, and remains an ordinary external function;
- conditionals use mandatory-parenthesized `if` and `elif` conditions, mandatory arm blocks, an optional final `else`, and do not accept `else if`;
- conditional arms are tested left to right until the first true condition, only the selected block executes, and every arm has an independent lexical child scope;
- a conditional definitely returns only when it has `else` and every arm definitely returns;
- copy construction uses `init(ref other: T)` and is recognized from the enclosing class and exact parameter signature;
- copy assignment uses `assign(ref other: T)` and is recognized from the enclosing class and exact parameter signature;
- constructors, copy constructors, copy assignment members, and destructors may have side effects;
- direct initialization and returning freshly constructed values permit optional copy elision;
- optional copy elision may omit side-effectful copy construction and temporary destruction, but never changes assignment into construction;
- ordinary instance `fn` methods have read-only receivers and mutable instance methods use `mut fn`;
- receiver mutability is enforced statically, propagates through inline subobjects, and has no runtime representation;
- read-only receiver access and `final` fields are shallow across `shared` ownership;
- receiver mutability is part of exact virtual-override and interface-method compatibility;
- `ref name: T` and `mut ref name: T` are non-owning alias-binding modes, not reference value types;
- parameter aliases are the only alias bindings in the first implementation, while restricted lexical local aliases are reserved for a later stage;
- alias parameters accept both inline places and matching shared pointees without separate function variants;
- all aliases are non-rebindable and non-escaping, and future local aliases remain subject to the same syntax-directed lifetime restrictions;
- every shared allocation retains its complete dynamic type metadata across base, `Obj`, and interface conversions;
- final shared release invokes the most-derived complete-object destruction entry and frees the original allocation exactly once;
- shared destruction is automatically dynamic and does not require `destroy` to be declared virtual;
- every alias-bound argument has a caller-owned anchor for the complete call;
- stable locals and parameters can serve as zero-overhead anchors, while replaceable shared places use hidden shared copies;
- borrowing an inline subobject reached through shared storage anchors the containing shared allocation;
- borrow-anchor selection is syntax-directed and never performs a runtime object-graph search.

The remaining open questions do not invalidate the core memory-model direction, but some must be resolved before their associated features become normative.

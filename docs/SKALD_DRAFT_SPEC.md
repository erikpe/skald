# Skald Draft Language Specification

Status: legacy migration input.

This document preserves broader Skald design material while focused language
documents are verified and established. It is authoritative only for areas
that do not yet have a focused owner, and its sketches do not imply current
compiler support or frozen design. Start with the
[language overview](language/README.md) and [status matrix](language/STATUS.md).

Skald began as a design exploration derived from Niflheim, but it is a distinct
language rather than a backwards-compatible revision. Its central departure is
the memory model: Skald is not garbage collected.

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

Implemented type, value, literal, and expression semantics have moved to
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md). The remaining
subsections in this legacy draft are migration input for focused class,
ownership, and future-type documents; they are not evidence of implementation
or frozen design.

### 4.1 Primitive Types

The primitive type model and `unit` result semantics are authoritative in
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md#type-model).
Target representation and foreign mappings belong to later compiler and
runtime documentation.

#### 4.1.1 Numeric Literals

Literal typing, ranges, rounding, and value semantics are authoritative in
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md#literal-types-and-ranges).
Lexical spelling is authoritative in the
[implemented grammar](language/GRAMMAR.md#literals).

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

**Specification status:** exploratory and not implemented. The
[status matrix](language/STATUS.md#not-implemented) is authoritative; the
details below are migration input rather than a frozen string contract.

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

Implemented local declaration, initialization, visibility, scope, and shadowing
rules are authoritative in
[Functions and Control Flow](language/FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals).
Future local alias and ownership forms remain design input in their focused
sections until their maturity changes.

### 5.2 Functions

Implemented declarations, parameter categories, calls, results, and returns are
authoritative in
[Functions and Control Flow](language/FUNCTIONS_AND_CONTROL_FLOW.md). Broader
shared, optional, array, and polymorphic signatures remain design input owned
by their focused feature areas.

### 5.3 Function Values

Function values, calls through expression values, closures, and lambda literals
are not implemented or frozen. Their current maturity is authoritative in the
[status matrix](language/STATUS.md#not-implemented). Syntax, variance, capture,
and callable-source rules must be designed before this area becomes an
implementation contract.

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

Implemented receiver access and propagation rules are authoritative in
[Classes and Lifecycle](language/CLASSES_AND_LIFECYCLE.md#receivers-and-access).
Future interaction with shared ownership, `final`, inheritance, and interfaces
remains migration input for those focused feature areas.

#### 5.4.2 Restricted Stage-0 Inline-Object Profile

The historical restricted-profile narrative has been replaced by the current,
stage-independent class, initialization, and object-place contract in
[Classes and Lifecycle](language/CLASSES_AND_LIFECYCLE.md).

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
value-parameter = identifier ":" (primitive-type | class-name)
alias-parameter = ["mut"] "ref" identifier ":" class-name
```

`ref` and `mut ref` are parameter binding modes, not type constructors. The
bound name's type is the named class, and the mode is represented separately
from that type. `ref` is a keyword; `mut ref` is the only mutable spelling.
`ref mut`, repeated modifiers, and a binding mode in a local, field, result,
static, element, or capture position are invalid.

Alias parameters are accepted on internally defined top-level functions,
instance methods, and initializers. An external declaration cannot contain an
alias parameter. Ordinary internal by-value parameters may use concrete class
names; the resulting parameter follows the focused
[owning value-parameter contract](language/CLASSES_AND_LIFECYCLE.md#owning-value-parameters)
and is not an alias.
External value parameters remain primitive-only. This alias profile accepts
only an exact concrete class as the designated type. Primitive, `unit`,
optional, array, `shared`, interface, and function alias types, along with
inheritance and implicit conversions, remain outside the profile.

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

Local alias declarations, primitive alias parameters, shared sources and borrow
anchors, polymorphism, whole-object replacement, and alias-bearing function
values remain deferred. Exact-class internal value parameters and results are
specified by [classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md).

#### 5.4.4 Frozen Class-Typed Inline-Field Profile

The historical field-profile narrative has been replaced by the current
[inline containment](language/CLASSES_AND_LIFECYCLE.md#fields-and-finite-containment),
[ordinary initialization](language/CLASSES_AND_LIFECYCLE.md#ordinary-initializer-contract),
and [object-place](language/CLASSES_AND_LIFECYCLE.md#object-places-and-projections)
contracts. Compiler representation and target-layout details remain migration
input for their later implementation documents.

#### 5.4.5 Frozen Local Deterministic-Destruction Profile

The implemented normal-flow lifetime, registration, cleanup, destructor-body,
and field-order rules now live in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#lifetime-registration-and-normal-cleanup).
The archived roadmap records implementation history; this draft no longer
duplicates its earlier, narrower destructor-body restrictions.

#### 5.4.6 Frozen Exact-Class Object-Value Profile

The implemented copy capabilities, object assignment, owning parameters,
results, temporaries, elision, and exactly-once cleanup rules now live in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#copy-capabilities).
Alias binding remains in Section 4.5 until its focused migration.

### 5.5 Initialization Members

The exact-class initializer declaration and definite-field contract is
specified in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#ordinary-initializer-contract).
Default initialization, constructor families, and base initialization remain
unimplemented; their maturity belongs in the
[status matrix](language/STATUS.md#not-implemented).

### 5.6 Copy Constructors and Copy Assignment

The implemented declaration slots, independently synthesized capabilities,
operation selection, field order, and self-assignment behavior are specified
in [classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#lifecycle-declarations).
Future inheritance and ownership-bearing field kinds must extend that contract
in their focused design documents.

### 5.7 Destruction Members

The implemented destructor body and complete-object order are specified in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#complete-object-destruction).
Dynamic-type, shared-allocation, base-subobject, array, and exceptional cleanup
remain future design rather than current lifetime semantics.

---

## 6. Assignment, Copying, and Object Lifetime

The complete implemented exact-class contract is now authoritative in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md), including the
distinction between construction, assignment, and destruction. Alias-rooted
access and replacement restrictions remain in Section 4.5 until DOC7.

### 6.1 Optional Copy Elision

The two implemented, deterministically selected constructor-elision forms and
their grouping-sensitive boundary are specified in
[permitted copy elision](language/CLASSES_AND_LIFECYCLE.md#permitted-copy-elision).

### 6.2 Assignment to Parameters

Owning exact-class value parameters and their cleanup are specified in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#owning-value-parameters).
Alias mutation and non-rebinding rules remain in Section 4.5 until their
focused ownership migration.

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

Implemented expression and operator semantics have moved to
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md#expressions).
Implemented statement, block, conditional, return, call-statement, and
evaluation-order semantics have moved to
[Functions and Control Flow](language/FUNCTIONS_AND_CONTROL_FLOW.md).

Loops, iteration, `break`, and `continue` are not implemented or frozen. Their
scope, evaluation order, exit cleanup, nested targets, mutation behavior, and
iterator protocol require a focused design before implementation.

### 10.1 Conditional Statements

The implemented conditional contract is authoritative in
[Functions and Control Flow](language/FUNCTIONS_AND_CONTROL_FLOW.md#conditionals).
Accepted source shape is authoritative in the
[grammar](language/GRAMMAR.md#blocks-and-statements).

### 10.2 Returns and Call Statements

Return typing, definite-return analysis, call-statement legality, result
sequencing, and cleanup-before-return are authoritative in
[Functions and Control Flow](language/FUNCTIONS_AND_CONTROL_FLOW.md#returns-and-definite-return).

### 10.3 Operators

Implemented arithmetic, overflow boundaries, and unavailable operators are
authoritative in
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md#operators).
Maturity for additional primitive operations remains in the
[status matrix](language/STATUS.md#not-implemented).

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

The implemented language has no casts, implicit conversions, equality, or type
tests. Exact-type requirements and the maturity of future conversion and
equality behavior are authoritative in
[Types, Values, and Expressions](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).

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

The object-cast and type-test notes above remain migration input for the focused
polymorphism design. Per the [status matrix](language/STATUS.md#not-implemented),
they are not implemented or frozen language behavior.

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

The implemented runtime ABI is version 4. Every compiler-generated process
entry wrapper calls the no-op marker `ska_rt_abi_v4` before entering Skald
code. The marker's version is part of its linker symbol, so an archive built
for another ABI cannot satisfy the reference and executable linking fails.
Every incompatible ABI revision must introduce a new marker name and update
the compiler reference in the same change. `ska_rt_abi_version()` remains
available for runtime inspection and direct contract tests; querying it is not
the executable compatibility check.

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
  contract in [the implemented grammar](language/GRAMMAR.md), but the complete
  language still needs token and comment rules, additional literal families,
  later operator precedence and associativity, and rules for resolving
  syntactic ambiguities.
- **Name, type, and call resolution:** the implemented subset defines
  single-file function/class and lexical-local resolution without overloading
  or implicit conversions. The complete language still needs cross-module
  references, declaration cycles, overload availability or prohibition,
  candidate selection, conversion ranking, and ambiguity diagnostics.
- **Primitive edge-case semantics:** the implemented boundary and open signed
  `i64` overflow behavior are owned by
  [Types, Values, and Expressions](language/TYPES_AND_VALUES.md#operators).
  Division, remainder, shifts, explicit casts, comparisons, decimal floating
  formatting, and future constant evaluation remain open. Every additional
  backend must separately validate its target realization.
- **Evaluation and cleanup ordering:** the implemented subset defines
  left-to-right operands/arguments plus receiver, field, and direct-
  construction order. The current normal-flow cleanup, full-expression, and
  temporary rules are authoritative in
  [classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#temporaries-and-full-expressions).
  The complete language still needs cleanup sequencing for loops, exceptions,
  and later control-flow forms.
- **Initialization rules:** the implemented inline-object profile defines
  straight-line definite initialization, exact direct field construction,
  normal-return subobject liveness, nested access, acyclic containment, and
  exact-class copy capabilities in the current no-inheritance model. Default
  initialization in other storage contexts, base-subobject ordering, branching
  or throwing initializers, and partial-construction cleanup remain open.
- **Static storage lifetime:** initialization and destruction order within and across modules, dependency cycles, and failure during static initialization.
- **Polymorphic narrowing through aliases:** checked downcasts and interface casts are named, but the scoped alias-binding form for using a successfully narrowed object is not yet defined. It must inherit access mode and remain within the source alias's lifetime.
- **Modules, build model, linkage, and foreign interfaces:** Section 3.1 defines the implemented single-file exact-symbol profile and its planned extension over all primitive value types. Source-to-module mapping, import discovery, exports, separate compilation, symbol visibility, cross-module external-declaration coalescing, other ABI types, alternate calling conventions, and ownership rules for foreign calls remain open.
- **Required library and runtime surface:** Sections 13.1 through 13.3 define only bootstrap scalar observation operations. The minimum facilities for general I/O, decimal floating formatting, dynamic storage or collections, diagnostics, and other practical programs are not yet identified. This is especially relevant to the eventual self-hosting compiler, even if it is outside the core language semantics.

The implemented normal-flow lifecycle contract is authoritative in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md). Loop,
failed-construction, and exceptional cleanup remain broader ownership-model
gaps that must be settled before their associated features are implemented.

### 15.3 Open Design Questions

The following decisions are intentionally not finalized by this draft:

1. Should whole-object replacement through `mut ref` exist with explicit syntax?
2. Which explicit array initialization forms should be added for non-defaultable element types?
3. How much of the old Niflheim unsafe systems-layer proposal should exist in Skald, if any?
4. What is the exact checked-exception syntax and lowering strategy?

### 15.4 Resolved Decisions

Resolved decisions in this draft:

- the language is named Skald, its compiler is named `skac`, and source files use the `.ska` suffix;
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
- `ska_rt_println_i64` writes the shortest ASCII signed decimal representation and one LF, and a detected incomplete output is unrecoverable;
- the current runtime ABI implements `ska_rt_println_bool`, which writes
  lowercase ASCII `true` or `false` and one LF, uses the same unrecoverable
  detected-output-failure policy, and remains an ordinary external function;
- runtime ABI version 4 implements `u64` and `u8` decimal output plus exact raw-bit `f64` observation, all as ordinary external functions;
- exact-class initialization, copy capabilities, assignment, object parameters
  and results, temporaries, permitted elision, and deterministic destruction
  follow the implemented
  [class lifecycle contract](language/CLASSES_AND_LIFECYCLE.md);
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

# Niflheim2 Draft Language Specification

Status: exploratory draft.

This document defines a new language temporarily called **Niflheim2**. Niflheim2 is inspired by Niflheim, but it is not a backwards-compatible revision of Niflheim. The central difference is the memory model: Niflheim2 is not garbage collected. It uses deterministic object lifetimes, value semantics, built-in shared ownership, and call-scoped borrowed parameters.

The goal of this document is to describe the language itself, not its standard library.

---

## 1. Purpose and Scope

Niflheim2 is a learning-oriented, statically typed, compiled language. It should be practical for small personal projects while remaining simple enough that the compiler and runtime can be understood by one person.

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

Niflheim2 distinguishes four important ways an object can be used:

```nif
fn takes_value(dog: Dog) -> unit;         // copies an inline Dog value
fn takes_ref(dog: ref Dog) -> unit;       // borrows a Dog for this call
fn takes_mut(dog: mut ref Dog) -> unit;   // mutably borrows a Dog for this call
fn takes_shared(dog: shared Dog) -> unit; // copies a shared heap handle
```

Example call behavior:

```nif
var d: Dog = Dog();
var s: shared Dog = new Dog();

takes_value(d);       // copies Dog
takes_value(s);       // illegal unless an explicit pointee-copy form is later added

takes_ref(d);         // borrows inline Dog
takes_ref(s);         // borrows shared pointee

takes_mut(d);         // mutably borrows inline Dog
takes_mut(s);         // mutably borrows the shared pointee; s itself is not rebound

takes_shared(s);      // copies shared handle; increments reference count
takes_shared(d);      // illegal; inline Dog is not heap/shared
```

Key memory-model decisions:

- `Dog` is an inline value type.
- `shared Dog` is a non-null owning reference-counted heap handle.
- `ref Dog` and `mut ref Dog` are parameter modes, not general value types.
- `ref` parameters cannot be stored, returned, captured, assigned, or converted to `shared`.
- every borrowed argument has a caller-owned anchor that keeps its storage alive for the complete call;
- ordinary instance `fn` methods have read-only receivers, while `mut fn` methods have mutable receivers;
- `mut ref` is mutable but not exclusive. Two `mut ref` parameters may refer to the same object.
- Optional values are explicit using postfix `?`, for example `Dog?` or `shared Dog?`.
- Plain `Dog`, `shared Dog`, `ref Dog`, and `mut ref Dog` are never null.

---

## 3. Source Files, Modules, and Visibility

Niflheim2 keeps Niflheim's module-oriented shape unless a later design decision replaces it.

Supported declaration kinds:

- imports;
- classes;
- interfaces;
- top-level functions;
- external functions.

Module import forms are inherited from Niflheim:

```nif
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

```nif
util.Counter
util.make_counter()
```

Unqualified names resolve local-first. If multiple imports provide the same unqualified name and there is no local declaration shadowing them, unqualified use is a compile-time ambiguity error.

---

## 4. Types

### 4.1 Primitive Types

Niflheim2 uses the same primitive value types as Niflheim:

- `i64`
- `u64`
- `u8`
- `bool`
- `double`
- `unit`

Primitive types are always value types.

Default values:

- numeric types default to zero;
- `bool` defaults to `false`;
- `unit` has a single value.

### 4.2 Object Types

Class types are inline object types by default:

```nif
var dog: Dog = Dog();
```

An inline object has deterministic lifetime. It is constructed at initialization and destroyed when its storage lifetime ends. Assignment updates an already-live object and does not end its lifetime or invoke its destructor.

The word "inline" describes language semantics, not a required physical stack layout. A compiler may place values in registers, stack slots, caller-provided return storage, or optimized-away storage as long as observable construction/destruction behavior is preserved.

### 4.3 Shared Types

`shared T` is a built-in owning heap handle:

```nif
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

In Niflheim2, `Obj` is usually meaningful as a borrowed or shared polymorphic type:

```nif
fn describe(value: ref Obj) -> Str;
fn retain(value: shared Obj) -> unit;
```

Standalone inline variables of type `Obj` are not allowed initially:

```nif
var value: Obj = Dog(); // illegal
```

This avoids slicing arbitrary object values down to an empty or partial root object. Concrete object values should use their concrete class type, while polymorphic APIs should use `ref Obj`, `mut ref Obj`, or `shared Obj`.

### 4.5 Borrowed Parameter Modes

`ref T` and `mut ref T` are only valid in function, method, constructor, and interface method parameter declarations.

They are not general types. The following are illegal:

```nif
var local_ref: ref Dog = ...;
fn get_dog() -> ref Dog;
class Kennel {
    dog: ref Dog;
}
```

`ref T`:

- provides read-only access to an existing object for the duration of the call;
- may bind to an inline `T`;
- may bind to the pointee of a `shared T`;
- may call read-only instance methods but not `mut fn` methods on the borrowed object or its inline subobjects;
- cannot assign fields of the borrowed object or pass the object or its inline subobjects as `mut ref` arguments;
- cannot be assigned or rebound;
- cannot escape the call.

`mut ref T`:

- provides mutable access to an existing object for the duration of the call;
- may bind to a mutable inline `T`, including a `final` inline field reached through mutable containing-object access, because `mut ref` cannot replace the whole borrowed object;
- may bind to the pointee of a `shared T` handle even when the handle is stored in a `final` field, because finality of the handle is shallow;
- may call both read-only `fn` methods and mutable `mut fn` methods;
- cannot be assigned or rebound;
- cannot escape the call;
- does not imply exclusive access.

Example:

```nif
fn rename(dog: mut ref Dog, name: Str) -> unit {
    dog.name = name;
}

var d: Dog = Dog();
var s: shared Dog = new Dog();

rename(d, "Ada");
rename(s, "Turing");
```

Aliasing is allowed:

```nif
fn swap_names(a: mut ref Dog, b: mut ref Dog) -> unit {
    var tmp: Str = a.name;
    a.name = b.name;
    b.name = tmp;
}

var dog: Dog = Dog();
swap_names(dog, dog); // allowed; both parameters refer to the same object
```

This may produce surprising program behavior. It remains memory-safe because borrowed parameters cannot outlive the call and because the caller keeps the storage behind every borrow alive until the call returns.

Read-only access is an access restriction, not a guarantee that the object remains observably unchanged. Another aliased `mut ref` parameter may mutate the same object during the call. Code using a `ref T` simply cannot perform that mutation through the `ref T` access path.

#### 4.5.1 Borrow Anchors

Every `ref T` or `mut ref T` argument has a **borrow anchor** owned by the caller. The anchor guarantees that the storage containing the borrowed object remains alive for the complete dynamic execution of the call, including nested calls and exceptional cleanup. A borrowed parameter is still passed as a non-owning address; the anchor is caller-side state and is not part of the callee-visible parameter value.

Anchor selection is based on the source expression and its storage provenance:

- an inline local, inline value parameter, or inline static object is anchored by its existing storage;
- a pointee borrowed through a direct `shared T` local or `shared T` value parameter is anchored by that existing shared handle;
- a pointee borrowed from a replaceable shared place, such as a shared field, shared array element, or mutable shared static variable, is anchored by copying that handle into a hidden caller temporary;
- an inline field or base subobject reached through a shared object is anchored by a shared handle to the allocation that physically contains it;
- an inline array element is anchored by the array storage that physically contains it;
- an inline or shared temporary used as a borrowed argument has its lifetime extended until the call completes;
- forwarding an existing borrowed parameter to a nested call reuses the outer call's lifetime guarantee and does not create ownership from the borrow.

A stable shared local is the common zero-overhead heap-object case:

```nif
var dog: shared Dog = new Dog();
inspect(dog); // dog itself keeps the pointee alive; no shared copy is required
```

The callee cannot rebind a shared local belonging to its caller. Rebinding some other shared handle to the same allocation cannot destroy the pointee while the caller's local handle remains alive.

A replaceable shared place requires a hidden shared copy because code executed by the call may reach and overwrite the original place through another alias:

```nif
inspect(owner.dog); // owner.dog has type shared Dog
```

Conceptually, but not as user-visible source syntax, the caller lowers this as:

```nif
var __borrow_guard: shared Dog = owner.dog;
inspect_raw_address_of_pointee(__borrow_guard);
// __borrow_guard is released after normal or exceptional call completion
```

The hidden copy performs an ordinary `shared` retain and release. It does not allocate another pointee.

If the borrowed value is an inline field inside a shared object, the containing allocation is anchored instead:

```nif
class Owner {
    dog: Dog;
}

inspect(registry.current_owner.dog);
```

If `registry.current_owner` is a replaceable `shared Owner` place, the conceptual lowering is:

```nif
var __owner_guard: shared Owner = registry.current_owner;
inspect_raw_address_of_inline_field(__owner_guard, dog);
// __owner_guard is released after the call
```

The guard is a hidden `shared Owner` handle held in the caller's activation record or a register. It is not inserted into `registry`, stored beside `dog`, or found by walking the object graph. The compiler knows from the expression path that `dog` is physically contained in the `Owner` allocation. Even when the `shared Owner` handle is loaded from deep within a global structure, the compiler evaluates that lookup, copies or lifetime-extends the resulting handle, and then calculates the inline field address from the guarded allocation.

If a function or indexing operation returns a `shared Owner` value, the returned shared temporary itself may serve as the anchor:

```nif
inspect(registry.find_owner(id).dog);
```

Here the result of `find_owner` remains alive until `inspect` returns. No additional shared copy is required solely for borrowing if the returned temporary already owns the allocation.

The compiler establishes each required anchor as part of evaluating the corresponding argument, before later evaluation or user code can invalidate the source place. Hidden anchors are destroyed after the call in the ordinary cleanup order. Multiple borrows may use the same anchor; implementations may coalesce redundant hidden guards when doing so preserves observable retain, release, and destruction behavior.

Anchor selection is syntax-directed and local to expression lowering. It does not require a runtime ownership search, object-graph traversal, interprocedural lifetime inference, or general borrow checking. Safe code can maintain this property because borrowed values cannot be stored or returned and raw pointer construction is unavailable.

The initial language does not allow a borrow to target a conditionally alive payload, such as the contained `T` inside a `T?`, if another alias could remove that payload during the call. A later presence-binding design may add such borrows together with rules that preserve the payload lifetime.

### 4.6 Optional Types

Optionality is explicit and part of the type.

```nif
var dog: Dog? = maybe_get_dog();
var heap_dog: shared Dog? = maybe_get_shared_dog();
```

`T?` means either a `T` value or no value.

Plain non-optional values are never null:

```nif
var dog: Dog;             // always contains a Dog after definite initialization
var heap_dog: shared Dog; // always contains a valid shared handle
```

Optionality applies to the complete preceding type:

```nif
Dog?              // optional inline Dog
shared Dog?       // optional shared Dog handle
```

Because `ref` and `mut ref` are parameter modes rather than general types, optional borrowed parameters are written as:

```nif
fn inspect(dog: ref Dog?) -> unit;
fn rename_if_present(dog: mut ref Dog?) -> unit;
```

In this draft, `ref Dog?` means "borrow an optional Dog value". It does not mean "an optional borrow". Since borrowed parameters cannot be stored or returned, optional borrow values are not part of the initial model.

Open question: if later versions make `ref` a first-class type, the language should distinguish `(ref Dog)?` from `ref (Dog?)`.

The draft spelling for the empty optional value is:

```nif
none
```

Using a value of type `T?` requires explicit presence handling. The exact pattern-matching or unwrap syntax is deferred.

### 4.7 Array Types

Niflheim2 uses the same fixed-size array type constructor as Niflheim:

```nif
u8[]
i64[]
Dog[]
shared Dog[]
Dog[][]
```

Array construction:

```nif
var bytes: u8[] = u8[](1024);
var dogs: Dog[] = Dog[](8);
var heap_dogs: shared Dog?[] = shared Dog?[](8);
```

Default array construction is valid only when the element type has a default value:

```nif
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

Later versions may add explicit initialization forms for non-defaultable element types, such as initializer lists, fill constructors, or per-element generator syntax. For the first MVP implementation, `shared Dog[](8)` is illegal and `shared Dog?[](8)` is legal.

### 4.8 Str

`Str` is the built-in immutable string type.

`Str` is a small inline value, not a garbage-collected reference. It is backed by immutable byte storage containing `u8` bytes. The language assigns no Unicode or text-normalization semantics initially; string contents are raw bytes, as in Niflheim.

Conceptual shape:

```nif
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
- String manipulation should be implementable mostly in Niflheim2 code using `Str` methods and separate mutable builder/buffer types.
- A future `StrBuf` or byte-buffer type should provide mutable construction and editing, then produce an immutable `Str`.

String literals have type `Str`.

```nif
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

Niflheim2 keeps Niflheim's local declaration syntax:

```nif
var name: Type = initializer;
```

Examples:

```nif
var count: i64 = 0;
var dog: Dog = Dog();
var maybe_dog: Dog? = none;
var heap_dog: shared Dog = new Dog();
```

Statement terminators are kept as in Niflheim; ordinary statements and declarations use semicolons where Niflheim requires them.

Variables must be definitely initialized before use. Uninitialized values must not be observable.

### 5.2 Functions

Function declarations:

```nif
fn name(param1: Type, param2: Type) -> ReturnType {
    ...
}
```

Parameters may use value types, `shared` types, and borrowed parameter modes:

```nif
fn copy_in(dog: Dog) -> unit;
fn borrow_in(dog: ref Dog) -> unit;
fn mutate_in(dog: mut ref Dog) -> unit;
fn share_in(dog: shared Dog) -> unit;
```

Parameter passing:

- `T` copies the argument into the callee.
- `shared T` copies the shared handle into the callee.
- `ref T` passes a call-scoped read-only borrow.
- `mut ref T` passes a call-scoped mutable borrow.

Return values may be primitives, inline objects, optionals, arrays, function values, or `shared` handles. Returning `ref` or `mut ref` is illegal.

### 5.3 Function Values

Niflheim2 initially keeps Niflheim's capture-free function value model.

Type syntax:

```nif
fn(i64, i64) -> i64
fn(ref Dog) -> unit
```

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

```nif
class Dog extends Animal implements Named {
    name: Str;

    constructor(name: Str) {
        self.name = name;
    }

    virtual fn speak() -> unit {
        ...
    }
}
```

Classes support:

- fields;
- constructors;
- copy constructors;
- copy assignment;
- destructors;
- instance methods;
- static methods;
- static variables;
- `private` members;
- `final` fields;
- single inheritance via `extends`;
- interface conformance via `implements`;
- explicitly declared virtual methods;
- explicit `override` for overridden methods.

#### 5.4.1 Instance-Method Receiver Mutability

Every instance method has an implicit receiver access mode. Ordinary `fn` methods have a read-only receiver by default. Methods that may mutate the receiver are declared with `mut fn`:

```nif
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
- constructors, copy constructors, copy assignment members, and destructors have an implicitly mutable `self` and do not use the `mut fn` spelling;
- static methods and top-level functions have no receiver, so `mut` is not valid on them;
- receiver mutability is not part of capture-free function-value type syntax because instance method values are out of scope initially.

In a read-only instance method, code cannot:

- assign an instance field of `self`;
- call a `mut fn` method on `self`;
- mutate an inline field, base subobject, or inline array element contained in `self`;
- pass `self` or any of those inline subobjects to a `mut ref` parameter.

It may read and copy fields, call read-only methods, allocate objects, perform I/O, and modify separate objects or static state when otherwise permitted. Receiver read-only access is not a purity or side-effect annotation.

Read-only access is shallow across shared ownership. A read-only method cannot replace a `shared T` field of `self`, but receiver read-only access does not extend through the handle to the `T` pointee. The pointee remains mutable through a copied or otherwise available shared handle, even if runtime aliasing causes that pointee to be the same object as one reached through another read-only path:

```nif
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

Access through `ref T` uses the same read-only receiver rules. Access through `mut ref T`, a mutable inline object, or a `shared T` pointee may call both `fn` and `mut fn` methods. Restricting mutable access to read-only access is allowed and requires no runtime conversion; granting mutable access through an existing read-only path is illegal.

```nif
fn inspect(dog: ref Dog) -> Str {
    dog.rename("Rex");       // illegal: rename has a mutable receiver
    return dog.get_name();   // allowed
}

fn update(dog: mut ref Dog) -> unit {
    var old_name: Str = dog.get_name(); // allowed
    dog.rename("Rex");                  // allowed
}
```

The initial language has no separate `const T` type syntax. The compiler tracks receiver access mode during type checking, and binding an object to `ref T` restricts the available access rather than casting the object to a different type. This does not create a distinct runtime representation, change object layout, or emit a runtime cast. Access mode propagates through inline fields and base subobjects because those values are physically part of the receiver.

`final` is independent of receiver mutability and is shallow. A final field can be initialized during construction but cannot later be reassigned as a whole. A final inline object field may still be changed through its own `mut fn` methods when reached through a mutable containing object, and a final `shared T` field may still be used to mutate its separately allocated pointee. Finality prevents whole-field reassignment; it does not recursively freeze the field's internal state or an object graph.

### 5.5 Constructors

Constructors initialize object storage.

```nif
constructor(name: Str) {
    self.name = name;
}
```

Rules:

- a class may declare zero or more constructors;
- if no constructor is declared, a default or compatibility constructor may be synthesized when all fields can be initialized;
- copy constructors use `constructor(copy other: ref T)` syntax;
- subclass constructors must initialize the base subobject before subclass fields;
- `super(...)` is constructor-only in the initial language;
- if construction fails in a later exception-enabled language, only fully constructed subobjects are destroyed.

### 5.6 Copy Constructors and Copy Assignment

Copy construction initializes a new object from an existing object of the same type.

```nif
constructor(copy other: ref Dog) {
    self.name = other.name;
}
```

Copy assignment updates the value of an already-initialized object from an existing object of the same type. The destination object remains alive throughout the operation.

```nif
assign(copy other: ref Dog) {
    self.name = other.name;
}
```

Rules:

- `constructor(copy other: ref T)` is the copy constructor for class `T`;
- `assign(copy other: ref T)` is the copy assignment member for class `T`;
- copy constructors initialize uninitialized storage;
- copy assignment operates on an already-initialized object;
- copy construction initializes final fields like any other construction, but copy assignment cannot reassign a final field;
- copy assignment should handle self-assignment correctly;
- user-defined constructors, copy constructors, copy assignment members, and destructors may have side effects;
- the compiler may synthesize copy construction when the base subobject and all fields support copy construction;
- the compiler may synthesize copy assignment only when the base subobject and all fields support copy assignment and no final field would need to be reassigned;
- synthesized copy construction copies fields in declaration order;
- synthesized copy assignment assigns fields in declaration order unless a more precise rule is needed for safety;
- synthesized `shared T` field copy increments the shared handle reference count;
- synthesized `shared T` field assignment must retain the new handle before releasing the old handle to handle self-assignment and aliasing safely.

### 5.7 Destructors

Destructors run deterministically when an object's lifetime ends.

Draft syntax:

```nif
destructor {
    ...
}
```

Rules:

- each class may declare at most one destructor;
- destructors take no parameters and return `unit`;
- destructors must not throw in the initial exception design; if an exception escapes a destructor, the program terminates;
- destruction begins with the destructor body of the most-derived class;
- after a class's destructor body, its fields are destroyed in reverse declaration order;
- after that class's fields, its direct base subobject is destroyed using the same body-then-reverse-fields procedure;
- destruction continues through the base-class chain until the complete object has been destroyed;
- an absent user-declared destructor is treated as an empty destructor body, so fields and the base subobject are still destroyed;
- destroying a `shared T` handle may trigger complete dynamic destruction of the pointee if it was the last owner;
- assigning to an inline object never invokes that object's destructor or ends its lifetime, although its assignment member may release or destroy values owned by its fields.

For example, destruction of a heap-allocated `Dog extends Animal` occurs in this order:

1. `Dog` destructor body;
2. `Dog` fields in reverse declaration order;
3. `Animal` destructor body;
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

Niflheim2 uses copy semantics by default. Move semantics are not part of the initial language.

Assignment to an inline object updates an already-initialized value:

```nif
var dog: Dog = Dog("A");
dog = Dog("B");
```

This invokes the class assignment rules and leaves `dog` containing the new value. Assignment does not end `dog`'s lifetime and does not run `Dog`'s destructor. The assignment member may release or destroy old field values as part of updating them.

Copying:

- primitives copy by value;
- inline objects use `constructor(copy other: ref T)` or synthesized fieldwise copy construction;
- arrays copy according to the array copy policy, initially expected to be deep element copy;
- `shared T` copies the handle and increments the reference count;
- copying `shared T` does not invoke `T`'s copy constructor;
- `ref` and `mut ref` parameters cannot be copied as values.

Assignment:

- primitives assign by value;
- inline objects use `assign(copy other: ref T)` or synthesized fieldwise assignment;
- `shared T` assignment copies the new handle and releases the old handle;
- `ref` and `mut ref` parameters are not assignable or rebindable.

Classes with ownership-sensitive fields follow a "rule of three" style:

- if a class defines a destructor, it likely also needs explicit copy construction and copy assignment;
- if a class defines copy construction or copy assignment, it likely needs the other;
- the compiler may synthesize these operations when the base subobject and fields support them, subject to the final-field restriction on copy assignment above.

Constructors, copy constructors, copy assignment members, and destructors are not assumed to be side-effect-free. The compiler may optimize them only when the language explicitly permits elision or when it can prove observable behavior is unchanged.

### 6.1 Optional Copy Elision

Niflheim2 permits, but does not require, copy elision in two cases involving a fresh inline object constructor expression of the exact destination type:

1. direct initialization of a new object;
2. returning a freshly constructed object from a function.

Direct-initialization example:

```nif
var dog: Dog = Dog("Rex");
```

Without elision, the constructor expression creates a temporary `Dog`, `dog` is copy-constructed from that temporary, and the temporary is then destroyed. With elision, the implementation constructs `Dog("Rex")` directly in `dog`'s storage and omits both the copy-constructor call and the destruction of the omitted temporary.

Return example:

```nif
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

```nif
var dog: Dog = Dog("Old");
dog = Dog("New");
```

The constructor expression creates a source temporary, `dog.assign(copy ...)` or synthesized copy assignment updates the existing `dog`, and the source temporary is then destroyed. This is semantically assignment, not destruction followed by construction, and it is not eligible for the optional copy-elision rule. The compiler may replace it with another implementation only when it proves that construction, assignment, destruction, and borrow-visible behavior remain unchanged.

Move-only values are out of scope for the initial language.

### 6.2 Assignment to Parameters

Value parameters are local variables inside the callee:

```nif
fn f(dog1: Dog, dog2: ref Dog, dog3: mut ref Dog, dog4: shared Dog) -> unit {
    dog1 = Dog();      // ok: assigns to the local copy
    dog2 = Dog();      // illegal: ref parameter is not assignable
    dog3 = Dog();      // illegal: mut ref parameter is not assignable/rebindable
    dog4 = new Dog();  // ok: replaces local shared handle
}
```

Mutation through `mut ref` is allowed:

```nif
fn rename(dog: mut ref Dog, name: Str) -> unit {
    dog.name = name;   // ok
}
```

Whole-object replacement through `mut ref` is not part of the initial language because it is visually ambiguous with rebinding the parameter.

---

## 7. Heap Allocation and Shared Ownership

Heap allocation is explicit:

```nif
var dog: shared Dog = new Dog("Rex");
```

`new T(args...)`:

- allocates storage for `T`;
- constructs a `T` in that storage;
- returns `shared T`;
- never returns null;
- panics or aborts on out-of-memory in the initial language.

Reassigning a `shared T` variable releases the old handle:

```nif
var dog: shared Dog = new Dog("A");
dog = new Dog("B");
```

If the old handle was the last owner, the old heap object is destroyed immediately unless a caller-side borrow anchor still owns it. In that case, replacement releases the original handle, while the anchor delays destruction until the anchored call completes.

Borrow anchors also prevent replacement through another alias during a call from leaving a dangling borrowed parameter. Reassigning a `shared` variable after the anchored call has returned cannot leave a dangling `ref` value in user code because borrowed values cannot escape the call.

---

## 8. Classes, Inheritance, and Polymorphism

### 8.1 Inline Values and Slicing

Assigning a derived inline value to a base inline variable slices:

```nif
var derived: Dog = Dog();
var base: Animal = derived;
```

`base` contains a copied `Animal` base subobject. It does not remain dynamically connected to `derived`.

### 8.2 Shared Upcasts

`shared Derived` may be implicitly upcast to `shared Base` when `Derived extends Base`:

```nif
var dog: shared Dog = new Dog();
var animal: shared Animal = dog;
```

This copies the shared handle. The underlying heap object remains a `Dog`. The converted handle preserves the complete-object address and the allocation's dynamic `Dog` metadata; it does not replace that metadata with `Animal` metadata.

If `animal` becomes the last owner, releasing it runs the complete `Dog` destruction sequence and then frees the original `Dog` allocation. The static type `Animal` controls which operations are available through the handle, but it never selects shared destruction.

### 8.3 Borrowed Upcasts

`Derived` may be passed to a `ref Base` or `mut ref Base` parameter:

```nif
fn speak(animal: ref Animal) -> unit {
    animal.speak();
}

var dog: Dog = Dog();
var heap_dog: shared Dog = new Dog();

speak(dog);
speak(heap_dog);
```

The borrow refers to the original object or shared pointee. No slicing occurs for borrowed parameters.

### 8.4 Virtual Dispatch

Instance methods are non-virtual by default. Virtual dispatch is enabled only for methods explicitly declared `virtual`.

This follows the C++ direction: ordinary methods have direct-call semantics, while virtual methods opt into dynamic dispatch and per-object/type dispatch metadata.

Complete-object destruction of a shared allocation is separate from user-visible virtual method dispatch. Destructors do not use `virtual` syntax, and a base class does not need to opt into safe polymorphic destruction. The shared runtime always selects the compiler-generated complete-object destruction entry from the allocation's dynamic type metadata.

Example:

```nif
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
- private methods, static methods, and constructors are not virtual;
- non-virtual method calls are statically resolved;
- virtual read-only `fn` calls through `ref Base`, `mut ref Base`, and `shared Base` dispatch according to the dynamic object type;
- virtual `mut fn` calls require mutable receiver access and therefore cannot be made through `ref Base`;
- calls on sliced inline base values dispatch as the sliced base value.

---

## 9. Interfaces

Niflheim2 keeps Niflheim's interface concept, adjusted to the new value/shared/ref model.

Interface declarations contain method signatures:

```nif
interface Hashable {
    fn hash_code() -> u64;
}

interface Named {
    fn get_name() -> Str;
    mut fn set_name(name: Str) -> unit;
}
```

Classes declare conformance:

```nif
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

Interface use should primarily happen through borrowed parameters and shared handles:

```nif
fn print_hash(value: ref Hashable) -> unit {
    var h: u64 = value.hash_code();
}

var key: Key = Key();
var heap_key: shared Key = new Key();

print_hash(key);
print_hash(heap_key);
```

Standalone inline variables of interface type are not allowed initially:

```nif
var value: Hashable = Key(); // illegal
```

Interface use should go through `ref Interface`, `mut ref Interface`, and `shared Interface`. This avoids needing a general inline interface-object representation.

A `ref Interface` receiver may call only read-only interface methods. A `mut ref Interface` receiver may call both read-only and mutable interface methods. Interface dispatch does not change these access rules.

A `shared C` handle may be implicitly converted to `shared I` when class `C` implements interface `I`:

```nif
var heap_key: shared Key = new Key();
var hashable: shared Hashable = heap_key;
```

The interface conversion copies the same owning handle and preserves the complete-object address, reference count, allocation identity, and dynamic class metadata. If `hashable` is the final owner, release runs the complete dynamic `Key` destruction sequence before freeing the original allocation. Interface method tables participate in dispatch but do not select destruction.

---

## 10. Expressions and Statements

Niflheim2 keeps the ordinary expression and statement surface from Niflheim where it remains compatible with the new type model.

Statements:

- block;
- local variable declaration;
- assignment;
- expression statement;
- `if` / `else`;
- `while`;
- `for ... in`;
- `return`;
- `break`;
- `continue`;
- constructor-only `super(...)`.

Expressions:

- literals;
- local references;
- field access;
- static member access;
- function calls;
- method calls;
- constructor calls;
- `new` heap allocation;
- unary and binary operators;
- explicit casts;
- type tests with `is`;
- indexing and slicing;
- array construction.

### 10.1 Operators

Primitive operator rules match Niflheim initially:

- arithmetic operators require matching numeric operand types;
- signed/unsigned mixing requires explicit casts;
- unary minus is valid for `i64` and `double`;
- bitwise operators are valid for integer types;
- shift counts are checked;
- primitive casts are explicit.

Signed integer division and remainder follow the existing Niflheim policy unless revised: division rounds toward negative infinity, and the remainder has the divisor's sign.

### 10.2 Indexing, Slicing, and For-In

Arrays support:

```nif
arr.len()
arr[index]
arr[index] = value
arr[start:end]
arr[start:end] = value
```

Indexing and slicing syntax may also be structural sugar over methods:

```nif
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

Consequently, indexing and slicing reads are available through both read-only and mutable receiver access. Index and slice assignment require mutable receiver access. For built-in arrays, the same rule means that an array reached as an inline subobject through `ref T` can be read but not modified.

`for ... in` uses the existing Niflheim structural iteration shape:

```nif
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

Primitive casts are explicit and use Niflheim's existing primitive cast semantics.

Object casts:

- derived-to-base inline assignment slices;
- derived-to-base `ref` and `shared` conversions are implicit;
- class-to-implemented-interface `ref` and `shared` conversions are implicit;
- interface-to-`Obj` `ref` and `shared` conversions are implicit;
- downcasts are explicit and checked at runtime;
- interface casts are explicit and checked at runtime when not statically known;
- every conversion or checked cast of a shared handle preserves its ownership pointer, allocation identity, reference count, and complete dynamic class metadata.

`is` performs a runtime type/conformance test for object, shared, and borrowed receiver forms.

Equality:

- primitive equality is value equality;
- inline object equality is not implicit unless a later operator-overload or protocol rule is added;
- `shared T` equality compares object identity by default;
- borrowed object identity comparison may be provided explicitly later, but is not needed for the initial core;
- optional equality is defined only when the contained type has equality.

---

## 12. Error Model

The initial language keeps unrecoverable runtime failures:

- failed checked casts;
- out-of-bounds array access;
- invalid primitive casts such as out-of-range `double -> i64`;
- explicit panic;
- out-of-memory.

These failures panic and abort, unless a future rule explicitly maps a specific operation into checked exception handling.

### 12.1 Checked Exceptions

Checked exceptions are part of the Niflheim2 language design, but a first implementation may stage them after basic RAII, calls, and destruction are working.

Draft syntax:

```nif
class IoError extends Exception {
    message: Str;
}

fn read_file(path: ref Str) -> Str throws IoError {
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

Design constraints already implied by Niflheim2:

- unwinding must run destructors for all fully constructed inline locals, fields, arrays, and shared handles;
- destructors must not throw initially;
- throwing during destruction terminates the program;
- constructors that throw must destroy fully constructed subobjects but not run the destructor for the incomplete whole object;
- all compiler IR that can branch to exceptional control flow must preserve cleanup ordering.

Implementation may initially lower exceptions to an explicit hidden result/exception channel rather than native platform unwinding. This keeps the runtime smaller and makes destructor cleanup paths visible in the compiler.

---

## 13. Runtime Model

Niflheim2's runtime should be much smaller than Niflheim's GC runtime.

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

Each concrete class's dynamic type metadata contains a compiler-generated complete-object destruction entry or equivalent operation. Given the complete object payload, this entry runs the most-derived destructor body, destroys fields and base subobjects in the language-defined order, and returns without freeing an adjusted base or interface view address. The shared runtime frees the original allocation only after this entry completes.

Reference-count operations:

- retain on `shared` copy;
- release on `shared` destruction/overwrite;
- when release reaches zero, load the allocation's dynamic type metadata and invoke its complete-object destruction entry;
- after complete-object destruction, free the original allocation exactly once.

The static type of the releasing handle is not an input to destruction. A release through `shared Derived`, `shared Base`, `shared Obj`, or `shared Interface` follows the same allocation header and therefore selects the same most-derived destruction entry. This dynamic destruction is mandatory for all shared allocations and does not depend on whether any destructor or ordinary instance method was declared `virtual`.

Borrow anchors do not require a runtime ownership search or a separate runtime ownership structure. A hidden shared anchor uses the same retain and release operations as any other shared copy. Direct inline storage, stable shared locals and parameters, forwarded borrowed parameters, and already-owning temporaries require no additional reference-count operation solely for the borrow. Hidden anchors are compiler-managed caller temporaries and must participate in normal and exceptional cleanup.

Thread-safe reference counting is out of scope unless concurrency is added later.

---

## 14. Compatibility with Niflheim

Niflheim2 intentionally keeps many Niflheim ideas:

- statically typed compiled language;
- Niflheim primitive types;
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

Niflheim2 intentionally changes or removes:

- garbage-collected references;
- nullable reference values by default;
- implicit virtual dispatch by default;
- implicit mutable receiver access for every instance method; ordinary `fn` receivers are read-only and mutation requires `mut fn`;
- ordinary reference-typed locals/fields/returns;
- GC root/safepoint semantics;
- null as the default value for reference-like types;
- absence of recoverable exceptions; Niflheim2 adds checked exceptions to the design.

Old Niflheim code should not be expected to compile as Niflheim2 code without changes.

---

## 15. Open Design Questions

The following decisions are intentionally not finalized by this draft:

1. Should whole-object replacement through `mut ref` exist with explicit syntax?
2. Which explicit array initialization forms should be added for non-defaultable element types?
3. How much of the old Niflheim unsafe systems-layer proposal should exist in Niflheim2, if any?
4. What is the exact checked-exception syntax and lowering strategy?

Resolved decisions in this draft:

- local declarations keep Niflheim's `var name: Type` syntax;
- statement terminators stay as in Niflheim;
- destructor declarations use the name `destructor`;
- instance methods and constructors use `self`, not `__self`, for the current object;
- virtual dispatch is opt-in with `virtual`;
- inline interface-typed variables are not allowed initially;
- `Obj` remains the universal root type, mainly for `ref Obj`, `mut ref Obj`, and `shared Obj`.
- default array construction is valid only for element types with default values;
- array physical storage placement is an implementation detail;
- `Str` is an immutable small inline value backed by immutable byte storage;
- string literals lower to `Str` values backed by compiler-emitted static immutable bytes.
- copy construction uses `constructor(copy other: ref T)`;
- copy assignment uses `assign(copy other: ref T)`;
- constructors, copy constructors, copy assignment members, and destructors may have side effects;
- direct initialization and returning freshly constructed values permit optional copy elision;
- optional copy elision may omit side-effectful copy construction and temporary destruction, but never changes assignment into construction;
- ordinary instance `fn` methods have read-only receivers and mutable instance methods use `mut fn`;
- receiver mutability is enforced statically, propagates through inline subobjects, and has no runtime representation;
- read-only receiver access and `final` fields are shallow across `shared` ownership;
- receiver mutability is part of exact virtual-override and interface-method compatibility;
- every shared allocation retains its complete dynamic type metadata across base, `Obj`, and interface conversions;
- final shared release invokes the most-derived complete-object destruction entry and frees the original allocation exactly once;
- shared destruction is automatically dynamic and does not require a virtual destructor declaration;
- every borrowed argument has a caller-owned anchor for the complete call;
- stable locals and parameters can serve as zero-overhead anchors, while replaceable shared places use hidden shared copies;
- borrowing an inline subobject reached through shared storage anchors the containing shared allocation;
- borrow-anchor selection is syntax-directed and never performs a runtime object-graph search.

These questions are not blockers for the core memory-model direction.

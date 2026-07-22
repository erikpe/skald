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

Only the exact-class inline value and call-scoped alias cases above are current
compiler behavior. Every `shared` example is exploratory and subject to the
[future ownership boundary](language/ALIASES_AND_OWNERSHIP.md#future-ownership-boundary).

Key memory-model decisions:

- `Dog` is an inline value type.
- `shared Dog` is an exploratory non-null shared-ownership direction, not an
  implemented or frozen type.
- `ref name: Dog` and `mut ref name: Dog` are implemented alias-binding forms;
  `Dog` remains the bound name's type.
- Alias bindings currently exist only for exact-class parameters and cannot be
  stored, returned, captured, assigned, or converted to `shared`.
- Current eligible inline places remain live for the call without a separate
  borrow anchor; anchors for future ownership forms are not frozen.
- ordinary instance `fn` methods have read-only receivers, while `mut fn` methods have mutable receivers;
- `mut ref` is mutable but not exclusive. Two mutable alias parameters may refer to the same object.
- Optional values are explicit using postfix `?`, for example `Dog?` or `shared Dog?`.
- Plain `Dog` and `shared Dog` are never null, and an alias binding must always designate a live `Dog` place.

---

## 3. Source Files, Modules, and Visibility

The verified single-file compilation unit, top-level namespace, entry point,
and future module-design boundary have moved to
[modules and foreign interoperation](language/MODULES_AND_INTEROP.md). The old
import, export, qualification, and visibility examples were never frozen Skald
syntax and have been removed.

### 3.1 Restricted Bootstrap External Functions

The implemented primitive external-function contract is authoritative in
[modules and foreign interoperation](language/MODULES_AND_INTEROP.md). Target
classification, C type mappings, compiler-generated symbols, runtime version
markers, and linker invocation are implementation details owned by the
backend, runtime, and driver documentation.

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

Shared ownership is not implemented or frozen. Its retained direction and the
open borrow-anchor boundary are recorded in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md#future-ownership-boundary);
feature maturity remains authoritative in the
[status matrix](language/STATUS.md#not-implemented).

### 4.4 Universal Root Type

The exploratory `Obj` view direction and the unresolved semantic-root versus
physical-base choice are owned by
[polymorphism](language/POLYMORPHISM.md#universal-obj-views). `Obj` is not an
implemented or frozen type.

### 4.5 Alias Binding Modes

The implemented exact-class parameter modes, eligible places, access
propagation, forwarding, overlap, copy interaction, and non-escape rules are
authoritative in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md).

#### 4.5.1 Borrow Anchors

Current alias sources are stable inline places and require no borrow anchor.
Anchoring for future shared-owned, replaceable, temporary, optional, or array
storage is an open ownership-design boundary, summarized in
[future ownership](language/ALIASES_AND_OWNERSHIP.md#future-ownership-boundary).

#### 4.5.2 Deferred Local Alias Bindings

Local aliases are not implemented and their syntax, sources, lexical lifetime,
control-flow behavior, and anchoring rules are not frozen. The focused
[future ownership boundary](language/ALIASES_AND_OWNERSHIP.md#future-ownership-boundary)
owns those open constraints.

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

Optional alias parameters and aliases into optional payloads are not
implemented or frozen. A future design must distinguish access to a container
from a scoped view of a present payload and guarantee that a payload cannot
disappear while aliased; the current exact-class parameter contract does not
settle the syntax or lifetime rule.

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
- element aliasing, storage stability, and any anchoring requirement remain
  open with the array design.

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

Implemented exact-class declarations and members are authoritative in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md). Exploratory bases,
inherited members, virtual methods, conformance, and broader class features
are separated into the [polymorphism design](language/POLYMORPHISM.md) and the
[status matrix](language/STATUS.md#not-implemented).

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

The historical implementation profile has been replaced by the verified,
stage-independent source contract in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md). Target-specific
parameter realization remains an implementation concern in Section 13.5 and
the living backend architecture until its focused migration.

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
Alias binding is authoritative in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md).

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
access and replacement restrictions are authoritative in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md#access-propagation).

### 6.1 Optional Copy Elision

The two implemented, deterministically selected constructor-elision forms and
their grouping-sensitive boundary are specified in
[permitted copy elision](language/CLASSES_AND_LIFECYCLE.md#permitted-copy-elision).

### 6.2 Assignment to Parameters

Owning exact-class value parameters and their cleanup are specified in
[classes and lifecycle](language/CLASSES_AND_LIFECYCLE.md#owning-value-parameters).
Alias mutation and non-rebinding rules are specified in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md).

---

## 7. Heap Allocation and Shared Ownership

Heap allocation, `shared`, `new`, reference counting, and alias sources through
shared ownership are not implemented or frozen. Their exploratory direction
and unresolved semantic constraints are summarized in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md#future-ownership-boundary).
Runtime allocation and ownership mechanisms are outside the language contract
and move to the focused runtime ABI authority when designed.

---

## 8. Classes, Inheritance, and Polymorphism

Single inheritance, base subobjects, inherited lookup, lifecycle composition,
and dispatch are exploratory and authoritative in
[polymorphism](language/POLYMORPHISM.md). The active roadmap must freeze every
executable rule before implementation.

### 8.1 Inline Values and Slicing

The distinction between copied exact-base values and non-owning, non-slicing
views is described in
[values, slicing, and non-owning views](language/POLYMORPHISM.md#values-slicing-and-non-owning-views).

### 8.2 Shared Upcasts

Shared ownership is outside the planned polymorphism profile. Shared upcasts
remain future ownership design rather than a polymorphism contract.

### 8.3 Alias-Parameter Upcasts

Exploratory non-owning class upcasts must preserve source access and lifetime
without slicing. Their exact conversion rules remain a profile-freeze choice.

### 8.4 Virtual Dispatch

Non-virtual-by-default methods, opt-in virtual families, explicit overrides,
and dynamic calls through non-owning views are described in
[direct and virtual methods](language/POLYMORPHISM.md#direct-and-virtual-methods).
Syntax and compatibility remain unfrozen.

---

## 9. Interfaces

Nominal requirements, explicit conformance, non-owning interface views, and
their open profile decisions are described in
[interfaces](language/POLYMORPHISM.md#interfaces). Standalone interface values,
shared handles, and interface inheritance are outside the planned profile.

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

The implemented language has no casts, equality, type tests, or narrowing.
Core conversion maturity is authoritative in
[types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).
The exploratory distinction among slicing, non-owning upcasts, type tests, and
checked scoped narrowing is owned by
[polymorphism](language/POLYMORPHISM.md#type-tests-and-checked-narrowing); every
source form, conversion boundary, and failure rule remains unfrozen.

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

The version marker and executable link-compatibility mechanism are
implementation contracts owned by the
[runtime documentation](REPO_STRUCTURE.md#runtime) until DOC13 creates its
focused replacement. They are not language semantics.

The current runtime has no allocation, shared-ownership, reference-counting,
borrow-anchor, dynamic object-metadata, or garbage-collection responsibility.
Those mechanisms are future runtime design and do not become part of the ABI
until their source semantics are frozen and the focused runtime ABI authority
specifies them.

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
value, and does not make object types externally linkable. Compiler-generated
symbol spelling belongs to the
[backend documentation](REPO_STRUCTURE.md#x86-64-system-v-backend), not to the
language contract.

### 13.5 Stage-0 Alias-Parameter ABI

Implemented source behavior is authoritative in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md). The current pointer
parameter, argument-classification, frame, and projection realization belongs
to the [x86-64 System V backend](REPO_STRUCTURE.md#x86-64-system-v-backend)
until DOC12 replaces that architecture section with the focused backend
authority. It is an internal convention, not a source-visible reference type
or external object ABI.

---

## 14. Relationship to Niflheim

Skald originated in an exploratory draft called Niflheim2, which used the earlier Niflheim language and compiler as a design starting point. The memory model and several related semantics diverged enough that the project became a distinct language with a new name, compiler, source suffix, and repository. Niflheim remains historical context rather than a compatibility target or normative dependency of this specification.

Skald intentionally retains several ideas explored in Niflheim:

- statically typed compiled language;
- the primitive types `i64`, `u64`, `u8`, `bool`, `f64`, and `unit`;
- fixed-size arrays;
- a possible future module system, without inheriting Niflheim's source forms;
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
  call-scoped parameter aliases are implemented as described in
  [aliases and ownership](language/ALIASES_AND_OWNERSHIP.md).

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
  or implicit conversions. Future cross-module identity, lookup, visibility,
  and ambiguity choices are owned by
  [modules and foreign interoperation](language/MODULES_AND_INTEROP.md); later
  conversion ranking remains a separate open type-system question.
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
- **Polymorphism:** the intended inheritance, view, dispatch, interface,
  type-test, and narrowing constraints—and every choice still required before
  implementation—are collected in
  [polymorphism](language/POLYMORPHISM.md). This legacy draft does not freeze
  their syntax or failure behavior.
- **Modules and foreign interoperation:** current behavior and the unresolved
  multiple-file, visibility, build, coalescing, and broader FFI choices are
  owned by [modules and foreign interoperation](language/MODULES_AND_INTEROP.md).
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
- the exploratory polymorphism direction and its unresolved profile choices
  are owned by [polymorphism](language/POLYMORPHISM.md), not by this resolved-
  decision list;
- default array construction is valid only for element types with default values;
- array physical storage placement is an implementation detail;
- `Str` is an immutable small inline value backed by immutable byte storage;
- string literals lower to `Str` values backed by compiler-emitted static immutable bytes.
- the implemented single-file, entry-point, namespace, and primitive external-
  function contracts follow
  [modules and foreign interoperation](language/MODULES_AND_INTEROP.md);
- `ska_rt_println_i64` writes the shortest ASCII signed decimal representation and one LF, and a detected incomplete output is unrecoverable;
- the current runtime ABI implements `ska_rt_println_bool`, which writes
  lowercase ASCII `true` or `false` and one LF, uses the same unrecoverable
  detected-output-failure policy, and remains an ordinary external function;
- runtime ABI version 4 implements `u64` and `u8` decimal output plus exact raw-bit `f64` observation, all as ordinary external functions;
- exact-class initialization, copy capabilities, assignment, object parameters
  and results, temporaries, permitted elision, and deterministic destruction
  follow the implemented
  [class lifecycle contract](language/CLASSES_AND_LIFECYCLE.md);
- implemented exact-class alias parameters follow the focused
  [aliases and ownership contract](language/ALIASES_AND_OWNERSHIP.md); shared,
  local, anchored, and polymorphic alias extensions remain non-implemented
  design areas at the maturity recorded in the status matrix.

The remaining open questions do not invalidate the core memory-model direction, but some must be resolved before their associated features become normative.

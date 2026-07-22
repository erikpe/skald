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

Recoverable exceptions remain exploratory because they must extend
deterministic cleanup. Their retained constraints and open decisions are owned
by [errors and exceptional control flow](language/ERRORS.md).

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
- The implemented type set has no null value. Future optional syntax and
  semantics remain open in
  [types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).

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

Optional values are not implemented or frozen. The retained direction and open
syntax, presence, conversion, payload-lifetime, and lifecycle questions are
owned by [types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).
Legacy `T?` and `none` examples are not reserved syntax.

### 4.7 Array Types

Arrays, indexing, slicing, and element borrowing are not implemented or
frozen. Their coupled size, construction, lifetime, mutation, access, bounds-
failure, and iteration questions are owned by
[types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).
No bracket syntax, defaulting rule, or storage model from this legacy draft is
a language contract.

### 4.8 Str

Strings and string literals are not implemented or frozen. The retained
immutable-value direction and open type, literal, encoding, byte-semantics,
ownership, storage, and library questions are owned by
[types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families).
`Str`, its conceptual layout, and its literal-lowering strategies in this
legacy draft are not language contracts.

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
and would require an explicit extension of the current
[runtime ABI](compiler/RUNTIME_ABI.md#responsibility-boundary) when designed.

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

Indexing, slicing, iteration, and loops are not implemented or frozen. Array
access questions are owned by
[types and values](language/TYPES_AND_VALUES.md#conversions-and-future-value-families),
while loop scope, evaluation, cleanup, and iterator questions are owned by
[functions and control flow](language/FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability).
No bracket, `for ... in`, or structural-protocol example from this legacy
draft is accepted or reserved syntax.

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

Current compile-time rejection, the one verified bootstrap runtime-failure
boundary, and future exceptional-cleanup constraints are authoritative in
[errors and exceptional control flow](language/ERRORS.md). Cast, array,
allocation, and explicit-panic failures named by this legacy draft do not
describe implemented operations.

### 12.1 Checked Exceptions

Recoverable exceptions are exploratory and have no reserved syntax, type rules,
ownership model, failure behavior, or lowering contract. The only retained
constraint is that any future exceptional control flow must extend the
existing deterministic lifetime model, as described in
[recoverable and checked exceptions](language/ERRORS.md#recoverable-and-checked-exceptions).

---

## 13. Runtime Model

The current public C surface, version/link guard, platform requirements,
bootstrap output records, detected-failure behavior, and responsibility
boundary are authoritative in the [runtime ABI](compiler/RUNTIME_ABI.md).
They are implementation contracts rather than language semantics.

### 13.1 Bootstrap `i64` Output

The version-4 signed-integer output contract has moved to
[runtime output records](compiler/RUNTIME_ABI.md#output-records). Skald source
uses it through the ordinary external declaration mechanism described in
[modules and foreign interoperation](language/MODULES_AND_INTEROP.md).

### 13.2 Bootstrap `bool` Output

The version-4 boolean output contract has moved to
[runtime output records](compiler/RUNTIME_ABI.md#output-records). It remains an
ordinary restricted external function rather than a compiler intrinsic.

### 13.3 Bootstrap Remaining-Primitive Output

The version-4 unsigned-integer and exact-binary64 output contracts have moved
to [runtime output records](compiler/RUNTIME_ABI.md#output-records). Their
source declarations use the same ordinary foreign-interoperation boundary.

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
[backend and target contract](compiler/BACKEND.md), not to the
language contract.

### 13.5 Stage-0 Alias-Parameter ABI

Implemented source behavior is authoritative in
[aliases and ownership](language/ALIASES_AND_OWNERSHIP.md). The current pointer
parameter, argument-classification, frame, and projection realization belongs
to the [backend and target contract](compiler/BACKEND.md). It is an internal
convention, not a source-visible reference type or external object ABI.

---

## 14. Relationship to Niflheim

Skald originated in an exploratory draft called Niflheim2, which used the earlier Niflheim language and compiler as a design starting point. The memory model and several related semantics diverged enough that the project became a distinct language with a new name, compiler, source suffix, and repository. Niflheim remains historical context rather than a compatibility target or normative dependency of this specification.

Skald retains lessons from Niflheim but does not inherit its feature contracts.
Implemented behavior is owned by the focused Skald language documents;
unimplemented maturity is owned by the
[status matrix](language/STATUS.md#not-implemented). Niflheim syntax,
containers, strings, statics, callability, runtime failures, and exception
behavior must not be treated as Skald defaults.

Niflheim code should not be expected to compile as Skald code without substantial changes. The Niflheim repository may be consulted for historical implementation context, but Skald behavior is defined by Skald's own specification and documentation.

---

## 15. Specification Status and Open Design Questions

Current support and maturity are authoritative in the
[status matrix](language/STATUS.md). Implemented rules and actionable open
questions now live with their focused language owners. This legacy document no
longer maintains a second deferred-feature inventory, open-question list, or
chronological resolved-decisions appendix.

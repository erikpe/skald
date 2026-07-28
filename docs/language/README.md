# Skald Language Overview

Status: broad language authority. [Feature maturity and compiler
support](STATUS.md) are authoritative in the status matrix; focused semantic
documents define detailed rules as they are established.

Skald is an exploratory, statically typed, ahead-of-time compiled language for
small programs, personal projects, and compiler experimentation. Its design
combines an object-oriented source model with inline values, deterministic
resource management, explicit mutation, and implementation boundaries that
remain understandable.

The language is intentionally small. Design direction is not executable
compiler support: only features marked **implemented contract** in the status
matrix reach complete compilation. A frozen feature may have accepted
source/resolved forms that stop at an explicit later-phase gate.

## Core model

Every expression has an exact static type. The current value types are
primitive values and nominal class values; `unit` represents the absence of a
result payload. Skald does not currently apply implicit numeric conversions,
truthiness, or structural class compatibility.

Class values are inline by default. A class value contains its complete fields
rather than an implicit nullable object reference. Class-typed fields are
subobjects of their containing value, and each complete value has one
deterministic lifetime.

Execution is organized around top-level functions and class members with
explicit direct, static, virtual, or interface selection. Static methods are
class-owned and receiverless; instance receivers are evaluated before their
explicit arguments. Blocks introduce lexical scopes, and operands and
arguments have deterministic source order.

## Terms

| Term | Meaning |
|---|---|
| **value** | A typed result or stored entity. Primitive values carry scalar payloads; class values carry one complete nominal object. |
| **object** | A complete class value or one of its inline class subobjects. “Object” does not imply heap allocation. |
| **place** | An addressable storage location, such as a local, parameter, `self`, or a field path. |
| **binding** | A source name associated with a value place or a non-owning alias place. |
| **owner** | A place responsible for the lifetime and eventual destruction of its class value. |
| **shared owner** | A non-null owning `shared T` handle. It is a value distinct from the allocated object place it keeps alive. |
| **optional value** | An explicit `T?` or `shared? T` wrapper containing either no payload or one complete valid payload. Primitive, exact-class, and shared-owner optionals execute across internal owning boundaries; class unwrap supplies a bounded checked payload view and shared unwrap secures an ordinary non-null owner. |
| **shared dereference** | The bounded non-owning pointee place selected by `*owner`; `owner->member` selects one member through exactly one shared edge. |
| **array** | A built-in invariant fixed-size sequence. Inline `T[]` values deep-copy named sources and adopt produced backing; `shared T[]` owners share one allocation. Arrays support nested owning element categories, immutable length, checked indexing, copied slices, explicit shared projection, deterministic lifecycle, and call-scoped aliases on x86-64. |
| **alias** | A call-scoped, non-owning view of an existing class place. Read-only and mutable access are explicit; the static target may be a class, an ancestor, an interface, or `Obj`. |
| **exact class** | One nominal class identity as an owning value. Derived-to-base owning conversion slices into a new exact base value. |
| **lifecycle member** | A contextual `init`, `copy`, `assign`, or `destroy` class member occupying a dedicated semantic slot or overload set rather than the ordinary method namespace. Ordinary `init` declarations form an overload set; `copy`, `assign`, and `destroy` retain their distinct slots. |

## Values, places, and mutation

`var` declarations create owning local storage. Primitive values are copied as
primitive payloads. Class initialization, copy construction, assignment, and
destruction use the selected lifecycle operation for the exact class.
The implemented constructor model gives ordinary `init` declarations a
constructor-only overload set and gives copy construction the distinct
`copy(ref source: T)` declaration and `T(copy source)` selection form.

Value parameters own their incoming value. Current class value parameters are
copy-constructed by the caller and cleaned by the callee. `ref` and `mut ref`
parameters instead borrow an existing object place or supported polymorphic
view for one call; they do not copy or own the object. Ordinary methods have
read-only receivers, while `mut fn` methods may mutate through their receiver.

Assignment updates an already live value without beginning a new lifetime.
Construction begins a lifetime, and destruction ends it. Class fields are
constructed as part of their containing object and are destroyed in the
language-defined order.

## Lifetimes and safety direction

Skald is designed around deterministic lifetimes rather than tracing garbage
collection. Current owning values are inline and are cleaned on implemented
normal block and return exits. Copying and cleanup are explicit semantic
operations, even when a class receives synthesized field-wise behavior.

The broader direction keeps nullability and ownership visible in source types,
preserves or reduces access through conversions, and prevents non-owning views
from escaping their valid source lifetime. The implemented shared-ownership
profile
uses non-null `shared T` handles, deterministic last-owner destruction, and
hidden owning anchors for borrows from replaceable shared storage. Shared
allocation is explicit through `new T(arguments)` or `new T(copy source)`.
Pointee access is explicit: `.` stays within an inline object place, `->`
crosses one shared edge, and general object-place consumers require `*owner`.
Primitive, exact-class inline, and shared-owner optional values execute across
owning local, field, and internal callable boundaries, including dynamically
guarded checked class payload views and secured ordinary owners from
`shared? T` unwrap. The implemented source contract is described
in [Optional Values](OPTIONAL_VALUES.md): `T?` and `shared? T` make
absence visible without weakening ordinary types, `none` constructs absence,
`is some` and `is none` inspect presence, and postfix `!` performs checked
access. Exceptional control flow remains unimplemented and exploratory.

External function declarations are trusted ABI assertions. They form a focused
interoperation boundary rather than a proof that foreign code satisfies Skald
ownership or safety rules.

## Programs and implementation boundary

A program is the selected entry module plus the complete reachable import
graph of UTF-8 `.ska` source modules. Each module has one non-overloaded
top-level namespace, and execution starts at the selected module's defined
`fn main() -> i64`. Restricted exact-symbol external declarations connect
primitive values to the platform ABI. The implemented whole-program module
system provides path-derived identities, explicit imports, and module-level
visibility. Packages and separate compilation remain deferred.

Target layout, registers, calling conventions, compiler IR, generated symbols,
runtime allocation, and tool invocation are implementation concerns. They do
not define language meaning unless a focused language document explicitly
makes a result source-observable.

## Documentation

- [Feature status](STATUS.md) is the sole matrix for language maturity and
  current compiler support.
- The [implemented grammar](GRAMMAR.md) is the exact accepted syntax authority.
- [Types, values, and expressions](TYPES_AND_VALUES.md) defines the implemented
  type model, literals, exact-type rules, and operator availability.
- [Strings](STRINGS.md) freezes the unimplemented raw-byte
  `std::str::Str` descriptor, literal syntax, logical immutability, and
  compiler/standard-library boundary.
- [Arrays](ARRAYS.md) freezes the syntax-parsed inline/shared array type,
  construction, copying, adoption, indexing, slicing, nesting, alias, lifetime,
  and failure contract.
- [Optional values](OPTIONAL_VALUES.md) defines the explicit `T?` and
  `shared? T` source contract, including presence, checked
  access, lifecycle, failure, and the remaining aliasing exclusions.
- [Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) defines callable
  declarations, bindings and scopes, statements, returns, and evaluation order.
- [Classes and lifecycle](CLASSES_AND_LIFECYCLE.md) defines exact nominal
  classes, inline containment, receivers, ordinary initializer overloads,
  explicit copy construction, and object places, plus assignment, temporaries,
  and deterministic lifetime.
- [Aliases and ownership](ALIASES_AND_OWNERSHIP.md) defines implemented
  call-scoped aliases, non-exclusive access, and current inline lifetime.
- [Shared ownership and heap allocation](SHARED_OWNERSHIP.md) defines the
  implemented non-null shared value, ordinary and exact-class copy allocation,
  owner copy/release, dynamic destruction, cycle, and borrow-anchor semantics
  as the current x86-64 profile. Dynamic-type-preserving cloning remains
  deferred.
- [Polymorphism](POLYMORPHISM.md) defines implemented inheritance,
  class/interface/`Obj` views, slicing, virtual/interface dispatch, type tests,
  and checked object casts.
- [Object casts](OBJECT_CASTS.md) defines the implemented C-style plain
  checked-place profile, including owning inline copy consumers, and freezes
  the remaining shared-owner syntax and complete ownership-direction matrix.
- [Modules and foreign interoperation](MODULES_AND_INTEROP.md) defines the
  implemented module namespaces, imports, visibility, entry selection, and
  trusted primitive external-function boundary.
- [Errors and exceptional control flow](ERRORS.md) defines compile-time
  rejection, the current fatal runtime boundary, normal-flow cleanup limits,
  and the open checked-exception design.
- [Active roadmaps](../roadmaps/README.md) own implementation ordering and open
  profile decisions; archived roadmaps are history only.

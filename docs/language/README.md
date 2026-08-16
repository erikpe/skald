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
| **optional value** | An explicit `T?` or `(shared T)?` wrapper containing either no payload or one complete valid payload. Primitive, exact-class, and shared-owner optionals execute across internal owning boundaries; class unwrap supplies a bounded checked payload view and shared unwrap secures an ordinary non-null owner. `shared? T` is shorthand for the latter. |
| **shared dereference** | The bounded non-owning pointee place selected by `*owner`; `owner->member` selects one member through exactly one shared edge. |
| **array** | A built-in invariant fixed-size sequence. Inline `T[]` values deep-copy named sources and adopt produced backing; `shared T[]` owners share one allocation. Arrays support nested owning element categories, immutable length, checked indexing, copied slices, explicit shared projection, deterministic lifecycle, and call-scoped aliases on x86-64. |
| **string** | An exact `std::str::Str` class value describing an immutable finite sequence of `u8` bytes. Literals use immortal backing; ordinary standard-library construction and concatenation use dynamically reclaimed shared backing. Read-only brackets select checked byte access or constant-time descriptor slicing. |
| **alias** | A call-scoped, non-owning view of a live object place. A read-only object alias may select an existing place or materialize a compatible produced exact-class object in hidden caller-owned storage. The static target may be a class, an ancestor, an interface, or `Obj`; mutable aliases still require existing mutable places. |
| **exact class** | One nominal class identity as an owning value. Derived-to-base owning conversion slices into a new exact base value. |
| **generic class** | A compile-time class template with named type parameters. Each accepted explicit closed application denotes a distinct ordinary exact class after semantic specialization; no unresolved parameter or erased generic value exists at runtime. |
| **function value** | A non-null, capture-free reference to one exact internal top-level function or static method. Its canonical recursive function type fixes parameter modes and types plus the result; copying carries no receiver, environment, owner, or cleanup. |
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
parameters instead borrow an eligible object source or supported polymorphic
view for one call; they do not copy or own the object. Ordinary methods have
read-only receivers, while `mut fn` methods may mutate through their receiver.
A compatible exact-class producer initializes hidden caller-owned storage for
a read-only `ref`; that owner remains live through the complete call and is
then cleaned with the enclosing full expression. This behavior executes
through verified MIR and the ordinary native object-alias ABI. The rule does
not relax `mut ref` or transfer that storage to the callee.

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
`(shared T)?` unwrap. The implemented source contract is described
in [Optional Values](OPTIONAL_VALUES.md): `T?` and `(shared T)?` make
absence visible without weakening ordinary types, `none` constructs absence,
`is some` and `is none` inspect presence, and postfix `!` performs checked
access. `shared? T` is an exact source shorthand. Recursive identities, owning
lifecycle, expected-type-directed `some(...)` construction, one-layer checked
access, aliases, and internal callable boundaries now execute for nested
optionals. Optional arrays and arrays of shared optional-box owners execute
through their ordinary owning lifecycle. Shared optional boxes are implemented
for internal Skald positions; external optional signatures remain outside the
current C ABI.
Exceptional control flow remains unimplemented and exploratory.
The separate uncatchable panic and common unrecoverable-failure reporting
contract is implemented.

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
  type model, literals, exact-type rules, operator availability, and the
  complete implemented primitive operator profile and complete explicit
  primitive cast matrix. Integer division and remainder, floating
  division, bitwise operations, checked shifts, integer and floating
  comparisons, all twenty-five primitive casts, eager boolean operators, and
  short-circuit boolean expressions execute through verified MIR and the
  x86-64 backend. The twenty-two non-failing primitive cast cells use pure
  MIR; the three checked `f64`-to-integer cells use explicit verified control
  flow. Every cell is accepted from source and executes inline on x86-64.
- [Strings](STRINGS.md) defines the implemented raw-byte `std::str::Str`
  descriptor, literal syntax, logical immutability, ordinary standard-library
  operations including canonical boolean and integer formatting, exact
  optional boolean and integer parsing, and correctly rounded binary64
  parsing, the compiler/library boundary, and the remaining frozen binary64
  formatting contract.
- [Standard I/O](IO.md) defines the implemented nine-function `std::io` surface,
  raw-byte whole-input and exact-output behavior, stable failures, costs, and
  deliberate exclusions. Its private compiler/runtime foundation, x86-64
  lowering, exact standard-stream writes, and growable whole-input reads all
  execute through ordinary Skald library code over the private byte boundary.
- [Process arguments](PROCESS.md) defines the implemented, explicitly imported
  `std::process::args() -> Str[]` contract: a fresh raw-byte invocation-vector
  snapshot, including the host invocation name at index zero, decoded from
  Linux `/proc/self/cmdline` by ordinary standard-library code. It does not
  change the parameterless entry function or runtime ABI.
- [Vectors](VECTORS.md) defines the implemented generic `std::vec::Vec<T>`
  contract, including admitted element capabilities,
  capacity, growth, structural indexing and slicing, snapshot replacement,
  structural copy independence, and prompt removal cleanup.
- [Structural indexing and slicing](INDEXING_AND_SLICING.md) defines the
  implemented class/interface bracket protocol, exact method shapes, array
  precedence, receiver and evaluation rules, and `Vec<T>`/`Str` adoption
  boundary.
  Class and interface index and slice reads and assignments are implemented
  through verified ordinary direct, virtual, and witness call ownership.
  `Str` supplies immutable reads and `Vec<T>` supplies all four protocols.
- [Arrays](ARRAYS.md) defines the implemented syntax-parsed inline/shared array
  type, construction, copying, adoption, indexing, slicing, nesting, alias,
  lifetime, failure, and typed explicit element-list contract. Primitive,
  exact-class, inline-optional, recursively nested
  inline-array, shared-owner, and optional shared-owner families execute
  through one verified initialized-prefix protocol.
- [Static fields](STATIC_FIELDS.md) defines class-owned zero-default and
  explicit initialization, dependency-ordered eager startup, replacement,
  exact-reverse normal-return shutdown, diagnostics, and the unchanged
  runtime-ABI contract.
- [Optional values](OPTIONAL_VALUES.md) defines the explicit `T?` and
  canonical `(shared T)?` and shorthand `shared? T` source contract, including presence, checked
  access, lifecycle, failure, and the remaining aliasing exclusions.
- [Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) defines callable
  declarations, bindings and scopes, statements, returns, evaluation order,
  implemented `while` loops and targeted `break` and `continue` exits.
- [Capture-free function values](FUNCTION_VALUES.md) defines implemented recursive exact
  function types, eligible internal references, trivial non-null storage,
  indirect-call evaluation and ownership, closed generic composition, and the
  initial exclusions. Ordinary references and receiverless indirect calls are
  realized from exact target/signature metadata through trivial
  storage/transport, verified MIR, one-word x86-64 pointers, and the complete
  ordinary argument/result ABI, including deterministic static effects,
  retention, panic traces, and source-to-native conformance.
- [Classes and lifecycle](CLASSES_AND_LIFECYCLE.md) defines exact nominal
  classes, inline containment, receivers, ordinary initializer overloads,
  per-overload private factory boundaries, explicit copy construction, and
  object places, plus assignment, temporaries, and deterministic lifetime. It
  also owns the staged `private cell` whole-field replacement contract. The
  compiler accepts declarations and typed whole-field authorization; the
  status matrix and active roadmap distinguish that support from deferred MIR
  verification and execution.
- [Generic classes](GENERIC_CLASSES.md) defines implemented explicit closed generic class
  applications, structural substitution, inferred contextual requirements,
  nominal interface bounds, invariance, complete-class validation, and native
  execution.
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
  rejection, the current fatal runtime boundary, the frozen uncatchable panic
  design and sole static-message catalog, normal-flow cleanup limits, and the
  open checked-exception design.
- [Active roadmaps](../roadmaps/README.md) own implementation ordering and open
  profile decisions; archived roadmaps are history only.

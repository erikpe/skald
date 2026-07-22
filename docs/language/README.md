# Skald Language Overview

Status: broad language authority. [Feature maturity and compiler
support](STATUS.md) are authoritative in the status matrix; focused semantic
documents define detailed rules as they are established.

Skald is an exploratory, statically typed, ahead-of-time compiled language for
small programs, personal projects, and compiler experimentation. Its design
combines an object-oriented source model with inline values, deterministic
resource management, explicit mutation, and implementation boundaries that
remain understandable.

The language is intentionally small. Design direction is not compiler support:
only features marked **implemented contract** in the status matrix are accepted
by the current compiler.

## Core model

Every expression has an exact static type. The current value types are
primitive values and nominal class values; `unit` represents the absence of a
result payload. Skald does not currently apply implicit numeric conversions,
truthiness, or structural class compatibility.

Class values are inline by default. A class value contains its complete fields
rather than an implicit nullable object reference. Class-typed fields are
subobjects of their containing value, and each complete value has one
deterministic lifetime.

Execution is organized around top-level functions and statically selected
class members. Blocks introduce lexical scopes. Receivers, operands, and
arguments have deterministic source order; a receiver is evaluated before its
explicit arguments.

## Terms

| Term | Meaning |
|---|---|
| **value** | A typed result or stored entity. Primitive values carry scalar payloads; class values carry one complete nominal object. |
| **object** | A complete class value or one of its inline class subobjects. “Object” does not imply heap allocation. |
| **place** | An addressable storage location, such as a local, parameter, `self`, or a field path. |
| **binding** | A source name associated with a value place or a non-owning alias place. |
| **owner** | A place responsible for the lifetime and eventual destruction of its class value. |
| **alias** | A call-scoped, non-owning view of an existing exact-class place. Read-only and mutable access are explicit. |
| **exact class** | One nominal class identity without an inheritance conversion. The current compiler implements only exact-class behavior. |
| **lifecycle member** | A contextual `init`, `assign`, or `destroy` class member occupying a dedicated semantic slot rather than the ordinary method namespace. |

## Values, places, and mutation

`var` declarations create owning local storage. Primitive values are copied as
primitive payloads. Class initialization, copy construction, assignment, and
destruction use the selected lifecycle operation for the exact class.

Value parameters own their incoming value. Current class value parameters are
copy-constructed by the caller and cleaned by the callee. `ref` and `mut ref`
parameters instead borrow an existing exact-class place for one call; they do
not copy or own the object. Ordinary methods have read-only receivers, while
`mut fn` methods may mutate through their receiver.

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
from escaping their valid source lifetime. Shared allocation, polymorphic
views, optionals, and exceptional control flow are not current compiler
features, so their final safety rules remain subject to focused design.

External function declarations are trusted ABI assertions. They form a narrow
interoperation boundary rather than a proof that foreign code satisfies Skald
ownership or safety rules.

## Programs and implementation boundary

The current compilation unit is one UTF-8 `.ska` source file. Top-level
functions and classes share one non-overloaded namespace, and execution starts
at a defined `fn main() -> i64`. Restricted exact-symbol external declarations
connect primitive values to the platform ABI. Modules, imports, packages, and
separate compilation are not yet language features.

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
- The [draft specification](../SKALD_DRAFT_SPEC.md) remains migration input for
  detailed semantic areas that do not yet have focused documents. Its future
  sketches are not evidence of implementation or frozen design.
- [Active roadmaps](../roadmaps/README.md) own implementation ordering and open
  profile decisions; archived roadmaps are history only.

Focused documents for functions and control flow, classes and lifecycle,
aliases and ownership, polymorphism, modules and interoperation, and errors will
be linked here as their verified authorities are established.

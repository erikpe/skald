# Structural Indexing and Slicing

Status: **frozen design; class and interface indexing and slicing
implemented**.

This document is authoritative for the source-visible meaning of bracket
indexing and slicing on classes and interfaces. The current compiler
implements all four operations for class and interface receivers. Built-in
arrays retain their complete intrinsic behavior. Availability remains
authoritative in the [status matrix](STATUS.md).

The design adds no syntax. It gives the existing postfix bracket forms a
structural meaning when the receiver's static type is a class or interface:

```ska
value[key]
value[key] = replacement
value[start:end]
value[:end]
value[start:]
value[:]
value[start:end] = replacement
```

All forms are implemented for eligible class and interface receivers.

## Selection and precedence

Resolution selects exactly one of four case-sensitive ordinary instance
method names:

| Source use | Required declaration shape |
|---|---|
| `receiver[key]` | `fn index_get(key: K) -> R` |
| `receiver[key] = replacement` | `mut fn index_set(key: K, replacement: W) -> unit` |
| `receiver[start:end]` | `fn slice_get(start: i64?, end: i64?) -> R` |
| `receiver[start:end] = replacement` | `mut fn slice_set(start: i64?, end: i64?, replacement: W) -> unit` |

`K`, `R`, and `W` describe the selected declaration's ordinary resolved
types; they are not new generic-method syntax. Index key types are not fixed
to `i64`. Getter result and setter replacement types are independent, and a
type may provide only the reads or writes it supports.

Built-in arrays always take precedence. Exact inline arrays and shared arrays
reached through explicit dereference keep their existing intrinsic indexing
and slicing semantics. Arrays do not gain method-form `index_get`,
`index_set`, `slice_get`, or `slice_set` aliases.

A class participates when ordinary hierarchy lookup selects an accessible,
compatible instance method. An interface participates only when its static
interface declares the corresponding compatible requirement. These four
lookups are structural conveniences, not general structural typing, a marker
interface, or an operator-declaration system. Static methods, fields, and
static fields never satisfy the protocol.

Class indexing and slicing use inherited lookup, declaring-class privacy,
closed generic specialization, ordinary argument checking, and explicit
shared crossing with `owner->[...]`. Interface receivers select the matching
declared requirement and reuse ordinary witness dispatch, including explicit
shared-interface crossing.

## Method requirements

Getters are read-only instance methods. Setters are mutable instance methods
and must return exact `unit`. Index key and setter replacement parameters may
be ordinary value parameters or read-only `ref` parameters. `mut ref`
protocol operands are invalid because bracket syntax does not grant mutable
access to its operand expressions.

Slice methods use exact value parameters `start: i64?` and `end: i64?`.
Every supplied `i64` bound receives the existing one-layer optional injection;
an omitted bound receives `none`. Omission requests the collection's logical
beginning or end. The compiler neither calls `len()` nor evaluates the
receiver again.

The selected method body owns collection policy: bounds checks, negative-index
interpretation, logical length, copy-versus-view behavior, resizing policy,
replacement length requirements, alias handling, and overlapping-write
semantics. Structural brackets do not import built-in array policy into a
class.

## Receivers, access, and dispatch

Structural brackets grant exactly the access of the equivalent ordinary
call. Getters accept the receiver forms already valid for read-only methods;
setters additionally require a mutable receiver place. A produced exact-class
value may receive a getter through the existing produced-receiver temporary,
but a mutable setter may not mutate an unnamed produced inline value.

Shared class and interface owners still require explicit crossing with
`owner->[...]` or `(*owner)[...]`. Optional owners must be unwrapped first.
Existing anchors keep produced or replaceable shared owners live through the
call.

Ordinary inherited lookup, declaring-class privacy, virtual roots and
overrides, interface requirements, and closed generic specialization all
apply unchanged. A concrete method that is absent from an interface's static
requirements is not visible through an interface-typed receiver.

## Evaluation, ownership, and failure

Every present expression evaluates exactly once in this order:

1. receiver;
2. key, or supplied start followed by supplied end;
3. assignment replacement, when present; and
4. the selected call.

Omitted bounds perform no source evaluation. Ordinary argument modes, result
types, full-expression temporaries, copies, owner transfers, anchors, cleanup,
and direct, virtual, or interface dispatch govern the normalized call. The
protocol defines no borrowed result category and no compiler-known bounds or
slice-length failure. Method bodies may fail through ordinary language
mechanisms; array failures remain intrinsic and separately specified.

The implemented compiler verifies this ordinary-call behavior for primitive,
class, array, optional, shared-owner, and closed-generic results and
replacements. Named and produced sources, self-aliasing replacements, checked
receiver anchors, consuming calls, and reverse temporary cleanup retain their
existing ownership behavior; bracket spelling introduces no additional
lifetime category.

## Standard-library adoption

The initial library profile keeps `Vec<T>` and `Str` as ordinary Skald classes
rather than compiler language items for this feature.

`Vec<T>` will provide all four methods. Its slice bounds use logical length,
not capacity. `slice_get` returns an independent vector. `slice_set` preserves
destination length, requires equal logical lengths, assigns in increasing
order, and secures or copies the complete replacement before the first write
so self-aliasing and overlap have snapshot behavior. Existing `get` and `set`
may remain as wrappers while one implementation owns normalization and bounds
behavior.

`Str` provides only `index_get` and `slice_get`, sharing its checked byte lookup
and constant-time descriptor slicing. Omitted slice bounds map to the logical
beginning and end. It provides no setter and remains immutable. Literal,
named, produced, copied, explicitly dereferenced shared, and interface-selected
call paths use the same ordinary protocol machinery. `Vec<T>` adoption remains
pending.

## Exclusions

The initial design does not include iteration, structural length or capacity,
multi-index syntax, range values, inclusive ranges, strides, compound or
value-producing assignment, user-declared operators, automatic forwarding,
implicit shared or optional dereference, built-in-array method aliases,
external ABI mappings, recoverable bounds failures, or exceptions.

Compiler representation and normalization are specified by the
[structural indexing and slicing compiler contract](../compiler/INDEXING_AND_SLICING.md).
The confirmed decisions are preserved in the
[archived design record](../archive/STRUCTURAL_INDEXING_AND_SLICING_DESIGN_PROPOSAL.md).

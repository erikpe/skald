# Skald Polymorphism

Status: exploratory language-design authority. No inheritance, polymorphic
conversion, dynamic dispatch, interface, `Obj`, type-test, or narrowing rule is
implemented or frozen. The [status matrix](STATUS.md) is authoritative for
feature maturity, and the active
[polymorphism roadmap](../roadmaps/POLYMORPHISM_ROADMAP.md) owns the work needed
to freeze and implement an executable profile.

This document records the direction that later profile design must preserve
and the decisions it must still make. Examples and candidate spellings in
older documents are not accepted grammar or normative behavior.

The implemented baseline remains exact-class semantics:

- [classes and lifecycle](CLASSES_AND_LIFECYCLE.md) defines complete inline
  objects, initialization, copying, assignment, and destruction;
- [aliases and ownership](ALIASES_AND_OWNERSHIP.md) defines exact-class
  non-owning parameter aliases and access propagation; and
- [functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md) defines calls,
  evaluation order, results, and normal cleanup.

Polymorphism must extend those contracts rather than introduce a parallel
object, call, or lifetime model.

## Design direction

The intended profile combines:

- single class inheritance with at most one direct base and acyclic base
  chains;
- one inline base subobject inside each derived object;
- inherited fields and methods with deterministic lookup;
- base participation in initialization, copying, assignment, and destruction;
- non-virtual methods by default, opt-in virtual roots, and explicit
  overrides;
- nominal interfaces with explicit class conformance;
- non-owning class, interface, and `Obj` views for polymorphic calls;
- inline derived-to-base slicing as a distinct value conversion;
- non-slicing upcasts for non-owning views;
- runtime type or conformance tests; and
- explicit checked narrowing that produces a bounded non-owning view.

These points are exploratory constraints, not an executable specification.
The roadmap's profile-freeze task may refine them together, but later
implementation tasks must not fill in missing behavior independently.

## Classes and base subobjects

The intended hierarchy permits one direct class base; longer inheritance
chains arise through successive direct bases. Multiple class inheritance and
interface inheritance are outside the planned profile.

A derived inline value is intended to contain one complete base subobject as
part of the same complete object. The base is not a separately owned lexical
value and does not acquire an independent cleanup registration. Derived fields
remain direct subobjects alongside that base contribution.

Inherited member selection is intended to be nominal and deterministic. A
derived receiver may reach inherited fields and methods, but the exact rules
for lookup, redeclaration, hiding, collisions, declaration ownership, and
diagnostic precedence remain unfrozen.

The source spellings for declaring a base and explicitly initializing it are
also unfrozen. The roadmap currently uses `extends` and `super(...)` as
candidate contextual forms; neither belongs to the implemented grammar.

## Lifecycle composition

Every derived value is intended to retain one complete-object lifetime. Base
operations compose with the existing selected lifecycle operations rather than
being inferred from layout or copied as an untyped prefix.

The direction to freeze is:

- ordinary construction establishes the base before derived fields;
- copy construction and copy assignment consider the base capability before
  direct field capabilities;
- destruction runs the derived user body, then direct fields in reverse
  declaration order, then the complete base sequence; and
- base subobjects participate in the existing result, temporary, cleanup, and
  exactly-once lifetime rules.

The executable profile still must decide the exact base-initialization form,
missing or duplicate base initialization, synthesis behavior, interaction with
user lifecycle bodies, slicing sources, temporary and return destinations,
permitted elision, and diagnostic ordering. Failed construction, unwinding,
and partial-copy cleanup remain outside the planned normal-flow profile.

## Values, slicing, and non-owning views

An inline derived-to-base value conversion is intended to slice. It creates a
distinct exact base object by the selected base copy operation. The result no
longer denotes the derived complete object, does not retain a dynamic link to
it, and follows exact-base lifetime and dispatch rules.

A non-owning upcast is different. Passing a derived place through a base,
interface, or `Obj` alias view is intended to keep referring to the original
complete object without copying or slicing. The view cannot outlive its source
and cannot grant more access than the source place. Mutable access may be
restricted to read-only access; read-only access cannot become mutable.

The exact conversion set, implicit-versus-explicit boundary, conversion
ranking, overlap behavior across different views, and interaction with
grouping, results, and overload-free call selection remain to be frozen.
Shared owning handles are not part of this profile; shared upcasts and dynamic
shared destruction remain future ownership design.

## Direct and virtual methods

Ordinary instance methods are intended to remain statically selected.
Dynamic dispatch applies only to an explicitly opted-in virtual family, and an
override must explicitly declare that relationship. Receiver mutability is
part of compatibility: a call requiring mutable receiver access cannot be made
through a read-only view.

A virtual call through a base or interface view is intended to select the
implementation for the original complete object's dynamic class. That dynamic
behavior must survive forwarding and calls made through `self`. A call on a
sliced exact base value instead uses the sliced base's exact behavior.

Candidate `virtual` and `override` spellings are not frozen. The profile must
also settle which declarations may be virtual, exact signature compatibility,
root and override lookup, redeclaration errors, dispatch behavior during
lifecycle bodies, and deterministic diagnostic precedence.

## Interfaces

Interfaces are intended to be nominal collections of method requirements.
Classes explicitly declare conformance, and conformance is checked against the
class and its inherited implementations. Requirement and implementation access
must agree; a read-only view cannot invoke a mutable requirement.

The planned profile uses interfaces through non-owning views. Standalone inline
interface values, interface fields, default method bodies, interface
inheritance, and external interface signatures are excluded. Shared interface
handles depend on the separate shared-ownership design.

The `interface` and `implements` forms, requirement namespace, duplicate and
collision rules, conformance timing, inherited satisfaction, exact signature
compatibility, and conversion rules remain unfrozen.

## Universal `Obj` views

`Obj` is intended as a universal non-owning view target for class and interface
objects, not as a standalone inline value. A conversion to `Obj` should retain
the original complete-object identity, access, and lifetime rather than slice
or transfer ownership.

The profile must still decide whether `Obj` is a semantic root or a physical
base, how class and interface views convert to it, which operations it exposes,
and how it participates in type tests and narrowing. `Obj` is not an
implemented type name today.

## Type tests and checked narrowing

The intended model distinguishes observation from conversion. A type or
conformance test reports whether a polymorphic view designates a compatible
dynamic class. Checked narrowing explicitly attempts to produce a more
specific non-owning view.

A successful narrowed view must remain within a statically bounded scope,
preserve the source lifetime, and preserve or reduce access. It cannot escape,
become an owning value, or grant mutable access from a read-only source.

The candidate `is` spelling, checked-narrowing syntax, eligible source and
target combinations, static-success and static-impossibility rules, result
scope, condition integration, and failure behavior are all unfrozen. No cast
or type-test syntax is accepted by the current grammar.

## Decisions required before implementation

The roadmap's profile-freeze task must update this document with one coherent
answer for each group below:

| Decision group | Required choices |
|---|---|
| Source forms | Contextual words, modifier order, base initialization, interface declarations, type tests, and scoped narrowing. |
| Hierarchy and lookup | Cycle errors, inherited lookup, redeclaration and hiding, namespaces, override roots, conformance, and diagnostic precedence. |
| Object and view model | Whether `Obj` is semantic or physical, complete-object identity, non-owning view contents, forwarding, and valid conversions. |
| Lifecycle and values | Base construction, capabilities, assignment, destruction, slicing, results, temporaries, cleanup, and elision. |
| Dispatch | Virtual eligibility, exact compatibility, calls through `self`, sliced calls, and interface requirement selection. |
| Tests and narrowing | Static and dynamic cases, success scope, access preservation, lifetime, and failure behavior. |

Until those choices are frozen together, no subsection above should be read as
permission to implement its preferred syntax or infer its missing semantics.

## Exclusions

The planned polymorphism profile excludes multiple class inheritance,
interface inheritance, access modifiers, `final`, abstract members or classes,
default interface bodies, interface fields, overloads, covariant overrides,
standalone inline `Obj` or interface storage, shared ownership, heap allocation,
external object ABIs, cross-module polymorphic linkage, arrays, optionals, closures,
generics, statics, reflection, exceptions, failed-construction unwinding,
unsafe casts, user-visible dispatch structures, and user-defined conversions.

Their exclusion keeps the profile bounded; it does not settle their eventual
design.

## Implementation boundary

Source semantics require stable identity, access, lifetime, slicing, and
dispatch behavior. They do not prescribe compiler IDs or phase nodes, object
layout, base offsets, pointer adjustment, metadata records, table layout,
slots, symbols, registers, hidden arguments, calling conventions, backend
algorithms, or runtime allocation headers.

The active roadmap may freeze a target-independent semantic view model needed
to preserve source behavior. Target realization remains compiler/backend work
and must not become part of this language authority.

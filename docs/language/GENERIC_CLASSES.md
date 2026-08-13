# Generic Classes

Status: frozen design; syntax, template resolution, contextual mechanical
requirement analysis, and closed-specialization identity discovery implemented;
generated class specialization not implemented. This document defines the
intended initial source-visible generic-class contract. The
[status matrix](STATUS.md) remains authoritative for availability, and the
[implemented grammar](GRAMMAR.md) is the exact syntax accepted by the current
compiler. Generic declarations now receive stable template and parameter
identities, structural definition-site-resolved type terms, exact nominal
interface bounds, inferred storage/lifecycle requirements with exact origins,
and canonical closed application keys with deterministic reserved class
identities, caching, recursion handling, and provenance. Closed-type capability
evaluation delegates to the ordinary optional, array, shared-owner, alias,
stored-value, and class-lifecycle rules. A valid closed application remains
explicitly gated until generated declarations, bodies, and execution are
implemented.

Generic classes allow one class declaration to be specialized with explicit
closed type arguments. The initial feature is designed for reusable owning
types such as `Vec<T>` without erasing Skald's exact inline values, optionals,
arrays, shared owners, aliases, or deterministic lifecycle.

## Source forms

A generic class declares one or more type parameters:

```ska
public class Pair<A, B> {
    first: A;
    second: B;

    init(first: A, second: B) {
        self.first = first;
        self.second = second;
    }
}
```

A use supplies every argument explicitly:

```ska
var pair: Pair<i64, Str> = Pair<i64, Str>(1, "one");
var owner: shared Pair<i64, Str> = new Pair<i64, Str>(1, "one");
```

The frozen grammar extension is conceptually:

```text
generic-parameter-list     = "<" identifier {"," identifier} ">"
generic-argument-list      = "<" storage-type {"," storage-type} ">"
generic-where-clause       = "where" generic-requirement
                            {"," generic-requirement}
generic-requirement        = identifier ":" declaration-path

class-declaration          = "class" identifier [generic-parameter-list]
                             ["extends" named-type]
                             ["implements" declaration-path
                                 {"," declaration-path}]
                             [generic-where-clause]
                             "{" {class-member} "}"

named-type                 = declaration-path [generic-argument-list]
```

`where` is contextual in the generic class header. `extends` remains reserved
for a direct base class and does not express a type-parameter constraint.
Nested closing angle brackets are parsed in type context without changing the
ordinary expression meaning of comparisons or shifts.

All arguments are mandatory and arity is exact. A generic class name without
arguments is not a type, and a non-generic declaration rejects arguments.
Expected types do not infer omitted arguments:

```ska
var values: Vec<Str> = Vec<Str>(); // valid under this design
var invalid: Vec<Str> = Vec();     // invalid: arguments are not inferred
```

A closed application may appear wherever an exact class type is otherwise
permitted. Existing type constructors then compose normally:

```ska
Vec<Str>?
Vec<Str>[]
shared Vec<Str>
Outer<Vec<Str?>[]>
```

Inside a template, an application may contain its own parameters, such as
`Pair<T, Str>` or `Base<T>`. It must become closed through substitution before
it denotes an executable class.

## Exact specialization and identity

A generic declaration is a compile-time template, not a runtime class. Every
accepted closed application denotes one ordinary exact class identity:

```text
Vec<Str>                  != Vec<Str?>
Vec<shared Item>          != Vec<shared Interface>
Pair<i64, u64>            != Pair<u64, i64>
```

Equivalent occurrences of the same template and canonical arguments denote
the same exact class. Grouping and equivalent optional shorthand do not create
new identities.

An uninstantiated template has no value, layout, lifecycle, static storage,
dispatch table, or runtime identity. Generic classes have no raw type,
wildcard application, or erased runtime representation.

Every parameter is invariant. Ordinary class/interface views and shared-owner
casts may still apply to values at existing API boundaries, but they never
convert one generic class application into another:

```text
Vec<Derived>          is not Vec<Base>
Vec<shared Derived>   is not Vec<shared Interface>
```

## Structural type substitution

Substitution replaces a parameter as one complete semantic type and preserves
the surrounding constructors literally. It never flattens optional layers or
moves optionality through a shared owner.

For this declaration:

```ska
public class Vec<T> {
    private storage: T?[];
    private length: u64;

    init() {
        self.storage = T?[]();
        self.length = 0u;
    }
}
```

the backing types are:

| Application | Substituted backing type | Meaning |
|---|---|---|
| `Vec<Str>` | `Str?[]` | Array of optional strings |
| `Vec<Str?>` | `Str??[]` | Array with a distinct outer occupancy layer |
| `Vec<shared Str>` | `(shared Str)?[]` | Array of optional shared owners |
| `Vec<shared Interface>` | `(shared Interface)?[]` | Array of optional owning interface views |

`Vec<shared Str>` does not create `shared Str?[]`. The field is
`Array<Optional<Shared<Str>>>`, not a shared box containing an optional string.
The [optional-values contract](OPTIONAL_VALUES.md) remains authoritative for
every presence layer and shared/optional composition.

Substitution applies throughout the complete class: bases, fields, static
fields, parameter and result types, lifecycle signatures, casts, type tests,
nested applications, construction heads, and type-bearing body operations.

## Contextual argument requirements

There is no global set of types permitted as generic arguments. An argument is
valid when it satisfies every requirement created by how the corresponding
parameter is used in the complete class.

The compiler infers mechanical requirements for source contexts including:

- field and static storage;
- value parameters and results;
- read-only and mutable alias targets;
- optional payloads;
- array elements;
- shared targets;
- default construction;
- copy construction;
- assignment; and
- destruction.

These requirements are retained over complete structural terms. For example,
`T`, `T?`, and `T?[]` remain distinct subjects until a closed application
substitutes and interns them. Requirement dumps include the capability, source
origin, and stable reason so a later application diagnostic can point back to
the exact declaration or operation that introduced it.

These roles remain distinct even when their currently accepted type sets
overlap. A bare interface is a supported alias target but is not a stored
owning value or inline optional payload. A shared interface owner is a stored
value.

Consequently:

```ska
class Observer<T> {
    init() {}
    fn observe(ref value: T) -> unit {}
}

class Owner<T> {
    value: shared T;
    init(value: shared T) { self.value = value; }
}
```

may admit `Observer<Interface>` and `Owner<Interface>` under the ordinary alias
and shared-target rules. The vector above rejects `Vec<Interface>` because its
field requires `Interface?`, while `Vec<shared Interface>` succeeds because
the optional payload is the complete owning `shared Interface` value.

Requirements follow selected operations rather than broad class categories.
Declaring `storage: T?[]` does not itself require `T` to have a zero-argument
initializer, copy constructor, or assignment. A requested-length `T?[]`
defaults every element to outer absence without default-constructing `T`.
Copying an occupied payload, returning it by value, or replacing a possibly
present slot adds the exact copy or assignment requirement used by that
operation.

Unavailable synthesized copy construction or assignment does not by itself
make a closed class invalid. It becomes an error when some declaration or body
operation requires that capability, following the ordinary
[class lifecycle contract](CLASSES_AND_LIFECYCLE.md#copy-capabilities).

## Explicit interface constraints

Nominal behavior is expressed with a class-level `where` clause:

```ska
class SortedVec<T>
where T: Comparable
{
    // Comparable requirements may be selected on T here.
}
```

Multiple requirements are conjunctive:

```ska
class SortedSet<T>
where T: Comparable, T: Equatable
{
    // ...
}
```

The right side must name an interface. `T: Comparable` means that the argument
is an exact class whose effective nominal conformance includes `Comparable`.
It does not mean that `T` is the bare non-owning `Comparable` view, and it does
not override storage or lifecycle requirements inferred from the template.

A generic body cannot select a method from an unconstrained parameter merely
because a particular argument happens to declare a matching method. Generic
classes do not introduce structural duck typing.

A container parameterized by a shared pointee states that construction
explicitly:

```ska
class SharedSortedVec<T>
where T: Comparable
{
    storage: (shared T)?[];
    init() { self.storage = (shared T)?[](); }
}
```

This is instantiated as `SharedSortedVec<Item>` when `Item implements
Comparable`. It is distinct from an unconstrained `Vec<shared Comparable>`,
whose complete argument is an owning interface view.

Storage and lifecycle capabilities are inferred rather than written as
`stored`, `copy`, `assign`, `default`, or `destroy` bounds. Those source bounds
are outside the initial profile.

## Complete-class validation

Each requested closed application validates the complete specialized class,
including all fields, static fields, lifecycle members, initializers, methods,
bases, and interface conformances. An invalid unused method is not postponed
until a later call.

The effective requirement set is therefore the conjunction of every member's
requirements. Member-level `where` clauses are not part of the initial
feature. They are the intended future way to make one method conditionally
available; lazy method instantiation is not used as an implicit substitute.

Definition errors independent of an argument are rejected on the template,
including duplicate parameters, invalid or inaccessible bounds, unknown
nondependent names, and unsupported operations on unconstrained parameters.
Argument-dependent failures are reported at the closed application with a
note at the template use that generated the requirement.

## Class behavior

A closed generic class otherwise follows ordinary exact-class semantics:

```ska
Vec<Str>()
Vec<Str>(copy source)
new Vec<Str>()
new Vec<Str>(copy source)
Vec<Str>.factory(arguments)
```

Initializer overload resolution, access, allocation, publication, adoption,
copying, assignment, destruction, evaluation order, and cleanup are unchanged
after substitution. Construction directly through a parameter, such as `T()`
or `new T()`, is not supported because the initial constraint model has no
constructor requirement.

Each closed application has independent class-owned static state. For example,
`Cache<Str>.count` and `Cache<i64>.count` are distinct static fields with
independent initialization dependencies, activation, replacement, and
shutdown.

A generic class may extend an ordinary class or parameter-bearing generic
application, and an ordinary class may extend a closed application:

```ska
class Derived<T> extends Base<T> {
    init() { super(); }
}

class StringVec extends Vec<Str> {
    init() { super(); }
}
```

After substitution, the existing inheritance, finite-containment, override,
interface conformance, slicing, dispatch, base lifecycle, and access rules
apply to the exact closed identities. A template cannot extend a bare type
parameter or raw generic name.

Generic classes may implement existing non-generic interfaces. Each closed
application is checked independently because substituted signatures may affect
conformance. Generic interfaces and generic interface requirements are not in
the initial profile.

## Names, modules, and visibility

A generic class is one top-level declaration in the ordinary module namespace.
Importing or qualifying it does not instantiate it; applying closed arguments
does.

Type parameters are type-namespace bindings scoped over the header, bounds,
members, lifecycle declarations, signatures, and bodies. A parameter shadows
an unqualified imported or module declaration in that template; qualification
can still name the hidden declaration.

Names written in a template resolve in its definition module under its imports
and declaring-class access. Instantiation does not capture declarations from
the application module. Argument spellings resolve at the application site
and then enter the template as exact type identities.

Specialization grants no private access to an argument type. Template and
generated member visibility follow ordinary module and declaring-class rules.

## Diagnostics and determinism

An invalid application identifies both the argument and the originating
template use. A representative diagnostic is:

```text
Vec<Interface> is not a valid generic class application
  Interface cannot be an inline optional payload
  required by field `storage: T?[]` in `Vec<T>`
  use `shared Interface` when the vector must own interface values
```

Nested failures retain their outer-to-inner application and type-constructor
path. Lifecycle failures retain existing base and field capability paths.
Equivalent repeated failures do not create independently ordered diagnostic
cascades.

Closed identities, specialization order, diagnostics, static activation, and
generated artifacts are deterministic and do not depend on hash iteration.

## Initial exclusions

The frozen initial profile excludes:

- generic top-level functions, methods, and constructors independent of their
  class's parameters;
- generic interfaces and interface requirements;
- member-level constraints;
- generic aliases and other generic declaration families;
- argument inference, defaults, wildcards, existentials, and higher-kinded
  parameters;
- variance and conversions between applications;
- partial or explicit specialization;
- parameter construction and constructor bounds;
- base-class, negative, disjunctive, same-type, associated-type, and
  user-defined capability constraints;
- source-visible lifecycle bounds;
- lazy member validation;
- erased code, runtime type arguments, dictionaries, and reflection;
- separate-compilation or stable package ABI for specializations; and
- collection protocols, indexing sugar, or automatic replacement of the
  implemented `std::vec::VecObj` API.

The [generic-class compiler contract](../compiler/GENERIC_CLASSES.md) defines
specialization identities, requirement representation, phase ownership,
recursion handling, lower-IR boundaries, target realization, and testing.

# Generic Interfaces

Status: frozen design; source syntax, non-executable template identities,
definition-site template semantics, structural claims and bounds, closed
application discovery, ordinary closed interface materialization, exact
nominal class conformance, generic bounds, and bound-selected calls
implemented. Closed applications also use the ordinary interface alias,
shared-owner, optional, array, cast, type-test, and structural-call model;
lower-IR and native execution coverage is not yet complete. This document
defines the confirmed source-visible generic-interface contract. The
[status matrix](STATUS.md) remains authoritative for availability, and the
[implemented grammar](GRAMMAR.md) is the exact syntax accepted by the current
compiler. Resolution assigns distinct interface-template, template-requirement,
and owner-correct type-parameter identities. It retains parameter-bearing
applications in generic class/interface claims and bounds, deduplicates closed
requests with exact source origins, and materializes every valid requested
application as an ordinary exact interface identity. Generic-interface bounds
close to those exact identities and bound calls retain ordinary interface
dispatch.
Generic interface signatures retain
structural parameter, function, shared, optional, array, generic-class, and
generic-interface terms plus contextual capability requirements.

Generic interfaces parameterize an interface declaration with explicit type
arguments. A generic interface is a compile-time template rather than a
runtime value or interface. Every accepted closed application denotes one
exact ordinary interface after semantic specialization.

```ska
public interface Producer<T> {
    fn produce() -> T;
}

public interface Consumer<T> {
    mut fn consume(value: T) -> unit;
}
```

`Producer<Str>` and `Producer<u64>` are distinct interfaces. Neither the
template name `Producer` nor a parameter-bearing application has runtime
identity.

## Declarations and applications

The frozen grammar extension is conceptually:

```text
generic-parameter-list     = "<" identifier {"," identifier} ">"
generic-argument-list      = "<" storage-type {"," storage-type} ">"
generic-where-clause       = "where" generic-requirement
                            {"," generic-requirement}
generic-requirement        = identifier ":" named-type

interface-declaration      = "interface" identifier
                             [generic-parameter-list]
                             [generic-where-clause]
                             "{" {interface-requirement} "}"

class-declaration          = "class" identifier
                             [generic-parameter-list]
                             ["extends" named-type]
                             ["implements" named-type {"," named-type}]
                             [generic-where-clause]
                             "{" {class-member} "}"

named-type                 = declaration-path [generic-argument-list]
```

Top-level `public` visibility remains outside the declaration production.
Semantic analysis requires `implements` targets and `where` right sides to
denote interfaces.

A generic interface has one or more named parameters. Applications supply
every argument explicitly and in declaration order:

```ska
Producer<Str>
PairSource<Str, u64>
Producer<shared Readable>
```

Arity is exact. Empty parameter or argument lists, omitted or inferred
arguments, defaults, named arguments, wildcards, raw generic names, and
partial applications are invalid. A non-generic declaration rejects an
argument list.

Inside a generic class or interface, an application may contain parameters:

```ska
interface PairSource<A, B> {
    fn first() -> A;
    fn second() -> B;
}

class PairBox<A, B> implements PairSource<A, B> {
    /* ... */
}
```

Such an application becomes an interface only after enclosing substitution
closes every argument. Outside a generic template, every application must
already be closed.

Interface requirements may use enclosing interface parameters in parameter
and result types and in nested type applications. Requirements do not gain
their own generic parameter lists. Existing interface restrictions remain:
interfaces contain unique-named instance method requirements without bodies,
fields, lifecycle declarations, static requirements, default methods,
overloads, inheritance, or nested declarations.

## Exact identity and invariance

Every canonical closed application has one exact interface identity:

```text
Producer<Str>  != Producer<shared Str>
Tag<Str>       != Tag<u64>
left::Tag<Str> != right::Tag<Str>
```

This remains true when two applications have identical substituted
requirements or no requirements. Source grouping, imports, and equivalent
optional shorthand do not create new identities.

Applications are invariant in every parameter. Requirement position does not
infer covariance or contravariance:

```text
Producer<Derived>        is not Producer<Base>
Consumer<Base>           is not Consumer<Derived>
Producer<shared Derived> is not Producer<shared Obj>
```

Ordinary conversions may still occur at parameter, result, assignment, view,
or cast boundaries selected inside an implementation. They do not convert one
generic interface application into another.

## Contextual application validity

There is no global whitelist of generic-interface arguments. Each closed
application substitutes its arguments through every requirement signature and
nested application, then applies the ordinary type-position rules.

```ska
interface Producer<T> {
    fn produce() -> T;
}

interface Observer<T> {
    fn observe(ref value: T) -> unit;
}

interface Marker<T> {}
```

Representative consequences are:

- `Producer<Str>` is valid because `Str` is a valid owning result;
- `Producer<Readable>` is invalid because a bare interface view cannot be an
  owning result;
- `Producer<shared Readable>` may be valid because a shared interface owner is
  an owning result;
- `Observer<Readable>` may be valid because a bare interface is a valid
  read-only alias target; and
- `Marker<Readable>` may be valid because the parameter creates no storage,
  result, alias, or lifecycle obligation.

The complete interface is valid or invalid for an application. Requirements
are not instantiated lazily when first called. A failure identifies both the
argument and the requirement position or nested application that made it
invalid.

## Generic interface bounds

Template-level `where` clauses may name an ordinary interface or a generic
interface application:

```ska
class Pipeline<T, Source>
where Source: Producer<T>
{
    fn run(ref source: Source) -> T {
        return source.produce();
    }
}

interface RankedProducer<T, Rank>
where T: RankedAs<Rank>
{
    fn produce() -> T;
}
```

Interface-level bounds allow a generic interface signature to compose nested
generic applications whose own validity depends on nominal conformance.

At a closed application, each bound requires the exact argument to have
effective nominal conformance to the exact closed interface on the right.
The initial bound subject is a type parameter, and the satisfying argument is
an exact class. Bounds do not lift through `shared`, make a bare interface
view satisfy itself, or authorize structural conformance. Multiple bounds are
conjunctive.

A body may select a member on a parameter only through a declared bound. The
selection is fixed to the named interface requirement at the template's
definition site. A later specialization cannot redirect it to a same-named
class method or another bound. If multiple bounds expose an ambiguous name,
the template is invalid because the initial profile adds no qualified
bound-member syntax.

Calling a bound-selected requirement retains ordinary interface dispatch.
Knowing a closed type argument does not by itself change source-visible
dispatch or authorize devirtualization.

## Nominal conformance

An ordinary or generic class may claim a generic interface application:

```ska
class TextProducer implements Producer<Str> {
    fn produce() -> Str { /* ... */ }
}

class Box<T> implements Producer<T> {
    /* ... */
}
```

An ordinary class claim must be closed immediately. A generic class claim may
contain its parameters and closes independently for each class application.
After substitution, existing exact conformance rules apply:

- every requirement has one compatible effective public instance method;
- name, arity, parameter modes and types, result type, and receiver mutability
  match exactly;
- inherited methods and valid overrides may satisfy requirements;
- static and private methods do not satisfy requirements; and
- conformance is inherited only as the same exact closed application.

A class may implement multiple distinct applications of one template:

```ska
class Tagged implements Tag<Str>, Tag<u64> {}
```

Each is checked independently. Marker applications and applications whose
requirements can share one exact ordinary method are valid. Incompatible
same-named requirements remain unsatisfied because Skald does not add method
overloading. Repeating the same closed application or redundantly naming an
inherited exact conformance is invalid under the ordinary duplicate rules.

## Views, ownership, casts, and tests

A closed generic interface uses the existing interface value model:

```ska
fn inspect(ref value: Producer<Str>) -> unit { /* ... */ }
fn update(mut ref value: Consumer<Str>) -> unit { /* ... */ }
fn keep(value: shared Producer<Str>) -> shared Producer<Str> {
    return value;
}
```

Bare `Producer<Str>` is a non-owning object view. It is valid only in existing
view and alias positions and cannot be stored or returned as an owning value.
`shared Producer<Str>` is the ordinary owning interface handle and retains
existing copy, transfer, optional, array, field, parameter, result, cast, and
cleanup behavior.

Views, casts, shared casts, and type tests target one exact application:

```ska
value is Producer<Str>
(Producer<Str>) value
(shared Producer<Str>) owner
```

They succeed only when the complete object's dynamic class implements that
exact closed interface. No relation to another application of the same
template is implied.

A closed interface application may itself be a generic class argument. The
outer template's actual uses determine legality:

```ska
AliasOnly<Producer<Str>>
Vec<Producer<Str>>          // invalid when Vec needs an owning T value
Vec<shared Producer<Str>>   // valid when ordinary Vec requirements hold
```

Generic interfaces add no independently stored bare interface value,
interface boxing, escaping borrow, reference local, implicit owner creation,
interface method reference, or interface-to-interface implicit conversion.

## Names, modules, and visibility

An interface template is one top-level declaration in the ordinary module
namespace. It uses existing private-by-default visibility, `public`, imports,
qualification, collision rules, and declaration access. Naming or importing a
template does not instantiate it.

Type parameters are scoped over interface bounds and requirements. Names
written inside a template resolve in its definition module. Argument
spellings resolve at the application site and enter specialization as exact
semantic identities. Specialization does not capture caller-local imports or
grant private access to argument declarations.

Whole-program compilation may specialize templates across modules under
existing visibility. Separate-compilation ownership and stable package ABI for
generic templates remain deferred.

## Diagnostics and determinism

Diagnostics distinguish declaration errors, invalid applications, failed
bounds, and failed class conformance. Nested errors retain the outer-to-inner
application path. Repeated uses of one invalid application reuse one semantic
failure rather than allocating distinct identities or producing unstable
diagnostic cascades.

Template identities, closed interface identities, requirement identities,
specialization order, conformance maps, witness metadata, dumps, diagnostics,
and generated artifacts are deterministic and independent of hash iteration.

## Exclusions and future consumers

The frozen initial profile excludes:

- compiler-generated interfaces or conformances for primitive types;
- operator protocols, operator overloading, and primitive operator lowering
  through interfaces;
- iterable, iterator, range, generator, or sequence standard protocols and all
  new loop syntax or lowering;
- generic functions, generic methods, generic constructors, and requirement-
  local type parameters;
- inference, defaults, placeholders, wildcards, raw types, generic aliases,
  and partial or explicit specialization;
- variance, associated types, higher-kinded parameters, existential
  applications, interface inheritance, default methods, and same-type bounds;
- base-class, negative, disjunctive, structural, or user-defined capability
  bounds;
- method overloading and qualified bound-member disambiguation;
- erased generics, runtime dictionaries, reflection, and runtime
  specialization; and
- separate-compilation or stable package ABI for templates.

Future `Iterable<Item, State>` or `Add<Right, Result>` interfaces can use this
contract. Primitive conformances, operator syntax and lowering, iteration
semantics, and optimization guarantees require separate designs.

The [generic-interface compiler contract](../compiler/GENERIC_INTERFACES.md)
defines template identities, specialization, phase boundaries, witness
realization, and verification obligations. The archived
[design record](../archive/GENERIC_INTERFACES_DESIGN_PROPOSAL.md) preserves the
confirmed decisions, and the active
[implementation roadmap](../roadmaps/GENERIC_INTERFACES_ROADMAP.md) owns
delivery order without redefining this language contract.

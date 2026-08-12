# Generic Classes Design Proposal

Status: frozen design; GC1 through GC16 were confirmed together on 2026-08-12
and promoted into living language and compiler contracts before the
implementation roadmap was created.

This proposal adds generic class declarations whose type parameters are
replaced by explicit closed type arguments. It is aimed first at reusable
owning containers such as `Vec<T>`, where Skald's inline values, shared owners,
aliases, arrays, recursive optionals, deterministic lifecycle, and non-owning
object views make a universal erased `Obj` representation inappropriate.

The design uses semantic specialization. A generic declaration is a template,
not a runtime class. Each accepted closed application such as `Vec<Str>` or
`Vec<shared Interface>` becomes an ordinary exact class with its own stable
identity, fully substituted fields and signatures, selected lifecycle plans,
verified MIR, and target code. No unresolved type parameter reaches ordinary
resolved classes, HIR, MIR, verification, or the backend.

The promoted [language contract](../language/GENERIC_CLASSES.md) and
[compiler contract](../compiler/GENERIC_CLASSES.md) are authoritative. The
[status matrix](../language/STATUS.md) remains the sole authority for compiler
availability, and the [implemented grammar](../language/GRAMMAR.md) remains the
exact accepted syntax until implementation changes it. This frozen design does
not make generic syntax executable.

## Intended outcome

The initial generic-class feature should provide:

- generic class declarations with one or more named type parameters;
- explicit closed generic applications in every ordinary class-type position;
- generic construction, allocation, static selection, inheritance, and
  interface implementation after all arguments are supplied;
- literal structural substitution through arrays, optionals, shared owners,
  nested generic applications, and grouping;
- distinct exact class identities for distinct accepted argument lists;
- invariant generic classes with no erased runtime representation;
- inferred structural and lifecycle requirements derived from every use of a
  parameter in the complete class;
- explicit nominal interface constraints through a class-level `where` clause;
- definition-site name resolution and use-site validation of closed arguments;
- deterministic specialization caching, recursion diagnostics, semantic dumps,
  and backend names; and
- no public C runtime ABI extension.

The motivating vector shape is:

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

Its storage composes without a special generic-array rule:

| Application | Substituted backing type |
|---|---|
| `Vec<Str>` | `Str?[]` |
| `Vec<Str?>` | `Str??[]` |
| `Vec<shared Str>` | `(shared Str)?[]` |
| `Vec<shared Interface>` | `(shared Interface)?[]` |

`Vec<Interface>` is invalid for this declaration because the bare interface
view cannot become the payload of `T?`. That is a consequence of this
template's use of `T`, not a global ban on interface type arguments.

## Current boundary and architectural evidence

Skald currently has no generic declaration or application syntax. `T[]` is a
built-in array type constructor, not a user-defined generic application. The
implemented compiler nevertheless already has most of the closed-type
machinery specialization should reuse:

- recursive source types distinguish named, shared, optional, grouped, and
  array construction;
- resolution canonicalizes complete array and optional types bottom-up;
- resolved types distinguish exact classes, non-owning interface and `Obj`
  views, arrays, shared targets, and recursive optionals;
- class fields and value parameters reject bare interface and `Obj` views as
  non-owning values;
- aliases admit a different type family, including interface and `Obj` views;
- optional payload validation, array element validation, shared-target
  validation, and stored-value validation retain their own contextual rules;
- array lifecycle metadata independently selects default construction, copy
  construction, assignment, and destruction from the exact element type;
- exact-class copy capabilities are computed recursively through fields,
  optionals, arrays, shared owners, and bases;
- inline containment is validated from exact resolved field and base types;
  and
- HIR and MIR require concrete identities and executable lifecycle plans.

These boundaries favor specialization before ordinary HIR. Extending every
lower phase with an abstract `T`, runtime type descriptor, erased storage,
conditional cleanup, and generic calling convention would duplicate logic
that already works for closed types and would weaken the compiler's current
trust boundaries. The [compiler phase contract](../compiler/PHASES_AND_IR.md),
[array compiler contract](../compiler/ARRAYS.md), and
[optional compiler contract](../compiler/OPTIONAL_VALUES.md) remain the
authorities for those closed operations.

The sibling Niflheim repository deliberately has no generic syntax. Its
standard library uses a generated source template for primitive vector
specializations, while its reference-oriented object model permits broader
use of `Obj` plus checked casts. That demonstrates the maintenance problem and
container demand, but it does not provide a suitable type or ownership model
for Skald. Skald's exact inline values, optional layers, shared owners, and
deterministic lifecycle make semantic closed-type substitution the relevant
starting point.

## Design principles

1. **Generic arguments are not globally categorized as allowed or forbidden.**
   An application is valid exactly when its arguments satisfy every contextual
   requirement created by the template.
2. **Composition remains literal.** Substitution replaces a parameter leaf in
   the semantic type tree and then applies ordinary array, optional, shared,
   and generic constructors without flattening or regrouping.
3. **Templates and classes have different identities.** A source template is
   never mistaken for an executable exact class, and every closed application
   is an ordinary exact class after specialization.
4. **Mechanical constraints are inferred.** Storage, optional, array, alias,
   shared-target, default, copy, assignment, and destruction requirements come
   from the contexts and operations that need them.
5. **Nominal knowledge is explicit.** A generic body may select an interface
   requirement on `T` only when the class declares the corresponding interface
   constraint.
6. **The complete class is valid or invalid.** The initial profile does not
   hide unusable methods until call sites happen to instantiate them.
7. **Closed types reuse current semantics.** Specialization must not invent
   generic-only conversions, lifecycle operations, variance, or ownership.
8. **Lower phases stay concrete.** Ordinary resolved class declarations, HIR,
   MIR, verifiers, and targets see only accepted `ClassId` values and canonical
   closed types.
9. **Diagnostics retain both causes.** An invalid application points to the
   supplied argument and to the template use that generated the failed
   requirement.
10. **The initial feature remains finite.** Explicit arguments, deterministic
    caching, and conservative recursive-specialization checks take priority
    over inference, partial specialization, or type-level computation.

## Decision register

| ID | Decision | Confirmed direction | State |
|---|---|---|---|
| [GC1](#gc1--source-declarations-applications-and-constraints) | Source surface | Add `class C<T, U>`, closed `C<A, B>`, and class-level `where T: Interface` | **Confirmed** |
| [GC2](#gc2--template-parameter-and-instance-identity) | Identity | Separate template and parameter identities; give each closed application an ordinary `ClassId` | **Confirmed** |
| [GC3](#gc3--semantic-substitution-and-canonical-types) | Substitution | Substitute semantic type trees bottom-up and intern every closed compound type normally | **Confirmed** |
| [GC4](#gc4--contextual-requirement-model) | Allowed arguments | Infer role-specific predicates from every type position and operation; use no global argument whitelist | **Confirmed** |
| [GC5](#gc5--explicit-interface-constraints) | Expressed constraints | Express nominal interface knowledge with `where`; infer mechanical capabilities | **Confirmed** |
| [GC6](#gc6--class-wide-validation) | Validation granularity | Validate every member and lifecycle declaration for every closed instance | **Confirmed** |
| [GC7](#gc7--specialization-discovery-caching-and-recursion) | Specialization | Monomorphize explicit closed applications once through a deterministic worklist and cache | **Confirmed** |
| [GC8](#gc8--invariance-and-ordinary-compatibility) | Compatibility | Keep applications invariant and apply existing conversions only at their ordinary value boundaries | **Confirmed** |
| [GC9](#gc9--construction-static-state-and-lifecycle) | Class behavior | Specialize construction, allocation, static state, copying, assignment, and destruction per closed class | **Confirmed** |
| [GC10](#gc10--inheritance-interfaces-and-dispatch) | Object model | Permit closed generic bases and ordinary interfaces; validate hierarchy and conformance after substitution | **Confirmed** |
| [GC11](#gc11--modules-visibility-and-definition-context) | Modules | Resolve template names at the definition site and arguments at the application site under existing visibility | **Confirmed** |
| [GC12](#gc12--compiler-phase-and-ir-boundaries) | Compiler representation | Add a generic-template layer before ordinary resolved classes; keep HIR and MIR closed | **Confirmed** |
| [GC13](#gc13--target-realization-and-abi) | Target and ABI | Emit ordinary specialized class artifacts with deterministic distinct names and no runtime generic ABI | **Confirmed** |
| [GC14](#gc14--diagnostics-dumps-and-testing) | Quality | Make obligations, applications, specialization origins, and closed identities observable and deterministic | **Confirmed** |
| [GC15](#gc15--initial-exclusions) | Feature boundary | Exclude generic functions, methods, interfaces, inference, defaults, variance, erasure, and specialization syntax | **Confirmed** |
| [GC16](#gc16--promotion-and-roadmap-boundary) | Delivery | Confirm the complete register, promote living contracts, then create a PR-sized roadmap | **Confirmed** |

## GC1 — Source declarations, applications, and constraints

The confirmed grammar shape is:

```text
generic-parameter-list     = "<" identifier {"," identifier} ">"
generic-argument-list      = "<" storage-type {"," storage-type} ">"

generic-where-clause       = "where" generic-requirement
                            {"," generic-requirement}
generic-requirement        = identifier ":" declaration-path

class-declaration          = "class" identifier [generic-parameter-list]
                             ["extends" class-type]
                             ["implements" declaration-path
                                 {"," declaration-path}]
                             [generic-where-clause]
                             "{" {class-member} "}"

named-type                 = declaration-path [generic-argument-list]
class-type                 = named-type
```

The complete type grammar admits a closed generic application wherever an
exact class currently appears, after which existing postfix and prefix
constructors continue normally:

```ska
Vec<Str>
Vec<Str?>
Vec<shared Interface>
Vec<Str>?[]
shared Vec<Str>
Outer<Vec<Str?>[]>
```

Inside a generic template, an application may itself contain parameters, as
in `Pair<T, Str>` or `Base<T>`. Such a type is a template type term rather than
an executable class. Substitution must close it before it can request a
`ClassId`. Outside a generic template every application must already contain
only closed arguments.

All arguments are mandatory in the initial profile. A generic declaration
cannot be named as a raw class type, and a non-generic declaration rejects an
argument list. Arity is exact. Empty argument lists, variadic parameters,
default arguments, inferred placeholders, wildcards, and named arguments are
not accepted.

The confirmed interface-bound spelling is:

```ska
class SortedVec<T>
where T: Comparable
{
    // ...
}
```

Multiple constraints repeat the requirement form:

```ska
class SortedSet<T>
where T: Comparable, T: Equatable
{
    // ...
}
```

The initial grammar does not also add `<T: Comparable>` shorthand. One
canonical spelling makes declarations, dumps, recovery, and later extension
to relational constraints simpler. `extends` remains exclusively the spelling
for a class's direct base; it is not reused for a type-parameter bound.

`where` is contextual only after the optional class header clauses. Existing
uses of the spelling as an identifier remain unchanged. Angle brackets are
type-list delimiters only in generic declaration and type/application heads;
ordinary expression `<`, `>`, `<=`, `>=`, `<<`, and `>>` retain their current
meaning. The type parser must consume nested closing angles without changing
expression tokenization, so `Outer<Inner<Str>>` and `left >> right` remain
unambiguous in their respective contexts.

Generic construction and class-owned selection require the closed head:

```ska
var values: Vec<Str> = Vec<Str>();
var owner: shared Vec<Str> = new Vec<Str>();
var reserved: Vec<Str> = Vec<Str>.with_capacity(16u);
```

Expected types do not infer the omitted arguments in the initial feature;
`Vec()` and `Vec.with_capacity(16u)` continue to be invalid when `Vec` is a
generic template.

## GC2 — Template, parameter, and instance identity

Resolution assigns a `ClassTemplateId` to each generic class declaration and a
stable `TypeParameterId` to each parameter in source order. These identities
belong to the template layer and never stand in for `ClassId`.

A closed instance key is conceptually:

```text
GenericClassInstanceKey {
    template: ClassTemplateId,
    arguments: [ResolvedTypeKind],
}
```

Every argument in the key is already canonical: arrays and optionals carry
their interned identities, shared owners carry their exact resolved target,
and closed nested generic classes carry the `ClassId` assigned to their own
instance. Source spans and spelling variants never affect equality.

The specialization owner memoizes this key. The first valid request allocates
one ordinary `ClassId`; every later equivalent spelling reuses it. Distinct
argument sequences remain distinct exact classes even when their layouts or
generated instructions happen to match:

```text
Vec<Str>                  != Vec<Str?>
Vec<shared Item>          != Vec<shared Interface>
Pair<i64, u64>            != Pair<u64, i64>
```

An uninstantiated template has no layout, lifecycle table entry, dispatch
table, static storage, emitted body, or runtime identity. Its source identity
exists only so the compiler can validate and specialize it.

Within a template, each type parameter is a type-namespace binding whose scope
begins after the complete parameter list and includes the base, interfaces,
`where` clause, fields, static fields, lifecycle declarations, signatures, and
bodies. Duplicate parameter names are invalid. A parameter shadows an
unqualified imported or module declaration in that template scope; qualified
paths remain available to name the hidden declaration.

## GC3 — Semantic substitution and canonical types

The template layer represents types structurally, including a parameter leaf
and a generic application node in addition to the existing constructors:

```text
Parameter(TypeParameterId)
Named(declaration identity)
GenericClass(ClassTemplateId, arguments)
Shared(target)
Optional(payload)
Array(element)
```

Specialization recursively substitutes each parameter leaf with its complete
closed argument, then resolves and interns the resulting constructors from the
inside out. It never edits source text, concatenates display names, or reparses
a synthesized type spelling.

For a field declared as `T?[]`, substitution produces:

```text
T = Str
Array(Optional(Str))

T = Str?
Array(Optional(Optional(Str)))

T = shared Str
Array(Optional(Shared(Str)))

T = shared Interface
Array(Optional(Shared(Interface)))
```

Every optional layer remains independently absent or present. Shared binding
also remains literal: substituting `shared Str` into `T?` creates
`Optional<Shared<Str>>`; it does not create `Shared<Optional<Str>>`.

Substitution applies to the complete class declaration: direct base,
interfaces where applicable, fields, static fields, parameter and result
types, construction heads, casts, type tests, nested generic applications,
and every other type-bearing body node. Grouping affects source parsing and
diagnostics but does not create a distinct semantic identity.

The resulting types enter the existing resolved type interner. The generic
feature must not create parallel optional, array, shared, or class type tables
for specialized code.

## GC4 — Contextual requirement model

There is no global predicate such as "valid generic argument." All syntactically
resolved Skald types, including `unit`, bare interfaces, and `Obj`, may appear
as arguments when the template never uses them in a context that rejects them.

The template checker instead records requirements over structural type terms.
A requirement contains at least the term being constrained, the contextual
capability, and the source span and template construct that caused it:

```text
GenericRequirement {
    type_term: GenericTypeTermId,
    capability: GenericCapability,
    origin: Span,
    reason: GenericRequirementReason,
}
```

The initial internal capability vocabulary should preserve current semantic
roles rather than prematurely collapse them into one broad `stored` bit:

```text
FieldStorage(type)
StaticStorage(type)
ValueParameter(type)
ValueResult(type)
AliasTarget(type, access)
OptionalPayload(type)
ArrayElement(type)
SharedTarget(type)
DefaultConstructible(type)
CopyConstructible(type)
Assignable(type)
Destroyable(type)
```

The current accepted sets overlap, but their reasons and future evolution are
different. For example, an interface is a supported alias target but not a
field value or optional payload. A shared interface owner is a stored value,
while its interface target remains a non-owning object view reached through
the owner. Shared optional boxes also demonstrate why shared-target and array-
element support should not be assumed identical merely because both are
owning types.

Type formation and declarations generate structural requirements:

| Template use | Generated requirement |
|---|---|
| field `value: T` | `FieldStorage(T)` |
| static field `value: T` | `StaticStorage(T)` plus its initializer or zero-default requirement |
| value parameter `value: T` | `ValueParameter(T)` |
| result `-> T` | `ValueResult(T)` |
| alias parameter `ref value: T` | `AliasTarget(T, read_only)` |
| alias parameter `mut ref value: T` | `AliasTarget(T, mutable)` |
| type `T?` | `OptionalPayload(T)` |
| type `T[]` | `ArrayElement(T)` |
| type `shared T` | `SharedTarget(T)` |

Body operations add only the capabilities they actually select. Declaring a
field of type `T?[]` does not by itself require `T` to be default-constructible,
copyable, or assignable. Constructing `T?[](length)` needs the default plan of
`T?`, which is outer absence and therefore does not need a default plan for
`T`. Copying occupied elements, returning a stored payload by value, or
replacing a possibly present slot adds the corresponding copy or assignment
requirements.

Recursive optional and array capability rules reuse current semantics. In
particular:

```text
DefaultConstructible(Optional<T>)
    succeeds for every well-formed Optional<T>

CopyConstructible(Optional<T>)
    requires the payload's selected copy operation when the payload is nontrivial

Assignable(Optional<T>)
    requires every operation needed by absent-to-present, present-to-present,
    and present-to-absent transitions

CopyConstructible(Shared<T>) and Assignable(Shared<T>)
    operate on the owner and do not require the pointee to be copyable
```

Requirements are evaluated after substitution by the same validators and
capability planners used for non-generic closed types. The generic layer owns
obligation collection and source attribution; it does not own a second copy
or assignment implementation.

These examples show why the model is contextual:

```ska
class Vec<T> {
    storage: T?[];
    init() { self.storage = T?[](); }
}

class Observer<T> {
    init() {}
    fn observe(ref value: T) -> unit {}
}

class Owner<T> {
    value: shared T;
    init(value: shared T) { self.value = value; }
}
```

`Vec<Interface>` fails `OptionalPayload(Interface)`. `Observer<Interface>` may
pass because a bare interface is a supported read-only alias target.
`Owner<Interface>` may pass because `Interface` is a supported shared target
and the stored field is the resulting owner `shared Interface`.

## GC5 — Explicit interface constraints

Mechanical requirements are inferred because they follow uniquely from the
template's representation and operations. Requiring authors to repeat them
would duplicate compiler knowledge and allow annotations to drift from the
implementation.

Nominal member knowledge is different. A body cannot select methods or other
behavior from an unconstrained parameter merely because some later
specialization happens to provide a matching name. Skald must not acquire
C++-style accidental duck typing.

The initial expressed constraint is therefore interface conformance:

```ska
class SortedVec<T>
where T: Comparable
{
    // Calls through Comparable requirements are available here.
}
```

`T: Comparable` means that the argument must be an exact class whose effective
nominal conformance includes the named interface. It does not mean that `T` is
the bare non-owning `Comparable` view, and it does not automatically lift
through a shared owner. Thus a shared collection constrained by its pointee
uses a pointee parameter explicitly:

```ska
class SharedSortedVec<T>
where T: Comparable
{
    storage: (shared T)?[];
    init() { self.storage = (shared T)?[](); }
}
```

It is instantiated as `SharedSortedVec<Item>` when `Item implements
Comparable`. An unconstrained value-generic `Vec<T>` remains independently
capable of accepting the complete type argument `shared Comparable` when its
actual uses permit that stored owner.

The right side of an initial `where` requirement must resolve to an interface.
Base-class bounds, negative bounds, disjunction, same-type equations,
associated types, and user-defined capability declarations are deferred.
Multiple interface bounds are conjunctive. If distinct bounds expose
ambiguous member names, the template definition is rejected unless a later
explicit selection syntax resolves that ambiguity; the initial feature adds
no such syntax.

Calls selected through a bound retain the interface requirement identity and
ordinary dispatch semantics. Specialization may use an existing proven-safe
devirtualization, but generic syntax itself does not authorize a different
dispatch result.

The compiler records inferred requirements in semantic dumps and diagnostics
as part of the template's effective contract. Source spellings such as
`stored`, `copy`, `assign`, or `default` are not introduced initially. A later
design may expose named lifecycle constraints for API stability, but those
must be checked against the inferred set rather than become a second source of
truth.

## GC6 — Class-wide validation

Every requested closed application validates the complete specialized class:

- all fields and static fields;
- every initializer and static initializer;
- copy construction, copy assignment, and destruction declarations or
  synthesized capability;
- every ordinary, private, static, virtual, and override method;
- the direct base and interface conformances; and
- every type and operation in every body.

An unused method does not postpone a failed requirement until its first call.
Consequently the effective class requirement is the conjunction of
requirements generated by all members. This matches Skald's exact class
identities, complete member tables, lifecycle metadata, whole-program
checking, and deterministic output.

Unavailable synthesized copying does not by itself invalidate a class. As for
ordinary classes, it becomes an error only when some declaration or operation
requires that capability. For example, `Vec<NonCopying>` may exist when its
implementation never copies its storage, while a `get() -> T` body that must
copy from a live slot makes the complete specialization require the selected
copy operation.

The template definition is checked before any application for errors that do
not depend on a concrete argument: duplicate parameters, invalid bound names,
inaccessible declarations, unknown nondependent names, forbidden operations
on unconstrained parameters, malformed lifecycle declarations, and other
structural errors. Argument-dependent operation selection is represented as a
requirement or delayed closed selection, not silently accepted as an
unresolved body.

Member-level `where` clauses are deferred. They are the intended future way to
make only `get` require copying while leaving the core container usable for a
non-copyable element. Lazy method instantiation is not the substitute for that
source-visible contract.

## GC7 — Specialization discovery, caching, and recursion

Every syntactic closed application in a checked declaration or body requests
specialization, whether or not reachability analysis would later prove the
surrounding code unused. A parameter-bearing application inside a template
requests specialization when substitution first makes it closed. Specializing
one template may therefore discover nested closed applications in its
substituted fields, signatures, bases, static values, or bodies. A
deterministic worklist processes those requests until no new key remains.

The specialization cache has three conceptual states:

```text
Requested
InProgress(ClassId)
Complete(ClassId)
```

Allocating the `ClassId` at `InProgress` permits a reference to the identical
closed key to close a recursive graph. Existing inline-containment validation
then decides whether that recursion has a legal indirection boundary. For
example, recursion through a shared owner or array may be finite at runtime,
while direct inline self-containment remains invalid.

Generic transformation can otherwise request an infinite family:

```ska
class Expanding<T> {
    next: shared Expanding<T[]>;
    init(next: shared Expanding<T[]>) { self.next = next; }
}
```

The initial profile uses a conservative deterministic guard: if a
`ClassTemplateId` reappears on the active specialization stack with a
different argument sequence, the application is rejected as non-terminating
generic specialization. Re-entry with the identical key reuses the in-progress
instance. This deliberately rejects clever converging type transformations;
type-level computation is not an initial goal.

Failed specializations are retained as failed cache entries for the remainder
of the compilation so repeated uses do not produce duplicate class identities
or independently ordered diagnostic cascades.

Specialization order is stable from module/declaration source order, member
source order, and argument order. Hash-map iteration must not affect IDs,
dumps, diagnostics, static activation, or emitted symbol order.

## GC8 — Invariance and ordinary compatibility

Every generic application is invariant in every parameter:

```text
Vec<Derived>          is not Vec<Base>
Vec<shared Derived>   is not Vec<shared Base>
Vec<shared Derived>   is not Vec<shared Interface>
```

This remains true when ordinary values of the element type have valid
class/interface up-views or shared-owner casts. Those conversions may occur at
an existing parameter, assignment, or explicit cast boundary selected inside
the vector API; they do not convert one vector class into another.

There is no common-supertype search, covariance, contravariance, wildcard,
existential application, or raw generic type. Type equality compares the
template identity and every exact canonical argument.

## GC9 — Construction, static state, and lifecycle

A closed generic class uses ordinary class syntax after its explicit head:

```ska
Vec<Str>()
Vec<Str>(copy source)
new Vec<Str>()
new Vec<Str>(copy source)
Vec<Str>.factory(arguments)
```

Initializer overload resolution, accessibility, explicit copy construction,
allocation, publication, cleanup, and shared adoption are unchanged after
substitution. Construction headed directly by a type parameter, such as
`T()` or `new T()`, is excluded initially because interfaces do not describe
initializer sets and the initial constraint language has no constructor
requirement.

Each closed instance owns independent class metadata and static storage:

```text
Cache<Str>.count
Cache<i64>.count
```

These are distinct static fields with independently substituted initializer
bodies, dependency effects, eager activation, and reverse shutdown. A static
selection itself requests the closed instance even if no inline object of that
class is constructed.

Fields, bases, and bodies of each instance feed the existing exact lifecycle
analysis. Shared arguments copy, assign, and destroy owner handles without
copying pointees. Nested optionals preserve every state. Arrays preserve their
backing ownership and element lifecycle. Exact inline class arguments use
their selected construction, copying, assignment, and destruction plans.

The class's source spelling inside its own lifecycle signatures remains
explicitly closed:

```ska
class Box<T> {
    value: T;

    init(value: T) {
        self.value = value;
    }

    copy(ref source: Box<T>) {
        self.value = source.value;
    }
}
```

No new `Self` type is introduced by this proposal.

## GC10 — Inheritance, interfaces, and dispatch

A generic class may extend an ordinary class or a generic class application:

```ska
class Derived<T> extends Base<T> {
    init() {
        super();
    }
}

class StringVec extends Vec<Str> {
    init() {
        super();
    }
}
```

After substitution the direct base must be one accepted exact class. The
existing hierarchy, finite-containment, base initialization, inherited member,
override, slicing, copy, assignment, destruction, and dispatch validation then
run on the closed identities. A template cannot extend a raw generic name or
a type parameter in the initial feature.

Generic classes may implement existing non-generic interfaces. Each closed
instance receives its own conformance analysis because substituted member
signatures may affect whether exact requirements are met. A failed conformance
invalidates that application with notes at both the `implements` clause and
the argument-dependent member. Generic interfaces and generic interface
requirements are outside the initial profile.

Virtual families and interface tables belong to closed `ClassId` values. They
do not dispatch on a runtime type argument, and no generic witness dictionary
is passed at calls.

## GC11 — Modules, visibility, and definition context

A generic class is one top-level declaration in the existing module namespace
and obeys ordinary public/private import rules. Importing or qualifying the
template does not instantiate it; a closed application does.

Names written in the template are resolved in the template's definition
module under its imports and declaring-class access. Instantiation does not
re-resolve those names against the importing module and therefore cannot
capture unrelated declarations at the application site. Type argument
spellings are resolved at the application site, then passed as canonical
identities to the template.

Private type arguments may be used where existing visibility permits their
application. Generated members retain the template declaration's visibility
and declaring-class privacy. A specialization does not gain access to private
members of an argument type merely because that type was supplied as `T`.

The initial whole-program module model permits all requested specializations
to be known before HIR and code generation. Separate compilation, serialized
generic templates, cross-package specialization ownership, stable public
mangling, and binary distribution are deferred with the broader package and
separate-compilation design.

## GC12 — Compiler phase and IR boundaries

The compiler gains one generic-template layer between source declaration
collection and ordinary resolved classes. Its responsibilities are:

- assign template and parameter identities;
- resolve definition-site nondependent names and interface bounds;
- retain structural parameter-bearing type terms;
- collect contextual requirements with source origins;
- reject definition-level misuse of unconstrained parameters;
- discover and canonicalize explicit closed applications;
- specialize each unique application into a complete ordinary resolved class;
  and
- report application failures before ordinary HIR construction.

The conceptual pipeline becomes:

```text
syntax and modules
    -> declaration and template collection
    -> generic template validation and requirement collection
    -> closed-application worklist and semantic substitution
    -> ordinary ResolvedProgram containing only concrete classes
    -> existing optional/array/containment/conformance/capability checking
    -> closed HIR
    -> verified closed MIR
    -> target lowering
```

Some argument-dependent selections currently owned by resolution, including
initializer overloads, casts, and exact callable compatibility, cannot be
selected while a type term still contains a parameter. The template layer
must retain a typed delayed-selection form or specialize the already
name-resolved body before ordinary selection. It must not encode unresolved
operations as fake `ClassId`, overload, or callable identities.

The completed specialized declaration uses the existing resolved structures
wherever their invariants already apply. If a dedicated
`ResolvedGenericClassTemplate` is introduced, it remains separate from
`ResolvedClassDeclaration`; adding `Parameter` variants throughout ordinary
HIR and MIR is explicitly contrary to this design.

Specialization metadata needed only for diagnostics and dumps maps each closed
`ClassId` back to its template identity, canonical arguments, and application
origins. Executable phases do not need to inspect that provenance to recover
types or ownership.

## GC13 — Target realization and ABI

Each accepted closed class lowers exactly like an equivalent hand-written
ordinary class. Its inline layout is computed from substituted fields and
base; shared allocation layout and finalization use that exact class; method,
lifecycle, dispatch, static, and helper symbols remain statically known.

Emitted private symbols distinguish the template declaration and full exact
argument sequence through deterministic collision-free mangling. User-facing
dumps and diagnostics render qualified source names such as
`std::vec::Vec<std::str::Str>` rather than backend mangles or numeric IDs.

Specialization may increase code and metadata size. The initial feature uses
one specialization per exact argument key and performs no cross-type layout
sharing, erased helper generation, or identical-code folding in semantic
phases. Ordinary linker folding or later target optimization may merge code
only when observable identities and metadata remain correct.

No runtime type-argument descriptor, generic dispatch dictionary, reflection
record, allocation header field, public C function, or runtime ABI version
change is required. Existing external ABI restrictions continue to reject
generic class values wherever their equivalent ordinary exact class would be
rejected.

## GC14 — Diagnostics, dumps, and testing

Diagnostics must distinguish template-definition errors from invalid closed
applications.

Definition examples include:

- duplicate type parameters;
- a `where` right side that is not an interface;
- unknown or inaccessible definition-site names;
- field or member selection on an unconstrained parameter;
- type-parameter construction without a supported constructor constraint; and
- a raw or wrong-arity generic application.

Application diagnostics point first to the application and then explain the
requirement origin:

```text
Vec<Interface> is not a valid generic class application
  Interface cannot be an inline optional payload
  required by field `storage: T?[]` in `Vec<T>`
  use `shared Interface` when the vector must own interface values
```

Nested failures retain an outer-to-inner path through applications and type
constructors. Lifecycle failures reuse existing copy-capability field/base
paths beneath the generic requirement rather than replacing them with a flat
"constraint not satisfied" message. Repeated requests for the same failed key
produce one primary failure with secondary use sites where practical.

Deterministic syntax and semantic dumps should expose:

- template and parameter identities;
- source type terms and explicit interface bounds;
- inferred contextual requirements with origin spans;
- specialization keys and assigned `ClassId` values;
- the mapping from parameter to exact argument;
- substituted fields, bases, signatures, and body selections; and
- recursion/cache state when diagnosing a specialization cycle.

Focused tests should cover at least:

- parsing and recovery for declarations, applications, `where`, nested closing
  angles, comparisons, and shifts;
- parameter scope, shadowing, arity, qualification, imports, and visibility;
- canonical identity reuse across equivalent optionals and grouping;
- distinct identity across different templates, argument order, optionals,
  arrays, exact classes, interfaces, and shared targets;
- `Vec<Str>`, `Vec<Str?>`, `Vec<shared Str>`, and
  `Vec<shared Interface>` backing substitution;
- rejection of `Vec<Interface>`, `Vec<Obj>`, and `Vec<unit>` at the precise
  requirement site;
- acceptance of bare interfaces in a template that uses `T` only as a
  supported alias or shared target;
- optional default construction without a payload default operation;
- conditional copy and assignment availability through nested optionals and
  arrays;
- explicit interface constraints, nominal conformance, member lookup, and
  ambiguity;
- invariant application compatibility;
- generic bases, ordinary interfaces, virtual behavior, containment, and
  recursive specialization diagnostics;
- per-instance static storage, activation dependencies, and shutdown;
- HIR and MIR proving that no parameter-bearing type survives; and
- end-to-end native vector behavior across primitive, inline class, optional,
  shared exact, and shared interface-owner elements.

The active implementation roadmap assigns tests to their phase owners,
add golden behavior only when the complete pipeline is executable, run the
supported MSRV gate for Rust changes, and finish with the repository's
documented `make check` interface.

## GC15 — Initial exclusions

The initial generic-class feature does not include:

- generic top-level functions;
- generic methods or constructors independent of a class's parameters;
- generic interfaces or generic interface requirements;
- member-level `where` clauses;
- generic type aliases, enums, or other declaration families;
- omitted or inferred type arguments;
- default, variadic, wildcard, existential, or higher-kinded parameters;
- variance annotations or conversions between applications;
- partial or explicit specialization;
- type tests, pattern matching, or overload selection over type arguments;
- construction directly through a type parameter;
- base-class, constructor, negative, disjunctive, same-type, associated-type,
  or user-defined capability constraints;
- source-visible `stored`, `copy`, `assign`, `default`, or `destroy` bounds;
- lazy validation that removes unavailable members from selected instances;
- erased generic code, runtime dictionaries, reflection, or reified argument
  lists;
- separate-compilation or stable package ABI for specializations; or
- a new collection protocol, indexing syntax, iterator model, or replacement
  of `std::vec::VecObj` before generic vectors independently match its contract.

These exclusions keep the first implementation centered on the most important
use case: exact generic classes with explicit provided types. Later generic
functions can reuse the type-term, requirement, substitution, and
specialization model once class behavior has exercised it end to end.

## GC16 — Promotion and roadmap boundary

All GC1 through GC15 decisions were confirmed together. Promotion:

- added the frozen generic-class surface and semantics to focused living
  language documentation;
- retained the authoritative implemented grammar unchanged until the first
  implementation task accepts the confirmed syntax;
- added the template/specialization phase contract to compiler architecture and
  phase documentation;
- updated the status matrix from open question to frozen design;
- archived this proposal as the historical decision record; and
- created an active implementation roadmap whose tasks are ordered by stable
  ownership boundaries rather than by source file.

The active roadmap settles implementation sequencing without reopening the
language decisions. It orders syntax and identities, template type terms and
name resolution, requirement collection, closed specialization, class
declarations and lifecycle, bodies and nominal bounds, inheritance/statics,
lower-phase integration, and broad hardening by their stable dependencies.

The confirmation established that:

- `where T: Interface` is the sole initial written bound;
- mechanical constraints remain inferred and role-specific;
- every closed instance validates the complete class;
- all generic arguments are explicit;
- applications are invariant;
- each closed instance gets an ordinary exact class identity;
- HIR, MIR, and the backend remain free of unresolved parameters; and
- the initial exclusions are acceptable for the first executable profile.

The promoted living contracts now own the frozen language and compiler
meaning. This archived proposal preserves the decision record; the active
roadmap owns implementation order.

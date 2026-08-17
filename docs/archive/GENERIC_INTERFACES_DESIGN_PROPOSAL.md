# Generic Interfaces Design Proposal

Status: frozen design; GI1 through GI16 were confirmed together on 2026-08-17
and promoted into living language and compiler contracts before the
implementation roadmap was created.

This proposal extends Skald's existing interface model with explicit type
parameters and closed interface applications. It deliberately builds on the
implemented generic-class specialization boundary: a generic interface is a
compile-time template, and every requested closed application becomes an
ordinary exact interface before HIR. Existing interface views, nominal
conformance, witness dispatch, ownership, MIR, and backend behavior then
continue to operate on ordinary `InterfaceId` values.

The scope is generic interfaces themselves. Compiler-provided conformances for
primitive types, operator protocols and operator overloading, iteration
protocols, range types, and loop lowering are future consumers, not part of
this design.

The [status matrix](../language/STATUS.md) remains authoritative for compiler
availability, and the [implemented grammar](../language/GRAMMAR.md) remains the
exact accepted syntax. This proposal does not make generic-interface syntax
executable.

## Intended outcome

The initial generic-interface feature should provide:

- generic interface declarations with one or more named type parameters;
- explicit, fully closed interface applications in every position where an
  ordinary interface may currently appear;
- parameter-bearing interface applications inside generic class and interface
  templates, closed by semantic substitution;
- generic interface applications in `implements` clauses and generic `where`
  bounds;
- exact nominal conformance to each closed interface application;
- ordinary non-owning interface views and owning `shared` interface handles
  for closed applications;
- invariant applications with no implicit conversion between distinct
  argument lists;
- deterministic specialization, stable template and closed requirement
  identities, diagnostics, and dumps;
- unchanged interface-call representation and witness dispatch after
  specialization; and
- no runtime generic metadata, dictionaries, type arguments, or public runtime
  ABI extension.

A representative source shape is:

```ska
public interface Producer<T> {
    fn produce() -> T;
}

public interface Consumer<T> {
    mut fn consume(value: T) -> unit;
}

public class Box<T> implements Producer<T> {
    value: T;

    init(value: T) {
        self.value = value;
    }

    fn produce() -> T {
        return self.value;
    }
}

class Pipeline<T, Source>
where Source: Producer<T>
{
    fn run(ref source: Source) -> T {
        return source.produce();
    }
}
```

For `Box<Str>`, `Producer<T>` closes to `Producer<Str>`. That application has
one ordinary `InterfaceId`, its `produce` requirement has one ordinary
`InterfaceRequirementId`, and `Box<Str>` is checked against the substituted
signature `fn produce() -> Str`. No unresolved `T` reaches ordinary resolved
interfaces, HIR, MIR, or the backend.

## Current boundary and architectural evidence

Skald currently implements non-generic interfaces and closed generic classes.
The relevant boundaries are:

- a non-generic interface has one `InterfaceId`, and each source-order
  requirement has an `InterfaceRequirementId` owned by it;
- class conformance is nominal and exact, checked from the effective public
  instance methods after inheritance and override resolution;
- interface calls retain requirement identity through HIR and MIR, while the
  backend owns the concrete witness layout;
- bare interface values are non-owning object views and cannot be stored or
  returned as owning values;
- `shared Interface` is an ordinary owning handle and composes with fields,
  results, arrays, and optionals under the existing shared-owner rules;
- generic classes are compile-time templates whose accepted closed
  applications receive ordinary `ClassId` values before HIR;
- template type parameters are currently owned specifically by
  `ClassTemplateId`, and template types distinguish class applications but not
  interface applications;
- generic-class `implements` claims and `where` bounds currently resolve only
  to ordinary `InterfaceId` values; and
- a bound-selected call records an ordinary `InterfaceRequirementId`, so a
  parameter-bearing generic bound needs an equivalent stable template-level
  requirement identity before it can close.

These facts favor extending semantic specialization rather than introducing
runtime interface dictionaries. A closed interface application already has
all information needed by the ordinary conformance, view, type-test, cast,
dispatch, verifier, and backend paths.

Niflheim does not provide a generic-interface precedent to copy. Its current
interface design explicitly excludes generic interfaces. Its non-generic
interface and collection work is useful motivation, but Skald's implemented
closed specialization, exact inline values, and ownership distinctions are
the controlling architecture for this proposal.

## Design principles

1. **Templates and executable interfaces have different identities.** A
   generic declaration is never itself a runtime interface.
2. **Every lower-phase interface is closed.** Ordinary resolved declarations,
   HIR, MIR, verification, and targets see only `InterfaceId` and
   `InterfaceRequirementId` values.
3. **Conformance is exact and nominal.** Implementing `Producer<Str>` says
   nothing about `Producer<Obj>` or any other application.
4. **Substitution is structural.** Parameters are replaced in semantic type
   trees before existing type legality and conformance rules run.
5. **Ownership does not change.** A closed bare interface remains a non-owning
   view, while `shared` remains the owning interface form.
6. **Application legality is contextual.** An argument is accepted when every
   substituted requirement signature and nested application is legal; there
   is no global argument whitelist.
7. **Member selection has stable identity at both layers.** Template
   requirements support definition-time lookup; closed requirements support
   ordinary dispatch.
8. **One specialization mechanism coordinates classes and interfaces.**
   Interleaved requests must have deterministic caching, recursion handling,
   provenance, and failure behavior.
9. **No generic syntax grants optimization permission.** Bound-selected and
   interface-view calls retain ordinary interface semantics unless an
   independent proven-safe optimization applies.
10. **Future protocols do not distort the base feature.** Primitive
    conformances, operators, and iteration may use generic interfaces later,
    but add no special cases now.

## Decision register

| ID | Decision | Confirmed direction | State |
|---|---|---|---|
| [GI1](#gi1--source-declarations-and-applications) | Source surface | Add `interface I<T, U>`, require explicit applications, and admit parameter-bearing applications only inside templates | **Confirmed** |
| [GI2](#gi2--template-parameter-and-requirement-identity) | Identity | Add interface-template and template-requirement identities; generalize type-parameter ownership | **Confirmed** |
| [GI3](#gi3--closed-application-identity) | Closed representation | Give each canonical closed application one ordinary `InterfaceId` and ordinary requirement IDs | **Confirmed** |
| [GI4](#gi4--semantic-substitution-and-application-legality) | Substitution | Substitute structurally, then validate all closed signatures and nested applications contextually | **Confirmed** |
| [GI5](#gi5--generic-where-bounds) | Constraints | Permit closed or parameter-bearing generic interface applications on the right of template-level `where` bounds | **Confirmed** |
| [GI6](#gi6--nominal-conformance) | Conformance | Check each exact closed application independently after class and interface specialization | **Confirmed** |
| [GI7](#gi7--bound-member-selection-and-dispatch) | Calls | Resolve to a template requirement first, then map to the exact closed requirement and preserve ordinary dispatch | **Confirmed** |
| [GI8](#gi8--views-ownership-casts-and-tests) | Object model | Reuse bare-view, `shared`, cast, and type-test semantics for the exact closed interface | **Confirmed** |
| [GI9](#gi9--invariance-and-multiple-applications) | Compatibility | Keep applications invariant and allow multiple exact applications when ordinary method rules can satisfy all of them | **Confirmed** |
| [GI10](#gi10--specialization-discovery-caching-and-recursion) | Specialization | Use coordinated deterministic class/interface worklists, early IDs, cached failures, and conservative expansion guards | **Confirmed** |
| [GI11](#gi11--modules-visibility-and-name-resolution) | Modules | Reuse top-level visibility; resolve template-owned names at definition sites and arguments at application sites | **Confirmed** |
| [GI12](#gi12--compiler-phase-and-ir-boundaries) | Compiler architecture | Extend the template layer and close every application before ordinary resolved interfaces and HIR | **Confirmed** |
| [GI13](#gi13--target-realization-and-abi) | Runtime | Emit ordinary closed witness metadata without dictionaries, erasure, or ABI additions | **Confirmed** |
| [GI14](#gi14--diagnostics-dumps-and-testing) | Quality | Expose exact applications, obligations, mappings, origins, and recursion deterministically | **Confirmed** |
| [GI15](#gi15--initial-exclusions-and-future-consumers) | Boundary | Exclude primitives, operators, iteration, generic methods, variance, associated types, and erased generics | **Confirmed** |
| [GI16](#gi16--promotion-and-roadmap-boundary) | Delivery | Confirm the register, promote living contracts, then create a PR-sized implementation roadmap | **Confirmed** |

## GI1 — Source declarations and applications

The confirmed grammar shape is:

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

`public` remains the existing modifier on a top-level declaration rather than
part of the interface production. Semantic analysis, not the grammar,
requires every `implements` target and every `where` right side to denote an
interface.

All generic arguments are explicit and arity is exact. A generic interface
name without arguments is not a type, and a non-generic interface rejects an
argument list. Empty lists, omitted or inferred arguments, defaults,
wildcards, named arguments, and partial applications are excluded.

A closed application may appear wherever an ordinary interface currently can:

```ska
ref source: Producer<Str>
mut ref sink: Consumer<Str>
shared Producer<Str>
(shared Producer<Str>)?
Producer<Str>[]                 // rejected: still a bare non-owning element
(shared Producer<Str>)[]        // accepted under existing owner rules
```

The parser preserves the same recursive type structure and angle-bracket
evidence used by generic classes. Existing type-position rules decide whether
the resulting closed interface may be used as a view, alias target, shared
target, cast target, type-test target, generic argument, and so on.

Inside a generic template, an application may contain parameters:

```ska
interface PairSource<A, B> {
    fn first() -> A;
    fn second() -> B;
}

class PairBox<A, B> implements PairSource<A, B> { /* ... */ }
```

Such an application is a template type term. It must become fully closed
before it denotes an interface available to ordinary resolution or HIR.

An interface requirement may use parameters from its enclosing interface but
cannot declare method-level parameters of its own. Existing interface rules
remain unchanged: requirements are instance methods without bodies, fields,
static requirements, lifecycle declarations, overloads, or default methods.

## GI2 — Template, parameter, and requirement identity

Declaration collection assigns a generic interface a stable,
non-executable `InterfaceTemplateId`. It is distinct from both
`ClassTemplateId` and `InterfaceId`.

The current `TypeParameterId` is class-template-specific. Generic interfaces
generalize it around an explicit owner rather than encode an interface as a
synthetic class:

```text
GenericTemplateId =
    Class(ClassTemplateId)
    | Interface(InterfaceTemplateId)

TypeParameterId {
    owner: GenericTemplateId,
    index: source-order index,
}
```

This shared owner permits common parameter scopes, substitutions, diagnostic
rendering, and template-type traversal while keeping declaration-specific
tables strongly typed.

Each source requirement in a generic interface also needs a stable
`InterfaceTemplateRequirementId`:

```text
InterfaceTemplateRequirementId {
    interface: InterfaceTemplateId,
    index: source-order index,
}
```

This identity is required before any closed `InterfaceId` exists. It lets a
generic class body resolve `source.produce()` through a bound such as
`Source: Producer<T>` at the definition site. During specialization it maps
by owner and source-order index to the corresponding closed
`InterfaceRequirementId`.

Template IDs and template requirement IDs may appear only in the generic
semantic layer, diagnostics, dumps, and provenance. Runtime-facing layers
must not confuse them with executable interface identities.

## GI3 — Closed application identity

A canonical closed application key is conceptually:

```text
GenericInterfaceInstanceKey {
    template: InterfaceTemplateId,
    arguments: [ResolvedTypeKind],
}
```

The arguments use canonical semantic identities. Source spans, grouping,
optional shorthand, import aliases, and display spelling do not participate
in equality.

The first valid request for a key reserves one ordinary `InterfaceId`. Its
requirements receive ordinary `InterfaceRequirementId` values derived from
that closed owner and their source-order indexes. Equivalent requests reuse
the same identities; different templates or argument sequences remain
different even if their substituted signatures happen to be identical.

For example:

```text
Producer<Str>  != Producer<shared Str>
Tag<Str>       != Tag<i64>       // even when Tag has no requirements
left::Tag<Str> != right::Tag<Str>
```

An unrequested interface template has no ordinary `InterfaceId`, witness
metadata, or emitted artifact. The template remains visible as a top-level
declaration for lookup and future closed requests.

## GI4 — Semantic substitution and application legality

The template type representation gains a parameter-bearing interface form:

```text
InterfaceTemplate {
    template: InterfaceTemplateId,
    arguments: [ResolvedTemplateType],
}
```

Structural substitution replaces every parameter leaf, then closes nested
class and interface applications and interns existing compound types
bottom-up. It does not rewrite source text or flatten type constructors.

Every requested closed interface validates the complete substituted
declaration. This includes every parameter mode, parameter type, result type,
nested generic application, and declared `where` obligation. There is no lazy
per-requirement instantiation.

Argument legality follows actual use. For example:

```ska
interface Producer<T> {
    fn produce() -> T;
}

interface Observer<T> {
    fn observe(ref value: T) -> unit;
}

interface Marker<T> {}
```

- `Producer<Str>` is valid because `Str` is a valid owning result.
- `Producer<Readable>` is invalid because a bare interface view cannot be an
  owning result.
- `Producer<shared Readable>` may be valid because a shared interface owner is
  a valid owning result.
- `Observer<Readable>` may be valid because a bare interface is an eligible
  read-only alias target.
- `Marker<Readable>` may be valid because its parameter creates no storage,
  result, alias, or lifecycle obligation.

The compiler reuses the generic-class contextual-requirement model for
interface signatures. Definition-independent errors are diagnosed once on the
template. Application-dependent failures point both to the supplied argument
and to the substituted requirement position that made it invalid.

## GI5 — Generic `where` bounds

The right side of a template-level `where` requirement may be an ordinary
interface or a generic interface application:

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

Interface-level `where` clauses are included because they make nested
constrained applications composable. Without them, an interface signature
could not soundly mention a generic class or interface whose application
requires a nominal fact about one of the enclosing parameters.

A parameter-bearing right side remains a semantic interface-application term
until enclosing substitution closes it. At a closed application, each bound
requires the exact argument to have effective nominal conformance to the exact
closed interface application.

As in generic classes, the initial subject is a type parameter and nominal
conformance applies to exact class arguments. Bounds do not implicitly lift
through `shared`, treat a bare interface as satisfying itself, or create
structural/duck-typed conformance. Multiple bounds are conjunctive.

Ambiguous same-named requirements exposed through multiple bounds remain an
error because Skald has no qualified bound-member selection syntax. Generic
interfaces do not add such syntax.

## GI6 — Nominal conformance

Both ordinary and generic classes may claim closed generic interfaces:

```ska
class TextProducer implements Producer<Str> { /* ... */ }

class Box<T> implements Producer<T> { /* ... */ }
```

An ordinary class claim is closed immediately. A claim inside a generic class
retains a template interface term and closes with each class specialization.
After substitution, the existing conformance algorithm checks the exact
closed interface:

- every requirement must have one compatible effective public instance
  method;
- method name, arity, parameter modes and types, result type, and receiver
  mutability must match under the existing exact rules;
- inherited methods and overrides may satisfy requirements as they do today;
- static or private methods do not satisfy requirements; and
- the witness map records the closed `InterfaceRequirementId` to concrete
  `MethodId` relation.

Inherited conformance preserves its exact closed identity. A class conforming
to `Producer<Str>` through its base does not thereby conform to another
application.

Class and interface specialization must both be sufficiently published before
conformance runs. A class may reserve its `ClassId`, request the interface
applications in its claims, and perform conformance only after their closed
requirement signatures are complete.

## GI7 — Bound member selection and dispatch

Template resolution performs member lookup through the declared interface
bound, not through later argument-dependent class lookup. A selection records:

```text
- the bounded type parameter;
- the ordinary or parameter-bearing interface application;
- the ordinary InterfaceRequirementId or
  InterfaceTemplateRequirementId selected at the definition site; and
- the source member name and span for diagnostics.
```

When the enclosing class specializes, the interface application closes and a
template requirement maps to the same-index requirement owned by the closed
`InterfaceId`. The generated call then uses the ordinary requirement identity.

This preserves two important properties:

- a later class specialization cannot change which same-named interface
  requirement the source selected; and
- the call retains ordinary interface dispatch semantics even though the exact
  type argument is known during specialization.

Specialization alone does not authorize direct-call lowering. Any
devirtualization must be a separate optimization justified by the same rules
as a non-generic interface call.

## GI8 — Views, ownership, casts, and tests

A closed generic interface has exactly the current interface value model:

```ska
fn inspect(ref value: Producer<Str>) -> unit { /* ... */ }
fn update(mut ref value: Consumer<Str>) -> unit { /* ... */ }
fn keep(value: shared Producer<Str>) -> shared Producer<Str> {
    return value;
}
```

Bare `Producer<Str>` is a non-owning object view. It is valid only in the
existing view and alias positions and does not become an independently stored
or returned owning value. `shared Producer<Str>` is the existing owning
interface handle and retains its normal copying, transfer, optional, array,
field, parameter, result, cast, and cleanup behavior.

Class-to-interface views, checked casts, shared casts, and type tests target
the exact closed application:

```ska
value is Producer<Str>
(Producer<Str>) value
(shared Producer<Str>) owner
```

Runtime metadata asks whether the dynamic class implements that exact
`InterfaceId`. It does not match another application of the same template.
Generic interfaces add no interface boxing, escaping borrow, reference local,
implicit owner creation, or interface-to-interface conversion.

Closed generic interfaces may themselves be arguments to generic classes.
The outer template's contextual uses decide whether the bare view is legal:

```ska
AliasOnly<Producer<Str>>          // potentially valid
Vec<Producer<Str>>                // invalid if Vec stores T as an owning value
Vec<shared Producer<Str>>         // valid when ordinary Vec requirements hold
```

## GI9 — Invariance and multiple applications

Every generic interface application is invariant in every parameter:

```text
Producer<Derived>          is not Producer<Base>
Consumer<Base>             is not Consumer<Derived>
Producer<shared Derived>   is not Producer<shared Obj>
```

The result or parameter position of `T` does not infer covariance or
contravariance. There are no variance annotations, wildcard applications,
existentials, or conversions between applications in the initial feature.

A class may declare multiple distinct applications of the same template:

```ska
class Tagged implements Tag<Str>, Tag<i64> { /* ... */ }
```

Each application receives an independent conformance check and witness entry.
This is useful for marker interfaces and for cases where the same ordinary
method can exactly satisfy both closed signatures. Skald's lack of method
overloading naturally rejects incompatible requirements that would need two
same-named methods with different signatures. The design does not add a
broader blanket prohibition merely because the template identity is shared.

Duplicate claims of the same closed application, including redundant exact
inherited claims, use the existing duplicate-conformance rules.

## GI10 — Specialization discovery, caching, and recursion

Closed interface applications are discovered in type positions,
`implements` claims, bounds, casts, type tests, shared targets, class and
interface signatures, and nested generic applications. A parameter-bearing
application requests specialization when enclosing substitution first closes
it.

Class and interface requests can discover each other, so specialization has
one deterministic coordinator even if declaration-specific logic
and caches remain separate. Its request domain is conceptually:

```text
GenericSpecializationKey =
    Class(GenericClassInstanceKey)
    | Interface(GenericInterfaceInstanceKey)
```

An interface cache uses the same conceptual states as class specialization:

```text
Requested
InProgress(InterfaceId)
Complete(InterfaceId)
Failed { reserved_interface: InterfaceId? }
```

Allocating an ID at `InProgress` permits legal recursive graphs:

```ska
interface Chain<T> {
    fn next() -> shared Chain<T>;
}
```

The identical `Chain<Str>` key reuses its in-progress ID. Mutually recursive
class/interface signatures likewise close by publishing identities before
filling their declarations. Ordinary type and containment validation remains
responsible for whether each closed use is legal.

Generic transformation can request an infinite family, for example an
interface whose signature mentions `Expanding<T[]>`. The confirmed initial
guard matches generic classes: if the same generic template reappears on the
active cross-kind specialization path with a different argument sequence,
reject the application as non-terminating generic specialization. Identical
key re-entry is accepted. Failed keys remain cached so repeated uses do not
allocate new IDs or produce independently ordered diagnostic cascades.

Worklist order derives from module and declaration source order, requirement
order, type-tree order, and argument order. Hash iteration must not influence
identities, diagnostics, dumps, witness order, metadata, or symbols.

## GI11 — Modules, visibility, and name resolution

An interface template is a top-level declaration kind. It uses the existing
private-by-default visibility, `public` modifier, module qualification,
selective imports, ordinary imports, collision rules, and declaration
namespace.

The top-level symbol and resolved module declaration domains therefore gain
an `InterfaceTemplate(InterfaceTemplateId)` case. Lookup diagnoses whether a
name requires arguments, rejects arguments on non-generic declarations, and
reports wrong-kind uses in class bases, `implements` claims, and bounds.

Names written inside a template resolve in its definition module. This
includes ordinary declarations, nested class or interface templates, and
`where` targets. Closed type arguments resolve at the application site under
the caller's imports and visibility. Specialization substitutes resolved
identities and never repeats unqualified lookup in the application module.

Private declarations do not become accessible merely because a public
template mentions or specializes them. Existing public-surface validation and
whole-program module rules remain authoritative. Separate compilation,
package-distributed specialization ownership, and stable cross-package generic
ABI are deferred.

## GI12 — Compiler phase and IR boundaries

The target-independent pipeline remains:

```text
generic class/interface syntax
    -> template declarations, type terms, bounds, and selections
    -> coordinated closed semantic specialization
    -> resolved program whose ordinary executable declarations are closed
    -> typed HIR
    -> verified MIR
    -> backend
```

The template layer gains at least:

- `InterfaceTemplateId` and `InterfaceTemplateRequirementId` identities;
- generalized generic-template ownership for `TypeParameterId`;
- top-level interface-template symbols and module declarations;
- resolved interface-template declarations, parameters, requirements, bounds,
  and source spans;
- `InterfaceTemplate` terms in structural template types;
- parameter-bearing interface claims and bound terms;
- template bound selections that can retain template requirement identity;
- canonical interface specialization keys, states, provenance, and mappings;
  and
- a class/interface specialization coordinator with a cross-kind active path.

After specialization, the existing ordinary interface tables remain the only
input to type checking and HIR. Closed generated interfaces contain ordinary
resolved parameter and result types. Closed class claims contain ordinary
`InterfaceId` values. Bound-selected generated calls contain ordinary
`InterfaceRequirementId` values.

HIR and MIR need no type-parameter, interface-template, generic-application,
substitution, or dictionary variants. Their existing declaration-presence and
requirement-ownership verifiers continue to establish the lower-phase trust
boundary.

Shared infrastructure is extracted around template ownership,
structural substitution, application origins, deterministic scheduling,
recursion paths, and diagnostics. Class-specific lifecycle/body specialization
and interface-specific requirement specialization remain separate
owners rather than one large generic resolver module.

## GI13 — Target realization and ABI

Each closed interface application lowers as one ordinary interface identity.
For every conforming closed class, complete-object metadata records the exact
closed `InterfaceId` and its requirement-to-method witness mapping. Distinct
applications remain distinct metadata entries even if signatures or layouts
coincide.

Interface view representation, receiver passing, witness lookup, complete
object provenance, method ABI classification, checked casts, type tests, and
shared-owner behavior remain unchanged. The backend continues to choose
physical witness layout from ordinary MIR declarations.

Generated names and semantic dumps include the template's qualified identity
and canonical arguments so closed applications are distinguishable and stable.
Source spelling must not be used as the semantic cache key.

The initial feature adds no:

- runtime type-argument vector or descriptor;
- witness dictionary parameter on generic calls;
- erased generic-interface value;
- reflective query for generic arguments;
- runtime specialization or code generation; or
- public C runtime ABI operation or version change solely for generic
  interfaces.

## GI14 — Diagnostics, dumps, and testing

Diagnostics distinguish declaration, application, and conformance
failures. Important cases include:

- duplicate parameters or requirements in an interface template;
- raw generic interface names and wrong argument counts;
- arguments on non-generic interfaces and wrong declaration kinds;
- inaccessible templates or arguments;
- invalid parameter/result types after substitution;
- a nested class or interface application whose bound is not satisfied;
- an `implements` claim or `where` target that is not an interface;
- failure of an exact class to conform to an exact closed application;
- ambiguous same-named bound requirements;
- invalid casts or type tests between exact applications; and
- recursive specialization expansion, including the complete cross-kind path.

Application-dependent errors report both causes. For example,
`Producer<Readable>` points to `Readable` at the application and notes
that `Producer<T>.produce` uses `T` as an owning result.

Resolved dumps expose:

- interface templates, parameters, bounds, and template requirements;
- structural parameter-bearing interface applications;
- canonical closed application keys and assigned `InterfaceId` values;
- template-to-closed requirement mappings;
- class claims and conformance maps keyed by exact closed interface;
- bound selections before and after closure;
- application origins and specialization state transitions; and
- deterministic recursion paths for failures.

The validation matrix includes:

- parser and recovery coverage for declarations, nested closers, bounds,
  claims, casts, tests, and raw names;
- module visibility, qualification, aliases, collisions, arity, and wrong-kind
  resolution;
- canonical-key equality and distinction across primitive, class, interface,
  optional, array, shared, and nested generic arguments;
- substitution through parameter modes, results, functions, optionals, arrays,
  shared targets, nested class applications, and nested interface
  applications;
- contextual acceptance of bare interfaces in alias-only templates and
  rejection in owning result or storage positions;
- ordinary and generic classes conforming to closed applications;
- inherited conformance, overrides, receiver mutability, and exact signature
  mismatch;
- multiple applications of one template, marker interfaces, shared method
  witnesses, incompatible requirements, and duplicate claims;
- generic `where` bounds, definition-site selection, exact nominal checking,
  and ambiguous bound members;
- interface views, shared owners, ordinary and structural calls, casts, type
  tests, and produced results;
- identical recursion, mutual class/interface recursion, expanding recursion,
  cached failure, and output determinism; and
- resolved, HIR, MIR, verifier, x86-64, native golden, and runtime-ABI
  regression coverage.

Repository validation uses the supported `make docs-check`, focused
Rust test targets during implementation, and the documented golden/check
interfaces rather than ad hoc test runners.

## GI15 — Initial exclusions and future consumers

The initial generic-interface feature deliberately excludes:

- compiler-generated interfaces or conformances for primitive types;
- operator protocol definitions, operator overloading, or lowering primitive
  operators through interface calls;
- iterable, iterator, range, generator, or sequence standard protocols;
- `for`, `for ... in`, or any other new loop syntax or lowering;
- generic functions, generic methods, generic constructors, or requirement-
  local type parameters;
- type argument inference, defaults, placeholders, wildcards, raw types, or
  partial and explicit specialization;
- variance annotations, inferred variance, application subtyping, or
  interface-to-interface implicit conversion;
- associated types, higher-kinded parameters, existential applications,
  interface aliases, or same-type constraints;
- interface inheritance, default method bodies, fields, static requirements,
  lifecycle requirements, or explicit implementation blocks;
- interface method references or any new function-value behavior;
- method overloading or qualified disambiguation of colliding bound members;
- base-class, negative, disjunctive, structural, or user-defined capability
  bounds;
- treating primitive types, bare interface views, or shared-owner wrappers as
  satisfying nominal interface bounds without an independently designed
  conformance mechanism;
- erased generics, runtime dictionaries, reflection, or runtime
  specialization; and
- separate-compilation or stable package ABI for generic templates.

These exclusions are not judgments against later features. In particular, a
future standard library could define `Iterable<Item, State>`, and future
operator work could define `Add<Right, Result>`, using the exact generic
interface machinery confirmed here. Their compiler-provided conformances,
syntax lowering, optimization guarantees, and ergonomics require separate
designs after generic interfaces work end to end.

## GI16 — Promotion and roadmap boundary

GI1 through GI15 were confirmed as one coherent contract, including:

- closed `InterfaceId` specialization rather than dictionaries or erasure;
- interface-level `where` clauses;
- generalized parameter ownership and template requirement identities;
- exact invariant conformance and support for multiple closed applications;
- the coordinated class/interface recursion model; and
- the explicit future boundary around primitives, operators, and iteration.

Promotion added focused living language and compiler contracts, retained the
implemented grammar unchanged until syntax support lands, changed the status
matrix to frozen design, archived this proposal as the historical decision
record, and created an active
[implementation roadmap](../roadmaps/GENERIC_INTERFACES_ROADMAP.md).

The roadmap divides delivery into dependency-ordered PR-sized tasks. It must
not quietly reopen representation, ownership, conformance, dispatch, or ABI
decisions while scheduling implementation.

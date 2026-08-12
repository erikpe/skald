# Generic-Class Specialization

Status: frozen compiler design; syntax implemented, semantic specialization
not implemented. This document defines the target-independent compilation
contract for the initial [generic-class language design](../language/GENERIC_CLASSES.md).
The syntax AST now preserves generic declarations and closed applications;
resolution rejects them explicitly until the remaining phase products exist.

## Architectural outcome

Generic declarations are compile-time templates. The compiler specializes
every requested closed application before ordinary resolved classes enter type
checking and lowering. An accepted application receives one normal `ClassId`
and uses existing exact types, lifecycle analysis, HIR, MIR, verification, and
target lowering.

The core invariant is:

```text
generic syntax and template terms
    -> closed semantic specialization
    -> ordinary ResolvedProgram
    -> typed HIR
    -> verified MIR
    -> backend
```

No unresolved parameter reaches `ResolvedClassDeclaration`, ordinary HIR,
MIR, a verifier, or a backend. Generic classes add no erased storage, runtime
dictionary, type descriptor argument, reflective type list, or public runtime
ABI operation.

## Template and parameter identities

Declaration collection assigns each generic class a stable
`ClassTemplateId`. Its parameters receive ordered `TypeParameterId` values.
These identities belong to a template-specific semantic layer and are never
used where an executable `ClassId` is required.

A closed specialization key is conceptually:

```text
GenericClassInstanceKey {
    template: ClassTemplateId,
    arguments: [ResolvedTypeKind],
}
```

Arguments use canonical semantic identities. Array and optional IDs, shared
targets, primitive kinds, and already-closed nested class IDs participate in
the key; source spans, grouping, aliases of optional spelling, and display
text do not.

The specialization owner assigns one `ClassId` per unique key. Equivalent
requests reuse it; distinct template or argument identities remain distinct
even when layouts happen to match. An unrequested template has no ordinary
class table entry or executable artifact.

Specialization provenance maps every generated class back to its template,
canonical arguments, and application origins for dumps and diagnostics.
Executable consumers do not inspect this provenance to recover type or
ownership decisions.

## Template type terms and name resolution

The template layer retains structural types with parameter and generic-
application terms in addition to existing constructors:

```text
Parameter(TypeParameterId)
Named(declaration identity)
GenericClass(ClassTemplateId, arguments)
Shared(target)
Optional(payload)
Array(element)
```

This is separate from ordinary `ResolvedTypeKind`; adding a parameter variant
through resolved classes, HIR, and MIR violates the frozen boundary.

Nondependent names and interface bounds resolve once in the template's
definition module. Type arguments resolve at the application site before they
form a key. Specialization does not re-run unqualified lookup in the caller's
module.

Argument-dependent operations currently selected during resolution, such as
initializer overloads, casts, exact callable compatibility, and construction
heads, cannot receive placeholder identities. Template resolution retains a
name-resolved delayed selection or substitutes a definition-site-resolved body
before ordinary operation selection. Once a class is published into the
ordinary resolved program, every operation identity is concrete.

## Structural substitution and interning

Specialization recursively substitutes each parameter leaf with its complete
closed argument. It then constructs and interns compound types from children
to parents using the existing resolved type interner.

For `T?[]`, representative substitutions are:

```text
T = Str
Array(Optional(Str))

T = Str?
Array(Optional(Optional(Str)))

T = shared Interface
Array(Optional(Shared(Interface)))
```

The implementation does not rewrite or reparse source text. Optional layers
remain distinct, and substitution does not move a constructor across another
constructor. The ordinary resolved array, optional, optional-box, shared, and
class tables remain authoritative for closed code.

Substitution covers declarations and definitions: direct base, fields, static
fields and initializers, lifecycle members, parameters, results, nested
applications, casts, type tests, construction heads, and all other type-bearing
body nodes.

## Contextual requirements

Template validation records mechanical requirements over structural type terms
rather than assigning a global admissibility category to each parameter:

```text
GenericRequirement {
    type_term: GenericTypeTermId,
    capability: GenericCapability,
    origin: Span,
    reason: GenericRequirementReason,
}
```

The initial capability vocabulary preserves existing contextual owners:

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

Declaration contexts generate their corresponding requirements. Body
operation selection adds only capabilities actually required. Requirement
origins retain the exact source span and enclosing template construct.

After substitution, evaluation delegates to existing validators and lifecycle
planners. Optional default construction, recursive optional copy/assignment,
array element lifecycle, shared-owner retain/release, exact-class copy
capabilities, and stored/alias/shared-target eligibility remain single-sourced
in their ordinary phase owners.

The generic layer may expose a cohesive query facade over those owners, but it
must not implement parallel tables whose answers can diverge from non-generic
types.

## Nominal interface bounds

Class-level `where T: Interface` constraints are resolved at the definition
site and retained by interface identity. They authorize member selection only
through declared interface requirements and require each closed argument to be
an exact class with effective nominal conformance.

Bound-selected calls retain the interface requirement identity and ordinary
dispatch meaning. Generic specialization does not itself authorize
devirtualization. Multiple bounds are conjunctive, and ambiguous exposed
members are definition errors under the initial profile.

Inferred mechanical requirements and explicit nominal constraints form one
conjunctive effective contract. The compiler dumps both, but the source has no
written `stored`, `copy`, or assignment bound in the initial feature.

## Complete-class validation

The template layer rejects argument-independent errors before applications are
specialized. A requested closed application then validates every declaration,
body, lifecycle operation, base, and conformance belonging to the generated
class.

The initial compiler does not instantiate methods lazily or remove methods
whose requirements fail. Unavailable synthesized copy construction or
assignment remains ordinary capability metadata until an operation requires
it; complete-class validation does not turn absence alone into an error.

Every valid generated class enters the same ordinary validation order as a
hand-written class:

- optional and array eligibility;
- finite inline containment;
- hierarchy and override validity;
- interface conformance;
- exact-class and aggregate lifecycle capabilities;
- declarations and bodies; and
- HIR construction.

The precise orchestration may evolve, but no current validator may be bypassed
merely because its input originated in a template.

## Specialization discovery and recursion

Every explicit closed application in a checked program requests a
specialization. A parameter-bearing application inside a template requests one
when substitution first closes it. Generated fields, signatures, bases,
statics, and bodies may therefore add work to a deterministic specialization
queue.

The cache has conceptual states:

```text
Requested
InProgress(ClassId)
Complete(ClassId)
Failed
```

Allocating a `ClassId` at `InProgress` allows the identical key to close a
recursive graph. Existing containment rules then reject direct or indirect
infinite inline storage while permitting recursion across an owning
indirection boundary already supported by the language.

If the same `ClassTemplateId` reappears on the active specialization stack
with a different argument sequence, the initial implementation diagnoses
non-terminating specialization. This conservative rule rejects expanding
families such as `Expanding<T[]>` rather than attempting type-level
termination proofs.

Failed keys remain cached so repeated applications do not allocate new IDs or
emit independently ordered cascades. Queue traversal, ID assignment,
diagnostics, dumps, static planning, and emitted artifacts follow stable
module, declaration, member, and argument order rather than hash iteration.

## Closed class integration

A generated class uses ordinary member identities derived from its `ClassId`.
Its substituted base and fields feed the existing hierarchy, containment,
layout, lifecycle, and destruction machinery. Its methods, lifecycle bodies,
virtual families, interfaces, and call sites contain ordinary exact identities.

Each closed application owns independent static fields and static initializer
bodies. Static effects, dependency evidence, activation order, shutdown, and
backend storage treat those fields exactly like statics on hand-written
classes. A static selection requests specialization even if no value of the
class is otherwise constructed.

Generic inheritance produces a closed base before ordinary hierarchy
analysis. Generic classes may implement ordinary interfaces; conformance is
computed for every closed class. HIR and MIR dispatch remain class, virtual,
or interface dispatch over existing identities without runtime generic
witness dictionaries.

## Phase products and trust boundaries

The target-independent path becomes:

| Responsibility | Product and invariant |
|---|---|
| Syntax | Source-shaped parameters, applications, and `where` clauses with exact spans and recovery |
| Declaration collection | Ordinary symbols plus stable template/parameter identities; generic names are not prematurely assigned executable class identities |
| Template resolution | Definition-site names, structural type terms, nominal bounds, delayed argument-dependent selections, and inferred requirement origins |
| Specialization | Deterministic closed keys, substitution, requirement validation, complete ordinary resolved declarations/definitions, and provenance |
| Ordinary type checking | Existing validators and capability planners consume only closed types and classes |
| HIR | Existing typed operations and exact class/member identities; no parameter term |
| MIR and verification | Existing concrete layout-independent operations and lifecycle proofs; no generic instruction or runtime argument |
| Backend | Ordinary specialized layout and symbols; no runtime generic protocol |

The resolver facade remains responsible for returning either diagnostics or a
`ResolvedProgram` whose ordinary products satisfy their existing invariants.
Whether template specialization is exposed as a separately inspectable public
product is an implementation-roadmap choice, but deterministic template and
specialization dumps are required for review and debugging.

## Target and ABI realization

Each closed class lowers like an equivalent hand-written exact class. Layout
uses substituted fields and base; shared allocation metadata and finalization
use the generated class; methods, lifecycle operations, dispatch tables,
statics, and helpers have statically selected identities.

Private emitted symbols distinguish the template declaration and complete
argument identity through deterministic collision-free mangling. Human-facing
dumps render qualified semantic type names rather than mangles.

The initial implementation performs no semantic sharing between distinct
specializations. Later target optimization or linker folding may merge code
only when class identity, metadata, and observable behavior remain correct.

No C runtime entry point, allocation-header field, runtime ABI version change,
or external generic calling convention is introduced. Existing external ABI
eligibility applies to each generated exact class as it would to a hand-written
class.

## Diagnostics and dumps

Definition diagnostics own malformed parameter lists, duplicate parameters,
invalid bounds, inaccessible definition-site names, unconstrained operations,
and unsupported parameter construction. Application diagnostics own arity,
wrong-kind applications, failed nominal constraints, contextual eligibility,
lifecycle capability failures, and specialization recursion.

An application failure reports the application as primary and retains the
template requirement source as a note or secondary label. Nested applications
and constructors form an outer-to-inner path. Existing lifecycle field/base
failure paths remain nested beneath the generic origin.

Deterministic inspection includes:

- template and parameter identities;
- structural source type terms;
- explicit nominal and inferred contextual requirements;
- specialization keys, states, and generated `ClassId` values;
- parameter-to-argument mappings;
- substituted declarations and body selections; and
- application origins and recursion paths.

Ordinary resolved, HIR, MIR, and assembly dumps continue to show only closed
semantic identities and operations.

## Testing contract

Test ownership follows existing phase boundaries:

- lexer/parser tests own angle brackets, contextual `where`, nested closing
  angles, comparison/shift disambiguation, spans, and recovery;
- resolution tests own identities, scope, shadowing, arity, modules,
  visibility, canonical keys, substitution, caching, recursion, and dumps;
- type-checking tests own inferred contextual requirements, nominal bounds,
  complete-class validity, lifecycle selection, and diagnostics;
- HIR/MIR tests prove that specialized optionals, arrays, shared owners,
  inheritance, statics, calls, and cleanup use existing closed operations and
  that no parameter term survives;
- backend tests own deterministic mangling, independent static storage, exact
  layouts, dispatch metadata, and ordinary specialized emission; and
- golden tests own complete native behavior and byte-exact source diagnostics
  for representative primitive, inline, optional, shared exact, and shared
  interface-owner applications.

Cross-process determinism must cover specialization IDs, diagnostics, phase
dumps, assembly, and native observation. The active roadmap assigns focused
gates to each task and finishes with `make check`, `make msrv-check`,
`make robustness-long`, and `git diff --check`.

## Deliberate exclusions

The compiler contract does not provide generic functions, independent generic
methods or constructors, generic interfaces, member-level constraints,
inference, defaults, variance, partial specialization, parameter construction,
source-visible lifecycle bounds, lazy method validation, erased code, runtime
dictionaries, reflection, or separate-compilation specialization ownership.
Those require later language designs rather than implementation extensions to
this frozen profile.

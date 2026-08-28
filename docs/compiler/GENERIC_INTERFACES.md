# Generic-Interface Specialization

Status: implemented compiler contract. Resolution owns non-executable template
semantics and coordinated closed specialization; ordinary resolved IR, HIR,
verified MIR, and the x86-64 backend consume only closed exact interface and
requirement identities. This document defines the target-independent
compilation contract for the implemented
[generic-interface language](../language/GENERIC_INTERFACES.md). The
[status matrix](../language/STATUS.md) remains authoritative for compiler
availability.

Generic interface declarations are compile-time templates. Every requested
closed application is specialized into an ordinary exact interface before
typed HIR. Existing nominal conformance, interface views, complete-object
metadata, requirement calls, verified MIR, and backend witness dispatch then
operate on ordinary closed identities.

## Architectural outcome

The core pipeline invariant is:

```text
generic class/interface syntax
    -> template declarations, type terms, bounds, and selections
    -> coordinated closed semantic specialization
    -> resolved program with closed ordinary executable declarations
    -> typed HIR
    -> verified MIR
    -> backend
```

No unresolved parameter, interface-template application, or template
requirement identity reaches an ordinary resolved interface declaration, HIR,
MIR, verifier, or backend. Generic interfaces add no erased value,
type-argument descriptor, dictionary parameter, reflective metadata API, or
runtime specialization protocol.

The implementation extends the current generic-class template layer and
specialization boundary. Shared template infrastructure owns parameter
identity, structural substitution, application provenance, deterministic
scheduling, and recursion paths. Class-specific body/lifecycle work and
interface-specific requirement specialization remain separate cohesive owners
behind the resolution facade.

## Template and requirement identities

Declaration collection assigns each generic interface one stable
`InterfaceTemplateId`. It is non-executable and distinct from both
`ClassTemplateId` and `InterfaceId`.

Type-parameter identity is generalized around the declaration that owns it:

```text
GenericTemplateId =
    Class(ClassTemplateId)
    | Interface(InterfaceTemplateId)

TypeParameterId {
    owner: GenericTemplateId,
    index: source-order index,
}
```

This replaces the current assumption that every `TypeParameterId` is owned by
a `ClassTemplateId`. Class and interface parameter tables may remain strongly
typed while shared substitutions and diagnostics use the common owner.

Each generic-interface requirement receives a stable
`InterfaceTemplateRequirementId` containing its template owner and source-
order index. It supports definition-time lookup and bound-member selection
before a closed interface exists. It never substitutes for an ordinary
`InterfaceRequirementId` in executable IR.

Top-level symbol, module declaration, binding, import, qualification, and dump
vocabularies distinguish `InterfaceTemplateId` from ordinary interfaces and
class templates. Allocation follows canonical module and declaration order.

## Template semantic representation

The generic semantic layer retains interface template declarations containing:

- module, visibility, name, source spans, and ordered parameters;
- ordered template requirements with template requirement identities;
- structural parameter and result type terms with parameter modes;
- ordinary or parameter-bearing interface bounds;
- contextual type-use requirements and their origins; and
- definition-site member selections needed by enclosing generic class bodies.

The structural template type vocabulary gains:

```text
InterfaceTemplate {
    template: InterfaceTemplateId,
    arguments: [ResolvedTemplateType],
}
```

It composes recursively with parameter leaves, closed primitives, classes and
interfaces, generic class applications, functions, shared targets, optionals,
and arrays. It is separate from ordinary `ResolvedTypeKind`.

Generic class interface claims and bounds can no longer store only an
`InterfaceId`. Their template representation retains either an ordinary
interface or a parameter-bearing generic interface application until
substitution closes it.

The implemented specialization boundary turns every closed generic-interface
request into a span-free canonical key and reserves one `InterfaceId` before
closing its dependencies. Requests are deduplicated across signatures, claims,
bounds, aliases, shared targets, casts, tests, and nested generic arguments,
and keep all application origins in canonical module/source order. Complete
successful identities are published as ordinary
`ResolvedInterfaceDeclaration` entries. Ordinary closed type uses therefore
resolve to `ResolvedTypeKind::Interface`.

Nondependent names resolve once in the template's definition module. Type
arguments resolve at each application site before entering a canonical key.
Specialization never reparses source or repeats caller-relative name lookup.

## Contextual requirements and substitution

Interface templates reuse the generic-class contextual-requirement model.
Each parameter mode, parameter type, result type, nested class application,
nested interface application, and bound records the ordinary semantic
predicate it will require after substitution.

Substitution recursively replaces parameter leaves with complete closed
arguments and interns compound types bottom-up through existing type owners.
It preserves every optional, array, shared, and function layer exactly.

A closed application validates every requirement signature and declared
bound. It does not instantiate requirements lazily. Mechanical validity is
delegated to the existing stored-value, result, alias, optional, array,
shared-target, function-type, and lifecycle capability owners rather than
duplicated in generic-interface code.

For example, substituting a bare interface into an owning result fails the
ordinary result rule, while substituting it into a read-only alias target can
succeed. An unused marker parameter creates no artificial storage or lifecycle
requirement.

Definition-independent failures are diagnosed once on the template.
Application-dependent failures retain both the application origin and the
template type use or nested obligation that caused them.

## Closed application identities

A canonical closed interface key is:

```text
GenericInterfaceInstanceKey {
    template: InterfaceTemplateId,
    arguments: [ResolvedTypeKind],
}
```

Arguments use canonical semantic identities. Spans, grouping, import aliases,
display text, and equivalent shorthand are excluded from equality.

The first accepted request reserves one ordinary `InterfaceId`. Requirements
receive ordinary `InterfaceRequirementId` values from that closed owner and
their source-order indexes. Equivalent keys reuse those identities; distinct
templates or argument sequences remain distinct even when signatures or
layouts coincide.

The specialization table retains key, state transitions, template span,
ordered application origins, recursion path, and template-to-closed
requirement mapping. An unrequested interface template creates no ordinary
interface entry or emitted witness metadata.

## Coordinated specialization and recursion

Class and interface applications can discover each other through signatures,
claims, bounds, casts, type tests, shared targets, nested generic arguments,
and generated class declarations. One deterministic coordinator therefore
owns the cross-kind worklist and active path:

```text
GenericSpecializationKey =
    Class(GenericClassInstanceKey)
    | Interface(GenericInterfaceInstanceKey)
```

Interface specialization uses the conceptual states:

```text
Requested
InProgress(InterfaceId)
Complete(InterfaceId)
Failed { reserved_interface: InterfaceId }
```

Allocating the ordinary identity at `InProgress` closes identical recursive
and mutually recursive class/interface graphs. Materialization substitutes
each complete signature, interns compound types, assigns ordinary requirement
IDs by source index, and retains the template-to-closed requirement mapping.
Contextual validation then uses the existing ordinary capability and interface
signature checks. Because the ordinary interface table is dense, any failure
atomically suppresses the generated suffix for that compilation attempt while
retaining every reserved specialization as a cached failed identity.

If the same class or interface template reappears on the active cross-kind
path with a different argument sequence, specialization rejects the request as
non-terminating expansion. Identical-key re-entry reuses the in-progress
identity. Failed entries remain cached so repeated applications neither
allocate new identities nor reorder diagnostic cascades.

Worklist order derives from canonical module/declaration order, requirement or
member order, structural type-tree order, and argument order. Hash iteration
must not affect identities, diagnostics, dumps, conformance maps, witness
metadata, static effects, or target artifacts.

## Bounds and bound-member selection

A template bound retains its subject parameter plus an ordinary or structural
interface application. Closing the enclosing template substitutes the right
side, requests its interface specialization, and checks either exact nominal
conformance of an exact class argument or compiler-owned evidence for an exact
canonical primitive operator application. Class-template and interface-template
bounds share this query. Structurally distinct bounds that become the same
subject/application pair after substitution are rejected as duplicate closed
bounds without repeating the satisfaction-failure cascade.

The canonical-only primitive extension does not apply to other interfaces,
unsupported operator cells, foreign same-named templates, or applications
with different `Rhs` or `Output` arguments.

Definition-time member lookup through a parameter-bearing bound selects one
`InterfaceTemplateRequirementId`. Specialization closes the interface
application and maps that identity to the same-index ordinary
`InterfaceRequirementId`. Generated ordinary bodies record only the closed
requirement.

This mapping prevents application-dependent member reselection. It also
preserves ordinary interface dispatch: specialization does not rewrite a
bound call to a direct class call merely because the argument is known.
Ambiguity across multiple bounds is rejected before body specialization.

## Closed interface declarations and conformance

Interface specialization publishes one ordinary
`ResolvedInterfaceDeclaration` containing fully substituted ordinary
parameter and result types and ordinary requirement IDs. The existing
interface type checker remains authoritative for closed signature legality.

An ordinary class claim requests its closed interface immediately. A generic
class retains structural claims and closes them with each class application.
Class conformance runs only after the class hierarchy, effective methods, and
claimed interface requirements are complete.

The ordinary conformance algorithm checks exact name, arity, modes, parameter
types, result, receiver mutability, visibility, inheritance, and override
behavior. Its result maps each closed `InterfaceRequirementId` to one concrete
`MethodId`. Multiple applications of the same template remain separate
conformances and metadata entries. Existing method non-overloading and
duplicate exact-conformance rules apply unchanged.

Every successful resolved class declaration contains only ordinary exact
claims. Each generic-class specialization records its closed claims directly,
so two applications originating at the same template span cannot be confused.
Conformance diagnostics retain the claim, effective method, and originating
template requirement spans; resolved and HIR dumps expose the closed claim and
requirement-to-method identities.

Inherited conformance retains the exact closed `InterfaceId`. An override
replaces the inherited witness only when it continues to satisfy the exact
closed requirement.

## Ordinary type and object-model integration

After closure, `ResolvedTypeKind::Interface(InterfaceId)` and existing shared
targets represent generic interface applications. There is no generic
interface type variant in ordinary resolved declarations or lower IR.

The following existing paths consume the exact closed identity unchanged:

- `ref` and `mut ref` interface aliases and receiver access;
- class-to-interface and inherited views;
- owning `shared Interface` handles, optionals, arrays, fields, parameters,
  results, transfer, and cleanup;
- checked object-place and shared-owner casts;
- type tests and dynamic complete-object metadata queries;
- ordinary and structural interface calls; and
- produced owning results whose closed type is valid.

Bare interface applications remain non-owning and invalid in stored/result
positions that reject ordinary bare interfaces. Generic-interface support
does not add boxing, escaping references, method references, implicit owner
creation, or conversions between interface applications.

## HIR, MIR, verification, and target realization

Closed generated interfaces enter HIR through the existing interface
declaration and conformance tables. Calls retain ordinary
`InterfaceRequirementId` targets. HIR and MIR do not gain generic template,
argument, substitution, or dictionary nodes.

MIR verification continues to prove that every requirement belongs to a
declared closed interface, every conformance references declared classes and
methods, signatures match, and every interface call target is valid. Mutation
tests must independently reject template identities or undeclared closed
identities injected below resolution.

The backend emits each exact conformance in complete-object metadata and
chooses witness layout using ordinary closed MIR. Distinct applications remain
distinct metadata entries even when one method satisfies both. Interface view
representation, receiver passing, lookup, checked casts, shared ownership,
calling conventions, static effects, runtime traces, and symbols use their
existing contracts.

Generated semantic and target names include qualified template identity plus
canonical arguments. Generic interfaces add no public C runtime function,
runtime ABI version change, descriptor, dictionary, or reflective payload.

The implemented x86-64 path emits one ordinary complete-object metadata entry
per exact application. Distinct applications keep distinct witness slots even
when both slots contain the same concrete method address. Receiver/result
classification, shared retain/release, checked casts and tests, source trace
identity, and symbol determinism remain the ordinary interface contracts.

## Diagnostics, dumps, and verification strategy

Resolution diagnostics cover duplicate parameters and requirements, raw
names, arity, wrong kinds, visibility, invalid bounds, contextual signature
failure, failed conformance, ambiguous bound members, and recursive expansion.
Every application-dependent error retains ordered application and template
origins.

Inspectable products expose:

- interface templates, parameters, bounds, and template requirements;
- structural interface applications in signatures, claims, and bounds;
- canonical closed keys, states, origins, and assigned interface IDs;
- template-to-closed requirement mappings;
- exact class claims and conformance witness maps;
- bound selections before and after closure; and
- deterministic cross-kind recursion paths.

Testing is owned at each phase:

- syntax tests own source shape, punctuation spans, nested closers, contextual
  keywords, malformed recovery, and syntax dumps;
- resolution tests own identities, module behavior, structural terms,
  substitution, contextual validity, caches, recursion, diagnostics, dumps,
  bounds, and conformance;
- type-check and HIR tests own closed signature validation, calls, views,
  ownership, casts, tests, structural selection, and absence of template terms;
- MIR and verifier tests own declarations, witness targets, effects, ownership,
  mutation rejection, and deterministic lowering;
- backend tests own exact metadata, witness lookup, ABI classification,
  symbols, shared-owner behavior, and unchanged runtime ABI; and
- golden tests own multi-module native dispatch, multiple applications,
  ownership/lifecycle composition, checked failure, exact diagnostics, and
  independent-process determinism.

The [generic-interface conformance matrix](GENERIC_INTERFACES_TEST_MATRIX.md)
maps every language/compiler rule and deliberate exclusion to its primary
owner-local or source-to-native evidence.

The complete feature gate includes `make check`, `make msrv-check`,
`make robustness-long`, `make golden-determinism-test`, and
`git diff --check`.

The concrete phase-by-phase workflow and focused commands are documented in
[Debugging the Compiler](../development/DEBUGGING.md#follow-the-pipeline), and
test placement remains governed by [Testing](../development/TESTING.md#test-layers).

## Implemented operator-protocol specialization

The implemented [operator-protocol compiler contract](OPERATOR_OVERLOADING.md)
extends definition-site bound selection without changing ordinary generic-
interface identity. A canonical operator bound closes to either an ordinary
class witness or one compiler-owned primitive operation. The template records
one unique selected requirement; specialization maps it to an ordinary
interface call or existing primitive HIR operation and never reselects from
the concrete type.

Primitive evidence creates no closed object interface, witness metadata,
dictionary, cast, shared interface owner, or runtime representation. Multiple
applicable operator bounds remain an unranked definition-site error. Manual
bound calls and punctuation share the same specialization evidence.

Static primitive bound satisfaction and definition-site bound-call and
punctuation realization are implemented. Primitive applications use their
existing operation; class applications continue through the ordinary witness
path.

## Deliberate exclusions

This compiler contract does not itself provide object-level primitive conformances,
iteration protocols or loop lowering,
generic functions or methods, inference, defaults, variance, associated types,
interface inheritance, default methods, structural conformance, erased
generics, runtime dictionaries, reflection, runtime specialization, or stable
separate-compilation template ABI. The separately implemented
[general-iteration compiler contract](ITERATION.md) is an implemented consumer
of the closed-interface machinery. Operator protocols and their narrow
primitive-evidence exception are owned by the separately implemented contract
above. The frozen [generic-range compiler contract](RANGES.md) plans one
additional closed canonical successor realization for `u8`, `u64`, and `i64`;
it does not broaden ordinary primitive interface conformance.

The archived
[design record](../archive/GENERIC_INTERFACES_DESIGN_PROPOSAL.md) preserves the
confirmed decisions.

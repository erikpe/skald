# Operator-Protocol Lowering

Status: implemented operator-protocol compiler contract.
Resolution implements the canonical module, validation, identity product, and
complete non-generic class-operator selection. The executable pipeline erases
those selected applications to ordinary interface calls while retaining the
[implemented primitive operator
representation](PHASES_AND_IR.md#implemented-primitive-operator-representation)
for exact primitive operations. Overloaded `!=` wraps one selected `OpEq` call
in the existing exact boolean negation. Receiver carriers, result-capability
owners, MIR evaluation and cleanup, dispatch, static effects, target
retention, panic traces, and native lowering are integrated through their
ordinary call paths. The compiler-provided primitive registry and exact
canonical generic-bound satisfaction, definition-site generic selection, and
class-witness/primitive-intrinsic realization are implemented.
This document defines the complete compiler boundary for the implemented
[interface-based operator-overloading language contract](../language/OPERATOR_OVERLOADING.md).
Its ordinary-call produced primitive read-only alias prerequisite is
implemented independently below.

The feature is a semantic consumer of canonical generic interfaces and
existing primitive operations. Source punctuation is selected once before
typed HIR and then erased to either an existing primitive operation or an
ordinary interface call. It adds no operator-specific runtime dispatch.

## Canonical module and declaration identities

`std::ops` enters the request only through ordinary module reachability caused
by explicit protocol references, or when it is the selected entry. Operator
tokens do not create a `CompilerDependencyKind`, implicit import binding, or
late module-graph expansion. This differs deliberately from `for-in`, whose
syntax alone must acquire `std::iter`.

Once reachable, resolution validates the complete dependency-free canonical
bundle from the language contract exactly once. Validation checks module and
public declaration identity, interface arity and parameter order, requirement
name and order, receiver mutability, explicit parameter count, mode and type,
and result type. A replacement standard library must provide the same complete
bundle. Missing or ambiguous providers and cycles remain ordinary module-graph
failures at explicit import evidence; malformed canonical declarations fail
before typed HIR.

The validated product records exact `InterfaceTemplateId` and
`InterfaceTemplateRequirementId` values. Later phases never rediscover a
protocol by path, spelling, method name, or structural signature. Primitive-
only programs create no `std::ops` edge and continue to compile with
`--no-stdlib`.

The implemented resolved product is one fixed-order table keyed by
`CanonicalOperatorProtocol`. Each entry records its exact
`InterfaceTemplateId`, parameter identities classified as unary, predicate, or
binary, and its `InterfaceTemplateRequirementId`. Canonical names and shapes
are centralized in that key; consumers query the validated table and do not
repeat path, spelling, arity, or signature recognition. The product is
published only when all seventeen protocols validate, so no partial bundle can
reach later phases. Resolved dumps render the table in canonical protocol
order.

## Semantic selection

Syntax and resolved expressions retain source operator identity, unary or
binary shape, exact operator and operand spans, and evaluation order. Semantic
selection first consults the existing exact primitive matrix. A successful
primitive match immediately selects the existing primitive semantic operation
and does not require canonical declarations.

Otherwise resolution and type checking map the operator to one validated
protocol template, enumerate exact applications from the static left class,
closed generic class, canonical interface view, or definition-site generic
bounds, and deduplicate identical canonical applications. Applicability uses
the ordinary read-only alias relation for `Rhs`. Exactly one application must
remain. Selection never uses expected result type, conversion ranking,
exact-match preference, inheritance depth, or specialization arguments.

A resolved operator product retains the canonical protocol and an
identity-sorted zero, one, or many exact application candidates. Each candidate
retains:

- the source operator and operand spans and static types;
- the canonical template and template requirement;
- the exact selected closed application when already closed;
- the structural `Rhs` and `Output` terms for template selection;
- the class claim, inherited conformance, interface view, or generic-bound
  origin; and
- the primitive operation when the exact matrix won before protocol lookup.

Candidate evidence is sorted by canonical interface identity and stable source
origin. Type checking accepts exactly one candidate and immediately reuses the
ordinary interface-call checker, so completed HIR contains no operator-specific
class node. Zero candidates and multiple candidates remain distinct structured
diagnostics. Multiple applicable generic bounds fail at template definition;
specialization does not resolve their ambiguity.

## Primitive implementation evidence

The compiler owns one closed declarative mapping keyed by receiver primitive
type plus canonical closed operator-interface application. Each entry names
one already implemented target-independent primitive semantic operation:

```text
(u64, OpAdd<u64, u64>) -> AddU64
(u64, OpShiftLeft<u64, u64>) -> ShiftLeftU64
(f64, OpNeg<f64>) -> NegateF64
(u64, OpEq<u64>) -> EqualU64
(f64, OpLess<f64>) -> LessF64
```

The complete mapping is mechanically validated against the canonical bundle
and existing primitive matrix. There is no entry for an unsupported primitive
cell and source cannot add, override, or orphan one.

The implemented resolved registry contains sixty cells across `i64`, `u64`,
`u8`, `f64`, and `bool`. Its application key is derived from each semantic
operation, validation rejects missing, duplicate, unsupported, or inconsistent
cells, and resolved dumps render the registry in canonical order. Both class-
template and interface-template exact-bound validation query this same product.

Primitive evidence is a static bound-satisfaction and specialization fact,
not `ResolvedInterfaceType` object conformance. It never enters complete-object
metadata, witness tables, casts, interface views, shared ownership, reflection,
or dynamic dispatch. Ordinary non-operator bounds remain exact-class-only.

## Generic-bound specialization

Canonical operator bounds close to one of two semantic realization kinds:

```text
OperatorImplementation =
    ClassWitness { interface, requirement }
  | PrimitiveIntrinsic { operation }
```

The resolved representation retains this distinction until specialized body
checking chooses its HIR form. Definition-time operator
selection records the template requirement and structural operands. Closing a
class argument maps it to an ordinary exact interface and witness call;
closing a supported primitive maps it to the existing primitive operation.
No specialization searches concrete members or conformances again. Generated
bodies consult span-keyed closed selections: class witnesses reconstruct one
exact ordinary resolved interface call, while primitive intrinsics reconstruct
the corresponding ordinary resolved primitive expression. Typed HIR therefore
contains neither a structural type-parameter operation nor a protocol
placeholder.

Manual bound calls use the same selected evidence. A primitive-specialized
`left.op_add(right)` in a bound-authorized template becomes the corresponding
primitive HIR operation even though direct primitive member syntax remains
invalid.

## Produced primitive read-only aliases

The ordinary call checker implements produced primitive read-only alias
materialization independently of operator protocols. Any successfully checked compatible primitive value
expression may initialize one hidden caller-owned scalar temporary at its
ordinary argument position. The alias designates that temporary through the
unchanged internal alias ABI until the call completes; storage ends at the
enclosing full-expression boundary.

Existing primitive places continue to borrow directly. `mut ref` selection
continues to require an existing mutable place. HIR distinguishes direct
place borrowing from produced scalar storage and retains the checked
expression, exact type, and source span. MIR owns one bounded lifetime plan;
verification proves initialization
before alias use, liveness through the call, no mutation or escape, and one
storage end after result securing.

This independently useful ordinary-call prerequisite is implemented and
tested before protocol lowering consumes it.

## HIR erasure and MIR reuse

The implemented phase flow is:

```text
source operator
    -> source-shaped syntax and resolved operator selection
    -> exact primitive or canonical protocol realization
    -> existing primitive HIR operation OR ordinary HIR interface call
    -> existing MIR operation/call, ownership, and cleanup paths
```

Class realizations reuse `HirExpressionKind::InterfaceCall`, exact requirement
identity, receiver carrier, argument binding, result destination, static
effects, panic trace, body retention, and cleanup. Primitive realizations reuse
the selected existing unary, binary, division, shift, comparison, or boolean
operation. Overloaded `!=` emits one `OpEq` interface call and one existing
exact boolean negation after securing that result.

No unresolved protocol placeholder may reach HIR completion. There is no
`MirOverloadedOperator`, runtime operator dispatcher, operator witness kind,
backend lookup by source spelling, or operator-specific effect model. MIR
verification continues to validate the ordinary interface call or primitive
operation and additionally rejects any injected unresolved protocol or wrong
primitive-realization evidence.

## Evaluation, lifetimes, and effects

Lowering preserves ordinary eager-call order: secure the left receiver once,
then evaluate and secure the RHS once, bind the read-only alias, issue one
selected call, secure its result, and perform reverse full-expression cleanup.
Produced exact-class receivers and arguments use existing carriers; produced
primitive RHS values use the prerequisite scalar temporary. Unary operations
secure one receiver before the call.

Interface-call static effects, reachable target expansion, body retention,
panic traces, virtual replacement, shared anchors, checked views, produced
receiver cleanup, and result ownership remain the ordinary closed-world call
contracts. Primitive realizations keep exactly the existing evaluation,
failure, IEEE-754, and cleanup semantics. Transformations may inline or
devirtualize only without changing observable evaluation, calls, effects,
failure, result, or cleanup.

The implementation uses one resolved exact-interface receiver conversion for
bindings, groupings, checked casts, explicit shared dereference, and the final
unwrap of view-only shared optional boxes. Exact-class operators use the
ordinary complete-object receiver carrier. Selected operator expressions are
identified once as call-shaped producers for shared and optional result
owners; class, array, function, scalar, and specialized-generic results keep
their existing call-result paths. No result family has an operator-specific
ownership representation.

## Diagnostics, dumps, and determinism

Module/provider diagnostics precede canonical-bundle validation, which
precedes semantic operator and type-capability errors. Focused diagnostics
distinguish malformed protocols, unsupported operands, ambiguous applications,
RHS alias incompatibility, unsatisfied canonical bounds, invalid result
capabilities, missing primitive mapping cells, and ordinary conformance
failures. They retain operator and operand spans, static types, and ordered
claim or bound origins.

Resolved dumps expose canonical language-item identities and each selected
primitive or protocol application. Applications rejected only by read-only
RHS binding remain non-candidates but are dumped with their canonical identity
and ordered declaration origin. HIR dumps expose the resulting existing
operation or exact interface call; they need no enduring source-operator tag
after erasure. Import spans explain module reachability separately from
operator-selection evidence. Before erasure, type checking validates the
selected protocol, interface, requirement, RHS, and output mapping so malformed
injected resolved evidence becomes a focused diagnostic rather than reaching
an ordinary-call invariant.

Module, declaration, template, application, requirement, candidate,
specialization, witness, diagnostic, dump, static-effect, target, and artifact
order is independent of hash iteration, provider discovery, and source import
ordering.

## Backend, runtime, and validation boundary

The backend receives only existing verified primitive operations or ordinary
interface calls and metadata. It adds no calling convention, layout rule,
public symbol, runtime-managed value, allocation, reflection record, or runtime
ABI revision. Class calls use existing witness dispatch; primitive
specializations use the same instructions and checked control flow as direct
primitive syntax. The runtime never observes a protocol identity.

Hardening covers independent-process full-pipeline dumps under reordered source
creation and provider roots, preliminary and final MIR mutations, public-symbol
and runtime-reference snapshots, and bounded malformed/deep operator and
generic source generation. MIR mutations remain owned by the ordinary call,
primitive-operation, cleanup, target, metadata, and alias-lifetime verifiers;
there is no operator-specific MIR verifier or backend path.

The
[operator-overloading conformance matrix](OPERATOR_OVERLOADING_TEST_MATRIX.md)
maps canonical and replacement bundles, receiver and result families,
definition-site and primitive specialization, failures, cleanup, exclusions,
verified phase boundaries, artifacts, native behavior, and determinism to
their narrowest executable owners.

The archived [design record](../archive/OPERATOR_OVERLOADING_DESIGN_PROPOSAL.md)
preserves the alternatives and rationale.

The frozen [generic-range compiler contract](RANGES.md) reuses exact
`OpLess<T>` primitive and class realizations and plans a separate canonical
successor registry. Range syntax and fusion do not add an operator protocol or
change this lowering boundary.

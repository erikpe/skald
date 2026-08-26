# Operator Overloading Design Proposal

Status: draft for review. No decision in this document is frozen, promoted, or
scheduled for implementation. The directions marked **Recommended** form a
coherent initial design, while the questions marked **Open** are intended for
iteration before this proposal can be promoted into living language and
compiler contracts. An implementation roadmap must not be created from this
draft.

This proposal explores interface-based operator overloading now that generic
interfaces and general iteration are implemented. Its central direction is
that an eager source operator may select either an existing primitive
operation or one exact canonical generic-interface application. User classes
implement the canonical interfaces through ordinary public methods. Primitive
implementations are supplied by the compiler without boxing primitives or
inventing runtime primitive objects.

The proposal deliberately separates:

- source-visible operator selection and evaluation;
- the standard-library declarations that name operator protocols;
- compiler-provided primitive protocol implementations;
- generic-bound selection and specialization;
- representation prerequisites that are independently useful; and
- comparison, mutation, conversion, and optimization work that should not be
  smuggled into the initial feature.

The implemented primitive operator profile remains authoritative until this
design is frozen, promoted, and implemented. This draft does not make any
class operator valid source.

## Intended outcome

The eventual feature should make these forms coherent:

```ska
from std::ops import OpAdd;

class MyStr implements OpAdd<MyStr, MyStr> {
    private _value: Str;

    init(value: Str) {
        self._value = value;
    }

    fn op_add(ref rhs: MyStr) -> MyStr {
        return MyStr(self._value.concat(rhs._value));
    }
}

var greeting: MyStr = MyStr("hello ") + MyStr("world");
```

and:

```ska
from std::ops import OpAdd;

class Adder<T> where T: OpAdd<T, T> {
    init() {}

    fn add(left: T, right: T) -> T {
        return left + right;
    }
}

var adder: Adder<u64> = Adder<u64>();
var answer: u64 = adder.add(17u, 25u);
```

The first expression should execute one ordinary interface implementation on
`MyStr`. The second should accept the compiler-provided implementation of
`OpAdd<u64, u64>` and specialize the generic body to the existing intrinsic
wrapping `u64` addition. Neither case should allocate an iterator, box a
primitive, create an erased generic dictionary, or add a runtime operator
service.

## Current boundary and architectural evidence

Skald already has most of the reusable machinery:

- the complete current operator punctuation, precedence, associativity, and
  source-shaped unary/binary nodes are implemented;
- primitive operation selection belongs to type checking and reaches exact
  target-independent HIR and verified MIR operations;
- generic interfaces close every requested application to an ordinary exact
  `InterfaceId` and `InterfaceRequirementId` before HIR;
- generic class bodies select bound members at the definition site and retain
  ordinary interface dispatch after specialization;
- exact, inherited, produced, checked, shared-backed, and interface-view
  receivers already reach ordinary interface calls;
- produced exact-class read-only receivers and produced exact-class `ref`
  arguments already materialize caller-owned temporaries with full-expression
  cleanup; and
- `std::iter::Iterable<Item, State>` establishes a useful precedent: the
  protocol is declared in ordinary standard-library source, recognized by
  exact canonical identity and signature, and consumed by syntax without
  structural method discovery.

There are also material gaps:

- generic bounds currently accept only exact class arguments with declared
  nominal conformance, so a primitive such as `u64` cannot satisfy any bound;
- every bound-selected requirement call currently closes to object-interface
  dispatch, while a primitive has no object view or witness table;
- primitive `ref` arguments currently accept only an existing primitive
  binding or static field; literals, calls, arithmetic, casts, and primitive
  field reads cannot be materialized as read-only alias arguments;
- ordinary methods and interface requirements are not overloaded, so one
  class cannot currently supply several differently typed methods named
  `op_add`; and
- `std::lang::Equatable` already defines dynamic class equality through
  `equals(ref other: Obj)`, which is not the same contract as a statically
  typed `OpEq<Rhs>` protocol.

Niflheim has no interface-based operator-overloading design to reuse. Its
operator pipeline is useful evidence for retaining source operator identity
until semantic selection, but Skald's generic specialization, exact types,
inline class values, aliases, deterministic cleanup, and verified MIR remain
authoritative.

## Design principles

1. **Operators are semantic sugar over one selected implementation.** The
   parser does not expand an operator into source method syntax, and lower
   phases do not rediscover a protocol from method names.
2. **Primitive behavior does not regress.** Existing primitive operations keep
   their exact types, wrapping, failure, IEEE, short-circuit, HIR, MIR, and
   backend contracts.
3. **No implicit value conversion enters through overloading.** Candidate
   selection uses static operand types, may reuse only the access-preserving
   non-owning view compatibility already admitted by the protocol's `ref`
   parameter, and does not use the expected result type.
4. **Protocols are nominal and canonical.** Same-named user interfaces and
   same-named methods do not authorize operator syntax.
5. **Class operands avoid implicit copies.** Read-only receivers and `ref`
   right operands let inline values participate without requiring copy
   construction merely to perform an operation.
6. **Primitive protocol support is static.** It does not imply that a
   primitive can be converted to an interface view, stored behind
   `shared Interface`, cast as an object, or entered into witness metadata.
7. **Generic selection remains definition-site selection.** Specialization
   chooses how to realize an already selected protocol; it does not search the
   concrete type for a different meaning.
8. **Evaluation and cleanup remain ordinary call semantics.** Operand order,
   produced receiver lifetime, argument lifetime, result securing, and
   full-expression cleanup must not depend on whether `+` or `.op_add(...)`
   led to the call.
9. **The initial surface should be narrow enough to explain completely.**
   Equality, partial ordering, short-circuit logic, mutation, and conversion
   each carry contracts beyond simple eager dispatch and should be included
   only deliberately.

## Decision register

| ID | Question | Current direction | State |
|---|---|---|---|
| [OP1](#op1--protocol-ownership-and-canonical-identity) | Where do protocols live? | Ordinary declarations in canonical `std::ops`, validated and recognized by the compiler | **Recommended** |
| [OP2](#op2--protocol-shape-and-parameter-modes) | What is the interface shape? | One read-only receiver, a `ref` RHS for binary operators, and explicit generic output | **Recommended** |
| [OP3](#op3--initial-overloadable-operator-surface) | Which operators overload initially? | Eager algebraic operators first; equality and ordering require an explicit decision below | **Open** |
| [OP4](#op4--selection-and-ambiguity) | How is an implementation selected? | Built-in primitive match first; otherwise one canonical application from the left operand whose `ref Rhs` accepts the static RHS; no expected-result filtering | **Recommended**, specificity details open |
| [OP5](#op5--compiler-provided-primitive-implementations) | How do primitives implement protocols? | Compile-time implementation records mapped to existing primitive operations; no object conformance or witness | **Recommended** |
| [OP6](#op6--generic-bounds-and-definition-site-selection) | How do generic bounds compose? | Bounds may be satisfied by a class witness or canonical primitive implementation; selected uses specialize to the corresponding realization | **Recommended** |
| [OP7](#op7--evaluation-lifetimes-and-effects) | What are the evaluation rules? | Eager left-to-right call semantics with full-expression temporaries and a read-only receiver | **Recommended** |
| [OP8](#op8--primitive-read-only-alias-materialization) | Is primitive temporary materialization required? | Yes for arbitrary produced primitive expressions passed to protocol `ref`; `mut ref` remains place-only | **Recommended** |
| [OP9](#op9--compiler-phase-and-ir-boundaries) | Where is sugar erased? | Semantic selection before HIR; emit existing primitive operations or ordinary interface calls | **Recommended** |
| [OP10](#op10--inheritance-dispatch-and-manual-use) | How observable are protocols? | Ordinary public class implementations and interface dispatch; primitive implementations remain statically callable through canonical bounds | **Recommended** |
| [OP11](#op11--diagnostics-dependencies-and-determinism) | How are canonical declarations acquired and errors reported? | Typed compiler dependency on `std::ops`, exact validation, deterministic candidate evidence | **Recommended**, acquisition detail open |
| [OP12](#op12--promotion-and-roadmap-boundary) | When may implementation planning start? | Only after every open decision is settled and the complete contract is promoted | **Recommended** |

## Proposed standard-library surface

Under the recommended algebraic profile, a dependency-free `std::ops` module
would declare ordinary generic interfaces conceptually equivalent to:

```ska
public interface OpNeg<Output> {
    fn op_neg() -> Output;
}

public interface OpBitNot<Output> {
    fn op_bit_not() -> Output;
}

public interface OpAdd<Rhs, Output> {
    fn op_add(ref rhs: Rhs) -> Output;
}

public interface OpSub<Rhs, Output> {
    fn op_sub(ref rhs: Rhs) -> Output;
}

public interface OpMul<Rhs, Output> {
    fn op_mul(ref rhs: Rhs) -> Output;
}

public interface OpDiv<Rhs, Output> {
    fn op_div(ref rhs: Rhs) -> Output;
}

public interface OpRem<Rhs, Output> {
    fn op_rem(ref rhs: Rhs) -> Output;
}

public interface OpBitAnd<Rhs, Output> {
    fn op_bit_and(ref rhs: Rhs) -> Output;
}

public interface OpBitOr<Rhs, Output> {
    fn op_bit_or(ref rhs: Rhs) -> Output;
}

public interface OpBitXor<Rhs, Output> {
    fn op_bit_xor(ref rhs: Rhs) -> Output;
}

public interface OpShiftLeft<Rhs, Output> {
    fn op_shift_left(ref rhs: Rhs) -> Output;
}

public interface OpShiftRight<Rhs, Output> {
    fn op_shift_right(ref rhs: Rhs) -> Output;
}
```

The exact names remain open until OP1 through OP3 are confirmed. `OpShl` and
`OpShr` would be shorter but less immediately readable; `Add` would be
pleasant in source but less explicit beside ordinary domain interfaces. The
`Op` prefix is currently recommended because it makes compiler-recognized
language protocols visually distinct without adding a new namespace feature.

The protocol declarations are ordinary Skald source. A user class imports and
implements them normally. A lookalike interface in another module has no
operator meaning. Missing, inaccessible, or malformed canonical declarations
are diagnosed rather than replaced silently.

## OP1 — Protocol ownership and canonical identity

**Question:** Should operator interfaces be synthesized by the compiler or
declared in the standard library?

**Recommended direction:** Declare them in canonical `std::ops` source and
make the compiler recognize their exact module, visibility, generic arity,
requirement name, receiver mutability, parameter mode and type, and result
type. This mirrors `std::iter::Iterable`.

This division provides:

- source declarations that users can import, read, document, and implement;
- ordinary generic-interface identity and conformance for classes;
- one exact identity immune to same-named structural lookalikes;
- no hidden interface syntax or prelude;
- normal module visibility and specialization behavior; and
- compiler authority only where syntax and primitive implementation require
  it.

Compiler synthesis of the interfaces would avoid a module dependency but
would introduce declarations that source cannot own or inspect through the
ordinary module model. Pure library declarations would be insufficient
because operator syntax must recognize exact protocols and primitives cannot
declare class-style conformance. The canonical library-plus-compiler model is
therefore the recommended middle ground.

## OP2 — Protocol shape and parameter modes

**Question:** What do the generic arguments mean, and should the RHS be a
value or an alias?

**Recommended direction:** A binary protocol is parameterized as
`OpAdd<Rhs, Output>`. Its receiver is the ordinary implicit read-only
interface receiver, its one explicit operand is `ref rhs: Rhs`, and its result
is the owning `Output`. A unary protocol has only `Output`.

The `ref` RHS is important for inline class values:

- a value parameter would require an implicit copy before every operator
  call;
- non-copyable classes could not be right operands;
- the copy could add user-visible effects and cleanup unrelated to the
  operator implementation; and
- a read-only alias already has a call-bounded, non-escaping contract.

Produced exact-class RHS expressions already materialize safely for `ref`.
OP8 extends the same convenience to produced primitive expressions. A
protocol cannot use `mut ref` for its RHS, and its receiver requirement is not
`mut fn`. Operators therefore cannot replace either operand. A method may
still have ordinary effects allowed to a read-only receiver, such as reading
or mutating independently reachable shared state or calling other effectful
functions.

`Output` is explicit because Skald has no associated types. It may differ from
the left or right type, provided the closed interface signature permits it as
an ordinary result. Candidate selection must not infer or choose `Output` from
an expected destination type.

## OP3 — Initial overloadable operator surface

**Question:** Which existing source operators should consult a protocol when
the primitive matrix does not apply?

**Recommended initial core:** Include only eager algebraic operations whose
source meaning is one unary or binary call:

| Source | Canonical protocol | Primitive implementations |
|---|---|---|
| unary `-` | `OpNeg<Output>` | `i64 -> i64`, `f64 -> f64` |
| unary `~` | `OpBitNot<Output>` | each integer to itself |
| `+`, `-`, `*`, `/` | corresponding binary protocol | each existing exact numeric pair |
| `%` | `OpRem<Rhs, Output>` | each existing exact integer pair |
| `&`, `|`, `^` | corresponding bit protocol | each existing exact integer pair |
| `<<`, `>>` | corresponding shift protocol | each existing integer left type with `u64` RHS |

The primitive implementation set is exactly the existing primitive operator
matrix. Protocol support neither adds a primitive operation nor changes a
primitive result.

The following require explicit decisions before freeze:

- **Prefix `!`:** keeping it exact-`bool` preserves Skald's no-truthiness
  rule. An `OpNot<Output>` protocol is mechanically possible, but an arbitrary
  output would blur logical negation with the separate bitwise `~` operator.
- **Equality:** a typed `OpEq<Rhs>` could make `==` call `op_eq` and define
  `!=` as one negation of that result. Skald already has
  `std::lang::Equatable.equals(ref Obj)` for dynamic hash-key equality.
  Freezing a second equality protocol needs either a clear distinction and
  consistency rule or a migration of `Equatable` and `Map`.
- **Ordering:** a generic `Range<T>` ultimately needs an ordering bound. The
  choices include one interface with four boolean methods, four one-method
  interfaces, or a single partial-comparison result. The latter needs a
  public representation of less/equal/greater/unordered that does not yet
  exist. This should be settled with range design in mind rather than hidden
  inside `<` lowering.

`&&` and `||` are not overloadable. Their right operand is conditional and
cannot be represented by an ordinary eager interface call. Postfix optional
unwrap `!`, explicit shared dereference `*`, `is`, calls, member access,
indexing/slicing, casts, construction, and assignment retain their existing
specialized meanings. Compound assignment, increment, decrement, and range
syntax do not exist and are not reserved by this proposal.

## OP4 — Selection and ambiguity

**Question:** Given `left + right`, which implementation is selected?

**Recommended direction:** Selection uses this ordered semantic rule:

1. Evaluate neither operand during selection.
2. If the exact static primitive operand pair is in the implemented primitive
   matrix, select the existing primitive operation.
3. Otherwise map the operator to one canonical protocol template.
4. Enumerate exact effective applications supplied by the static left type or
   by its declared generic bounds.
5. Retain candidates whose `ref Rhs` can accept the static right source under
   ordinary read-only alias compatibility.
6. Require exactly one candidate and take its declared `Output` as the result
   type.

The expected result type does not filter candidates. There is no implicit
numeric cast, promotion, literal reinterpretation, owning copy, owner
dereference, optional unwrap, or user-defined conversion during selection. An
explicit cast may change an operand's static type before selection.

The current recommendation is that the protocol's declared `ref` mode retains
the same access-preserving class/interface/`Obj` view applicability as an
ordinary call. Otherwise `left.op_add(derived)` could be valid for
`ref rhs: Base` while `left + derived` failed despite selecting the same
requirement. If several applications eventually become applicable, an exact
`Rhs` match should outrank a non-owning view conversion; the remaining
most-specific view ranking or ambiguity rule is still open and should reuse an
existing callable-specificity relation rather than create an operator-only
one. Current method non-overloading makes this uncommon but must not be used as
a semantic shortcut.

Eligible left sources are initially:

- an exact class with direct or inherited conformance;
- a specialized generic class with a closed conformance;
- the exact canonical operator-interface view itself; and
- a generic type parameter through one exact declared bound.

Produced expressions use their static exact class normally. `Obj`, unrelated
interface views, shared handles without explicit dereference, optionals,
arrays, and function values do not gain structural operator lookup.

Zero candidates produce an unsupported-operator diagnostic. More than one
equally specific candidate produces an ambiguity diagnostic in canonical
interface-identity order. The current absence of method overloading means a
class will normally be able to satisfy only one application of a given
operator protocol, but the selection rule must not rely on that incidental
restriction.

## OP5 — Compiler-provided primitive implementations

**Question:** How can `u64` satisfy `OpAdd<u64, u64>` without becoming an
object?

**Recommended direction:** The compiler owns a closed, declarative mapping
from each supported primitive protocol application to one existing primitive
semantic operation. Conceptually:

```text
(canonical OpAdd, [u64, u64]) -> AddU64
(canonical OpShiftLeft, [u64, u64]) -> ShiftLeftU64
(canonical OpNeg, [f64]) -> NegateF64
```

The real key must use canonical template and exact type identities rather than
names. The mapping is validated against the installed canonical declaration
and the implemented primitive matrix.

This is static protocol implementation evidence, not ordinary object
conformance. It permits:

- satisfaction of a canonical operator bound;
- operator selection through that bound; and
- a bound-selected canonical requirement call to specialize to the same
  primitive operation.

It does not permit:

- `ref value: OpAdd<u64, u64>` to alias a `u64`;
- primitive-to-interface or interface-to-primitive casts;
- `shared OpAdd<u64, u64>` ownership of a primitive;
- primitive entries in complete-object metadata or witness tables;
- structural satisfaction of unrelated interfaces; or
- user declarations that add or replace primitive protocol implementations.

Unsupported primitive applications remain unsatisfied. For example, there is
no `OpRem<f64, f64>` merely because a protocol template exists.

## OP6 — Generic bounds and definition-site selection

**Question:** How does `where T: OpAdd<T, T>` work for both classes and
primitives?

**Recommended direction:** Extend bound satisfaction only for canonical
operator protocols. This applies uniformly when closing generic class bounds
and generic interface bounds. A closed bound may carry one of two
implementation kinds:

```text
OperatorImplementation =
    ClassWitness { interface, requirement }
  | PrimitiveIntrinsic { operation }
```

The exact Rust representation remains private, but the semantic distinction
must survive until the specialized body has selected its HIR form.

Inside a generic template, `left + right` is authorized and selected from the
declared bound at the definition site. The template records the selected
protocol template requirement and structural `Rhs`/`Output` terms. Closing
`Adder<MyNumber>` maps that selection to an ordinary exact interface call.
Closing `Adder<u64>` maps it to the existing `AddU64` HIR operation. Neither
specialization searches for a same-named method or a different conformance.

The same principle should apply when a generic body spells the canonical
bound requirement manually, such as `left.op_add(right)`. If a primitive can
satisfy the bound but the bound's own requirement becomes uncallable after
specialization, the bound would be internally inconsistent. Supporting this
manual form is therefore recommended, although direct primitive member syntax
such as `17u.op_add(2u)` remains invalid because primitives have no members.

Ordinary non-operator interface bounds remain exact-class-only. This proposal
does not generalize every interface to structural or primitive conformance.

## OP7 — Evaluation, lifetimes, and effects

**Question:** What observable execution does an overloaded operator have?

**Recommended direction:** An overloaded eager binary operation behaves as
one interface call with this order:

1. evaluate and secure the left receiver exactly once;
2. evaluate and secure the RHS exactly once;
3. bind the RHS through the protocol's read-only alias;
4. perform one selected interface call;
5. secure the owning or scalar result; and
6. clean completed temporaries in reverse completion order at the enclosing
   full-expression boundary.

A produced exact-class left operand uses the existing caller-owned produced
receiver temporary. A named receiver remains the same non-owning object. A
produced exact-class RHS uses the existing produced `ref` temporary; a
produced primitive RHS uses OP8. No operand is copied merely because operator
syntax is used.

For unary operators, the operand is evaluated and secured once as the
receiver before the call. Primitive implementations retain the existing
primitive evaluation and failure rules exactly.

The receiver is read-only and non-exclusive. Operator methods are not
implicitly pure, `const`, total, commutative, associative, or symmetric.
Ordinary calls, shared state, panic, allocation, and cleanup inside the method
remain observable. Optimization may devirtualize or inline only while
preserving those effects and the exact source order.

## OP8 — Primitive read-only alias materialization

**Question:** Is produced primitive storage still a prerequisite?

**Recommended direction:** Yes. With a `ref` RHS, these ordinary expressions
must work:

```ska
value + 1u
value + make_u64()
value + object.count
value + (u64) signed
value + (first * second)
```

The general extension should allow any successfully checked primitive value
expression to bind to a compatible read-only primitive `ref` parameter by
materializing the value once in caller-owned temporary scalar storage. The
temporary is initialized at the expression's ordinary argument position,
remains live through later argument effects and the complete call, and ends at
the enclosing full-expression boundary.

An existing compatible primitive place may continue to borrow directly.
`mut ref` remains restricted to an existing mutable place. The relaxation
does not create reference values, alias locals, escaping aliases, external
alias signatures, implicit conversions, or observable mutation of unnamed
storage.

This extension is independently useful for ordinary functions and methods and
can be implemented and documented before operator overloading. It is a strict
prerequisite for the recommended protocol shape, not an optional optimization.

## OP9 — Compiler phase and IR boundaries

**Question:** Which phases know that source used an overloaded operator?

**Recommended direction:** Preserve source operator identity through syntax
and resolved template analysis, select semantics once, and erase the sugar
before typed HIR:

```text
source operator
    -> source-shaped syntax and resolved operator
    -> exact primitive or canonical protocol selection
    -> primitive HIR operation OR ordinary HIR interface call
    -> existing MIR operation/call and cleanup paths
```

Generic template semantics need a definition-site operator-selection record
analogous to existing bound-member and iteration selections. Specialization
closes that record to either class-witness identities or a primitive intrinsic
implementation before body checking completes.

For a class implementation, HIR should reuse the ordinary
`HirExpressionKind::InterfaceCall`, receiver carriers, call arguments, result
destinations, static effects, panic traces, and cleanup. For a primitive
implementation, HIR should reuse the exact existing unary, binary, checked
division, checked shift, comparison, or other selected primitive operation.
There should be no `MirOverloadedOperator`, runtime operator dispatcher, or
backend lookup by source spelling.

Resolved and HIR dumps must expose enough selected protocol or primitive
evidence to explain the choice deterministically. Once an overloaded operator
has become an ordinary HIR interface call, preserving an additional operator
tag below HIR is not required unless diagnostics or inspection demonstrate a
concrete need.

MIR verification continues to prove ordinary interface calls and primitive
operations. It should additionally reject any unresolved protocol or
primitive-implementation placeholder reaching MIR. Static-effect inference
already expands interface calls through closed-world witness targets and
should require no operator-specific effect model.

## OP10 — Inheritance, dispatch, and manual use

**Question:** Does operator syntax bypass ordinary interface behavior?

**Recommended direction:** No. A class implementation is an ordinary public
instance method satisfying an exact ordinary interface application. Existing
rules govern exact signature matching, inherited conformance, override
replacement, receiver access, produced receivers, checked views, shared
anchors, and interface witness dispatch.

An inherited exact conformance remains available to a derived left operand and
retains its declared `Rhs` and `Output`. An override that continues to satisfy
the requirement updates the ordinary effective witness. Operator syntax does
not select private or same-named structural methods.

Users may call the public implementation method directly on class values.
Generic code may call it through a declared bound as described by OP6.
Primitive implementations have no direct primitive member syntax and are
observable only through primitive operators and canonical generic bounds.

Method overloading is not a prerequisite for the first useful version. Its
absence means one class will generally implement at most one differently typed
application of each operator protocol because all applications require the
same method name. Supporting `Vector + Vector` and `Vector + Scalar` on one
class will eventually require ordinary method overloading, explicit interface
implementation bodies, or another independently designed conformance
mechanism. Operator selection must report the current limitation honestly and
must not introduce operator-only hidden method overloads.

## OP11 — Diagnostics, dependencies, and determinism

**Question:** How does a source operator acquire `std::ops`, and what evidence
does selection retain?

**Recommended direction:** Operator protocols use the same request-local
canonical-language-item discipline as general iteration. A valid use records
typed compiler-dependency evidence without creating a source import binding.
Explicit imports remain required to name a protocol in `implements` or
`where`, while `left + right` does not require an otherwise unused import.

The acquisition timing remains open. The simplest rule is that every syntax
use of an overloadable operator requests `std::ops`, even when type checking
later selects a primitive operation. A more selective rule would avoid that
dependency for primitive-only programs but would require a safe late module
acquisition boundary that the current graph pipeline does not have. The draft
currently recommends the simple syntax-evidence rule unless inspection finds
a meaningful build or cyclic-dependency cost.

Diagnostics should distinguish:

- invalid or missing canonical protocol declarations;
- no built-in or protocol implementation for the operand types;
- ambiguous exact protocol applications;
- an exact RHS mismatch;
- an unsatisfied generic operator bound;
- an unavailable `Output` capability in its consuming context;
- a primitive protocol application that is not in the compiler matrix; and
- ordinary conformance failures in a user class.

Selection diagnostics retain the operator span, both operand spans and static
types, the selected or rejected application, and candidate claim/bound spans.
Candidate, specialization, diagnostic, dump, witness, effect, and target order
must remain independent of hash iteration and module discovery order.

## OP12 — Promotion and roadmap boundary

**Question:** When is this proposal ready to become implementation work?

**Recommended direction:** Do not create an implementation roadmap until:

- OP1 through OP12 have confirmed decisions;
- the exact canonical interfaces and initial operator table are fixed;
- equality and ordering are either designed or explicitly excluded from the
  initial frozen profile;
- primitive bound satisfaction and manual bound-call behavior are coherent;
- primitive read-only alias materialization is frozen or already implemented;
- canonical module acquisition is settled;
- generic specialization, class dispatch, temporary lifetime, static effects,
  dumps, diagnostics, and verifier obligations have been audited; and
- the complete accepted design is promoted into living language and compiler
  documentation.

Promotion should update at least:

- the operator and exact-selection contract in
  [Types, Values, and Expressions](../language/TYPES_AND_VALUES.md);
- generic bound and protocol behavior in
  [Generic Interfaces](../language/GENERIC_INTERFACES.md);
- primitive alias materialization in
  [Aliases and Ownership](../language/ALIASES_AND_OWNERSHIP.md);
- evaluation order and temporary lifetime in
  [Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md);
- interface conformance and dispatch in
  [Polymorphism](../language/POLYMORPHISM.md);
- feature maturity in the [status matrix](../language/STATUS.md);
- specialization and implementation evidence in the
  [generic-interface compiler contract](../compiler/GENERIC_INTERFACES.md);
- operation, call, HIR, MIR, verification, effects, and dump boundaries in
  [Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md);
- standard-library ownership in [`std/README.md`](../../std/README.md); and
- the relevant language/compiler documentation indexes.

Only then should a roadmap divide the work into reviewable vertical tasks.
Likely boundaries include primitive read-only alias materialization, canonical
protocol declarations and validation, primitive implementation evidence and
bound closure, class operator selection, generic definition-site selection,
and complete hardening. Those are design observations, not a frozen sequence.

## Equality and ordering design fork

The initial proposal should not claim that all punctuation is overloadable
until these contracts are chosen.

### Equality alternatives

1. **Keep `==` primitive-only initially.** This avoids conflict and lets the
   algebraic feature proceed, but generic class equality and a future unified
   equality story remain separate work.
2. **Add `OpEq<Rhs>`.** `==` calls `op_eq(ref rhs: Rhs) -> bool`; `!=` calls it
   once and negates the result. This is statically typed and composes with
   primitive bounds, but it overlaps the existing dynamic `Equatable`
   contract.
3. **Reuse `Equatable`.** This aligns operators with current maps and strings,
   but its `Obj` RHS is class-oriented, dynamically heterogeneous, and cannot
   coherently describe primitive bound calls without further redesign.

The current recommendation is alternative 1 until a migration or explicit
separation between dynamic hash-key equality and typed operator equality is
accepted.

### Ordering alternatives

1. **Keep ordering primitive-only initially.** A later range design may then
   add the minimum coherent ordering protocol before generic `Range<T>`.
2. **One `OpOrder<Rhs>` with four boolean methods.** This is direct and
   represents unordered `f64` naturally through four false results, but every
   implementation must provide all four operations and consistency is a user
   obligation.
3. **Four one-method protocols.** This permits narrow bounds such as the `<`
   operation needed by a range, but proliferates canonical interfaces and can
   produce inconsistent partial implementations.
4. **One partial-comparison method.** This is cohesive and evaluates once, but
   it requires a public less/equal/greater/unordered result design and
   corresponding lowering that Skald does not currently have.

The current recommendation is to compare alternatives 2 and 4 during review.
Ordering should be settled before claiming that a generic `Range<T>` can use
only these operator protocols.

## Alternatives rejected by the current baseline

### Special class methods without interfaces

Selecting any method named `op_add` would be structural, would bypass nominal
generic bounds, and would make same-named domain methods silently acquire
language meaning. Canonical interfaces provide explicit opt-in and exact
identity.

### Compiler-only hidden protocols

Hidden declarations simplify bootstrapping but make the source contract
uninspectable and split class conformance from ordinary generic interfaces.
The standard-library declaration plus compiler recognition model already works
for iteration.

### Value RHS parameters

Value parameters force class copies, exclude non-copyable RHS classes, and add
effects unrelated to the operator body. Read-only aliases plus safe temporary
materialization fit Skald's value and lifetime model better.

### Boxing primitives into interface objects

Boxing changes allocation, identity, lifetime, dispatch, and performance. It
is unnecessary in a monomorphizing compiler that already has exact primitive
HIR operations.

### Parser expansion to method calls

The parser has no types, canonical interface identities, generic-bound
selection, primitive implementation mapping, or lifecycle plans. Expansion
there would lose the operator's diagnostic boundary and force later phases to
reverse-engineer semantics from a method name.

### Expected-result-directed selection

Choosing `OpAdd<Rhs, Output>` from the destination type would make the same
subexpression change meaning by context, complicate diagnostics and generic
selection, and conflict with Skald's exact-type operator model. `Output` is a
result of selection, not an input to it.

### Operator-only method overloads

Hidden overloads would create a second member and dispatch system used only by
punctuation. Broader method overloading or explicit interface implementations
can be designed independently if real programs require multiple RHS types.

## Deliberate exclusions

Unless an open decision above is resolved differently, the initial feature
does not include:

- short-circuit `&&` or `||` protocols;
- truthiness or overloaded conditional conversion;
- postfix optional unwrap, explicit shared dereference, casts, type tests,
  indexing, slicing, calls, construction, or assignment protocols;
- compound assignment, increment, decrement, assignment expressions, or
  implicit mutation of operands;
- implicit casts, numeric promotions, contextual literals, conversion
  ranking, or expected-result selection;
- user-defined primitive implementations or orphan implementations;
- primitive interface views, boxing, witness metadata, dynamic casts, shared
  ownership, or reflection;
- structural same-named method discovery;
- method overloading, qualified bound-member disambiguation, associated types,
  default methods, interface inheritance, or generic methods;
- guaranteed devirtualization, inlining, constant folding, vectorization, or
  zero-cost abstraction claims;
- range syntax, `Range<T>`, iteration changes, generators, or numeric-step
  semantics; and
- new runtime ABI symbols or runtime-owned operator behavior.

A later `Range<T>` can consume confirmed addition and ordering protocols and
implement the already established `Iterable<T, T>` contract. Neither range
construction nor `for-in` lowering needs to be part of operator overloading.

## Test obligations after promotion

An eventual roadmap should allocate evidence to each owner, including:

- canonical `std::ops` declaration acquisition, validation, malformed forms,
  cycles, and same-named lookalikes;
- class conformance, inheritance, overrides, private/static/mutable mismatch,
  and unsupported multiple applications;
- exact operand/result selection, mixed-type rejection, ambiguity, and absence
  of expected-result filtering;
- arbitrary produced primitive RHS aliases and continued place-only
  `mut ref` behavior;
- produced/named/checked/shared-backed class receivers and RHS values;
- left-to-right effects, exactly-once evaluation, result securing, reverse
  cleanup, panic, and return composition;
- primitive implementation matrices and rejection of every unsupported cell;
- class and primitive satisfaction of the same generic bound;
- definition-site selection, nested specializations, recursion, duplicate
  bounds, ambiguous bounds, and manual bound requirement calls;
- resolved and HIR dumps showing exact selected evidence;
- MIR/verifier mutation tests preventing unresolved protocols, wrong witness
  identities, wrong primitive operations, bad aliases, or missing cleanup;
- static-effect and body-retention parity with ordinary interface calls;
- target legality, deterministic assembly, native execution, and primitive
  behavior parity with pre-feature operators;
- no primitive boxing, witness metadata, runtime symbol, or ABI-version
  change; and
- independent-process determinism under reordered providers and equivalent
  source discovery.

Every implementation task should run proportionate focused checks and the
repository's complete `make check` gate. Changes to Rust targets, manifests,
or supported syntax also require the documented supported-toolchain gate.

## Decisions required before freeze

- [ ] Confirm canonical module, interface, parameter, and requirement names.
- [ ] Confirm library declaration plus compiler recognition rather than fully
      compiler-synthesized protocols.
- [ ] Confirm read-only receiver, `ref` RHS, and explicit `Output` parameters.
- [ ] Confirm the initial overloadable operator table.
- [ ] Decide prefix `!`, equality, inequality, and ordering behavior.
- [ ] Confirm ordinary read-only alias applicability, exact-match preference,
      remaining specificity, and no expected-result filtering.
- [ ] Confirm candidate behavior for inherited claims, exact interface views,
      generic bounds, and ambiguity.
- [ ] Confirm compiler-provided primitive implementation semantics and exact
      primitive matrix.
- [ ] Confirm that primitive protocol evidence is static and creates no object
      interface view or witness.
- [ ] Confirm primitive satisfaction of canonical operator bounds and manual
      bound-requirement call behavior.
- [ ] Confirm left-to-right call-equivalent evaluation, effects, result
      securing, and temporary cleanup.
- [ ] Freeze or implement produced primitive read-only alias materialization.
- [ ] Confirm the semantic-selection and HIR erasure boundary.
- [ ] Confirm the current no-method-overloading limitation and future
      extension boundary.
- [ ] Confirm canonical module acquisition timing and dependency evidence.
- [ ] Audit generic specialization, interface dispatch, checked and produced
      receivers, shared anchors, static effects, panic traces, dumps,
      determinism, verifier obligations, backend legality, and runtime ABI.
- [ ] Promote every accepted decision into living language and compiler
      contracts without claiming implementation.
- [ ] Validate links and indexes, archive the frozen proposal, and only then
      create an implementation roadmap.

## Promotion criteria

This proposal may be frozen and archived only when:

- every decision in OP1 through OP12 is confirmed;
- every item under
  [Decisions required before freeze](#decisions-required-before-freeze) is
  complete;
- equality and ordering are either fully specified or explicitly excluded;
- examples, protocol declarations, generic bounds, and primitive behavior all
  agree;
- representation prerequisites and core feature work are distinguished;
- living documentation contains the complete authoritative contract without
  requiring this proposal;
- the status matrix labels the promoted feature as frozen rather than
  implemented;
- no implementation roadmap depends on an open representation choice; and
- documentation links, indexes, and terminology validate cleanly.

After promotion, this file becomes the historical decision record under
`docs/archive/`. A new roadmap may then schedule implementation from the
promoted living contracts.

# Indexed Array Construction Design Proposal

Status: frozen, promoted, validated, and archived on 2026-09-06. IAC1 through
IAC11 adopt the confirmed decisions below. Living language and compiler
documentation own the frozen direction; this document preserves the decision
record while the active
[indexed array construction implementation roadmap](../roadmaps/INDEXED_ARRAY_CONSTRUCTION_ROADMAP.md)
owns delivery.

This proposal defines a destination-typed array construction form whose length
is known before allocation and whose elements are initialized by repeatedly
evaluating one index-bound expression. It closes the gap between dynamic
default construction and fixed source element lists without weakening Skald's
exact types, deterministic ownership, or unpublished-backing protocol.

The motivating library operation is:

```ska
fn to_array() -> T[] {
    return T[](
        self._length;
        index => self._storage[index]!
    );
}
```

This must work for every existing valid `Vec<T>` element family, including
non-default-constructible exact classes and non-null shared owners of exact
classes, interfaces, and `Obj`. The design therefore cannot mean default
construction followed by assignment.

## Intended outcome

The frozen feature will provide:

- explicit inline `T[](length; index => expression)` construction;
- symmetric shared-outer `new T[](length; index => expression)` construction;
- one immutable `i64` index binding scoped to the element expression;
- one length evaluation, one checked allocation, and increasing-index element
  evaluation;
- direct initialization of each previously uninitialized slot through the
  ordinary destination-typed owning rules;
- no default-construction or copy-assignment requirement merely because the
  result has dynamic length;
- a bounded per-element temporary and cleanup epoch;
- verified dynamic initialized-prefix and publication semantics;
- an ordinary-library implementation of `Vec<T>.to_array()` with no compiler
  knowledge of `Vec`; and
- no public runtime ABI addition.

Confirmation freezes a language and compiler contract; it does not make the
syntax executable. The status matrix distinguishes that frozen direction from
current availability, and the implemented grammar remains unchanged until the
implementation roadmap activates the syntax.

## Current boundary and motivating evidence

Skald currently supports:

```ska
T[]()
T[](length)
T[](copy source)
T[]{element0, element1}

new T[]()
new T[](length)
new T[](copy source)
new T[]{element0, element1}
```

`T[](length)` and `new T[](length)` default-initialize every element. They
therefore require a default plan for `T`. Element lists instead allocate
unpublished backing and directly initialize each listed position, so they can
construct arrays of non-defaultable element types.

The existing `Vec<T>` uses private `T?[]` capacity storage. Outer absence marks
an unused slot and permits vectors of types that cannot be default-constructed.
A direct library implementation like this is consequently invalid:

```ska
fn to_array() -> T[] {
    var result: T[] = T[](self._length);
    for (index in 0u .. self._length) {
        result[(i64) index] = self._storage[(i64) index]!;
    }
    return result;
}
```

Generic applications validate the complete specialized class. The
`T[](self._length)` operation adds a default-construction requirement even
when `to_array` is never called and even though every live output slot is later
assigned. In the current standard-vector matrix this rejects existing valid
applications such as `Vec<Item>`, `Vec<shared Item>`,
`Vec<shared Readable>`, and `Vec<shared Obj>`.

Adding a zero-argument initializer to selected exact classes would not solve
the semantic problem. Polymorphic shared targets have no exact allocation
class to default, and observable default initialization followed by assignment
would perform unnecessary lifecycle effects.

The implemented
[explicit element-list construction](../language/ARRAYS.md#explicit-element-list-construction)
already supplies the relevant static-length foundation: exact destination
typing, allocation before element effects, direct initialization, an
unpublished initialized prefix, complete publication, and category-specific
ownership plans. Indexed construction generalizes that protocol to a dynamic
length and canonical loop without adding a mutable array state.

## Niflheim precedent

Niflheim's generated primitive vector classes implement `to_array()` by
returning a slice of primitive `T[]` backing. That establishes the useful
surface expectation that conversion returns an independent exact-length array
containing only the live prefix.

The implementation does not transfer to Skald's generic vector. Niflheim's
classes are separately generated for defaultable primitive types and keep
`T[]` capacity storage. Skald's one generic `Vec<T>` admits ownership families
that require optional occupancy storage and direct initialization. Skald's
exact lifecycle, generic validation, and verified publication rules are
therefore authoritative here.

## Design principles

1. **Construction initializes; it never fills live placeholders.** Each
   expression result initializes one new slot directly.
2. **The result type remains explicit.** The construction spelling determines
   the exact invariant array type before checking the expression.
3. **Length is known before element effects.** One checked allocation can
   precede every generated element.
4. **The binding is syntax-local, not a closure.** The feature adds no callable
   value, capture object, generic function, or escaping environment.
5. **Existing owning rules compose.** Named sources copy, produced values may
   initialize or transfer directly, and shared owners retain or transfer under
   their ordinary provenance rules.
6. **Temporary storage stays bounded.** Each element evaluation and its
   cleanup complete before the next index begins.
7. **Partial backing is never source-visible.** Only a complete initialized
   prefix equal to the requested length may be published.
8. **`Vec` remains ordinary library code.** The compiler recognizes array
   construction, not a standard-vector class or method name.
9. **The first form is indexed, not iterable.** Unknown-length collection,
   filtering, flattening, and general comprehension syntax remain separate.

## Decision register

| ID | Decision | Recommended direction | State |
|---|---|---|---|
| [IAC1](#iac1--source-form) | Source form | Add `T[](length; index => expression)` and its shared-outer counterpart | **Confirmed** |
| [IAC2](#iac2--grammar-and-punctuation) | Grammar | Use `;` to separate length from generation and add `=>` as one punctuation token | **Confirmed** |
| [IAC3](#iac3--index-binding) | Index binding | Introduce one immutable lexical `i64` binding scoped only to the element expression | **Confirmed** |
| [IAC4](#iac4--type-selection-and-compatibility) | Type checking | Select explicit `T` first and check the expression as direct owning initialization of `T` | **Confirmed** |
| [IAC5](#iac5--evaluation-order-and-observable-effects) | Evaluation order | Evaluate length once, allocate once, then evaluate indices in increasing order | **Confirmed** |
| [IAC6](#iac6--element-ownership-and-lifecycle) | Ownership | Reuse the complete element-list initialization categories without defaulting or assignment | **Confirmed** |
| [IAC7](#iac7--temporary-lifetime-and-control-flow) | Dynamic epoch | Clean each element's non-transferred temporaries before beginning the next element | **Confirmed** |
| [IAC8](#iac8--generic-requirements) | Generic requirements | Infer only requirements selected by length, expression, destination initialization, and completed-array use | **Confirmed** |
| [IAC9](#iac9--partial-construction-publication-and-failure) | Partial construction | Track a dynamic initialized prefix and publish only after it equals the checked length | **Confirmed** |
| [IAC10](#iac10--compiler-representation-and-runtime-boundary) | Compiler boundary | Preserve a typed indexed mode through HIR and verified MIR; add no runtime ABI service | **Confirmed** |
| [IAC11](#iac11--standard-vector-adoption-and-delivery) | Adoption | Implement and document `Vec<T>.to_array()` only after the general form is executable | **Confirmed** |

## Confirmed source surface

### Inline construction

```ska
var squares: i64[] = i64[](10u; index => index * index);

var copies: Item[] =
    Item[](items.len(); index => items[index]);
```

The first expression produces ten elements for indices `0` through `9`. The
second directly copy-constructs each `Item` slot from the corresponding named
source. Neither construction asks `i64` or `Item` for a placeholder value.

### Shared outer construction

Leading `new` retains its existing meaning: it allocates and returns one
shared owner of the complete outer array.

```ska
var owners: shared (shared Item)[] = new (shared Item)[](
    count;
    index => new Item(index)
);
```

This produces one shared outer array whose elements are distinct shared
owners. Parentheses in the element type retain their existing ownership
meaning. Indexed construction adds no array covariance or inferred common
supertype.

### Optional and nested elements

```ska
var optional_values: i64?[] = i64?[](
    count;
    index => if_value(index)
);

var rows: i64[][] = i64[][](
    row_count;
    row => build_row(row)
);
```

The expression must already produce a value compatible with the exact stored
element type. Optional injection and nested-array named-copy or
produced-adoption behavior remain ordinary destination initialization.

### Standard-vector conversion

The intended library method is:

```ska
fn to_array() -> T[] {
    return T[](
        self._length;
        index => self._storage[index]!
    );
}
```

The result has length `self._length`, never the vector capacity. It owns
independent array backing. Inline elements follow their ordinary copy
semantics, while shared elements become independent owner handles to the same
pointees. Later mutation, clearing, growth, or destruction of the vector does
not change the returned array.

An empty vector returns `T[]()`-equivalent empty backing. Static type checking
still validates the element expression and its requirements; a runtime length
of zero does not erase an invalid generic operation.

## Confirmed grammar

The punctuation set would gain `=>`. The focused grammar addition is:

```text
array-construction-initializer
                 = array-construction-arguments
                 | array-element-list
                 | indexed-array-initializer

indexed-array-initializer
                 = "(" expression ";" identifier "=>" expression ")"
```

The first expression is the requested length and must have exact type `u64`.
The identifier is a binding declaration, not an expression lookup. The final
expression is the repeated element initializer.

Parsing remains unambiguous after the first parenthesized expression:

- `)` completes existing default-length construction;
- `;` enters indexed construction;
- `,` retains the existing invalid multi-argument diagnostic; and
- `copy` remains the dedicated existing copy-construction marker immediately
  after `(`.

Ordinary postfix operations may follow the closing parenthesis. The feature
does not introduce general `=>` expressions, anonymous functions, match arms,
or comprehensions.

Line breaks are insignificant, so compact and expanded spellings are
equivalent:

```ska
var values: i64[] = i64[](4u; index => index + 1);

var values: i64[] = i64[](
    4u;
    index => index + 1
);
```

## Detailed decisions

### IAC1 — Source form

Adopt `T[](length; index => expression)` and
`new T[](length; index => expression)`.

The form remains visibly part of the existing typed array-construction family.
The semicolon separates the once-evaluated size from the repeatedly evaluated
initializer. The arrow makes the new binding relationship explicit.

The initial feature supplies exactly one index binding and one expression. It
does not accept a statement block, multiple clauses, a condition, a supplied
step, or an omitted binding.

### IAC2 — Grammar and punctuation

Add `=>` as one exact lexer token rather than tokenizing `=` followed by an
unrelated operator. Retain exact spans for `(`, `;`, the binding, `=>`, and
`)` in syntax IR and deterministic syntax dumps.

The semicolon is preferred over a comma because the form is not an ordinary
two-argument call. `T[](length, expression)` would obscure the binding and
invite fill-constructor or overload interpretations. Reusing `->` would
overload both result-type and shared-member punctuation with a third meaning.

### IAC3 — Index binding

The index binding has exact type `i64`, is immutable, and is in scope only in
the element expression. Array lengths are restricted to the maximum `i64`
value, so every generated position is exactly representable and can index an
array without a cast.

The binding is initialized to `0`, increases by one after each completed
element, and is never observable for an empty construction. Name collision,
shadowing, and invalid-assignment diagnostics should follow the existing
lexical binding rules rather than creating array-specific exceptions.

The binding is not captured storage and cannot outlive an evaluation except by
contributing its copied primitive value to an ordinary produced result.

### IAC4 — Type selection and compatibility

The explicit array type resolves before the element expression is checked.
Each dynamic position is an owning initialization destination of that exact
stored element type.

Type checking applies the same destination-directed compatibility used by one
element of `T[]{...}`. It adds no implicit numeric conversion, common-supertype
search, inferred array type, class covariance, or shared-array covariance.

### IAC5 — Evaluation order and observable effects

Execution is defined in this order:

1. evaluate the length expression exactly once;
2. validate the maximum length and checked layout requirements;
3. allocate unpublished inline or shared-outer backing;
4. initialize the dynamic prefix counter to zero;
5. for each increasing index, evaluate the element expression exactly once and
   directly initialize that position;
6. advance the prefix only after that initialization completes normally;
7. clean the completed element evaluation epoch; and
8. publish only when the prefix equals the requested length.

Allocation failure occurs before the first element effect. A zero length
performs no element evaluation. Effects from one element, including ordinary
destructor effects from its non-transferred temporaries, complete before the
next element begins.

The expression may read or mutate surrounding state through operations already
legal at that source position. Indexed construction adds no snapshot or
concurrent-modification rule. Such effects occur deterministically in index
order.

### IAC6 — Element ownership and lifecycle

The existing element-list categories apply dynamically:

- primitive results store their exact bits;
- named exact-class sources copy-construct the destination;
- eligible fresh construction or exact-class results initialize the final slot
  directly;
- inline optionals use ordinary absence, injection, conditional payload, and
  presence publication;
- named nested arrays deep-copy and produced nested arrays adopt backing;
- named shared owners retain into the destination and produced shared owners
  transfer; and
- optional shared owners preserve their ordinary conditional ownership and
  zero-niche behavior.

A repeated named shared owner therefore gives every element an independent
strong handle to one pointee. A repeated `new Item(index)` expression creates
one distinct allocation per evaluated index. A named inline class is copied
once per position. These are consequences of ordinary source provenance, not
special fill behavior.

No element is default-constructed before this operation, and no
copy-assignment operation is selected for a newly initialized position.

### IAC7 — Temporary lifetime and control flow

Each element evaluation is one bounded dynamic cleanup epoch. Temporaries,
anchors, checked guards, optional wrappers, and non-transferred owners created
while evaluating an element are cleaned after that slot is complete and before
the next index begins. Values transferred into the completed slot are excluded
from that cleanup exactly once.

This per-element boundary is required to avoid retaining an unbounded number
of dynamic temporaries until the enclosing statement ends. It is analogous to
one ordinary loop-body iteration, while the complete construction remains one
owning expression result.

The initializer is an expression, not a statement block. `break`, `continue`,
and statement-form `return` do not gain a new target or meaning. Calls inside
the expression retain their ordinary return behavior. Unrecoverable panic
retains the current non-unwinding contract.

### IAC8 — Generic requirements

Generic requirement collection must distinguish indexed construction from
default-length construction. It records:

- exact `u64` production for the length;
- legal stored array element type `T`;
- the operations selected while evaluating the expression; and
- the owning initialization operation selected from the expression into `T`.

It must not record default construction or copy assignment merely because the
result length is dynamic. The completed array retains its independently
computed ordinary copy, assignment, destruction, parameter, and result
requirements when later operations consume it.

This distinction lets the complete `Vec<T>` specialization remain valid for
all currently admitted vector element families. `Vec<T>.to_array()` may still
require copying `T` because its particular source is a named occupied storage
slot; that is the correct requirement and is already required by existing
vector reads and iteration.

### IAC9 — Partial construction, publication, and failure

Backing remains unpublished while its initialized prefix is smaller than the
requested length. Only positions below the prefix are live. The current
position becomes part of the prefix only after its selected initialization
operation completes normally.

Verification must reject:

- projection of a position outside the current construction slot;
- duplicate initialization or prefix advancement;
- advancement before lifecycle-bearing initialization completes;
- an alternate CFG edge that skips an index or reaches publication early;
- reuse of an old index epoch after advancement;
- publication when prefix and requested length are not equal; and
- later use, cleanup, or publication of already consumed backing.

The current unrecoverable-failure model does not promise cleanup after a panic
or allocation failure. Future recoverable exceptions must define dynamic
prefix cleanup before indexed construction may unwind. Normal completion
publishes an ordinary array whose later destruction processes every element in
reverse index order.

### IAC10 — Compiler representation and runtime boundary

Syntax and resolved IR should retain the length expression, punctuation spans,
binding declaration and identity, element expression, exact array identity,
and inline-versus-shared ownership.

HIR should add one indexed construction mode containing:

- the typed length expression;
- one immutable `i64` binding;
- one destination-directed stored-value initialization plan for the element;
- the exact element and array identities; and
- the spans required for diagnostics and dumps.

The plan is checked once but executed once per dynamic index. Lower phases must
not rediscover owning behavior from expression shape.

MIR should reuse the existing unpublished-backing and category-specific final
destination machinery, while extending the static element-list protocol with
a dynamic length, dynamic prefix, and canonical loop. The verified form must
retain:

- one block-local `u64` requested length value;
- one unpublished backing and one `u64` initialized prefix;
- a checked conversion or proof exposing the current prefix as the bound
  `i64` index;
- one loop header proving `prefix < length` before element evaluation;
- one category-specific initialization of the exact `backing[prefix]` place;
- one completion transition advancing the prefix;
- one backedge to the canonical header; and
- one exit proof that `prefix == length` before inline or shared publication.

Exact private instruction names remain an implementation-roadmap decision, but
the dynamic completion relation must be explicit and independently verifiable.
It must not be encoded as default initialization, ordinary array assignment,
or a backend-recognized source pattern.

The backend should lower only verified ordinary CFG, checked allocation,
element destination operations, prefix advancement, and publication. The
runtime remains a checked byte allocator and failure reporter; it receives no
expression, callback, lifecycle function, prefix state, or vector identity.
No public runtime entry point or ABI-version change is part of the frozen
design.

### IAC11 — Standard-vector adoption and delivery

After indexed construction is executable, add
`fn to_array() -> T[]` to `std::vec::Vec<T>` using ordinary source. Cover empty
and spare-capacity vectors, growth, vector/result independence, and primitive,
exact-class, optional, nested-array, nested-vector, shared-exact,
shared-interface, and heterogeneous `shared Obj` elements.

The vector method is downstream validation, not the compiler implementation
mechanism. Compiler tests must demonstrate the general array form without
importing `std::vec`, and vector tests must demonstrate that no special class
identity is required.

## Rejected alternatives

### Default construction followed by assignment

This adds the wrong generic requirement, rejects polymorphic shared elements,
and creates observable initializer, assignment, and destructor effects that
are absent from direct construction.

### A compiler-recognized `Vec.to_array`

This couples array semantics to one library class, creates a privileged method
name, and cannot serve other dynamic exact-array producers. It contradicts the
ordinary-library boundary maintained by the existing vector implementation.

### `ArrayBuilder<T>` as the primitive

A source-level builder can conveniently express conditional or unknown-length
collection, but finalizing optional occupancy storage into `T[]` still needs
the same privileged direct-publication operation. Making the builder a
language item moves rather than resolves the compiler boundary and largely
duplicates `Vec<T>`.

### A consuming `Vec<T>.into_array()`

Consumption can avoid some element copies but cannot reinterpret `T?[]`
occupancy backing as invariant `T[]` backing. It needs the same dynamic
initialization mechanism and should be considered separately after
`to_array()` establishes copying semantics.

### General iterable comprehension or `collect()`

An unknown-length source requires growth or multiple traversal, defines
iterator mutation and failure behavior, and interacts with generic callable or
adapter design. Indexed construction deliberately takes a known length and one
expression so it can allocate exactly once and publish one canonical prefix.

### Member-level default constraints

Conditional generic methods are not implemented, and a default bound would
exclude the ownership families that motivate this work. Even with future
member constraints, default-and-overwrite would retain the wrong observable
lifecycle semantics.

### Fill construction

A fill source raises a separate evaluation decision: evaluate once and copy
the result, or reevaluate once per slot. Indexed construction is explicit that
its expression runs once per index. A future convenience fill form can define
its own policy or lower to indexed construction when observationally valid.

## Diagnostics, dumps, and determinism

Diagnostics should distinguish:

- missing length, semicolon, binding, arrow, expression, or closing
  parenthesis;
- a non-`u64` length;
- an invalid or conflicting binding;
- assignment to the immutable index;
- an invalid array element type; and
- a failing destination-initialization operation at the element expression.

Errors should retain the full construction span plus exact length, binding,
and element-expression spans. Generic application failures should attribute
requirements to indexed element initialization rather than default array
construction.

Syntax, resolved, HIR, and MIR dumps must preserve their phase-owned facts in
stable source and identity order. Cross-process determinism must cover nested
indexed constructions, generic specializations, shared ownership, and CFG
generation. No diagnostic wording is frozen by this proposal.

## Relationship to existing contracts

Promotion updates:

- [Grammar](../language/GRAMMAR.md) for `=>` and the indexed initializer;
- [Arrays](../language/ARRAYS.md) for source behavior, ownership, evaluation,
  failure, costs, and deferred exclusions;
- [Vectors](../language/VECTORS.md) for `to_array()` after implementation;
- [Generic classes](../language/GENERIC_CLASSES.md) for indexed-construction
  requirement inference;
- [Evaluation order](../language/FUNCTIONS_AND_CONTROL_FLOW.md) for repeated
  expression and cleanup epochs;
- [Array compiler contract](../compiler/ARRAYS.md) for typed plans, dynamic
  prefixes, publication, and verification;
- [Phases and IR](../compiler/PHASES_AND_IR.md) for representation ownership;
- [Backend](../compiler/BACKEND.md) and
  [runtime ABI](../compiler/RUNTIME_ABI.md) for the unchanged service boundary;
- [status](../language/STATUS.md), language/compiler indexes, testing guidance,
  and debugging guidance where relevant.

The completed element-list design and implementation records remain historical
inputs. Indexed construction extends their direct-initialization model but
does not rewrite the meaning of `T[]{...}`.

## Deliberately deferred decisions

This proposal does not define or reserve:

- inferred or expected-type-only array construction;
- statement blocks in element position;
- unknown-length iterable collection;
- filtering, flattening, nested clauses, spreads, or general comprehensions;
- fill-once, repetition, or cloning syntax;
- anonymous functions, closures, captures, or generic functions;
- mutable array length, capacity, append, insertion, or removal;
- parallel or unordered element evaluation;
- inclusive, descending, or stepped index generation;
- array covariance or implicit element conversion;
- consuming vector conversion or zero-copy optional-storage reinterpretation;
- recoverable exception cleanup; or
- new runtime allocation or lifecycle callback APIs.

These may build on indexed construction later, but none should be inferred from
the initial syntax or representation.

## Confirmed contract audit

The pre-freeze audit answered all of the following affirmatively against the
promoted language and compiler contracts:

1. Can every existing element-list stored-value initialization category be
   represented once and executed safely at a dynamic prefix place?
2. Does the `i64` binding follow directly from the existing maximum array
   length, including every overflow and allocation-failure path?
3. Can per-element cleanup reuse loop-epoch ownership without retaining a
   temporary across the backedge or cleaning a transferred value?
4. What explicit MIR proof or protocol lets verification establish
   `prefix == length` on the only publication edge?
5. Can static lifecycle and whole-world reachability discover every callable,
   initializer, copy operation, and destructor reachable from the repeated
   expression?
6. Do nested indexed constructions preserve distinct inner and outer prefix
   identities and cleanup state?
7. Do optional class and optional owner destinations publish their inner state
   before advancing the outer array prefix?
8. Does shared-outer construction keep the owner unpublished until the dynamic
   prefix is complete?
9. Do generic requirement origins identify the repeated expression rather than
   incorrectly requesting element default or assignment capability?
10. Can all target backends consume the same verified dynamic-prefix MIR
    without learning the source syntax or vector identity?

## Frozen decision checklist

- [x] Confirm the inline and shared-outer indexed source forms.
- [x] Confirm `;` and the new `=>` punctuation rather than an ordinary argument
      list or reused arrow.
- [x] Confirm one immutable `i64` index binding and its lexical scope.
- [x] Confirm explicit result typing and ordinary direct element
      initialization compatibility.
- [x] Confirm length, allocation, increasing-index evaluation, and effect
      order.
- [x] Confirm category-specific ownership and the absence of default or
      assignment requirements.
- [x] Confirm the per-element temporary cleanup epoch.
- [x] Confirm generic requirement attribution and complete-class validation
      behavior.
- [x] Confirm the dynamic prefix, completion, exit-proof, and publication
      invariants.
- [x] Confirm typed HIR/MIR ownership and the unchanged runtime ABI boundary.
- [x] Confirm `Vec<T>.to_array()` as the first ordinary-library adopter.
- [x] Complete the contract audit and promote confirmed behavior into living
      documentation before creating an implementation roadmap.

## Delivery test obligations

The active implementation roadmap assigns these tests to their owning layers:

- lexer/parser tests for `=>`, inline/shared forms, nesting, postfix use,
  malformed separators, missing components, and recovery;
- resolution tests for binding scope, identity, shadowing, generic template
  traversal, dependency collection, and deterministic dumps;
- type-check tests for length typing, immutable index use, exact destination
  compatibility, every element ownership category, and inaccessible lifecycle
  operations;
- generic requirement tests proving the absence of accidental default and
  assignment requirements for non-defaultable exact and polymorphic shared
  element types;
- HIR/MIR tests for one dynamic initialization plan, canonical CFG, exact
  prefix destination, per-element cleanup, nested prefixes, inline/shared
  publication, and rejection of malformed transitions;
- reachability and static-lifecycle tests for operations reachable only through
  the repeated expression;
- backend tests for primitive widths, class destinations, optionals, nested
  arrays, shared owners, zero length, dynamic length, and allocation failure;
- golden tests for evaluation order, destructor timing, named-copy versus
  produced-transfer behavior, and native output;
- standard-vector tests for `to_array()` across the existing element-family
  matrix, logical length versus capacity, empty conversion, and independence;
- deterministic full-golden repetitions for syntax-to-native observations;
- the repository `make check` gate; and
- `make msrv-check` because compiler syntax and Rust sources change.

## Promotion and delivery boundary

Every design decision and audit question is resolved. The frozen contract is
promoted into living language and compiler documentation, this proposal is
archived, and the separate indexed array construction roadmap owns the
PR-sized delivery sequence based on the inspected phase owners.

Implementation must not begin by silently accepting syntax beyond its staged
task boundary in the living grammar. The roadmap orders representation and
verification work before standard-vector adoption and keeps the runtime ABI
unchanged unless a later confirmed design explicitly supersedes this boundary.

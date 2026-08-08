# Explicit Array Element-List Construction Design Proposal

Status: draft design proposal; AEL1 through AEL10 await review and
confirmation. Current implemented behavior remains authoritative in the living
language and compiler documentation. No implementation roadmap may begin
until this proposal is confirmed, promoted into those authorities, and
archived as a frozen decision record.

This proposal adds one destination-typed array construction form whose length
and element values come from an explicit ordered source list. It addresses the
current gap between scalar or class construction and array default-length
construction without weakening Skald's exact types, deterministic evaluation,
owning value semantics, or unpublished-backing model.

The proposal deliberately separates:

- current empty, default-length, and explicit-copy array construction;
- the proposed source and lifecycle meaning of explicit element lists;
- representation invariants that must be settled before roadmap work; and
- fill, generator, inference, and related collection features that remain
  separate design questions.

## Intended outcome

The frozen design should provide:

- explicit inline `T[]{elements}` and shared `new T[]{elements}` construction;
- a length determined by the number of listed elements;
- direct initialization of each previously uninitialized array slot;
- deterministic allocation, evaluation, initialization, publication, and
  cleanup behavior;
- the same destination-typed owning initialization rules already used by
  other stored `T` values, without array-specific conversions;
- construction of nonempty arrays whose element type has no default plan;
- preservation of named-copy, produced-value, shared-owner, optional, nested
  array, and exact-class lifecycle behavior;
- no requirement that an element support copy assignment or default
  construction merely to appear in an element list;
- verified target-independent initialized-prefix and publication semantics;
  and
- no public runtime ABI extension.

Freezing this proposal will not make element-list construction executable.
The [status matrix](../language/STATUS.md) remains the sole authority for
compiler availability, and the
[implemented grammar](../language/GRAMMAR.md) remains the exact accepted
syntax until implementation changes it.

## Current boundary

Skald currently accepts these array construction forms:

```ska
T[]()
T[](length)
T[](copy source)

new T[]()
new T[](length)
new T[](copy source)
```

`T[](length)` and `new T[](length)` request the element type's default plan.
This is not universally raw zero-initialization:

- integers, `f64`, and `bool` receive their defined zero or false value;
- supported optionals receive `none`;
- an exact class element invokes one selected zero-argument initializer;
- a nested inline array element becomes one valid empty array;
- a non-null shared class element receives one distinct allocation and
  initialization; and
- a shared array element receives one distinct empty shared allocation.

An element type without a default plan can still construct an empty array, but
cannot currently construct a nonempty array except by copying an existing
exact array. No fill value, per-index generator, inferred array literal, or
explicit element list is implemented.

The authoritative current source behavior is in
[Arrays](../language/ARRAYS.md). The compiler's canonical array identities,
lifecycle capabilities, unpublished backing, initialized prefix, publication,
produced-backing adoption, verification, and backend responsibilities are in
the [Array Compiler and Runtime Contract](../compiler/ARRAYS.md).

The earlier Niflheim2 draft anticipated initializer lists, fill constructors,
or per-element generators as possible later forms. That is useful historical
direction, but this proposal is based on Skald's implemented ownership and
lifecycle model and selects only an explicit element list.

## Design principles

1. **Construction initializes; it does not assign.** Every listed expression
   initializes one previously uninitialized slot. The compiler must not
   default-construct the array and then assign over those values.
2. **The element type stays explicit.** The construction spelling determines
   one exact array identity before any element is checked.
3. **Existing owning rules compose.** An element position behaves like an
   owning initialization destination of its declared stored type; it does not
   invent array-specific conversions or lifecycle members.
4. **Effects remain deterministic.** Allocation occurs at a defined point,
   and element expressions evaluate exactly once from left to right.
5. **Partial backing is never a value.** No array is published until every
   listed element is live.
6. **Storage and target details remain private.** The source form does not
   promise stack storage, static data, contiguity, unrolled machine code, or a
   particular descriptor layout.
7. **The first addition stays narrow.** Fill, generator, comprehension,
   inference, and multidimensional-shape syntax do not enter through this
   proposal.

## Decision register

Every direction below is proposed rather than confirmed. Review may revise a
direction, but AEL1 through AEL10 must all have explicit confirmed outcomes
before promotion or implementation-roadmap work.

| ID | Decision | Proposed direction | State |
|---|---|---|---|
| [AEL1](#ael1--source-form) | Source form | Add `T[]{...}` and `new T[]{...}` | **Proposed** |
| [AEL2](#ael2--list-shape-empty-form-and-separators) | List grammar | Allow empty and nonempty lists; comma-separated with no trailing comma | **Proposed** |
| [AEL3](#ael3--type-selection-and-compatibility) | Type selection | Use the explicit element type and ordinary owning-initialization compatibility | **Proposed** |
| [AEL4](#ael4--allocation-evaluation-and-temporary-order) | Evaluation order | Allocate first, then evaluate and initialize elements once from left to right | **Proposed** |
| [AEL5](#ael5--element-initialization-and-ownership) | Ownership behavior | Initialize each slot directly using category-specific existing owning rules | **Proposed** |
| [AEL6](#ael6--capabilities-and-resulting-array-operations) | Capability requirements | Require only the operation selected by each element source; preserve the resulting array's ordinary capabilities | **Proposed** |
| [AEL7](#ael7--publication-failure-and-cleanup) | Partial construction | Maintain an unpublished initialized prefix and publish only after completion | **Proposed** |
| [AEL8](#ael8--compiler-representation-and-runtime-boundary) | Compiler boundary | Preserve ordered source and typed initialization plans through verified MIR; add no runtime ABI | **Proposed** |
| [AEL9](#ael9--diagnostics-dumps-and-recovery) | Compiler quality | Preserve exact spans and deterministic recovery/dumps without freezing diagnostic wording | **Proposed** |
| [AEL10](#ael10--promotion-and-delivery-boundary) | Freeze and delivery | Promote and archive the complete design before creating an implementation roadmap | **Proposed** |

## Proposed source surface

### Inline arrays

An inline element list constructs one produced owning inline array:

```ska
var numbers: i64[] = i64[]{10, 20, 30};

var points: Point[] = Point[]{
    Point(1, 2),
    Point(3, 4)
};
```

The length is the number of elements: `numbers.len()` is `3u`, and
`points.len()` is `2u`.

The form composes recursively:

```ska
var rows: i64[][] = i64[][]{
    i64[]{1, 2},
    i64[]{3, 4, 5}
};
```

This remains a jagged array. The proposal adds no rectangular shape or
equal-inner-length rule.

### Shared outer arrays

Leading `new` constructs and publishes one non-null shared outer-array owner:

```ska
var points: shared Point[] = new Point[]{
    Point(1, 2),
    Point(3, 4)
};
```

As with current shared array construction, `new` applies to the outer array.
Parentheses continue to place ownership inside the element type:

```ska
var owners: (shared Point)[] = (shared Point)[]{
    new Point(1, 2),
    new Point(3, 4)
};

var shared_owners: shared (shared Point)[] = new (shared Point)[]{
    new Point(1, 2),
    new Point(3, 4)
};
```

The first value owns inline array backing containing shared-owner elements.
The second value owns a shared outer allocation containing shared-owner
elements. Existing type grouping remains authoritative.

### Optional elements

Element-list construction uses existing optional initialization:

```ska
var values: i64?[] = i64?[]{none, 10, none, 20};
var owners: (shared? Point)[] = (shared? Point)[]{
    none,
    new Point(1, 2)
};
```

This does not add optional array values. `shared? T[]` remains an optional
owner of one complete array allocation, while `(shared? T)[]` remains an array
whose elements are optional shared owners.

## Proposed grammar

The planned grammar addition is:

```text
array-element-list = "{" [expression {"," expression}] "}"

array-construction-expression
                   = array-inline-type array-construction-arguments
                   | "new" array-inline-type array-construction-arguments
                   | array-inline-type array-element-list
                   | "new" array-inline-type array-element-list
```

This uses punctuation already recognized by the lexer. The array type remains
part of the primary expression, and ordinary postfix operations may follow the
completed construction:

```ska
i64[]{10, 20}[0]
new i64[]{10, 20}->[1]
```

Braces deliberately avoid overloading the existing one-expression
parenthesized form. `T[](value)` continues to mean default-length construction
and continues to require `value: u64`; it never means a one-element array.

## AEL1 — Source form

**Question:** Which syntax introduces explicit element-list construction?

**Proposed decision:** Add exactly:

```ska
T[]{element0, element1}
new T[]{element0, element1}
```

The explicit `T[]` target agrees with current array construction and avoids an
expected-type inference system solely for collection expressions. `new`
retains its current meaning of shared outer allocation.

Do not add `[element0, element1]`, `T[](element0, element1)`, a contextual
`values` marker, or a new `array` keyword in this profile.

Although the form is visually literal-like, the language contract should call
it **element-list construction** rather than an array literal. Its elements
are arbitrary runtime expressions with effects and owning lifetimes, and the
result ordinarily owns dynamically allocated backing.

## AEL2 — List shape, empty form, and separators

**Question:** Are empty braces valid, and what separator rules apply?

**Proposed decision:** Accept zero or more comma-separated expressions and do
not accept a trailing comma in the initial profile.

```ska
i64[]{}          // valid; equivalent in value and lifecycle to i64[]()
i64[]{1}         // valid one-element array
i64[]{1, 2}      // valid
i64[]{1, 2,}     // invalid in the initial profile
```

Allowing `{}` makes the grammar regular and gives generated or edited lists a
valid zero-element state. It does not deprecate `T[]()`, which remains the
canonical capability-independent empty construction already used throughout
the language and standard library.

Rejecting a trailing comma matches current argument, parameter, and import
list policy. A general trailing-separator design can reconsider all list forms
together later.

An element-list count must fit the existing maximum array length of
`i64::MAX`. In practice the source and compiler resource limits are much
smaller, but the semantic bound remains uniform.

## AEL3 — Type selection and compatibility

**Question:** How is the element type chosen and how is each source checked?

**Proposed decision:** The explicit array type resolves first to one canonical
`ArrayTypeId`. Every list position is then checked as an owning
initialization source for that array identity's stored element type.

This means:

- primitives retain exact-type requirements and receive no implicit numeric
  conversion or contextual literal reinterpretation;
- exact-class values use the same exact or explicitly checked owning source
  rules as an owning local of the element class;
- optional injection and `none` use the existing optional contract;
- shared owners use existing target compatibility, explicit cast, retain, and
  produced-transfer rules;
- nested inline array values require the exact nested array identity; and
- array invariance remains unchanged.

The element list introduces no overload set, common-supertype search, numeric
promotion, or best-element-type inference. An invalid element is diagnosed at
that expression while later elements remain recoverable for additional
checking.

Empty `T[]{}` obtains its type from `T[]`; it does not require a default, copy,
assignment, or element expression from which to infer one.

## AEL4 — Allocation, evaluation, and temporary order

**Question:** In what order do allocation, element expressions, and their
temporaries execute?

**Proposed decision:** Abstract execution is:

1. determine the constant element count from the source list;
2. allocate one unpublished inline or shared outer backing for that count;
3. report an existing allocation failure before evaluating any element when
   backing allocation cannot complete;
4. evaluate the first element expression exactly once;
5. initialize its destination slot completely;
6. continue with each later expression in source order; and
7. publish the complete array only after the final slot is live.

An element expression starts only after the preceding slot is completely
initialized. Calls, checked accesses, owner operations, and other effects
therefore have deterministic left-to-right order.

Ordinary full-expression temporary rules remain in force. Temporaries
completed while evaluating an element remain live through the enclosing local
initializer, assignment right side, argument, return expression, condition,
or other existing full-expression boundary unless their existing operation
has a narrower consumer-bounded rule. Directly initialized element storage is
part of the array, not a temporary.

The compiler may optimize allocation or initialization only when allocation
failure order, expression effects, value bits, ownership, lifecycle calls,
panic behavior, and cleanup remain observationally identical.

## AEL5 — Element initialization and ownership

**Question:** What operation establishes each element's lifetime?

**Proposed decision:** Each position is an uninitialized owning destination,
and its source selects the same category-specific initialization behavior as
an owning destination of that stored type:

| Element category and source | Initialization behavior |
|---|---|
| Primitive | Evaluate and store the exact primitive value |
| Exact class, ungrouped fresh construction | Run the selected ordinary initializer directly in the element slot when eligible under the existing direct-destination rule |
| Exact class, object-returning call | Supply the element slot as the final result destination when eligible under the existing result rule |
| Exact class, existing named place or non-elided produced source | Run the selected copy constructor exactly as the ordinary owning context requires |
| Inline optional | Initialize absent or present using the existing conditional payload lifecycle |
| Inline array, named source | Deep-copy into distinct nested backing |
| Inline array, produced source | Adopt the produced nested backing |
| Shared owner, named source | Copy/retain the owner according to the existing target relation |
| Shared owner, produced source | Transfer/adopt the produced owner |
| Optional shared owner | Initialize absent, or secure/copy/adopt the contained owner according to the existing optional-owner rules |

Grouping continues to have its existing effect on exact-class copy elision.
Element-list construction must not silently broaden class elision beyond the
ordinary direct-destination and object-result rules promoted with this design.

For example:

```ska
var source: Point = Point(1, 2);
var points: Point[] = Point[]{
    Point(3, 4), // eligible direct construction in slot 0
    source       // copy construction in slot 1
};
```

For a shared-owner element, named copying and produced transfer preserve their
ordinary distinction. The list does not promise a fresh pointee for every
slot unless each element expression itself creates one:

```ska
var owner: shared Point = new Point(1, 2);
var owners: (shared Point)[] = (shared Point)[]{owner, owner};
// Both slots own the same Point allocation.

var distinct: (shared Point)[] = (shared Point)[]{
    new Point(1, 2),
    new Point(1, 2)
};
// The slots own distinct Point allocations.
```

## AEL6 — Capabilities and resulting array operations

**Question:** Which element lifecycle capabilities are prerequisites?

**Proposed decision:** Element-list construction does not require the element
type's default plan or copy-assignment plan. Each element requires only the
operation selected for its actual source:

- a primitive store requires no class lifecycle capability;
- an ungrouped fresh exact-class construction requires its selected ordinary
  initializer and access authorization, but need not require copy
  construction when it initializes directly;
- a named exact-class source requires the selected copy constructor;
- a named inline array source requires that nested array's copy plan;
- a produced inline array source requires valid produced-backing adoption;
- a named shared owner requires its ordinary owner-copy/target operation; and
- optional sources require the operation for the selected absent or present
  case.

Consequently, this becomes valid even when `NoDefault` has no zero-argument
initializer:

```ska
var values: NoDefault[] = NoDefault[]{
    NoDefault(10),
    NoDefault(20)
};
```

The resulting array type retains its independently computed lifecycle
capabilities. Constructing one value does not make later named deep copy,
element assignment, slice copy, whole-array assignment, or any other
operation available when the element type lacks that operation. A produced
element-list result may still be adopted by its immediate owning destination
without deep-copying it.

## AEL7 — Publication, failure, and cleanup

**Question:** When does the array become live, and what happens to partial
construction?

**Proposed decision:** Backing remains unpublished while elements are
initialized. The compiler tracks one increasing initialized prefix:

- slots below the prefix are complete live values;
- the next slot is incomplete until its selected initialization returns
  normally;
- slots above the prefix are uninitialized storage and may not be read,
  copied, assigned, destroyed, borrowed, or published; and
- successful completion advances the prefix by exactly one.

Only a prefix equal to the declared list length may be published as an inline
produced array or shared-array owner. Once published, ordinary array lifetime,
deep-copy, adoption, reverse destruction, release, and anchor behavior apply
unchanged.

Skald currently has no recoverable construction failure. Allocation and other
source-reachable panic paths are non-returning and non-unwinding, so the
language guarantees no remaining source-level cleanup after reporting begins.
This proposal does not design exceptional partial-prefix cleanup. A future
recoverable-exception design must explicitly account for already initialized
elements and unpublished backing rather than treating raw slots as live.

On every normal lifetime end, a completed array destroys elements in reverse
index order and releases backing exactly as current arrays do. Element-list
construction does not introduce a second destruction order.

## AEL8 — Compiler representation and runtime boundary

**Question:** Which internal invariants must be frozen before implementation
can be planned?

**Proposed decision:** Preserve the following target-independent boundary:

- syntax retains the array type, optional `new`, both brace spans, every comma,
  every ordered element expression, and the complete construction span;
- resolution assigns the exact recursive array identity and retains one
  distinct element-list construction mode without deciding lifecycle or
  layout;
- HIR records inline versus shared outer ownership and one ordered,
  destination-directed initialization plan per element;
- each HIR element plan explicitly selects its primitive, class, optional,
  inline-array, shared-owner, or optional-owner behavior and required
  lifecycle identities;
- MIR allocates unpublished backing for the exact list count, evaluates and
  initializes elements in order, advances a verified initialized prefix, and
  publishes only a complete backing;
- MIR ownership and lifetime verification proves exact types, selected
  operations, source consumption, prefix order, complete publication, and
  cleanup accounting;
- backend instruction selection implements verified element initialization
  through the same category-specific machinery used by other owning
  destinations; and
- the C runtime continues to provide checked byte allocation and release
  without learning array types, element lists, lifecycle operations, or
  initialized prefixes.

The exact Rust enum and struct names, whether constant list positions use an
index storage or immediate semantic position, whether machine code is
unrolled, helper selection, physical layout, register allocation, and
optimization strategy remain implementation decisions.

No new public runtime function, metadata format, or ABI-version change is
required by the proposed semantics. If implementation inspection later finds
an ABI change unavoidable, the proposal must be reopened before roadmap work
rather than treating that change as an implementation detail.

## AEL9 — Diagnostics, dumps, and recovery

**Question:** Which compiler-quality behavior belongs to the frozen boundary?

**Proposed decision:** Syntax, resolved, HIR, and MIR dumps remain
deterministic and visibly distinguish empty, default-length, explicit-copy,
and explicit element-list construction.

The parser should recover from:

- a missing element after a comma;
- a trailing comma under the proposed initial grammar;
- a missing closing brace;
- malformed nested construction; and
- a misplaced brace after a non-array type or expression.

Recovery should preserve later elements, statements, and declarations when
their boundaries are unambiguous. Type diagnostics should label the failing
element expression and identify the expected stored element type or missing
selected lifecycle capability.

The language does not freeze diagnostic codes, exact compile-time wording,
secondary-label text, or follow-on count. Exact source spans, deterministic
ordering, stable recovery boundaries, and non-panicking malformed-input
handling remain compiler obligations.

## AEL10 — Promotion and delivery boundary

**Question:** When may implementation-roadmap work begin?

**Proposed decision:** No implementation roadmap is written or started until:

1. AEL1 through AEL10 are explicitly confirmed;
2. the complete design is audited against existing array, class lifecycle,
   optional, shared-owner, expression-order, panic, IR, backend, and runtime
   contracts;
3. every confirmed source and representation rule is promoted into living
   authoritative documentation;
4. the status matrix identifies the feature as frozen but unimplemented;
5. this proposal is archived as the historical decision record; and
6. links and indexes are validated.

Promotion should update, at minimum:

- [Arrays](../language/ARRAYS.md) for construction, type, ownership, lifecycle,
  evaluation, publication, failure, and exclusions;
- [the implemented grammar](../language/GRAMMAR.md) with a clearly marked
  frozen-but-unaccepted extension until implementation changes availability;
- [Classes and Lifecycle](../language/CLASSES_AND_LIFECYCLE.md) for exact-class
  array-element destinations, direct construction, result placement, grouping,
  temporaries, and cleanup;
- [Optional Values](../language/OPTIONAL_VALUES.md) and
  [Shared Ownership](../language/SHARED_OWNERSHIP.md) where element destination
  composition needs to be stated;
- [Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md) if
  the full-expression audit requires an explicit list-element boundary note;
- [Errors and Exceptional Control Flow](../language/ERRORS.md) for the
  non-unwinding partial-construction boundary;
- [the status matrix](../language/STATUS.md) for frozen versus implemented
  availability;
- the [Array Compiler and Runtime Contract](../compiler/ARRAYS.md) and
  [Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md) for
  source retention, typed initialization plans, initialized-prefix
  verification, and publication; and
- compiler and language documentation indexes.

Only after promotion should a separate implementation roadmap inspect current
module ownership and divide the work into PR-sized tasks. This proposal does
not establish those tasks, their codes, or their implementation order.

## Contract audit questions for review

The confirmation pass must resolve these questions against current living
contracts rather than assuming the proposed text is already compatible:

- Does treating an array element as an eligible direct exact-class
  initialization/result destination require a narrow extension to the current
  class elision wording, and does grouping still prevent the same cases?
- Do optional-present element sources already have a complete
  destination-directed rule for every implemented optional payload category?
- Do shared class/interface/`Obj` targets need any additional wording to make
  clear that element lists reuse existing explicit target compatibility rather
  than adding array covariance?
- Does preserving all completed element-expression temporaries to the enclosing
  full-expression boundary agree with current cleanup lowering for arguments,
  results, assignments, conditions, and nested lists?
- Can existing MIR initialized-prefix verification represent heterogeneous
  destination-directed element initialization without weakening publication
  or produced-value accounting?
- Can the x86-64 backend reuse ordinary destination initialization for every
  legal element category without a new runtime entry point or layout promise?

These are audit obligations, not invitations to move implementation detail
into the source contract. Any contradiction that changes observable behavior
must revise the applicable AEL decision before confirmation.

## Decisions required before roadmap work

- [ ] Confirm the explicit `T[]{...}` and `new T[]{...}` source forms.
- [ ] Confirm typed empty braces, comma separation, and the initial rejection
      of trailing commas.
- [ ] Confirm explicit element-type selection and reuse of ordinary owning
      initialization compatibility without inference or array covariance.
- [ ] Confirm allocation-before-elements, exact left-to-right evaluation, and
      enclosing full-expression temporary lifetime.
- [ ] Confirm category-specific direct slot initialization, including class
      direct destinations, nested-array adoption/deep copy, shared-owner
      retain/transfer, and optional initialization.
- [ ] Confirm per-source capability requirements and the independence of the
      resulting array type's later copy and assignment capabilities.
- [ ] Confirm unpublished initialized-prefix, complete publication, current
      non-unwinding failure, and reverse completed-array destruction rules.
- [ ] Confirm the syntax/resolution/HIR/MIR/verifier/backend representation
      boundary and unchanged public runtime ABI.
- [ ] Confirm span, recovery, dump, and non-normative diagnostic boundaries.
- [ ] Complete the contract audit questions without leaving a representation
      decision unresolved.
- [ ] Promote the complete confirmed design into living language and compiler
      documentation, including frozen-but-unimplemented status.
- [ ] Validate links and indexes, then archive this proposal before creating
      an implementation roadmap.

## Deliberately deferred decisions

This proposal does not define or reserve:

- inferred `[element0, element1]` array literals;
- expected-type-only array construction;
- fill-value construction or whether a fill source evaluates once or once per
  element;
- per-index generators, lambdas, closures, comprehensions, iterator-based
  collection, or callback construction;
- multidimensional rectangular shapes or contiguous nested storage;
- spread, concatenation, repetition, ranges, or conditional list elements;
- trailing separators as a language-wide policy;
- mutable-length vectors, capacity, append, insertion, removal, or collection
  classes;
- static immutable array data, constant-expression arrays, compile-time
  allocation, or deduplication;
- implicit numeric conversion, common-supertype selection, array covariance,
  or dynamic element typing;
- general move syntax or a change to inline class value semantics;
- recoverable exceptions, unwinding, or partial-prefix cleanup after a caught
  construction failure;
- whole shared-array pointee replacement; or
- new array runtime entry points or a revised public ABI.

Fill and generator construction deserve separate decisions because their
ownership behavior differs materially. Repeating a named shared owner would
make every slot share one allocation, while evaluating a generator expression
per index could create distinct allocations and introduces a new binding and
control-flow surface. Neither should be inferred from element-list semantics.

## Eventual test obligations

A later implementation roadmap should allocate tests to their owning layers,
but the frozen design establishes these eventual coverage families:

- syntax for inline/shared, empty/single/multiple, nested, optional, and
  ownership-grouped lists;
- rejection and recovery for trailing commas, missing expressions/braces,
  untyped lists, and parenthesized multi-element imitations;
- exact element typing and precise failing-element diagnostics;
- non-default-constructible classes initialized from explicit fresh
  constructions;
- named versus fresh class sources and observable copy-constructor effects;
- named nested arrays deep-copied and produced nested arrays adopted;
- named shared owners retained, produced owners transferred, and distinct
  `new` expressions kept distinct;
- optional absent and present elements across primitive, class, and shared
  owner payloads;
- left-to-right expression effects and allocation failure before element
  effects;
- full-expression temporary and hidden-anchor cleanup after complete list
  consumption;
- increasing initialized-prefix publication and reverse destruction;
- inline local, field, parameter, result, assignment, slice/nesting, and
  shared-outer consuming contexts;
- HIR/MIR dump determinism and verifier mutation tests for missing, duplicate,
  out-of-order, mismatched, or post-publication initialization;
- allocation, lifecycle, owner-count, adoption, and cleanup probes in native
  golden tests;
- assembler acceptance and deterministic output; and
- absence of a new runtime symbol or ABI-version change.

Every implementation task that changes Rust or accepted Skald syntax should
run focused owner tests, the repository's complete `make check` gate, and the
documented supported-toolchain gate.

## Promotion criteria

This proposal may be frozen and archived only when:

- AEL1 through AEL10 have explicit confirmed decisions;
- every item under
  [Decisions required before roadmap work](#decisions-required-before-roadmap-work)
  is complete;
- the contract audit has resolved every material interaction;
- all deliberate deferrals remain outside the promoted contract except as
  explicit exclusions;
- living language and compiler documentation contains the complete
  authoritative design without requiring readers to consult this proposal;
- the status matrix clearly distinguishes frozen design from compiler
  availability;
- no implementation roadmap has started from an unresolved decision; and
- documentation links, indexes, and terminology have been validated.

After promotion, this file becomes a historical decision record under
`docs/archive/`. Only then may an explicit array element-list implementation
roadmap be created and scheduled.

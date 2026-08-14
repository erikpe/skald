# Structural Indexing and Slicing Design Proposal

Status: frozen design record. SIS1 through SIS15 were confirmed together on
2026-08-14 and promoted into living language and compiler contracts before the
implementation roadmap was created.

The living [language contract](../language/INDEXING_AND_SLICING.md) and
[compiler contract](../compiler/INDEXING_AND_SLICING.md) are authoritative for
the frozen direction. The
[active roadmap](../roadmaps/STRUCTURAL_INDEXING_AND_SLICING_ROADMAP.md) owns
delivery order and progress.

This proposal lets ordinary classes and interfaces opt into bracket indexing
and slicing through structurally selected instance methods. It is intended to
make reusable containers such as `Vec<T>` concise, give immutable classes such
as `Str` natural read syntax, and preserve Skald's existing ownership,
evaluation-order, dispatch, and explicit shared-dereference rules.

The proposal adopts Niflheim's useful separation between built-in array
operations and name-based collection sugar, but does not copy its parser-level
slice rewrite. Skald retains bracket source structure until resolution, keeps
built-in arrays on their existing typed intrinsic path, and normalizes accepted
class or interface sugar to ordinary selected calls before HIR.

## Intended outcome

The confirmed design provides:

- right-side class and interface indexing through `index_get`;
- left-side class and interface indexing through `index_set`;
- right-side class and interface slicing through `slice_get`;
- left-side class and interface slicing through `slice_set`;
- independently optional slice bounds without a second receiver evaluation;
- arbitrary signature-selected index key types;
- read-only collections that implement getters without setters;
- ordinary inheritance, privacy, virtual dispatch, interface dispatch,
  argument modes, result ownership, and full-expression cleanup;
- the existing explicit `->` or `*` crossing for shared class and interface
  owners;
- an unchanged built-in array language and compiler contract;
- no new target-independent runtime operation, target operation, or public C
  runtime ABI; and
- focused diagnostics and deterministic phase dumps that expose the selected
  structural operation.

Freezing this design does not make the feature executable. The
[status matrix](../language/STATUS.md) remains the sole authority for compiler
availability, and the
[implemented grammar](../language/GRAMMAR.md) remains the exact accepted
syntax and meaning until implementation changes it.

## Current boundary

Skald already parses postfix bracket projections in all four index and slice
shapes:

```ska
value[index]
value[start:end]
value[:end]
value[start:]
value[:]
```

The parser also retains a bracket projection on the left of `=` as
assignment-shaped syntax. Ordinary `[...]` and shared `->[...]` projections
participate in the same postfix chains as calls, member access, unwrap, and
other postfix operations.

Today every such source node is named and resolved as an array projection.
Type checking accepts it only when the receiver is an exact inline array or
when the shared spelling explicitly selects a shared array pointee. Built-in
array indexing and slicing then lower through dedicated typed HIR, verified
MIR, and target operations. The [array language contract](../language/ARRAYS.md)
explicitly excludes structural `index_get`, `index_set`, `slice_get`, and
`slice_set` protocols.

The current standard library exposes the underlying behavior through ordinary
methods instead:

- `Vec<T>` has `get(index: i64) -> T` and
  `mut set(index: i64, value: T) -> unit`;
- `Str` has `byte(index: i64) -> u8` and
  `slice(start: i64, end: i64) -> Str`; and
- neither class receives compiler significance from those names.

The compiler already has the downstream machinery needed by structural sugar:
resolved direct, static, instance, and interface calls; read-only and mutable
receiver checks; exact, virtual, and interface dispatch; target-directed
primitive, class, array, optional, and shared arguments and results; produced
receiver carriers; explicit shared dereference; full-expression anchors and
cleanup; call reachability; MIR verification; and native call lowering.

The design gap is therefore primarily a source-to-resolved-call selection
contract rather than a new runtime collection model.

## Niflheim precedent

Niflheim defines this conceptual mapping:

```text
obj[index]       -> obj.index_get(index)
obj[index] = rhs -> obj.index_set(index, rhs)
obj[begin:end]   -> obj.slice_get(begin, end)
obj[begin:end] = rhs
                 -> obj.slice_set(begin, end, rhs)
```

Its index key type is selected from the method signature, reads and writes can
use different value types, slice bounds are `i64`, built-in arrays remain a
special case, and compatible interface requirements participate in the same
sugar.

That direction is a useful semantic precedent, but its parser rewrites an
omitted slice end to a casted `obj.len()` call while also retaining `obj` as the
`slice_get` or `slice_set` receiver. An effectful receiver therefore appears in
two source-AST positions unless a later phase stabilizes it. Skald's array
contract already requires one receiver evaluation, precise source ordering,
and explicit temporary ownership. This proposal keeps the structural idea but
uses optional bound parameters so no hidden `len()` receiver use is required.

Niflheim remains historical guidance rather than normative Skald behavior.
Skald's inline values, deterministic destruction, explicit owners, aliases,
and target-directed call results remain authoritative.

## Design principles

1. **Sugar selects ordinary behavior.** A class or interface bracket operation
   chooses one ordinary method or requirement and then follows normal call
   semantics.
2. **Arrays stay intrinsic.** Exact built-in arrays retain their existing
   language meaning, lifecycle operations, failures, HIR, MIR, and backend
   realization.
3. **Read and write capabilities are independent.** A getter is not required
   for assignment and a setter is not required for reading.
4. **Receiver access remains visible.** Read sugar uses read-only instance
   behavior; write sugar requires a mutable receiver and mutable instance
   behavior.
5. **Omission is data, not a hidden second call.** Structural slice methods
   receive `none` for an omitted bound and an implicitly present value for a
   supplied bound.
6. **Evaluation occurs once in source order.** The receiver, supplied bounds or
   index, and assignment source each evaluate exactly once.
7. **Static type controls selection.** A concrete class uses class hierarchy
   lookup; an interface-typed receiver uses only requirements declared by that
   interface.
8. **Ownership composes through calls.** The selected declaration's ordinary
   parameter modes and result type determine copying, borrowing, adoption,
   anchors, and cleanup.
9. **Method bodies own collection policy.** Negative-index interpretation,
   logical length, bounds checks, resizing, copying, aliasing, and overlap
   behavior are class semantics unless the receiver is a built-in array.
10. **The first feature stays narrow.** It adds no iterator protocol, operator
    declaration system, inferred collection type, or compound assignment.

## Decision register

| ID | Decision | Confirmed direction | State |
|---|---|---|---|
| [SIS1](#sis1--source-surface-and-precedence) | Source surface | Reuse existing postfix bracket and assignment syntax; built-in arrays take precedence | **Confirmed** |
| [SIS2](#sis2--protocol-names-and-structural-eligibility) | Protocol identity | Select exact instance names `index_get`, `index_set`, `slice_get`, and `slice_set` structurally | **Confirmed** |
| [SIS3](#sis3--index-read-contract) | Index reads | Lower `receiver[key]` to one ordinary read-only `index_get` call with signature-selected key and result types | **Confirmed** |
| [SIS4](#sis4--index-assignment-contract) | Index writes | Lower `receiver[key] = source` to one mutable unit-returning `index_set` call; do not require or invoke `index_get` | **Confirmed** |
| [SIS5](#sis5--slice-bound-representation) | Omitted bounds | Require value parameters `start: i64?` and `end: i64?`; pass omitted bounds as `none` | **Confirmed** |
| [SIS6](#sis6--slice-read-and-assignment-contracts) | Slice operations | Select read-only `slice_get` and mutable unit-returning `slice_set`; keep read and write value types independent | **Confirmed** |
| [SIS7](#sis7--receiver-access-shared-owners-and-produced-values) | Receiver behavior | Apply existing access and explicit-dereference rules, including produced-receiver restrictions and anchors | **Confirmed** |
| [SIS8](#sis8--inheritance-visibility-and-dispatch) | Object model | Use ordinary inherited lookup, declaring-class privacy, virtual selection, and interface requirements | **Confirmed** |
| [SIS9](#sis9--evaluation-order-lifetimes-and-failure) | Effects and lifetime | Evaluate receiver, operands, and source exactly once in source order and reuse ordinary call cleanup | **Confirmed** |
| [SIS10](#sis10--built-in-array-boundary) | Array coexistence | Keep array brackets intrinsic and do not add structural method-form array operations | **Confirmed** |
| [SIS11](#sis11--compiler-phase-and-ir-boundaries) | Representation | Generalize source AST naming, select calls during resolution, and leave no structural-sugar node in HIR or MIR | **Confirmed** |
| [SIS12](#sis12--diagnostics-dumps-and-recovery) | Compiler quality | Preserve punctuation spans, distinguish unsupported operations, and dump selected identities deterministically | **Confirmed** |
| [SIS13](#sis13--standard-library-adoption) | Library use | Add protocol entry points to `Vec<T>` and read-only protocol entry points to `Str` without making either a compiler language item | **Confirmed** |
| [SIS14](#sis14--initial-exclusions) | Feature boundary | Exclude iteration, multi-indexing, strides, compound assignment, user-declared operators, and implicit shared dereference | **Confirmed** |
| [SIS15](#sis15--promotion-and-delivery-boundary) | Delivery | Confirm the register, promote living contracts, then create a PR-sized implementation roadmap | **Confirmed** |

## Frozen source behavior

### Indexing a mutable vector-like class

```ska
class Buffer {
    private values: i64[];

    init(length: u64) {
        self.values = i64[](length);
    }

    fn index_get(index: i64) -> i64 {
        return self.values[index];
    }

    mut fn index_set(index: i64, value: i64) -> unit {
        self.values[index] = value;
    }
}

var buffer: Buffer = Buffer(4u);
var first: i64 = buffer[0];
buffer[1] = first + 1;
```

The two bracket forms select different methods. The write does not first read
the destination and does not require `index_get` to exist.

### Signature-driven keys

```ska
class FlagMap {
    init() {}

    fn index_get(key: bool) -> i64 {
        // ...
    }

    mut fn index_set(key: bool, value: i64) -> unit {
        // ...
    }
}

var values: FlagMap = FlagMap();
var yes: i64 = values[true];
values[false] = 42;
```

Indexing does not hard-code `i64`. The key expression is checked against the
selected method parameter using ordinary argument compatibility.

### Read-only strings

```ska
var text: Str = "skald";
var first: u8 = text[0];
var tail: Str = text[1:];
var copy: Str = text[:];
```

`Str` may implement `index_get` and `slice_get` without implementing either
setter. `text[0] = 'S'` is rejected as unavailable index assignment rather
than falling back to a getter or exposing the private byte storage.

### Shared receivers

```ska
var owner: shared Buffer = new Buffer();
var first: i64 = owner->[0];
owner->[1] = first;

var same: i64 = (*owner)[0];
```

`owner[0]` remains invalid. Bracket sugar does not add an implicit shared
dereference. Each `->` or `*` crosses exactly one shared edge under the current
ownership and anchor rules.

### Interface receivers

```ska
interface ByteSequence {
    fn index_get(index: i64) -> u8;
    fn slice_get(start: i64?, end: i64?) -> Str;
}

fn first_byte(ref sequence: ByteSequence) -> u8 {
    return sequence[0];
}
```

An interface-typed receiver is eligible only when the interface declares the
required exact requirement. A concrete implementation method that is absent
from the receiver's static interface does not become visible structurally.

## Frozen structural contracts

The following signatures use `K`, `R`, and `W` as design metavariables, not
new Skald generic-method syntax.

### Index read

```text
fn index_get(key: K) -> R
```

The method must be an ordinary read-only instance method. `K` and `R` are the
ordinary resolved parameter and result types. The key parameter may use a
value or read-only `ref` binding mode; its normal argument eligibility applies.
A mutable `mut ref` key is not eligible because an index operation does not
grant permission to mutate its operand.

### Index assignment

```text
mut fn index_set(key: K, replacement: W) -> unit
```

The method must be a mutable instance method with exactly two parameters and
an exact `unit` result. The key and replacement parameters may use value or
read-only `ref` modes. Mutable alias parameters are not eligible. Their
ordinary modes still determine whether a source is copied, transferred, or
borrowed for the call.

The getter result `R` and setter replacement `W` are deliberately independent.
A class may expose only one operation, and a class exposing both is not
required to make `R` and `W` equal. Each bracket use checks only its selected
operation.

### Slice read

```text
fn slice_get(start: i64?, end: i64?) -> R
```

The method must be a read-only instance method. Its first two parameters are
exact optional-`i64` value parameters. A supplied `i64` bound uses existing
one-layer optional injection; an omitted bound supplies `none`.

### Slice assignment

```text
mut fn slice_set(start: i64?, end: i64?, replacement: W) -> unit
```

The method must be a mutable instance method. Its first two parameters are the
same exact optional-`i64` value parameters, its third parameter may use value
or read-only `ref` mode, and its result is exact `unit`. A mutable alias
replacement is not eligible.

The slice getter result and setter replacement type are independent. The
compiler does not require a slice to return the receiver type, an array, a
view, or any other distinguished shape.

## SIS1 — Source surface and precedence

**Question:** Does structural collection sugar add syntax, and how does it
coexist with arrays?

**Confirmed decision:** Add no token or new bracket grammar. Reinterpret the
existing postfix projection source shape according to the receiver's resolved
static type.

The existing forms remain:

```text
postfix-expression
    = primary-expression
      { ... | index-or-slice-suffix | shared-index-or-slice-suffix }

index-or-slice-suffix
    = "[" index-or-slice-bounds "]"
shared-index-or-slice-suffix
    = "->" "[" index-or-slice-bounds "]"
```

Selection precedence is:

1. an ordinary exact array receiver uses built-in array semantics;
2. a shared array receiver reached through `->` or explicit `*` uses built-in
   shared-array projection;
3. an exact class receiver selects the applicable structural instance method;
4. an interface receiver selects the applicable declared requirement; and
5. other receivers are rejected.

Arrays never opt out of their intrinsic meaning by declaring or importing a
class method with a protocol spelling. A class never acquires built-in bounds
or array lifecycle merely because it implements a protocol method.

Assignment remains statement syntax rather than a value-producing expression.
No chained assignment or assignment-expression value is introduced.

## SIS2 — Protocol names and structural eligibility

**Question:** How does a type opt in?

**Confirmed decision:** Reserve no new keyword or annotation. The resolver looks
for exactly one applicable ordinary instance member with the exact name
selected by the source operation:

```text
index_get
index_set
slice_get
slice_set
```

The names are case-sensitive. They remain legal ordinary method and interface
requirement names and may be called explicitly. The compiler should centralize
their spellings in one collection-sugar owner rather than compare raw strings
through multiple phases.

Eligibility is structural at the statically selected declaration boundary:

- a class participates when ordinary hierarchy lookup selects a compatible
  accessible instance method;
- an interface participates when it declares a compatible requirement;
- a static method, field, static field, wrong-arity method, wrong-access method,
  or incompatible signature does not satisfy the operation; and
- no marker interface or registration declaration is required.

Structural eligibility does not mean general structural class compatibility.
Classes remain nominal exact types with their existing hierarchy and
interfaces. Only these four source operations perform name-based member
selection.

## SIS3 — Index read contract

**Question:** What does `receiver[key]` mean for a class or interface?

**Confirmed decision:** Select `index_get`, validate the read protocol, and
normalize the operation to the equivalent ordinary instance or interface call:

```text
receiver[key] -> receiver.index_get(key)
```

The receiver is checked before the key, and the key is checked through the
selected parameter's ordinary mode and type. The call result has the selected
method's exact result type and follows its ordinary target-directed behavior:

- primitives produce scalar values;
- exact classes produce owning results consumed or cleaned normally;
- arrays produce owning array results with named or produced provenance;
- shared results transfer or retain one owner under existing call rules; and
- optional results preserve their exact optional identity and payload plan.

The protocol itself adds no result conversion, borrowing result, or view
result category.

## SIS4 — Index assignment contract

**Question:** What does `receiver[key] = source` mean?

**Confirmed decision:** Select `index_set` directly and normalize the statement
to one ordinary unit-returning call:

```text
receiver[key] = source -> receiver.index_set(key, source)
```

The receiver, key, and source are each evaluated once in that order. The
getter is neither selected nor required. Setter argument modes determine
whether the key and source are passed as values or read-only aliases. The
receiver must grant mutable access and the method or requirement must declare
mutable receiver access.

An `index_set` returning a non-`unit` type does not become a valid assignment
merely because an ordinary explicit call could consume that result elsewhere.
The structural signature is malformed for assignment and should receive a
protocol-focused diagnostic.

## SIS5 — Slice bound representation

**Question:** How are independently omitted class-slice bounds represented
without evaluating the receiver twice?

**Confirmed decision:** Require exact `i64?` value parameters for the first two
slice arguments. A supplied bound uses existing implicit optional injection,
and an omitted bound is a synthetic resolved `none` carrying the omission's
source position.

The mapping is:

```text
receiver[start:end] -> receiver.slice_get(start, end)
receiver[:end]      -> receiver.slice_get(none, end)
receiver[start:]    -> receiver.slice_get(start, none)
receiver[:]         -> receiver.slice_get(none, none)
```

The corresponding `slice_set` mapping appends the assignment source as the
third argument.

`none` has protocol meaning:

- omitted start requests the collection's logical beginning; and
- omitted end requests the collection's logical end.

The method implementation owns how those positions are computed and checked.
The compiler does not select a `len` method, cast a length, or evaluate the
receiver again. Direct calls such as `value.slice_get(1, 3)` remain valid
through ordinary one-layer optional injection.

Alternatives rejected by the confirmed direction are:

- parser rewriting to `receiver.len()`, which duplicates an effectful receiver
  unless the compiler introduces a separate stabilization operation;
- an `i64` sentinel, which consumes a valid value and entangles omission with
  index normalization;
- separate method names for every omission shape, which expands four source
  forms into an unnecessary protocol family; and
- forbidding omitted class bounds while arrays accept them, which gives the
  same bracket syntax inconsistent expressive power.

## SIS6 — Slice read and assignment contracts

**Question:** How do structural slice reads and writes select behavior?

**Confirmed decision:** A read selects `slice_get`; an assignment selects
`slice_set` directly.

```text
receiver[start:end]
    -> receiver.slice_get(start?, end?)

receiver[start:end] = source
    -> receiver.slice_set(start?, end?, source)
```

The `?` notation above describes optional injection and is not source syntax.
The getter must be read-only. The setter must be mutable and return `unit`.
Neither operation requires the other, and their result and replacement types
are not coupled.

Structural slicing does not inherit built-in array behavior automatically. A
class method decides whether a slice is a copy or view, whether negative
bounds are supported, how omission maps to logical positions, whether a write
requires equal lengths, and how overlap behaves. The focused living language
contract should state these as method-implementation responsibilities so users
do not infer array snapshot semantics for arbitrary classes.

## SIS7 — Receiver access, shared owners, and produced values

**Question:** Which receiver forms can use the protocol?

**Confirmed decision:** Reuse ordinary method and interface receiver rules.

- Mutable owning locals, value parameters, mutable aliases, mutable fields,
  mutable static fields, and mutable `self` paths may call setters when their
  existing access rules allow it.
- Read-only aliases and read-only method receivers may call getters but not
  setters.
- A produced exact class value may use a getter through the existing
  produced-receiver carrier and full-expression temporary. It may not call a
  mutable setter merely to mutate a discarded unnamed inline result.
- Shared class and interface owners require `owner->[...]` or
  `(*owner)[...]`; ordinary `owner[...]` remains an implicit-dereference error.
- Optional shared owners must be unwrapped before the explicit dereference.
- Produced or replaceable shared owners retain the same checked anchors and
  cleanup boundary used by explicit method calls.

The syntax grants no access that an explicit call to the selected declaration
would lack.

## SIS8 — Inheritance, visibility, and dispatch

**Question:** How does protocol selection interact with the object model?

**Confirmed decision:** Use the same declaration lookup and dispatch metadata as
ordinary calls.

- Inherited methods participate through the existing class hierarchy index.
- Declaring-class private methods are usable only from the declaring class's
  bodies under the ordinary privacy rule.
- Private methods retain direct selection.
- Virtual roots and overrides use ordinary exact-versus-dynamic call
  selection and preserve their virtual family and slot.
- Interface receivers select only an exact requirement declared by their
  static interface.
- Shared interface owners use the existing interface-view and dereference
  machinery.
- A class method that happens to have a protocol name does not satisfy an
  interface-typed operation unless the interface declares the corresponding
  requirement and the class conformance maps it normally.

Closed generic class specializations participate as ordinary exact classes.
For example, `Vec<Str>` selects the specialized `index_get` result type `Str`;
no unresolved template parameter reaches HIR.

## SIS9 — Evaluation order, lifetimes, and failure

**Question:** What order and lifetime guarantees apply?

**Confirmed decision:** Structural sugar has the same order as its conceptual
ordinary call, with assignment syntax fixing the replacement as the last
argument:

```text
index read:       receiver, key, call
index assignment: receiver, key, source, call
slice read:       receiver, supplied start, supplied end, call
slice assignment: receiver, supplied start, supplied end, source, call
```

Omitted bounds perform no source evaluation. Every present expression is
evaluated exactly once. Receiver selection and null/absence checks happen
before explicit operand effects under existing call and dereference rules.

All completed temporaries remain governed by the ordinary full-expression
tracker. Target-directed call arguments secure owners or create copies before
the call as already required. Receiver anchors remain live through operand
evaluation and the call. Results transfer, copy, adopt, or clean up according
to their ordinary exact type.

The protocol adds no compiler-known bounds or slice-length failure. A method
may call `panic` or otherwise terminate under ordinary semantics. Built-in
array projections retain their distinct compiler-known failure reasons.

## SIS10 — Built-in array boundary

**Question:** Does structural sugar replace or expose array implementation?

**Confirmed decision:** No. Arrays remain a privileged exact type constructor
with their current intrinsic operations.

For arrays, the compiler continues to own:

- exact `i64` indices and supplied bounds;
- negative normalization;
- checked element and bound projections;
- copied slice reads;
- checked equal-length slice assignment;
- element lifecycle selection;
- alias anchors and produced backing;
- compiler-known failure reasons; and
- dedicated HIR, MIR, verification, and target lowering.

The first structural-sugar profile does not make
`array.index_get(index)`, `array.index_set(index, value)`,
`array.slice_get(start, end)`, or `array.slice_set(...)` valid method-form
aliases. Adding method-form array operations would be a separate compatibility
surface with no benefit to bracket sugar.

## SIS11 — Compiler phase and IR boundaries

**Question:** Where should the sugar be represented and eliminated?

**Confirmed decision:** Preserve generic bracket syntax through parsing, select
the final operation during resolution, and expose only ordinary calls or true
array operations to type checking and lower phases.

### Syntax

Rename source-only `ArrayProjectionExpr`, `ArrayProjectionOperator`, and
`ArrayProjectionBounds` vocabulary to neutral bracket or subscript vocabulary.
The parser retains:

- the receiver;
- ordinary versus shared-arrow spelling;
- index versus slice shape;
- independently optional bounds;
- bracket, colon, and arrow spans; and
- the complete expression span.

The source AST makes no type or method choice.

### Resolution

Resolution determines the receiver's static kind and then:

- retains a resolved array projection for a built-in array;
- selects one accessible class method and emits an ordinary resolved method
  call for class sugar;
- selects one interface requirement and emits an ordinary resolved interface
  call for interface sugar;
- converts a structural assignment to a resolved unit-call statement; and
- inserts resolved `none` operands for omitted structural slice bounds.

The selector should be one cohesive resolver concern shared by read and write
forms. It should use centralized protocol names and ordinary hierarchy,
privacy, shared-dereference, and interface receiver services rather than
manufacturing a fake source member-access AST and resolving the receiver twice.

### Type checking and HIR

Type checking validates the protocol-specific method shape before applying
ordinary call checking. Once validated, existing method and interface call
paths own receiver access, arguments, result types, optional injection,
ownership, and dispatch. HIR contains an ordinary method or interface call and
no structural-sugar operation.

The source bracket or colon span may serve as the call-site selection span for
diagnostics. HIR retains final declaration identities and dispatch metadata,
not protocol strings.

### MIR, verification, backend, and runtime

MIR lowering sees ordinary calls and therefore needs no new structural
instruction. Existing call lowering already stabilizes method receivers before
arguments, selects direct, virtual, or interface targets, and realizes every
supported result category. Reachability, static-effect inference, cleanup,
verification, target lowering, and ABI marshaling continue to inspect ordinary
calls.

No public C runtime symbol, descriptor field, vtable shape, interface witness
shape, or target ABI change is required.

## SIS12 — Diagnostics, dumps, and recovery

**Question:** What quality guarantees should accompany the feature?

**Confirmed decision:** Preserve exact source punctuation and diagnose failures
at the phase that owns them without freezing final prose in the design.

Focused diagnostics should distinguish at least:

- receiver type is not indexable;
- receiver type is not index-assignable;
- receiver type is not sliceable;
- receiver type is not slice-assignable;
- required protocol member is missing;
- selected member is a field, static field, or static method;
- getter is mutable rather than read-only;
- setter is read-only rather than mutable;
- arity or slice-bound parameter type is incompatible;
- setter result is not `unit`;
- a protocol alias parameter requests mutable access;
- selected member is private at the use site;
- receiver lacks mutable access;
- shared receiver needs explicit dereference; and
- ordinary argument or result compatibility fails after a valid protocol is
  selected.

Syntax recovery continues to own missing expressions, colons, and right
brackets. It must not invent a valid method selection from malformed syntax.

AST dumps retain the bracket source shape. Resolved dumps distinguish true
array operations from selected method/interface calls and show their canonical
identities. HIR and MIR dumps show ordinary call targets with deterministic
IDs and no raw protocol-name lookup. Repeated compilation must produce
byte-identical diagnostics and dumps.

## SIS13 — Standard-library adoption

**Question:** Which standard-library types should demonstrate the protocol?

**Confirmed decision:** Adopt it first in `Vec<T>` and `Str` while keeping both
types ordinary Skald classes.

### `Vec<T>`

Add public protocol entry points for index reads, index writes, slice reads,
and slice writes. The implementation may retain `get` and `set` as
compatibility wrappers during the initial change; one implementation should
own normalization and bounds behavior so the two API spellings cannot drift.

`Vec<T>` slice bounds use its logical length rather than storage capacity.
`slice_get` returns an independent `Vec<T>` containing the selected logical
elements. `slice_set` preserves the destination length, requires the
replacement vector's logical length to equal the selected range, and replaces
elements in increasing index order. The implementation must secure or copy the
complete replacement source required for self-aliasing and later writes before
changing the first destination element. These are documented `Vec<T>`
semantics rather than behavior inferred for every structural protocol method.

### `Str`

Add read-only `index_get` and `slice_get` entry points. They should share the
current checked byte and `O(1)` descriptor-slice implementations. `slice_get`
maps omitted start to the logical beginning and omitted end to the logical end
before using the existing normalized range logic.

`Str` does not add either setter. Bracket assignment remains invalid and its
backing storage remains private. Literal materialization still depends only on
the exact `std::str::Str` representation contract; the string language item
does not acquire a required protocol method or compiler-selected string helper.

The compiler selects these methods only because the source uses generic
structural bracket sugar. It does not special-case the identities of `Vec<T>`
or `Str`.

## SIS14 — Initial exclusions

**Question:** What remains outside the first profile?

**Confirmed decision:** Exclude:

- `for` iteration and iterator lifetime protocols;
- structural `len`, capacity, resizing, append, or collection construction;
- multi-index forms such as `matrix[row, column]`;
- inclusive ranges, strides, reverse-range syntax, or range values;
- compound indexed assignment such as `value[key] += source`;
- increment, decrement, or assignment expressions;
- user-declared operator blocks, annotations, traits, or typeclasses;
- generic methods or generic interfaces introduced solely for the protocol;
- inferred key, result, or replacement type variables outside ordinary closed
  class specialization;
- automatic protocol forwarding or derivation;
- implicit shared or optional unwrap;
- method-form aliases for built-in arrays;
- compiler enforcement of class-specific bounds, overlap, or copying policy;
- external ABI mappings for structurally indexable classes; and
- recoverable bounds failures or exceptions.

These exclusions do not prevent ordinary classes from implementing similarly
named explicit methods or composing existing arrays internally.

## SIS15 — Promotion and delivery boundary

**Question:** What must happen after design confirmation?

**Confirmed decision:** Confirm SIS1 through SIS15 together, then promote the
frozen meaning before implementation begins.

Promotion should:

1. create one focused living language contract for structural indexing and
   slicing;
2. create or extend the focused compiler contract for syntax, resolution,
   call normalization, diagnostics, and lower-phase non-expansion;
3. update the implemented grammar's planned/current wording only as the
   corresponding implementation task lands;
4. update Arrays, Vectors, Strings, Classes and Lifecycle, Shared Ownership,
   compiler phases, testing guidance, and the status matrix at their owning
   implementation milestones;
5. archive this confirmed proposal as the historical decision record; and
6. create a separate PR-sized implementation roadmap ordered by stable phase
   boundaries.

The later roadmap should not use this proposal as a substitute for task
checklists, validation commands, or progress state.

## Validation obligations for implementation

The implementation roadmap should include proportionate coverage at every
owning boundary.

### Syntax and resolution

- all index and four slice-bound shapes in read and assignment positions;
- postfix chaining, grouping, shared arrows, and explicit dereference;
- arrays retaining resolved array nodes while classes become selected calls;
- inherited, private, static, virtual, override, and interface selections;
- missing and malformed protocol members;
- generic closed-specialization selection; and
- deterministic AST and resolved dumps.

### Type checking and HIR

- non-`i64` index keys driven by method signatures;
- getter/setter independence and different read/write value types;
- read-only getters and mutable setters;
- optional slice-bound injection for every omission shape;
- value and read-only alias parameter modes;
- rejection of mutable alias protocol operands;
- primitive, class, array, shared, optional, and generic-substituted results
  and replacement arguments;
- produced and checked receivers;
- virtual and interface HIR targets; and
- focused diagnostics for every protocol-shape failure.

### MIR, verification, and native behavior

- receiver-before-operands and assignment-source-last evaluation;
- exactly-once effectful receivers, keys, bounds, and replacement sources;
- produced receiver storage and cleanup;
- shared-owner and optional-owner anchors through later argument effects;
- target-directed copy, adoption, retention, and cleanup around calls;
- direct, virtual, and interface native dispatch;
- no new runtime dependency or backend collection instruction;
- `Vec<T>` read/write behavior across representative element categories;
- `Str` byte and omitted-bound slice reads plus rejected writes; and
- complete compile and native determinism for representative goldens.

Focused implementation checks should run the syntax, resolver, type-check,
call lowering, object receiver, interface dispatch, shared ownership, optional,
array, and generic specialization suites affected by each task. Before
roadmap completion, run `make check`, the supported-toolchain gate when Rust
targets or manifests are touched, and proportionate full-determinism goldens.

## Confirmation record

The freezing review explicitly confirmed:

- [x] the four exact protocol names;
- [x] structural rather than marker-interface eligibility;
- [x] arbitrary signature-driven index key types;
- [x] independent read and write capabilities and value types;
- [x] read-only getters and mutable unit-returning setters;
- [x] optional-`i64` slice bound parameters and `none` omission;
- [x] ordinary value/read-only-alias operand modes and rejection of mutable
      alias operands;
- [x] built-in array precedence and unchanged intrinsic behavior;
- [x] explicit shared dereference and existing receiver access;
- [x] inherited, private, virtual, and interface selection;
- [x] one source-ordered evaluation of every present operand;
- [x] resolved-call normalization with no HIR/MIR sugar node;
- [x] `Vec<T>` and read-only `Str` adoption boundaries;
- [x] diagnostics, dumps, tests, and runtime non-expansion; and
- [x] the initial exclusions and promotion sequence.

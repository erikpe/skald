# Private Cell Fields Design Proposal

Status: frozen design. PC1 through PC13 were confirmed together on 2026-08-16
and promoted into the living language and compiler contracts before the
implementation roadmap was created. The implemented
[grammar](../language/GRAMMAR.md) and
[status matrix](../language/STATUS.md) remain authoritative for current
compiler behavior.

This proposal adds a narrow form of interior mutability to ordinary instance
fields. A `private cell` field behaves like the equivalent ordinary private
field except that code lexically owned by its declaring class may replace the
complete field through a read-only object place. The exception does not grant
mutable access to the containing object, propagate into the field's contents,
or introduce a `Cell<T>` type.

The immediate motivation is lazy private state such as a cached string hash:

```ska
public class Str {
    private cell _hash_code: u64?;

    fn hash_code() -> u64 {
        if (self._hash_code is some) {
            return self._hash_code!;
        }

        var result: u64 = self._calculate_hash_code();
        self._hash_code = result;
        return result;
    }
}
```

The method remains an ordinary read-only `fn` at its call boundary. Callers do
not need a mutable receiver merely because the implementation memoizes a
result.

## Intended outcome

The initial feature should provide:

- contextual `private cell name: T;` syntax for instance fields;
- ordinary private visibility, storage, initialization, reads, lifecycle, and
  layout for those fields;
- complete field replacement from a read-only place, but only in code
  lexically owned by the field's declaring class;
- no mutable access propagation into an inline object, array element,
  optional payload, shared pointee, or other nested place;
- support for every type already legal in an ordinary instance field, using
  that type's existing assignment and lifecycle behavior;
- composition with inheritance, interfaces, closed generic classes,
  polymorphic views, call-scoped aliases, and produced read-only receivers;
- explicit typed HIR and verified MIR evidence for the exceptional write
  authorization without pretending that the complete receiver is mutable;
- unchanged target layout, internal calling convention, runtime API, and
  public C ABI; and
- focused diagnostics and deterministic phase dumps for syntax, access, and
  invalid attempts to extend the permission into nested mutation.

Freezing this design did not by itself make the syntax executable. The planned
language and compiler contracts are now promoted, and the separate
implementation roadmap owns compiler work.

## Current boundary and architectural evidence

Instance fields currently use this grammar:

```text
field-declaration = ["private"] identifier ":" storage-type ";"
```

`private` is contextual in the class-member position. Fields are public by
default, and an ordinary private field is accessible only from a callable
whose lexical class owner is the field's exact declaring class. Visibility is
retained through resolution and erased after access is authorized.

Every object-place root supplies one access capability for its complete
projection path. An owning local, owning value parameter, or `mut ref`
parameter is mutable; a `ref` parameter is read-only; and `self` receives the
access declared by the current member body. Field and base projections
preserve that access. Consequently an ordinary `fn` may read a field but may
not assign it, call a mutable method through it, or pass it as `mut ref`.

Field assignment is already type-directed rather than a raw store. Depending
on the field type, replacement may copy a scalar, invoke exact-class copy
assignment, transfer or retain a shared owner, update an optional, replace an
array descriptor and backing, destroy displaced state, and run selected user
lifecycle code. Initializers, synthesized copying, assignment, destruction,
evaluation order, cleanup, anchoring, and overlap rules are already explicit
compiler responsibilities.

Aliases are deliberately non-exclusive. Existing calls may pass overlapping
read-only and mutable aliases, and replaceable array or shared fields already
use guards or hidden anchors where later effects could invalidate borrowed
payload or backing. A cell write must reuse these rules; it must not become an
unverified store that bypasses lifecycle, optional guards, ownership, or
anchoring.

The present type checker rejects a field assignment when its receiver has
read-only access. HIR and MIR then rely on typed field identities, receiver
origins, and access metadata, while verification proves that writes target a
legal live destination. The frozen feature therefore fits as a narrow
authorization on a complete selected field assignment. It does not require a
new source-visible reference kind or runtime container.

The sibling Niflheim repository contains no directly applicable cell-field
feature. Rust's `Cell<T>` provides the naming precedent and the useful
whole-value replacement intuition, but Skald's feature remains a field
modifier integrated with Skald's own value, ownership, and lifecycle rules.

## Design principles

1. **`cell` modifies one field, not its type.** `T` retains exactly the same
   type identity and ordinary operations inside and outside a cell field.
2. **The permission is narrow.** Read-only access gains complete replacement
   of the selected cell field and nothing else.
3. **Privacy contains the effect.** Only the declaring class may name the
   field, preserving the class as the abstraction boundary for physical
   mutation.
4. **Read-only does not mean pure.** An ordinary `fn` continues to describe
   receiver access and call compatibility, not freedom from all observable
   effects.
5. **Ordinary assignment remains authoritative.** A cell write uses the same
   evaluation, ownership, lifecycle, failure, and cleanup behavior as the
   equivalent write through a mutable receiver.
6. **Receiver access stays truthful.** The compiler records an authorized cell
   write rather than upgrading the complete receiver or projection path to
   mutable access.
7. **Lifecycle state is ordinary state.** Copying an object copies whether its
   cache is populated; no hidden reset, omission, or transient-field behavior
   is implied.
8. **No runtime borrowing model is introduced.** The feature has no borrow
   counter, dynamic exclusivity check, wrapper allocation, or implicit lock.
9. **Concurrency is separate.** A cell write is neither atomic nor
   synchronized and establishes no cross-thread memory-order guarantee.
10. **The first feature stays local.** Public cells, static cells, properties,
    general mutable references through read-only roots, and a library
    `Cell<T>` abstraction remain separate designs.

## Decision register

| ID | Decision | Confirmed direction | State |
|---|---|---|---|
| [PC1](#pc1--source-syntax-and-contextual-spelling) | Source syntax | Add contextual `private cell name: T;` only for instance fields | **Confirmed** |
| [PC2](#pc2--field-identity-type-and-visibility) | Declaration meaning | Keep an ordinary private `FieldId` and stored type plus one cell capability bit and modifier span | **Confirmed** |
| [PC3](#pc3--authorized-whole-field-replacement) | Interior write | Permit complete replacement through read-only access from the exact declaring class | **Confirmed** |
| [PC4](#pc4--permission-does-not-propagate) | Nested mutation | Preserve the root's read-only access for every operation other than complete replacement of the selected cell | **Confirmed** |
| [PC5](#pc5--assignment-evaluation-and-lifecycle) | Write behavior | Reuse the field type's ordinary assignment, ownership, evaluation, failure, and cleanup rules | **Confirmed** |
| [PC6](#pc6--reads-aliases-and-overlap) | Borrowing | Keep ordinary reads and non-exclusive call-scoped aliases; reuse all existing anchors and guards | **Confirmed** |
| [PC7](#pc7--initialization-copying-and-destruction) | Object lifecycle | Treat cell state exactly like ordinary field state in every initializer and lifecycle plan | **Confirmed** |
| [PC8](#pc8--methods-dispatch-and-logical-state) | Callable contract | Keep `fn` read-only at the call boundary; cell writes add no signature or dispatch effect | **Confirmed** |
| [PC9](#pc9--inheritance-generics-and-polymorphic-views) | Composition | Preserve declaring-class privacy and specialize the cell marker with ordinary closed generic fields | **Confirmed** |
| [PC10](#pc10--compiler-representation-and-verification) | Compiler boundary | Carry explicit cell metadata and cell-write authorization through typed and verified phases | **Confirmed** |
| [PC11](#pc11--target-runtime-and-concurrency-boundary) | Realization | Use ordinary field layout and stores; add no ABI, runtime, atomicity, or synchronization contract | **Confirmed** |
| [PC12](#pc12--diagnostics-dumps-and-testing) | Quality | Diagnose malformed or over-broad uses precisely and make the capability deterministic in dumps and tests | **Confirmed** |
| [PC13](#pc13--standard-library-adoption-and-delivery) | Delivery | Freeze and promote the general feature before separately extending `Str` and its literal language-item contract | **Confirmed** |

## PC1 — Source syntax and contextual spelling

The frozen declaration is:

```ska
class Cache {
    private cell _value: u64?;
}
```

The planned grammar shape is:

```text
field-declaration      = ordinary-field-declaration
                       | cell-field-declaration
ordinary-field-declaration
                       = ["private"] identifier ":" storage-type ";"
cell-field-declaration = "private" "cell" identifier ":"
                         storage-type ";"
```

Both words remain contextual. The parser selects the modifier form only when
`private cell` is followed by a field name and `:`. Existing or otherwise
ordinary identifier uses remain available:

```ska
class Names {
    cell: i64;          // public ordinary field named `cell`
    private: i64;       // public ordinary field named `private`
    private cell: i64;  // private ordinary field named `cell`
}

fn cell(private: i64) -> i64 {
    return private;
}
```

`cell name: T;` without `private`, `private static cell name: T;`, reordered
or repeated modifiers, and `private cell` before a method or lifecycle member
receive focused syntax diagnostics. A static field may still be named `cell`:

```ska
private static cell: u64?;
```

The initial feature does not support a declaration initializer because
ordinary instance fields do not have one.

## PC2 — Field identity, type, and visibility

A cell field is one ordinary instance field with:

- its existing dense `FieldId` and declaration order;
- ordinary declaring `ClassId`, name, stored type, and private visibility;
- one cell capability marker; and
- the exact `cell` modifier span for dumps and diagnostics.

It occupies the same member namespace and participates in collision,
inheritance, containment, layout, initialization, copying, assignment, and
destruction exactly as an ordinary field. It does not create a synthetic
class, wrapper object, getter, setter, method, static slot, or distinct type.

The type of `self._value` in the example remains `u64?`, not `Cell<u64?>`.
Generic requirement inference and stored-type validation inspect the declared
type exactly as they would for an ordinary field.

Cell fields are always private in the initial feature. The existing
declaring-class access rule remains authoritative: same-module code, derived
classes, unrelated classes, and top-level functions cannot select the field.
The modifier is not inherited visibility and does not introduce `protected`
or friend access.

## PC3 — Authorized whole-field replacement

Code lexically owned by the field's exact declaring class may assign the
complete cell field even when the selected object place is read-only:

```ska
class Cache {
    private cell _value: u64?;

    fn remember(value: u64) -> unit {
        self._value = value;
    }

    static fn clear(ref target: Cache) -> unit {
        target._value = none;
    }
}
```

The same permission applies whether the read-only root is `self`, a `ref`
parameter, an authorized checked class view, or another existing read-only
object place accepted by ordinary field selection. The destination must end
at the declared cell field. Grouping and canonical base projection do not
change the selected identity.

The permission is unnecessary but harmless through an already mutable root.
In a `mut fn`, initializer, copy assignment, or destructor, a cell field keeps
all operations that the equivalent ordinary private field would have.

Compiler-generated direct initialization and synthesized lifecycle remain
separate trusted operations; they do not need to manufacture a source-level
cell write.

## PC4 — Permission does not propagate

The special permission authorizes only replacement of the selected field as
one complete destination. It does not upgrade the receiver, the containing
object, or the stored value to mutable access.

Given:

```ska
class Holder {
    private cell _item: Item;
    private cell _values: i64[];
    private cell _maybe: Item?;

    fn refresh(ref replacement: Item) -> unit {
        self._item = replacement;       // allowed: complete cell replacement
        self._values = i64[]{1, 2, 3};  // allowed: complete cell replacement

        self._item.change();            // rejected: mutable nested receiver
        mutate(self._item);              // rejected: mutable alias through read-only access
        self._values[0] = 4;             // rejected: element mutation
        self._maybe!.change();           // rejected: optional-payload mutation
    }
}
```

An ordinary read-only method call, value read, copy source, or eligible `ref`
argument remains allowed. Explicitly dereferencing a shared field, indexing
an array field, unwrapping an optional field, or projecting an inline class
field follows the root's ordinary access rules; the cell marker does not flow
through those operations.

When the root itself is mutable, ordinary existing rules still apply. The
feature does not permanently make a cell field opaque or non-borrowable; it
only keeps the new read-only exception at whole-field granularity.

## PC5 — Assignment evaluation and lifecycle

An authorized cell write is the same type-directed field assignment that
would occur through a mutable receiver. It preserves the existing contract
for:

- destination and source evaluation order;
- exact type compatibility and generic capability requirements;
- scalar bit copying;
- exact-class copy assignment and self-assignment behavior;
- optional injection, presence changes, and payload lifecycle;
- shared-owner retain, transfer, release, and dynamic destruction;
- array replacement, backing ownership, and element cleanup;
- displacement and destruction of the old field value;
- full-expression temporaries and reverse cleanup; and
- current non-unwinding failure behavior.

For an exact-class field, invoking its selected copy-assignment operation is
part of complete field replacement and is allowed. The permission does not
make an independently written nested field assignment or mutable method call
legal.

No cell write may lower to an untyped memory store or skip an unavailable
assignment capability. A generic `private cell _value: T;` acquires the same
contextual requirements from an actual replacement as an ordinary mutable
field assignment of `T`.

## PC6 — Reads, aliases, and overlap

Reading a cell field is indistinguishable from reading the equivalent private
field. Existing rules determine whether the selected source is copied,
forwarded as `ref`, used as a receiver, secured behind an owner anchor, or
consumed through an optional guard.

Cell does not add an escaping reference, local alias, exclusive borrow, or
runtime borrow state. Skald's existing non-exclusive alias rule remains in
force: aliases may overlap, and effects occur in source order. Existing
field-replacement protections remain mandatory:

- array backing borrowed across later effects uses the ordinary backing
  anchor;
- a shared pointee reached through a replaceable owner uses the ordinary
  owner anchor;
- optional checked payload use retains its ordinary guard and invalidation
  rules; and
- an inline class field continues to designate its stable field storage and
  follows ordinary assignment/lifecycle sequencing.

The cell marker is not a justification for bypassing any of these mechanisms.
MIR verification must reject a cell-write operation whose place, guard,
anchor, liveness, or ownership evidence would be invalid for the corresponding
ordinary field assignment.

## PC7 — Initialization, copying, and destruction

A cell field has no special default. Every ordinary initializer must initialize
it exactly once under the existing direct-field initialization rules. Its
declared type decides whether a zero/default source exists.

Synthesized and explicit lifecycle treat the field normally:

- copy construction copies the source field's current state;
- copy assignment assigns the source field's current state;
- whole-object assignment does not clear or recompute a cache implicitly;
- destruction destroys the current field value in ordinary reverse field
  order; and
- failed or partial construction follows existing initialized-prefix rules.

For a cached optional hash, copying a populated object therefore copies the
populated cache, while copying an unpopulated object copies `none`. A different
policy must be written explicitly in user lifecycle code; `cell` does not mean
`transient`, `lazy`, `derived`, or `ignore when copying`.

Cell fields count normally for containment-cycle validation, layout,
capability synthesis, and class size. The modifier changes neither source
field order nor physical layout requirements.

## PC8 — Methods, dispatch, and logical state

An ordinary `fn` that writes a cell remains a read-only-receiver callable:

- callers may invoke it through either mutable or read-only access;
- an interface requirement remains `fn`, not `mut fn`;
- virtual roots and overrides retain the same receiver-access compatibility;
- function-value and call signatures do not gain a hidden effect component;
  and
- produced exact-class values may invoke such a read-only method through the
  existing bounded receiver mechanism.

This is sound because Skald's `fn` marker controls operations available
through the receiver; it is not a purity, const-evaluation, determinism, or
data-race-freedom promise. A cell write may be observable through a later
method call and may run lifecycle code selected by assignment. Classes should
use it for state that preserves their public logical invariants, such as
caches, memoized summaries, and bookkeeping.

No interface exposes a cell declaration. Conformance depends only on methods,
so a class may use private cells to implement an ordinary read-only interface
method without changing that method's signature.

## PC9 — Inheritance, generics, and polymorphic views

A private cell follows ordinary private-field inheritance:

- a base-class body may update its own declared cell through `self`;
- a derived-class body cannot select the base's private field;
- invoking an inherited base method may update the base cell as part of that
  method's behavior; and
- selection through a class/interface/`Obj` view retains the existing exact
  complete-object origin and does not grant direct field access.

Closed generic-class specialization substitutes a cell field's type and
preserves its marker and declaring specialized class identity. Each closed
specialization has ordinary independent object storage. Replacement is
available only when the substituted type and selected source satisfy the same
requirements as an ordinary field assignment.

No generic parameter, nominal interface bound, or type identity records
"cellness." It is a property of one field declaration, not a capability of
values of its type.

## PC10 — Compiler representation and verification

Syntax should retain one explicit cell-field modifier and exact span rather
than inferring the feature from a name or type. Resolution should preserve the
marker on ordinary field declarations, dumps, specialization input, and
specialized fields alongside private visibility.

After ordinary selection and declaring-class privacy succeed, type checking
should authorize a read-only assignment only when:

1. the destination ends at one selected field identity;
2. that field is declared `cell`;
3. the current callable's lexical class owner is the field's exact declaring
   class; and
4. the operation replaces the complete field rather than mutating a nested
   projection or borrowing it mutably.

The typed representation should distinguish this authorization from ordinary
mutable-place assignment. One suitable shape is a field-write access kind such
as `MutablePlace | DeclaringClassCell`, attached to the typed assignment or
destination. The concrete Rust name is implementation-private, but HIR and MIR
must not forge `Mutable` on the complete receiver because later projections
could accidentally inherit that broader capability.

MIR must retain enough information for independent verification. The verifier
should check the selected field marker, exact whole-field destination,
read-only-or-mutable receiver compatibility, lexical callable owner where it
remains relevant, field type, assignment family, liveness, ownership, guards,
anchors, and cleanup. Mutated or hand-built MIR that applies cell
authorization to an ordinary field or nested destination must fail
deterministically.

Layout and backend lowering consume the same `FieldId`, type, and target
address as an ordinary assignment. Source visibility may still be erased after
authorization; the durable cell capability remains only where later trust
boundaries need to verify the exceptional write.

## PC11 — Target, runtime, and concurrency boundary

A cell field uses ordinary object storage, alignment, offsets, copy plans,
destruction plans, and target stores. The modifier adds no header word,
wrapper allocation, pointer indirection, hidden lock, borrow counter, runtime
function, public symbol, or runtime ABI revision.

The source contract does not promise that cell reads or writes are atomic,
even when the stored value fits in one machine word. It defines no
happens-before relationship, volatile access, lock-free guarantee, or race
resolution. If Skald later introduces shared-memory concurrency, atomic cells,
locks, thread confinement, and the status of ordinary data races require a
separate design. That work must not retroactively interpret `private cell` as
synchronized storage.

## PC12 — Diagnostics, dumps, and testing

Diagnostics should retain exact modifier, declaration, receiver, selected
field, and nested-projection spans. Wording remains implementation-owned, but
errors must distinguish at least:

- a cell modifier without required private visibility;
- `cell` applied to a static field, method, initializer, or lifecycle member;
- duplicate or misplaced modifiers;
- ordinary private-access failure outside the declaring class;
- assignment to an ordinary field through read-only access;
- nested mutation, a mutable method call, or `mut ref` forwarding through a
  cell reached only read-only;
- type or assignment-capability failure after cell authorization; and
- invalid cell metadata or authorization discovered by MIR verification.

Syntax and resolved dumps should display `private cell` deterministically.
Typed and MIR dumps should expose the narrow write authorization without
changing unrelated mutable/read-only receiver output.

Focused validation should include:

- parser acceptance, contextual identifier disambiguation, malformed forms,
  recovery, spans, and nesting limits;
- resolution identity, privacy, inheritance, namespace collision, and dump
  tests;
- positive read-only whole replacements for representative primitive,
  optional, exact-class, shared-owner, and array fields;
- negative nested field, array element, optional payload, mutable receiver,
  and `mut ref` uses from a read-only root;
- ordinary broader mutation through a genuinely mutable root, proving that
  cell does not remove existing permissions;
- closed generic specialization and inferred assignment requirements;
- synthesized and explicit lifecycle, self-assignment, overlap, anchors,
  guards, destruction order, and failure-path tests;
- virtual/interface and produced read-only receiver composition;
- HIR/MIR dump and verifier mutation tests;
- native golden behavior demonstrating a cache populated through a read-only
  method; and
- full compiler, golden, runtime, documentation, MSRV, determinism, and diff
  hygiene gates through the repository Makefile.

## PC13 — Standard-library adoption and delivery

The general feature should be frozen, promoted into focused living language
and compiler contracts, and divided into a PR-sized implementation roadmap
before implementation starts. The roadmap should settle syntax and field
metadata before typed write authorization, typed authorization before MIR
verification, and verified semantics before broad composition and standard
library adoption.

Adding a cached hash to `std::str::Str` is a motivating follow-up, not an
implicit part of the cell-field implementation. The canonical string language
item currently requires exactly three direct fields, and compiler-created
string literals initialize exactly `_storage`, `_start`, and `_length`.
Adopting:

```ska
private cell _hash_code: u64?;
```

therefore also requires an explicit string-language-item change:

- validate a fourth exact private cell field;
- initialize it to `none` for every compiler-materialized literal;
- preserve ordinary synthesized lifecycle for cached and uncached values;
- update resolved, HIR, MIR, verifier, backend, dump, documentation, and
  fixture metadata that currently records three fields; and
- implement and test `Str` equality and hashing independently of the general
  cell mechanism.

Keeping that adoption separate proves the language feature with ordinary
classes first and prevents compiler-known string construction from obscuring
the core authorization boundary.

## Initial exclusions

This proposal does not add:

- public, protected, module-visible, inherited-access, or interface cell
  declarations;
- static, top-level, thread-local, atomic, volatile, or synchronized cells;
- a generic `Cell<T>` library type or special methods such as `get`, `set`,
  `replace`, or `take`;
- mutable references obtained through an otherwise read-only cell path;
- propagation of cell permission into nested fields, array elements,
  optional payloads, or shared pointees;
- source-visible references, escaping aliases, properties, getters/setters,
  observers, or mutation effects in callable types;
- automatic cache invalidation, once-only initialization, recursion
  detection, poison state, or computed-field semantics;
- alternate copying, assignment, serialization, reflection, or destruction
  behavior for cell state; or
- any promise that an ordinary `fn` is pure, deterministic, const-evaluable,
  re-entrant, or safe for concurrent invocation.

## Promotion and implementation boundary

PC1 through PC13 were confirmed as one contract on 2026-08-16. The frozen
planned behavior is promoted into the focused class/access, grammar, compiler
phase, and status documentation. The separate
[implementation roadmap](PRIVATE_CELL_FIELDS_ROADMAP.md) owns delivery without
reopening these decisions.

That roadmap does not combine the general language feature with the `Str`
language-item migration. Extending the exact string descriptor and literal
materialization remains a separately approved follow-up after private cell
fields execute through the complete compiler.

# Final Fields Design Proposal

Status: frozen, implemented, and archived. FF1 through FF14 were confirmed
together on 2026-08-16 and promoted into the living language and compiler
contracts before the implementation roadmap was created. The implemented
[grammar](../language/GRAMMAR.md) and
[status matrix](../language/STATUS.md) remain authoritative for current
compiler behavior.

This proposal adds shallow `final` instance and class-owned static fields.
The immediate motivation is a primitive box that exposes its payload as a
direct public read without also exposing an independently assignable field:

```ska
public class BoxF64 implements Equatable, Hashable {
    final value: f64;

    init(value: f64) {
        self.value = value;
    }
}
```

Fields are public by default, so callers may read `box.value` without a getter.
They may not assign `box.value` as an independent field destination after
construction. A mutable complete value remains replaceable, however:

```ska
var left: BoxF64 = BoxF64(1.0);
var right: BoxF64 = BoxF64(2.0);

left.value = 3.0; // invalid: direct assignment to a final field
left = right;     // valid: replacement of the complete mutable value
```

The complete-value rule is source semantics. Skald may realize `left = right`
through synthesized or user-defined copy assignment and field-wise operations,
but those lowering details do not make a `var` binding non-replaceable.

## Intended outcome

The initial feature should provide:

- contextual `final name: T;` and `private final name: T;` instance fields;
- contextual `final static name: T = expression;` and
  `private final static name: T = expression;` class-owned fields;
- ordinary direct reads, visibility, field identity, layout, ownership, and
  lifecycle behavior;
- exact-once final-field initialization under the existing ordinary and copy
  constructor rules;
- rejection of independent final-field replacement after construction;
- continued assignment of mutable complete class values, including classes
  whose representation contains final fields;
- permission for synthesized and user-defined copy assignment to update the
  exact class's own direct final fields;
- shallow behavior for inline objects, arrays, optionals, and shared owners;
- preservation through inheritance, closed generic specialization, produced
  values, interfaces, and existing call-scoped alias rules;
- explicit typed and verified authorization for final-field writes performed
  as part of complete-value assignment;
- no field-layout, internal calling-convention, runtime, or ABI change; and
- focused diagnostics and deterministic dumps for declaration modifiers and
  every accepted or rejected write context.

This proposal does not add immutable local storage, a `let` declaration, final
parameters, final results, final array elements, immutable classes, or
transitive constness.

## Current boundary and architectural evidence

Ordinary instance fields are public by default and may be marked `private`.
The implemented `private cell` form adds a separate, narrowly verified
interior-replacement permission. Static fields use a distinct class-owned
identity and may likewise be public or private. Every static field is currently
mutable after eager initialization.

An ordinary initializer and a copy constructor operate on incomplete `self`.
Their bodies are straight-line sequences of assignments to direct fields;
every direct field must be initialized exactly once. A derived ordinary
initializer first invokes one selected base initializer. Copy construction
processes the direct base before direct fields. Skald has no synthesized
ordinary initializer, while copy construction may be synthesized when the
base and every direct field support it.

Copy assignment instead operates on a complete live object. A user-defined
`assign(ref source: T)` is a general mutable `unit` body: it may use ordinary
control flow and update any supported subset of fields. Synthesized assignment
processes the direct base and then direct fields in declaration order when
each selected operation is available. Whole-object assignment does not end or
restart the destination lifetime and already permits self-assignment.

This separation supplies the required semantic boundaries:

- construction initializes final storage exactly once;
- independent field assignment is rejected after construction; and
- selected complete-value assignment receives explicit permission to update
  final representation fields while the destination remains live.

Skald's sibling Niflheim draft uses shallow final fields but makes a class
non-synthesizably assignable when final state would change. Skald deliberately
does not adopt that restriction. In Skald, `var value: T` denotes mutable
storage for one complete inline `T` value, so `value = source` remains valid
when `T` supports its ordinary complete-value assignment operation.

## Design principles

1. **Finality belongs to a field destination.** It rejects independent
   replacement of that field; it is not a const-qualified type.
2. **Mutable complete values remain replaceable.** A field-wise lifecycle
   implementation does not redefine the source-level `destination = source`
   operation as a sequence of forbidden user field assignments.
3. **Construction remains exact.** Final fields use the same direct,
   exact-once initialization rules as ordinary fields.
4. **User assignment remains expressive.** An explicit `assign` retains its
   existing locals, control flow, calls, and arbitrary subset of updates.
5. **Assignment authorization is lexical and exact.** Only the final field's
   exact declaring-class copy-assignment lifecycle may write it directly.
6. **Finality is shallow.** It does not freeze nested inline state, array
   elements, optional payload state reached through an existing mutable view,
   or a separately allocated shared pointee.
7. **Visibility is independent.** Public and private final fields differ only
   in who may select them, not in finality or representation.
8. **Lifecycle operations remain authoritative.** Copying, owner retention,
   displaced-value release, destruction, evaluation order, and failure reuse
   the field type's existing rules.
9. **Verification records exceptional writes honestly.** The compiler must
   not erase finality early or pretend every mutable receiver can write a final
   field.
10. **No general immutable-binding feature is implied.** Local and parameter
    storage remain outside this proposal.

## Decision register

| ID | Decision | Frozen direction |
|---|---|---|
| [FF1](#ff1--source-syntax-and-contextual-spelling) | Source syntax | Add contextual final instance and final static field forms in one canonical modifier order |
| [FF2](#ff2--field-identity-visibility-and-type) | Declaration identity | Preserve the ordinary field/static identity and stored type plus one final marker and span |
| [FF3](#ff3--independent-field-assignment) | Direct writes | Reject independent replacement of a final field after initialization |
| [FF4](#ff4--complete-value-replacement) | Whole values | Keep mutable complete class values assignable even when they contain final fields |
| [FF5](#ff5--construction-and-copy-construction) | Construction | Initialize every direct final field exactly once under existing ordinary/copy constructor rules |
| [FF6](#ff6--user-defined-and-synthesized-copy-assignment) | Assignment lifecycle | Permit the exact class's explicit or synthesized copy assignment to update its own direct final fields |
| [FF7](#ff7--shallow-state-and-nested-values) | Depth | Pin only the selected field slot, not recursively reachable or contained mutable state |
| [FF8](#ff8--static-final-fields) | Class-owned storage | Require an explicit eager initializer and reject every later root assignment |
| [FF9](#ff9--inheritance-generics-dispatch-and-visibility) | Composition | Preserve exact ownership, specialization, lookup, dispatch, and privacy rules |
| [FF10](#ff10--aliases-produced-values-and-observation) | Aliasing | Preserve current alias eligibility while granting no replacement-capable alias to a final slot |
| [FF11](#ff11--compiler-representation-and-verification) | Compiler boundary | Carry final metadata and exact construction/assignment write authorization through verified IR |
| [FF12](#ff12--target-runtime-and-optimization-boundary) | Realization | Reuse ordinary field layout and lifecycle lowering without a runtime or ABI feature |
| [FF13](#ff13--diagnostics-dumps-and-testing) | Quality | Cover modifier recovery, every write boundary, lifecycle composition, and malformed products |
| [FF14](#ff14--standard-library-adoption-and-promotion) | Delivery | Adopt final fields only after the general contract is frozen and implemented |

## FF1 — Source syntax and contextual spelling

The frozen instance forms are:

```ska
final value: f64;
private final token: u64;
```

The frozen static forms are:

```ska
final static EMPTY_HASH: u64 = 0x7374725f656d7074u;
private final static DOMAIN: u64 = 0x1234u;
```

The canonical grammar direction is:

```text
field-declaration
    = ["private"] identifier ":" storage-type ";"
    | ["private"] "final" identifier ":" storage-type ";"
    | "private" "cell" identifier ":" storage-type ";"

static-field-declaration
    = ["private"] "static" identifier ":"
      storage-type ["=" expression] ";"
    | ["private"] "final" "static" identifier ":"
      storage-type "=" expression ";"
```

`final` remains contextual. The modifier form is recognized only when the
complete declaration lookahead matches. Existing identifier positions remain
available:

```ska
final: i64;                 // public ordinary field named `final`
private final: i64;         // private ordinary field named `final`
final static: i64;          // public final instance field named `static`
static final: i64;          // public mutable static field named `final`
private final cell: i64;    // private final instance field named `cell`
```

Reordered, duplicated, incomplete, or cross-category modifiers receive focused
syntax diagnostics. In particular, `static final name`, `final private name`,
and a combined `private final cell name` modifier form are invalid. The
spelling `final` remains available for methods, functions, parameters, locals,
types, and other existing identifier positions.

Finality applies only to stored instance and class-owned static fields. It is
invalid on methods, ordinary initializers, copy operations, destructors,
interfaces, or other declarations. This proposal defines no local or
parameter syntax.

## FF2 — Field identity, visibility, and type

A final instance field remains one ordinary field with:

- its existing dense `FieldId`, declaring `ClassId`, name, and declaration
  order;
- ordinary public-default or explicit private visibility;
- its unchanged stored type;
- one final marker and exact modifier span; and
- every ordinary containment, layout, lifecycle, and member-namespace rule.

A final static field analogously retains one ordinary `StaticFieldId`,
declaring class, generic-application identity, selected type, visibility, and
initializer identity plus the final marker and span.

Finality is not part of type identity. Reading a `final value: f64` produces an
ordinary `f64`; reading a final `Item` or owner uses the same place, copy,
alias, and ownership rules as reading the equivalent mutable field. Finality
does not create `final T`, `const T`, a wrapper allocation, or a distinct
calling-convention representation.

`private` and `final` are orthogonal. A public final field is directly readable
where its class is accessible. A private final field first applies ordinary
declaring-class privacy. Neither visibility form grants an assignment
exception.

`final` and `cell` are mutually exclusive capabilities. A cell exists to
authorize selected replacement outside ordinary mutable access, while finality
forbids independent replacement after construction.

## FF3 — Independent field assignment

After construction, a final field cannot be selected as the destination of an
ordinary field-assignment operation:

```ska
class BoxF64 {
    final value: f64;

    init(value: f64) {
        self.value = value;
    }

    mut fn reset() -> unit {
        self.value = 0.0; // invalid
    }
}

fn reset(mut ref box: BoxF64) -> unit {
    box.value = 0.0; // invalid
}
```

Mutable receiver access is necessary for ordinary field replacement but no
longer sufficient when the selected field is final. The prohibition applies
equally outside the class, in ordinary instance or static methods, in a
destructor, and in a derived or unrelated class. Private visibility does not
grant the declaring class a final-field setter.

The restriction is tied to the selected final field. Assigning a mutable
ancestor complete value is a different operation owned by FF4. Automatic
destruction or release when an object's lifetime ends is likewise lifecycle
cleanup rather than a source assignment to the final field.

## FF4 — Complete-value replacement

A mutable complete class destination remains replaceable:

```ska
var left: BoxF64 = BoxF64(1.0);
var right: BoxF64 = BoxF64(2.0);
left = right; // valid
```

The destination selection, exact-class source rules, source evaluation,
self-assignment behavior, temporary lifetime, and selected copy-assignment
operation remain unchanged. The destination lifetime does not end or restart.
Final fields may change as representation state of the selected complete-value
assignment.

The same distinction composes through fields:

```ska
class Holder {
    final box: BoxF64;

    init(box: BoxF64) {
        self.box = box;
    }
}

var holder: Holder = Holder(BoxF64(1.0));
var other: Holder = Holder(BoxF64(2.0));

holder.box = BoxF64(3.0); // invalid: direct final-field replacement
holder = other;           // valid: complete Holder replacement
```

If an outer field is mutable, assigning that complete outer field may invoke
the contained class's assignment operation, which may update its own final
representation. Finality therefore does not remove copy-assignment capability
from a class or recursively from classes that contain it.

This rule does not make a read-only root mutable, add whole-object replacement
through an alias, or make complete `self` assignable inside its own member
body. Existing destination restrictions remain authoritative.

## FF5 — Construction and copy construction

Final fields use the existing incomplete-object contract without an additional
write-once analysis:

- every direct field, final or mutable, is initialized exactly once;
- user ordinary and copy constructors remain straight-line direct-field
  initialization bodies without locals or control flow;
- fields may be initialized in any source order;
- an uninitialized field and incomplete complete `self` remain unreadable;
- an initialized direct field may be used by later initialization expressions;
- a derived ordinary initializer calls one selected base initializer first;
- copy construction processes the direct base before direct fields; and
- the object becomes live only after all direct fields are initialized.

The exact declaring-class constructor initializes its own direct final fields.
A derived constructor does not directly initialize inherited final fields; the
selected base lifecycle owns them.

Synthesized copy construction includes final fields in ordinary declaration
order and remains available whenever the base and every field support copy
construction. Finality does not itself remove that capability. Skald continues
to have no synthesized ordinary initializer.

## FF6 — User-defined and synthesized copy assignment

Both selected copy-assignment forms may update final representation fields.

A synthesized assignment processes the base and then every direct field in
declaration order. Its generated field operations receive complete-value
assignment authorization, including operations targeting final fields.

A user-defined `assign(ref source: T)` retains all of its current freedom:

- `self` is complete, live, and mutable;
- locals, nested blocks, conditionals, loops, calls, and supported returns
  remain available;
- any supported subset of mutable or final direct fields may be updated;
- a final field may be assigned zero, one, or multiple times;
- writes may be conditional or occur in loops; and
- a compatible value may come from any ordinary expression rather than the
  corresponding source field.

For example:

```ska
assign(ref source: BoxF64) {
    if (source.value == 0.0) {
        self.value = 0.0;
    }
    else {
        self.value = source.value;
    }
}
```

The permission is lexical and exact. It applies only to direct final fields
declared by the class owning that copy-assignment lifecycle. It does not flow
dynamically into helper methods called by `assign`, and it does not authorize a
direct write to an inherited or nested class's final field:

```ska
assign(ref source: Outer) {
    self.inner = source.inner;              // valid complete Inner assignment
    self.inner.value = source.inner.value;  // invalid if Inner.value is final
}
```

Base assignment handles base final fields before the derived body. Assigning a
complete nested mutable field selects that nested class's assignment, whose
own lifecycle receives its exact authorization.

An explicit assignment body is not required to make the destination equal to
the source or update every final field. User copy assignment already defines
the class's custom assignment semantics and may preserve, normalize, or
recompute any supported subset of ordinary state.

## FF7 — Shallow state and nested values

Finality prevents replacement of one selected stored field; it does not
recursively freeze the field type or reachable graph.

Given mutable access to the containing object:

- a final inline class field may call its mutable methods and update its own
  non-final nested fields;
- a final inline array field may update elements and slices where ordinary
  array access permits, but its descriptor/backing value cannot be replaced;
- a final optional cannot be cleared, injected, or replaced as a container,
  while an already-present class payload may be mutated through an existing
  supported mutable checked view;
- a final shared field cannot replace its owner handle, while the separately
  allocated pointee remains mutable through ordinary explicit dereference; and
- a final function or scalar field is simply not independently replaceable.

Read-only roots remain read-only. Finality never upgrades receiver access or
authorizes nested mutation that the equivalent mutable field and root could
not already perform.

Replacing an enclosing mutable complete value may change final state observed
through another overlapping path. Finality provides no snapshot, identity,
exclusivity, or concurrency guarantee.

## FF8 — Static final fields

A final static field must declare one explicit initializer:

```ska
class Limits {
    final static MAXIMUM: u64 = 100u;
    private final static DOMAIN: u64 = 0x1234u;

    init() {}
}
```

The initializer uses the ordinary eager static-initialization expression,
dependency, publication, ownership, and failure contract. Generated startup
initialization is the sole root write. Initializer-free final statics are
invalid rather than treating zero-filled storage as an implicit write-once
value.

After publication, `Class.field = value` is invalid from every source context,
including bodies owned by the declaring class. A final static is itself the
root destination and has no mutable enclosing class value whose assignment
could replace it.

Final static state remains shallow. A final inline object static may mutate
nested non-final state through ordinary methods, a final array may mutate
elements, and a final shared owner may mutate its pointee. Normal reverse
static shutdown still destroys or releases the stored value; cleanup is not a
source replacement.

A mutable static class field whose stored class contains final instance fields
remains assignable as a complete value. That selected class copy assignment
may update its contained final representation under FF4 and FF6.

## FF9 — Inheritance, generics, dispatch, and visibility

Final fields retain ordinary nearest-member lookup, collision, no-hiding, and
inherited identity. A derived class may read an inherited public final field
but may not assign it directly. A private final field remains selectable only
from its exact declaring class under ordinary privacy; finality then applies
independently.

Base ordinary initialization, base copy construction, and base copy assignment
own base final state. Derived lifecycle bodies receive no direct authorization
for inherited final fields.

Closed generic specialization substitutes the stored type and preserves the
final marker, modifier evidence, declaration order, and exact application-owned
static identity. Each closed final static owns its own eagerly initialized
slot under the existing generic static rules.

Finality changes no virtual slot, method override, interface conformance,
dispatch signature, produced-object identity, or checked-cast rule. It is a
stored-destination restriction rather than a callable modifier.

## FF10 — Aliases, produced values, and observation

Ordinary reads and read-only alias uses remain unchanged. Existing aliases are
non-owning, call-scoped, non-exclusive, and non-escaping; finality adds no new
reference value or lifetime.

An alias operation is permitted only when its existing capability cannot
replace the selected final field slot. For example, a mutable alias to an
inline object may continue to mutate that object's nested non-final state when
whole-object replacement through the alias is already unsupported. No alias
form may be introduced or reinterpreted as a way to rebind a final scalar,
owner, optional container, array descriptor, or complete inline field.

Produced objects remain read-only roots. Their public final fields may be read
through the existing produced-field path, but finality grants no mutable
produced receiver, write, or escaping alias.

Because aliases are non-exclusive, complete replacement of an enclosing
mutable value may be observed through another valid path under the existing
overlap and anchor rules. Finality does not promise stable observation across
ancestor replacement.

## FF11 — Compiler representation and verification

Syntax should retain the exact `final` span. Resolution should attach final
metadata to canonical instance/static declarations before privacy and write
checking. Closed specialization must copy the marker while allocating normal
specialized identities.

HIR should distinguish at least these semantic write categories:

- construction initialization of incomplete direct field storage;
- ordinary mutable field assignment;
- declaring-class private-cell assignment; and
- exact declaring-class complete-value-assignment authorization for a final
  field.

The final-field authorization must identify the selected `FieldId` and owning
copy-assignment lifecycle. It must not be represented by clearing the final
marker, upgrading all mutable access, or granting the declaring class a
general setter. Synthesized assignment operations need equivalent exact
evidence even when represented as selected field lifecycle steps rather than
one source body.

Preliminary and final MIR verification should independently prove that:

- every final marker names a declared ordinary field/static identity;
- constructor initialization targets incomplete direct storage owned by the
  exact lifecycle and continues to satisfy exact-once initialization;
- a post-construction final-field write occurs only in the exact declaring
  class's selected user or synthesized copy-assignment operation;
- inherited and nested writes use their own selected lifecycle rather than
  forged outer authorization;
- ordinary methods, destructors, static writes, and unrelated operations carry
  no final-field authorization;
- final static storage has exactly one explicit planned publication and no
  ordinary write; and
- ownership, optional, array, shared, lifetime, cleanup, and assignment
  metadata remain valid for the underlying stored type.

Verified dumps should expose final declaration metadata and the reason an
otherwise-forbidden write is accepted. The backend should consume only final
verified MIR and never reconstruct authorization from names or source shape.

## FF12 — Target, runtime, and optimization boundary

Final and mutable fields of the same stored type have identical size,
alignment, offsets, representations, calling conventions, ownership headers,
and destruction requirements. The backend lowers verified constructor and
assignment operations through existing type-directed machinery.

The feature adds no runtime flag, write barrier, wrapper allocation, dynamic
const check, borrow counter, synchronization, atomicity, volatility, runtime
service, public symbol, or ABI revision.

Direct public field access avoids an ordinary source-level getter call. Any
further constant propagation, load elimination, or inlining benefit is an
optimizer opportunity, not a source guarantee. Ancestor complete-value
replacement and shallow nested mutation prevent the compiler from treating a
final field as globally invariant without a separate proof.

## FF13 — Diagnostics, dumps, and testing

Focused diagnostics should cover:

- malformed, reordered, duplicated, or unsupported modifier combinations;
- a missing initializer on a final static field;
- direct assignment to public and private final fields through mutable roots;
- attempts from the declaring class, derived classes, ordinary methods,
  static methods, destructors, helpers called by `assign`, and unrelated code;
- direct inherited and nested final-field writes from an outer `assign`;
- valid ordinary/copy initialization and valid exact user/synthesized
  assignment authorization; and
- attempts to apply `final` outside instance/static field declarations.

Deterministic phase tests should cover syntax recovery, declaration identity,
visibility, inheritance, generic specialization, HIR/MIR dumps, final-write
authorization, malformed intermediate products, static lifecycle planning,
and backend trust-boundary rejection.

Native source-to-observation tests should cover:

- direct reads without getters;
- ordinary and copy construction;
- `var` assignment between final-bearing values;
- fresh-source and self-assignment;
- explicit assignment with zero, repeated, conditional, and loop-carried
  final writes;
- nested complete assignment versus rejected nested direct writes;
- base and derived lifecycle composition;
- primitive, exact-class, optional, array, shared-owner, and function-valued
  field families where currently supported;
- public/private final statics, dependency ordering, shallow mutation, and
  normal shutdown; and
- aliases, produced reads, virtual/interface calls, and closed generics.

No test should infer finality from a physical layout or require a new runtime
symbol.

## FF14 — Standard-library adoption and promotion

The first standard-library consumer should be `BoxF64`:

```ska
public class BoxF64 implements Equatable, Hashable {
    final value: f64;

    init(value: f64) {
        self.value = value;
    }

    fn equals(ref other: Obj) -> bool {
        if (!(other is BoxF64)) {
            return false;
        }
        return _to_bits(self.value) == _to_bits(((BoxF64) other).value);
    }

    fn hash_code() -> u64 {
        return mix_u64(_to_bits(self.value) ^ 0x6636345f626f7801u);
    }
}
```

Other primitive box classes may reuse the same surface after their own binary
equality and domain-separated hashing contracts are defined. Standard-library
adoption must follow the general feature rather than introducing a compiler
exception for boxes.

Promotion updated the authoritative class lifecycle, grammar, static field,
compiler phase, debugging, testing, and status documents before implementation
began. The [implementation roadmap](FINAL_FIELDS_ROADMAP.md) divides
syntax/metadata, instance write semantics, copy-assignment authorization,
static lifecycle, native composition, and standard-library migration by
stable compiler boundaries rather than restating this design.

## Initial exclusions

The initial final-field feature does not define:

- immutable local bindings or any `let` syntax;
- final parameters, results, captures, array elements, optional payload
  declarations, or top-level/module storage;
- immutable or frozen classes, deep constness, transitive object-graph
  immutability, or a const-qualified type;
- final methods, classes, virtual families, interface requirements, ordinary
  initializers, copy operations, or destructors;
- final private cells or any other combined cell/final capability;
- declaration initializers for instance fields;
- lazy, thread-local, atomic, synchronized, volatile, or externally supplied
  final storage;
- compile-time constant evaluation or embedding for final statics;
- a guarantee that final state is unchanged by assignment of an enclosing
  mutable complete value;
- alias exclusivity, stable snapshots, runtime borrowing, or data-race safety;
  or
- optimizer guarantees beyond ordinary direct field access.

## Frozen delivery boundary

FF1 through FF14 are frozen together because syntax, complete-value
replacement, explicit copy-assignment freedom, shallow state, and verified
write authorization depend on one another. Changes to those decisions require
an explicit design revision rather than an implementation-task shortcut.

Freezing and promotion do not make the syntax executable. The planned
[implementation roadmap](FINAL_FIELDS_ROADMAP.md) owns staged delivery. Until
its relevant tasks complete, the implemented grammar and current mutable
instance and static field behavior remain authoritative.

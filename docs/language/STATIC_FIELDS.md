# Static Fields

Status: **complete zero-default storage profile implemented; declaration
initializer syntax through verified lifecycle MIR implemented**. This document is
authoritative for the current source-visible static-field profile. The
[status matrix](STATUS.md) remains authoritative for compiler availability,
and the [implemented grammar](GRAMMAR.md) remains the exact syntax accepted by
the current compiler.

Static fields are mutable class-owned places. Initializer-free declarations
use the implemented zero-default profile: their initial live value is
established without running Skald code and their type must admit one complete
all-zero value. Declaration initializer expressions are accepted, resolved,
type-checked as direct stored-value initialization, and lowered to structurally
verified preliminary lifecycle MIR, planned into an explicit lifecycle schema,
and checked with a target-independent effect/dependency certificate. The
compiler deliberately stops before coordinator synthesis and executable
initialized-field output.

The compiler parses static-field declarations, assigns independent resolved
identities, includes them in the inherited member namespace, validates the
complete zero-default storage-type set, and lowers primitive, inline-optional,
optional shared-owner, and inline-array operations to receiver-free typed HIR
and MIR places. It also retains and resolves optional declaration initializer
expressions under stable identities, selects their ordinary stored-value
operations, and retains those operations in typed HIR. These expressions do
not yet execute; preliminary MIR makes their calls, temporaries, ownership
operations, cleanup, and publication boundary available to whole-program
static-effect inference. That analysis conservatively summarizes direct
and deep static uses across calls, dynamic dispatch, copy operations,
destructors, shared releases, optionals, and arrays. Static-lifetime planning
then includes eventual-value destruction of every owning-capable field,
rejects self-dependencies and cycles, and selects one deterministic activation
order with exact-reverse shutdown. The plan does not yet execute.

## Declaration syntax

A static field is a direct class member:

```ska
class Metrics {
    static requests: u64;
    private static enabled: bool;

    init() {}

    static fn reset() -> unit {
        Metrics.requests = 0u;
        Metrics.enabled = false;
    }
}
```

The selected forms are exactly:

```text
static name: T;
private static name: T;
static name: T = expression;
private static name: T = expression;
```

Every static field is mutable; `final`, constant, lazy, thread-local, and
externally supplied variants are not part of this profile.

`static` remains contextual. In a class member, `static` followed by an
identifier and `:` begins a static-field declaration, while `static:` begins
an ordinary instance field whose name is `static`. The same spelling remains
available for methods, functions, parameters, locals, and other existing
identifier positions. `private static:` is likewise a private instance field
named `static`; `private static name:` is a private static field.

## Declaration initializer resolution

An explicit initializer uses the ordinary expression grammar. The compiler
assigns it a callable-like identity derived from the declaration's canonical
static-field identity, then resolves it after all modules, declarations,
class hierarchies, members, overload candidates, string language items, and
static identities are available. Forward declarations and imported or
selectively imported declarations therefore use the same resolution rules as
ordinary expressions.

The declaring class is the initializer's lexical privacy owner, so its private
static members are accessible. The context has no object receiver,
parameters, locals, or base-initialization capability: `self`, `super`, and
bare instance or static member lookup are invalid. Static members must be
selected through a class spelling. Inherited selection retains the declaring
field identity and creates neither another initializer nor another storage
slot.

Resolution retains canonical static-field uses, selected calls and dispatch
families, and source spans for later dependency analysis. Type checking then
uses the ordinary stored-value initialization machinery in a receiver-free,
parameter-free context with the same declaring-class privacy. It retains one
initializer identity, destination type, selected construction, copy or owner
transfer, and full-expression ownership plan in HIR. Neither phase infers
initialization order or makes the expression executable.

## Identity, namespace, and inheritance

Each static declaration owns exactly one storage location. Its identity is
separate from every instance-field and method identity, and adding a static
field does not change object layout or renumber instance fields.

Static fields participate in the ordinary non-overloaded class-member
namespace. A direct declaration therefore collides with a direct or inherited
instance field, instance method, static method, or static field of the same
name. Lifecycle declarations retain their existing dedicated slots. A derived
class cannot hide or redeclare an inherited static field.

Selecting an inherited static field through a derived class identifies the
same base declaration and the same storage:

```ska
class Base {
    static count: u64;
    init() {}
}

class Derived extends Base {
    init() { super(); }
}

fn update() -> unit {
    // These designate one Base-owned place.
    Base.count = 1u;
    Derived.count = 2u;
}
```

There is never one slot per derived class, object, construction, or module
binding.

## Selection and access

A static field is selected explicitly through a class declaration spelling:

```ska
Class.name
module_binding::Class.name
```

The qualified module and class lookup rules are the same as for static
methods. A module binding must be directly imported, the class must be visible
from the using module, and qualification does not bypass class-member privacy.
The class spelling is a declaration path, not an expression: selecting a
static field evaluates no receiver and produces no receiver-side effects.

Static fields do not participate in bare value lookup. Code uses `Class.name`
even from a body owned by `Class`. Existing lexical lookup still applies to
the class spelling: a parameter or local named `Class` shadows that
unqualified class name, so `Class.name` is then object selection rather than
static selection. A module-qualified spelling remains available when its
module binding is in scope.

Object-qualified static access is invalid. Class-qualified instance-field or
instance-method access is also invalid. A static field is a place and value
source where its stored type permits one; it is not callable. These wrong-kind
uses are diagnosed rather than redirected to another member category.

The existing declaring-class privacy rule applies after member selection. A
private static field is accessible exactly from a callable lexically owned by
its declaring class, including its static methods, instance methods,
initializers, copy constructor, copy assignment, and destructor. A derived
class, unrelated class, top-level function, same-module caller, or importer
receives no additional access.

Every static field is mutable program-owned storage. Mutation requires no
instance receiver and no `mut` receiver capability. Existing compatible `ref`
and `mut ref` parameters may borrow a static place for one call; aliases remain
non-owning, call-scoped, and unable to escape or become stored values.

## Initializer-free storage types and initial values

The complete initializer-free profile is:

| Static field type | Value before Skald entry begins |
|---|---|
| `i64` | `0` |
| `u64` | `0u` |
| `u8` | `0u8` |
| `f64` | Positive binary64 zero, with every representation bit clear |
| `bool` | `false` |
| Primitive `T?` | `none`; no payload is live |
| Exact-class `T?` | `none`; no payload object is live |
| `shared? T` for any currently supported shared target | `none`; no strong owner exists |
| Inline `T[]` for any legal array element type | The allocation-free empty array value, equivalent to `T[]()` |

An empty inline array constructs no elements. Its element type therefore need
not support default construction. In particular, a static `u8[]` is a valid
empty buffer and later composes with the ordinary array alias and standard-I/O
rules.

The following are invalid static-field types because zero-filled storage is
not one complete valid value under their existing contracts:

- an exact inline class `T`;
- a non-optional owner `shared T`, including `shared T[]`;
- a bare interface or `Obj` view;
- `unit`; and
- any other type not listed in the supported table.

For initializer-free declarations, this restriction does not make ordinary
exact objects nullable, permit zero as a `shared T` handle, default-construct
a class, or invent a foreign representation. Such a declaration using an
unsupported storage type is rejected at its type rather than acquiring an
implicit initializer.

## Explicit initializer types and typed semantics

An explicit initializer permits the ordinary stored field types: all five
primitives, exact inline classes, supported inline optionals, non-optional and
optional shared owners, and inline arrays. Strings use their ordinary exact
class and language-item behavior. `unit`, a bare interface, a bare `Obj` view,
aliases, and otherwise unsupported stored forms remain invalid.

The declaration is direct initialization of previously uninitialized program
storage, not assignment to a zero/default value. Consequently, it uses the
same target-directed rules as an instance-field initializer: an ungrouped
matching constructor or object-returning call can produce the exact object
directly; a named or grouped exact object selects copy construction; shared
production adopts an owner while named shared storage copies it; optional and
array values retain their ordinary presence, element, copy, adoption, and
cleanup rules. Constructor overload selection, privacy, type mismatch, and
unavailable-copy diagnostics are likewise the ordinary ones.

The expression is evaluated once in source order within one complete
full-expression plan. Typed HIR retains canonical static sources, calls,
constructor and copy selections, temporary ownership, and source spans.
Preliminary MIR then lowers the selected operations through the ordinary body
machinery and places full-expression cleanup after an explicit destination
publication edge. Dependency analysis therefore scans executable operations
instead of reconstructing source intent. This milestone does not yet define or
run an activation order.

## Reads, writes, and replacement

Primitive reads and writes use their ordinary value behavior. Optional,
shared-optional, and array operations retain their existing copy, adopt,
assignment, guard, alias, and ownership rules. Static storage adds a place
root, not a second family of raw or unchecked operations.

Replacing a static value performs the ordinary operation required by its
type. Consequently:

- replacing or clearing a present exact-class optional assigns or destroys
  its old payload under the existing optional lifecycle matrix;
- replacing a present optional shared owner secures the incoming owner before
  releasing the displaced owner; and
- replacing a nonempty inline array releases its displaced elements and
  backing under the ordinary array rules.

Checked optional payload views, shared pointee views, array elements, slices,
and call aliases retain their current guards and backing or owner anchors when
the static container could be replaced later in the same full expression.
Existing operator, cast, control-flow, call, and I/O behavior is unchanged
when its operand or buffer happens to come from a static place.

## Initialization and lifetime

Every initializer-free supported static field in the reachable program is
live with the value listed above before the selected Skald entry function
begins. All such slots become available simultaneously. No source code runs
to establish them, so there is no declaration order, class order, module
order, import order, dependency order, or lazy first-use order to observe.
Explicitly initialized declarations cannot yet reach executable compilation;
after producing preliminary lifecycle MIR, the compiler constructs and verifies
explicit lifecycle definitions, begin/publish/destroy transitions,
deterministic transitive static-effect summaries, conservative dynamic targets,
source-facing witnesses, evidenced lifetime dependencies, and the complete
activation and reverse-shutdown plan. The driver then reports `DRV001` rather
than silently discarding the initializer or emitting zero-only assembly.
`DRV001` marks unavailable coordinator synthesis rather than unavailable
dependency planning or lifecycle verification.

Planning rejects an initializer that can directly or transitively access its
own field before publication. Cleanup proven to occur after publication may
use the newly live field. If initialization or eventual-value destruction of
field `F` may access field `T`, `T` must activate before `F`; a self-edge or
cycle is diagnosed as `STA001` or `STA002`. This includes destruction of an
initializer-free optional, shared-owner, or array field whose owning contents
could be installed by ordinary replacement. Callable recursion alone is not a
field-lifetime cycle.

A static slot has process lifetime. It is not registered in any lexical scope,
does not begin or end lifetime on a function call, and is not cleaned when the
selected Skald entry function or generated host entry returns. The final
contents remain live until process termination. In particular, a final
present optional object, optional shared owner, or nonempty inline-array
backing is deliberately not destroyed or released by generated shutdown code.
The host operating system may reclaim process resources without invoking
Skald lifecycle operations.

This final no-cleanup rule does not suppress ordinary replacement effects
during execution. It also does not change cleanup of locals, parameters,
results, temporaries, instance fields, or full-expression anchors.

## Failure and diagnostics

Declaring or selecting a static field introduces no runtime failure of its
own. Existing operations retain their existing failures: for example, absent
optional unwrap, invalid array access, allocation failure, and ownership-count
overflow behave exactly as they do for non-static storage.

The implementation must reject each error at the phase that owns it:

- malformed modifiers or declaration shape at syntax analysis;
- unsupported stored types at type checking, on the declared type;
- direct or inherited member collisions during class namespace resolution;
- inaccessible private selections after resolving the declaration;
- object-selected static fields and class-selected instance members as
  wrong-kind selections; and
- attempts to call a static field as a non-callable target.

Diagnostic wording and codes remain compiler behavior. Malformed declarations
are syntax errors, namespace and privacy rules are enforced during resolution,
and `TYP042` rejects either an initializer-free declaration whose type lacks a
complete all-zero live value or an explicit declaration whose type cannot
store a value. Explicit expressions otherwise use ordinary type, overload,
privacy, copy-capability, and ownership diagnostics. `STA001` and `STA002`
report static lifetime self-dependencies and cycles with declaration, access,
and transitive call/lifecycle evidence. `DRV001` marks the current
post-planning lifecycle-synthesis boundary. Every initializer-free declaration
accepted by zero-default validation can be used through its documented
primitive, inline-optional, optional shared-owner, or inline-array operations
and reaches verified MIR and native execution.

## Runtime, ABI, and representation boundary

Static fields add no public C symbol, runtime call, allocator behavior, panic
reason, garbage-collector root, trace operation, startup hook, shutdown hook,
or process-wrapper lifecycle step. Runtime ABI version 8 and its compatibility
marker remain unchanged. Source `public` visibility does not export a native
static symbol, and static fields are not permitted in external declarations.

For every initializer-free declaration that reaches the backend, the x86-64
implementation emits one deterministic, target-private, writable, aligned,
zero-filled slot and addresses it through ordinary verified typed places.
Symbol spelling, section choice, alignment calculation, relocation form,
compiler identities, IR, and layout are implementation details. They must
preserve one canonical slot per declaration, the specified initial value and
lifetime, deterministic output, and the absence of object-layout or
callable-ABI changes.

## Exclusions

The executable profile does not yet include:

- executable declaration initialization, dependency ordering, or arbitrary
  constant evaluation;
- static initializer blocks, lifecycle members, or deinitializers;
- module initialization or shutdown and their ordering;
- executable exact inline-class or non-optional shared-owner static
  initialization;
- top-level or module-owned global variables;
- interface-owned static fields;
- external, exported, thread-local, atomic, synchronized, `final`, or constant
  static storage;
- reflection or source-visible static symbol identity;
- garbage collection or runtime root registration; or
- cleanup of final static contents at normal process exit.

The active [static-initialization roadmap](../roadmaps/STATIC_FIELD_INITIALIZATION_ROADMAP.md)
owns lifecycle MIR lowering, dependency ordering, eager startup, and reverse
normal-return shutdown items above. Extending the remaining boundaries
requires a separate design rather than an inference from either the
zero-default profile or initializer syntax.

# Zero-Default Static Fields

Status: **frozen design; primitive execution implemented**. This document is authoritative for
the selected source-visible static-field profile. The
[status matrix](STATUS.md) remains authoritative for compiler availability,
and the [implemented grammar](GRAMMAR.md) remains the exact syntax accepted by
the current compiler.

Static fields are mutable class-owned places whose initial live value is
established without running Skald code. This initial profile deliberately
supports only types for which zero-filled storage is already one complete
valid value. It therefore adds useful counters, flags, optional caches, and
inline-array registries without introducing declaration expressions, module
initialization, shutdown ordering, garbage collection, or a new runtime
service.

The compiler parses static-field declarations, assigns independent resolved
identities, includes them in the inherited member namespace, validates the
complete zero-default storage-type set, and lowers primitive reads, writes,
and call aliases to receiver-free typed HIR and MIR places. Primitive statics
execute through deterministic native storage. Owning zero-default declarations
are accepted but their source uses remain staged.

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
```

There is no initializer after the type and no separate static initializer.
Every static field is mutable; `final`, constant, lazy, thread-local, and
externally supplied variants are not part of this profile.

`static` remains contextual. In a class member, `static` followed by an
identifier and `:` begins a static-field declaration, while `static:` begins
an ordinary instance field whose name is `static`. The same spelling remains
available for methods, functions, parameters, locals, and other existing
identifier positions. `private static:` is likewise a private instance field
named `static`; `private static name:` is a private static field.

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

## Supported storage types and initial values

The complete initial profile is:

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

This restriction does not make ordinary exact objects nullable, permit zero
as a `shared T` handle, default-construct a class, or invent a foreign
representation. A declaration using an unsupported storage type is rejected
at its type rather than acquiring an implicit initializer.

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

Every supported static field in the reachable program is live with the value
listed above before the selected Skald entry function begins. All slots become
available simultaneously. No source code runs to establish them, so there is
no declaration order, class order, module order, import order, dependency
order, or lazy first-use order to observe.

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

Diagnostic wording and codes remain compiler behavior. During the current
implemented primitive stage, malformed declarations are syntax errors, namespace and
privacy rules are enforced during resolution, `TYP042` rejects a declaration
whose type lacks a complete all-zero live value, and `TYP043` rejects source
use of an accepted owning static type until its ownership-specific stage.
Primitive static programs reach verified MIR and native execution. `TYP043`
continues to reserve accepted optional, shared-owner, and array uses for their
ownership-specific roadmap stages.

## Runtime, ABI, and representation boundary

Static fields add no public C symbol, runtime call, allocator behavior, panic
reason, garbage-collector root, trace operation, startup hook, shutdown hook,
or process-wrapper lifecycle step. Runtime ABI version 8 and its compatibility
marker remain unchanged. Source `public` visibility does not export a native
static symbol, and static fields are not permitted in external declarations.

The x86-64 implementation emits one deterministic,
target-private, writable, aligned, zero-filled slot per declaration and to
address it through ordinary verified typed places. Symbol spelling, section
choice, alignment calculation, relocation form, compiler identities, IR, and
layout are implementation details. They must preserve one canonical slot per
declaration, the specified initial value and lifetime, deterministic output,
and the absence of object-layout or callable-ABI changes.

## Exclusions

This frozen profile does not include:

- declaration initializers or arbitrary constant evaluation;
- static initializer blocks, lifecycle members, or deinitializers;
- module initialization or shutdown and their ordering;
- exact inline-class or non-optional shared-owner static initialization;
- top-level or module-owned global variables;
- interface-owned static fields;
- external, exported, thread-local, atomic, synchronized, `final`, or constant
  static storage;
- reflection or source-visible static symbol identity;
- garbage collection or runtime root registration; or
- cleanup of final static contents at normal process exit.

These exclusions are settled boundaries of the initial profile. Extending one
requires a separate design rather than an inference from zero-default static
storage.

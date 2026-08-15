# Capture-Free Function Values

Status: frozen design; syntax, canonical closed type identity, and eligible
ordinary callable-reference resolution implemented behind a type-check gate. The
[status matrix](STATUS.md) is authoritative for availability, and the
[implemented grammar](GRAMMAR.md) is the exact accepted source shape. Function
references can be inspected in resolved compiler output, but storage,
transport, indirect calls, and execution remain unavailable. The active
[implementation roadmap](../roadmaps/FUNCTION_VALUES_ROADMAP.md) owns delivery.

This contract adds non-null, capture-free function values to Skald. A function
value names one exact internal top-level function or static method and carries
no receiver or captured environment. It can be stored, copied, transported,
returned, and called through the same internal argument, result, ownership,
and cleanup rules as a direct call.

## Function types

The frozen recursive type syntax is:

```text
function-type           = "fn" "(" [function-type-parameter
                          {"," function-type-parameter}] ")"
                          "->" storage-type
function-type-parameter = storage-type
                        | "ref" storage-type
                        | "mut" "ref" storage-type
```

Parameter names do not occur in a function type. Parameter mode, parameter
type, order, arity, and result type all participate in exact type identity:

```ska
fn() -> unit
fn(i64, f64) -> bool
fn(ref Item, mut ref Counter) -> unit
fn(fn(i64) -> bool, i64) -> bool
fn(i64) -> fn(bool) -> unit
```

Function types are invariant. There is no parameter contravariance, result
covariance, mode adaptation, currying, argument omission, implicit receiver
binding, or generated wrapper. Grouping, import spelling, source spans, and
optional shorthand do not create different identities.

A function may return an array or optional value, but the initial feature does
not admit a function value as an array element or optional payload:

```text
fn() -> i64[]       frozen and supported by the initial design
fn() -> i64?        frozen and supported by the initial design
(fn() -> i64)[]     excluded initially
(fn() -> i64)?      excluded initially
```

## Forming values and access

A value may be formed from an accessible internal top-level function or an
accessible static method on an ordinary class or closed generic-class
specialization:

```ska
var parse: fn(Str) -> i64 = parse_value;
var imported: fn(i64) -> bool = util::accept;
var ordinary: fn(i64) -> i64 = Math.increment;
var specialized: fn(i64) -> i64 = Identity<i64>::apply;
```

The current compiler resolves all four forms to an exact target and signature,
records the target as address-taken, and then stops the program at type
checking. Generic function signatures are closed recursively, so different
specializations retain different method targets even when they share one
canonical function type. No resolved reference is executable yet.

The source name selects one exact callable identity and its canonical
signature. An expected function type validates that selection but does not
perform overload selection or adaptation.

Visibility is checked when the reference is formed. A private static method
may be captured only where it is nameable; a validly formed value can then be
passed or returned and called without repeating member-name access checks.
Module qualification and imports retain their ordinary lookup rules.

Instance, virtual, and interface method selections are not values. Neither are
initializers, copy/assignment/destruction members, generated lifecycle bodies,
external declarations, intrinsics, raw generic templates, or unclosed static
methods.

Lexical value lookup shadows a same-named top-level function. A direct named
call remains direct when no value binding shadows its callee name:

```ska
fn transform(value: i64) -> i64 { return value; }

fn apply(transform: fn(i64) -> i64, value: i64) -> i64 {
    return transform(value); // indirect call through the parameter
}
```

## Storage and lifecycle

A function value is an always-valid trivial scalar. It supports explicitly
initialized locals and reassignment, value parameters and results, instance
fields, explicitly initialized static fields, synthesized field copying and
assignment, and contextual generic storage.

Copying duplicates only the callable reference. There is no allocation,
capture environment, owner retain/release, copy constructor, destructor, or
runtime-managed resource. Ordinary locals and fields retain their existing
definite-initialization rules. A function-valued static must have an explicit
initializer because zero is not a valid function value:

```ska
class Hooks {
    static valid: fn() -> unit = default_hook;
    static invalid: fn() -> unit; // rejected
}
```

Function-valued fields add no destruction step. Explicit object-copy syntax
does not apply to function values. `ref` and `mut ref` parameters cannot alias
the variable slot holding a function value in the initial feature.

## Internal callable composition

Function values may cross every internal callable family as ordinary value
parameters or results: top-level and static functions, initializers, instance
methods, virtual overrides, interface requirements and implementations, and
closed generic-class members. Override and conformance checking require exact
canonical function types.

The referenced function's own signature may use every supported internal
parameter and result family, including aliases, inline objects, arrays,
optionals, shared owners, aggregate caller-owned results, and another function
value. Indirect calls reuse those established ownership and result rules; they
do not define a primitive-only callback ABI.

## Indirect calls and evaluation

Every function-typed expression is callable with ordinary argument syntax:

```ska
callback(value)
holder.callback(value)
Hooks.callback(value)
choose_callback()(value)
factory.produce().callback(value)
```

The callee expression evaluates exactly once before explicit arguments. Its
value is secured in compiler-owned temporary storage; arguments then evaluate
exactly once from left to right. The call runs only after the callee and all
arguments complete successfully. Results are secured or transferred before
full-expression cleanup under the same rules as the corresponding direct
call.

If callee evaluation terminates, no argument runs. If an argument terminates,
the indirect call does not run. The existing non-unwinding abrupt-termination
boundary remains unchanged.

Cast precedence is deliberately unchanged. `(f)(argument)` remains an
object-cast candidate rather than grouped callable syntax; unambiguous forms
such as `f(argument)` and other postfix chains are available.

## Closed generic composition

Function types may contain class type parameters in parameter or result
positions. Specialization substitutes the complete structural signature and
interns one closed function type before ordinary class type checking. A static
method reference inside a template becomes an exact callable reference only
while specializing that body.

Function types may also be explicit closed generic arguments. They satisfy
ordinary stored-scalar, value parameter/result, copying, assignment, and
destruction requirements. They do not satisfy requirements for optional
payloads, array elements, shared targets, or alias targets in the initial
feature.

Separate closed class specializations retain separate static-method target
identities even when their substituted signatures share one function type.
Generic top-level functions, method-level type parameters, and runtime generic
callable identities remain excluded.

## Initial exclusions

The frozen initial feature excludes:

- lambdas, nested functions, captures, closures, and capture inference;
- bound or unbound instance methods and virtual/interface method values;
- nullable function values, null literals, optional function values, and
  arrays of function values;
- `shared` function values and callback-slot aliases;
- casts, equality, ordering, hashing, formatting, reflection, serialization,
  and byte conversion involving function values;
- external, intrinsic, runtime, initializer, lifecycle, and generated-body
  references;
- source-visible calling conventions, raw addresses, C callbacks, and stable
  separate-compilation ABI; and
- any promise that future closures use the same representation or implicitly
  convert to capture-free function values.

The target-independent and x86-64 realization is frozen in the
[compiler contract](../compiler/FUNCTION_VALUES.md). The complete decision
record is preserved in the
[archived design proposal](../archive/FUNCTION_VALUES_DESIGN_PROPOSAL.md).

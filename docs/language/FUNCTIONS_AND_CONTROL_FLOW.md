# Skald Functions and Control Flow

Status: authoritative for implemented callable, binding, scope, statement,
control-flow, return, and evaluation-order semantics. The
[status matrix](STATUS.md) is authoritative for feature maturity, the
[grammar](GRAMMAR.md) defines accepted source syntax, and
[types and values](TYPES_AND_VALUES.md) defines expression typing.

## Callable boundary

The [modules and interoperation contract](MODULES_AND_INTEROP.md) defines the
current single-file compilation unit and top-level namespace. All top-level
declarations are known before any body is resolved, so a function may call a
later function and direct recursion is supported. Calls select a named
function or a named instance method directly; functions and methods are not
first-class values.

The callable rules in this document also apply to instance methods where their
class-owned receiver rules permit. Initializers, copy assignment, and
copy construction and destructors use more specialized body and result
contracts owned by
[class and lifecycle semantics](CLASSES_AND_LIFECYCLE.md).

## Function declarations

A function definition has a name, an ordered parameter list, an explicit
result type, and a body. Calls to internal and external functions use the same
arity and exact-type checking. The bodyless external form, entry point, and
interoperation trust boundary are defined by
[modules and foreign interoperation](MODULES_AND_INTEROP.md).

## Parameters

Parameters are ordered bindings initialized when the callable begins. Their
binding category is separate from the declared type:

| Source category | Implemented meaning |
|---|---|
| `name: T` | A value parameter owning callee-local parameter storage. Primitive arguments are copied as values; exact-class arguments are copy-constructed for the call. |
| `ref name: T` | A call-scoped read-only alias to an existing exact-class place. |
| `mut ref name: T` | A call-scoped mutable alias to an existing exact-class place. |

Value parameters may use implemented primitive or exact-class types. Alias
parameters use exact class types and are supported only on internal callables.
Aliases do not own or copy their object. Their access, source-place, forwarding,
overlap, and non-escape rules are defined by
[aliases and ownership](ALIASES_AND_OWNERSHIP.md).

Parameter names must be unique within one callable. Parameters and the
callable body's outermost block occupy the same lexical scope, so an outermost
local cannot redeclare a parameter. A nested block may shadow it.

## Calls and results

A call supplies exactly one argument per parameter. Value arguments must have
the exact declared type; alias arguments must designate a compatible place and
provide the required access. The complete argument list is checked even when
one argument is invalid, so independent source errors can be reported.
When an alias argument borrows a shared pointee, the caller must write
`*owner`; passing the raw shared handle instead selects an owning value and is
rejected for the alias parameter.

Functions and methods are not overloaded. The ordinary-initializer overload
set reuses these argument-binding rules, then applies its separate compile-time
applicability and specificity rules from
[classes and lifecycle](CLASSES_AND_LIFECYCLE.md#ordinary-initializer-overloads).

A primitive-returning call is an expression of its declared type. A
`unit`-returning call has no result payload. An exact-class-returning internal
call produces an object for one of the supported object initialization,
assignment, argument, or return contexts; it is not an ordinary scalar value.

Exact-class value arguments are distinct callee-owned objects. Each is
copy-constructed before the call, transferred to its value parameter, and
destroyed by the callee after its body-owned locals. Exact-class results are
completed as caller-owned objects. Detailed copy selection, result-object
lifetime, temporaries, and permitted elision are defined by
[class lifecycle semantics](CLASSES_AND_LIFECYCLE.md#owning-value-parameters).

The implemented shared-ownership profile adds non-null shared value parameters
and results without changing source evaluation order. Named
arguments and returns copy an owner; produced values transfer their existing
owner; the callee owns each shared value parameter; and the caller owns a
completed shared result. Explicitly dereferenced shared receivers and alias
arguments use hidden anchors where needed. Dereferenced checked places extend
those anchors
through their complete immediate consumer. The complete rule is owned by
[Shared Ownership and Heap Allocation](SHARED_OWNERSHIP.md#strong-owner-value-semantics).

## Lexical scopes and locals

A block is a lexical scope, except that a callable's outermost block shares the
parameter scope. Each nested block creates a child scope. Each `if`, `elif`,
and `else` body is a separate child scope.

A local declaration creates owning storage and requires an explicit type and
initializer. Its initializer is resolved and evaluated before the new binding
becomes visible. The local is visible from the following statement through the
end of its block.

Two bindings cannot have the same name in one scope. A nested scope may shadow
an outer binding, and the outer binding becomes visible again after the nested
scope. A local binding also shadows a same-named top-level callable at an
expression or call site. Type names continue to resolve in the top-level type
namespace rather than through local value bindings.

Leaving a block destroys successfully initialized owning class locals from
last completed initialization to first. Primitive locals require no lifetime
cleanup. Detailed destruction behavior is owned by class lifecycle semantics.

## Statements and blocks

The implemented general body forms are:

- local declarations;
- nested blocks;
- `if`/`elif`/`else` conditionals;
- returns;
- call statements;
- assignment-shaped class and field operations.

Assignment operation selection and initializer-body restrictions are
[class semantics](CLASSES_AND_LIFECYCLE.md#ordinary-initializer-contract).
Arbitrary expression statements are not supported. An expression
statement is valid only when its outer operation, through any grouping, is a
function or method call returning `unit`. A value-returning call cannot be
discarded.

## Conditionals

An `if` statement has one `if` arm, zero or more `elif` arms, and an optional
final `else`. Every condition must have type exactly `bool`; there is no
truthiness conversion. A conditional is a statement and does not produce a
value.

Conditions are evaluated in source order. Evaluation stops at the first
condition producing `true`, and only that arm executes. If no condition is
true, the `else` body executes when present; otherwise execution continues
after the conditional.

Each condition resolves in the scope containing the complete conditional.
Each arm body has its own child scope. A binding declared in one arm is not
visible in another arm, in a later `elif` condition, or after the conditional.

## Returns and definite return

A callable returning `unit` uses `return;` and may also fall through the end of
its body. It cannot attach a value to `return`.

A callable returning a primitive or exact-class value must use a matching
`return` value on every reachable path. Reaching the closing brace is an error.
A nested block definitely returns when its reachable execution cannot reach
that block's closing brace. A conditional definitely returns only when it has
an `else` body and every `if`, `elif`, and `else` body definitely returns.

Return first evaluates and preserves its result. An exact-class result is
fully constructed or copied into its result object. Full-expression
temporaries are then destroyed in reverse completion order, followed by live
owning locals from inner scopes to outer scopes and owning value parameters in
reverse parameter order. The preserved result is transferred only after those
cleanups. Cleanup for exceptions and loop exits is not part of the implemented
control-flow model. The current abrupt-termination boundary and constraints on
future exceptional cleanup are owned by
[errors and exceptional control flow](ERRORS.md#cleanup-and-abrupt-termination).

## Evaluation order

Skald uses deterministic source order for the implemented expressions and
calls:

1. a unary operand is evaluated before its operator;
2. binary operands are evaluated left to right;
3. a field receiver place is selected before the field is read;
4. an assignment destination place is selected before its right-hand source;
5. a method receiver is selected before explicit arguments;
6. explicit function, method, and constructor arguments are evaluated left to
   right;
7. an exact-class value-argument copy completes before the next argument is
   evaluated;
8. object destination storage is selected before construction or an
   object-producing call, then the receiver and explicit arguments follow the
   ordering above;
9. conditional conditions are evaluated in arm order and stop after the first
   true result;
10. a return result is completed before its cleanup sequence begins.

Grouping does not change the order of the enclosed expression. It can affect
the limited object-materialization and elision rules, which are class lifecycle
concerns.

The frozen [object-cast profile](OBJECT_CASTS.md) evaluates its source once,
establishes any required lifetime anchor, performs a dynamic check when needed,
then supplies the checked place or shared owner to its consuming context.
Postfix calls on a cast result use explicit grouping, for example
`((Leaf) value).read()`. Cast execution does not reorder receivers or later
arguments.

The shared copy-allocation form `new T(copy source)` is one such
consuming context. It evaluates and target-checks the copy source before
allocating the destination, keeps the source and any anchor live through
exact-`T` copy construction, and secures the produced owner before
full-expression cleanup.

## Unsupported control flow and callability

Loops, `break`, `continue`, iteration, function values, closures, lambda
literals, and calls through expression values are not implemented or frozen.
Their maturity is recorded in the [status matrix](STATUS.md#not-implemented).
No loop scope, cleanup, iterator, capture, or callable-type rule should be
inferred from legacy examples.

## Implementation boundary

The source semantics above do not require a particular control-flow graph,
intermediate representation, block numbering, hidden result argument, frame
layout, register assignment, symbol spelling, or calling convention. Those are
compiler, backend, runtime, and interoperability concerns. An implementation
may choose different internal structures while preserving scope, ordering,
result, and cleanup behavior.

# Skald Functions and Control Flow

Status: authoritative for implemented callable, binding, scope, statement,
control-flow, return, evaluation-order, and primitive-binding-reassignment
semantics, including implemented `while` loops and the frozen but
unimplemented `break` and `continue` contract. The
[status matrix](STATUS.md) is authoritative for feature maturity, the
[grammar](GRAMMAR.md) defines accepted source syntax, and
[types and values](TYPES_AND_VALUES.md) defines expression typing.

## Callable boundary

The [modules and interoperation contract](MODULES_AND_INTEROP.md) defines the
current single-file compilation unit and top-level namespace. All top-level
declarations are known before any body is resolved, so a function may call a
later function and direct recursion is supported. Calls select a named
function, a named instance method, or a class-selected static method directly;
functions and methods are not first-class values.

The callable rules in this document also apply to instance and static methods
where their class-owned rules permit. Initializers, copy assignment, and
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

### Primitive binding reassignment

An initialized primitive `var` local or primitive value parameter is
replaceable with an exactly typed value:

```text
var count: i64 = 1;
count = count + 1;

fn advance(value: i64) -> i64 {
    value = value + 1;
    return value;
}
```

This contract applies to `i64`, `u64`, `u8`, `f64`, and `bool`. The
left-hand identifier resolves before the right-hand expression and must select
an already-declared `var` local or value parameter. Parenthesizing the complete
destination is transparent: `(count) = 2;` selects the same binding identity.
Normal lexical lookup applies, so an assignment inside a nested scope updates
the innermost visible declaration and an outer local or parameter becomes
visible again when that scope ends. Parameters are callee-local value storage;
reassignment does not mutate the caller's argument.

The right-hand expression must have exactly the binding's declared primitive
type. It is evaluated exactly once, and its value is stored only after that
evaluation succeeds. Any full-expression temporaries created while evaluating
the source are cleaned after the store under their existing lifetime rules.
Reassignment neither begins a new lifetime nor changes cleanup registration;
primitive locals remain initialized from declaration through the end of their
scope, and primitive value parameters remain initialized from callable entry
through callable exit.

Primitive binding reassignment is a statement and produces no value. Assignment
expressions, chaining, compound assignment, increment and decrement,
destructuring, local aliases, alias-parameter rebinding, and non-primitive
value-parameter rebinding are not part of this contract. Existing
primitive-field, object, shared-owner, optional, array, and array-element
assignment retain their own rules.
Initializer and copy-constructor bodies that admit only direct receiver-field
initialization do not gain local reassignment.

The parser retains an identifier or grouped identifier followed by `=` as
assignment-shaped syntax. Resolution classifies the primitive-local meaning
and preserves the selected local identity through typed HIR and verified MIR.

## Statements and blocks

The implemented general body forms are:

- local declarations;
- nested blocks;
- `if`/`elif`/`else` conditionals;
- returns;
- call statements;
- primitive binding reassignment;
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

The [optional-values contract](OPTIONAL_VALUES.md#presence-tests) adds
`value is some` and `value is none` as `bool`-producing tests. They execute for
primitive and exact-class optional locals, fields, parameters, and produced
call results and
do not add optional truthiness. A presence test does not narrow the declared
type. Primitive payload use spells checked postfix unwrap `value!`; a class
payload unwrap supplies a bounded checked place to its immediate field,
method, alias, cast, type-test, or owning-copy consumer.

Conditions are evaluated in source order. Evaluation stops at the first
condition producing `true`, and only that arm executes. If no condition is
true, the `else` body executes when present; otherwise execution continues
after the conditional.

Each condition resolves in the scope containing the complete conditional.
Each arm body has its own child scope. A binding declared in one arm is not
visible in another arm, in a later `elif` condition, or after the conditional.

## While loops and loop exits

`while` is implemented. `break` and `continue` have a frozen source design,
are reserved words, and currently receive a focused unsupported-feature
diagnostic. The selected
[grammar](GRAMMAR.md#while-loops-and-reserved-loop-exits) is:

```text
while (condition) {
    statements
}

break;
continue;
```

The parentheses and body block are mandatory. `while` is a statement and
produces no value. Under the frozen exit contract, `break` and `continue` are
statements, and `break` carries no value. Labels and labeled exits are not
part of this contract.

The condition must have type exactly `bool`; loops add no truthiness
conversion. Execution evaluates the condition before the first possible
iteration and once before every later attempted iteration. It preserves the
resulting primitive boolean, completes the condition's full-expression
cleanup, and only then enters the body for `true` or continues after the loop
for `false`. No condition temporary, checked view, optional guard, shared
temporary, or array anchor remains live during the body or after the loop.

The condition resolves in the scope containing the complete `while`
statement. The statement introduces no additional lexical scope of its own.
Its body is an ordinary child block scope, and a binding declared there is not
visible in the condition or after the loop. Each entered iteration begins a
fresh dynamic lifetime for every body-local binding. Normal body completion
destroys live owning body locals in the ordinary reverse order before the next
condition evaluation. Enclosing locals remain live across the loop and retain
assignments performed by the body.

Unlabeled `break` and `continue` select the nearest lexically enclosing loop.
Using either statement outside a loop is an error. `continue` cleans every
live owning local in nested scopes and the loop body scope before beginning the
next condition test. `break` performs the same body-side cleanup before
continuing after the selected loop. Neither exit cleans a local declared
before that loop. Cleanup follows the existing inner-to-outer,
reverse-declaration order. `return` continues to clean every exited function
scope, while unrecoverable panic remains non-unwinding.

For definite-return analysis, every `while` conservatively has a
condition-false fallthrough path, including `while (true)`. A non-`unit`
callable therefore cannot rely only on such a loop to satisfy its return
requirement. Later constant folding may remove an executable false edge but
does not change source acceptance or definite-return diagnostics.

Later implementation slices add `break` and `continue` using this
already-frozen contract. `for`, `for ... in`, `do while`, an unconditional
`loop` form, iterator protocols, loop expressions, value-carrying `break`,
loop `else`, and labels remain unfrozen.

## Returns and definite return

A callable returning `unit` uses `return;` and may also fall through the end of
its body. It cannot attach a value to `return`.

A callable returning a primitive or exact-class value must use a matching
`return` value on every reachable path. Reaching the closing brace is an error.
A nested block definitely returns when its reachable execution cannot reach
that block's closing brace. A conditional definitely returns only when it has
an `else` body and every `if`, `elif`, and `else` body definitely returns.

The implemented [`std::error::panic`](ERRORS.md#frozen-panic-design) call is a non-returning
call statement. Its reachable path cannot reach the block's closing brace, so
it satisfies definite return. This special flow result does not add a general
`never` type or permit the call in expression position.

Return first evaluates and preserves its result. An exact-class result is
fully constructed or copied into its result object. Full-expression
temporaries are then destroyed in reverse completion order, followed by live
owning locals from inner scopes to outer scopes and owning value parameters in
reverse parameter order. The preserved result is transferred only after those
cleanups. Normal `while` body completion performs its implemented loop
cleanup. Targeted `break` and `continue` cleanup has the
[frozen behavior above](#while-loops-and-loop-exits); exceptional cleanup
remains exploratory. The current abrupt-termination boundary and constraints
on future exceptional cleanup are owned by
[errors and exceptional control flow](ERRORS.md#cleanup-and-abrupt-termination).

## Evaluation order

Skald uses deterministic source order for the implemented expressions and
calls:

1. a unary operand is evaluated before its operator;
2. binary operands are evaluated left to right;
3. a field receiver place is selected before the field is read;
4. an assignment destination place is selected before its right-hand source;
5. a method receiver is selected before explicit arguments;
6. explicit function, instance-method, static-method, and constructor
   arguments are evaluated left to right; a static call's class spelling is
   not evaluated;
7. an exact-class value-argument copy completes before the next argument is
   evaluated;
8. object destination storage is selected before construction or an
   object-producing call, then the receiver and explicit arguments follow the
   ordering above;
9. conditional conditions are evaluated in arm order and stop after the first
   true result;
10. a `while` condition is completed and cleaned before its branch, and each
    normal body completion is cleaned before the next condition evaluation;
11. a return result is completed before its cleanup sequence begins.

Grouping does not change the order of the enclosed expression. It can affect
the limited object-materialization and elision rules, which are class lifecycle
concerns.

Primitive binding reassignment uses item 4 without introducing
an effectful destination computation: resolution selects the binding identity,
the source evaluates once, the completed scalar is stored, and source
full-expression cleanup follows the store.

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

The panic call uses these ordinary argument rules: its one exact
`std::str::Str` value argument is evaluated and copied exactly once. An
unrecoverable failure during that production wins; after reporting starts,
the call terminates without performing remaining cleanup.

## Unsupported control flow and callability

The frozen `break` and `continue` contract remains unimplemented. Other loop
forms, iteration and iterator protocols, function values, closures, lambda
literals, and calls through expression values are neither implemented nor
frozen. Their maturity is recorded in the
[status matrix](STATUS.md#not-implemented). No semantics for those deferred
features should be inferred from legacy examples.

## Implementation boundary

The source semantics above do not require a particular control-flow graph,
intermediate representation, block numbering, hidden result argument, frame
layout, register assignment, symbol spelling, or calling convention. Those are
compiler, backend, runtime, and interoperability concerns. An implementation
may choose different internal structures while preserving scope, ordering,
result, and cleanup behavior.

# Implemented Skald Grammar

Except for the clearly labeled frozen next-slice extension near the end, this
document describes the source language accepted by the current compiler. It is
intentionally narrower than the broader design in
[`SKALD_DRAFT_SPEC.md`](../docs/SKALD_DRAFT_SPEC.md).

## Lexical structure

Skald source is UTF-8. Identifiers are currently ASCII:

```text
identifier-start    = A..Z | a..z | _
identifier-continue = identifier-start | 0..9
identifier          = identifier-start identifier-continue*
```

Reserved keywords are:

```text
fn var return extern unit
i64 u64 u8 f64 bool true false
if elif else
class self mut ref
```

`init` is contextual: it introduces the special initializer form only in a
class body. Elsewhere it is an ordinary identifier.

Ignored trivia consists of ASCII space, tab, carriage return, newline, and
`//` line comments. Block comments are not supported.

Punctuation and operators are:

```text
( ) { } , : ; . -> + - * =
```

## Literals

```text
i64-literal = decimal-digits
u64-literal = decimal-digits "u"
u8-literal  = decimal-digits "u8"

f64-literal = decimal-digits "." decimal-digits [exponent]
            | decimal-digits exponent
exponent    = ("e" | "E") ["+" | "-"] decimal-digits

bool-literal = "true" | "false"
```

Leading `-` is the unary negation operator, not part of a literal token.
Underscores, hexadecimal/octal/binary integers, integer suffixes other than
`u` and `u8`, leading-dot floats, trailing-dot floats, `f64` suffixes, `NaN`,
and infinity spellings are not accepted.

Literal types come from spelling; expected context does not reinterpret them.
The accepted integer ranges are:

- `i64`: `0` through `9223372036854775807`, plus the unary-negated magnitude
  `9223372036854775808` for `i64::MIN`;
- `u64`: `0u` through `18446744073709551615u`;
- `u8`: `0u8` through `255u8`.

Finite decimal `f64` literals are rounded to IEEE-754 binary64 using nearest,
ties-to-even. A spelling that rounds to infinity is rejected. Typed IR stores
the resulting raw bits rather than retaining host-formatted floating values.

Malformed numeric text is consumed as one invalid token when possible, giving
one focused diagnostic rather than a cascade.

## Compilation unit and declarations

```text
compilation-unit = top-level-declaration* EOF

top-level-declaration = function-definition
                      | external-function-declaration
                      | class-declaration

function-definition = "fn" identifier parameter-list
                      "->" result-type block

external-function-declaration = "extern" "fn" identifier parameter-list
                                "->" result-type ";"

parameter-list  = "(" [parameter ("," parameter)*] ")"
parameter       = value-parameter | alias-parameter
value-parameter = identifier ":" primitive-type
alias-parameter = ["mut"] "ref" identifier ":" class-name
class-name      = identifier

primitive-type = "i64" | "u64" | "u8" | "f64" | "bool"
result-type    = primitive-type | "unit"
```

Trailing commas are not accepted. `unit` is a result type only; it is not a
parameter or local-storage type. The parser accepts alias parameters on
defined functions, external declarations, methods, and initializers so later
phases can apply declaration-specific legality rules. An alias parameter must
use a syntactic class name; primitive aliases are not part of this profile.

Functions and classes share one non-overloaded top-level namespace. All
declarations are collected before bodies are resolved, so forward calls and
recursion are valid. A duplicate declaration is an error. The entry point must
be a defined, non-external `fn main() -> i64`.

External declarations use their source identifier as an exact linker symbol.
Their parameters and results are restricted to implemented primitive values or
`unit`; object-bearing and alternate-name FFI are not supported.

## Statements and blocks

```text
block = "{" statement* "}"

statement = local-declaration
          | return-statement
          | call-statement
          | conditional-statement
          | field-assignment
          | block

local-declaration = "var" identifier ":" local-type "=" expression ";"
local-type        = primitive-type | identifier

return-statement = "return" [expression] ";"
call-statement   = expression ";"

conditional-statement = "if" "(" expression ")" block
                        ("elif" "(" expression ")" block)*
                        ["else" block]

field-assignment = receiver-place "." identifier "=" expression ";"
receiver-place   = identifier | "self" | "(" receiver-place ")"
```

A call statement must be a call returning `unit`; arbitrary expressions and
value-returning calls cannot be discarded. A `unit` function may use
`return;` or fall through its closing brace. A value-returning function must
return a value on every reachable path.

Conditions have exactly type `bool`; Skald has no implicit truthiness.
`elif` is a distinct keyword and grammar form. `else if` is not accepted.
Conditions are resolved in the containing scope, while every arm body owns a
separate child scope.

Parameters and a function body's outermost block share one lexical scope. A
local becomes visible only after its initializer. Nested blocks may shadow
outer bindings; duplicate names in one scope are errors. A local also shadows a
top-level callable at a call site.

General local assignment, compound assignment, chained assignment, and
assignment expressions are not implemented.

## Expressions

```text
expression     = additive
additive       = multiplicative (("+" | "-") multiplicative)*
multiplicative = unary ("*" unary)*
unary          = "-" unary | postfix
postfix        = primary (member-suffix | call-suffix)*

member-suffix = "." identifier
call-suffix   = "(" [arguments] ")"
arguments     = expression ("," expression)*

primary = identifier
        | numeric-literal
        | bool-literal
        | "self"
        | "(" expression ")"
```

Postfix operations bind most tightly, followed by unary `-`, multiplication,
then addition and subtraction. Unary operators associate right-to-left; binary
and postfix operations associate left-to-right.

A direct function call target is an ungrouped identifier selected during
resolution. Function values and calls through arbitrary expressions are not
implemented. Member selection and construction are also resolved before type
checking; later phases never select declarations by source name.

Operands, receivers, and arguments evaluate deterministically from left to
right. A receiver is evaluated before explicit arguments.

## Primitive semantics

Initializers, arguments, returns, assignments, and binary operands require
exactly matching types. There are no implicit conversions, promotions, or
expected-type literal inference.

`+`, `-`, and `*` accept two operands of the same numeric type:

- `i64` uses signed integer operations; overflow behavior is not yet specified;
- `u64` wraps modulo 2^64;
- `u8` wraps modulo 256 and is canonicalized at observable boundaries;
- `f64` uses IEEE-754 binary64 operations in the default nearest/ties-to-even
  environment.

Unary `-` accepts `i64` and `f64`. It does not accept `u64`, `u8`, or `bool`.
Division, remainder, comparisons, bitwise operations, shifts, and casts are not
implemented.

## Inline classes

```text
class-declaration = "class" identifier "{" class-member* "}"

class-member = field-declaration
             | initializer-declaration
             | method-declaration

field-declaration = identifier ":" primitive-type ";"

initializer-declaration = "init" parameter-list block

method-declaration = ["mut"] "fn" identifier parameter-list
                     "->" result-type block
```

Classes are nominal. Fields and ordinary methods share one non-overloaded
member namespace. Each class, including an empty class, must declare exactly
one explicit `init`; initializer overloading and synthesized initializers are
not available.

Initializer bodies are straight-line sequences of:

```ska
self.field = primitive_expression;
```

Every field must be assigned exactly once. A field cannot be read before its
own assignment. Right-hand expressions may use primitive literals,
initializer parameters, already initialized fields, primitive operations, and
supported top-level function calls. Initializers cannot contain locals,
blocks, conditionals, call statements, returns, construction, or instance
method calls.

Construction is legal only as the complete initializer of a new exact-type
local:

```ska
var counter: Counter = Counter(40);
```

It is not a general object expression and cannot be grouped for another use,
passed, returned, copied, assigned to existing storage, or used as a receiver.
Constructor arguments evaluate left to right before `init` begins.

Fields may be read or assigned through an inline local or `self`. Ordinary
`fn` methods have read-only receivers. `mut fn` methods may assign fields and
call mutable methods; read-only methods may only read fields and call
read-only methods. A local inline object permits either receiver mode. Dispatch
is static and direct.

The current native-code object profile has only primitive fields and primitive
by-value parameters and results. It does not include object fields, object
values in arguments/results, copying, `assign`, `destroy`, inheritance,
interfaces, virtual calls, casts, `shared`, access
modifiers, static members, `final`, or object FFI.

Restricted alias parameters are implemented end to end. Binding mode remains
separate from nominal class type in every semantic IR. A `ref` parameter may
read fields, call read-only methods, and be forwarded to `ref`; a `mut ref`
parameter may additionally write fields, call mutable methods, and satisfy
either alias mode. Mutable inline locals and mutable method `self` may satisfy
either mode, while read-only `self` may satisfy only `ref`.

An alias argument must be an already-live exact-class place: an inline local,
method `self`, a forwarded alias parameter, or a grouped form of one of these.
Aliases are call-scoped, non-owning, non-storable, non-returnable, and
non-exclusive; passing the same local to multiple mutable alias parameters is
valid. Initializers may receive aliases, but their not-yet-live `self` cannot
be an alias argument. Calls retain one left-to-right sequence of primitive
values and alias places, with a method receiver selected first.

MIR represents an alias parameter as an indirect place base. The Linux x86-64
System V backend passes one integer-class pointer per alias without copying
object bytes. The complete declaration, place, access, lifetime, IR, and ABI
contract is in the
[restricted stage-0 alias-parameter profile](../docs/SKALD_DRAFT_SPEC.md#543-restricted-stage-0-alias-parameter-profile).
Ordinary by-value parameters remain primitive-only. Local aliases, primitive
aliases, shared sources, borrow anchors, object fields/elements, polymorphic
conversion, and whole-object replacement through an alias are not implemented.

## Frozen next-slice extension: class-typed inline fields

This section freezes the parser-facing extension for the next object-model
slice. **It is not part of the grammar accepted by the current compiler.** The
implemented grammar above remains authoritative until the
[Class-Typed Inline Object Fields Roadmap](../docs/INLINE_OBJECT_FIELDS_ROADMAP.md)
is complete.

The extension changes the class field type and projected assignment-place
productions:

```text
field-declaration = identifier ":" field-type ";"
field-type        = primitive-type | class-name
class-name        = identifier

field-assignment = receiver-place "." identifier "=" expression ";"
receiver-place   = place-root ("." identifier)*
place-root       = identifier | "self" | "(" receiver-place ")"
```

It does not admit named types for by-value parameters, results, external
functions, primitive locals, or any other type position. A named local type
continues to use the existing direct-construction-only object-local rules.
`unit` remains invalid as a field type. The projected receiver production
selects an assignment-shaped statement from syntax already expressible by the
postfix parser; it does not create general assignment expressions. No new
token, keyword, precedence level, or postfix suffix is introduced.

A class field is initialized in the enclosing class's straight-line `init`
body with the existing assignment-shaped statement:

```ska
self.primitive = primitive_expression;
self.child = Child(arguments);
```

For a class field, the complete right side must be an ungrouped constructor of
the field's exact class. This is construction into the field's storage, not
assignment of an object value. Only a direct field of the current initializer's
`self` is a construction destination. Grouping around `self` remains
transparent; construction into a nested path or any already-live place is not
part of the extension.

After construction, the existing postfix syntax can designate nested inline
places:

```ska
outer.inner.value
outer.inner.observe()
inspect(outer.inner)
```

A path starts at an inline local, live method `self`, or alias parameter and
may cross class-typed fields. A class endpoint is valid only as a method
receiver or exact-class alias argument; it is not an ordinary object value. A
primitive endpoint may be read, and may be assigned when the path has mutable
access. Grouping around a place is transparent.

The root binding's access applies to the complete path. A read-only method
receiver or `ref` root permits reads, read-only calls, and `ref` arguments. A
mutable local, mutable receiver, or `mut ref` root additionally permits
primitive writes, mutable calls, and `mut ref` arguments. Whole-object
replacement remains invalid.

Each direct primitive or class field is initialized exactly once. A class
field becomes live only after its nested initializer returns normally. A path
through an uninitialized field is invalid; later initializer statements may
use an already-completed field as a receiver or alias argument. The incomplete
enclosing `self` remains invalid as a complete receiver or alias argument.

Class containment must be acyclic. Direct self-containment and indirect cycles
are source-level semantic errors. Forward references, repeated acyclic field
types, diamonds, and empty contained classes are valid.

The exact declaration, liveness, evaluation-order, diagnostic, IR, layout, and
future-lifecycle contract is in the
[frozen class-typed inline-field profile](../docs/SKALD_DRAFT_SPEC.md#544-frozen-class-typed-inline-field-profile).

## Recovery and nesting

The parser accumulates structured diagnostics and synchronizes at parameter,
statement, block, class-member, and top-level boundaries. Structurally invalid
declarations are omitted from the partial AST; later semantic phases run only
after the preceding phase succeeds.

Recursive syntax nesting is limited to 128 active levels across blocks,
grouping, unary expressions, and postfix calls. Exceeding the limit reports
`PAR005` and skips the affected declaration without recursive recovery.

## Not yet implemented

The following broader-language features remain design or implementation work:

- loops and iterators;
- arrays and optionals;
- strings and standard-library containers;
- class-typed inline object fields and nested object places;
- object value parameters/results and general temporaries;
- deterministic destruction and cleanup;
- inheritance, interfaces, virtual dispatch, and access control;
- local alias declarations and alias sources beyond inline locals, method
  `self`, and forwarded parameters;
- `shared` ownership and aliasing through shared sources;
- checked exceptions;
- multiple source files, modules, generics, and closures.

Their intended direction is discussed in the draft specification and
[`Future Development Boundaries`](../docs/NEXT_SLICE_BOUNDARIES.md).

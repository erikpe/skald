# Grammar

This directory is reserved for the canonical Skald grammar and parser-facing grammar notes. The complete language grammar is still an identified specification gap. The first-slice, output, `bool`/conditional, and `u64` contracts below describe implemented behavior.

## First vertical slice lexical contract

M1 implements only the token surface needed by the first vertical slice. This is an implementation contract for that slice, not the final complete-language grammar.

Keywords:

```text
fn var return i64
```

Identifiers use ASCII spelling:

```text
identifier-start    = A..Z | a..z | _
identifier-continue = identifier-start | 0..9
```

Decimal integer literals contain one or more ASCII digits. A decimal digit sequence immediately followed by ASCII letters, `_`, or `.` is consumed as one malformed literal so that forms such as `12abc`, `1_000`, `12.5`, and `0xff` produce one focused diagnostic. The lexer validates spelling but deliberately leaves `i64` range checking to type checking.

Punctuation and operators:

```text
( ) { } , : ; -> + - * =
```

Ignored trivia consists of ASCII space, tab, carriage return, newline, and `//` line comments. A line comment ends immediately before newline or at end of file. Block comments are not part of the first slice.

Skald source is UTF-8, but non-ASCII characters are not accepted in M1 identifiers. An unsupported Unicode scalar is consumed as one invalid token with a byte-accurate span and character-based diagnostic column.

## First vertical slice grammar

M2 implements this source grammar:

```text
compilation-unit = function-declaration* EOF

function-declaration = "fn" identifier parameter-list "->" "i64" block
parameter-list       = "(" [ parameter ("," parameter)* ] ")"
parameter            = identifier ":" "i64"

block                = "{" statement* "}"
statement            = local-declaration | return-statement | block
local-declaration    = "var" identifier ":" "i64" "=" expression ";"
return-statement     = "return" expression ";"

expression           = additive
additive             = multiplicative (("+" | "-") multiplicative)*
multiplicative       = unary ("*" unary)*
unary                = "-" unary | postfix
postfix              = primary ("(" [ arguments ] ")")*
arguments            = expression ("," expression)*
primary              = identifier | decimal-integer | "(" expression ")"
```

Trailing commas are not accepted in M2. Calls are parsed as postfix expressions without name lookup or call-target validation; M3 owns those semantic decisions. Parenthesized expressions remain explicit grouped AST nodes so the source AST preserves their full spans.

Operator precedence, from highest to lowest, is:

1. postfix call;
2. unary `-`;
3. binary `*`;
4. binary `+` and `-`.

Unary `-` associates right-to-left. The binary operators and repeated postfix calls associate left-to-right. Parentheses override precedence.

Parser recovery may synthesize missing punctuation to retain a useful source AST. Structurally incomplete declarations and statements are omitted from that AST, diagnostics are accumulated, and later semantic phases must not run when parsing reports errors.

## `i64` output extension contract

O0 fixed the following grammar and semantic contract for the post-M8 `i64`
output extension. O3 implemented `unit`, unit returns, and restricted call
statements; O5 implemented the remaining `extern` declaration syntax. The
accepted extension grammar is:

```text
compilation-unit = top-level-declaration* EOF

top-level-declaration          = function-declaration
                               | external-function-declaration
function-declaration           = "fn" identifier parameter-list "->" type block
external-function-declaration  = "extern" "fn" identifier parameter-list "->" type ";"
parameter-list                 = "(" [ parameter ("," parameter)* ] ")"
parameter                      = identifier ":" "i64"
type                           = "i64" | "unit"

block                 = "{" statement* "}"
statement             = local-declaration
                      | return-statement
                      | call-statement
                      | block
local-declaration     = "var" identifier ":" "i64" "=" expression ";"
return-statement      = "return" [ expression ] ";"
call-statement        = expression ";"
```

The expression grammar and precedence remain unchanged. Although
`call-statement` is parsed through the general expression grammar, semantic
analysis accepts it only when, after ignoring grouping parentheses, its
outermost operation is a function call and its result type is `unit`. An
arithmetic expression, binding, literal, or value-returning call cannot be
discarded as a statement in this extension. This is intentionally narrower
than the complete draft language's eventual expression-statement rules.

Return behavior is determined by the declared function result type:

- an `i64` function uses `return expression;`, the expression must have type
  `i64`, and every reachable path must return a value;
- a `unit` function uses `return;` and must not provide an expression;
- reaching the closing brace of a `unit` function is an implicit `return;`;
- reaching the closing brace of an `i64` function is a compile-time error.

The entry point remains a source-defined `fn main() -> i64`. An external
declaration never supplies the entry point, and declaring an external function
named `main` is a compile-time error.

### Restricted external-function profile

The implemented O-series profile supports only top-level external declarations
whose parameters are by-value `i64` values and whose result is `i64` or
`unit`. Parameter names are mandatory. Alias parameters, `shared`, objects,
arrays, optionals, function values, variadic arguments, alternate link names,
and user-selected calling conventions are outside this profile.

Defined and external functions occupy one non-overloaded top-level function
namespace. Repeating a name is a compile-time error in every combination,
including two identical external declarations and an external declaration plus
a definition. As in the first slice, the first declaration remains selected
only for diagnostic recovery. Locals may shadow functions under the existing
lexical rules.

The source identifier of an external declaration is its exact linker symbol;
there is no mangling, module prefix, or `link_name` override in this profile.
Calls use stable resolved callable identities below name resolution rather than
reselecting the declaration by this string. Compiler-generated symbols for
Skald-defined functions must use a target-private spelling that cannot equal
any valid external source identifier. The compiler must not reserve an
otherwise valid identifier prefix merely to avoid its own symbol collisions.

External calls use the selected target's C ABI. On the initial Linux x86-64
System V target, Skald `i64` corresponds to C `int64_t`, and a Skald `unit`
result corresponds to C `void`. `unit` has no ABI payload or meaningful result
register. Calls evaluate arguments from left to right before entering the
external function, consistently with ordinary Skald calls.

An external declaration is a trusted statement about the linked symbol. The
compiler checks Skald call sites against the declared signature but cannot
verify the definition supplied to the linker. A missing symbol is a link error;
a supplied definition with an incompatible C ABI type is outside Skald's
language guarantees. General foreign linkage, cross-module declaration
coalescing, and ownership-bearing foreign calls remain unspecified.

Resolution assigns external and defined functions dense callable IDs in their
shared source order. Resolved IR, HIR, and MIR retain every external signature
and its exact-symbol linkage but allocate no definition/body entry for it.
Calls below resolution use only the stable ID. The x86-64 backend selects the
external symbol from declaration metadata and sends arguments through the same
System V call-lowering path used for Skald definitions. An unavailable symbol
therefore remains valid through compilation and fails only when the driver
invokes the linker.

## `bool` and conditional extension contract

C0 fixed the source and semantic contract for the C-series slice. C2 implements
the straight-line boolean grammar below through the x86-64 target. C3 and C4
provide verified multi-block MIR and backend branch support, and C5 implements
the conditional grammar below end-to-end.

The implemented straight-line subset adds these keywords:

```text
bool true false
```

The implemented conditional subset adds `if`, `elif`, and `else`. All use
only punctuation already present in the lexer. `true` and `false` are boolean
literals, not identifiers.

### Straight-line boolean grammar

C2 extends the implemented O-series grammar as follows:

```text
function-declaration          = "fn" identifier parameter-list "->" result-type block
external-function-declaration = "extern" "fn" identifier parameter-list "->" result-type ";"
parameter-list                = "(" [ parameter ("," parameter)* ] ")"
parameter                     = identifier ":" value-type
result-type                   = value-type | "unit"
value-type                    = "i64" | "bool"

local-declaration = "var" identifier ":" value-type "=" expression ";"
primary           = identifier
                  | decimal-integer
                  | "true"
                  | "false"
                  | "(" expression ")"
```

All other expression productions and precedence levels remain unchanged.
`bool` may appear in parameters, function results, initialized locals, and the
existing call expression path. `unit` remains payload-free and is not a
parameter or local type. The entry point remains exactly
`fn main() -> i64`.

`bool` is distinct from `i64`; neither implicitly converts to the other.
Local initializers, arguments, and return expressions must exactly match their
declared types. The literals `false` and `true` are the only literal boolean
values. This slice adds no casts, equality, ordering, logical negation,
`&&`, or `||`.

The restricted exact-symbol external profile accepts
by-value `bool` parameters and `bool` results alongside its existing `i64` and
`unit` forms. On Linux x86-64 System V, Skald `bool` maps to C `bool`
(`_Bool`). Outgoing values are canonical false or true. An external boolean
result is normalized from the ABI result byte before it becomes a Skald value.
Alias parameters, ownership-bearing values, alternate symbol names, variadic
calls, and user-selected calling conventions remain unsupported.

### Conditional grammar and semantics

C5 adds the implemented statement production:

```text
statement    = local-declaration
             | return-statement
             | call-statement
             | if-statement
             | block
if-statement = "if" "(" expression ")" block
               ("elif" "(" expression ")" block)*
               ["else" block]
```

Conditions and blocks are mandatory. There may be any number of `elif` arms
and at most one final `else`. `elif` is the only chained-arm spelling;
`else if` is invalid because `else` must be followed immediately by a block.
The construct is a statement and produces no value.

Every condition must have type exactly `bool`. Conditions are evaluated from
left to right until the first `true` result. Only that arm executes, and later
conditions and blocks are skipped. When every condition is `false`, the
`else` block executes if present; otherwise execution continues after the
statement. Integers and other values are not conditions without a separately
specified explicit conversion.

Every condition resolves in the scope containing the whole `if` statement.
Every arm block creates an independent child scope. A binding declared in an
arm is unavailable in other arms, later `elif` conditions, and after the
statement. Existing nested-block shadowing rules apply within an arm.

An `if` statement definitely returns only when it has an `else` block and
every `if`, `elif`, and `else` block definitely returns. The rule composes
through nested blocks and conditionals. Consequently, a non-`unit` function
may rely on an exhaustive, all-returning conditional to satisfy its mandatory
return, while a conditional without `else` can never do so by itself. Unit
functions retain implicit fallthrough return.

The initial conditional profile does not include `if` expressions, implicit
truthiness, boolean operators or casts, pattern matching, optional presence
tests, flow-sensitive narrowing, loops, or branch optimization. Constant
conditions retain ordinary source semantics; optimization is not required for
correctness.

C6 completes this grammar slice's external coverage with nested exhaustive
conditionals, non-exhaustive return rejection, exact parser and semantic
diagnostics, and repeated-process determinism checks. It does not expand the
grammar or introduce any of the excluded forms above.

## Remaining primitive extension contract

T0 fixes the syntax and semantic contract for the planned `u64`, `u8`, and
`f64` extension. These forms are not implemented merely because they appear in
this section; T3, T4, and T6 enable them only after each has a complete path
through the supported backend.

T2 implements the shared numeric scanner and carries an explicit literal kind,
original spelling, and complete span through the source and resolved IR. T3
enables `u64` and the concise `u` suffix end-to-end. Contracted `u8` and `f64`
spellings remain recognized at the lexical boundary but deliberately invalid
until T4 and T6. This feature gate prevents a parser or later phase from
guessing a type by inspecting suffix text.

The extension adds these case-sensitive type keywords:

```text
u64 u8 f64
```

`double` is not a type keyword and remains an ordinary identifier. The numeric
literal grammar is:

```text
ascii-digit     = "0" | "1" | "2" | "3" | "4"
                | "5" | "6" | "7" | "8" | "9"
decimal-digits  = ascii-digit+
exponent        = ("e" | "E") ["+" | "-"] decimal-digits

i64-literal     = decimal-digits
u64-literal     = decimal-digits "u"
u8-literal      = decimal-digits "u8"
f64-literal     = decimal-digits "." decimal-digits [exponent]
                | decimal-digits exponent
numeric-literal = i64-literal | u64-literal | u8-literal | f64-literal
```

The alternatives are classified by their complete spelling: the suffix is
part of one numeric token, not an identifier token following an integer.
Numeric-looking malformed text is consumed together where possible. This
includes unknown or uppercase suffixes, `u64`, incomplete exponents, repeated
decimal points, digit separators, and identifier tails. A decimal point must
have digits on both sides, so `.5` and `1.` are rejected in this profile.

An unsuffixed integer always has type `i64`; expected type does not reinterpret
it. The suffix `u` selects `u64`, and `u8` selects `u8`. Decimal-point and
exponent forms select `f64`; there is no `f64` suffix. Leading `-` remains the
existing unary operator and is never part of a literal token.

Integer bounds are checked during type checking:

- `u64`: `0u` through `18446744073709551615u`;
- `u8`: `0u8` through `255u8`;
- `i64`: the existing range and unary-minus `i64::MIN` rule remain unchanged.

A decimal `f64` spelling is converted to nearest IEEE-754 binary64 with ties
to even. Results may be subnormal or underflow to positive zero. A literal
that rounds to infinity is rejected as out of range. Source spelling and span
remain available for diagnostics, while typed IR stores the resulting raw bits
and deterministic dumps use 16 lowercase hexadecimal digits.

The extended type productions are:

```text
value-type  = "i64" | "u64" | "u8" | "bool" | "f64"
result-type = value-type | "unit"
primary     = identifier
            | numeric-literal
            | "true"
            | "false"
            | "(" expression ")"
```

Every initializer, argument, non-`unit` return, and binary arithmetic operand
must match exactly. There is no expected-type literal inference, implicit
promotion, signed/unsigned mixing, or primitive cast in this slice. Conditions
still require exactly `bool`, and `main` remains exactly
`fn main() -> i64`.

The existing `+`, `-`, and `*` tokens apply to equal numeric operand types.
Unsigned results wrap modulo their width, with `u8` canonicalized to `0..=255`
at every observable boundary. `f64` uses IEEE-754 binary64 operations under the
default round-to-nearest, ties-to-even environment. Unary `-` accepts `i64` and
`f64`, but not `u64` or `u8`. Division, remainder, exponentiation, bitwise and
shift operators, casts, comparisons, and implicit conversions remain outside
this extension.

The restricted external profile maps `u64`, `u8`, and `f64` to C `uint64_t`,
`uint8_t`, and compatible binary64 `double`. On Linux x86-64 System V, integer
and SSE arguments use independent register sequences, `u8` is normalized at
Skald boundaries, and `f64` returns in `%xmm0`. The exact ABI and bootstrap
output records are normative in Sections 3.1 and 13.3 of the draft
specification.

## First vertical slice name resolution

M3 uses two passes over a single compilation unit. The first pass collects every uniquely named top-level function in source order; the second resolves function bodies. Calls may therefore refer to functions declared later in the file and may be recursive. Function overloading is not part of the first slice, so repeating a top-level function name is an error and the first declaration remains the selected one.

Function, parameter, and local identities are dense, deterministic IDs assigned in source order. Parameter and local IDs include their owning function ID. Resolved binding uses contain a parameter or local ID, and resolved direct calls contain a function ID; later phases must not compare source names to choose declarations. Resolution also selects the unique function named `main` as the entry candidate, if present, while M4 owns entry-signature validation and the missing-entry diagnostic.

Name visibility follows these rules:

- parameters and the function body's outermost block share one lexical scope;
- a duplicate parameter, or a top-level local with the same name as a parameter, is an error;
- a local becomes visible only after its initializer, so its initializer cannot refer to the binding being declared;
- a nested block creates a scope and may shadow a binding in an enclosing scope;
- leaving a nested block restores the enclosing binding;
- duplicate names in the same lexical scope are errors, and the first binding remains selected for recovery;
- a local binding shadows a function name, including at a call site.

The first slice has no function values. A bare function name is therefore invalid as a value. A direct-call target must be an unparenthesized identifier that resolves to a function; calling a local, calling an unknown name, or calling another expression form is diagnosed. Resolution returns a partial resolved program with accumulated diagnostics, but later semantic phases must not run when resolution reports errors.

## First vertical slice type checking

M4 has one semantic type, `i64`. Every function parameter, local, return value, literal, binding expression, call, grouped expression, and arithmetic expression has that type. Local initializers and return expressions must match their declarations, unary `-` requires `i64`, and binary `+`, `-`, and `*` require two `i64` operands and produce `i64`. The typed HIR records these choices as `NegateI64`, `AddI64`, `SubtractI64`, and `MultiplyI64` operations rather than carrying unresolved source operators forward.

Every direct call is checked against the already resolved function ID. Its argument count must exactly match the target's parameter count, argument types are checked positionally, and the HIR call retains the same exact function ID. Argument and operand evaluation order remains a later MIR concern; M4 preserves source order in its vectors and expression tree.

Decimal literals are converted during M4:

- `0` through `9223372036854775807` are valid positive `i64` values;
- unary minus may directly enclose, with optional grouping parentheses, the magnitude `9223372036854775808`, producing `-9223372036854775808` (`i64::MIN`);
- that magnitude is invalid without the enclosing unary minus;
- larger positive or negative magnitudes are diagnosed as out of range.

This special treatment of `i64::MIN` is signed-literal normalization, not general constant folding. Arithmetic overflow behavior remains outside the first-slice contract.

An `i64`, `u64`, or `bool` function must return a value on every reachable path. A
return in an unconditionally executed nested block satisfies this requirement,
as does an `if` statement with a final `else` when every arm definitely returns. A
`unit` function may use `return;` or reach its closing brace; attaching any
expression to its return is invalid. Conversely, `return;` is invalid in an
`i64` function. Unit-returning calls have type `unit`, which cannot be used in
an `i64` value context. Expression statements accept only calls returning
`unit`; in particular, an `i64` call cannot be silently discarded. The entry
candidate selected by resolution must still have the exact signature
`fn main() -> i64`.

Type checking accumulates diagnostics across functions but emits an executable `HirProgram` only when the entire resolved program succeeds. Consequently, every expression in an available HIR program has a concrete type, every operation is selected, every call has a checked arity and exact target, and the entry function is valid.

## First vertical slice evaluation and MIR

M5 fixes expression evaluation order to left-to-right. A binary expression completely evaluates its left operand before its right operand. A direct call evaluates arguments completely in source order before performing the call. Nested expressions follow the same recursive rule. This ordering is part of the first-slice language behavior, even where current `i64`-only expressions have no visible side effects beyond calls.

MIR separates addressable storage from transient computed values:

- parameters and source locals receive dense, owner-qualified storage IDs;
- constants, loads, unary and binary results, and value-returning call results receive dense, owner-qualified value IDs;
- local initialization becomes an explicit value computation followed by a store;
- reading a parameter or local becomes an explicit load;
- arithmetic and direct calls are ordered three-address instructions;
- return is a basic-block terminator with an optional operand selected by the
  declared result type.

C3 extends target-independent MIR with explicit `Goto` and boolean `Branch`
terminators. Blocks and branch targets have stable owner-qualified IDs, and
terminators expose successors in semantic order. C5 lowers source conditionals
to explicit condition, arm, false-continuation, and optional join blocks. It
omits a join when every exhaustive arm terminates and continues to omit source
statements after an unconditional return.

Successful lowering runs the MIR verifier in debug builds. The verifier checks
function ownership and density of storage, value, and block IDs; parameter
storage order; single definitions; block-local definition-before-use; operand
and storage types; direct-call targets, argument counts, and signature types;
return types; entry blocks; branch condition types; target ownership and
existence; and block termination. Every represented block is checked even when
unreachable. Storage, rather than transient values, carries state across block
edges until a later SSA design explicitly changes that rule. Backends consume
verified MIR and do not inspect HIR, resolved source names, or the AST.

O2 changed the compiler representation without changing first-slice language
behavior. Resolved IR, typed HIR, and MIR store dense callable
declarations separately from optional local definitions. A declaration owns
the stable function ID, canonical signature, and linkage; a definition owns
the body and body-local state. MIR calls are dedicated instructions with an
explicit direct target and optional result ID. O3 uses that representation:
every `i64` call has a result, while a `unit` call has none. Unit returns also
have no MIR operand, and implicit fallthrough in a unit function lowers to the
same payload-free return. The verifier rejects unit-typed storage and transient
values, making the no-payload rule explicit. The backend derives internal or external symbols from declaration
linkage; call instructions never contain linker-symbol strings.

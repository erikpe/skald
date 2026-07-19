# Grammar

This directory is reserved for the canonical Skald grammar and parser-facing grammar notes. The complete language grammar is still an identified specification gap. The contracts below are normative only for the implemented vertical-slice milestones.

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

Every first-slice function returns `i64` and must contain an unconditional return. Because M4 has no conditional control flow, a return in an unconditionally executed nested block also satisfies this requirement. The entry candidate selected by M3 must exist and have the exact signature `fn main() -> i64`.

Type checking accumulates diagnostics across functions but emits an executable `HirProgram` only when the entire resolved program succeeds. Consequently, every expression in an available HIR program has a concrete type, every operation is selected, every call has a checked arity and exact target, and the entry function is valid.

## First vertical slice evaluation and MIR

M5 fixes expression evaluation order to left-to-right. A binary expression completely evaluates its left operand before its right operand. A direct call evaluates arguments completely in source order before performing the call. Nested expressions follow the same recursive rule. This ordering is part of the first-slice language behavior, even where current `i64`-only expressions have no visible side effects beyond calls.

MIR separates addressable storage from transient computed values:

- parameters and source locals receive dense, owner-qualified storage IDs;
- constants, loads, unary and binary results, and call results receive dense, owner-qualified value IDs;
- local initialization becomes an explicit value computation followed by a store;
- reading a parameter or local becomes an explicit load;
- arithmetic and direct calls are ordered three-address instructions;
- return is a basic-block terminator using an already computed value.

The first slice has no branches, so each lowered function has one entry basic block. Unconditionally unreachable statements after a return are not lowered. Blocks still have explicit IDs and terminators, allowing conditional control-flow edges and additional blocks to be introduced without redesigning instruction or function representation.

Successful lowering runs the MIR verifier in debug builds. The verifier checks function ownership and density of storage, value, and block IDs; parameter storage order; single definitions and use-before-definition; operand and storage types; direct-call targets, argument counts, and signature types; return types; entry blocks; and block termination. Backends consume verified MIR and do not inspect HIR, resolved source names, or the AST.

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

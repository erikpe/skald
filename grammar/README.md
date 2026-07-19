# Grammar

This directory is reserved for the canonical Skald grammar and parser-facing grammar notes. The complete grammar is still an identified specification gap, so no file here is normative yet.

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

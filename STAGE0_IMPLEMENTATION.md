# Stage-0 Implementation Steps (Python)

This document outlines concrete steps to get the stage-0 language subset operational using Python for the compiler. The goal is a small, working compiler that emits Linux x86-64 assembly and can compile simple programs with defer.


## 1) Prerequisites

- Python 3.11+ (3.10 OK if needed).
- Linux x86-64 toolchain for assembling and linking:
  - clang (or gcc)
- Optional for Windows host: WSL2 with Ubuntu (recommended).


## 2) Project Layout (Suggested)

- src/
  - tokens.py
  - lexer.py
  - parser.py
  - ast.py
  - types.py
  - symbols.py
  - typecheck.py
  - codegen.py
  - main.py
- runtime/
  - runtime.c
- tests/
  - *.toy
  - *.out


## 3) Stage-0 Language Subset (Target)

Implement these features first:
- Types: i64, bool, unit, *T
- Decls: fn, extern fn, struct
- Statements: var, if/else, while, return, defer, block, expr;
- Expressions: literals, variables, unary, binary, call, field, assignment


## 4) Step-by-Step Build

Step 1: Tokenizer
- Implement a token stream with kind, lexeme, line, column.
- Support keywords, identifiers, integer literals, operators, punctuation.
- Ensure longest-match for operators (== != <= >= && || ->).

Step 2: AST Definitions
- Implement minimal dataclasses for AST nodes.
- Keep lvalues as standard Expr nodes (Var, Unary(*), Field).

Step 3: Parser (Recursive Descent)
- Use the grammar in GRAMMAR_EBNF.md.
- Implement expression precedence ladder.
- Parse declarations in a loop until EOF.

Step 4: Symbol Tables and Struct Layout
- Global tables for structs and functions.
- Per-function scopes for locals.
- Compute struct field offsets and size (C-like alignment).

Step 5: Type Checking
- Assign types to expressions and statements.
- Enforce:
  - var requires initializer
  - assignment target is lvalue
  - operator operand types are valid
  - function call args match params
- Decide how null is typed (only where *T is expected).

Step 6: Defer Lowering (Single Exit)
- Add a hidden return slot for non-unit functions.
- Rewrite return statements to store into ret_slot and jump to fn_exit.
- Defer is call-form: defer f(args); capture args at registration.

Step 7: Codegen (Intel x86-64)
- Emit prologue/epilogue and stack allocation.
- Generate assembly for expressions and statements.
- Implement short-circuit for && and || with labels.
- Respect SysV ABI for calls.

Step 8: Runtime (C)
- Implement minimal runtime:
  - print_i64, read_i64 (optional)
  - malloc/free/realloc wrappers
- Keep runtime ABI compatible with your language.

Step 9: Build/Run Pipeline
- Compile .toy -> out.s (your compiler)
- Link with runtime:
  - clang out.s runtime/runtime.c -o prog
- Run and compare output.


## 5) Minimal Compiler API (Python)

You can wire a simple driver:

- main.py
  - parse args: input file, output .s path
  - run: lex -> parse -> typecheck -> codegen
  - write .s


## 6) Testing Strategy

Golden tests
- tests/add.toy, tests/add.out
- tests/while_sum.toy, tests/while_sum.out
- tests/defer_return.toy, tests/defer_return.out

Test runner (optional)
- A Python script that compiles each .toy, links with runtime, runs, and diffs output.


## 7) Quick Milestones

- M1: Expressions + print_i64
- M2: if/else + while
- M3: functions + calls
- M4: defer on return paths
- M5: structs + field access


## 8) Common Pitfalls

- Stack alignment: ensure 16-byte alignment before call.
- Call-clobbered registers: save or avoid keeping values across calls.
- Defer ordering: LIFO, per scope.
- Return handling: all returns must go through fn_exit.


## 9) Next Steps (After Stage-0)

- Implement lexer/parser in the toy language and compile with stage-0.
- Port type checker and codegen.
- Gradually expand the language (arrays, casts, etc.).

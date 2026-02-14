# Skald Language MVP Specification (Stage-0)

This document defines a minimal, self-contained MVP language and compiler plan for a Skald, statically typed language. It is written to be usable by both humans and AI agents. The goal is to build a handcrafted lexer, parser, type checker, and x86-64 code generator (Linux SysV ABI). The compiler is first implemented in a host language (e.g., Python) and later reimplemented in the Skald language for self-hosting.

Contents
- Goals and non-goals
- Language principles
- MVP language spec
- Memory model and defer
- Suggested compiler pipeline
- Data structures (AST, types, symbols)
- Defer lowering strategy
- Minimal IR (optional)
- x86-64 codegen patterns
- Runtime and toolchain
- Bootstrapping plan
- Testing strategy


## Goals and Non-goals

Goals
- Statically typed, small, consistent syntax.
- Handcrafted lexer and recursive descent parser.
- Manual memory management (C-style) with a safer cleanup mechanism using defer.
- Structs and raw pointers.
- Linux x86-64 target only (System V ABI).
- Fast path to a working compiler for a few contest-style demo programs.

Non-goals (MVP)
- No garbage collector.
- No ownership/borrow checker.
- No exceptions or stack unwinding.
- No generics, macros, or modules.
- No full C ABI compatibility beyond basic call/return.


## Language Principles

- Explicit, simple rules over cleverness.
- No hidden allocations.
- No implicit conversions between numeric types (add casts later if desired).
- Every control-flow path is explicit for the compiler (important for defer lowering).


## MVP Language Specification

### Lexical
- Identifiers: [A-Za-z_][A-Za-z0-9_]*
- Integer literals: decimal only.
- Comments: // line comment, /* ... */ block comment (optional but easy).
- Keywords:
  fn, struct, let (optional), var, if, else, while, return, defer, true, false, null, extern, sizeof, as

### Types
Built-in:
- i64, u64, u8, bool, unit

Pointer types:
- *T

Structs:
- struct Name { field: Type, ... }

Notes
- unit is the type with exactly one value. It represents "no meaningful value" (typed void).
- No arrays or generics in MVP (optional later).

### Declarations
- struct declaration
- function declaration
- extern function declaration

Examples
- struct Pair { a: i64, b: i64 }
- extern fn malloc(size: u64) -> *u8;
- fn add(x: i64, y: i64) -> i64 { return x + y; }

### Statements
- Block: { stmt* }
- Var declaration: var x: T = expr;
- Assignment: x = expr;  *p = expr;  obj.field = expr;
- if / else
- while
- return expr; or return; (only in unit-returning functions)
- defer f(x, y);

### Expressions
- Literals: integer, true, false, null
- Variables
- Unary: -x, !x, *p, &x
- Binary: + - * / %  == != < <= > >=  && ||
- Calls: f(a, b)
- Field access: expr.field
- Pointer field access sugar: ptr->field (equivalent to (*ptr).field)
- Compile-time size query: sizeof(T)
- Explicit casts: expr as Type
- Grouping: (expr)


## Memory Model and Defer

Memory model (MVP)
- Stack locals: automatic storage.
- Heap: explicit allocation via malloc/free/realloc in the runtime.
- Pointers are raw and copy by value (no ownership tracking).

Defer
- defer f(x, y); registers a call to run when the current lexical scope exits.
- Arguments are captured at the point of registration.
- Defers run in LIFO order per scope.
- Defers run on normal scope exit and on return that exits the scope.
- No exceptions in MVP. If a later panic/abort is added, define it as "no defers".


## Suggested Compiler Pipeline (Stage-0)

1) Lexing
- Convert source into a token stream with file/line/column spans.

2) Parsing
- Handwritten recursive descent parser.

3) Name resolution + type checking
- Resolve symbols per scope.
- Type check expressions and statements.
- Build struct layout table (field offsets, size, alignment).

4) Lowering / Desugaring
- Transform return into single-exit form.
- Manage defer stacks and scope-exit cleanup.

5) Codegen
- Emit x86-64 assembly (Intel syntax) for SysV ABI.
- Link with a tiny C runtime to provide I/O and memory primitives.


## Data Structures (Concrete Shapes)

AST nodes (example minimal set)
- Program(decls)
- StructDecl(name, fields)
- FnDecl(name, params, ret, body)
- ExternFnDecl(name, params, ret)

Types (semantic, after type check)
- TyI64, TyU64, TyI32, TyU32, TyBool, TyUnit
- TyI64, TyU64, TyU8, TyBool, TyUnit
- TyPtr(pointee)
- TyStruct(name)
- TyFn(params, ret)

Symbol tables
- Global: structs, functions
- Local: stack of scopes with local symbols and stack offsets


## Defer Lowering Strategy

Goal: make the backend simple and still ensure defers run.

Simple approach
- Use a hidden return slot for non-unit functions.
- Replace every return expr with:
  - evaluate expr into ret_slot
  - jump to fn_exit label
- At fn_exit, emit all pending defers in correct order, then return.

Implementation hint
- Maintain a stack of scopes in codegen, each with a list of defer blocks.
- When a scope exits normally, emit its defers in reverse order.
- When returning, emit defers for all active scopes (inner to outer).


## Minimal IR (Optional)

You can skip this and generate assembly directly from AST, but a tiny IR can help debugging.

IR sketch
- IRBlock(label, insns, terminator)
- Insns: BinOp, Load, Store, Call, Cmp
- Terminators: Jmp, Cjmp, Ret


## x86-64 Codegen Patterns (Intel Syntax)

Literals
- i64: mov rax, IMM64
- bool: mov rax, 0 or 1
- null: xor rax, rax

Local load/store
- load: mov rax, qword ptr [rbp - off]
- store: mov qword ptr [rbp - off], rax

Address-of
- lea rax, [rbp - off]

Deref load/store
- load: mov rax, qword ptr [rax]
- store: mov qword ptr [rcx], rax

Arithmetic
- add/sub/imul on rax/rcx with stack temps
- div/mod using cqo + idiv

Compare
- cmp rcx, rax
- set<cond> al
- movzx rax, al

Short-circuit
- Implement && and || with labels and conditional jumps

Calls
- Args in rdi, rsi, rdx, rcx, r8, r9
- Return in rax
- Align stack to 16 bytes at call boundary


## Runtime and Toolchain

Minimal runtime functions (C)
- print_i64(x: i64) -> unit
- read_i64() -> i64 (optional)
- malloc(size: u64) -> *u8
- free(p: *u8) -> unit
- realloc(p: *u8, size: u64) -> *u8

Toolchain
- Emit .s and link with gcc:
  gcc out.s runtime.c -o prog
- Target Linux x86-64 SysV ABI


## Bootstrapping Plan

Stage-0
- Implement compiler in Python.
- Build a tiny runtime in C.

Stage-1
- Reimplement lexer and parser in the Skald language.
- Compile them with stage-0.

Stage-2
- Port type checker and codegen.
- Self-hosting compiler can compile itself.


## Testing Strategy

Golden tests
- Compile sample program to .s
- Link and run
- Compare stdout against expected output

Suggested test cases
- arithmetic precedence
- if/else
- while loop sum
- function call and return
- defer runs on early return


## Minimal Example

fn main() -> i64 {
  var p: *u8 = malloc(16);
  defer free_ptr(p);

  print_i64(123);
  return 0;
}

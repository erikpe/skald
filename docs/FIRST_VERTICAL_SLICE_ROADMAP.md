# First Vertical Slice Roadmap

Status: proposed implementation roadmap.

The first vertical slice proves that the complete Skald toolchain can translate one source file into an observable native Linux program. Its purpose is to validate architecture and interfaces, not to approximate the full language prematurely.

## 1. Completion Demonstration

The slice is complete when this kind of program can be compiled, linked, and observed through its process exit status:

```ska
fn twice(value: i64) -> i64 {
    return value * 2;
}

fn main() -> i64 {
    var result: i64 = twice(20);
    return result + 2;
}
```

The expected observable result is exit status `42` on Linux.

The end-to-end path must be the real intended path:

```text
.ska source
  → lexer
  → parser
  → resolution
  → type checking and typed HIR
  → MIR
  → x86-64 SysV backend
  → textual assembly
  → system assembler/linker + Skald C runtime
  → executable
  → exit status
```

No phase may be bypassed with hard-coded assembly for the demonstration program.

## 2. Language Subset

### Included

- one UTF-8 source file with the canonical `.ska` suffix;
- top-level functions defined in that file;
- one primitive type: `i64`;
- function parameters passed by value;
- local `var` declarations with mandatory initializers;
- decimal `i64` literals;
- local references;
- direct calls to functions in the same file;
- `return` statements;
- parentheses;
- unary negation and binary `+`, `-`, and `*` over `i64`;
- semicolon-terminated declarations and statements where required by the language syntax;
- an entry function with exact signature `fn main() -> i64`.

In this roadmap, “local functions” means functions defined in the same single-file compilation unit. Nested function declarations are not included.

### Explicitly excluded

- additional source files, modules, imports, or exports;
- external functions or foreign calls;
- classes, methods, `init`, `assign`, and `destroy`;
- `shared`, alias parameters, object lifetime operations, or heap allocation;
- every primitive type other than `i64`;
- optionals and arrays;
- strings, standard-library facilities, input, or printed output;
- conditionals, loops, `break`, and `continue`;
- checked exceptions and panic handling;
- function values, closures, interfaces, inheritance, and virtual dispatch;
- global or static variables;
- casts, type tests, indexing, and slicing;
- user-selectable optimization levels;
- AArch64 code generation.

Unsupported syntax must produce a diagnostic or be rejected by the grammar. It must not be silently accepted with placeholder semantics.

### Deliberately narrow semantic edge cases

The vertical slice tests use arithmetic whose mathematical result is representable by `i64`. Full overflow, division, and remainder behavior are specification gaps and are not silently settled by this roadmap. Division and remainder are excluded from the slice.

Function arguments and expression operands should be lowered in a deterministic documented order from the start. The initial implementation should use left-to-right evaluation so calls introduced into expressions do not later expose accidental backend ordering.

## 3. Entry Point and Observable Behavior

The source program must define exactly one valid `main` function:

```ska
fn main() -> i64
```

It takes no parameters. Missing, duplicate, parameterized, or incorrectly typed entry functions are compile-time errors.

The generated executable returns the result of Skald `main` to the Linux process environment. Shell-visible exit status is limited by Linux/POSIX process conventions, so golden execution cases should use expected results in `0..=255`. The Skald function still computes an `i64`; narrowing at the process boundary is an entry-wrapper/toolchain concern and must be documented by the initial ABI implementation.

There is no source-level input or output in this slice. Compiler diagnostics use stderr, but compiled Skald programs communicate only through exit status.

## 4. Initial CLI Contract

The desired user-facing commands are:

```text
skac input.ska --emit asm -o build/input.s
skac input.ska -o build/input
```

The assembly-only form stops after deterministic textual assembly emission. The executable form emits assembly and then invokes the host C compiler driver to assemble and link it with `libskald_runtime.a`.

Initial constraints:

- host and target are Linux x86-64 System V;
- the system C compiler driver is used as assembler/linker frontend;
- subprocess failures are reported clearly, including which tool failed;
- normal diagnostics do not include unstable absolute paths when a repository-relative path is available;
- `--help` and `--version` work independently of compilation.

## 5. Milestones

Each milestone ends with tests and a deterministic dump or artifact. Later milestones consume the public output of earlier ones.

Progress summary:

- [x] M0 — Repository and quality baseline
- [ ] M1 — Source ownership, diagnostics, and lexing
- [ ] M2 — Parser and AST
- [ ] M3 — Declaration collection and resolution
- [ ] M4 — Type checking and typed HIR
- [ ] M5 — MIR lowering and verification
- [ ] M6 — x86-64 System V backend
- [ ] M7 — Runtime, link driver, and native execution
- [ ] M8 — Vertical-slice hardening

### M0 — Repository and quality baseline

- [x] Cargo workspace with thin `skac` binary and `skald-compiler` library.
- [x] Explicit phase modules matching the architecture document.
- [x] Minimal buildable C runtime archive and direct ABI smoke test.
- [x] Compiler, runtime, and golden test categories.
- [x] Formatting, lint, and test commands documented and available through the root `Makefile`.
- [x] No third-party Rust dependencies introduced by M0.
- [x] Basic tests for the pre-pipeline `--help`, `--version`, and usage-error behavior of `skac`.

- [x] **Exit criterion:** the Rust workspace checks, the C runtime builds and passes its ABI smoke test, and repository structure matches its documentation.

### M1 — Source ownership, diagnostics, and lexing

- [ ] Source IDs, byte ranges, line maps, and spans.
- [ ] Structured diagnostics with stable plain-text rendering.
- [ ] Tokens for the included subset.
- [ ] Decimal `i64` literal scanning with malformed-literal diagnostics.
- [ ] Deterministic token dump.

- [ ] **Exit criterion:** lexer unit tests cover valid tokens, whitespace/comments selected by the grammar, invalid characters, malformed literals, and accurate spans.

### M2 — Parser and AST

- [ ] Source AST for functions, parameters, blocks, local declarations, returns, calls, and included expressions.
- [ ] Explicit precedence and associativity for unary negation and `+`, `-`, `*`.
- [ ] Recovery sufficient to report more than one independent syntax error when practical.
- [ ] Deterministic AST dump.

- [ ] **Exit criterion:** parser tests cover the demonstration program, precedence, malformed declarations, missing punctuation, and recovery without semantic lookup.

### M3 — Declaration collection and resolution

- [ ] Stable function, parameter, and local IDs.
- [ ] Single-file function table.
- [ ] Lexical local scopes.
- [ ] Duplicate declaration, unknown name, and invalid call-target diagnostics.
- [ ] Direct calls resolved to function IDs.

- [ ] **Exit criterion:** later phases never resolve source strings to choose declarations.

### M4 — Type checking and typed HIR

- [ ] The sole semantic type `i64`.
- [ ] Function signature and entry-point validation.
- [ ] Type checking for literals, locals, calls, return values, and arithmetic.
- [ ] Explicit typed operation and direct-call nodes in HIR.
- [ ] Deterministic HIR dump.

- [ ] **Exit criterion:** every executable HIR expression has a type and every call has an exact target and checked arity.

### M5 — MIR lowering and verification

- [ ] Explicit function bodies, local storage/value IDs, calls, arithmetic, and returns.
- [ ] Deterministic left-to-right evaluation.
- [ ] Basic blocks and terminators, even though the slice has no conditional branch.
- [ ] MIR verifier for ownership of IDs, operand types, call signatures, and terminated blocks.
- [ ] Deterministic MIR dump.

- [ ] **Exit criterion:** no source-name lookup or AST traversal is required below MIR lowering.

### M6 — x86-64 System V backend

- [ ] Target registry with `x86_64-sysv` as the only accepted target.
- [ ] Integer argument and return lowering for the required arities.
- [ ] Stack-frame layout for parameters, locals, calls, and temporaries.
- [ ] Instruction selection for literals, copies, calls, negation, addition, subtraction, and multiplication.
- [ ] Correct stack alignment and callee-saved register behavior.
- [ ] Deterministic GNU-compatible textual assembly.
- [ ] Target legality checks that reject unsupported MIR rather than miscompile it.

The simplest correct register strategy is acceptable initially, including stack-heavy code. Register allocation is an isolated backend concern and can improve later without changing MIR.

- [ ] **Exit criterion:** assembly-shape tests cover ABI edges and generated assembly can be assembled successfully.

### M7 — Runtime, link driver, and native execution

- [x] Versioned minimal runtime ABI and static archive established by M0.
- [ ] Generated or linked entry-point boundary for `fn main() -> i64`.
- [ ] Driver support for assembly-only and executable output.
- [ ] Robust host tool invocation and failure reporting.
- [ ] Native golden runner recording process exit status.

- [ ] **Exit criterion:** the demonstration program and several function/call/arithmetic variants produce their expected exit statuses.

### M8 — Vertical-slice hardening

- [ ] Compile-failure golden cases for every supported syntactic and semantic category.
- [ ] Deterministic output checked across repeated runs.
- [ ] No Rust panic for malformed source in the supported grammar surface.
- [ ] MIR verifier run in tests and appropriate debug/development paths.
- [ ] Architecture documentation reconciled with implementation.
- [ ] A clean boundary list for the next language slice.

- [ ] **Exit criterion:** all compiler, runtime, and golden suites pass from a clean checkout using documented commands.

## 6. Test Matrix for the Slice

Minimum successful golden cases:

- [ ] Constant exit value.
- [ ] Unary negative value routed through in-range arithmetic to a nonnegative exit status.
- [ ] Local initialization and return.
- [ ] One direct function call.
- [ ] Multiple parameters within register-passed SysV arguments.
- [ ] A call result used by another arithmetic expression.
- [ ] Nested calls that validate evaluation and temporary handling.

Minimum compile-failure cases:

- [ ] Invalid token and malformed integer literal.
- [ ] Missing semicolon, delimiter, or return expression.
- [ ] Duplicate function, parameter, or local name where prohibited.
- [ ] Unknown local or function.
- [ ] Wrong call arity.
- [ ] Unsupported type.
- [ ] Missing or invalid `main`.
- [ ] Unsupported language construct with a clear diagnostic.

Minimum backend/runtime checks:

- [ ] Assembly accepted by the system toolchain.
- [x] Runtime archive builds with warnings treated as errors.
- [ ] Stack alignment across a nested call.
- [ ] Exit status propagation for representative values.
- [ ] Toolchain failure produces a driver error rather than a compiler panic.

## 7. Quality Gates

The vertical slice is not complete merely because one program runs. Completion requires:

- [x] `cargo fmt --check` is available and currently passes.
- [x] `cargo clippy --workspace --all-targets` runs with warnings denied and currently passes.
- [x] `cargo test --workspace` is available and currently passes.
- [x] Runtime C build and runtime ABI smoke test are available and currently pass.
- [ ] Golden suite passes on Linux x86-64.
- [x] `git diff --check` currently passes.
- [ ] Documented phase dumps are usable for debugging.
- [ ] No known phase-boundary shortcut remains that later work must immediately undo.

Compile-time performance should be measured once a meaningful corpus exists. The first slice should avoid obviously expensive architecture—especially repeated whole-program scans and repeated string-based lookup—but should not build caching or incremental compilation before measurements justify it.

## 8. Immediately Following the Slice

The next feature slice should be selected from demonstrated architectural needs rather than from a desire to maximize syntax quickly. Plausible next steps include `bool` and `if`, broader primitive arithmetic, additional statements, or the first deterministic inline object. Arrays, optionals, loops/iterators, and checked exceptions remain explicitly deferred in the language specification.

AArch64 should follow after the target interface and MIR have survived enough x86-64 work to expose their real boundaries. Its implementation is an architectural test: semantic phases should remain unchanged, while ABI lowering, instruction selection, frame/register planning, and assembly emission are supplied by the new backend.

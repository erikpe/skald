# `bool` and Conditional Control Flow Roadmap

Status: C0–C1 complete; C2 is the next implementation task.

This roadmap adds the `bool` primitive type, bootstrap boolean output, and
Niflheim-style `if` / `elif` / `else` statements. It is split into reviewable,
PR-sized tasks that keep the compiler buildable and preserve all previously
supported programs after every task.

The completed slice should compile and run programs such as:

```ska
extern fn ska_rt_println_bool(value: bool) -> unit;

fn report(primary: bool, secondary: bool) -> unit {
    if (primary) {
        ska_rt_println_bool(true);
    }
    elif (secondary) {
        ska_rt_println_bool(false);
    }
    else {
        ska_rt_println_bool(primary);
    }
}

fn main() -> i64 {
    report(false, true);
    return 0;
}
```

with exact stdout:

```text
false
```

`elif` is a distinct keyword, as in Niflheim. `else if` is not an alternate
spelling. Conditions use parentheses, every arm uses a block, and the complete
construct is a statement rather than a value-producing expression.

## 1. Scope and Design Constraints

### Included

- the primitive type `bool` in function parameters and results, initialized
  locals, expressions, calls, and restricted external declarations;
- the literals `true` and `false`;
- exact type checking with no implicit numeric truthiness;
- `if`, zero or more `elif` arms, and an optional final `else` arm;
- source-ordered, short-circuit selection of conditional arms;
- lexical scope for every arm block;
- branch-aware return-completeness analysis;
- target-independent multi-block MIR with explicit conditional and
  unconditional terminators;
- control-flow-aware MIR verification;
- deterministic x86-64 System V block and branch lowering;
- `ska_rt_println_bool(bool)` in the C runtime, writing `true` or `false` and
  one LF to stdout;
- exact native and compile-failure golden coverage.

### Explicitly excluded

- `else if` as an alias for `elif`;
- `if` expressions or values produced by conditional arms;
- implicit conversion of integers, objects, handles, or optionals to `bool`;
- explicit primitive casts to or from `bool`;
- equality, ordering, logical negation, `&&`, and `||`;
- conditional local declarations without an initializer;
- pattern matching, optional presence tests, or flow-sensitive type narrowing;
- loops, `break`, `continue`, and exceptional control-flow edges;
- constant folding, unreachable-block elimination, branch simplification, SSA,
  or phi nodes;
- AArch64 branch lowering;
- a general formatting or standard-library output API.

Those features may build on this slice, but none should enter incidentally.
In particular, short-circuit logical operators require expression-level CFG
lowering and should receive their own contract instead of being represented as
ordinary eager binary operations.

### Language contract for this slice

The conditional grammar is:

```text
if-statement = "if" "(" expression ")" block
               ("elif" "(" expression ")" block)*
               ["else" block]
```

Each condition must have type `bool`. Conditions are evaluated from left to
right until one evaluates to `true`; only that arm executes, and no later
condition is evaluated. If every condition is `false`, the `else` arm executes
when present. With no `else`, execution continues after the statement.

Every condition is resolved in the scope containing the whole statement.
Every arm block creates its own child scope. A declaration in one arm is not
visible in another arm, a later `elif` condition, or after the statement.

An `if` statement definitely returns only when it has an `else` arm and every
`if`, `elif`, and `else` block definitely returns. This rule composes through
nested blocks and conditionals. It is used to enforce that every reachable path
through an `i64` or `bool` function returns a value. A `unit` function retains
implicit fallthrough return behavior.

### Boolean representation and ABI contract

`bool` is a semantic type in HIR and MIR, not an alias for `i64`. `true` and
`false` are the only Skald boolean values. MIR constants and backend-produced
boolean values remain canonical zero or one, but physical storage width stays
a target decision; the initial stack-heavy backend may continue assigning an
eight-byte home to every scalar value.

For the restricted Linux x86-64 System V external-function profile, Skald
`bool` corresponds to C `bool` (`_Bool`). Boolean arguments use the ABI integer
class. A boolean result received from an external function is normalized from
the ABI result byte before becoming a Skald value; no unspecified upper return
register bits may enter MIR-visible behavior. This extends only the existing
restricted exact-symbol profile and does not settle the general FFI.

The runtime operation is:

```c
void ska_rt_println_bool(bool value);
```

It writes exactly `true\n` or `false\n`, using lowercase ASCII and no other
bytes. Like `ska_rt_println_i64`, it completes and flushes the record before
returning, and a detected write or flush failure terminates unsuccessfully.
C1 adds the public symbol in runtime ABI version 3.

### Architectural rules

1. `bool` remains distinct from `i64` in every target-independent phase.
2. `ska_rt_println_bool` is an ordinary exact-symbol external function. No
   compiler phase recognizes its spelling.
3. Source syntax is not enabled until it has a complete path through the
   currently supported x86-64 target.
4. The AST and HIR preserve a flat ordered arm list and optional `else` block;
   they do not discard `elif` source structure by pretending it was written as
   nested `else if` statements.
5. MIR represents control flow with block terminators, never with special
   calls or high-level conditional instructions.
6. Temporary MIR values remain block-local in this non-SSA representation.
   State shared across branches uses storage. The verifier enforces this
   simple rule until an explicit SSA design replaces it.
7. MIR lowering emits blocks in deterministic ID order. The backend may not
   choose semantics from AST or HIR structure.
8. Target block labels are deterministic, function-local, and cannot collide
   with source-visible external symbols.
9. Return-completeness is a semantic control-flow property, not a parser or
   backend heuristic.
10. No optimization is required for correctness. Constant conditions still
    lower to valid explicit control flow in this slice.

## 2. Progress Summary

- [x] C0 — Specify boolean and conditional behavior
- [x] C1 — Add and directly test the runtime boolean output ABI
- [ ] C2 — Implement straight-line `bool` values end-to-end
- [ ] C3 — Add multi-block MIR and control-flow verification
- [ ] C4 — Lower multi-block control flow on x86-64 System V
- [ ] C5 — Implement `if` / `elif` / `else` end-to-end
- [ ] C6 — Add comprehensive golden coverage and harden the slice

Milestone checkboxes below should be marked as implementation progresses. A
task is complete only when its acceptance criteria and relevant quality gates
pass.

## 3. PR-Sized Implementation Tasks

### C0 — Specify boolean and conditional behavior

**Purpose:** Make syntax, evaluation, typing, scoping, return analysis, runtime
output, and ABI behavior explicit before encoding them across compiler phases.

- [x] Update `docs/SKALD_DRAFT_SPEC.md` to specify `if` / `elif` / `else`,
      including required parentheses and blocks.
- [x] Replace the draft's `if` / `else` statement entry with
      `if` / `elif` / `else` and state that `elif` is the only chained-arm
      spelling.
- [x] Specify exact-`bool` conditions, source-order evaluation, skipped later
      conditions, and selected-arm-only execution.
- [x] Specify branch scopes and branch-aware definite-return behavior.
- [x] Extend the implemented grammar contract with boolean types, literals,
      and conditional statements.
- [x] Specify the restricted external `bool` ABI mapping and result
      normalization boundary.
- [x] Specify exact `ska_rt_println_bool` bytes and failure behavior.
- [x] Record all deliberately excluded boolean operators, conversions,
      conditional expressions, optimizations, and control-flow forms.

**Tests:** Manual consistency review among the draft specification, grammar
contract, runtime ABI types, and this roadmap. No implementation behavior
changes.

**Acceptance criteria:** Every later task has one unambiguous contract for
syntax, typing, evaluation order, scopes, control flow, external ABI behavior,
and observable output, without silently expanding the slice.

### C1 — Add and directly test the runtime boolean output ABI

**Purpose:** Establish boolean output independently of compiler parsing,
typing, or code generation.

- [x] Add `ska_rt_println_bool(bool value)` to the public C runtime header.
- [x] Include the standard C boolean type without exposing implementation-only
      libc types through the public ABI.
- [x] Emit exactly `true\n` or `false\n` and reuse a small common checked-write
      boundary where that improves clarity without over-generalizing I/O.
- [x] Preserve the existing unrecoverable detected-write-failure policy.
- [x] Increment `SKALD_RUNTIME_ABI_VERSION`.
- [x] Extend the direct C runtime harness with exact output for `false`, `true`,
      and consecutive mixed calls.
- [x] Verify header/archive ABI-version agreement and unsuccessful termination
      after a forced boolean-output failure.
- [x] Update runtime ABI and runtime-test documentation.

**Tests:** `make runtime-test` under C11, `-Wall -Wextra -Werror`, including
exact captured bytes, consecutive calls, ABI version agreement, and the
failure-path child process.

**Acceptance criteria:** A direct C consumer can call the public function and
observe the specified bytes; failure cannot return as success; no compiler
support is needed to test the ABI.

### C2 — Implement straight-line `bool` values end-to-end

**Purpose:** Add one real scalar type through the existing pipeline before
introducing control flow, forcing type and ABI assumptions to become explicit
without mixing them with CFG work.

- [ ] Lex `bool`, `true`, and `false` as distinct keywords.
- [ ] Parse `bool` in supported parameter, result, local, and external
      declaration type positions, plus boolean literal expressions.
- [ ] Preserve boolean types and literals in deterministic AST and resolved
      dumps.
- [ ] Add `bool` to resolved types and typed HIR, with distinct literal nodes
      and exact type checking for locals, calls, and returns.
- [ ] Keep `main` exactly `fn main() -> i64`.
- [ ] Extend the restricted external profile to by-value `bool` parameters and
      `bool` results as specified by C0.
- [ ] Add `bool`, canonical boolean constants, loads, stores, call arguments,
      and returns to MIR without encoding them as `i64` operations.
- [ ] Extend MIR verification for boolean storage, values, signatures, calls,
      and returns.
- [ ] Extend x86-64 ABI and frame lowering for internal and external boolean
      parameters/results, including normalization of external results.
- [ ] Add the source-to-runtime declaration and call path for
      `ska_rt_println_bool` without a name-based intrinsic.
- [ ] Replace the existing `bool` unsupported-type golden with focused valid
      and invalid boolean cases.

**Tests:** Lexer/parser and recovery tests; resolution, HIR, MIR, verifier, and
dump tests; backend ABI and assembly-shape tests; exact compile-failure cases
for bool/i64 mismatches and invalid `main`; and a native golden printing
literal, local, parameter, and function-returned booleans.

**Acceptance criteria:** Straight-line programs can declare, pass, return, and
print true boolean values through the complete pipeline. `bool` never becomes
interchangeable with `i64`, external results are canonicalized, existing
programs retain deterministic assembly, and the runtime symbol is ordinary
external linkage.

### C3 — Add multi-block MIR and control-flow verification

**Purpose:** Establish a small, explicit target-independent CFG before source
conditionals depend on it.

- [ ] Add unconditional `Goto` and boolean `Branch` MIR terminators with stable
      target `BlockId`s and source spans.
- [ ] Keep `Return` as a terminator and require exactly one terminator on every
      emitted block.
- [ ] Add deterministic MIR construction helpers for allocating blocks,
      selecting the current block, and terminating it exactly once.
- [ ] Extend MIR dumps with stable block targets and branch conditions.
- [ ] Verify entry-block validity, dense block IDs, target ownership and
      existence, terminator presence, and boolean branch conditions.
- [ ] Enforce that transient value uses are defined earlier in the same block;
      storage remains the explicit mechanism for values crossing block edges.
- [ ] Validate every represented block even when it is unreachable; do not make
      dead-block removal a prerequisite for valid MIR.
- [ ] Expose deterministic successor information suitable for later analyses
      without introducing a general graph framework prematurely.
- [ ] Keep target legality rejection for multi-block MIR until C4.

**Tests:** Hand-built MIR fixtures for jumps, diamonds, joins, and multiple
returns; exact dump tests; verifier mutation tests for missing or foreign
targets, wrong condition type, use across a block boundary, and unterminated
blocks; construction-helper tests for attempted duplicate termination; and
pass-pipeline preservation tests.

**Acceptance criteria:** MIR can faithfully represent the CFG needed by
conditionals, invalid graphs fail at the verifier boundary, dumps are
deterministic, no source syntax changes, and the backend still rejects shapes
it cannot yet lower.

### C4 — Lower multi-block control flow on x86-64 System V

**Purpose:** Make verified conditional MIR executable before enabling source
syntax that produces it.

- [ ] Extend the target assembly model with deterministic local labels,
      unconditional jumps, and conditional branches.
- [ ] Give every MIR block a collision-proof function-local assembly label.
- [ ] Emit blocks in stable `BlockId` order rather than traversal-dependent
      order.
- [ ] Lower a canonical boolean branch without leaking target comparison or
      register details into MIR.
- [ ] Preserve existing call alignment, frame planning, and return behavior in
      every block.
- [ ] Remove the initial single-block target-legality restriction only after
      every new terminator is supported.
- [ ] Keep malformed or unsupported MIR as a structured backend error rather
      than a panic.
- [ ] Ensure emitted multi-block assembly is accepted by the system assembler.

**Tests:** Target-legality tests; exact assembly-shape tests for forward and
backward jumps, a diamond with a join, branch-local calls, and returns in both
arms; system-assembler acceptance; and a native executable produced from a
hand-built verified MIR conditional.

**Acceptance criteria:** The backend executes verified multi-block MIR with the
specified branch semantics, labels and output remain deterministic, no
frontend syntax is inspected, and all old single-block programs are unchanged
apart from deliberately documented assembly formatting improvements.

### C5 — Implement `if` / `elif` / `else` end-to-end

**Purpose:** Enable structured source conditionals only after their semantic IR
and target execution path are ready.

- [ ] Lex `if`, `elif`, and `else` as distinct keywords.
- [ ] Parse one `if` arm, an ordered vector of zero or more `elif` arms, and an
      optional `else` block with complete spans.
- [ ] Recover cleanly from missing parentheses, conditions, blocks, and
      malformed or misplaced `elif`/`else` tokens.
- [ ] Preserve the flat source arm structure in deterministic AST dumps.
- [ ] Resolve each condition in the containing scope and every arm body in an
      independent child scope.
- [ ] Add an explicit typed HIR conditional statement with ordered boolean
      conditions and blocks.
- [ ] Diagnose non-boolean conditions without implicit truthiness.
- [ ] Generalize definite-return analysis so a conditional returns only when
      it has `else` and all arms definitely return.
- [ ] Lower condition evaluation, true arms, false continuation, and the
      optional join into explicit deterministic MIR blocks.
- [ ] Avoid creating fallthrough joins that are unreachable because every arm
      terminates.
- [ ] Preserve source-order condition side effects and skip all conditions and
      bodies after the first selected arm.
- [ ] Compile at least one source conditional through assembly, linking, and
      native execution in this task.

**Tests:** Parser shape/recovery and dump tests; resolver scope tests; HIR type
and definite-return tests; MIR exact-order and CFG dump tests; verifier-backed
lowering tests; backend integration tests; and focused successful and
compile-failure native goldens.

Required failures include an `i64` condition, missing required delimiters or
blocks, standalone `elif`/`else`, rejected `else if`, branch-local use outside
its scope, and a value-returning function with a non-exhaustive conditional.

**Acceptance criteria:** `if` / `elif` / `else` compiles through the public
pipeline with exact boolean conditions, lexical arm scopes, source-ordered
short-circuit selection, correct return analysis, verified MIR, and native
x86-64 execution.

### C6 — Add comprehensive golden coverage and harden the slice

**Purpose:** Prove externally observable behavior and reconcile all public
documentation with the completed implementation.

- [ ] Add exact stdout goldens for `true`, `false`, locals, parameters,
      function returns, and consecutive boolean output calls.
- [ ] Cover a true first arm, false fallthrough to `elif`, selection among
      multiple `elif` arms, final `else`, and no-arm-selected behavior without
      `else`.
- [ ] Use condition functions with observable output to prove left-to-right
      evaluation and that later conditions are skipped after a match.
- [ ] Cover nested conditionals and both exhaustive and non-exhaustive return
      analysis.
- [ ] Keep stdout and process exit status independently asserted.
- [ ] Add exact compile-failure goldens for every new lexer, parser, resolution,
      and type-check diagnostic category not already covered.
- [ ] Confirm repeated compiler runs produce identical assembly and diagnostics
      for boolean and multi-block programs.
- [ ] Update `README.md`, `grammar/README.md`, `docs/REPO_STRUCTURE.md`,
      `docs/DEBUGGING.md`, `docs/NEXT_SLICE_BOUNDARIES.md`, runtime and golden
      test documentation, and the draft specification where status changed.
- [ ] Record remaining boolean-operation, conversion, optimization, FFI, and
      control-flow questions explicitly.
- [ ] Run the complete repository quality gates from a clean build state.

**Tests:** All compiler and CLI tests, runtime tests, successful and
compile-failure goldens, formatting, Clippy with warnings denied, and
`git diff --check`.

**Acceptance criteria:** The complete source-to-runtime behavior is covered by
stable exact observations; evaluation order and branch selection are proven,
not inferred; every new failure family has a stable diagnostic; and all public
documents describe the implemented boundary consistently.

## 4. Required Quality Gates for Every Task

Each implementation PR must run the relevant focused tests and, before being
marked complete, the full applicable repository checks:

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test` when the runtime or ABI is touched
- [ ] `make golden-test` when source behavior, diagnostics, MIR, backend,
      runtime linking, or golden expectations are touched
- [ ] `git diff --check`

These global checkboxes describe the final clean-build gate for C6. Individual
tasks should record their own gate results in their implementation change; they
must not mark the whole roadmap complete early.

A task must not leave accepted source to fail in a later phase merely because
that phase has not been implemented. C3 and C4 therefore add CFG infrastructure
behind the existing source language, and C5 enables conditional syntax only
after verified backend support exists.

## 5. Completion Definition

This roadmap is complete when all C0–C6 and quality-gate checkboxes are marked,
and the following path is covered end-to-end:

```text
bool types and true/false literals
  → resolved and exactly typed boolean values
  → canonical MIR boolean values
  → ordinary external runtime boolean output

if / elif / else source arms
  → scoped, typed HIR conditional
  → explicit verified MIR blocks and terminators
  → deterministic x86-64 labels and branches
  → source-ordered native behavior with exact stdout
```

No target-independent phase may encode `bool` as `i64`, no compiler phase may
recognize the output function by spelling, no conditional may accept implicit
truthiness, and no backend may reconstruct source-level conditional semantics.

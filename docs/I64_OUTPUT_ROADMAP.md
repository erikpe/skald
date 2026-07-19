# `i64` Output and Golden-Test Observability Roadmap

Status: O0–O5 complete; O6 planned.

This roadmap adds exact stdout observation to native golden tests and the
smallest clean language/runtime path for printing an `i64`. It is split into
reviewable, PR-sized slices. Every slice must preserve a buildable compiler,
keep all previously supported programs working, and finish with tests and
updated documentation.

The completed slice should compile and run a program such as:

```ska
extern fn ska_rt_println_i64(value: i64) -> unit;

fn main() -> i64 {
    ska_rt_println_i64(42);
    ska_rt_println_i64(-7);
    return 0;
}
```

with exact stdout:

```text
42
-7
```

The low-level external function is a bootstrap/runtime facility. A future
standard library may wrap it with a user-facing name and richer I/O behavior;
this roadmap does not establish `ska_rt_println_i64` as the final standard
library API.

## 1. Scope and Design Constraints

### Included

- exact stdout expectations for native golden tests;
- the `unit` type in the implemented scalar subset;
- unit-returning local and external functions;
- `return;` in unit-returning functions;
- call expression statements;
- restricted external function declarations;
- external calls using the target C ABI;
- `ska_rt_println_i64(i64) -> unit` in the C runtime;
- x86-64 System V lowering for the new call and return forms;
- deterministic phase dumps and diagnostics for all new representations.

### Explicitly excluded

- strings, formatting syntax, or general text output;
- stdin or file I/O;
- imports, standard-library discovery, or automatic preludes;
- a stable general-purpose foreign-function interface;
- external objects, arrays, shared handles, alias parameters, or callbacks;
- variadic functions or user-selected calling conventions;
- dynamic linking policy and separate compilation;
- AArch64 lowering;
- recovery from stdout write failures in Skald code.

The supported `extern fn` subset is an implementation stepping stone. It must
be documented precisely, but it must not silently settle the complete foreign
interface and ownership model identified as open in the draft specification.

### Architectural rules

1. `ska_rt_println_i64` is not recognized by spelling in the parser, type
   checker, MIR lowering, or backend. It is an ordinary external declaration
   whose linker symbol follows the restricted external-linkage contract.
2. Resolution remains the only phase that selects a source name. Calls below
   resolution use stable callable identities.
3. Function signatures and function bodies are distinct concepts. External
   declarations have signatures and symbols but no Skald body.
4. `unit` has no runtime payload. MIR and the backend must not invent a
   meaningful integer result for a unit call.
5. Calls have an optional result in executable IR. A value-returning call must
   define a value; a unit-returning call must not.
6. The entry point remains exactly `fn main() -> i64`. This roadmap does not
   change process exit-status behavior.
7. Runtime output is compared byte-for-byte. Tests must not normalize line
   endings, trim whitespace, or decode and re-encode output.
8. Existing run goldens without a stdout expectation continue to require empty
   stdout.

## 2. Progress Summary

- [x] O0 — Specify the implemented output and external-call contract
- [x] O1 — Add exact stdout expectations to the golden runner
- [x] O2 — Separate callable declarations from local function bodies
- [x] O3 — Implement `unit`, unit returns, and call statements end-to-end
- [x] O4 — Add and directly test the runtime `i64` output ABI
- [x] O5 — Implement restricted external declarations and calls end-to-end
- [ ] O6 — Add observable golden coverage and harden the completed slice

Milestone checkboxes below should be marked as each PR is completed. A
milestone is not complete until its acceptance criteria and relevant repository
quality gates pass.

## 3. PR-Sized Implementation Slices

### O0 — Specify the implemented output and external-call contract

**Purpose:** Settle the deliberately narrow behavior required by this slice
before encoding it independently in several compiler phases.

- [x] Add the restricted `extern fn` grammar to `grammar/README.md`.
- [x] Specify `unit` returns, `return;`, and call expression statements for the
      implemented subset.
- [x] Specify that non-unit functions require `return expression;` and unit
      functions use `return;` or may reach the end of their body.
- [x] Restrict expression statements initially to call expressions of type
      `unit`, preventing accidental discarded values without adding general
      expression-statement semantics prematurely.
- [x] Specify the initial external ABI surface: `i64` parameters and `i64` or
      `unit` results, by value, using the target C ABI.
- [x] Specify external symbol naming, duplicate-name behavior, and that an
      external declaration cannot provide the program entry point.
- [x] Specify `ska_rt_println_i64`: signed decimal spelling, one trailing LF,
      no leading padding, and behavior for the entire `i64` range.
- [x] Define a detected stdout write or flush failure as an unrecoverable
      runtime error with unsuccessful process termination rather than silent
      success; its exact status and diagnostic remain unspecified.
- [x] Update `docs/SKALD_DRAFT_SPEC.md` to distinguish this implemented,
      restricted subset from the still-open complete FFI and library design.

**Tests:** Documentation examples are checked manually against the grammar and
ABI types. No implementation behavior changes in this slice.

**Acceptance criteria:** The syntax, type rules, symbol mapping, output bytes,
and failure policy needed by later slices are unambiguous, while broader FFI
and standard-library questions remain explicitly open.

### O1 — Add exact stdout expectations to the golden runner

**Purpose:** Make stdout a first-class golden expectation independently of how
Skald programs eventually produce it.

- [x] Teach `tests/golden/runner.rs` to load an optional same-named `.stdout`
      sidecar for each `run/**/*.ska` case.
- [x] Treat a missing `.stdout` sidecar as an exact expectation of zero stdout
      bytes, preserving all existing tests.
- [x] Compare expected and actual stdout as bytes without trimming or newline
      conversion.
- [x] Report useful escaped or otherwise unambiguous expected/actual output on
      mismatch, including trailing-newline differences.
- [x] Continue requiring empty runtime stderr and the expected `.exit` status.
- [x] Separate sidecar loading and execution-result comparison into focused
      helpers instead of growing `run_native_case` further.
- [x] Add focused runner tests or fixtures for empty output, exact output,
      missing/extra trailing LF, and non-UTF-8 mismatch reporting.
- [x] Update `tests/golden/README.md` with the `.stdout` convention.

**Tests:** Golden-runner helper tests plus the complete existing golden suite.
All current native cases must pass without adding `.stdout` files.

**Acceptance criteria:** The runner can express exact stdout expectations, its
comparison behavior is tested, and existing exit-only cases retain identical
semantics.

### O2 — Separate callable declarations from local function bodies

**Purpose:** Prepare the IR for bodyless external declarations without adding
new source syntax or changing existing language behavior.

- [x] Introduce an explicit callable/function declaration model containing a
      stable identity, source name for diagnostics, signature, and linkage.
- [x] Represent a Skald function definition as a declaration plus a body rather
      than assuming every callable identity indexes a body.
- [x] Preserve dense, deterministic identities and source declaration order.
- [x] Update resolved IR, HIR, MIR, dumps, and verifier APIs so call signature
      lookup does not require a local body.
- [x] Give executable call IR an explicit target abstraction suitable for both
      local and external calls; do not encode the distinction in symbol-name
      strings scattered through the backend.
- [x] Generalize call results to be optional at the MIR representation boundary
      while retaining the invariant that every currently emitted `i64` call
      has a result.
- [x] Keep local symbol generation owned by the backend, deterministic, and in
      a target-private namespace that cannot collide with exact external
      identifiers.
- [x] Avoid compatibility fields or parallel legacy/new call paths once the
      migration is complete.

**Tests:** Update resolution, HIR, MIR lowering, MIR verification, dump, and
backend tests. Add verifier tests for an unknown callable, signature mismatch,
and invalid result presence. Run all existing native goldens to prove that the
refactor has no observable effect.

**Acceptance criteria:** No phase below resolution chooses a call by source
name; signatures can exist without bodies; calls can represent an absent
result; all first-slice programs retain deterministic assembly and identical
runtime behavior. Internal assembly symbols intentionally change to the
collision-proof target-private spelling fixed by O0.

### O3 — Implement `unit`, unit returns, and call statements end-to-end

**Purpose:** Add the language semantics required to invoke output naturally,
without fake integer return values or ignored temporary storage.

- [x] Lex and parse `unit` types, `return;`, and the restricted call statement.
- [x] Add corresponding AST nodes with complete spans and deterministic dumps.
- [x] Resolve call statements through the same name-resolution path as call
      expressions.
- [x] Add `unit` to resolved types, typed HIR, and MIR.
- [x] Enforce the documented return rules and diagnose value/unit mismatches.
- [x] Represent unit call statements as effectful MIR instructions with no
      result `ValueId`.
- [x] Represent unit function returns without a return operand.
- [x] Verify call result presence, return operand presence, and types against
      their signatures.
- [x] Lower local unit calls and unit returns for x86-64 System V without
      reading or storing a fictitious `%rax` result.
- [x] Preserve `fn main() -> i64` as the only valid entry signature.

**Tests:** Lexer/parser recovery tests; resolver and type-checker diagnostics;
HIR/MIR dump tests; verifier mutation tests; backend assembly-shape tests; and
a native golden with a local unit-returning helper called for effect before
`main` returns an `i64`.

Required compile-failure coverage includes returning a value from a unit
function, omitting a value from an `i64` return, using a unit call where `i64`
is required, and discarding an `i64` call if the restricted statement rule is
adopted.

**Acceptance criteria:** Local unit functions compile and execute through the
full pipeline; unit has no payload in MIR or machine code; invalid unit/value
mixing is diagnosed before MIR; existing `i64` behavior is unchanged.

### O4 — Add and directly test the runtime `i64` output ABI

**Purpose:** Establish the runtime service independently of compiler code
generation.

- [x] Add `ska_rt_println_i64(int64_t value)` to the public runtime header.
- [x] Implement locale-independent signed decimal output followed by exactly
      one LF byte.
- [x] Implement the O0 write-failure policy without exposing C implementation
      details as Skald language semantics.
- [x] Bump `SKALD_RUNTIME_ABI_VERSION` because the public ABI contract changes.
- [x] Keep formatting/output logic cohesive and avoid exposing `FILE *` or
      other libc types in the public ABI.
- [x] Extend direct C runtime tests to capture and compare output for zero,
      positive, negative, `INT64_MIN`, `INT64_MAX`, and consecutive calls.
- [x] Keep runtime tests silent on success and warnings clean under the
      repository's C11 `-Wall -Wextra -Werror` policy.
- [x] Document the new runtime symbol and ABI behavior in
      `docs/REPO_STRUCTURE.md` and `tests/runtime/README.md`.

**Tests:** `make runtime-test`, including exact byte comparisons and ABI-version
agreement between the header and linked archive.

**Acceptance criteria:** A C consumer can call the public runtime symbol and
observe the specified bytes for the entire representative range; runtime tests
pass without compiler involvement.

### O5 — Implement restricted external declarations and calls end-to-end

**Purpose:** Connect ordinary Skald call semantics to the runtime ABI without a
name-based intrinsic or backend special case.

- [x] Lex and parse `extern fn name(parameters) -> type;` with recovery and
      deterministic AST output.
- [x] Collect external declarations in the same callable namespace as local
      definitions and diagnose all duplicate combinations consistently.
- [x] Preserve the source external name as explicit linkage metadata selected
      during resolution.
- [x] Enforce the O0 restricted external signature and entry-point rules during
      semantic analysis.
- [x] Carry external signatures and linkage through HIR and MIR declarations;
      do not synthesize empty function bodies.
- [x] Extend MIR verification to validate external call arity, argument types,
      result presence, and symbol metadata.
- [x] Emit external calls through the existing target call-lowering path and
      x86-64 System V ABI argument rules.
- [x] Ensure only local definitions are emitted as assembly function bodies.
- [x] Ensure unresolved external symbols fail in the linker/toolchain layer
      with a driver error and never cause a compiler panic.
- [x] Add deterministic dumps that visibly distinguish defined and external
      callable declarations.

**Tests:** Phase-level valid and recovery cases, duplicate-name diagnostics,
restricted-signature diagnostics, external-`main` rejection, MIR verifier
tests, assembly-shape tests for an external symbol call, and toolchain failure
coverage for an unavailable symbol.

**Acceptance criteria:** A source-declared external function is type checked
from its declaration, lowered through stable identities, and emitted as a C-ABI
call to its declared linker symbol. The implementation contains no spelling
check for `ska_rt_println_i64` outside tests and example source.

### O6 — Add observable golden coverage and harden the completed slice

**Purpose:** Prove the complete source-to-stdout path and reconcile all public
documentation with the implementation.

- [ ] Add a native golden that declares and calls
      `ska_rt_println_i64(i64) -> unit` and uses a `.stdout` sidecar.
- [ ] Cover zero, positive, negative, computed, and function-returned values.
- [ ] Cover consecutive calls to prove source-order side effects.
- [ ] Cover `i64::MIN` and `i64::MAX` formatting.
- [ ] Keep the golden program's process exit status independently observable
      through its `.exit` sidecar.
- [ ] Add exact compile-failure goldens for the new syntax and semantic error
      categories not already covered by phase tests.
- [ ] Confirm repeated compiler runs still produce identical assembly and
      diagnostics for programs containing external calls.
- [ ] Update `README.md`, `grammar/README.md`, `docs/REPO_STRUCTURE.md`,
      `docs/DEBUGGING.md`, `docs/NEXT_SLICE_BOUNDARIES.md`, and the draft
      specification where their implemented-status descriptions changed.
- [ ] Record any deliberately deferred FFI, unit, or I/O questions rather than
      leaving behavior implicit.
- [ ] Run the complete repository quality gates from a clean build state.

**Tests:** All compiler tests, runtime tests, successful and compile-failure
goldens, formatting, Clippy with warnings denied, and `git diff --check`.

**Acceptance criteria:** A Skald program can print exact signed `i64` lines to
stdout through a declared runtime function; native goldens assert stdout and
exit status independently; every new failure category has a stable diagnostic;
all documentation describes the implemented boundary consistently.

## 4. Required Quality Gates for Every Slice

Each implementation PR must run the relevant focused tests and, before being
marked complete, the full applicable repository checks:

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --all-targets`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
- [ ] `make runtime-test` when the runtime or ABI is touched
- [ ] `make golden-test` when source, semantics, MIR, backend, runtime linking,
      or golden expectations are touched
- [ ] `git diff --check`

A slice must not be marked complete if it leaves accepted source to fail in a
later phase merely because that phase has not been implemented. New syntax is
enabled only when its complete path through the currently supported target is
ready, or is guarded by a clear unsupported-feature diagnostic.

## 5. Completion Definition

This roadmap is complete when all O0–O6 checkboxes are marked, all quality
gates pass, and this behavior is covered end-to-end:

```text
.ska external declaration and calls
  → resolved callable identities and signatures
  → typed unit call statements
  → verified effectful MIR calls
  → x86-64 System V external calls
  → versioned C runtime output
  → exact `.stdout` golden comparison
```

No compiler phase may recognize the output function by name, no unit call may
manufacture a meaningful integer result, and no stdout assertion may depend on
process exit-status encoding.

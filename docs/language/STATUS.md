# Skald Language and Compiler Status

Status: authoritative feature-maturity and compiler-support matrix.

This document answers whether a language area is implemented, settled for
implementation, exploratory, or unresolved. Detailed semantic documents own
the rules themselves; the [language overview](README.md) defines the broad
model.

## Maturity labels

- **Implemented contract** — accepted or emitted by the current compiler and
  protected by implementation and tests.
- **Frozen design** — source-visible behavior deliberately settled for an
  active implementation plan but not yet fully implemented.
- **Exploratory direction** — useful non-normative constraints or examples;
  details may change before implementation.
- **Open question** — no implementation-ready source and semantic contract
  exists yet.
- **Implementation detail** — compiler, target, runtime, driver, or test
  behavior rather than a language feature.
- **History** — prior implementation sequence preserved only in archived
  roadmaps or Git.

An implemented row describes only the stated boundary. A broader feature name
does not imply support beyond that boundary. No unimplemented language feature
is currently classified as **frozen design**: the active polymorphism work must
first settle its executable profile, and the remaining directions do not yet
have focused implementation plans.

## Implemented language

| Area | Maturity | Current compiler boundary |
|---|---|---|
| Source and declarations | **Implemented contract** | One UTF-8 source file; ASCII identifiers; line comments; top-level functions, exact-symbol external functions, and nominal classes. |
| [Primitive types and literals](TYPES_AND_VALUES.md#literal-types-and-ranges) | **Implemented contract** | `i64`, `u64`, `u8`, `f64`, and `bool`; payload-free `unit` results; spelling-selected numeric types with checked literal ranges. |
| [Primitive expressions](TYPES_AND_VALUES.md#expressions) | **Implemented contract** | Exact-type `+`, `-`, and `*`; unary `-` for `i64` and `f64`; grouping, direct calls, and field selection; no implicit conversions or truthiness. |
| [Bindings and scopes](FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals) | **Implemented contract** | Typed `var` locals, value parameters, lexical blocks, declaration-before-use for locals, nested shadowing, and duplicate rejection within one scope. Primitive local reassignment is not implemented. |
| [Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#statements-and-blocks) | **Implemented contract** | Forward calls, recursion, direct calls, `unit` call statements, `return`, and mandatory-block `if`/`elif`/`else` with exact `bool` conditions and definite-return checking. |
| Entry point and primitive interoperation | **Implemented contract** | A defined `fn main() -> i64`; trusted external declarations using their source name as the linker symbol and by-value primitive parameters/results, with `unit` also allowed as a result. |
| Exact nominal classes | **Implemented contract** | Inline class values; primitive and exact-class fields; exactly one ordinary initializer; statically selected read-only and mutable receiver methods; no inheritance or dynamic dispatch. |
| Inline containment and initialization | **Implemented contract** | Acyclic exact-class subobjects, direct field construction, straight-line definite field initialization, nested field places, and access-preserving projected receivers. |
| Call-scoped aliases | **Implemented contract** | `ref` and `mut ref` parameters over existing exact-class places, including forwarding and deliberate overlap. Aliases are non-owning, exact-type, and restricted by their access mode. Local aliases and external alias signatures are unsupported. |
| Deterministic object lifetime | **Implemented contract** | Optional user `destroy` bodies, normal block/conditional/return cleanup, reverse local and field order, and exactly-once cleanup for owning inline values. Failed construction and exceptional cleanup are outside the implemented control-flow model. |
| Exact-class copying and assignment | **Implemented contract** | User or recursively synthesized copy construction and copy assignment for live local, parameter, and supported projected destinations, including self-assignment behavior selected before lowering. |
| Class value parameters and results | **Implemented contract** | Internal caller-created parameter copies, caller-owned result storage, function and method object results, produced-object sources, bounded full-expression temporaries, and the two supported constructor-elision cases. Object-bearing external signatures are unsupported. |
| [Evaluation order](FUNCTIONS_AND_CONTROL_FLOW.md#evaluation-order) | **Implemented contract** | Operands and arguments evaluate left to right; receivers precede explicit arguments; construction, object production, and normal cleanup preserve the specified order. |
| Runtime scalar observation | **Implemented contract** | Repository runtime support for line-oriented `i64`, `u64`, `u8`, and `bool` output plus raw-bit `f64` observation, reached through ordinary restricted external declarations rather than language built-ins. |

The [implemented grammar](GRAMMAR.md) is the precise accepted syntax authority.
Focused semantic documents will replace the remaining explanations in the
draft specification without changing the matrix above unless compiler behavior
changes.

## Compiler availability

| Surface | Status | Current boundary |
|---|---|---|
| Compiler input | **Implemented contract** | One canonical `.ska` source path per invocation. |
| Target registry | **Implemented contract** | `x86_64-sysv` is the only accepted target name and the default. |
| Host/toolchain execution | **Implemented contract** | Linux x86-64 System V assembly and native executable linking through the configured C compiler driver and versioned C runtime. |
| Artifacts | **Implemented contract** | Textual assembly with `--emit asm`, or a linked executable by default. |
| Linux AArch64 backend | **Exploratory direction** | Named as a future backend direction; no target entry or active backend implementation roadmap exists. |

Target ABI and runtime mechanics are implementation details. Their later
focused documents will own those contracts without turning them into portable
language guarantees.

## Not implemented

| Area | Maturity | Current direction or unresolved boundary |
|---|---|---|
| Inheritance and polymorphism | **Exploratory direction** | Single inheritance, base subobjects, opt-in virtual dispatch, interfaces, `Obj` views, type tests, and checked narrowing are planned together. The [polymorphism roadmap](../roadmaps/POLYMORPHISM_ROADMAP.md) still begins by freezing syntax, semantics, ownership, metadata, and failure behavior. |
| Shared ownership and heap allocation | **Exploratory direction** | Non-null shared owning handles, reference counting, dynamic complete-object destruction, and borrow anchors are intended, but source and runtime contracts are not frozen. |
| Optionals, arrays, and strings | **Exploratory direction** | Likely type forms and broad constraints exist; construction, access, lifetime, conversion, and library contracts remain open. |
| Recoverable and checked exceptions | **Exploratory direction** | Deterministic cleanup is a constraint, but throw/catch syntax, exception sets, failed-construction behavior, and lowering remain open. |
| Multiple files and modules | **Open question** | Imports, exports, visibility, source-to-module mapping, separate compilation, packages, and cross-module linkage are unspecified. |
| [Loops and iteration](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability) | **Open question** | `while`, `for`, `break`, `continue`, iterator protocols, and their cleanup boundaries are unspecified. |
| [Remaining primitive operations](TYPES_AND_VALUES.md#operators) | **Open question** | Comparisons, division, remainder, bitwise operations, shifts, explicit casts, signed-overflow behavior, and broader floating operations are not implemented as a settled group. |
| [Function values](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability), closures, and generics | **Open question** | Direct named calls are implemented; callable values, capture, generic declarations, inference, and specialization are not specified for Skald. |
| Static state and broader class features | **Open question** | Static members, access control, abstract/final forms, overloads, reflection, and user-defined conversions are not current language contracts. |
| Standard library | **Open question** | No Skald-written standard library is implemented; current scalar output is bootstrap runtime interoperation. |

These rows are deliberately brief. Future behavior becomes normative only in a
focused language document and reaches **frozen design** only after its active
roadmap resolves the implementation-dependent choices.

## Verification basis

The implemented rows were checked across the complete current pipeline rather
than copied from legacy prose:

- lexer and parser owners plus syntax, recovery, and exact AST tests establish
  the accepted source surface;
- resolution and type-checking owners plus focused diagnostics and resolved/HIR
  dumps establish names, types, access, lifecycle selection, and exclusions;
- HIR-to-MIR lowering, MIR verification, deterministic dumps, and backend tests
  establish evaluation, ownership, cleanup, target legality, and native
  realization;
- public API tests compose the intentional phase facades and the complete
  source-to-assembly entry point;
- the golden runner recursively discovers `tests/golden/run/**/*.ska` and
  `tests/golden/compile_fail/**/*.ska`, checks deterministic assembly or
  diagnostics, and executes successful programs;
- the runtime Makefile builds the contract, successful-output, and
  output-failure C harnesses documented in the
  [runtime test guide](../../tests/runtime/README.md);
- the backend registry and its focused test expose only `x86_64-sysv`.

The corresponding supported commands are `make compiler-test`,
`make golden-test`, `make runtime-test`, and the complete `make check` gate.
Case counts and exhaustive test inventories are intentionally not duplicated
here; repository discovery and focused test owners remain authoritative.

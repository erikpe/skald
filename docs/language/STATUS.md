# Skald Language and Compiler Status

Status: authoritative feature-maturity and compiler-support matrix.

This document answers whether a language area is implemented, settled for
implementation, exploratory, or unresolved. Detailed semantic documents own
the rules themselves; the [language overview](README.md) defines the broad
model.

## Maturity labels

- **Implemented contract** — accepted or emitted by the current compiler and
  protected by implementation and tests.
- **Frozen design** — source-visible behavior deliberately settled for
  implementation but not yet fully implemented.
- **Exploratory direction** — useful non-normative constraints or examples;
  details may change before implementation.
- **Open question** — no implementation-ready source and semantic contract
  exists yet.
- **Implementation detail** — compiler, target, runtime, driver, or test
  behavior rather than a language feature.
- **History** — prior implementation sequence preserved only in archived
  roadmaps or Git.

An implemented row describes only the stated boundary. A broader feature name
does not imply support beyond that boundary.

## Implemented language

| Area | Maturity | Current compiler boundary |
|---|---|---|
| [Source and declarations](MODULES_AND_INTEROP.md#current-compilation-unit) | **Implemented contract** | One UTF-8 `.ska` source file; ASCII identifiers; line comments; top-level functions, exact-symbol external functions, and nominal classes. |
| [Primitive types and literals](TYPES_AND_VALUES.md#literal-types-and-ranges) | **Implemented contract** | `i64`, `u64`, `u8`, `f64`, and `bool`; payload-free `unit` results; spelling-selected numeric types with checked literal ranges. |
| [Primitive expressions](TYPES_AND_VALUES.md#expressions) | **Implemented contract** | Exact-type `+`, `-`, and binary `*`; unary `-` for `i64` and `f64`; grouping, direct calls, field selection, and explicit shared dereference; no implicit conversions or truthiness. |
| [Bindings and scopes](FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals) | **Implemented contract** | Typed `var` locals, value parameters, lexical blocks, declaration-before-use for locals, nested shadowing, and duplicate rejection within one scope. Primitive local reassignment is not implemented. |
| [Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#statements-and-blocks) | **Implemented contract** | Forward calls, recursion, direct calls, `unit` call statements, `return`, and mandatory-block `if`/`elif`/`else` with exact `bool` conditions and definite-return checking. |
| [Entry point and primitive interoperation](MODULES_AND_INTEROP.md) | **Implemented contract** | A defined `fn main() -> i64`; trusted external declarations using their source name as the linker symbol and by-value primitive parameters/results, with `unit` also allowed as a result. |
| [Exact nominal classes](CLASSES_AND_LIFECYCLE.md#exact-nominal-classes) | **Implemented contract** | Inline class values; primitive and class fields; one or more ordinary initializer overloads with static most-specific selection for direct construction and `super(...)`; and read-only and mutable receiver methods. Opt-in virtual dispatch is tracked separately under dynamic polymorphism. |
| [Static inheritance semantics](POLYMORPHISM.md#hierarchies-and-declaration-namespaces) | **Implemented contract** | Canonical single-base identity; mandatory `super(...)`; complete lifecycle composition; inherited fields and direct methods through verified base projections; access-preserving class/`Obj` views; exact-copy owning slices; and x86-64 execution. |
| [Dynamic polymorphism](POLYMORPHISM.md) | **Implemented contract** | Opt-in virtual methods, exact inherited interface conformance, class/interface/`Obj` views and forwarding, virtual/interface calls, type tests, and checked object casts execute on x86-64. |
| [Checked object-place casts](OBJECT_CASTS.md) | **Implemented contract** | Unary `(T) source` casts preserve access and complete-object provenance for class/interface method receivers, alias arguments, field access and mutation, exact-class copy construction and assignment, value parameters, results, and owning slicing. Static views project directly; dynamic failure terminates; produced inline sources use owning temporaries, while replaceable or produced shared sources use verified anchors through checked-view cleanup. |
| [Inline containment and initialization](CLASSES_AND_LIFECYCLE.md#ordinary-initializer-contract) | **Implemented contract** | Acyclic exact-class subobjects, direct field construction, straight-line definite field initialization, nested field places, and access-preserving projected receivers. |
| [Call-scoped aliases](ALIASES_AND_OWNERSHIP.md) | **Implemented contract** | `ref` and `mut ref` parameters over inline and explicitly dereferenced shared object places, including ancestor/interface/`Obj` views, forwarding, and deliberate overlap. Dereferenced stable shared owners borrow directly; fields, nested places, and produced owners use verified hidden call anchors. Aliases remain non-owning and access-restricted; local aliases and external alias signatures are unsupported. |
| [Deterministic object lifetime](CLASSES_AND_LIFECYCLE.md#lifetime-registration-and-normal-cleanup) | **Implemented contract** | Optional user `destroy` bodies, normal block/conditional/return cleanup, reverse local and field order, and exactly-once cleanup for owning inline values. Failed construction and exceptional cleanup are outside the implemented control-flow model. |
| [Exact-class copying and assignment](CLASSES_AND_LIFECYCLE.md#copy-capabilities) | **Implemented contract** | User or recursively synthesized copy construction and copy assignment for live local, parameter, and supported projected destinations, including self-assignment behavior selected before lowering. User copy construction uses the distinct contextual `copy(ref source: T)` lifecycle declaration; `init` signatures are always ordinary. |
| [Explicit copy construction](CLASSES_AND_LIFECYCLE.md#copy-construction-and-object-sources) | **Implemented contract** | `T(copy source)` selects exact-`T` copy construction once from a target-directed checked source. Guaranteed views are static, dynamically possible forwarded views terminate on failed checks, impossible views are rejected, ancestor sources slice deliberately, and ordinary `T(arguments)` never falls back to copy construction. |
| [Shared ownership and heap allocation](SHARED_OWNERSHIP.md) | **Implemented contract** | Non-null strong `shared T` handles plus optional `shared? T` ownership, ordinary and exact-class copy allocation, named copy and produced transfer, secure assignment, internal parameters/results, owning fields, compatible polymorphic views, deterministic dynamic last-owner destruction, and verified hidden borrow anchors execute on x86-64. `owner!` secures a normal non-null owner before ordinary `*` or `->` access. Strong cycles deliberately leak; weak ownership, whole-pointee assignment, and dynamic cloning remain excluded. |
| [Shared object casts](OBJECT_CASTS.md) | **Implemented contract** | `(shared T) source` preserves one allocation, dynamically checks when required, copies named owners, and transfers produced owners. It never allocates, slices, or copies payload. Distinct allocation is explicit through `new T(copy source)`. |
| [Class value parameters and results](CLASSES_AND_LIFECYCLE.md#owning-value-parameters) | **Implemented contract** | Internal caller-created parameter copies, caller-owned result destinations, function and method object results, produced-object sources, bounded full-expression temporaries, and the two supported constructor-elision cases. Object-bearing external signatures are unsupported. |
| [Evaluation order](FUNCTIONS_AND_CONTROL_FLOW.md#evaluation-order) | **Implemented contract** | Operands and arguments evaluate left to right; receivers precede explicit arguments; construction, object production, and normal cleanup preserve the specified order. |
| [Optional values](OPTIONAL_VALUES.md) | **Implemented contract** | Primitive and exact-class `T?` plus class/interface/`Obj` `shared? T` locals, fields, internal parameters/results, methods, interfaces, overrides, initializer overloads, conditional lifecycle, checked class payload views, secured shared unwrap, inline optional-container aliases, one-word optional-owner ABI, casts after unwrap, and anchors execute through verified MIR. Optional truthiness, implicit unwrap, external optional signatures, optional references, and aliases to optional shared owners remain rejected. |
| [Arrays](ARRAYS.md) | **Implemented contract** | Invariant inline `T[]`, shared `shared T[]`, and optional-shared `shared? T[]` arrays execute through verified MIR on x86-64 with every documented owning element category, recursive jagged nesting, empty or dynamic default construction, immutable `len()`, signed checked indexing, named deep copy, produced-backing adoption, arbitrary-length inline replacement, copied slices, checked equal-length slice assignment with snapshot semantics, explicit shared projection, call-scoped aliases with detached-backing anchors, internal owning boundaries, checked allocation, and deterministic reverse cleanup. |
| Runtime scalar observation | **Implemented contract** | Repository runtime support for line-oriented `i64`, `u64`, `u8`, and `bool` output plus raw-bit `f64` observation, reached through ordinary restricted external declarations rather than language built-ins. |

The [implemented grammar](GRAMMAR.md) is the precise accepted syntax authority.
Focused semantic documents own the corresponding language rules; this matrix
changes only when feature maturity or compiler support changes.

## Compiler availability

| Surface | Status | Current boundary |
|---|---|---|
| Compiler input | **Implemented contract** | One canonical `.ska` source path per invocation. |
| Target registry | **Implemented contract** | `x86_64-sysv` is the only accepted target name and the default. |
| Host/toolchain execution | **Implemented contract** | Linux x86-64 System V assembly and native executable linking through the configured C compiler driver and versioned C runtime. |
| Artifacts | **Implemented contract** | Textual assembly with `--emit asm`, or a linked executable by default. |
| Linux AArch64 backend | **Exploratory direction** | Named as a future backend direction; no target entry or active backend implementation roadmap exists. |

Target ABI and runtime mechanics are implementation details owned by the
[backend](../compiler/BACKEND.md) and
[runtime ABI](../compiler/RUNTIME_ABI.md) documents, not portable language
guarantees.

## Not implemented

| Area | Maturity | Current direction or unresolved boundary |
|---|---|---|
| Strings | **Exploratory direction** | An immutable language-facing string value is intended; its type/literal forms, encoding, byte semantics, ownership, storage, and library contract are open. |
| [Recoverable and checked exceptions](ERRORS.md#recoverable-and-checked-exceptions) | **Exploratory direction** | Deterministic cleanup is a constraint, but syntax, exception values and sets, handlers, failed-construction behavior, and propagation remain open. |
| [Multiple files and modules](MODULES_AND_INTEROP.md#future-modules-and-broader-interoperation) | **Open question** | Imports, exports, visibility, source-to-module mapping, separate compilation, packages, and cross-module linkage are unspecified. |
| [Loops and iteration](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability) | **Open question** | `while`, `for`, `break`, `continue`, iterator protocols, and their cleanup boundaries are unspecified. |
| [Remaining primitive operations](TYPES_AND_VALUES.md#operators) | **Open question** | Comparisons, division, remainder, bitwise operations, shifts, explicit primitive casts, signed-overflow behavior, and broader floating operations are not implemented as a settled group. |
| [Function values](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability), closures, and generics | **Open question** | Direct named calls are implemented; callable values, capture, generic declarations, inference, and specialization are not specified for Skald. |
| Static state and broader class features | **Open question** | Static members, access control, abstract/final forms, method/function overloads, reflection, and user-defined conversions are not current language contracts. Ordinary initializer overloading is implemented separately above. |
| Standard library | **Open question** | No Skald-written standard library is implemented; current scalar output is bootstrap runtime interoperation. |

These rows are deliberately brief. Future behavior becomes normative only in a
focused language document and reaches **frozen design** only after that design
settles its source-visible and implementation-dependent choices. A roadmap
then owns implementation order rather than redefining the frozen contract.

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
  diagnostics, and compares repeated native status and output;
- the runtime Makefile builds the contract, successful/failing allocation,
  and successful/failing output C harnesses documented in the
  [runtime test guide](../../tests/runtime/README.md);
- the backend registry and its focused test expose only `x86_64-sysv`.

The [development workflow](../development/README.md) defines the supported
validation interface, and `make help` provides its current detailed command
inventory. Case counts and exhaustive test inventories are intentionally not
duplicated here; repository discovery and focused test owners remain
authoritative.

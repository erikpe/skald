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
| [Source and declarations](MODULES_AND_INTEROP.md#compilation-units-and-modules) | **Implemented contract** | UTF-8 `.ska` source modules; ASCII identifiers; line comments; top-level functions, exact-symbol external functions, nominal classes, and interfaces. |
| [Primitive types and literals](TYPES_AND_VALUES.md#literal-types-and-ranges) | **Implemented contract** | `i64`, `u64`, `u8`, `f64`, and `bool`; payload-free `unit` results; spelling-selected numeric types with checked literal ranges. |
| [Primitive expressions](TYPES_AND_VALUES.md#expressions) | **Implemented contract** | Exact-type `+`, `-`, and binary `*` with fixed-width wrapping integer semantics; unary `-` for `i64` and `f64`; grouping, direct calls, field selection, and explicit shared dereference; no implicit conversions or truthiness. |
| [Primitive operator profile](TYPES_AND_VALUES.md#implemented-primitive-operator-profile) | **Implemented contract** | Complete exact primitive unary and binary matrices; no implicit conversion; implemented precedence with non-associative comparison/`is`; wrapping integer arithmetic; floor signed division and divisor-sign remainder; checked `u64` shift counts; IEEE binary64 division and unordered comparison; exact boolean equality and mandatory short-circuit `&&`/`||`; selected-path full-expression cleanup; and three compiler-known panic reasons. Every operation executes through typed HIR, verified MIR, and native x86-64. |
| [Integer division and remainder](TYPES_AND_VALUES.md#implemented-integer-division-and-remainder) | **Implemented contract** | Binary `/` and `%` accept two identical `i64`, `u64`, or `u8` operands. Signed division floors and remainder has the divisor's sign; the signed-minimum pair is defined. Both evaluate eagerly through checked verified MIR, and zero divisors use distinct common-reporter panic reasons. |
| [Floating division](TYPES_AND_VALUES.md#implemented-floating-division) | **Implemented contract** | Binary `/` accepts two exact `f64` operands and follows IEEE-754 binary64 behavior through verified non-failing MIR and x86-64. Signed zero, infinity, subnormal, overflow, underflow, and NaN remain ordinary results; zero divisors never enter a panic path. |
| [Integer bitwise and shift operators](TYPES_AND_VALUES.md#implemented-integer-bitwise-and-shift-operators) | **Implemented contract** | Prefix `~` and binary `&`, `|`, and `^` accept exact integer operands. `<<` and `>>` accept an exact `i64`, `u64`, or `u8` left operand and `u64` count, check the count before shifting, and preserve signed or unsigned fixed-width meaning. All evaluate eagerly left to right, canonicalize `u8`, and execute through verified MIR and native x86-64; excessive counts use the common panic reporter. |
| [Primitive integer comparisons](TYPES_AND_VALUES.md#integer-comparisons) | **Implemented contract** | Exact-type `==`, `!=`, `<`, `<=`, `>`, and `>=` for `i64`, `u64`, and `u8` execute through verified MIR and x86-64 with signed `i64` ordering, unsigned `u64`/`u8` ordering, and canonical `bool` results. |
| [Floating comparisons](TYPES_AND_VALUES.md#floating-comparisons) | **Implemented contract** | Exact-type `==`, `!=`, `<`, `<=`, `>`, and `>=` for `f64` execute through verified MIR and x86-64 with IEEE unordered NaN semantics, signed-zero equality, ordinary infinity ordering, and canonical `bool` results. |
| [Eager boolean operators](TYPES_AND_VALUES.md#boolean-negation-and-equality) | **Implemented contract** | Prefix `!` and exact-type `bool` equality and inequality execute eagerly through verified MIR and x86-64 with canonical results. Postfix optional unwrap binds before prefix negation; boolean ordering and truthiness are not accepted. |
| [Short-circuit boolean expressions](FUNCTIONS_AND_CONTROL_FLOW.md#short-circuit-logical-expressions) | **Implemented contract** | Exact-type `bool` `&&` and `||` evaluate left to right through structured HIR and verified path-dependent MIR. The right operand is skipped when selected by the left result, skipped effects and failures do not occur, selected temporaries clean in reverse completion order, and results remain canonical across every expression consumer and primitive ABI boundary. |
| [Primitive integer casts](TYPES_AND_VALUES.md#explicit-integer-casts) | **Implemented contract** | All nine explicit total two's-complement/modulo casts among `i64`, `u64`, and `u8` execute through verified MIR and x86-64. They preserve same-width bits, retain the low byte when narrowing, zero-extend `u8` when widening, and require no runtime support. |
| [Complete explicit primitive cast matrix](TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix) | **Implemented contract** | All twenty-five explicit casts among `i64`, `u64`, `u8`, `f64`, and `bool` are accepted from source and execute inline on x86-64. Twenty-two cells are pure; checked `f64`-to-integer casts use verified success/failure control flow and the common panic reporter. No cast is implicit. |
| [Bindings and scopes](FUNCTIONS_AND_CONTROL_FLOW.md#lexical-scopes-and-locals) | **Implemented contract** | Typed `var` locals, value parameters, lexical blocks, declaration-before-use for locals, nested shadowing, duplicate rejection within one scope, and exact-type reassignment of initialized `i64`, `u64`, `u8`, `f64`, and `bool` locals and value parameters. Grouped destinations preserve lexical identity; sources evaluate once and store before full-expression cleanup. Parameter reassignment changes only callee-local value storage. |
| [Functions and control flow](FUNCTIONS_AND_CONTROL_FLOW.md#statements-and-blocks) | **Implemented contract** | Forward calls, recursion, direct calls, `unit` call statements, `return`, and mandatory-block `if`/`elif`/`else` with exact `bool` conditions and definite-return checking. |
| [Entry point and primitive interoperation](MODULES_AND_INTEROP.md#program-entry-point) | **Implemented contract** | A defined `fn main() -> i64`; trusted external declarations using their source name as the linker symbol and by-value primitive parameters/results, with `unit` also allowed as a result. |
| [Intrinsic declarations](MODULES_AND_INTEROP.md#intrinsic-function-declarations) | **Implemented contract** | Contextual bodyless top-level `intrinsic fn`; one closed exact-path registry accepts public `std::error::panic(message: std::str::Str) -> unit` and the five private canonical `std::io` byte operations. Calls resolve by stable identity and become dedicated panic or I/O operations before executable IR. |
| [Panic and compiler-known unrecoverable failures](ERRORS.md#frozen-panic-design) | **Implemented contract** | Canonical imported `std::error::panic(Str)`, checked cast, optional, array, string-bound, valid host-allocation, and ownership-count failures use one length-delimited reporter. Panic is uncatchable and non-unwinding; compiler/runtime defects remain hard traps. |
| [Exact nominal classes](CLASSES_AND_LIFECYCLE.md#exact-nominal-classes) | **Implemented contract** | Inline class values; primitive and class fields; one or more ordinary initializer overloads with static most-specific selection for direct construction and `super(...)`; per-overload public or private visibility; and read-only and mutable receiver methods. Opt-in virtual dispatch is tracked separately under dynamic polymorphism. |
| [Declaring-class member privacy](CLASSES_AND_LIFECYCLE.md#declaring-class-privacy) | **Implemented contract** | Public-by-default fields, instance/static methods, and ordinary initializers; contextual `private`; exact declaring-class access across every class-owned body, receiver, and construction form; inherited namespace preservation; post-selection initializer access without fallback; and privacy erasure before HIR. Private methods are direct and do not satisfy interfaces. Copy, assignment, and destruction visibility remain unsupported. |
| [Static methods](CLASSES_AND_LIFECYCLE.md#static-methods) | **Implemented contract** | Public `static fn` and composed `private static fn`; receiverless class and qualified-module calls; inherited identity-preserving selection; declaring-class privacy; ordinary parameters, results, ownership, and cleanup; and explicit exclusion from receiver access, virtual dispatch, and interfaces. Static fields remain unsupported. |
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
| [Arrays](ARRAYS.md) | **Implemented contract** | Invariant inline `T[]`, shared `shared T[]`, and optional-shared `shared? T[]` arrays execute through verified MIR on x86-64 with every documented owning element category, recursive jagged nesting, empty or dynamic default construction with call-site authorization of private class initializers, immutable `len()`, signed checked indexing, named deep copy, produced-backing adoption, arbitrary-length inline replacement, copied slices, checked equal-length slice assignment with snapshot semantics, explicit shared projection, call-scoped aliases with detached-backing anchors, internal owning boundaries, checked allocation, and deterministic reverse cleanup. |
| [Strings](STRINGS.md) | **Implemented contract** | Raw-byte literals conditionally load and validate the exact `std::str::Str` language item, then execute through deterministic immortal backing and ordinary descriptor lifecycle. The canonical Skald standard-library class provides copying construction, checked byte observation, generic byte equality, `O(1)` slicing, independent byte-array conversion, concatenation, canonical boolean and integer formatting, exact optional boolean and integer parsing, shortest round-tripping binary64 formatting, correctly rounded optional binary64 parsing, and dynamic last-owner reclamation; invalid bounds call the imported panic intrinsic through the ordinary `std::str`/`std::error` cycle, without compiler-selected method names or a string runtime ABI. |
| [Standard I/O](IO.md) | **Implemented contract** | Explicitly imported `read_stdin`, `read_file`, `write_stdout`, and `write_stderr` execute as ordinary Skald code over `Str` and five private `u8[]` intrinsics. Reads grow geometrically through EOF and close successful files; writes complete partial transfers. Runtime ABI version 7, dedicated HIR, verified MIR, checked x86-64 pointer/length lowering, exact host calls, and stable all-or-panic failures are implemented. Scalar observability output remains available unchanged. |
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
| [Recoverable and checked exceptions](ERRORS.md#recoverable-and-checked-exceptions) | **Exploratory direction** | Deterministic cleanup is a constraint, but syntax, exception values and sets, handlers, failed-construction behavior, and propagation remain open. |
| [Multiple files and modules](MODULES_AND_INTEROP.md#initial-module-system) | **Implemented contract** | Whole-program compilation supports path-derived modules, anonymous composable roots, `::` qualification, explicit module and selective imports, private-by-default top-level declarations, cyclic multi-module imports with direct self-import rejection, file or logical entry selection, controlled standard-library lookup, deterministic filesystem resolution and identities, and compatible external-ABI coalescing. The in-memory source-text convenience API deliberately has no filesystem/module request context; separate compilation and package distribution remain deferred. |
| [`while`](FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits) | **Implemented contract** | Mandatory-block statement with an exact-`bool` condition, per-test full-expression cleanup, a fresh body lifetime per entered iteration, enclosing mutation, nesting, return, and conservative fallthrough for definite return. |
| [`break`](FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits) | **Implemented contract** | Value-free statement targeting the nearest enclosing loop by stable identity, with deterministic cleanup of every exited body scope and preservation of enclosing state. |
| [`continue`](FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits) | **Implemented contract** | Value-free statement targeting the nearest enclosing loop, with deterministic cleanup of exited body scopes before a fresh condition evaluation. |
| [Other loops and iteration](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability) | **Open question** | `for`, `for ... in`, `do while`, unconditional loops, iterator protocols, loop expressions and values, loop `else`, and labels are unspecified. |
| [Deferred operator and conversion work](TYPES_AND_VALUES.md#deferred-operator-and-conversion-work) | **Open question** | Power, floating remainder, object and user-defined operators, implicit promotion, non-primitive and user-defined conversions, total floating ordering, selectable overflow modes, compound assignment, and other explicitly excluded operations require separate designs. |
| [Primitive string conversions](STRINGS.md#frozen-primitive-textual-conversions) | **Implemented contract** | Ten explicit `Str.from_<type>` and `Str.to_<type>` methods cover all five primitives through ordinary `std::str` source. Formatting is canonical ASCII and locale-independent; parsing is optional and complete-input. Binary64 formatting is shortest and bit-round-tripping, while binary64 parsing is correctly rounded. No method is a compiler intrinsic or runtime conversion service. |
| [Function values](FUNCTIONS_AND_CONTROL_FLOW.md#unsupported-control-flow-and-callability), closures, and generics | **Open question** | Direct named calls are implemented; callable values, capture, generic declarations, inference, and specialization are not specified for Skald. |
| Static state and broader class features | **Open question** | Static fields and state, abstract/final forms, method/function overloads, reflection, and user-defined conversions are not current language contracts. Declaring-class privacy, static methods, and ordinary initializer overloading are implemented separately above. |
| Broader standard library | **Open question** | The canonical string module, primitive textual conversions, and initial whole-stream I/O surface are implemented. Collections, general formatting and parsing, and broader library organization remain open. |

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

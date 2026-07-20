# Repository Structure and Compiler Architecture

Status: implemented first-vertical-slice architecture and forward design contract.

This document describes the implemented structure of the stage-0 Skald compiler repository and records the architectural contract for extending it. Future-facing components are identified explicitly rather than implied to exist.

## 1. Design Principles

The initial `skac` compiler is written in Rust and targets Linux. Its design should optimize first for clarity, maintainability, and extension rather than cleverness or premature optimization.

The following principles guide the implementation:

1. **A visible pipeline.** Compilation is a sequence of named phases with explicit inputs, outputs, and invariants. The driver orchestrates phases but does not absorb their logic.
2. **One semantic authority per phase.** Tokens own lexical facts, syntax owns source structure, resolution owns symbol identity, typed HIR owns language-level type facts, and MIR owns executable evaluation and cleanup order.
3. **Forward-only dependencies.** A later phase may depend on the output model of an earlier phase. Earlier phases must not depend on later phases, and backends must not inspect AST or type-checker internals.
4. **Data across boundaries.** Phase interfaces should primarily exchange explicit, inspectable data structures rather than callbacks into mutable phase state.
5. **No hidden global compilation state.** Source files, diagnostics, target selection, options, interning, and caches belong to an explicit compilation session or request.
6. **Stable identities after resolution.** Resolution assigns program identities
   defined by a neutral `identity` module. Later phases preserve typed IDs
   rather than depending on resolver internals, source names, or object
   identity.
7. **Diagnostics survive lowering.** Source spans and useful origin information remain available as syntax becomes progressively less source-shaped.
8. **Deterministic output.** Diagnostics, IR dumps, symbol ordering, generated labels, and assembly should be stable across runs.
9. **Verify phase invariants.** Important IR boundaries have inexpensive verifier passes. Invalid compiler state is caught close to the phase that produced it.
10. **User errors are not panics.** Invalid Skald programs produce structured diagnostics. Rust panics indicate compiler defects or violated internal invariants.
11. **Optimization is explicit.** Analysis and transformation passes live in named pipelines. They are not hidden inside parsing, type checking, or assembly emission.
12. **Keep the runtime small.** Skald has no garbage collector. The C runtime should contain only facilities that cannot reasonably or safely live in generated code or the future Skald standard library.
13. **Keep targets isolated.** Target ABI rules, instruction selection, register and frame planning, and assembly syntax stay behind a backend interface.
14. **Build for replacement.** A phase implementation may evolve without forcing unrelated phases to change, provided its documented output contract remains stable.

## 2. Relationship to Niflheim

The sibling Niflheim repository at `../niflheim` is an important design reference. It contains useful language behavior, diagnostics, test organization, runtime conventions, compiler passes, ABI work, and backend experience that Skald should consult frequently.

Reference does not mean direct architectural inheritance. Niflheim grew organically and accumulated coupling and technical debt as its scope expanded. Skald should reuse proven decisions and test ideas while deliberately improving:

- phase ownership and boundaries;
- typed, stable IDs;
- separation between syntax, semantic IR, and executable IR;
- explicit pass orchestration and verification;
- separation of common backend contracts from target code;
- compiler/runtime ABI documentation;
- deterministic dumps and diagnostics;
- avoidance of legacy and replacement pipelines living indefinitely beside one another.

Niflheim is neither a source dependency nor a normative dependency. Skald's specification and repository documentation take precedence.

## 3. Top-Level Layout

```text
skald/
├── Cargo.toml
├── Cargo.lock
├── Makefile
├── README.md
├── crates/
│   ├── skac/
│   └── skald-compiler/
├── docs/
├── grammar/
├── runtime/
│   ├── include/
│   └── src/
├── std/
├── tests/
│   ├── compiler/
│   ├── runtime/
│   └── golden/
├── samples/
└── scripts/
```

### `crates/skac/`

The `skac` binary crate is intentionally thin. It owns process-level concerns such as command-line arguments and process exit codes, then delegates compilation to `skald-compiler`.

It must not contain lexer, parser, type-checker, IR, or target implementation logic.

### `crates/skald-compiler/`

The Rust library containing the compiler pipeline. It begins as one library crate with strongly separated modules. A phase should become its own crate only when that produces a concrete build, reuse, or dependency benefit; the initial architecture should not pay a multi-crate tax for every small phase.

```text
src/
├── lib.rs
├── test_support.rs       # `cfg(test)` only
├── dump_format.rs        # private deterministic dump primitives
├── driver/
├── function_table.rs
├── identity.rs
├── source.rs
├── diagnostics/
├── lexer/
├── syntax/
├── resolve/
├── hir/
├── typeck/
├── mir/
├── passes/
└── backend/
    └── x86_64_sysv/
```

Modules with multiple implementation responsibilities use the recursive
directory layout. Their `mod.rs` files act as concise facades: they document
the boundary, declare private implementation modules, and explicitly re-export
the intended public API. Substantial module-level unit tests live in an
adjacent behavior-oriented `tests/` directory; smaller suites may use
`tests.rs`, and small cohesive modules may keep a few tightly local tests
inline.

Cross-phase formatting stays deliberately narrow. The private `dump_format`
module owns only the byte-identical indentation, quoted-string escaping, and
source-span suffix used by compiler dumps; each phase still owns the structure
and vocabulary of its representation. Diagnostic type sets likewise remain
owned by the phase enforcing them, while one diagnostics helper renders those
sets consistently as source type names.

The completed R0–R12 cleanup audit establishes the current maintenance
baseline. Stable IDs are owned by `identity`, not resolution; typed dense and
sparse tables have one implementation; `BlockFlow` is the sole structured
return-flow result consumed by both type checking and MIR lowering; and all
test-only mutation and fixture APIs are excluded from production builds. The
largest production module is the cohesive MIR verifier rather than a catch-all
pipeline phase. No migration compatibility layer remains.

### `runtime/`

The minimal C runtime and its public ABI header. It builds as a static archive and is linked with generated assembly by the system C toolchain. ABI version 2 adds `ska_rt_println_i64(int64_t)`, a bootstrap output service that writes the shortest locale-independent ASCII decimal representation and one LF byte to stdout. ABI version 3 adds `ska_rt_println_bool(bool)`, which writes lowercase ASCII `true` or `false` and one LF. T1 implements ABI version 4 with unsigned decimal observation for `u64` and `u8` plus exact raw-bit observation for binary64 `f64`. All operations share internal formatting helpers and one checked record-writing boundary, flush the completed record before returning, and terminate the process unsuccessfully after a detected write or flush failure.

The runtime keeps C library implementation types such as `FILE *` private. Its public surface uses fixed-width integer types and standard C `bool`, and direct C consumers verify both header/archive version agreement and externally observable behavior. Later likely responsibilities include allocation, reference-count operations, panic reporting, runtime type metadata helpers, and other narrowly defined primitives. Garbage collection, root stacks, tracing, safepoints, and write barriers do not belong here.

### `grammar/`

Canonical grammar sources and parser-facing notes. The language grammar is not yet complete, so this directory initially records that status rather than pretending a partial grammar is normative.

### `std/`

Future Skald standard-library source. It is separate from the C runtime: functionality that can be expressed safely and efficiently in Skald should eventually live here.

### `tests/`

- `compiler/` is reserved for larger cross-phase fixtures and compiler integration tests. Current phase-level Rust tests live beside their implementation.
- `runtime/` contains direct C tests of the runtime ABI and implementation.
- `golden/` contains complete source-to-diagnostic, source-to-assembly, and source-to-executable cases.

### `samples/`

Small demonstration and bring-up programs. Samples are not substitutes for regression tests.

### `scripts/`

Thin wrappers for repeated repository workflows. Compiler behavior must remain available through `skac` or library APIs rather than existing only inside shell scripts.

### Root development commands

The root `Makefile` provides discoverable wrappers around the native Rust and C build tools:

| Command | Purpose |
|---|---|
| `make fmt` | format Rust source |
| `make fmt-check` | check Rust formatting without modifying files |
| `make build-check` | type-check every Rust workspace target |
| `make lint` | run Clippy across the workspace with warnings denied |
| `make compiler-test` | run Rust workspace tests |
| `make golden-test` | run native source-to-executable golden cases |
| `make runtime` | build the C runtime archive |
| `make runtime-test` | build and run direct C runtime tests |
| `make check` | run the complete repository validation suite |

These commands are convenience entry points, not replacements for Cargo or the runtime Makefile. Build output belongs under ignored `target/` and `build/` directories.

## 4. Compiler Pipeline

The implemented compiler pipeline is:

```text
compilation request
    → source database
    → tokens
    → syntax AST
    → resolved program
    → typed HIR
    → target-independent MIR
    → explicit MIR verification/pass pipeline
    → target backend lowering
    → target machine/assembly model
    → textual assembly
    → system assembler and linker + C runtime
    → Linux executable
```

Each arrow is an explicit API boundary. The driver composes them through `compile_source_to_assembly`; phase implementations remain independently callable and do not depend on the driver.

| Phase | Primary input | Primary output | Must establish |
|---|---|---|---|
| Driver/session | CLI or library request | compilation session and ordered phase execution | options, target, source ownership, diagnostic policy |
| Source | paths and bytes | source IDs, text, spans, line maps | stable source identity and valid offset ranges |
| Lexer | one source | token stream | token kinds, literal spelling, trivia policy, precise spans |
| Parser | token stream | source AST | grammatical structure without semantic lookup |
| Resolution | AST declarations and names | resolved program and typed symbol IDs | unique identities, scopes, duplicate and missing-name diagnostics |
| Type checking | resolved program | typed HIR | expression types, legal calls and operations, no unresolved semantic choices |
| MIR lowering | typed HIR | target-independent executable IR | explicit evaluation order, temporaries, calls, branches, returns, and eventually cleanup |
| MIR passes | verified MIR | verified MIR plus analyses | named, ordered transformations with preserved semantics |
| Backend | MIR and target options | target-specific machine/assembly model | ABI lowering, instruction selection, frame and register decisions |
| Assembly emission | target model | deterministic textual assembly | valid toolchain input and stable symbol/section conventions |
| Toolchain/link | assembly and runtime archive | executable | object generation and Linux linkage |

The driver publishes assembly and executable artifacts through one shared
same-directory temporary-file boundary. A completed artifact becomes visible
at its destination with one rename; failed compilation, output, or toolchain
work leaves an existing destination unchanged, and ordinary failure paths
remove the unpublished temporary file through RAII. An explicit CLI output
that has the same filesystem identity as its source is rejected before the
compiler pipeline runs, including aliases through symbolic or hard links.

### Source and diagnostics

All phases use a common span representation backed by the source database. Diagnostics contain structured severity, message, labels, and notes; terminal rendering happens at the driver edge. Tests should be able to assert diagnostic structure without depending on terminal colors or absolute paths.

M1 establishes UTF-8 byte offsets as the internal span unit and one-based Unicode-scalar line/column locations for display. Source IDs follow deterministic insertion order. The plain-text diagnostic renderer is color-free and stable for tests; richer terminal or editor renderers can consume the same diagnostic structures later.

The implemented M1 lexer returns a token stream and accumulated diagnostics together, preserving `Invalid` tokens for recovery instead of aborting on the first lexical error. T2 replaces the original integer-only path with one numeric scanner and an explicit source-level literal classification shared by tokens, AST, and resolved IR. The lexer retains spellings indirectly through complete spans and does not convert semantic values. It recognizes the boundaries of contracted `i64`, `u64`, `u8`, and `f64` spellings; T3 enables unsuffixed `i64` and `u`-suffixed `u64`, T4 enables `u8`-suffixed literals, and T6 enables decimal-point and exponent `f64` literals. Malformed forms remain one recoverable invalid token. Its lexical contract is recorded in [`grammar/README.md`](../grammar/README.md).

### Syntax AST

The AST mirrors source constructs and preserves spans. It must not become the long-lived semantic representation. In particular, later phases should not repeatedly resolve strings or attach growing sets of optional semantic fields to parser nodes.

M2 implements the source AST as separate node, parser, and dump modules behind the public `syntax` boundary. O5 generalizes its source-ordered top-level declaration list to distinguish Skald definitions from bodyless `extern fn` declarations without attaching optional bodies to one oversized node. T2 represents numeric literals uniformly as a classified kind, original spelling, and complete span; resolution preserves all three, and only type checking converts the spelling to a semantic value. The recursive-descent parser performs no name or type lookup. It uses explicit precedence levels and recovers at parameter, statement, block, and top-level declaration boundaries, returning a partial AST together with accumulated structured diagnostics. The exact implemented grammar is recorded in [`grammar/README.md`](../grammar/README.md).

One `Parser` object owns source text, tokens, cursor state, and accumulated
diagnostics. Its implementation is split by grammar responsibility under
`syntax/parser/`: `declaration` owns top-level forms, parameters, and source
types; `statement` owns blocks, conditionals, locals, and returns; `expression`
owns precedence, grouping, and calls; and `recovery` owns synchronization and
expression-start classification. Token consumption, span construction, and
diagnostic emission remain centralized on the parser state. Source type tokens
map to `TypeKind` in one place, while each caller explicitly selects a result
or stored-value context to determine whether `unit` is accepted.

R6 bounds recursively nested syntax with one shared parser counter. Function
bodies, nested blocks, grouped and unary expressions, and active postfix calls
may occupy at most 128 levels. Crossing the limit produces `PAR005`, skips the
rest of the affected declaration iteratively, and prevents its partial AST from
entering resolution. The common guard adds no allocation to ordinary parsing;
the canonical limit and recovery contract are documented in
[`grammar/README.md`](../grammar/README.md).

### Resolution and typed HIR

Resolution assigns stable IDs and establishes scopes before type checking. Typed HIR preserves enough source structure for good diagnostics but makes chosen operations and call targets explicit. A backend must never perform name lookup, overload selection, or language-level type checking.

`FunctionId`, `ClassId`, `FieldId`, `InitializerId`, `MethodId`, `CallableId`,
`ParameterId`, `LocalId`, and `BindingId` are defined in the neutral `identity`
module rather than resolved IR. Resolution remains responsible for assigning
them when it selects declarations and bindings from source. Resolved IR, typed
HIR, MIR, and backends then share those identities directly; later phases do
not import identity types through `resolve` or choose program entities by
comparing source names. Identity construction remains crate-private, while
indexing, ownership queries, ordering, and deterministic display are stable
phase-independent operations.

OBJ1 makes `CallableId` a tagged identity whose alternatives directly contain
a top-level `FunctionId`, class-owned `InitializerId`, or class-owned
`MethodId`. It is both the semantic executable-declaration identity and the
owner of that body's parameters, locals, MIR storage, transient values, and
blocks; MIR verification errors likewise identify their callable owner. There
is no second global code-generation body number and therefore no translation
map to maintain or reconstruct by name. Existing function-owned IDs retain
their `fN` display, while member bodies use owner-qualified forms such as
`c2:init0` and `c2:method3`.

The private `function_table` module provides the two established storage
shapes shared by resolved IR, HIR, and MIR: dense declaration entries ordered
by `FunctionId`, and sparse definition slots whose missing entries represent
bodyless declarations. It centralizes ID/slot validation, lookup, deterministic
iteration, occupancy counting, and test-only mutation bookkeeping. Each phase
retains its own public declaration and definition table wrappers and record
types; the utility exposes neither raw vectors nor a general arena or ID-trait
framework.

OBJ1 deliberately leaves the shared table utilities function-specific rather
than introducing a generic arena or identity-index trait. OBJ2 adds MIR's
narrow dense class table alongside its canonical class/member records. OBJ6
will add the resolver's phase-owned form; a container should be shared only
when two real tables have the same owner and density rules.

OBJ4 adds MIR's deterministic member-definition table, keyed directly by
`CallableId`, while retaining the existing dense/sparse top-level function
tables. `MirDefinitionRef` presents their common executable-body interface to
verification, frame planning, dumps, and code generation without erasing the
different declaration ownership models. A member body identifies one explicit
receiver storage slot owned by the initializer or method callable; that slot is
an addressable class place in MIR but a saved pointer home in the backend.

M3 implements resolution as declaration collection followed by body resolution. Its separate resolved representation has a dense source-ordered function declaration table, a separately indexed definition table, owner-qualified parameter and local IDs, ID-based binding uses, and ID-based direct calls. O5 places external declarations in the same non-overloaded namespace and ID sequence, records their source identifier as exact-symbol linkage, and leaves their definition slot absent. Declarations own names, signatures, and linkage; definitions own locals and bodies. Public tables support lookup by ID but intentionally provide no name-based declaration-selection API. The `main` name is resolved once into an optional entry candidate; type checking rejects an external candidate and requires a defined `fn main() -> i64`.

Each function resolver owns an explicit lexical scope stack. Parameters share the outer function-body scope, nested blocks push scopes, and local initializers are resolved before their binding is introduced. Duplicate and lookup failures produce structured diagnostics with source labels. The precise first-slice rules are recorded in [`grammar/README.md`](../grammar/README.md).

M4 lowers successful resolved input into a distinct typed HIR. HIR preserves the declaration/definition split, so call checking consults canonical typed signatures without requiring a local body. Implemented semantic types are `i64`, `u64`, `u8`, `f64`, `bool`, and payload-free `unit`; every HIR expression stores its type, primitive operators are explicit typed operations, and calls retain exact checked function IDs. O5 validates the deliberately restricted external ABI profile during this phase, C2 extends it with by-value boolean parameters and results, T3 adds by-value `u64`, T4 adds by-value `u8`, and T6 adds by-value `f64`. Decimal spelling is converted exactly once here: integer families receive independent range checking, while finite `f64` spellings are rounded to nearest binary64 with ties to even and stored below HIR as raw bits. Boolean, signed, unsigned, and floating literals remain distinct typed nodes. Entry-signature, call-arity, expression, initializer, return-value, and mandatory-return checks accumulate diagnostics across the program. HIR is deliberately all-or-nothing: failed type checking returns diagnostics but no executable `HirProgram`, preventing M5 from consuming partial typed state.

The type checker separates program and function responsibilities. Its
`program` module orchestrates checking, validates entry and external
declarations, and constructs the final all-or-nothing HIR. Each function body
is checked through a `FunctionChecker` that owns references to the current
program, declaration, definition, return type, and shared diagnostic sink.
Recursive block and expression calls therefore pass only the syntax node that
changes. Focused `function`, `expression`, and `literal` modules own statement
and conditional flow, expressions/calls/bindings/operators, and numeric
conversion/range diagnostics respectively.

Type checking also computes one authoritative `BlockFlow` summary while it
checks every block and conditional. `FallsThrough` means at least one path can
reach the construct's end; `Terminates` currently means every path returns
from the function. The summary is stored in typed HIR. Missing-return
diagnostics inspect the function body's summary, while MIR lowering uses the
conditional summary to decide whether a join block is required. Neither phase
recursively re-analyzes an earlier representation.

Future control flow should extend this same computation rather than add a
parallel analysis. Loops first need to contribute fallthrough versus proven
non-fallthrough behavior. If later phases must distinguish function return,
divergence, checked-exception propagation, or other non-local exits,
`BlockFlow` can become a richer outcome set while retaining one composition
operation in type checking. MIR cleanup and edge lowering can then consume the
specific outcomes they need from the recorded summary.

### MIR

MIR is target-independent and executable in shape. It uses explicit basic blocks and terminators even though the first slice lowers only straight-line functions. It owns facts such as:

- exact evaluation order;
- explicit temporaries and local identities;
- direct call targets;
- control-flow edges and returns;
- primitive operations with defined types;
- later, construction state, cleanup edges, alias anchors, retain/release operations, and exceptional exits.

The initial MIR need not use static single assignment form. IDs and control-flow APIs should nevertheless avoid assuming that mutable local slots are the only possible representation. SSA can later be introduced as:

- a distinct IR between MIR construction and backend lowering;
- an optional conversion and optimization pipeline over MIR; or
- a replacement internal representation behind a preserved MIR/backend contract.

That choice should be made when real optimization requirements exist. The current architecture must make it possible without prematurely building an SSA framework.

M5 implements a three-address MIR with separate owner-qualified storage, transient value, and basic-block IDs. MIR has a dense callable declaration table containing canonical signatures and linkage, plus a sparse definition table containing executable bodies. Source parameters and locals map to explicit storage slots, while instruction results are immutable value IDs; this hybrid keeps mutation visible without committing the entire IR to SSA. Calls are effectful instructions with explicit stable targets, argument lists, and optional result IDs. Expression lowering emits instructions recursively in deterministic left-to-right order.

C3 adds target-independent `Goto` and boolean `Branch` terminators alongside
`Return`. The public MIR body builder allocates dense blocks in stable order,
switches the current insertion block explicitly, rejects instructions after a
terminator, and rejects duplicate termination. Terminators expose deterministic
successors, with a branch's true edge before its false edge. Transient values
are block-local in this non-SSA MIR; state crossing an edge must use explicit
storage.

C5 adds structured conditional statements to AST, resolved IR, and typed HIR
while preserving their flat source-ordered arms. HIR-to-MIR lowering expands
them into deterministic condition, body, false-continuation, and optional join
blocks. Conditions remain in the containing lexical scope, each body has its
own child scope, transient MIR values do not cross block edges, and lowering
omits unreachable joins when every exhaustive arm terminates.

The MIR verifier is a separate public boundary and checks declaration and definition associations, linkage/body consistency, external exact-symbol metadata, ID ownership, parameter order, definition/signature agreement, block-local use ordering, storage/value types, direct-call targets and result presence, return types, dense block IDs, entry blocks, boolean branch conditions, control-flow target ownership and existence, and terminators. OBJ2 extends that boundary with canonical class/member ownership, nominal class storage, projection-chain typing, exact construction targets, receiver typing, member signatures, and the rule that class objects are places rather than transient values. It validates unreachable blocks as well as reachable ones. Lowering invokes it through a debug assertion, and focused tests deliberately corrupt valid MIR to cover rejection paths. Its stable textual dump exposes classes and callable declarations separately from definitions and shows semantic places without target offsets.

### Restricted inline-object extension boundary

OBJ0 specifies the architecture for the first inline-object profile. OBJ1
implements its neutral identity and executable-body ownership foundation. OBJ2
implements target-independent class/member metadata, places, construction and
receiver calls, plus their verifier boundary. OBJ3 implements the target layout,
object frame-allocation, and projected-address boundary. OBJ4 implements
executable member definitions and the hidden receiver ABI. OBJ5–OBJ9 implement
the remaining frontend and integration work.
Public syntax remains disabled until the complete path exists.
The extension must preserve these boundaries:

- the neutral identity layer owns stable class, field, initializer, method, and
  executable-body identities, while resolution assigns them;
- AST, resolved IR, HIR, and MIR each own phase-appropriate class/member tables
  rather than sharing source nodes or mutable resolver state;
- HIR records nominal class types, selected member identities, and receiver
  access modes; no backend repeats member selection;
- MIR generalizes scalar storage access to an addressable place consisting of a
  storage base plus semantic projections such as a field identity;
- zero-projection places remain the one representation for scalar locals, so
  object work does not leave a parallel scalar-only load/store path;
- object locals are storage places but not transient MIR values in this slice;
  object copies, arguments, results, and aggregate rvalues remain invalid;
- initialization is an explicit operation into a destination place, distinct
  from assignment or an object-producing rvalue;
- a direct method call carries a receiver place separately from its explicit
  primitive values, preserving receiver-before-argument evaluation;
- the MIR verifier owns projection typing, construction-target, receiver, and
  scalar-value invariants before any backend sees the program;
- MIR retains class/field identities but no byte offsets, target alignment,
  registers, or linker spellings.

The initial x86-64 backend owns one checked, immutable target data-layout
service. It computes class layouts once in dependency order, rejects recursive
inline containment and checked-arithmetic failures, and lays
out fields in declaration order, gives empty classes size/alignment one, uses
8/8 for `i64`, `u64`, and `f64`, and 1/1 for `u8` and `bool`. Each object local
receives one contiguous aligned frame allocation. The frame-layout boundary
resolves both zero-projection scalar places and nested field projections to a
single frame-relative address. Narrow projected `u8` and `bool` accesses use
byte loads/stores while transient scalar homes retain their existing canonical
representation. Instruction selection and semantic lowering never duplicate
layout arithmetic or contain byte offsets.

Initializers and methods receive an address to complete object storage as a
hidden first integer-class System V argument. OBJ4 materializes that address
from a MIR receiver place, saves it in the callee's receiver pointer home, and
resolves receiver field places through the saved address. Forwarding `self` to
another method reloads the same address without introducing a pointer MIR
value. The receiver consumes an integer argument location but no SSE location,
so independent integer/SSE counters continue to classify explicit parameters
and source-ordered overflow arguments use the aligned stack area. Internal
function, initializer, method, block, and epilogue symbols come from one
collision-proof identity-based service. Object types remain prohibited in the
exact-symbol C ABI.

These structures deliberately prepare for field-projection chains, recursive
layouts, cleanup state, receiver aliases, base projections, virtual calls, and
shared metadata without implementing them early. Future slices should extend
the place and construction models rather than introduce object pointers or
byte-offset instructions into target-independent IR.

### Passes and verification

Each implemented IR has a deterministic textual dump suitable for tests. The public renderers and focused debugging workflow are indexed in [`DEBUGGING.md`](DEBUGGING.md). MIR has a verifier that checks IDs, block termination, operand types, call signatures, and target-independent invariants.

The explicit MIR pass pipeline currently performs unconditional verification and no transformations. Future passes must declare what analyses they require or invalidate, and their ordering belongs at this visible boundary. Correctness must never depend on an optimization pass having run.

## 5. Backend Structure

The backend interface accepts verified target-independent MIR, layout information defined by the compiler/runtime ABI, and target options. It returns target-specific output or structured diagnostics.

The initial backend is `x86_64_sysv`, targeting Linux with the System V AMD64 ABI. Its internal concerns should remain separated even if initially implemented in few files:

- target legality checks;
- ABI classification and call lowering;
- instruction selection;
- virtual register or value-location planning;
- stack-frame layout;
- physical register allocation;
- prologue/epilogue and call-site emission;
- symbol mangling and section policy;
- deterministic assembly formatting.

An AArch64 Linux backend is expected after the x86-64 pipeline is established. Adding it should require a new target module and registry entry, not conditionals scattered through semantic phases. Cross-target tests should distinguish:

- semantic tests shared by every backend;
- assembly-shape tests runnable without executing the target;
- native execution tests gated by the host architecture or an explicit emulator.

M6 implements the first backend behind a small target registry that currently accepts only `x86_64-sysv`. Target legality is checked before lowering, with target-independent MIR verification running first. C4 removes the temporary single-block restriction after implementing every MIR terminator. Invalid graphs remain structured backend errors rather than panics or silently altered code.

The x86-64 implementation separates ABI classification, frame planning, instruction selection, a typed target assembly model, and GNU textual emission. Every MIR storage slot and transient value initially receives an eight-byte stack home in a 16-byte-aligned fixed frame. This intentionally stack-heavy strategy uses `%rax` and `%rcx` as caller-saved scratch registers and preserves only the frame pointer; future register allocation can therefore replace a contained location-planning decision without changing MIR. C4 gives every MIR block a collision-proof `.Lska_fn_N_block_M` label and emits blocks in stable ID order. `Goto` becomes an unconditional jump; `Branch` loads and tests its canonical boolean condition, jumps to the true target when nonzero, and otherwise jumps explicitly to the false target. A function-local epilogue label centralizes frame teardown for returns from any block.

R7 keeps instruction selection behind one exhaustive `MirInstruction`
dispatcher while splitting operation policy under `x86_64_sysv/lower/`.
`assignment` owns constants, loads, and exhaustive integer and floating unary
and binary rvalue selection; `call` owns incoming parameter spills and outgoing
System V calls; `value` owns stack-home movement and canonicalization; and
`terminator` owns returns, jumps, and branches. The parent lowering module now
orchestrates functions and target labels only. ABI classification, frame
planning, legality checks, the typed machine model, and assembly emission remain
independent boundaries.

T4 keeps `u8` in those general eight-byte homes while defining only the low
eight bits as its language value. One typed instruction-selection helper
zero-extends `%al` into `%rax` before a produced or incoming `u8` is stored or
returned. Arithmetic therefore wraps modulo 256 at every MIR value boundary,
and register arguments, stack arguments, internal results, and external
results all enter general Skald use in canonical `0..=255` form.

T5 replaces positional integer-only argument helpers with one complete scalar
call-layout abstraction. Linux x86-64 System V integer-class and SSE-class
registers have independent counters; exhausted classes share deterministic,
source-ordered eight-byte stack slots in a 16-byte-aligned outgoing area. The
layout belongs entirely to the backend, while MIR retains only semantic types.

Target-independent MIR now represents `f64` constants as raw binary64 bits and
uses explicit typed add, subtract, multiply, and negate operations. The x86-64
machine model contains XMM operands and caller-saved `%xmm14`/`%xmm15`
selection scratch registers. Lowering uses scalar SSE2 moves and arithmetic,
returns `f64` in `%xmm0`, and handles internal and external calls identically.
Hand-built verified MIR tests cover this path directly, while T6 source tests
exercise the same representation through the complete frontend pipeline.

System V integer-class parameters, including `bool`, use `%rdi`, `%rsi`, `%rdx`, `%rcx`, `%r8`, and `%r9`; `f64` parameters independently use `%xmm0` through `%xmm7`; exhausted parameters use the shared stack area. Parameters are spilled into their frame homes on entry. Integer-class results use `%rax`, `f64` uses `%xmm0`, and an external C boolean result is normalized from `%al` before storage. `unit` has no result payload and neither reads nor writes a fictitious return register. The backend selects call symbols centrally from declaration linkage: internal definitions use deterministic GNU-local `.Lska_fn_N` symbols that cannot collide with valid exact external identifiers, while external declarations retain their declared symbol. Assembly-shape tests cover mixed register/stack ABI boundaries, frame and scratch-register policy, call linkage, boolean normalization, integer and SSE2 instructions, legality rejection, native floating execution, and acceptance by the system assembler.

T7 audits primitive extension points across source kinds, semantic types,
typed operations, HIR-to-MIR lowering, MIR verification and dumps, target ABI
classification, and instruction selection. Matches at independent phase
boundaries intentionally remain exhaustive so a new enum variant creates a
compile-time list of decisions. Within the x86-64 backend, all payload types
now pass through one exhaustive scalar ABI-class function rather than repeated
integer-type lists. A focused test enumerates every current primitive, while
native goldens cover register-only and independently exhausted integer/SSE
calls.

The target-specific assembly model remains owned by the backend and does not leak target registers or ABI details into MIR.

## 6. Assembly, Runtime, and Link Boundary

`skac` emits textual assembly rather than machine code or object files. The assembly is an inspectable compiler artifact and a stable debugging boundary. The M7 compilation API owns forward phase orchestration from source text through backend emission, while source-file I/O, output publication, and subprocess execution remain at the driver edge.

The build flow is:

1. `skac` compiles `.ska` input to a target assembly file;
2. the system C compiler driver assembles that file;
3. the generated object is linked with the Skald runtime archive and required system libraries;
4. the result is a Linux executable.

The CLI supports both `--emit asm` and executable output. In executable mode, the driver streams assembly to the C compiler over standard input, links the runtime archive, and publishes a successfully linked temporary file to the requested output path. It never evaluates a shell command. Missing runtime archives, process-start failures, write/wait failures, nonzero tool exits, and publication failures are reported as driver errors rather than panics. `CC` selects the compiler driver, while `SKALD_RUNTIME_ARCHIVE` overrides the source-tree runtime archive path.

The x86-64 backend emits a C-compatible `main` wrapper around the ID-selected Skald entry function. The wrapper preserves System V stack alignment and returns the Skald `i64` result through `%rax`; C observes its low 32 bits as `int`, and Linux exposes the low eight bits as process status. Skald functions keep stable internal symbols and do not acquire C linkage merely because one is the language entry function.

Compiler-generated calls into C use a versioned, documented ABI with a consistent symbol prefix such as `ska_rt_`. Runtime headers are the C authority; matching compiler-side declarations and layout constants should be centralized and tested for parity.

## 7. Testing Strategy

Testing follows the useful high-level split from Niflheim while adapting it to Rust.

### Compiler tests

- lexer and parser tests assert tokens, AST shape, spans, and recovery;
- resolution and type-check tests assert stable IDs, semantic types, and diagnostics;
- HIR and MIR tests use deterministic dumps and verifier failures;
- pass tests check one transformation at a time plus semantic preservation where practical;
- backend tests assert ABI decisions and emitted assembly without requiring native execution;
- driver tests check CLI behavior, phase selection, and toolchain command construction.

Fast Rust unit tests live beside the module under test. Larger compiler fixtures and cross-module tests belong under `tests/compiler/` when they are needed.

R8 centralizes repeated unit-test setup in the compiler's `cfg(test)`-only
`test_support` module. Its source helpers expose lexing, parsing, resolution,
type checking, MIR lowering, and assembly generation as explicit boundaries.
Each helper requires only earlier phases to succeed, leaving diagnostics or
errors from the named phase for its caller to assert. The same module provides
unique RAII temporary directories and files for compiler tests; CLI integration
tests use an equivalent private helper because compiler-internal test code is
not exported to dependent crates. Drop-based cleanup also runs during assertion
unwinding. None of these helpers or their dependencies enter production library
builds.

R9 keeps larger phase-level suites beside their implementation while splitting
them into behavior-oriented modules. MIR tests are grouped by builder,
lowering, control flow, verification, and dumps. Type-checker tests are grouped
by declarations, expressions, literals, control flow, diagnostics, and dumps;
syntax and resolution follow the same principle where their suites benefit.
The x86-64 suite separates instruction selection, calls, control flow,
legality, assembler acceptance, and native execution, while ABI classification
tests remain beside the ABI implementation. Shared fixtures stay in private
parent or support modules, preserving access to implementation details without
exporting test APIs or moving phase-level coverage into golden tests.

### Runtime tests

Small C harnesses compile directly against the runtime archive. This isolates runtime behavior from compiler correctness and catches ABI mismatches early. R10 separates the suite into contract, successful-output, and fatal-output binaries that the runtime Makefile builds and runs deterministically. The contract harness owns the ABI-version and platform assertions. The successful-output harness redirects stdout to a temporary file and compares exact bytes across zero, signed values, both `i64` extrema, boolean false and true, and consecutive calls. T1 adds `u64` and `u8` boundaries, raw binary64 patterns for both zero signs, finite extrema, infinity, and a retained NaN payload, plus mixed consecutive records. The fatal-output harness closes stdout in isolated child processes and verifies that every bootstrap output operation terminates unsuccessfully after a detected write failure. Only system-error reporting and exact f64-bit construction are shared by the two output harnesses; production runtime code remains independent of test support. All harnesses compile as C11 with `-Wall -Wextra -Werror`.

### Golden tests

Golden cases exercise the complete public behavior. A case may specify expected diagnostics, assembly fragments, process exit status, or a combination. Test metadata should use repository-relative paths and avoid unstable absolute filenames or incidental temporary labels.

M7 provides a deliberately small Rust native runner. It discovers `.ska` files under `tests/golden/run/`, reads the expected process status from a matching `.exit` sidecar, builds the runtime archive, invokes the public `skac` binary, executes the result, and reports every case before returning failure. M8 extends the same runner with `tests/golden/compile_fail/` and exact `.stderr` snapshots. O1 adds optional `.stdout` sidecars with exact byte comparison; absence continues to require empty stdout, and runtime stderr remains empty. O6 supplies the end-to-end integer-output case. C2 adds an end-to-end boolean-output case covering literals, locals, parameters, and function returns, plus exact bool/i64 and entry-point failures. C5 and C6 add ordered and nested conditional execution, exhaustive and non-exhaustive return analysis, branch-scope failures, and every conditional parser diagnostic family. Successful assembly and failed diagnostics are each produced twice in independent compiler processes and compared for determinism. `make golden-test` runs this suite directly; it is also part of the workspace test suite.

Every implemented language feature should normally receive:

- focused phase-level tests;
- at least one successful end-to-end case;
- compile-failure coverage for its most important invalid forms;
- runtime coverage when it changes the C ABI or ownership implementation.

## 8. Dependency Direction

The desired logical dependency direction is:

```text
source + diagnostics
        ↑
lexer → syntax → resolve → typeck/HIR → MIR → passes
                                           ↓
                                      backend API
                                      ↙         ↘
                              x86_64_sysv     aarch64 (later)

driver depends on and orchestrates all phases
runtime shares only an explicit ABI contract with code generation
```

This diagram describes allowed knowledge, not necessarily Rust crate dependencies. Cycles across these boundaries are architectural defects and should be corrected rather than hidden behind broad utility modules.

## 9. Near-Term Restraint

The first vertical slice should not introduce infrastructure merely because a mature compiler might eventually need it. In particular, it does not need parallel compilation, incremental queries, a general optimization manager, SSA, object-file writing, a package manager, or a large runtime.

It does need boundaries clean enough that those features can be added later without replacing the entire compiler. The completed first-slice scope and milestones are recorded in [FIRST_VERTICAL_SLICE_ROADMAP.md](FIRST_VERTICAL_SLICE_ROADMAP.md), the selected object work is split in [INLINE_OBJECTS_ROADMAP.md](INLINE_OBJECTS_ROADMAP.md), and its preserved extension boundaries are listed in [NEXT_SLICE_BOUNDARIES.md](NEXT_SLICE_BOUNDARIES.md).

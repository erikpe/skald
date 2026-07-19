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
6. **Stable identities after resolution.** Later phases refer to declarations, locals, blocks, and values through typed IDs rather than source names or object identity.
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
├── driver/
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
adjacent `tests.rs`; small cohesive modules may remain single files and keep a
few tightly local tests inline.

### `runtime/`

The minimal C runtime and its public ABI header. It builds as a static archive and is linked with generated assembly by the system C toolchain. ABI version 2 adds `ska_rt_println_i64(int64_t)`, a bootstrap output service that writes the shortest locale-independent ASCII decimal representation and one LF byte to stdout. ABI version 3 adds `ska_rt_println_bool(bool)`, which writes lowercase ASCII `true` or `false` and one LF. Both operations use one internal checked record-writing boundary, flush the completed record before returning, and terminate the process unsuccessfully after a detected write or flush failure.

The runtime keeps C library implementation types such as `FILE *` private. Its public surface uses fixed-width integer types and standard C `bool`, and direct C consumers verify both header/archive version agreement and externally observable behavior. Later likely responsibilities include allocation, reference-count operations, panic reporting, runtime type metadata helpers, and other narrowly defined primitives. Garbage collection, root stacks, tracing, safepoints, and write barriers do not belong here.

### `grammar/`

Canonical grammar sources and parser-facing notes. The language grammar is not yet complete, so this directory initially records that status rather than pretending a partial grammar is normative.

### `std/`

Future Skald standard-library source. It is separate from the C runtime: functionality that can be expressed safely and efficiently in Skald should eventually live here.

### `tests/`

- `compiler/` contains phase-level compiler tests, cross-phase integration tests, and shared fixtures. Small Rust unit tests may also live beside their implementation, following Rust convention.
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

The implemented first-slice pipeline is:

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

### Source and diagnostics

All phases use a common span representation backed by the source database. Diagnostics contain structured severity, message, labels, and notes; terminal rendering happens at the driver edge. Tests should be able to assert diagnostic structure without depending on terminal colors or absolute paths.

M1 establishes UTF-8 byte offsets as the internal span unit and one-based Unicode-scalar line/column locations for display. Source IDs follow deterministic insertion order. The plain-text diagnostic renderer is color-free and stable for tests; richer terminal or editor renderers can consume the same diagnostic structures later.

The implemented M1 lexer returns a token stream and accumulated diagnostics together, preserving `Invalid` tokens for recovery instead of aborting on the first lexical error. Its deliberately narrow lexical contract is recorded in [`grammar/README.md`](../grammar/README.md).

### Syntax AST

The AST mirrors source constructs and preserves spans. It must not become the long-lived semantic representation. In particular, later phases should not repeatedly resolve strings or attach growing sets of optional semantic fields to parser nodes.

M2 implements the source AST as separate node, parser, and dump modules behind the public `syntax` boundary. O5 generalizes its source-ordered top-level declaration list to distinguish Skald definitions from bodyless `extern fn` declarations without attaching optional bodies to one oversized node. The recursive-descent parser performs no name or type lookup. It uses explicit precedence levels and recovers at parameter, statement, block, and top-level declaration boundaries, returning a partial AST together with accumulated structured diagnostics. The exact implemented grammar is recorded in [`grammar/README.md`](../grammar/README.md).

### Resolution and typed HIR

Resolution assigns stable IDs and establishes scopes before type checking. Typed HIR preserves enough source structure for good diagnostics but makes chosen operations and call targets explicit. A backend must never perform name lookup, overload selection, or language-level type checking.

M3 implements resolution as declaration collection followed by body resolution. Its separate resolved representation has a dense source-ordered function declaration table, a separately indexed definition table, owner-qualified parameter and local IDs, ID-based binding uses, and ID-based direct calls. O5 places external declarations in the same non-overloaded namespace and ID sequence, records their source identifier as exact-symbol linkage, and leaves their definition slot absent. Declarations own names, signatures, and linkage; definitions own locals and bodies. Public tables support lookup by ID but intentionally provide no name-based declaration-selection API. The `main` name is resolved once into an optional entry candidate; type checking rejects an external candidate and requires a defined `fn main() -> i64`.

Each function resolver owns an explicit lexical scope stack. Parameters share the outer function-body scope, nested blocks push scopes, and local initializers are resolved before their binding is introduced. Duplicate and lookup failures produce structured diagnostics with source labels. The precise first-slice rules are recorded in [`grammar/README.md`](../grammar/README.md).

M4 lowers successful resolved input into a distinct typed HIR. HIR preserves the declaration/definition split, so call checking consults canonical typed signatures without requiring a local body. Implemented semantic types are `i64`, `bool`, and payload-free `unit`; every HIR expression stores its type, primitive operators are explicit typed operations, and calls retain exact checked function IDs. O5 validates the deliberately restricted external ABI profile during this phase, and C2 extends it with by-value boolean parameters and results. Decimal spelling is converted here, including normalization of the signed `i64::MIN` boundary. Boolean literals remain distinct typed nodes rather than integer constants. Entry-signature, call-arity, expression, initializer, return-value, and mandatory-return checks accumulate diagnostics across the program. HIR is deliberately all-or-nothing: failed type checking returns diagnostics but no executable `HirProgram`, preventing M5 from consuming partial typed state.

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

M5 implements a three-address MIR with separate owner-qualified storage, transient value, and basic-block IDs. MIR has a dense callable declaration table containing canonical signatures and linkage, plus a sparse definition table containing executable bodies. Source parameters and locals map to explicit storage slots, while instruction results are immutable value IDs; this hybrid keeps mutation visible without committing the entire IR to SSA. Calls are effectful instructions with explicit stable targets, argument lists, and optional result IDs. Expression lowering emits instructions recursively in deterministic left-to-right order. The branch-free source subset still produces one entry block per defined function and omits unreachable statements after a return.

C3 adds target-independent `Goto` and boolean `Branch` terminators alongside
`Return`. The public MIR body builder allocates dense blocks in stable order,
switches the current insertion block explicitly, rejects instructions after a
terminator, and rejects duplicate termination. Terminators expose deterministic
successors, with a branch's true edge before its false edge. Transient values
are block-local in this non-SSA MIR; state crossing an edge must use explicit
storage.

The MIR verifier is a separate public boundary and checks declaration and definition associations, linkage/body consistency, external exact-symbol metadata, ID ownership, parameter order, definition/signature agreement, block-local use ordering, storage/value types, direct-call targets and result presence, return types, dense block IDs, entry blocks, boolean branch conditions, control-flow target ownership and existence, and terminators. It validates unreachable blocks as well as reachable ones. Lowering invokes it through a debug assertion, and focused tests deliberately corrupt valid MIR to cover rejection paths. Its stable textual dump exposes declarations separately from definitions and shows instructions, terminators, and stable control-flow targets in block-ID order.

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

System V integer-class parameters, including `bool`, arrive in `%rdi`, `%rsi`, `%rdx`, `%rcx`, `%r8`, and `%r9`, followed by stack arguments. Parameters are spilled into their frame homes on entry, and scalar results are returned in `%rax`; an external C boolean result is normalized from `%al` before storage. `unit` has no result payload and neither reads nor writes a fictitious return register. Calls reserve an independently 16-byte-aligned outgoing stack area when more than six arguments are present. The backend selects call symbols centrally from declaration linkage: internal definitions use deterministic GNU-local `.Lska_fn_N` symbols that cannot collide with valid exact external identifiers, while external declarations retain their declared symbol. Assembly-shape tests cover the register/stack ABI boundary, frame and scratch-register policy, call linkage, boolean normalization, all initial instructions, legality rejection, and acceptance by the system assembler.

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

Fast Rust unit tests should usually live beside the module under test. Larger compiler fixtures and cross-module tests belong under `tests/compiler/`.

### Runtime tests

Small C harnesses compile directly against the runtime archive. This isolates runtime behavior from compiler correctness and catches ABI mismatches early. The output harness redirects stdout to a temporary file and compares exact bytes across zero, signed values, both `i64` extrema, boolean false and true, and consecutive calls. Child-process checks close stdout and verify that neither bootstrap output operation can return successfully after a detected write failure.

### Golden tests

Golden cases exercise the complete public behavior. A case may specify expected diagnostics, assembly fragments, process exit status, or a combination. Test metadata should use repository-relative paths and avoid unstable absolute filenames or incidental temporary labels.

M7 provides a deliberately small Rust native runner. It discovers `.ska` files under `tests/golden/run/`, reads the expected process status from a matching `.exit` sidecar, builds the runtime archive, invokes the public `skac` binary, executes the result, and reports every case before returning failure. M8 extends the same runner with `tests/golden/compile_fail/` and exact `.stderr` snapshots. O1 adds optional `.stdout` sidecars with exact byte comparison; absence continues to require empty stdout, and runtime stderr remains empty. O6 supplies the end-to-end integer-output case. C2 adds an end-to-end boolean-output case covering literals, locals, parameters, and function returns, plus exact bool/i64 and entry-point failures. Successful assembly and failed diagnostics are each produced twice in independent compiler processes and compared for determinism. `make golden-test` runs this suite directly; it is also part of the workspace test suite.

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

It does need boundaries clean enough that those features can be added later without replacing the entire compiler. The completed first-slice scope and milestones are recorded in [FIRST_VERTICAL_SLICE_ROADMAP.md](FIRST_VERTICAL_SLICE_ROADMAP.md), and the extension contract for the next slice is listed in [NEXT_SLICE_BOUNDARIES.md](NEXT_SLICE_BOUNDARIES.md).

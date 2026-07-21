# Repository Structure and Compiler Architecture

This document describes the current Skald repository and the architectural
contract for extending it. Completed implementation plans are archived under
[`docs/archive/`](archive/README.md).

## Design principles

1. **Visible pipeline.** Compilation is a sequence of named phases with
   explicit inputs, outputs, and invariants.
2. **One authority per fact.** Syntax owns source shape, resolution owns name
   selection, HIR owns language types and selected operations, MIR owns
   executable order, and backends own target details.
3. **Forward dependencies.** Earlier phases never depend on later phases, and
   backends do not inspect AST or type-checker state.
4. **Stable identities.** Resolution assigns neutral typed IDs. Later phases do
   not repeat source-name lookup.
5. **Explicit state.** Sources, diagnostics, target selection, and compiler
   products belong to a compilation request rather than hidden globals.
6. **Determinism.** Diagnostics, IR dumps, table iteration, symbols, labels, and
   assembly are stable across processes.
7. **Verified boundaries.** Invalid source produces diagnostics; invalid IR is
   rejected close to the phase that produced it.
8. **Replaceable implementation.** Internal strategies may evolve behind
   documented phase interfaces.
9. **Small runtime.** Skald has no garbage collector. Facilities that can live
   in generated code or the future standard library stay out of the C runtime.
10. **Isolated targets.** ABI, layout, registers, frames, instructions, and
    assembly syntax stay behind backend interfaces.

Clarity and maintainability take priority over cleverness or premature
optimization.

## Relationship to Niflheim

The sibling Niflheim repository is a design and testing reference. It contains
useful experience in language behavior, diagnostics, runtimes, compiler passes,
and backends. Skald reuses lessons rather than implementation: Niflheim is not a
source dependency, architectural dependency, or normative specification.

## Top-level layout

```text
skald/
├── Cargo.toml
├── Makefile
├── README.md
├── crates/
│   ├── skac/
│   └── skald-compiler/
├── docs/
│   └── archive/
├── grammar/
├── runtime/
├── samples/
├── scripts/
├── std/
└── tests/
    ├── compiler/
    ├── golden/
    └── runtime/
```

### `crates/skac/`

The thin binary crate owns process-level command-line behavior and delegates
compilation to `skald-compiler`. It contains no lexer, parser, semantic, IR, or
target logic.

### `crates/skald-compiler/`

The Rust library contains the compiler pipeline:

```text
src/
├── lib.rs
├── source.rs
├── identity.rs
├── function_table.rs
├── dump_format.rs
├── diagnostics/
├── lexer/
├── syntax/
├── resolve/
├── hir/
├── typeck/
├── mir/
├── passes/
├── backend/
│   └── x86_64_sysv/
└── driver/
```

Large modules use directory-based private submodules with a concise `mod.rs`
facade. Phase-specific tests live beside their implementation and are split by
behavior when they become substantial. Shared test pipelines and mutation
helpers are compiled only under `cfg(test)`.

`dump_format` deliberately shares only low-level formatting primitives. Every
phase owns the vocabulary and structure of its own deterministic dump.

### `runtime/`

The C11 runtime builds `libskald_runtime.a` and exposes a versioned ABI. Its
current bootstrap output operations are:

```text
ska_rt_println_i64
ska_rt_println_u64
ska_rt_println_u8
ska_rt_println_f64_bits
ska_rt_println_bool
```

Integer operations print locale-independent decimal records; boolean output is
lowercase; floating output exposes exact binary64 bits. Every successful record
ends with LF and is flushed. A detected write or flush failure terminates the
process unsuccessfully.

Future runtime responsibilities may include allocation, reference counting,
panic support, and dynamic type metadata. Garbage collection, tracing roots,
safepoints, and write barriers do not belong here.

### Other directories

- `grammar/` documents the exact implemented source subset.
- `std/` is reserved for Skald-written standard-library code.
- `samples/` contains small demonstration programs, not regression fixtures.
- `scripts/` contains thin workflow wrappers; compiler behavior remains
  available through `skac` or library APIs.
- `tests/runtime/` directly tests the C ABI.
- `tests/golden/` tests complete source-to-native and source-to-diagnostic
  behavior.
- `docs/archive/` contains completed implementation roadmaps for historical
  reference.

## Compiler pipeline

```text
compilation request
    → source database
    → tokens
    → syntax AST
    → resolved program
    → typed HIR
    → target-independent MIR
    → verification/pass pipeline
    → target backend
    → target assembly model
    → textual assembly
    → system assembler/linker + C runtime
    → Linux executable
```

The driver composes the phases through `compile_source_to_assembly`; each phase
also remains independently testable.

| Phase | Output responsibility |
|---|---|
| Source | source IDs, text ownership, byte spans, line/column mapping |
| Lexer | token kinds, literal spelling boundaries, trivia policy |
| Parser | recoverable source-shaped AST without semantic lookup |
| Resolution | declarations, scopes, stable identities, selected names |
| Type checking | exact types, receiver access, selected operations, typed HIR |
| MIR lowering | explicit storage, values, places, calls, construction, control flow |
| MIR passes | target-independent verification and future transformations |
| Backend | legality, layout, ABI, frames, instruction selection, target model |
| Assembly emission | deterministic GNU assembly |
| Driver/toolchain | files, subprocesses, atomic artifacts, executable linkage |

### Source and diagnostics

All phases use `SourceId`, byte-based `Span`, and the source database. Human
locations are one-based and Unicode-scalar aware. Diagnostics retain severity,
code, labels, and notes until the driver renders stable plain text. Compiler
phases do not print errors directly.

The lexer returns tokens and accumulated diagnostics together. Invalid tokens
are retained for recovery. One numeric scanner classifies integer and floating
spellings without converting semantic values.

### Syntax

The recursive-descent parser is split by declaration, statement, expression,
and recovery responsibilities. It performs no name or type lookup. Source types
are parsed once, with callers choosing whether a context permits `unit` or a
named class type.

The parser represents parameter binding mode separately from source type.
Value, `ref`, and `mut ref` parameters share one parameter-list parser across
functions, external declarations, methods, and initializers. Modifier and type
spans remain source-shaped in the AST and deterministic dump.

One shared nesting budget limits recursive blocks, groups, unary expressions,
and postfix calls to 128 active levels. Excessive input produces a source
diagnostic and iterative declaration recovery rather than stack overflow.

### Resolution and identities

Neutral identities include functions, classes, fields, initializers, methods,
callables, parameters, locals, and bindings. `CallableId` identifies both a
top-level function and class-owned executable bodies, and owns that body's
local MIR identities.

Resolution collects top-level declarations before resolving bodies. Functions
and classes share one namespace; members remain class-owned. Resolved IR, HIR,
MIR, and backends carry IDs directly. Public lower-phase tables intentionally
offer no name-based selection API.

Resolved parameters carry value/read-only-alias/mutable-alias mode separately
from `ResolvedType`. Alias class names resolve through the same top-level class
namespace as local object types, and alias names receive ordinary
callable-owned `ParameterId` identities. Existing object-place resolution can
therefore select fields and methods through an alias binding while grouped
call arguments retain their source expression shape. Resolution deliberately
does not decide alias access or whether a call argument is a value or place.

Dense declaration tables and sparse optional-definition tables share private
validated storage utilities while retaining phase-specific public wrappers.
Member definitions use stable callable keys and deterministic ordering.

### Typed HIR

HIR contains exact semantic types, selected primitive operations, function and
member targets, receiver access, object/field places, and destination-oriented
construction. Failed type checking produces diagnostics but no executable HIR.

Callable bodies share one checking engine with explicit context for functions,
initializers, and methods. Type checking also computes `BlockFlow`, the
authoritative structured fallthrough/termination summary consumed by return
diagnostics and MIR lowering.

Numeric spelling is converted exactly once during type checking. Integer
families receive independent range checks; finite `f64` is converted to raw
binary64 bits. No backend infers types from spelling.

HIR parameter descriptors carry value/read-only-alias/mutable-alias mode
orthogonally to `Type`, and every callable signature query returns those same
descriptors. Calls and constructions retain one source-ordered argument list
whose entries are either typed scalar values or exact-class object places.

One `HirAccess` vocabulary describes read-only or mutable capability for
method receivers and alias places. Type checking derives it centrally for
locals, `self`, and alias parameters, permits mutable-to-read-only reduction,
and enforces field, method, forwarding, and non-escaping rules. HIR therefore
contains all source-level alias decisions needed by later phases.

### MIR

MIR is executable in shape but target-independent. It separates:

- addressable storage and projected places;
- block-local transient scalar values;
- direct calls and receiver-bearing method calls;
- initialization into a destination from ordinary assignment/store;
- basic blocks with `Return`, `Goto`, and boolean `Branch` terminators.

MIR signatures use ordered parameter descriptors that keep value,
read-only-alias, and mutable-alias modes separate from the underlying type.
Calls and initializations retain one ordered argument sequence containing
explicit value and place variants. Alias parameter homes are indirect place
bases, distinct from owning storage, and carry verifier-visible access.

Expression and argument lowering preserves the language's left-to-right order.
State crossing block edges uses storage because MIR is not currently SSA.
Objects occupy class-typed places and are never transient scalar `MirValue`s.
Field projections retain semantic `FieldId`s, not target offsets.

The verifier checks ID ownership and density, declaration/definition agreement,
storage and value types, definition-before-use, call signatures and receivers,
place projection typing, construction targets, block targets, branches,
returns, and termination—including unreachable blocks. Verification runs after
lowering, in the pass pipeline, and at the backend boundary.

The pass pipeline currently verifies without transforming. SSA conversion or
optimization should enter as an explicit pass or replaceable IR boundary when
concrete optimization work justifies it.

The verifier checks alias mode/type agreement, parameter storage, argument
kind, place ownership and projection, access sufficiency, read-only writes and
mutable receiver calls, and the external-alias exclusion before backend
lowering. Dumps expose modes, indirect bases, and argument kinds without
target offsets or registers. These contracts are defined in the
[alias-parameter implementation profile](SKALD_DRAFT_SPEC.md#543-restricted-stage-0-alias-parameter-profile),
and are covered by hand-built MIR tests. Source HIR-to-MIR alias lowering and
the pointer ABI remain future roadmap steps; the driver reports that precise
capability boundary for alias-bearing source today.

## x86-64 System V backend

The backend separates:

- target legality;
- primitive and class data layout;
- System V argument/result classification;
- frame layout;
- MIR-to-machine lowering;
- typed assembly representation;
- GNU assembly emission;
- identity-derived symbols.

Class fields are laid out in declaration order with checked size/alignment
arithmetic. `i64`, `u64`, and `f64` use 8-byte size/alignment; `u8` and `bool`
use 1 byte. Empty classes remain addressable with size/alignment one. Object
locals receive aligned contiguous frame storage, and projected places are
resolved to addresses only in the backend.

Initializers and methods receive the object address as a hidden first integer-
class argument. Integer and SSE arguments use independent register sequences;
overflow arguments share source-ordered stack slots. The current lowering is
intentionally stack-heavy and can later be replaced by register allocation
without changing MIR.

The planned alias ABI reuses the indirect-address mechanics without treating
an alias as an implicit receiver. Each alias is one integer-class pointer in
source parameter order, stored in a pointer-sized callee home and dereferenced
before field projection. `ref` and `mut ref` have identical machine
representations. The slice adds no object copy, allocation, retain/release,
borrow anchor, or external object ABI.

Internal symbols derive from stable identities. External declarations retain
their exact source symbol. The generated C-compatible `main` wrapper calls the
ID-selected Skald entry function and exposes its low result bits as the Linux
process status.

## Driver and artifacts

`skac` supports executable output and `--emit asm`. Executable mode streams
assembly to the configured C compiler driver and links the runtime archive. It
does not construct a shell command.

Assembly and executable publication use same-directory temporary files and one
final rename. Failures preserve existing output and clean unpublished
temporaries through RAII. Output paths that alias the input through the same
path, a symbolic link, or a hard link are rejected.

## Testing

The repository uses four complementary layers:

1. colocated Rust unit tests for phase behavior and invariants;
2. exact AST/resolved/HIR/MIR/assembly dumps;
3. C runtime contract, successful-output, and fatal-output tests;
4. golden source programs for native behavior and exact diagnostics.

Golden programs are compiled in independent processes to compare assembly or
diagnostics byte-for-byte. Native cases separately check stdout, stderr, and
process status. Inline-object integration additionally compares AST, resolved
IR, HIR, MIR, and assembly across processes.

The root commands are:

| Command | Purpose |
|---|---|
| `make fmt` | format Rust source |
| `make fmt-check` | verify formatting |
| `make build-check` | check all Rust targets |
| `make lint` | run Clippy with warnings denied |
| `make compiler-test` | run workspace tests |
| `make golden-test` | run native and compile-failure goldens |
| `make runtime` | build the runtime archive |
| `make runtime-test` | run direct runtime tests |
| `make check` | run the complete validation suite |

## Extension policy

Future language work must preserve name-independent lower phases, target-
independent MIR, structural verification, deterministic artifacts, and the
runtime/backend boundary. Planned feature ordering and unresolved constraints
are described in [Future Development Boundaries](NEXT_SLICE_BOUNDARIES.md).

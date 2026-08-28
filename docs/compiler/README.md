# Compiler Architecture

Status: authoritative for durable compiler responsibilities, repository roles,
extension policy, and the repository-internal compiler crate API. Detailed
phase and IR contracts are owned by [Phases and IR](PHASES_AND_IR.md).

Skald uses a visible, forward-moving compiler pipeline. Each phase owns one
kind of decision, exposes an inspectable product, and passes stable identities
forward instead of asking later phases to repeat source analysis.

## Architecture principles

1. **One owner per decision.** Syntax owns source shape, resolution owns name
   selection, typed HIR owns checked language operations, MIR owns executable
   order, and backends own target realization.
2. **Forward dependencies.** Earlier phases do not depend on later phases.
   Backends consume MIR rather than AST, resolved IR, or type-checker state.
3. **Explicit request state.** Sources, diagnostics, targets, phase products,
   and artifacts belong to a compilation request rather than hidden globals.
4. **Name-independent lower phases.** Resolution assigns typed identities;
   lower phases preserve them without source-name lookup.
5. **Verified trust boundaries.** Source mistakes remain structured
   diagnostics. Invalid MIR is rejected before target lowering.
6. **Deterministic products.** Stable ordering and phase-owned renderers make
   diagnostics, dumps, and generated artifacts reproducible.
7. **Replaceable internals.** Private algorithms and file organization may
   evolve behind small phase facades.
8. **Isolated platform concerns.** Layout, ABI, registers, frames, machine
   instructions, runtime linkage, and host tools stay outside language and
   target-independent phase contracts.

Clarity and maintainability take priority over cleverness or premature
optimization.

## Repository roles

- `crates/skac` is the thin process entry point. It delegates command-line and
  compilation behavior to the compiler library.
- `crates/skald-compiler` owns sources, diagnostics, phases, target selection,
  backend dispatch, and driver orchestration.
- `crates/skald-docs-check` validates the repository documentation structure;
  it is tooling, not a compiler phase.
- `crates/skald-golden` owns strict golden-spec decoding, contained fixture
  resolution, immutable plan expansion, selection, read-only inspection,
  exact-byte expectations, deterministic dependency scheduling, and isolated
  compiler, linker, and native child processes under one bounded worker pool.
  It reuses
  the repository-internal compiler driver facade for linkage but remains
  repository tooling; production compiler crates do not depend on it.
- `runtime/` provides the separately built C runtime archive behind a
  versioned ABI.
- `tests/` contains reusable non-Rust corpora, end-to-end goldens, and direct
  runtime harnesses. Rust unit and integration tests remain with their owning
  crate.
- `docs/language/` owns source-visible semantics. Compiler documentation links
  there instead of restating language rules.

Target legality, layout, calling conventions, and code generation are defined
by the [backend and target contract](BACKEND.md). The public runtime C surface
and compiler/runtime compatibility mechanism are defined by the
[runtime ABI](RUNTIME_ABI.md). The implemented ownership, ordinary/copy-allocation,
header, finalizer, anchor, and compiler/runtime responsibility design is
defined by the
[shared-ownership compiler and runtime contract](SHARED_OWNERSHIP.md). Driver
behavior is defined by
[driver and artifacts](DRIVER_AND_ARTIFACTS.md). The frozen multiple-file
request, provider, loading, identity, and linkage design is defined by the
[module-system compiler contract](MODULE_SYSTEM.md). Typed logical paths,
request-local module/provider/package identities, provenance records, and the
driver request model are implemented foundations. Deterministic filesystem
provider normalization and exact candidate lookup are available behind the
`module` facade. The facade also provides positional/logical entry selection,
reachable parsed graph loading, deterministic graph identities and dumps, and
multi-module cycles with focused direct self-import rejection. The resolver
can collect a loaded graph into deterministic
per-module declaration indexes and one flat whole-program IR; unqualified
uses see owned declarations plus explicit selective ordinary bindings. Direct
module imports create exact default or aliased qualified bindings, and
selective imports bind only requested directly owned public declarations;
both resolve once to existing semantic identities without re-exporting.
Declaration indexes and bindings distinguish ordinary classes and interfaces
from class and interface templates; interface-template parameters and
requirements have stable non-executable identities without consuming ordinary
interface IDs.
Definition-site generic-interface resolution publishes a separate inspectable
semantic table containing structural requirement signatures, parameter modes,
bounds, type uses, exact source origins, and deferred contextual capabilities.
It never inserts parameterized terms into ordinary resolved type tables.
Compatible cross-module external declarations retain separate `FunctionId`
values while sharing one verified, symbol-owning external-link identity.
The request pipeline and CLI compile positional or logical entries with
anonymous roots and replaceable or disabled standard-library lookup. The
in-memory single-file adapter continues to reject imports. Test
ownership and selection are defined by
[Testing](../development/TESTING.md), and inspection workflows by
[Debugging the Compiler](../development/DEBUGGING.md). Contributor
prerequisites and validation are defined by the
[development workflow](../development/README.md).

The implemented
[structural indexing and slicing compiler contract](INDEXING_AND_SLICING.md)
uses neutral source-only bracket vocabulary. Resolution retains built-in arrays
and normalizes eligible class and interface index and slice brackets to
ordinary direct, virtual, or interface calls before HIR. The implemented
selection and ownership matrix add no lower IR, backend operation, or runtime
ABI.

The implemented [function-value compiler contract](FUNCTION_VALUES.md) assigns
canonical closed function-type identity, exact callable references, explicit
HIR and MIR indirect calls, verification, conservative address-taken effects,
one-word x86-64 realization, and the unchanged runtime boundary to their
owning phases. Function types, eligible ordinary callable references, generic
reference closure, trivial stored/callable HIR, complete indirect-call HIR
argument/result planning, callable-address MIR, receiverless indirect MIR
targets, target-independent verification, and one-word x86-64 realization are
implemented. Native calls reuse the complete ordinary internal ABI and exact
identity-derived callable symbols. Static-lifecycle analysis expands each
indirect call to its deterministic exact-signature address-taken set, verifies
that retention inventory in the lifecycle certificate, and preserves exact
source-callable runtime traces. Complete source-to-native composition and
negative conformance are covered by the golden suite; the
[implementation roadmap](../archive/FUNCTION_VALUES_ROADMAP.md) records the
rollout.

The implemented [strings compiler contract](STRINGS.md) defines canonical
`std::str::Str` discovery and validation, intrinsic produced-value lowering,
verified immortal shared-array backing, deterministic literal data, and the
compiler/standard-library/runtime boundary. Literal syntax, conditional
`std::str` reachability, exact validation, typed HIR production, verified
target-independent descriptor materialization, deterministic x86-64 backing
emission, and ordinary literal lifecycle are compiler phase products.
Copying construction, observation, slicing, byte-array conversion, and
concatenation remain ordinary source in the canonical standard-library
module. Its invalid byte and slice bounds call the canonical panic intrinsic
through the ordinary `std::str` and `std::error` import cycle.

Integer division and remainder, floating division, bitwise operations, checked
shifts, integer and floating comparisons, primitive casts, eager boolean
operators, and short-circuit boolean expressions are complete source-to-native
phase products. Type checking selects their exact operand and result kinds;
HIR and MIR retain semantic
operations and explicit checked control flow where required, verification
checks those contracts, and x86-64 realizes them without a new runtime ABI.
The detailed ownership boundary is documented in
[Phases and IR](PHASES_AND_IR.md#implemented-primitive-operator-boundary).

The complete implemented primitive operator profile has separately documented
[phase and IR boundary](PHASES_AND_IR.md#implemented-primitive-operator-representation),
[target boundary](BACKEND.md#implemented-primitive-operator-target-boundary), and
[unchanged runtime ABI boundary](RUNTIME_ABI.md#implemented-primitive-operator-abi-boundary).
Those contracts select exact typed operations, structured short-circuit HIR,
ordinary MIR CFG with canonical result carriage, path-correct full-expression
cleanup, verified compiler-known failures, and mechanical target realization.
They do not claim support for the explicitly deferred operator and conversion
work in the [status matrix](../language/STATUS.md#not-implemented).

The staged [operator-protocol lowering contract](OPERATOR_OVERLOADING.md)
extends operators without adding a runtime mechanism. Its dependency-free
`std::ops` source, complete reachable-bundle validation, exact resolved
identity table, complete non-generic class selection, and closed primitive
bound-evidence registry are implemented.
Selected class uses erase before completed HIR to ordinary interface calls;
overloaded `!=` negates one secured equality call, while canonical
primitive-bound uses erase to existing primitive operations, and produced
primitive read-only arguments use the implemented caller-owned scalar
temporaries. Ordinary receiver carriers, result owners, dispatch, evaluation,
cleanup, panic traces, static effects, and target retention are shared without
operator-specific lower IR. Generic-bound punctuation remains staged. The
remaining operator design adds no overloaded-operator MIR node, backend
semantic lookup, runtime service, or ABI revision and is not implemented yet.

The frozen
[complete explicit primitive cast matrix](../language/TYPES_AND_VALUES.md#frozen-complete-explicit-primitive-cast-matrix)
has separately selected
[phase and IR](PHASES_AND_IR.md#frozen-complete-primitive-cast-representation),
[target](BACKEND.md#frozen-complete-primitive-cast-target-boundary), and
[unchanged runtime ABI](RUNTIME_ABI.md#frozen-complete-primitive-cast-abi-boundary)
boundaries. The archived
[implementation roadmap](../archive/PRIMITIVE_CAST_MATRIX_ROADMAP.md) records
the completed migration to a cohesive primitive-cast vocabulary. All
twenty-five cells are accepted from source and execute inline on x86-64. The
twenty-two non-failing cells use pure MIR; the three checked
`f64`-to-integer cells lower to explicit, verified success/failure control
flow.

The implemented optional representation, IR, verification, x86-64 layout,
checked-view, and internal calling-convention decisions are owned by the
[optional-values compiler contract](OPTIONAL_VALUES.md). Recursive source type
syntax normalizes canonical `(shared T)?` and `shared? T` shorthand through
one deterministic recursive resolved identity table. Primitive and exact-class optional locals, fields,
lifecycle, internal callable boundaries, optional shared owners, and inline
optional-container aliases execute through typed HIR and verified MIR. This
includes bounded checked class payload views, dynamic presence guards,
zero-niche optional owners, and exact virtual/interface signatures.

That same contract now carries canonical optional identities and recursive
lifecycle plans through generalized executable MIR and x86-64 realization.
Distinct scalar, aggregate, and shared-owner operations remain only where the
runtime work differs. Nested payload access, aliases, and internal callable
boundaries use those recursive plans. Tagged optional arrays reuse ordinary
array lifecycle across all supported aggregate, internal callable,
array-element, and checked-alias positions. Shared optional boxes carry
resolved, HIR, and MIR targets for `shared T?`; construction, owner transfer,
replacement, cleanup, arrays, internal calls, and static lifecycle reach
verified target-independent MIR and execute on x86-64 with deterministic exact
descriptors and finalizers. This adds no C runtime ABI surface; external
optional signatures remain unsupported by the existing C ABI.

The frozen [generic-class specialization contract](GENERIC_CLASSES.md) adds a
template and closed-specialization layer before ordinary resolved classes.
Every accepted closed application receives one exact `ClassId` and reuses
existing optional, array, shared-owner, containment, lifecycle, HIR, MIR, and
backend machinery. No unresolved parameter reaches ordinary resolved classes
or lower IR, and no runtime generic ABI is introduced. Template semantics,
contextual requirements, canonical closed keys, deterministic reserved class
identities, caching, finite-recursion handling, and complete ordinary closed
declarations and resolved bodies are implemented; lifecycle and later phases
use the existing closed HIR, verified MIR, backend, and native paths.

The implemented
[generic-interface specialization contract](GENERIC_INTERFACES.md) extends
that boundary with interface-template and template-requirement identities,
parameter-bearing claims and bounds, coordinated class/interface
specialization, and exact closed interface identities. Successful closure
reuses ordinary conformance, views, witness calls, HIR, verified MIR, and
backend metadata without runtime dictionaries or an ABI change. The
[conformance matrix](GENERIC_INTERFACES_TEST_MATRIX.md) maps the complete
contract and its exclusions to executable evidence.

The implemented [general-iteration compiler contract](ITERATION.md) selects one
canonical closed `std::iter::Iterable<Item, State>` application, retains a
structured source and HIR loop with a loop-duration receiver plan, and lowers
it to ordinary interface calls, optional operations, ownership cleanup, and
cyclic MIR. It deliberately adds no iterator MIR operation, backend primitive,
runtime service, or ABI revision. The canonical dependency-free source module,
typed module-dependency evidence, structural template validation, exact
resolved language-item identities, structured source syntax, implicit
dependency activation, deterministic syntax dumps, nominal protocol selection,
exact resolved iteration evidence, definition-site generic-bound selection,
item/loop scopes, structured HIR, lifecycle planning, ordinary-MIR lowering,
verification, and native execution are implemented.

The compiler implements the recursive array source surface, canonical
exact identities, typed HIR operations, and verified target-independent MIR.
The x86-64 target executes inline and shared-outer arrays containing
primitives, optionals, exact classes, recursively nested inline arrays, and
ordinary or optional shared owners of exact classes and arrays. Construction,
length, checked element access, named deep copy, produced-backing adoption,
arbitrary-length replacement, class fields, internal owning calls/results,
secure shared-element replacement, and deterministic element cleanup execute.
Copied slices and checked equal-length slice assignment execute with
negative-relative bounds and overlapping-write snapshot semantics. Call-scoped
whole-array and exact element aliases execute with detached-backing and
shared-owner anchors. The implemented contract is owned by the
[array compiler and runtime contract](ARRAYS.md).

That contract also implements typed explicit `T[]{...}` and `new T[]{...}`
element-list construction. The feature preserves ordered destination
initialization, unpublished-prefix
verification, existing class/optional/nested-array/shared-owner operations,
and the current runtime ABI. Syntax and resolution retain those brace forms
exactly, and type checking now emits one ordered, destination-directed HIR
plan per element. Every legal stored element plan executes through verified
MIR and x86-64 for both outer ownership modes; exact classes reuse ordinary final-destination
initializer, call-result, copy-construction, full-expression, and
reverse-destruction machinery, optionals reuse ordinary absence, injection,
payload, publication, and conditional cleanup, nested arrays reuse exact
recursive deep copy or produced-backing adoption, and shared-owner families
reuse ordinary retain/adopt/release and zero-niche optional operations.
Availability remains authoritative in the [status matrix](../language/STATUS.md).

Class-owned static fields pass through delayed initializer resolution,
stored-value HIR, structurally verified preliminary MIR, exhaustive
whole-program effect inference, deterministic lifetime planning, and an
independently verified final lifecycle certificate. The x86-64 backend emits
private slots and dependency-ordered initializer/finalizer coordinators around
entry without changing runtime ABI version 9. The authoritative boundaries are
[Static Fields](../language/STATIC_FIELDS.md) and
[Compiler Phases and Intermediate Representations](PHASES_AND_IR.md#pipeline-contract).

The [standard I/O compiler and runtime contract](IO.md) defines the implemented
five-intrinsic boundary over `u8[]`, dedicated HIR/MIR operations, x86-64
pointer/length lowering, and runtime ABI version 9. It deliberately leaves
buffering, completion loops, `Str` conversion, and public failures in Skald
standard-library code. Runtime ABI version 9 implements the independently
tested host byte operations. The closed intrinsic registry and dedicated HIR
are implemented together with semantic MIR operations, checked range offsets,
backing-anchor verification, and exact scalar results. The x86-64 target forms
checked byte pointer/remaining-length pairs and calls the exact version-9
runtime operations. The complete nine-function public surface is implemented
in Skald, including primitive line output, partial-write completion, and
growable read-until-EOF loops.

## Pipeline

```text
source database
    -> tokens
    -> syntax AST
    -> generic templates and closed resolved specialization
    -> resolved program
    -> typed HIR
    -> target-independent MIR
    -> verification and MIR passes
    -> selected backend
    -> assembly
```

The current driver composes this path for one source file. The frozen module
contract extends the source and resolution front of the same pipeline to a
reachable graph; it does not introduce a second semantic linker or a separate
lower pipeline. Every phase entry point and product also remains independently
usable by repository tests and debugging tools. Compiler source-file I/O, host tool
invocation, runtime linkage, and artifact publication are driver
responsibilities outside the phase pipeline.

See [Phases and IR](PHASES_AND_IR.md) for inputs, outputs, invariants,
verification, dumps, and trust boundaries, and
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md) for orchestration and output
behavior.

The
[private cell field representation](PHASES_AND_IR.md#private-cell-field-representation)
carries declaration metadata, typed whole-field write authorization, and
independently verified per-instruction MIR evidence without upgrading receiver
access. Ordinary backend assignment machinery executes verified writes. The
same representation composes with ordinary lifecycle and alias protections,
closed specialization, inheritance, virtual/interface dispatch, and eligible
capture-free function values.

The implemented
[final field representation](PHASES_AND_IR.md#final-field-representation)
preserves contextual declaration evidence through resolved IR, closed
specialization, typed HIR, and verified MIR without changing ordinary
declaration or layout identity. Final instance fields use ordinary construction,
copy construction, complete-value assignment, read, destruction, layout, and
backend paths. Type checking rejects independent slot replacement; exact user
assignment writes and synthesized assignment plans carry independently verified
final-update evidence. Final statics retain explicit publication evidence
through the certified eager lifecycle, reject source root replacement, and use
ordinary backend slots and reverse shutdown. Standard-library primitive boxes
use the same representation for public payload fields without a compiler
exception.

## Compiler crate API policy

`skald-compiler` is unpublished and repository-internal. Its public Rust API
supports the workspace binary, integration tests, and debugging tools; it is
not a compatibility promise across compiler revisions.

The crate exposes responsibility-oriented facades:

| Facade | Public responsibility |
|---|---|
| `source`, `diagnostics` | source ownership, spans, structured diagnostics, rendering |
| `lexer`, `syntax` | phase entry points, products, diagnostic codes, deterministic dumps |
| `resolve`, `typeck`, `hir` | semantic phase entry points, products, typed identities, deterministic dumps |
| `mir`, `passes` | MIR schema, lowering, verification, pass sequencing, deterministic dumps |
| `identity`, `literal` | shared target-independent identity and source-literal vocabulary |
| `module` | validated logical module paths and request-local module provenance |
| `backend` | target registry and assembly-emission boundary |
| `driver` | typed compilation requests, complete one-source compilation, and command-line orchestration |

These namespaces, rather than private source files, are the supported way for
repository consumers to cross a compiler boundary. Facades use explicit
re-exports; implementation modules, state machines, table storage, builders,
and target internals remain private.

Public phase-product fields allow inspection and phase-specific debugging.
They do not make arbitrary constructed or mutated AST, resolved IR, HIR, or
MIR valid. A product is trusted to satisfy its producer's invariants; MIR is
the exception with an explicit public verifier because it is the backend trust
boundary.

Test-only pipeline helpers, malformed-IR fixtures, and mutation hooks are
compiled only for crate tests and remain crate-visible. Tests that need only
the intentional public surface belong in the crate integration-test directory.

## Extension policy

A substantial compiler or language extension should:

1. settle source-visible behavior in the focused language authority;
2. update the grammar or state explicitly that syntax is unchanged;
3. assign identities during resolution rather than adding lower-phase name
   lookup;
4. make checked types, access, and selected operations explicit in HIR;
5. make storage, evaluation, ownership operations, and control flow explicit
   in MIR;
6. extend MIR verification before a backend relies on the new representation;
7. keep target layout and ABI decisions out of target-independent IR;
8. make every backend either support new MIR or reject it structurally;
9. add focused phase tests, deterministic dumps, source diagnostics, and
   end-to-end coverage in the layer that owns each guarantee; and
10. update living documentation with the behavior and archive the completed
    implementation roadmap.

Optimization must preserve correctness rather than establish it. New
transformations belong in an explicit named pass pipeline. SSA or another IR
boundary should be introduced only when concrete optimization work justifies
its maintenance cost.

New targets belong behind the backend boundary. New runtime services must use
the versioned runtime boundary instead of leaking host assumptions into
target-independent phases. Multiple-file compilation follows the implemented
[source-visible module contract](../language/MODULES_AND_INTEROP.md#initial-module-system)
and [compiler module contract](MODULE_SYSTEM.md) without redefining them in an
implementation roadmap.

Plain checked-place casts and their owning inline copy consumers are
implemented. The complete source-visible matrix is defined in
[Object Casts](../language/OBJECT_CASTS.md); compiler phases must consume that
authority rather than infer ownership from cast syntax.

Ordinary direct and base-initializer overload selection belongs to type
checking. Per-overload initializer visibility is checked only after unique
selection against the callable's lexical class owner, then erased before HIR.
Copy construction has a distinct lifecycle identity. The explicit
copy-construction source mode is carried separately from ordinary arguments
through syntax, resolution, HIR, and MIR. Its phase boundary is defined in
[Compiler Phases and Intermediate Representations](PHASES_AND_IR.md).

Feature maturity and implementation order belong in the
[language status matrix](../language/STATUS.md) and
[active roadmap index](../roadmaps/README.md), not in compiler architecture.

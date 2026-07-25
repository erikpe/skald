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
[driver and artifacts](DRIVER_AND_ARTIFACTS.md). Test ownership and selection
are defined by [Testing](../development/TESTING.md), and inspection workflows
by [Debugging the Compiler](../development/DEBUGGING.md). Contributor
prerequisites and validation are defined by the
[development workflow](../development/README.md).

The frozen optional representation, IR, verification, x86-64 layout,
checked-view, and internal calling-convention decisions are owned by the
[optional-values compiler contract](OPTIONAL_VALUES.md). Syntax and flat
resolved identities are implemented; primitive optional locals, fields, and
internal callable boundaries also execute through typed HIR and verified MIR.
Current phase and backend descriptions remain authoritative while
resource-bearing optional payload work is still planned.

## Pipeline

```text
source database
    -> tokens
    -> syntax AST
    -> resolved program
    -> typed HIR
    -> target-independent MIR
    -> verification and MIR passes
    -> selected backend
    -> assembly
```

The driver composes this path for one source file. Every phase entry point and
product also remains independently usable by repository tests and debugging
tools. Source I/O, host tool invocation, runtime linkage, and artifact
publication are driver responsibilities outside the phase pipeline.

See [Phases and IR](PHASES_AND_IR.md) for inputs, outputs, invariants,
verification, dumps, and trust boundaries, and
[Driver and Artifacts](DRIVER_AND_ARTIFACTS.md) for orchestration and output
behavior.

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
| `backend` | target registry and assembly-emission boundary |
| `driver` | complete compilation and command-line orchestration |

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
target-independent phases. Multiple-file compilation must first settle the
open language and build contracts in
[Modules and Foreign Interoperation](../language/MODULES_AND_INTEROP.md).

Plain checked-place casts and their owning inline copy consumers are
implemented. The complete source-visible matrix is defined in
[Object Casts](../language/OBJECT_CASTS.md); compiler phases must consume that
authority rather than infer ownership from cast syntax.

Ordinary direct and base-initializer overload selection belongs to type
checking, and copy construction has a distinct lifecycle identity. The
explicit copy-construction source mode is carried separately from ordinary
arguments through syntax, resolution, HIR, and MIR. Its phase boundary is
defined in
[Compiler Phases and Intermediate Representations](PHASES_AND_IR.md).

Feature maturity and implementation order belong in the
[language status matrix](../language/STATUS.md) and
[active roadmap index](../roadmaps/README.md), not in compiler architecture.

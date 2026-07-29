# While Loops and Loop Exits Roadmap

Status: in progress; L0 through L4 are complete and L5 is next.

This roadmap adds executable `while` statements and then the already-designed
`break` and `continue` statements without making source acceptance the
experiment that discovers whether Skald's lifetime model supports cycles. The
durable result is a generic cyclic MIR foundation: static storage identities
can have repeated dynamic lifetime epochs, verifier analyses converge across
backedges, HIR retains structured loop effects, and backends continue to lower
ordinary verified control-flow edges mechanically.

The frozen source and representation contracts live in
[Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits),
the [grammar](../language/GRAMMAR.md#while-loops-and-reserved-loop-exits), and
[Compiler Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md#while-loop-representation).
The historical decision rationale is retained in the archived
[while-loop design proposal](../archive/WHILE_LOOPS_DESIGN_PROPOSAL.md).

## Scope and invariants

This roadmap includes:

- explicit repeatable live/dead lifetime epochs for static MIR storage;
- finite, deterministic verification of cyclic generic control flow;
- stable callable-local loop identities and composable HIR control effects;
- cleanup planning to an explicit retained lexical depth;
- structured HIR `while` statements lowered to ordinary MIR branches and
  jumps;
- the canonical initial preheader, header, body, latch, and exit shape;
- source activation of `while`, followed by separate `break` and `continue`
  slices;
- exact-`bool` conditions, per-attempt condition cleanup, and fresh body
  lifetimes on every entered iteration;
- correct behavior for every currently implemented local, temporary,
  ownership, optional, shared, array, view, and cleanup family;
- deterministic dumps, diagnostics, native behavior, and verifier failures;
  and
- transformation invariants that later CFG optimizations must preserve.

The following invariants apply throughout:

1. Source loop legality and definite-return behavior do not depend on constant
   folding, reachability analysis, or another optimization.
2. Every `while` conservatively permits fallthrough, including
   `while (true)`.
3. A condition is evaluated exactly once before each attempted iteration, and
   all of its full-expression state ends before either successor begins.
4. Each entered body iteration starts fresh dynamic lifetime epochs for its
   body-local storage and ends them on every edge that leaves the applicable
   scope.
5. Enclosing storage remains live across the loop.
6. HIR keeps loops and targeted loop effects structured. MIR uses generic
   target-independent blocks, branches, and jumps rather than a source-loop
   terminator.
7. `break` and `continue` resolve to stable `LoopId`s before type checking;
   lowering never rediscovers their target from source nesting.
8. Cleanup is attached to the control-flow edge that leaves scopes and is
   planned to a retained lexical depth. It is never inferred by a backend.
9. MIR verification is valid for cycles before source syntax can construct
   one and accepts a backedge only when its lifetime and ownership state is
   compatible with the target.
10. Lifetime markers have no runtime effect. The runtime ABI remains unchanged
    and backends do not acquire loop semantics.
11. MIR pass correctness is defined by verified generic CFG and lifetime
    invariants, not by preserving the initial block layout.
12. New substantial algorithms and state machines receive cohesive private
    modules; existing `mod.rs` files remain facades with narrow explicit
    re-exports.

Explicitly excluded from this roadmap:

- value-producing loops or value-carrying `break`;
- labels, multi-level exits, and user-visible loop identities;
- `for`, `do`, infinite-loop, iterator, or pattern-loop syntax;
- declarations in the `while` header;
- truthiness or implicit conversion to `bool`;
- a proof that literal-true loops diverge or satisfy definite return;
- exceptions, unwinding, or cleanup pads;
- SSA, phi nodes, or a requirement to add a loop optimization;
- a dedicated MIR loop instruction or backend loop stack; and
- a runtime service or runtime ABI version change.

## Progress

- [x] L0 — Add repeatable MIR storage lifetime epochs
- [x] L1 — Make MIR verification cycle-safe
- [x] L2 — Add structured loop identities and control effects
- [x] L3 — Lower internal HIR `while` loops through generic MIR
- [x] L4 — Activate source `while` end to end
- [ ] L5 — Add targeted `break` statements
- [ ] L6 — Add targeted `continue` statements
- [ ] L7 — Harden loop lifecycles and optimization boundaries

A task is complete only when its checklist, focused tests, exit criteria, and
applicable repository gates pass. Each Rust task runs focused owner tests
during development, then `make check` and `make msrv-check`. Documentation-only
changes run `make docs-check`. The Makefile remains the local and external
automation interface; this roadmap does not add repository CI.

## PR-sized implementation sequence

### L0 — Add repeatable MIR storage lifetime epochs

**Purpose:** Give a static MIR storage identity explicit dynamic lifetime
boundaries before any generic backedge can execute its declaration twice.

- [x] Add target-independent live/dead lifetime operations, equivalent to
      `StorageLive(storage)` and `StorageDead(storage)`, with stable storage
      identity and source spans.
- [x] Define entry-state treatment for receivers and parameters and explicit
      epoch boundaries for source locals, return storage, spills, temporaries,
      checked views, shared allocations and anchors, optional unwraps, and
      array-owned compiler storage.
- [x] Migrate HIR-to-MIR lowering so every currently emitted storage use occurs
      in a live epoch and every normal lifetime end emits cleanup before the
      corresponding dead boundary.
- [x] Keep lexical-scope epochs distinct from full-expression epochs without
      changing source evaluation or cleanup order.
- [x] Add structural MIR checks for invalid storage IDs, use while dead,
      duplicate live, duplicate dead, cleanup after dead, and a live local left
      on an ordinary function exit.
- [x] Teach MIR dumps to display lifetime boundaries deterministically.
- [x] Make the x86-64 backend validate and otherwise emit no code for lifetime
      operations; frame allocation remains based on static storage identities.
- [x] Keep lifetime modeling in cohesive MIR model, lowering, verification,
      and dump owners rather than growing their facade modules.
- [x] Update the phase/IR document only if implementation reveals a
      representation invariant not already covered by the frozen contract.

**Tests:** MIR builder and dump tests; straight-line and diamond-shaped
hand-built MIR; verifier mutations for every invalid epoch transition; focused
lowering tests covering primitive locals, inline owners, shared and optional
owners, arrays, views, anchors, and full-expression temporaries; backend
assembly tests proving the markers emit no instructions; all existing compiler
and golden tests; `make check`; `make msrv-check`.

**Exit criteria:** Every MIR body produced for the currently implemented
language has explicit, verified storage lifetime epochs, invalid straight-line
epoch use is rejected deterministically, native behavior and the runtime ABI
are unchanged, and no source loop syntax is accepted.

### L1 — Make MIR verification cycle-safe

**Purpose:** Establish finite verifier behavior and stable backedge state before
source or HIR lowering can introduce generic cyclic control flow.

- [x] Audit every verifier domain that carries state across CFG edges,
      including object initialization and cleanup, shared ownership and
      release, optional initialization and guards, arrays and anchors, checked
      views, and full-expression state.
- [x] Give each state domain a finite merge and convergence rule over cyclic
      CFGs; share a small worklist abstraction only where repeated ownership
      and behavior justify it.
- [x] Reset per-lifetime facts at live/dead epoch boundaries so re-entering a
      body can initialize, use, clean, and end the same static storage identity
      again.
- [x] Require compatible state at ordinary joins and backedges, including
      agreement that no condition temporary or body-local lifetime leaks onto
      the next attempted iteration.
- [x] Verify every represented block, including unreachable cyclic components,
      without treating optimization or dead-block removal as a correctness
      prerequisite.
- [x] Bound diagnostic emission so malformed cycles terminate verification and
      report deterministic, non-duplicated errors.
- [x] Preserve verification at the MIR pass boundary and confirm pass
      round-trips do not assume an acyclic block order.
- [x] Extract large verifier state machines into descriptive private modules
      and keep the verifier facade focused on orchestration.

**Tests:** Hand-built generic self-loops and multi-block cycles; a valid
live/initialize/use/cleanup/dead iteration; mutations with a live value crossing
the backedge, double initialization in one epoch, missing cleanup, released
shared owners, inconsistent optionals, active guards or views, live anchors,
and disagreeing array ownership; unreachable cycles; deterministic error-order
tests; pass-pipeline preservation; generative robustness cases with a bounded
cyclic CFG; `make check`; `make msrv-check`.

**Exit criteria:** Verification terminates deterministically for every
represented cyclic CFG, accepts well-formed repeated lifetimes across generic
backedges for all current storage families, rejects incompatible cyclic state,
and source loop syntax remains unavailable.

### L2 — Add structured loop identities and control effects

**Purpose:** Represent loop meaning and exit targets explicitly in semantic
IR, independently of parsing and executable CFG construction.

- [x] Add a stable callable-local `LoopId` identity with the same
      owner-validation and deterministic-display discipline as existing
      callable-local identities.
- [x] Replace the two-state HIR block-flow result with a composable effect set
      containing fallthrough, function exit, divergence, `Break(LoopId)`, and
      `Continue(LoopId)`.
- [x] Preserve the existing return, panic, conditional, block, and callable
      completeness behavior while migrating it to the effect-set operations.
- [x] Add a structured HIR `while` node with its `LoopId`, exact condition,
      body, conservative fallthrough effect, and source span.
- [x] Extend deterministic HIR dumps and test fixtures without adding source
      syntax or making source diagnostics depend on HIR internals.
- [x] Give cleanup planning an opaque retained-scope depth and a non-consuming
      operation that plans precisely the scopes exited by a targeted edge.
- [x] Define a lowering loop context that associates a `LoopId` with its exit
      target, latch target, and retained cleanup depth, ready for later
      `break` and `continue`.
- [x] Keep identity, flow composition, cleanup-depth planning, and lowering
      context in cohesive owners behind the existing phase facades.

**Tests:** Identity ownership and display tests; effect-set sequencing, union,
conditional, nested-block, return, and panic tests; manually constructed HIR
loop fixtures and exact dumps; cleanup-planner tests for zero, one, and several
exited scopes and repeated non-consuming plans; regression tests for existing
definite-return diagnostics; `make check`; `make msrv-check`.

**Exit criteria:** Semantic IR can describe structured loops and targeted
future exits without source syntax, existing control-flow typing is unchanged,
and cleanup lowering has an explicit stable target-depth API rather than
loop-specific scope popping.

### L3 — Lower internal HIR `while` loops through generic MIR

**Purpose:** Prove executable loop CFG, condition cleanup, repeated lifetimes,
and backend behavior against manually constructed typed HIR before accepting
the source form.

- [x] Lower HIR `while` to deterministic preheader, condition header, body,
      latch, and exit blocks using only generic MIR `Goto` and `Branch`
      terminators.
- [x] Evaluate the exact boolean condition once in the header on every
      attempted iteration, preserve its scalar result, and finish all
      full-expression cleanup before branching.
- [x] Enter the body as a child lexical scope, emit fresh storage-live epochs
      on each dynamic entry, and emit cleanup followed by storage-dead
      boundaries on normal completion before reaching the latch.
- [x] Route body fallthrough through the dedicated latch and route the latch
      back to the condition header.
- [x] Preserve enclosing locals and their assignments across the loop and keep
      the exit-state compatible with zero iterations.
- [x] Treat every HIR `while` as potentially falling through regardless of
      literal conditions or body effects.
- [x] Extend MIR and backend dumps with deterministic backward targets without
      introducing a dedicated loop opcode or target-owned loop structure.
- [x] Exercise the same generic lowering with primitive, inline object,
      shared, optional, array, checked-view, anchor, and control-effectful
      condition/body fixtures.
- [x] Confirm the verified MIR pass pipeline and x86-64 backend mechanically
      accept the canonical graph and its backward edge.

**Tests:** HIR-to-MIR unit tests for zero and repeated iterations, condition
effects and cleanup ordering, body-local epoch restart, enclosing assignment,
nested internal loops, return from a body, and every current ownership/storage
family; exact MIR dumps; verifier mutations of the canonical graph; backend
assembly shape and native execution from internal fixtures where practical;
`make check`; `make msrv-check`.

**Exit criteria:** Manually constructed valid HIR loops lower, verify, pass
through the MIR pipeline, and execute correctly for the full current storage
family, while the lexer/parser still do not accept source `while`.

### L4 — Activate source `while` end to end

**Purpose:** Expose the frozen `while` statement only after every downstream
phase already supports its complete semantics.

- [x] Reserve `while`, `break`, and `continue` as keywords together, preserving
      stable token spans, dumps, recovery, and focused diagnostics for the two
      not-yet-supported statements.
- [x] Parse exactly `while (expression) block`, with mandatory parentheses and
      braces, as a statement-only AST node.
- [x] Resolve the condition in the enclosing scope, allocate a stable
      callable-local `LoopId`, and resolve the body as an ordinary child block.
- [x] Type-check the condition as exact `bool`, reject truthiness and other
      types, and lower the resolved loop to the structured HIR representation.
- [x] Preserve conservative fallthrough and existing callable
      return-completeness diagnostics, including for `while (true)`.
- [x] Extend AST, resolved, HIR, MIR, and assembly dumps deterministically.
- [x] Add source-to-native coverage for zero, one, and many iterations,
      enclosing mutations, nesting, body-local cleanup, condition cleanup, and
      return from a loop body.
- [x] Update the grammar, language status, control-flow, phase/IR, backend, and
      debugging/test documents from frozen planned `while` behavior to current
      implemented behavior while retaining `break` and `continue` as frozen
      but unimplemented statements.

**Tests:** Lexer keyword and former-identifier tests; parser success, recovery,
and exact dump tests; resolution scope and identity tests; exact-bool and
definite-return type-checker tests; cross-phase dumps; compile-failure goldens
for malformed syntax, wrong condition type, and temporarily unsupported
`break`/`continue`; native goldens for the behavior above; `make check`;
`make msrv-check`.

**Exit criteria:** The frozen `while` form compiles and runs end to end with
the full implemented value and cleanup model, all three control words are
reserved, `break` and `continue` receive stable unsupported diagnostics, and
the living documentation describes the actual implemented boundary.

### L5 — Add targeted `break` statements

**Purpose:** Add the first loop-local exit by reusing stable loop identity,
effect composition, and cleanup-to-depth foundations.

- [ ] Parse exactly `break;` as a statement with deterministic recovery and
      spans.
- [ ] Resolve it to the nearest enclosing `LoopId` and reject use outside a
      loop before type checking.
- [ ] Represent `Break(LoopId)` in HIR and propagate it through nested blocks
      and conditional arms without treating it as function termination.
- [ ] Stop ordinary block fallthrough on the break path while retaining other
      effects from sibling conditional paths.
- [ ] Plan and emit cleanup for every scope exited between the statement and
      the target loop boundary, followed by a generic jump to that loop's exit
      block.
- [ ] Preserve enclosing-loop storage and avoid cleaning scopes outside the
      targeted loop.
- [ ] Cover nearest-loop behavior in nested loops and nested conditionals; do
      not add labels or multi-level syntax.
- [ ] Update living grammar, control-flow, status, phase/IR, and testing
      documentation in the same change.

**Tests:** Parser/recovery tests; outside-loop and nearest-loop resolution
diagnostics; HIR effect-set and dump tests; cleanup planner/lowering tests for
several nested scopes; verifier mutations; native goldens for immediate,
conditional, nested-block, nested-loop, ownership-heavy, and zero-prior-
iteration exits; compile-failure goldens; `make check`; `make msrv-check`.

**Exit criteria:** `break;` exits exactly the nearest loop, performs each
required cleanup once in reverse lexical order, preserves enclosing state, and
has no label or value semantics.

### L6 — Add targeted `continue` statements

**Purpose:** Add iteration-local transfer using the already-verified lifetime
restart and the canonical latch target.

- [ ] Parse exactly `continue;` as a statement with deterministic recovery and
      spans.
- [ ] Resolve it to the nearest enclosing `LoopId` and reject use outside a
      loop before type checking.
- [ ] Represent `Continue(LoopId)` in HIR and compose it through nested blocks
      and conditionals independently from `break` and function exit.
- [ ] Plan and emit cleanup for every scope exited between the statement and
      the target loop body boundary, followed by a generic jump to that loop's
      latch rather than directly to the body or exit.
- [ ] Ensure the latch reaches a fresh condition evaluation only after body
      cleanup and storage-dead boundaries complete.
- [ ] Preserve enclosing-loop storage and target the nearest loop in nested
      cases.
- [ ] Cover mixed `break`, `continue`, `return`, fallthrough, and panic effects
      without adding a generic control-transfer escape hatch to source.
- [ ] Update living grammar, control-flow, status, phase/IR, and testing
      documentation in the same change.

**Tests:** Parser/recovery tests; outside-loop and nearest-loop resolution
diagnostics; HIR effect and dump tests; cleanup and latch-target MIR tests;
verifier mutations; native goldens for immediate, conditional, nested-block,
nested-loop, condition-effect, ownership-heavy, and mixed-exit behavior;
compile-failure goldens; `make check`; `make msrv-check`.

**Exit criteria:** `continue;` performs all and only the exited body-scope
cleanup, reaches the nearest loop's latch, re-evaluates and cleans the condition
once, and starts fresh body-local lifetime epochs only if the next condition is
true.

### L7 — Harden loop lifecycles and optimization boundaries

**Purpose:** Close cross-feature gaps and leave a verifier and test boundary
that later loop forms and CFG optimizations can safely reuse.

- [ ] Build a source-to-observation lifecycle matrix covering primitive,
      inline object, shared, optional primitive/class/shared, array, checked
      view, alias, anchor, and compiler-temporary behavior on condition false,
      body fallthrough, `break`, `continue`, `return`, and nested loops.
- [ ] Verify condition temporaries never enter a body or exit state and body
      lifetimes never enter a latch, header, or loop exit state.
- [ ] Add malformed-MIR mutations for redirected loop edges, omitted lifetime
      boundaries, leaked ownership, incompatible joins/backedges, foreign
      `LoopId`-derived fixtures, and unreachable cyclic components.
- [ ] Extend generative robustness to bounded generic CFG cycles and confirm
      verifier termination and deterministic diagnostics.
- [ ] Add pass-pipeline tests that reorder blocks or insert equivalent
      intermediate blocks while preserving successor, lifetime, cleanup, and
      observable-order invariants.
- [ ] Confirm that verified alternative generic CFG shapes lower mechanically
      on x86-64 without backend recognition of the canonical source shape.
- [ ] Audit touched Rust modules by responsibility, keeping facades concise,
      extracting only demonstrated cohesive loop/lifetime concerns, and
      placing tests with their owners.
- [ ] Remove rollout wording and roadmap task codes from living code, tests,
      and current documentation; retain codes only in roadmap history.
- [ ] Reconcile all living language, compiler, backend, runtime, debugging, and
      test documents and archive this roadmap only after the complete quality
      gate passes from an artifact-free snapshot.

**Tests:** Focused compiler, CLI, docs-checker, runtime-regression, verifier,
backend, dump, golden, and bounded robustness tests; `make check`;
`make msrv-check`; the roadmap-closing clean-snapshot gate required by the
roadmap process.

**Exit criteria:** The complete loop and loop-exit profile behaves correctly
across every current storage family and exit kind, malformed cyclic MIR is
rejected deterministically, equivalent verified CFG shapes remain legal for
future optimization, no backend or runtime owns source loop semantics, living
documentation contains no rollout state, and the roadmap is ready to archive.

## Ordering and dependencies

The dependency chain is deliberately strict:

```text
L0 repeatable storage epochs
  -> L1 finite cyclic verification
  -> L2 structured identities/effects and cleanup depth
  -> L3 internal HIR-to-native loop proof
  -> L4 source while activation
  -> L5 break
  -> L6 continue
  -> L7 lifecycle and optimization hardening
```

L0 precedes cyclic verification because every ownership analysis needs an
explicit event that distinguishes one iteration's dynamic storage lifetime
from the next. L1 precedes loop IR consumers because otherwise an ordinary
backedge can make verification diverge or falsely report repeated
initialization. L2 then establishes semantic identity and targeted cleanup
without parser pressure. L3 uses internal HIR fixtures to prove the complete
downstream path. Only L4 changes accepted source, so there is no intermediate
release that parses a loop which later phases cannot safely compile.

`break` and `continue` remain separate PRs because they have different target
blocks and observable cleanup edges even though they share resolution and HIR
infrastructure. The later exit slices do not block the usefulness of the
initial `while` feature. L7 is hardening rather than the first point of
correctness: each earlier task must satisfy its own full exit criteria.

This roadmap depends on the current deterministic-cleanup, shared-ownership,
optional, array, panic, generic MIR CFG, pass-pipeline, and x86-64 backend
baselines. It has no runtime roadmap dependency and requires no runtime ABI
change. Later loop forms and CFG optimizations may depend on this roadmap, but
they are not prerequisites and must not expand its tasks.

# While Loops Design Proposal

Status: confirmed, promoted, and archived on 2026-07-29. W1 through W13 adopt
their recommended decisions. The living language, phase/IR, runtime ABI, and
backend documents now own the frozen source, representation, and
target-boundary behavior; the
[implementation roadmap](WHILE_LOOPS_ROADMAP.md) owns delivery
order, and this archived proposal retains the decision rationale and promotion
record.

This proposal defines a first `while` loop for Skald and the compiler
boundaries needed to add `break`, `continue`, other loop forms, and
target-independent optimizations without redesigning source semantics or
deterministic cleanup.

The proposal deliberately separates:

- source-visible behavior that should become a frozen language contract;
- confirmed representation invariants required before implementation;
- private implementation details that may continue to evolve; and
- later features that the first implementation need not expose.

## Intended outcome

The first loop feature should provide:

- a strict, statement-only `while` form with an exact `bool` condition;
- deterministic condition evaluation and full-expression cleanup on every
  attempted iteration;
- a fresh lexical body lifetime on every entered iteration;
- correct cleanup on normal completion, `return`, and future `break` and
  `continue` edges;
- structured loop meaning through HIR and ordinary control-flow edges through
  MIR;
- verifier support for cyclic control flow and repeated storage lifetimes;
- no runtime ABI addition and no backend-owned loop semantics; and
- a stable path to later `for`, labeled exits, loop analyses, and
  semantics-preserving optimization.

The initial implementation may expose only `while`. The design nevertheless
settles the control and cleanup boundaries on which `break` and `continue`
would depend.

## Current boundary

Loops, `break`, `continue`, and their cleanup behavior remain unimplemented.
Their source design is now frozen in the living
[control-flow contract](../language/FUNCTIONS_AND_CONTROL_FLOW.md#while-loops-and-loop-exits),
[implemented grammar](../language/GRAMMAR.md#while-loops-and-loop-exits),
and [status matrix](../language/STATUS.md#not-implemented). The frozen
[phase and IR representation](../compiler/PHASES_AND_IR.md#while-loop-representation)
owns loop identity, structured HIR effects, repeatable MIR lifetime epochs,
generic CFG lowering, cyclic verification, transformation invariants, and
private implementation freedom. The
[runtime ABI](../compiler/RUNTIME_ABI.md#loop-abi-boundary) confirms no
new service or version, while the
[backend contract](../compiler/BACKEND.md#while-loop-target-boundary) requires
only mechanical realization of verified generic MIR.

Skald already has useful foundations:

- blocks own lexical scopes and deterministic reverse cleanup;
- typed HIR computes one structured block-flow result;
- MIR has ordinary basic blocks, branches, and jumps;
- the cleanup planner can plan the same lexical cleanup for several outgoing
  edges without consuming its state;
- MIR verification is an explicit target-independent trust boundary; and
- the backend already consumes verified MIR control-flow edges rather than
  source statements.

Two current representations are not sufficient for source loops:

1. HIR block flow distinguishes only fallthrough from function termination,
   so it cannot represent loop-local exits.
2. MIR storage has a static identity but no explicit repeatable lifetime
   boundary. Several verifier domains remember that storage was initialized or
   released, which makes a body local appear to be initialized twice when its
   declaration executes on a later iteration.

The second issue is a prerequisite for accepting loops containing the full
implemented family of local and temporary values. It must not be hidden by
restricting loop bodies to primitive operations.

## Decision register

The decisions are ordered so that source meaning is settled before compiler
representation and representation before implementation scheduling.

| ID | Decision | Confirmed decision | State |
|---|---|---|---|
| [W1](#w1--source-form-and-keywords) | Source form and keyword reservation | `while (condition) block`; reserve `while`, `break`, and `continue` | **Confirmed** |
| [W2](#w2--loop-result-and-future-exit-values) | Statement or expression loop | Statement only; future `break` has no value | **Confirmed** |
| [W3](#w3--condition-type-order-and-full-expression-boundary) | Condition semantics | Exact `bool`, evaluated once before each attempted iteration, then cleaned | **Confirmed** |
| [W4](#w4--lexical-scope-and-per-iteration-lifetimes) | Scope and body lifetime | Condition in enclosing scope; body is a fresh child scope per iteration | **Confirmed** |
| [W5](#w5--definite-return-and-literal-conditions) | Definite return | Every `while` conservatively permits fallthrough | **Confirmed** |
| [W6](#w6--break-continue-and-loop-targeting) | Future exit targeting | Nearest loop, resolved to a stable internal loop identity; labels deferred | **Confirmed** |
| [W7](#w7--cleanup-on-loop-control-edges) | Exit cleanup | Clean every exited lexical scope to the target loop boundary | **Confirmed** |
| [W8](#w8--structured-control-effect-summary) | HIR flow representation | A composable set of fallthrough, function exit, and targeted loop effects | **Confirmed** |
| [W9](#w9--repeatable-mir-storage-lifetimes) | Repeated storage lifetime | Explicit dynamic live/dead epochs for static MIR storage identities | **Confirmed** |
| [W10](#w10--hir-and-mir-loop-representation) | Executable representation | Structured HIR; generic MIR branches and jumps | **Confirmed** |
| [W11](#w11--canonical-initial-cfg-shape) | Initial CFG shape | Dedicated condition entry, latch, and exit | **Confirmed** |
| [W12](#w12--verification-and-optimization-contract) | Pass invariants | Verify cyclic lifetime state; keep source legality independent of optimization | **Confirmed** |
| [W13](#w13--first-implementation-scope) | Delivery scope | Implement `while` first; implement `break` and `continue` as later slices | **Confirmed** |

On 2026-07-29, W1 through W13 were confirmed exactly as recommended. The
alternatives remain below as decision rationale and are not active design
choices.

## Proposed source behavior

### W1 — Source form and keywords

**Question:** What is the exact source shape, and which future control words
should become unavailable as identifiers when `while` is introduced?

**Confirmed decision:**

```text
statement       = ... | while-statement
while-statement = "while" "(" expression ")" block
```

Example:

```text
var index: u64 = 0;
while (index < 10) {
    print(index);
    index = index + 1;
}
```

The parentheses and block are mandatory. A single unbraced statement is not a
loop body.

Reserve `while`, `break`, and `continue` together when `while` enters the
grammar. Skald is committing to those control concepts even if the latter two
remain temporarily unsupported statements. Reserving them together prevents a
later source-compatibility break in which existing bindings named `break` or
`continue` become invalid.

Do not reserve `for`, `in`, `loop`, `do`, or label syntax merely because they
are plausible future features.

**Alternatives:**

1. Reserve only `while` and reserve the exit words when implemented. This
   minimizes the immediate keyword change but creates another deliberate
   source break later.
2. Make the words contextual in statement-leading position. This preserves
   more identifier uses but makes token and recovery behavior less uniform
   with the existing control-flow grammar.
3. Permit an unparenthesized condition or unbraced body. This adds grammar
   variants without improving the semantic model.

**State:** **Confirmed**

### W2 — Loop result and future exit values

**Question:** Is `while` a statement or an expression, and can a future
`break` carry a value?

**Confirmed decision:**

`while` is a statement producing no value. Future first versions of the exit
statements have exactly these shapes:

```text
break-statement    = "break" ";"
continue-statement = "continue" ";"
```

They are not expressions, and `break` does not carry a result.

This matches Skald's existing statement-oriented control flow and avoids
introducing result typing, join storage, or value-producing divergence merely
to obtain basic loops. A later expression-loop feature may extend `break` with
a value as an additive grammar and type-system feature or introduce a
different loop expression. That later design must not retroactively change the
meaning of statement `while`.

**Alternatives:**

1. Make every loop an expression now. This requires deciding the type of zero
   iterations, the relationship between condition failure and `break` values,
   join typing, ownership transfer, and destruction of rejected result paths.
2. Give statement loops value-carrying `break` syntax whose value is currently
   discarded. This creates an unused semantic feature and weakens Skald's rule
   against silently discarded values.

**State:** **Confirmed**

### W3 — Condition type, order, and full-expression boundary

**Question:** When is the condition evaluated and when do its temporaries end?

**Confirmed decision:**

1. Evaluate the condition before the first possible iteration.
2. Require its static type to be exactly `bool`; do not add truthiness.
3. Evaluate it exactly once for each attempted iteration.
4. Preserve its resulting primitive boolean value.
5. Complete full-expression cleanup, including checked views, temporary
   objects, optional guards, shared owners, and array anchors.
6. Branch to the body when the preserved value is `true`; otherwise continue
   after the loop.
7. After normal body completion or `continue`, repeat from step 1.

Condition effects therefore happen even when the body is not entered, and no
condition temporary remains live during the body or after the loop.

This rule extends the existing exact-boolean and deterministic
full-expression model. It is source-visible because destructors, shared
releases, allocation failure, and panic may be observable.

**Alternative:** Keep condition temporaries live through the selected body.
That may retain resources longer, complicates every backedge state, and gives
loops a different full-expression boundary from conditionals.

**State:** **Confirmed**

### W4 — Lexical scope and per-iteration lifetimes

**Question:** Which bindings are visible to the condition and body, and when
does body-local storage begin and end?

**Confirmed decision:**

- The condition resolves in the scope containing the complete `while`
  statement.
- The `while` statement introduces no additional source scope of its own.
- The body block is an ordinary child lexical scope.
- Bindings declared in the body are not visible in the condition or after the
  loop.
- Each entered iteration begins a fresh dynamic lifetime for every body-local
  binding.
- Normal body completion destroys live owning body locals in the ordinary
  reverse order before the next condition evaluation.
- Enclosing locals remain live across the complete loop and preserve
  assignments made by the body.

For example, `item` is initialized and destroyed separately on every entered
iteration:

```text
while (condition()) {
    var item: Resource = Resource();
    use(ref item);
}
```

The proposal does not add declarations to the loop header.

**Alternative:** Treat the body storage as one lifetime spanning the complete
loop. That conflicts with executing the declaration repeatedly and cannot
represent zero iterations naturally.

**State:** **Confirmed**

### W5 — Definite return and literal conditions

**Question:** Can a loop satisfy definite return merely because its condition
is written as `true`?

**Confirmed decision:**

Every `while` conservatively has a condition-false fallthrough path for
source-level definite-return analysis, including `while (true)`.

Consequences:

- A non-`unit` function cannot rely solely on `while (true)` to satisfy its
  return requirement.
- A return after the loop remains reachable for semantic checking.
- Constant propagation or branch folding may later remove the false edge from
  executable MIR without changing whether the source program is accepted.

This is consistent with the current structural treatment of conditionals and
keeps language legality independent of an optimizer or general constant
evaluator. Later recognizing a narrow statically infinite form would accept
additional programs and can be designed separately with reachable `break`
effects.

**Alternatives:**

1. Recognize only the literal token `true`. This is useful for intentional
   infinite loops but introduces a special constant-sensitive flow rule.
2. Use a general compile-time constant evaluator. This is broader than the
   loop feature and risks making diagnostics dependent on optimization-like
   reasoning.

**State:** **Confirmed**

## Future loop exits

### W6 — `break`, `continue`, and loop targeting

**Question:** How should future exit statements select a loop, and how much of
that model should be established with the first `while`?

**Confirmed decision:**

- Unlabeled `break` and `continue` select the nearest lexically enclosing loop.
- Resolution assigns each loop a deterministic callable-local `LoopId` in
  source order.
- Resolved and typed exit statements carry their selected `LoopId`; lower
  phases do not recover the target from nesting depth.
- A loop lowering context stores separate break and continue destinations.
- Labels and labeled exits are deferred. If later adopted, name resolution
  maps a label to the same existing `LoopId` model.

`continue` must not be defined internally as “jump to the while condition.”
It targets a loop-specific continuation destination. For `while` that
destination leads to condition reevaluation; for a future counter-style loop
it may execute an update step first.

`break` or `continue` outside a loop is a source diagnostic. An exit inside a
nested loop selects that nested loop unless a future label explicitly selects
another.

**Alternatives:**

1. Store a numeric nesting depth on each exit. Inserting or restructuring a
   loop changes that encoding, and labels later require replacing it.
2. Leave exits unresolved until MIR lowering. This moves a source name and
   control-selection decision into a lower phase.
3. Freeze labels now. This requires choosing label declaration syntax,
   namespaces, shadowing, and diagnostics without an immediate use.

**State:** **Confirmed**

### W7 — Cleanup on loop-control edges

**Question:** Which values are cleaned before `break` and `continue` transfer
control?

**Confirmed decision:**

Each loop context records the lexical cleanup depth that remains active at its
destination. An exit performs full-expression cleanup, then cleans every live
owning scope between its source and that retained depth, in the ordinary
inner-to-outer and reverse-declaration order.

For `while`:

- `continue` cleans nested blocks and the loop body scope, then transfers to
  the loop's continue destination;
- `break` performs the same body-side cleanup, then transfers to the loop exit;
- neither exit cleans a local declared before the `while`;
- normal body fallthrough cleans the body scope before the backedge;
- `return` retains its existing all-scope cleanup; and
- unrecoverable panic retains the current non-unwinding behavior.

The cleanup planner should expose one depth-based edge-planning operation
rather than separate ad hoc implementations for every exit statement. This is
also a useful foundation for later non-loop control forms, but this proposal
does not define recoverable exception unwinding.

All ordinary incoming edges to the loop exit must agree on the lifetime and
ownership state that remains live there. A `break` edge that still owns a body
local is malformed.

**Alternative:** Lower exits as raw jumps and add cleanup afterward in a
separate repair pass. This makes correctness depend on reconstructing lexical
ownership from CFG shape and obscures the existing source-ordered cleanup
contract.

**State:** **Confirmed**

## Compiler representation

### W8 — Structured control-effect summary

**Question:** How should type checking compose fallthrough, function exits,
and targeted loop exits?

**Confirmed decision:**

Replace the current two-state block-flow concept with a composable summary
whose conceptual outcomes are:

```text
FallThrough
Return
Diverge
Break(LoopId)
Continue(LoopId)
```

The summary is a set of possible outcomes, not one enum value. A conditional
may return on one arm and fall through or break on another.

Composition follows structured source execution:

- in a statement sequence, only `FallThrough` reaches the next statement;
- other outcomes remain possible results of the complete sequence;
- conditional arms combine their outcome sets;
- a loop consumes `Break` and `Continue` outcomes targeting itself;
- a consumed `Break` contributes to loop fallthrough;
- a consumed `Continue` contributes only to the next loop test;
- returns, divergence, and exits targeting an outer loop propagate; and
- under W5, every `while` also contributes condition-false fallthrough.

The exact Rust data structure, bitset layout, and public type names remain
private compiler choices. The invariant to confirm is that different effects
remain distinguishable until the structured construct that owns them consumes
them.

**Alternatives:**

1. Add `Breaks` and `Continues` variants to the current single enum. This
   cannot represent conditionals with different effects on different paths.
2. Continue treating every non-fallthrough statement as function termination.
   This makes missing-return analysis and MIR join construction incorrect.
3. Recompute separate analyses for definite return, reachability, and lowering.
   That recreates the duplicated control-flow reasoning the current HIR summary
   was introduced to avoid.

**State:** **Confirmed**

### W9 — Repeatable MIR storage lifetimes

**Question:** How can one static MIR storage identity represent a body local or
temporary whose dynamic lifetime repeats?

**Confirmed decision:**

Keep one static `StorageId` and storage declaration, but make each dynamic
lifetime epoch explicit in MIR with operations equivalent to:

```text
StorageLive(storage)
...
cleanup or release when required
StorageDead(storage)
```

The exact instruction names are not frozen. Their required meaning is:

- use or initialization requires the storage to be live;
- beginning a second lifetime while it is live is invalid;
- ending a lifetime while it is dead is invalid;
- owned initialized contents must be correctly destroyed, released, moved, or
  transferred before the lifetime ends;
- ending a lifetime clears per-lifetime initialization, field, ownership,
  move, release, checked-view, and optional state associated with that storage;
- beginning a later epoch starts from the declared uninitialized state;
- body-local and reusable temporary storage is dead at a loop header;
- enclosing storage may remain live across the backedge; and
- parameters, receiver storage, and hidden result storage may use documented
  entry/exit lifetime conventions rather than source-emitted markers.

The markers are target-independent verification facts. Frame planning may
reuse a fixed stack slot, and target lowering may erase the markers after all
analyses that need them.

The verifier migration must cover every stateful storage family, not only the
first optional or shared-owner failure encountered. Inline objects containing
owned fields, optional payloads, arrays, shared fields, full-expression
temporaries, anchors, and checked views all need coherent epoch behavior.

**Alternatives:**

1. Infer lifetime reset from cleanup instructions and backedges. Primitive
   storage has no cleanup, moved storage may not have ordinary cleanup, and
   later optimizations can obscure the inference.
2. Give each iteration a distinct `StorageId`. A static CFG cannot allocate an
   unbounded number of identities.
3. Special-case verifier duplicate-initialization checks when a block is in a
   loop. This loses the distinction between a valid new lifetime and an
   invalid repeated initialization within one lifetime.

**State:** **Confirmed**

### W10 — HIR and MIR loop representation

**Question:** Where should the source loop remain structured, and where should
it become ordinary control flow?

**Confirmed decision:**

- AST and resolved IR retain the source `while` shape.
- HIR contains a typed `HirWhile` with its `LoopId`, exact-`bool` condition,
  body, flow summary, and source span.
- MIR lowers source `while` to ordinary `Goto` and `Branch` terminators.
- Future `break` and `continue` lower to cleanup plus ordinary jumps.
- The array-specific generated-loop terminator is not reused for source
  `while`; it carries array construction and lifecycle invariants that source
  loops do not have.
- MIR need not retain semantic loop identity for correctness after targeted
  exits have become block edges. Optional source-loop metadata may be retained
  later for diagnostics, debug information, or optimization hints, but loop
  optimizations must remain able to analyze generic CFG.

This keeps source semantics inspectable in HIR while giving later passes a
uniform CFG rather than a growing family of source-specific terminators.

**Alternatives:**

1. Preserve `While` as a MIR terminator. This hides a multi-block region behind
   one terminator and requires every CFG analysis and backend to understand it.
2. Reuse the array loop terminator. This couples source control flow to
   array-specific backing, index, length, and prefix invariants.
3. Lower directly from resolved syntax to CFG and omit typed `HirWhile`. This
   moves exact condition typing and structured flow meaning out of HIR.

**State:** **Confirmed**

### W11 — Canonical initial CFG shape

**Question:** What initial MIR shape best supports cleanup, `continue`, and
later loop analysis without becoming a language guarantee?

**Confirmed decision:**

```text
preheader -> condition-entry
condition true -> body-entry
condition false -> exit
body fallthrough or continue -> latch -> condition-entry
break -> exit
return -> function exit
```

The condition expression may create additional blocks for checked operations.
Its final success block completes the condition full expression before the
boolean branch.

A dedicated latch gives every normal backedge one target and provides the
correct future `continue` abstraction. A unique exit gives condition-false and
cleaned `break` edges one join whose live-state equality can be verified.

This is an initial lowering invariant and deterministic dump choice, not
source-visible behavior. Optimization passes may split, merge, redirect, or
remove blocks while preserving verified semantics.

**Alternatives:**

1. Jump directly from every body completion and `continue` site to the
   condition. This is executable but creates multiple backedges and complicates
   canonicalization and update-bearing future loops.
2. Put the condition in the preceding source block. Its instructions would be
   skipped on the backedge or preceding source statements would accidentally
   repeat.

**State:** **Confirmed**

### W12 — Verification and optimization contract

**Question:** Which invariants must hold across cyclic MIR and future
transformations?

**Confirmed decision:**

Verification must:

- reach a finite fixpoint for every dataflow domain in a cyclic CFG;
- distinguish current dynamic lifetime state from historical execution;
- require compatible storage, initialization, ownership, field, checked-view,
  and full-expression state at joins and backedges;
- prove body-local lifetime completion before the latch or exit;
- reject cleanup duplication, missing cleanup, use while dead, live-again
  without dead, and dead-again without live;
- verify every block structurally even when source flow or optimization makes
  it unreachable; and
- preserve deterministic error ordering.

Optimization must:

- consume valid MIR and produce valid MIR;
- be followed by verification at the pipeline boundary;
- never establish correctness that lowering omitted;
- never change source acceptance, type diagnostics, or definite-return
  diagnostics;
- preserve condition evaluation count and source ordering;
- treat destruction, retain/release, allocation, panic, checked failure,
  lifetime boundaries, and full-expression cleanup as effects unless a
  narrower proof establishes otherwise; and
- derive loop structure from the generic CFG when performing dominator,
  natural-loop, liveness, invariant-code-motion, or induction analyses.

Constant branch folding may eliminate a literal-false loop exit after semantic
checking. Loop-invariant code motion may hoist a proven pure operation, but it
may not hoist an operation whose evaluation frequency, failure, cleanup, or
ownership effects are observable.

The current mutable-storage MIR does not need phi nodes for loop-carried source
variables. If a future optimization layer uses SSA, it may introduce header
phi nodes in a derived IR without changing source or HIR loop meaning.

**Alternative:** Make a loop canonicalization or optimization pass responsible
for repairing malformed lifetime state. That makes correctness depend on
optimization being enabled and violates the existing pass boundary.

**State:** **Confirmed**

## Delivery boundary

### W13 — First implementation scope

**Question:** Should `while`, `break`, and `continue` ship in one implementation
slice?

**Confirmed decision:**

Confirm their shared semantics together, but implement them in ordered slices:

1. repeatable MIR storage lifetimes and cyclic verifier invariants;
2. `while` from tokens through native execution, including all implemented
   local, temporary, ownership, optional, array, and cleanup families;
3. `break` with loop identity and cleanup-to-depth;
4. `continue` using the same identity and cleanup machinery;
5. optimization and lifecycle hardening beyond the correctness baseline.

This keeps each change reviewable while preventing the first slice from
choosing a representation that later exits cannot use. Reserving exit keywords
under W1 is independent of accepting their statement grammar.

**Alternatives:**

1. Ship all three statements together. This provides a more complete user
   feature but combines lifetime epochs, cyclic verification, structured flow,
   exit resolution, and two cleanup transfers in one large change.
2. Implement primitive-only `while` first. This exposes a source construct
   whose legal body depends on representation accidents and postpones the
   central deterministic-lifetime problem.
3. Implement `while` before deciding future exits. This risks hard-coding the
   backedge and cleanup behavior that this proposal is intended to settle.

**State:** **Confirmed**

## Diagnostics

The confirmed feature should have structured diagnostics for at least:

- a missing condition, parenthesis, body block, or other malformed loop form;
- a condition whose static type is not exactly `bool`;
- future `break` or `continue` outside an enclosing loop;
- any future labeled-exit syntax until labels are separately frozen;
- a non-`unit` callable whose source flow can still reach its closing brace;
  and
- malformed MIR lifetime, cleanup, ownership, or backedge state at the
  verifier boundary.

Parser recovery should stop at the loop's balanced condition or body boundary
without inventing a partially valid loop. Semantic diagnostics should identify
the condition or exit statement span and avoid exposing internal block or
storage IDs except in compiler IR verification errors and dumps.

## Determinism and inspection

- AST and resolved dumps preserve the source `while` shape and span.
- Resolved and HIR loop identities are allocated deterministically in
  callable source order.
- HIR dumps show the typed condition, body, and structured flow effects without
  backend block details.
- MIR dumps expose lifetime boundaries, cleanup order, and ordinary control
  edges deterministically.
- Block allocation and dump order are deterministic for the canonical initial
  CFG.
- Optimized dumps, when optimization exists, belong to the named pass stage
  and need not preserve unoptimized block numbering.
- Generated assembly remains deterministic and gains no loop-specific runtime
  symbol.

## Test obligations

### Source and semantic tests

- zero, one, and several iterations;
- nested loops and conditionals;
- exact-`bool` acceptance and rejection of every other implemented type;
- condition effects and cleanup when initially false and after later tests;
- enclosing binding mutation visible after the loop;
- body-local visibility rejection outside the body;
- conservative missing-return behavior for `while (true)` under W5; and
- future nearest-loop exit selection and outside-loop diagnostics.

### HIR and MIR tests

- deterministic loop identities and dumps;
- structured flow composition for mixed conditional outcomes;
- canonical condition, body, latch, and exit edges;
- condition full-expression cleanup before either successor;
- fresh storage lifetime epochs on the body cycle;
- ordinary, nested, optional, shared, class, array, and temporary cleanup on
  normal backedges;
- cleanup-to-depth on future break and continue edges; and
- outer storage remaining live and body storage dead at the header and exit.

### Verifier mutation tests

- initialization twice within one epoch;
- a second valid initialization after live/dead/live;
- missing live or dead markers;
- use or cleanup while dead;
- dead storage with live owned contents;
- mismatched header or exit state;
- retained checked views or full-expression temporaries on a backedge;
- duplicated or skipped cleanup;
- invalid backedge targets and unterminated blocks; and
- cycles whose dataflow requires more than one worklist visit.

### Backend and end-to-end tests

- forward condition and exit branches plus a deterministic backward edge;
- loop-carried primitive mutation;
- calls and preserved values across the backedge;
- observable body-local construction and destruction once per entered
  iteration;
- shared, optional, array, and string behavior in condition and body;
- nested loops with future exits;
- assembler acceptance and native execution; and
- unchanged runtime ABI and deterministic assembly.

The eventual implementation roadmap should assign focused tests to their
owning phase and use `make check` as the complete ordinary repository gate.
Rust or manifest changes also require the documented MSRV check.

## Explicit exclusions

This proposal does not freeze or implement:

- `for`, `for ... in`, `do while`, or an unconditional `loop` form;
- iterator, iterable, generator, coroutine, or async protocols;
- loop expressions or value-carrying `break`;
- loop `else` clauses;
- labels or labeled `break` and `continue` syntax;
- declarations in a loop header;
- recoverable exceptions or stack unwinding;
- compile-time execution or a general constant evaluator;
- mandatory SSA, phi nodes, loop unrolling, vectorization, or
  loop-invariant-code motion;
- a source-visible iteration counter;
- concurrency or memory-model rules; or
- any loop-specific runtime ABI service.

The confirmed identities, control-effect summary, cleanup depth, lifetime
epochs, and generic CFG are intended to make these features possible where
appropriate, not to define their source behavior prematurely.

## Promotion criteria

Decision confirmation and promotion into frozen living contracts are
complete:

- [x] W1 through W13 have explicit confirmed decisions.
- [x] The selected decisions have been checked for contradictory assumptions.
- [x] Promote source-visible rules into the implemented grammar's planned
      direction, the focused control-flow contract, and the status matrix
      without requiring readers to consult this proposal.
- [x] Promote compiler representation invariants into the phase and IR
      contract without freezing private Rust organization or exact CFG
      numbering.
- [x] Confirm in the runtime ABI and backend contracts that the feature
      requires no new runtime service or backend-owned loop semantics.
- [x] Create a PR-sized implementation roadmap that orders repeatable storage
      lifetimes and cyclic verifier foundations before source acceptance.
- [x] Run final documentation link and index validation after the promotion,
      archive move, and roadmap addition.

The living language and compiler documents are authoritative. This proposal is
retained in `docs/archive/` as the historical decision record, and the
implementation roadmap remains in `docs/roadmaps/` until complete.

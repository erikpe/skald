# Integer Bitwise Operators and Checked Shifts Roadmap

Status: complete; BW0 through BW4 are complete.

This roadmap implements the next frozen primitive-operator family: exact-width
integer bitwise complement, conjunction, disjunction, and exclusive
disjunction, followed by eager shifts with checked `u64` counts. It delivers
both families from source syntax through verified MIR and native x86-64 while
keeping pure bitwise operations separate from shifts' source-reachable failure
control flow.

The governing language contract is the
[frozen primitive operator profile](../language/TYPES_AND_VALUES.md#frozen-primitive-operator-profile),
the source shape is the
[frozen primitive-operator expression extension](../language/GRAMMAR.md#frozen-primitive-operator-expression-extension),
and checked failure uses the existing
[panic catalog](../language/ERRORS.md#frozen-panic-design).

## Scope and invariants

- Prefix `~` accepts exactly `i64`, `u64`, or `u8`, complements every bit in
  that type's fixed width, and returns the identical type.
- Binary `&`, `|`, and `^` require two operands of the same exact integer type
  and return that type. They do not accept `bool` and perform no implicit cast,
  promotion, narrowing, signedness change, truthiness conversion, or
  expected-type reinterpretation.
- `<<` and `>>` accept an `i64`, `u64`, or `u8` left operand and exactly `u64`
  on the right. The result has the left operand's type; shifts are the sole
  mixed-type shape in this roadmap.
- Left shift inserts zero low bits and discards high bits. Right shift is
  arithmetic for `i64` and logical for `u64` and `u8`. Every `u8` producer is
  canonical in `0..=255`.
- Counts `0u..63u` are valid for `i64` and `u64`; counts `0u..7u` are valid for
  `u8`. A count at or above the left width reaches the exact
  `shift count out of range` compiler-known panic. The compiler never relies
  on target count masking, and a constant excessive count is not reclassified
  as a compile-time type or range error.
- A unary operand evaluates exactly once. Every eager binary left operand
  evaluates exactly once before the right operand, and both operands complete
  before a shift-count check or operation.
- On a successful operation, all completed temporaries retain the existing
  enclosing full-expression lifetime and reverse cleanup order. Once shift
  failure reporting begins, no remaining source-level cleanup is guaranteed,
  consistent with the existing non-returning, non-unwinding panic contract.
- Prefix operators associate right to left. Shift, `&`, `^`, and `|` chains
  each associate left to right. From tighter to looser binding, the new binary
  tiers are additive, shift, `&`, `^`, `|`, comparison/`is`, `&&`, and `||`.
- Longest-match tokenization keeps `&&` and `||` distinct from eager `&` and
  `|`, and keeps `<<`/`>>` distinct from comparison punctuation.
- Syntax and resolved IR retain source operator identity, operand shape, and
  spans. Typed HIR and MIR retain exact target-independent operation flavor,
  width, operand types, result type, and shift failure capability.
- Bitwise operations are ordinary pure scalar rvalues. Shifts are eager value
  expressions with an explicit target-independent range-check edge; they are
  control-affecting for enclosing expression lowering even though both
  operands are eager.
- MIR verification proves exact operand and result types, definition before
  use, canonical `u8` results, checked-shift correspondence, and a terminal
  failure edge with the exact shift reason. The x86-64 backend mechanically
  realizes only verified MIR.
- Source support is not advertised as implemented until syntax, exact type
  selection, HIR, verified MIR, x86-64 lowering, diagnostics, dumps, native
  results, failure output, and living documentation agree for that family.
- The runtime header, ABI version, and public symbol set remain unchanged.
  Checked shifts reuse the sole length-delimited panic reporter with static
  compiler-owned message bytes.
- Division, remainder, floating comparisons, floating bit operations,
  rotations, bit-count utilities, compound assignment, boolean eager bitwise
  operations, user-defined operators, and constant folding are excluded.

### Settled representation boundary

Pure bitwise HIR and MIR operations carry one semantic operation kind and one
exact integer kind. Signedness does not change `~`, `&`, `|`, or `^` value
bits, but the integer kind remains explicit so width, operand types, result
type, dumps, verification, and later consumers never infer it from source
spelling or a target register.

Shift HIR and MIR carry direction and the exact left integer kind separately
from the fixed `u64` count type. This identifies arithmetic versus logical
right shift target-independently and avoids forcing shifts into the current
same-type binary-operation query shape. Exact Rust enum and module names remain
private.

MIR lowering evaluates and secures the left value, then evaluates and secures
the count, then branches on an explicit width-aware count check. The failure
successor terminates with `shift count out of range`; only the verified success
continuation performs the shift and produces the result. The verifier ties the
check width, secured operands, selected shift flavor, result type, success
continuation, and failure reason together, so an unchecked or mismatched shift
cannot reach instruction selection. Scalar carriers bridge the control-flow
edge because MIR transient values remain block-local.

This shape follows Skald's existing checked-operation and static-termination
boundaries. It also avoids Niflheim's backend-owned checked-shift convention:
Niflheim is useful evidence for the operand matrix, edge cases, and target
instruction choices, but Skald keeps the semantic check and failure reason
explicit in verified MIR and reuses its own common panic reporter.

## Repository gates

Every task runs its focused tests, `make check`, and `git diff --check`. Tasks
that change Rust phase models, accepted syntax, public facades, or backend code
also run `make msrv-check`. The closing task additionally runs
`make robustness-long` and the full gates from an artifact-free snapshot or
clean checkout.

Tests stay with their owner: lexer, syntax, resolution, type checking, HIR/MIR
lowering and dumps, MIR verification, and backend selection tests remain
colocated; public cross-phase behavior belongs in the compiler integration
tests; reusable source observations belong in the top-level compile-failure
and native golden corpus. Runtime tests prove the ABI remains unchanged rather
than adding operator-specific C harnesses.

## Progress

- [x] BW0 — Establish executable pure bitwise operations
- [x] BW1 — Enable bitwise source expressions end to end
- [x] BW2 — Establish executable checked shifts
- [x] BW3 — Enable checked shift source expressions end to end
- [x] BW4 — Harden and promote the complete family

## PR-sized implementation sequence

### BW0 — Establish executable pure bitwise operations

**Purpose:** Complete the target-independent and x86-64 path for the low-risk
pure operations before source syntax can select them.

- [x] Add cohesive exact-width HIR and MIR operation carriers for integer
      complement, conjunction, disjunction, and exclusive disjunction,
      including exact operand/result queries and deterministic mnemonic
      vocabulary.
- [x] Lower unary complement and eager binary bitwise operands exactly once;
      retain the existing left-to-right spill behavior when a right operand
      contains control flow, and add no block, failure edge, cleanup action,
      runtime call, or exceptional capability.
- [x] Extend MIR verification for the closed `i64`/`u64`/`u8` matrix, matching
      value definitions, result types, block-local use, and canonical `u8`
      production, with deterministic one-invariant mutation diagnostics.
- [x] Extend the private x86-64 machine model, target legality, and instruction
      selection for complement, AND, OR, and XOR while preserving exact 64-bit
      patterns and canonicalizing the low byte of `u8` results.
- [x] Extend HIR/MIR facades and dumps only with the minimal semantic
      vocabulary needed by downstream consumers; do not expose target
      register, opcode, or flag choices.
- [x] Keep source construction of `~`, `&`, `|`, and `^` disabled in this task;
      direct HIR/MIR and backend tests establish the complete downstream path.
- [x] Preserve all existing arithmetic, comparison, eager boolean,
      short-circuit, label, frame, runtime, and ABI observations.

**Tests:** Direct HIR/MIR truth-pattern matrices for all four operations and
three integer kinds; zero, alternating bits, sign-bit, all-ones, and `u8`
boundaries; eager left-to-right and right-control-effect spilling; verifier
mutations for wrong operand/result type, integer kind, definition order, and
noncanonical `u8`; deterministic dumps; selector and emission tests; system
assembler acceptance; existing source rejection; `cargo test --locked -p
skald-compiler`, `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Verified HIR, MIR, and x86-64 can represent and execute every
pure bitwise operation with exact fixed-width results and no runtime or control
effect, while the accepted source language remains unchanged.

### BW1 — Enable bitwise source expressions end to end

**Purpose:** Connect the proven pure operations to their complete source
syntax, precedence, exact selection, diagnostics, and native observations.

- [x] Lex `~`, `&`, `|`, and `^` while preserving longest-match `&&` and `||`,
      invalid-token recovery, exact spans, and existing punctuation behavior.
- [x] Add right-associative prefix `~` plus separate left-associative `&`, `^`,
      and `|` parser tiers between additive expressions and the unified
      comparison/`is` tier; preserve postfix priority, casts, logical
      precedence, non-associative comparisons, recovery, and nesting limits.
- [x] Preserve exact operator identity, operand shape, grouping, and spans in
      AST and resolved IR without selecting integer width or target behavior.
- [x] Select prefix complement for exactly one integer operand and each binary
      operation for two identical integer types. Check binary operands in
      source order and reject mixed integers, `bool`, `f64`, `unit`, optional,
      array, class, object-view, and shared-owner combinations before HIR.
- [x] Produce focused diagnostics that identify the operator and actual
      operand types without suggesting truthiness, promotion, or an implicit
      cast.
- [x] Add deterministic token, AST, resolved, HIR, and MIR dumps for nesting,
      grouping, calls, fields, casts, comparisons, and logical composition.
- [x] Add native and compile-failure goldens covering fixed-width value
      patterns, exactly-once left-to-right effects, locals, fields, parameters,
      calls, returns, assignments, conditions through comparisons, arrays,
      optional unwrap, and every currently valid eager-expression consumer.
- [x] Update the implemented grammar, operator surface, status matrix,
      compiler phase/backend contracts, and testing/debugging guidance when
      source support becomes active; continue to report shifts as frozen but
      unimplemented.

**Tests:** Lexer distinction matrices for `&`/`&&` and `|`/`||`; parser
precedence and associativity for unary, additive, each bitwise tier,
comparison/`is`, `&&`, and `||`; nesting and recovery; complete valid and
invalid type matrices; AST/resolved/HIR/MIR dump tests; evaluation-order tests
with calls and mutation; extrema and `u8` canonicalization; compile-failure and
native goldens; `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** `~`, `&`, `|`, and `^` have one deterministic accepted
source-to-native path for every frozen integer case, every excluded operand
combination fails before HIR, existing logical and comparison parses remain
unchanged, and living documentation advertises only the implemented pure
bitwise subset.

### BW2 — Establish executable checked shifts

**Purpose:** Settle and verify the fallible eager-expression representation
before source syntax can depend on target count checks or failure control flow.

- [x] Add exact HIR and MIR shift operations carrying direction, left integer
      kind, fixed `u64` count type, result type, right-shift flavor, left width,
      and compiler-known failure capability without encoding an x86 opcode.
- [x] Classify shift HIR as control-affecting so an enclosing eager expression
      secures an earlier block-local scalar before shift lowering introduces a
      count-check edge.
- [x] Lower the left operand once, then the right operand once, securing both
      values across blocks only after each operand has completed; emit the
      explicit count check after both operands and perform the shift only on
      its success continuation.
- [x] Route the failure continuation directly to a new distinct MIR
      `shift count out of range` termination reason. Preserve successful-path
      full-expression lifetime and the existing rule that panic guarantees no
      remaining cleanup after reporting begins.
- [x] Extend MIR verification to prove exact left/count/result types, width and
      signedness flavor, secured-carrier storage, check-before-operation
      dominance, exact success and failure targets, matching termination
      reason, block-local value use, and no ordinary successor from failure.
- [x] Extend deterministic MIR dumps and the static termination pool with the
      exact catalog message without renumbering or otherwise changing existing
      panic-message observations.
- [x] Extend the x86-64 machine model and instruction selection to compare the
      unsigned count with 64 or 8 before any target shift, use the required
      variable-count register only after success, select left, arithmetic
      right, or logical right shift mechanically, and canonicalize `u8`.
- [x] Keep source construction of `<<` and `>>` disabled in this task. Direct
      HIR/MIR tests exercise valid results and the exact native panic path.
- [x] Keep the public runtime header, `ska_rt_abi_v6`, reporter signature,
      frame contract, and public symbol set unchanged.

**Tests:** Direct operation matrices for both directions and all three left
types; counts zero, one, width minus one, exactly width, above width, and
`u64::MAX`; negative and sign-bit `i64` arithmetic right shifts; high-bit
discard on left shift; `u8` canonicalization; operand calls and nested
control-affecting expressions; verifier mutations for non-`u64` counts,
mismatched widths/flavors, unchecked operations, wrong carriers, swapped
success/failure edges, wrong termination reason, and use before definition;
static-message pool stability; assembly ordering proving compare/branch before
shift; exact native panic stderr and exit; repository gates.

**Exit criteria:** Directly constructed checked shifts execute with the frozen
portable semantics, excessive counts reach only the exact verified panic
reason before any target shift, malformed checked-shift CFG is rejected, and
no shift source expression is accepted yet.

### BW3 — Enable checked shift source expressions end to end

**Purpose:** Activate the complete checked-shift language feature only after
every selected operation has a verified executable downstream path.

- [x] Lex `<<` and `>>` by longest match without disturbing `<`, `<=`, `>`,
      `>=`, comparison-chain recovery, or existing logical punctuation.
- [x] Insert one left-associative shift parser tier between additive and `&`;
      preserve the complete frozen precedence ladder, source spans, grouping,
      nesting limits, and deterministic recovery.
- [x] Preserve left/right shift identity in AST and resolved IR without
      selecting a width, signedness, result type, failure edge, or target
      instruction early.
- [x] Select shifts only for an integer left operand and exact `u64` right
      operand, returning the left type. Check operands in source order and
      reject every other left or count type before HIR with focused actual-type
      diagnostics and no implicit conversion suggestion.
- [x] Exercise arbitrary currently valid operands, including calls, fields,
      array access, optional unwrap, casts, assignments used by later
      expressions, nested short-circuit expressions, allocation-backed
      effects, and operands that can fail before the count check.
- [x] Add deterministic phase dumps showing source direction, exact selected
      shift flavor, secured operands, count-check CFG, success-only shift,
      failure termination, result carriage, and enclosing cleanup.
- [x] Add source-to-native goldens for all widths, directions, boundary counts,
      signed arithmetic right shift, high-bit discard, `u8` canonicalization,
      every expression consumer, left-to-right effects, successful cleanup,
      and exact excessive-count panic output. Literal excessive counts must
      compile and fail through the same runtime path as dynamic counts.
- [x] Update living grammar, language/compiler status, phase/backend contracts,
      errors wording, debugging/testing guidance, and links to describe the
      now implemented complete bitwise-and-shift family without claiming
      division, remainder, floating operations, or constant folding.

**Tests:** Lexer longest-match matrices; parser precedence for additive,
shift, all three bitwise tiers, comparison/`is`, and logical expressions;
left-associative chains and grouping; exact valid and invalid type matrices;
diagnostic and phase-dump determinism; counts around 8 and 64 plus
`u64::MAX`; native values and exact panic stderr/exit; call-based evaluation
order, failure-before-check, all-success cleanup, and every-consumer goldens;
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** Every frozen bitwise and checked-shift source expression has
one typed, verified, deterministic, executable path; every excluded operand
shape fails before HIR; excessive counts cannot be masked by x86-64; and all
living documentation reports the complete family as implemented.

### BW4 — Harden and promote the complete family

**Purpose:** Close the family with exhaustive boundary coverage, stable
observations, maintainable ownership, and no rollout-only structure before
division and remainder introduce their own failure semantics.

- [x] Complete valid and invalid matrices across literals, grouped values,
      bindings, parameters, fields, calls, casts, arrays, optional unwrap,
      assignments, comparisons, logical operands, conditions, and returns.
- [x] Prove all precedence and associativity boundaries, including nested
      prefix `~`, prefix/postfix composition, additive-to-shift binding,
      `&`/`^`/`|`, comparison/`is`, and `&&`/`||`, plus deterministic rejection
      of malformed chains and incomplete operators.
- [x] Add independent-process determinism coverage for diagnostics and all
      phase dumps plus stable assembly, panic-message, and native observations
      where focused owner tests do not already provide it.
- [x] Audit touched lexer, syntax, resolution, type-check, HIR, MIR lowering,
      control-effect, verifier, termination, backend, facade, dump, and test
      owners by responsibility. Resolve high-priority hotspots and place any
      material out-of-scope findings in a separately indexed discoveries
      document.
- [x] Remove roadmap codes and rollout wording from living code, tests, and
      general documentation; retain milestone vocabulary only in roadmap and
      archive documents.
- [x] Confirm the runtime ABI non-change, existing panic-message stability,
      documentation links, artifact cleanliness, and the boundary reserving
      division/remainder and floating operators for later roadmaps.
- [x] Mark this roadmap complete, move it to `docs/archive/`, update the active
      and archive indexes, and repair incoming relative links.

**Tests:** Exhaustive source, HIR, MIR, verifier, backend, compile-failure,
native-value, native-panic, precedence, every-consumer, and determinism
matrices; focused documentation and link checks; `make check`,
`make msrv-check`, `make robustness-long`, `git diff --check`, and an
artifact-free final full gate.

**Exit criteria:** Exact-width integer bitwise operations and checked shifts
are complete deterministic contracts from source through native x86-64;
existing arithmetic, comparison, logical, ownership, cleanup, failure, and ABI
behavior remains intact; living documentation contains no rollout state; and
the completed roadmap is archived.

## Ordering and dependencies

Pure bitwise operations come first because they reuse the established eager
scalar path and add no failure or control-flow representation. Their source
activation follows only after verified MIR and x86-64 can execute the entire
family. Checked shifts come next as a separate representation step because
their mixed operand types, block-local value carriage, source-reachable panic,
and target count-masking hazard must be proven before syntax can select them.
The final source task then connects that settled path to arbitrary valid
operands and every expression consumer, and broad hardening closes the family.

This roadmap depends on the completed primitive integer operations, eager
boolean operators, and short-circuit boolean expressions; the implemented
common panic reporter and static termination pool; canonical scalar and `u8`
boundaries; the frozen primitive operator profile; and the existing
full-expression lifetime contract. It adds no dependency on division,
remainder, floating comparisons, or a future optimization pipeline.

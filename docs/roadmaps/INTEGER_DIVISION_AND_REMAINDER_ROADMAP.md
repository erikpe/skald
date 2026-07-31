# Integer Division and Remainder Roadmap

Status: in progress; DR0 through DR3 are complete and DR4 is next.

This roadmap implements the frozen integer `/` and `%` family for exact
`i64`, `u64`, and `u8` operands. It delivers floor division and matching
remainder semantics from source syntax through verified MIR and native x86-64,
including explicit zero-divisor failure and the defined, non-trapping
`i64::MIN / -1` and `i64::MIN % -1` results.

The governing language contract is the
[frozen primitive operator profile](../language/TYPES_AND_VALUES.md#frozen-primitive-operator-profile),
the source shape is the
[frozen primitive-operator expression extension](../language/GRAMMAR.md#frozen-primitive-operator-expression-extension),
and checked failure uses the existing
[panic catalog](../language/ERRORS.md#frozen-panic-design).

## Scope and invariants

- `/` and `%` require two operands of the same exact integer type: `i64`,
  `u64`, or `u8`. The result has that identical type. There is no implicit
  cast, promotion, narrowing, signedness change, truthiness conversion, or
  expected-type reinterpretation.
- Unsigned division has the usual quotient and remainder semantics. Signed
  `i64` division rounds toward negative infinity, and signed remainder is zero
  or has the divisor's sign, with `r = dividend - q * divisor`.
- The signed sign matrix is therefore fixed: `7 / 3 == 2`, `-7 / 3 == -3`,
  `7 / -3 == -3`, and `-7 / -3 == 2`; the corresponding remainders are `1`,
  `2`, `-2`, and `-1`.
- Division by zero reaches the exact `integer division by zero`
  compiler-known panic. Remainder by zero reaches the distinct exact
  `integer remainder by zero` panic. A literal zero divisor is not
  reclassified as a compile-time type or range error.
- `i64::MIN / -1` is defined as `i64::MIN`, and `i64::MIN % -1` is defined as
  `0`. These are successful language results. No implementation may expose
  the x86-64 signed-division trap for this pair.
- Every eager binary left operand evaluates exactly once before the right
  operand. Both operands complete before the divisor check and operation.
- On success, all completed temporaries retain the existing enclosing
  full-expression lifetime and reverse cleanup order. Once failure reporting
  begins, no remaining source-level cleanup is guaranteed, consistent with
  the non-returning, non-unwinding panic contract.
- `*`, `/`, and `%` occupy one left-associative multiplicative tier above
  additive expressions and below prefix expressions. Existing postfix,
  shift, bitwise, comparison/`is`, `&&`, and `||` precedence remains intact.
- Lexing preserves `//` as a line-comment introducer. `/` is division, `%` is
  remainder, and spaced `/ /` is two division tokens rather than a comment.
- Syntax and resolved IR retain source operator identity, operand shape, and
  spans. Typed HIR and MIR retain exact target-independent operation flavor,
  integer kind, operand/result types, and zero-divisor failure capability.
- Division and remainder are eager value expressions with an explicit
  target-independent zero check. They are control-affecting for enclosing
  expression lowering even though both operands are eager.
- MIR verification proves exact operand and result types, definition before
  use, canonical `u8` results, divisor-check correspondence, success-only
  arithmetic, and a terminal failure edge with the exact operation-specific
  reason. The x86-64 backend mechanically realizes only verified MIR.
- Source support is not advertised as implemented until syntax, exact type
  selection, HIR, verified MIR, x86-64 lowering, diagnostics, dumps, native
  results, failure output, arbitrary valid operands, and living documentation
  agree for the complete integer family.
- The runtime header, ABI version, frame contract, and public symbol set remain
  unchanged. Both failures reuse the sole length-delimited panic reporter with
  static compiler-owned message bytes.
- Floating-point division, floating-point remainder, division or remainder on
  mixed integer types, compound assignment, constant folding, strength
  reduction, and other arithmetic optimization are excluded. In particular,
  recognizing `/` does not make `f64 / f64` valid in this roadmap.

### Settled representation boundary

Division/remainder HIR and MIR carry one semantic operation kind and one exact
integer kind. The operation is target-independent: it describes floor
division and divisor-signed remainder directly rather than x86 truncating
division, register conventions, flags, or a corrective instruction sequence.
Exact Rust enum and module names remain private.

MIR lowering evaluates and secures the dividend, then evaluates and secures
the divisor, then branches on an explicit zero check. The failure successor
terminates with the operation-specific reason; only the verified success
continuation performs division or remainder and produces the result. A scalar
carrier joins the successful result back into its enclosing expression because
MIR transient values remain block-local. The verifier ties the operation,
integer kind, secured operands, zero check, success continuation, failure
reason, result carrier, dominance, and canonical `u8` result together.

The signed overflow pair is not another semantic failure edge. On x86-64 the
backend must recognize `i64::MIN` divided by `-1` before issuing `idiv`, produce
the frozen quotient or remainder directly, and otherwise use signed division.
For ordinary non-exact signed division, it corrects x86's truncation-toward-zero
quotient and dividend-signed remainder when the remainder is nonzero and its
sign differs from the divisor. Unsigned division zero-extends the dividend
before `div`; `u8` inputs and results remain canonical.

Niflheim supplies useful evidence for that floor-correction sequence and for
the sign, extrema, high-bit, and narrow-width test matrix. Skald does not adopt
backend-owned failure semantics or rely on a target trap: its zero-divisor
check and exact reason remain explicit in verified MIR, and its signed
overflow pair is guarded before native division.

## Repository gates

Every task runs its focused tests, `make check`, and `git diff --check`. Tasks
that change Rust phase models, accepted syntax, public facades, or backend code
also run `make msrv-check`. The closing task additionally runs
`make robustness-long` and the full gates from an artifact-free snapshot or
clean checkout.

Tests stay with their owner: lexer, syntax, resolution, type checking, HIR/MIR
lowering and dumps, MIR verification, and backend selection tests remain
colocated; public cross-phase behavior belongs in compiler integration tests;
reusable source observations belong in the top-level compile-failure and
native golden corpus. Runtime tests prove the ABI remains unchanged rather
than adding operator-specific C harnesses.

## Progress

- [x] DR0 — Define checked integer division and remainder semantics
- [x] DR1 — Lower and verify the checked control-flow shape
- [x] DR2 — Execute verified operations on x86-64
- [x] DR3 — Enable the complete source family end to end
- [ ] DR4 — Harden and promote the complete family

## PR-sized implementation sequence

### DR0 — Define checked integer division and remainder semantics

**Purpose:** Establish one exact target-independent representation and
inspection vocabulary before lowering, native instructions, or source syntax
can depend on it.

- [x] Add cohesive HIR and MIR semantic operation carriers for division and
      remainder, including exact integer kind, operand/result queries,
      deterministic mnemonics, and compiler-known zero-divisor capability.
- [x] Classify both operations as control-affecting so enclosing eager
      expressions secure earlier block-local scalars before later lowering can
      introduce a divisor-check edge.
- [x] Add distinct target-independent termination reasons for integer division
      by zero and integer remainder by zero without changing existing reason
      identity or message ordering.
- [x] Record floor quotient, divisor-signed remainder, and the successful
      `i64::MIN / -1` pair in operation-level APIs and tests without exposing a
      target opcode or treating overflow as failure.
- [x] Extend HIR/MIR facades and deterministic dumps only with the semantic
      vocabulary downstream consumers require; keep substantial
      implementation in responsibility-focused private modules.
- [x] Keep source construction and executable MIR lowering disabled in this
      task. Directly constructed model tests establish the closed semantic
      matrix and prevent incomplete syntax activation.
- [x] Preserve all existing arithmetic, bitwise, shift, comparison, logical,
      cleanup, failure, runtime, and ABI observations.

**Tests:** Direct HIR/MIR operation matrices for `/` and `%` across
`i64`/`u64`/`u8`; exact operand/result and failure-capability queries; all four
signed sign combinations; exact and non-exact division; extrema including the
`i64::MIN / -1` pair; deterministic operation and termination dumps; existing
source rejection; focused crate tests, `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** HIR and MIR can describe every frozen integer division and
remainder case, including its exact failure capability and non-failing signed
overflow result, while accepted source syntax and executable backend behavior
remain unchanged.

### DR1 — Lower and verify the checked control-flow shape

**Purpose:** Prove evaluation order, failure control flow, result carriage, and
cleanup boundaries before any target division instruction can be selected.

- [x] Lower the dividend exactly once, secure it, lower the divisor exactly
      once, and secure it only after each operand has completed; emit the zero
      check after both operands and perform the semantic operation only on its
      success continuation.
- [x] Route the failure continuation directly to the matching division- or
      remainder-by-zero termination reason, with no operation, cleanup, or
      ordinary successor after reporting begins.
- [x] Carry only the successful result through an exact typed scalar slot into
      the enclosing expression, preserving existing full-expression lifetime
      and reverse cleanup on every successful path.
- [x] Support nested control-affecting operands and enclosing expressions so a
      division or remainder result can safely participate in another eager,
      checked, short-circuit, or cleanup-bearing expression.
- [x] Extend MIR verification to prove exact dividend/divisor/result types,
      secured-carrier storage, zero-check operands, check-before-operation
      dominance, exact success and failure targets, matching operation and
      termination reason, result initialization, block-local use, and no
      ordinary successor from failure.
- [x] Extend deterministic MIR dumps to expose the secured operands, zero
      check, success-only semantic operation, exact failure termination,
      result carrier, join, and enclosing cleanup without target details.
- [x] Keep source construction and x86-64 emission disabled; directly
      constructed HIR drives lowering and verifier coverage.

**Tests:** Direct lowering for values, calls, nested checked shifts, nested
division/remainder, allocation-backed effects, and operands that terminate
before the zero check; exactly-once left-to-right traces; successful cleanup
order; verifier mutations for wrong kinds/types/carriers, unchecked
operations, operation-before-check, swapped edges, wrong failure reason,
uninitialized joins, use before definition, and reachable failure successors;
deterministic dumps and repository gates.

**Exit criteria:** Every directly constructed operation lowers to one verified
checked diamond with correct result and cleanup carriage, malformed or
unchecked division/remainder MIR is rejected deterministically, and source
still cannot select the feature.

### DR2 — Execute verified operations on x86-64

**Purpose:** Mechanically realize the verified language semantics without
exposing hardware traps, truncating signed semantics, or target count/width
accidents.

- [x] Extend the private x86-64 machine model, legality checks, register-use
      model, instruction selection, emission, and dumps with the minimal
      signed and unsigned divide machinery required by verified MIR.
- [x] Append the exact `integer division by zero` and
      `integer remainder by zero` bytes to the static termination pool without
      renumbering any existing message, and route both verified failure
      reasons through the existing common panic reporter.
- [x] For `u64` and `u8`, zero-extend the high dividend half before unsigned
      `div`, select quotient or remainder as requested, and canonicalize every
      `u8` result.
- [x] For `i64`, test the `i64::MIN` and `-1` pair before `idiv`; synthesize
      `i64::MIN` for division and zero for remainder on that successful path so
      no raw processor exception is possible.
- [x] For all other `i64` inputs, use signed division and correct a nonzero
      truncating remainder whose sign differs from the divisor by decrementing
      the quotient and adding the divisor to the remainder.
- [x] Preserve divisor-check dominance in selected machine control flow and
      reject any backend request that lacks the verified semantic/check shape.
- [x] Keep source syntax disabled; direct HIR/MIR integration tests and native
      harnesses execute the complete result and failure matrix.
- [x] Keep the public runtime header, `ska_rt_abi_v6`, reporter signature,
      frame contract, and public symbol set unchanged.

**Tests:** Native quotient/remainder matrices for all four `i64` sign
combinations, exact and non-exact cases, zero and one, `i64::MIN`, `i64::MAX`,
`-1`, high-bit `u64`, `u64::MAX`, and `u8` boundaries; the identity
`r = dividend - q * divisor` where representable; canonical `u8`; literal and
dynamic zero divisors with exact stderr and exit; explicit proof that
`i64::MIN / -1` and `% -1` return without a signal; static-message pool
stability; assembly ordering showing the zero and signed-overflow guards before
`div`/`idiv`, signed correction after ordinary `idiv`, system assembler
acceptance, and repository gates.

**Exit criteria:** Verified operations execute with the frozen quotient,
remainder, and panic semantics for every integer kind; neither zero divisors
nor the signed overflow pair can reach a hardware trap; source remains
unchanged.

### DR3 — Enable the complete source family end to end

**Purpose:** Activate `/` and `%` only after every selected source expression
has a verified, executable downstream path.

- [x] Lex `/` and `%` with exact spans while preserving `//` comments,
      whitespace distinctions such as `/ /`, invalid-token recovery, and all
      existing punctuation behavior.
- [x] Extend the left-associative multiplicative parser tier from `*` to
      `*`, `/`, and `%`; preserve prefix and postfix priority, additive and
      later tiers, comparison non-associativity, nesting limits, grouping, and
      deterministic recovery.
- [x] Preserve division versus remainder identity, operand shape, grouping,
      and spans in AST and resolved IR without selecting an integer kind,
      failure edge, or target instruction early.
- [x] Select each operation only for two identical `i64`, `u64`, or `u8`
      operands, returning that type. Check operands in source order and reject
      mixed integers, `bool`, `f64`, `unit`, optional, array, class,
      object-view, and shared-owner combinations before HIR.
- [x] Keep primitive operator selection cohesive in a focused private module
      rather than expanding the general primitive checker with target or
      control-flow knowledge.
- [x] Produce focused diagnostics that identify the operator and actual
      operand types without suggesting promotion, an implicit cast, or
      floating-point support.
- [x] Exercise arbitrary currently valid operands, including calls, fields,
      array access, optional unwrap, casts, assignments used by later
      expressions, nested checked shifts, nested division/remainder,
      short-circuit expressions, allocation-backed effects, and operands that
      fail before the divisor check.
- [x] Cover every current expression consumer: locals, assignments, call
      arguments, returns, fields, arrays, optional injection, comparisons,
      logical operands, and conditions.
- [x] Add deterministic token, AST, resolved, HIR, MIR, and assembly dumps plus
      source-to-native and compile-failure goldens for the complete valid,
      invalid, result, failure, precedence, effect, and cleanup matrices.
- [x] Update the living grammar, operator/type surface, status matrix,
      compiler phase/backend contracts, error catalog wording, and
      testing/debugging guidance when source support becomes active. Continue
      to report floating division/remainder and optimization as unimplemented.

**Tests:** Lexer matrices for `/`, `%`, `//`, `/ /`, adjacency, comments at
end of expressions, and malformed punctuation; parser precedence and
left-associativity across prefix, multiplicative, additive, shifts, bitwise,
comparison/`is`, `&&`, and `||`; complete valid and invalid type matrices;
phase-dump determinism; source native edge values and exact zero-panics;
call-based evaluation order, failure-before-check, successful cleanup, and
every-consumer goldens; `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Every frozen integer `/` and `%` expression, including
arbitrary valid operands, has one typed, verified, deterministic, executable
path; every excluded operand shape fails before HIR; comments and existing
precedence remain stable; and living documentation advertises exactly the
implemented integer family.

### DR4 — Harden and promote the complete family

**Purpose:** Close the family with exhaustive boundary coverage, stable
observations, maintainable ownership, and no rollout-only structure.

- [ ] Complete valid and invalid matrices across literals, grouped values,
      bindings, parameters, fields, calls, casts, arrays, optional unwrap,
      assignments, comparisons, logical operands, conditions, and returns.
- [ ] Prove all precedence and associativity boundaries, including mixed
      `*`/`/`/`%` chains, prefix/postfix composition, additive and shift
      boundaries, bitwise tiers, comparison/`is`, and `&&`/`||`, plus
      deterministic rejection of malformed chains and incomplete operators.
- [ ] Complete native extrema and property-style coverage for floor quotient,
      divisor-signed remainder, quotient/remainder identity, unsigned high-bit
      values, `u8` canonicality, both zero failures, and the signed overflow
      pair without relying on compile-time folding.
- [ ] Add independent-process determinism coverage for diagnostics and all
      phase dumps plus stable assembly, panic-message, and native observations
      where focused owner tests do not already provide it.
- [ ] Audit touched lexer, syntax, resolution, type-check, HIR, MIR lowering,
      control-effect, verifier, termination, backend, facade, dump, and test
      owners by responsibility. Resolve small high-value maintainability
      improvements and place material out-of-scope findings in a separately
      indexed discoveries document.
- [ ] Remove roadmap codes and rollout wording from living code, tests, and
      general documentation; retain milestone vocabulary only in roadmap and
      archive documents.
- [ ] Confirm the runtime ABI non-change, existing panic-message stability,
      documentation links, artifact cleanliness, and the exclusion of
      floating operations and optimization.
- [ ] Mark this roadmap complete, move it to `docs/archive/`, update the active
      and archive indexes, and repair incoming relative links.

**Tests:** Exhaustive source, HIR, MIR, verifier, backend, compile-failure,
native-value, native-panic, precedence, comment-lexing, every-consumer,
property, and determinism matrices; focused documentation and link checks;
`make check`, `make msrv-check`, `make robustness-long`, `git diff --check`,
and an artifact-free final full gate.

**Exit criteria:** Exact-width integer division and remainder are complete
deterministic contracts from source through native x86-64; defined edge cases
never leak target traps; existing arithmetic, bitwise, shift, comparison,
logical, ownership, cleanup, failure, comment, and ABI behavior remains intact;
living documentation contains no rollout state; and the completed roadmap is
archived.

## Ordering and dependencies

The semantic model comes first because floor division, operation-specific
failure, and the signed overflow pair must not be inferred from native
instructions. Checked lowering and verification follow as a separate task so
evaluation order, carriers, dominance, cleanup, and terminal failure can be
reviewed without backend mechanics. Native execution then proves every result
and failure case before source syntax can select it. Source activation is one
complete vertical slice for arbitrary valid operands, followed by broad
hardening and archival.

This roadmap depends on the completed primitive integer, eager boolean,
short-circuit boolean, bitwise, and checked-shift work; the implemented common
panic reporter and append-only static termination pool; canonical scalar and
`u8` boundaries; the frozen primitive operator profile; and the existing
full-expression lifetime contract. It adds no dependency on floating-point
division or remainder, constant evaluation, or an optimization pipeline.

# Remaining Floating-Point Operators Roadmap

Status: in progress; FP0 and FP1 are complete, and FP2 is next.

This roadmap completes the remaining frozen primitive operators for `f64`:
IEEE-754 binary64 division followed by unordered equality and ordering. The
two families share exact-type selection, eager evaluation, raw-bit floating
storage, verified scalar MIR, and the x86-64 SSE path, but remain separate
semantic milestones because division produces exceptional values while
comparisons must classify those values without accidentally imposing a total
order.

The governing language contract is the
[frozen primitive operator profile](../language/TYPES_AND_VALUES.md#frozen-primitive-operator-profile),
the source shape is the
[frozen primitive-operator expression extension](../language/GRAMMAR.md#frozen-primitive-operator-expression-extension),
and feature maturity is owned by the
[language status matrix](../language/STATUS.md#frozen-language-designs).

## Scope and invariants

- Binary `/` accepts two exact `f64` operands and returns `f64`. The operation
  performs no implicit cast, promotion, narrowing, integer-to-floating
  conversion, truthiness conversion, or expected-type reinterpretation.
- Floating division follows IEEE-754 binary64 behavior in the default
  round-to-nearest, ties-to-even environment. Overflow, gradual underflow,
  subnormal results, signed zero, infinity, and NaN remain ordinary floating
  outcomes.
- A positive or negative zero divisor never reaches the integer-division
  panic path and never creates another compiler-known failure. Nonzero divided
  by zero produces the appropriately signed infinity; zero divided by zero
  produces NaN.
- `==`, `!=`, `<`, `<=`, `>`, and `>=` accept two exact `f64` operands and
  return canonical `bool`. Existing exact integer comparisons and boolean
  equality remain unchanged.
- Floating comparisons are unordered when either operand is NaN: `==` is
  false, `!=` is true, and all four ordering predicates are false. This rule
  applies with NaN in either operand position.
- Positive and negative zero compare equal. Neither is less than or greater
  than the other. Negative infinity orders below every finite value, positive
  infinity orders above every finite value, and equal infinities compare
  equal.
- The language does not define a total floating order and does not promise a
  NaN payload, NaN sign, signaling-state preservation, or observable
  floating-point status flags. Tests observe contract-level results rather
  than target-specific NaN payload accidents.
- Every eager binary left operand evaluates exactly once before the right
  operand, and both operands complete before the operation. Completed
  temporaries retain the existing enclosing full-expression lifetime and are
  cleaned in reverse completion order.
- Floating division and comparison are pure scalar rvalues. They add no
  language-level branch, check, failure edge, cleanup action, runtime call, or
  exceptional capability. A target may use local flag-to-boolean instruction
  sequences for comparison without introducing source-visible control flow.
- `/` remains in the existing left-associative multiplicative tier with `*`
  and `%`. All six comparisons remain in the existing non-associative unified
  comparison/`is` tier. Lexer, parser, AST, and resolved-IR operator identities
  and source spans are already present and must remain stable.
- Typed HIR and MIR retain the exact target-independent floating operation,
  operand types, and result type. Floating comparison results are canonical
  booleans; neither representation exposes SSE registers, condition flags, or
  instruction mnemonics.
- MIR verification proves exact operand and result types, definition before
  use, and canonical boolean production. The x86-64 backend mechanically
  realizes only verified MIR.
- Source support for a family is not advertised as implemented until exact
  selection, diagnostics, HIR, verified MIR, x86-64 lowering, dumps, native
  edge cases, arbitrary valid operands, and living documentation agree for
  the entire family.
- The runtime header, `ska_rt_abi_v6`, frame contract, compiler-known panic
  catalog, and public symbol set remain unchanged. Existing raw-bit `f64` and
  boolean observation functions are sufficient for native golden tests.
- Floating remainder, power, implicit numeric promotion, mixed-type
  comparison, total ordering, configurable rounding or exception modes,
  constant folding, algebraic reassociation, compound assignment, and
  user-defined operators are excluded.

### Settled representation boundary

Floating division HIR and MIR use the existing eager floating arithmetic
shape with one additional semantic operation identity. It is explicitly
non-failing even when the divisor is zero. MIR must not borrow the checked
diamond, termination reason, or control-affecting classification used by
integer division and remainder. The x86-64 target realizes the verified
operation with scalar binary64 division while the portable model remains
independent of SSE register and opcode choices.

Floating comparison extends the existing semantic comparison predicate with
an exact `f64` operand flavor. The predicate and floating operand flavor must
survive HIR, MIR, dumps, verification, and instruction selection explicitly;
unordered behavior must not be inferred from an integer signedness path or a
single target condition code. Every predicate's native lowering accounts for
the unordered flag and produces an ordinary canonical `bool`.

The parser already recognizes `/` and all six comparison spellings at their
frozen precedence. Activation therefore occurs in exact operator selection,
not through another grammar rollout. In particular, the existing integer
division selector must admit exact `f64 / f64` without weakening its checked
integer contract, and the existing comparison selector must add `f64` without
admitting boolean ordering or mixed primitive types.

Niflheim supplies useful implementation evidence: its x86-64 path uses scalar
binary64 division and an unordered comparison followed by explicit parity
gating for every predicate. Skald retains its own authoritative semantic IR,
verifier, register model, operand ordering, and canonical-boolean rules; the
evidence is a warning against treating floating comparison as an ordinary
integer condition-code mapping, not a representation to copy wholesale.

## Repository gates

Every task runs its focused tests, `make check`, and `git diff --check`. Tasks
that change Rust phase models, accepted source programs, public facades, or
backend code also run `make msrv-check`. The closing task additionally runs
`make robustness-long` and the full gates from an artifact-free snapshot or
clean checkout.

Tests stay with their owner: exact selection and diagnostics belong with type
checking; HIR/MIR operations, lowering, dumps, and verifier mutations remain
colocated with those phases; target legality, selection, assembly, and native
execution remain in the x86-64 backend; reusable source observations belong in
the top-level compile-failure and native golden corpus. Runtime tests prove
the ABI remains unchanged rather than adding operator-specific C harnesses.

## Progress

- [x] FP0 — Establish executable IEEE floating division
- [x] FP1 — Enable floating division from source end to end
- [ ] FP2 — Establish executable unordered floating comparisons
- [ ] FP3 — Enable floating comparisons from source end to end
- [ ] FP4 — Harden and promote the complete primitive-operator profile

## PR-sized implementation sequence

### FP0 — Establish executable IEEE floating division

**Purpose:** Complete the target-independent and x86-64 path for non-failing
binary64 division before source selection can depend on it.

- [x] Add a cohesive floating division identity to HIR and MIR, including
      exact `f64` operand/result queries and a deterministic mnemonic, while
      reusing the established eager floating arithmetic representation.
- [x] Classify division as a pure, non-failing scalar operation. It must not
      acquire the integer division/remainder zero-check capability,
      control-affecting lowering, termination reason, or panic metadata.
- [x] Lower both operands exactly once from left to right and preserve the
      existing spill/security behavior when the right operand contains control
      flow or either operand carries full-expression temporaries.
- [x] Extend MIR verification for exact `f64` operands and result,
      definition-before-use, block-local use, and operation identity, with
      deterministic one-invariant diagnostics for malformed programs.
- [x] Extend the private x86-64 machine model, legality checks, register-use
      model, instruction selection, emission, and dumps with scalar binary64
      division; keep target opcode and register details below MIR.
- [x] Exercise finite, signed-zero, subnormal, overflow, underflow, infinity,
      and representative NaN inputs and results. Assert exact raw bits only
      where the contract fixes them; classify NaN without pinning a payload,
      sign, or signaling state.
- [x] Keep source selection of `f64 / f64` disabled in this task. Direct HIR,
      MIR, and backend fixtures establish the complete downstream path while
      the accepted source subset remains unchanged.
- [x] Preserve existing integer division/remainder checks and failures,
      floating addition/subtraction/multiplication/negation, cleanup, runtime,
      and ABI observations.

**Tests:** Direct HIR/MIR operation and dump tests; verifier mutations for
wrong operation, operand, result, or value-definition types; backend legality,
selection, and assembly tests; native exact-bit cases for ordinary exact and
rounded quotients, every contract-fixed sign combination involving zero and
infinity, finite/subnormal boundaries, overflow, and underflow; classification
without payload assertions for `0.0 / 0.0`, infinity divided by infinity, and
NaN operands; exactly-once operand traces; existing source rejection; system
assembler acceptance and repository gates.

**Exit criteria:** Verified HIR, MIR, and x86-64 can represent and execute every
frozen floating-division category with IEEE outcomes and no panic or semantic
failure edge, while source availability remains unchanged.

### FP1 — Enable floating division from source end to end

**Purpose:** Activate exact `f64 / f64` only after every selected source
expression has a verified and executable downstream path.

- [x] Route `/` selection cleanly between exact floating division and the
      existing exact integer family. Admit only `f64 / f64`; preserve the
      integer checked semantics and reject every mixed or nonnumeric pair.
- [x] Check operands in source order and produce focused diagnostics that name
      `/` and both actual operand types without suggesting implicit promotion,
      a cast, integer semantics, or a zero-divisor failure.
- [x] Preserve existing tokens, comment recognition, multiplicative
      associativity, precedence, grouping, comparison-chain rejection, source
      operator identity, and exact spans; add focused regression tests rather
      than changing the already-active grammar.
- [x] Carry selected division through deterministic HIR and MIR dumps using
      portable floating vocabulary and no target instruction details.
- [x] Exercise arbitrary valid operands and consumers, including bindings,
      calls, fields, array access, primitive optional unwrap, nested
      arithmetic, arguments, returns, assignments, comparison inputs once
      available, and cleanup-bearing receivers or allocation-backed effects.
- [x] Prove left-to-right exactly-once evaluation and existing
      full-expression cleanup when a division operand contains a call,
      selected-path control flow, a checked operation, or an owning temporary.
- [x] Add source-native exact-bit golden coverage for positive and negative
      infinity, signed zero, subnormal, overflow, and underflow. Exercise NaN
      production without pinning its bits, and prove literal and dynamic zero
      divisors complete without panic output or abnormal process status.
- [x] Update living language and compiler documentation to advertise floating
      division as implemented while leaving floating comparisons explicitly
      frozen and pending.

**Tests:** Exact type-selection matrix and diagnostics for `f64`, mixed
numeric, boolean, unit, optional, array, class, object-view, and shared-owner
operands; precedence and comment regressions; HIR/MIR snapshots; arbitrary
operand/consumer and cleanup traces; top-level native exact-bit goldens for
contract-fixed IEEE results plus payload-independent NaN execution; repeated
assembly/native determinism; integer division failure regressions and
repository gates.

**Exit criteria:** Every valid source `f64 / f64` expression executes with the
frozen IEEE behavior in every supported consumer, zero divisors cannot reach a
panic path, invalid pairs fail before HIR, and documentation identifies only
floating comparisons as still pending in the frozen profile.

### FP2 — Establish executable unordered floating comparisons

**Purpose:** Make unordered semantics explicit and verified before source
selection can expose any floating predicate.

- [ ] Extend HIR and MIR comparison operands with an exact floating flavor
      shared by `==`, `!=`, `<`, `<=`, `>`, and `>=`; retain the predicate,
      exact `f64` operands, canonical `bool` result, and deterministic
      mnemonic explicitly.
- [ ] Keep floating comparisons eager, pure, and non-failing. Reuse existing
      left-to-right operand lowering and full-expression lifetime behavior
      without introducing semantic branch blocks or runtime calls.
- [ ] Extend MIR verification to reject predicate/operand-flavor mismatches,
      non-`f64` operands, non-boolean results, noncanonical boolean producers,
      undefined values, and block-local misuse deterministically.
- [ ] Extend the private x86-64 machine model, legality checks, flag/register
      uses, selection, emission, and dumps with an unordered scalar comparison
      and the minimal byte operations needed to form a canonical boolean.
- [ ] Define each native predicate explicitly from ordered/unordered target
      flags: equality requires ordered-and-equal; inequality accepts
      unordered-or-not-equal; all ordering predicates require ordered plus the
      requested relation.
- [ ] Verify target operand order for all asymmetric predicates and ensure NaN
      in either source position follows the same unordered truth row. Do not
      derive one predicate by unsafe inversion of another unless unordered
      behavior remains explicit and proven.
- [ ] Keep source selection of all six `f64` comparisons disabled in this
      task. Direct HIR/MIR/backend construction provides the closed truth-table
      and malformed-model coverage first.
- [ ] Preserve exact signed and unsigned integer comparison lowering, boolean
      equality, `is`, non-associative syntax, control-flow consumers, runtime,
      and ABI observations.

**Tests:** Direct HIR/MIR matrices for every predicate; verifier mutations for
wrong flavor, predicate, operand/result type, boolean canonicality, and value
definition; backend assembly assertions for unordered comparison, parity
gating, operand direction, and canonical extension; native truth tables for
finite less/equal/greater pairs, `+0.0` versus `-0.0`, both infinities,
representative quiet and signaling NaN encodings with differing payload bits
in both positions, and NaN versus NaN; observe only predicate results and not
floating-point status flags; exactly-once operand traces; existing source
rejection; system assembler acceptance and repository gates.

**Exit criteria:** Verified HIR, MIR, and x86-64 execute all six predicates
with the complete unordered truth table and canonical boolean results, while
source availability remains unchanged.

### FP3 — Enable floating comparisons from source end to end

**Purpose:** Expose the proven predicates as one complete source family and
compose them with every existing boolean and control-flow consumer.

- [ ] Select all six predicates for exactly two `f64` operands while
      preserving exact integer comparison and boolean equality matrices.
      Continue rejecting mixed primitives and boolean ordering before HIR.
- [ ] Check operands in source order and produce focused, stable diagnostics
      that identify the predicate and actual types without suggesting
      promotion, truthiness, or total ordering.
- [ ] Preserve the existing unified non-associative comparison/`is` parser
      tier, grouping requirements, operator identity, exact spans, recovery,
      and precedence against bitwise, logical, cast, prefix, and postfix
      expressions.
- [ ] Carry selected predicates through deterministic resolved, HIR, and MIR
      dumps with exact floating flavor and canonical `bool` result.
- [ ] Exercise arbitrary valid operands, including calls, fields, arrays,
      optional unwrap, nested arithmetic and division, checked expressions,
      shared/object receivers, and cleanup-bearing effects; exercise results
      in bindings, arguments, returns, assignments, equality, `!`, `&&`,
      `||`, `if`/`elif`, and `while`.
- [ ] Prove both operands execute once left to right, all selected-path
      temporaries survive to the enclosing full-expression boundary, and
      logical short-circuiting still skips an unselected comparison operand
      subtree completely.
- [ ] Add source-native goldens that generate infinity and NaN through the now
      implemented division operator, cover NaN in both positions for all six
      predicates, and observe signed-zero and infinity behavior without
      relying on unavailable literal spellings.
- [ ] Update living documentation to advertise floating equality and ordering
      as implemented and remove language claiming any operation inside the
      frozen primitive matrix remains unavailable.

**Tests:** Exact selection and diagnostic matrices; comparison-chain,
precedence, and recovery regressions; HIR/MIR dumps; arbitrary operands and
every boolean/control-flow consumer; cleanup and short-circuit traces; native
truth tables built from source-generated exceptional values; repeated
cross-process assembly and native determinism; integer/boolean comparison and
`is` regressions; repository gates.

**Exit criteria:** Every valid source floating comparison produces the frozen
unordered canonical boolean in every supported consumer, invalid pairs fail
before HIR, exceptional values compose from division without special syntax,
and the entire frozen primitive operator matrix is source-executable.

### FP4 — Harden and promote the complete primitive-operator profile

**Purpose:** Close the cohesive effort only after division, comparisons, and
the previously implemented operator families remain correct together at every
phase boundary.

- [ ] Audit the complete primitive operator matrix against the frozen design,
      grammar, status matrix, HIR/MIR representations, verifier, x86-64 target,
      diagnostics, dumps, and golden corpus; close every mismatch rather than
      leaving rollout exceptions.
- [ ] Add compact table-driven and property coverage: exact division raw-bit
      cases; the complete unordered comparison truth row; trichotomy for
      ordered unequal/equal finite values and infinities; signed-zero equality;
      predicate duals only where IEEE unordered semantics makes them valid;
      and canonical boolean results in all consumers.
- [ ] Stress nested compositions of floating division and comparison with
      eager arithmetic, checked integer operations, short-circuit logic,
      optional unwrap, arrays, calls, object/shared receivers, allocation,
      return, assignment, conditions, loops, and full-expression cleanup.
- [ ] Confirm deterministic diagnostics and phase dumps under repeated and
      cross-process compilation, including multiple NaN bit patterns without
      promising result payload preservation.
- [ ] Audit facade-oriented Rust module ownership. Perform small local cleanup
      directly; record only genuinely larger, out-of-scope maintainability
      work in a focused discoveries file with evidence, impact, and a
      post-roadmap recommendation.
- [ ] Update the implemented operator table and prose, grammar availability,
      compiler/backend documentation, status matrix, language overview, and
      roadmap index so the complete primitive operator profile is an
      implemented contract and only explicitly deferred operator work remains
      open.
- [ ] Verify the runtime ABI version, public header, symbol inventory, panic
      catalog, and message ordering are byte-for-byte unchanged by the
      floating families.
- [ ] Remove temporary rollout-only fixture rewriting, comments, exclusions,
      and task-code references that no longer describe the finished compiler;
      keep permanent semantic tests and documentation task-code-free.
- [ ] Run focused suites, `make check`, `make msrv-check`,
      `make robustness-long`, `git diff --check`, and the full gates from an
      artifact-free snapshot or clean checkout; archive this roadmap only
      after all checks pass and no pending discovery blocks the contract.

**Tests:** Full exact-type and predicate matrices; division and comparison
edge/property suites; arbitrary operand/consumer and cleanup stress; integer,
boolean, `is`, short-circuit, checked-operation, ABI, panic-catalog, assembler,
and native regressions; deterministic golden output; robustness and clean-tree
gates.

**Exit criteria:** The complete frozen primitive operator design is an
implemented, documented, verified, deterministic, and natively tested
contract; floating division and comparisons leave no partial phase path or
rollout residue; the runtime ABI and failure catalog are unchanged; and this
roadmap is archived.

## Ordering and dependencies

The frozen operator design, implemented expression grammar, raw-bit `f64`
model, SSE scalar/ABI path, eager operand lowering, canonical boolean model,
and earlier integer/boolean operator roadmaps are prerequisites and are
already complete. No other active roadmap blocks this work.

FP0 proves the non-failing division representation and target behavior before
FP1 admits source programs that can select it. That completed source division
then provides the ordinary language mechanism used by FP3 goldens to construct
infinities and NaNs. FP2 may begin after FP0's shared floating machine-model
changes settle, but comparison source activation still waits for its own full
unordered downstream path. FP4 depends on both semantic milestones and is the
only task allowed to promote the complete primitive-operator profile or
archive this roadmap.

## Completion definition

This roadmap is complete only when all of the following are true:

- exact `f64 / f64` executes with the frozen IEEE-754 binary64 behavior,
  including non-panicking positive and negative zero divisors;
- all six exact `f64` comparisons execute with unordered NaN semantics,
  signed-zero equality, ordinary infinity ordering, and canonical booleans;
- arbitrary valid operands and every existing result consumer preserve
  exactly-once evaluation, selected-path behavior, and full-expression
  cleanup;
- syntax, diagnostics, HIR, verified MIR, target lowering, assembly, native
  results, deterministic dumps, and living documentation agree;
- integer and boolean operators, `is`, runtime ABI, and compiler-known panic
  behavior have no regressions; and
- the status matrix promotes the complete primitive operator profile from
  frozen design to implemented contract, leaving floating remainder and the
  other explicitly deferred operators outside this roadmap.

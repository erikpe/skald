# Operator Semantics Design Proposal

Status: proposed; O1 through O12 contain recommended decisions awaiting
confirmation. This proposal must be iterated, confirmed, and promoted into
living language and compiler contracts before any operator implementation
roadmap is written.

This proposal defines a coherent source and semantic model for Skald operators.
It expands the deliberately small implemented primitive surface without
confusing planned language meaning with current compiler support. Niflheim
provides useful evidence for operator coverage, signed division, shifts, and
short-circuit lowering, but Skald retains its own exact-type model,
non-chainable comparisons, deterministic evaluation order, ownership, cleanup,
and panic contracts.

The proposal deliberately separates:

- decisions that should become one frozen operator contract;
- representation invariants needed before implementation can be divided into
  independent roadmaps;
- current compiler behavior, which remains authoritative in the living
  documentation until promotion and implementation; and
- deferred design areas that must not delay freezing the initial operator
  contract.

## Intended outcome

The frozen operator design should provide:

- one explicit operator and operand-type matrix for all five primitive types;
- no implicit numeric conversion, promotion, truthiness, or expected-type
  reinterpretation at operator boundaries;
- deterministic precedence, associativity, evaluation order, temporary
  lifetime, and failure behavior;
- wrapping two's-complement `i64` arithmetic rather than target-dependent
  signed overflow;
- integer division and remainder with floor-division semantics;
- checked shift counts with portable signed and unsigned shift meaning;
- conventional IEEE-754 binary64 division and unordered comparison behavior;
- mandatory short-circuit evaluation for boolean conjunction and disjunction;
- one non-associative comparison tier containing contextual `is`;
- target-independent semantic operations that do not expose x86-64 accidents;
  and
- enough separation for later roadmaps to implement selected operator families
  without reopening shared source semantics.

Freezing this proposal does not make an operator implemented. The
[status matrix](../language/STATUS.md) remains the sole authority for compiler
availability.

## Current boundary

The implemented language has primitive `i64`, `u64`, `u8`, `f64`, and `bool`
values. Its current operator surface is:

- exact-type binary `+`, `-`, and `*` for the four numeric types;
- unary `-` for `i64` and `f64`;
- exact-type `==`, `!=`, `<`, `<=`, `>`, and `>=` for the three integer types;
- all nine explicit casts among `i64`, `u64`, and `u8`;
- prefix `*` for explicit shared dereference;
- postfix `!` for checked optional unwrap; and
- contextual type and presence tests through `is`.

The current grammar has postfix, unary/cast, multiplicative, additive,
comparison, and `is` levels. Comparisons and `is` are separately
non-associative, with `is` binding more weakly than integer comparison. The
implemented behavior remains defined by
[Types, Values, and Expressions](../language/TYPES_AND_VALUES.md),
[the grammar](../language/GRAMMAR.md), and
[Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md).

The current compiler does not recognize most proposed punctuation, has no
short-circuit expression representation, and has no target-independent
division, remainder, bitwise, shift, floating-comparison, or boolean logical
operation. Those are implementation facts, not constraints on the proposed
design.

## Design principles

1. **Exact types remain visible.** Operator selection does not insert a cast or
   reinterpret a literal from its spelling-selected type.
2. **Source meaning is target-independent.** Hardware masking, flags, traps,
   and undefined or implementation-defined host-language behavior do not define
   Skald results.
3. **Evaluation remains deterministic.** Operands evaluate exactly once in
   source order, except that a short-circuited right operand is not evaluated.
4. **Failure is semantic.** A source-reachable invalid divisor or shift count
   uses Skald's compiler-known panic boundary rather than a raw target trap.
5. **Optimization preserves abstract execution.** Constant folding and
   algebraic simplification must preserve value bits, NaN behavior, panics,
   evaluation, and cleanup.
6. **Freezing and delivery are separate.** The complete design freezes
   together, while implementation may proceed through several independently
   reviewed roadmaps.

## Decision register

Every decision in this register must be confirmed or deliberately revised
before promotion. An implementation roadmap must not treat a recommended row
as frozen.

| ID | Decision | Recommended direction | State |
|---|---|---|---|
| [O1](#o1--initial-operator-surface) | Initial operator surface | Freeze the primitive matrix below; do not include power or object operators | **Recommended** |
| [O2](#o2--exact-type-selection) | Operand compatibility and cast extension boundary | Select operators from actual static types only; shifts are the sole mixed-type shape, and future explicit casts complete before selection | **Recommended** |
| [O3](#o3--precedence-associativity-and-is) | Precedence and `is` | Use bitwise-before-comparison precedence and one non-associative comparison/`is` tier | **Recommended** |
| [O4](#o4--evaluation-and-temporary-lifetimes) | Evaluation and cleanup | Left-to-right, exactly once; logical RHS conditional; all completed temporaries live to the enclosing full-expression boundary | **Recommended** |
| [O5](#o5--integer-overflow) | Integer overflow | Wrap `i64`, `u64`, and `u8` arithmetic modulo their width | **Recommended** |
| [O6](#o6--integer-division-and-remainder) | Division edge behavior | Floor division, divisor-sign remainder, panic on zero, wrap `i64::MIN / -1` | **Recommended** |
| [O7](#o7--bitwise-and-shift-semantics) | Bitwise and shifts | Integer-only bitwise operations; `u64` shift count; panic at or above the left width | **Recommended** |
| [O8](#o8--floating-point-semantics) | Floating arithmetic and comparisons | IEEE-754 binary64 division and unordered comparisons; no floating `%` | **Recommended** |
| [O9](#o9--boolean-operators) | Boolean operations | `!`, short-circuit `&&`/`||`, and exact `bool` equality | **Recommended** |
| [O10](#o10--panic-and-diagnostic-boundaries) | Failure and diagnostics | Extend the compiler-known panic catalog; keep compile-time wording non-normative | **Recommended** |
| [O11](#o11--compiler-representation-boundary) | HIR, MIR, and optimization | Typed eager operations; structured short-circuit HIR lowered to verified CFG; no runtime-owned operator semantics | **Recommended** |
| [O12](#o12--promotion-and-delivery-boundary) | Freeze and roadmap ordering | Promote the whole design first, then create separate implementation roadmaps | **Recommended** |

## Proposed source surface

### Unary operators

| Operator | Operand | Result | Meaning |
|---|---|---|---|
| `-` | `i64` | `i64` | Wrapping two's-complement negation |
| `-` | `f64` | `f64` | IEEE-754 sign negation |
| `!` | `bool` | `bool` | Logical negation |
| `~` | `i64`, `u64`, or `u8` | Operand type | Bitwise complement within the operand width |
| `*` | supported `shared T` owner | Existing pointee-place result | Existing explicit shared dereference |

Unary `+` is not part of the initial operator surface. It is rejected rather
than acting as an identity operation.

Prefix `!` and postfix `!` are distinct by position:

```ska
!condition // boolean negation
optional!  // checked optional unwrap
```

### Binary primitive operators

| Operators | Left operand | Right operand | Result |
|---|---|---|---|
| `+`, `-`, `*`, `/` | One numeric type | The identical type | The operand type |
| `%` | One integer type | The identical type | The operand type |
| `&`, `|`, `^` | One integer type | The identical type | The operand type |
| `<<`, `>>` | `i64`, `u64`, or `u8` | `u64` | The left type |
| `==`, `!=` | One primitive type | The identical type | `bool` |
| `<`, `<=`, `>`, `>=` | One numeric type | The identical type | `bool` |
| `&&`, `||` | `bool` | `bool` | `bool` |

Here, numeric means `i64`, `u64`, `u8`, or `f64`; integer means `i64`, `u64`,
or `u8`.

Boolean values have logical operations and equality, but no ordering,
arithmetic, shifts, or bitwise operations. `unit` has no operator. This
proposal adds no primitive operation to optionals, arrays, class values,
object views, or shared-owner handles.

## O1 — Initial operator surface

**Question:** Which operators belong to the first frozen primitive profile?

**Recommended decision:** Freeze exactly the unary and binary matrices above.
This includes source and semantic behavior even when a particular operator is
implemented later.

Power is deliberately absent. The proposal does not reserve `**`, choose an
exponent type, establish precedence, or settle negative-exponent and overflow
behavior.

Floating remainder is also absent from the profile. `%` is integer-only; a
future design may add a floating operation or library function after choosing
between truncating remainder, IEEE remainder, and other conventions.

## O2 — Exact-type selection

**Question:** Can operator selection convert either operand, and may a future
expansion of explicit casts alter operator selection?

**Recommended decision:** No operator in this profile performs an implicit
cast, promotion, narrowing, signedness change, boolean conversion, truthiness
conversion, or expected-type reinterpretation.

Except for shifts, both operands must have the identical static type. Shift
operators require an integer left operand and exactly `u64` on the right.

The boundary between casts and operators is frozen as follows:

- an explicit cast evaluates as part of its operand expression according to
  the cast's own contract;
- when the cast succeeds, it produces an ordinary value with exactly its
  declared target type before the surrounding operator is selected or
  executed;
- operator selection observes only the operands' resulting static types, not
  whether a value came from a literal, binding, call, cast, or other
  expression;
- an operator never inserts, requests, or selects a cast;
- adding a new explicit source/target cast pair may make more operand
  expressions constructible, but does not add a mixed-type operator case or
  change the result type or behavior of an existing operator; and
- the explicit cast matrix may therefore be designed and implemented
  independently after this operator design is frozen, including after some or
  all frozen operators are implemented.

Examples:

```ska
1 + 1u         // invalid: i64 and u64
1u8 < 2u       // invalid: u8 and u64
true == false  // valid
1.0 == 1.0     // valid f64 comparison
1u8 << 3u      // valid; result is u8
1u8 << 3u8     // invalid; count must be u64
(i64) 1u8 + 2  // valid; the cast produces i64 before + is selected
```

The existing explicit integer-cast matrix remains unchanged. Expansion of
explicit casts is outside this proposal. A future proposal for implicit
conversion, mixed-type operator resolution, or contextual literal typing
would cross this frozen boundary and must explicitly reopen O2 rather than
presenting itself as a cast-only extension.

## O3 — Precedence, associativity, and `is`

**Question:** How do all proposed operators compose, and where does contextual
`is` belong?

**Recommended decision:** From tightest to loosest binding:

1. postfix unwrap, member access, shared member access, calls, indexing, and
   slicing;
2. prefix `-`, `!`, `~`, explicit shared dereference `*`, and primitive or
   object casts;
3. binary `*`, `/`, and `%`;
4. binary `+` and `-`;
5. `<<` and `>>`;
6. `&`;
7. `^`;
8. `|`;
9. `==`, `!=`, `<`, `<=`, `>`, `>=`, and contextual `is`;
10. `&&`;
11. `||`.

Postfix, arithmetic, shift, bitwise, and logical chains associate left to
right. Prefix operators associate right to left. The complete comparison tier
is non-associative and accepts at most one comparison or `is` suffix without
grouping.

The intended expression skeleton is:

```text
expression                = logical-or-expression
logical-or-expression     = logical-and-expression
                            {"||" logical-and-expression}
logical-and-expression    = comparison-expression
                            {"&&" comparison-expression}
comparison-expression     = bitwise-or-expression
                            [comparison-suffix]
comparison-suffix         = comparison-operator bitwise-or-expression
                          | "is" (view-target | "some" | "none")
comparison-operator       = "==" | "!=" | "<" | "<=" | ">" | ">="
bitwise-or-expression     = bitwise-xor-expression
                            {"|" bitwise-xor-expression}
bitwise-xor-expression    = bitwise-and-expression
                            {"^" bitwise-and-expression}
bitwise-and-expression    = shift-expression
                            {"&" shift-expression}
shift-expression          = additive-expression
                            {("<<" | ">>") additive-expression}
additive-expression       = multiplicative-expression
                            {("+" | "-") multiplicative-expression}
multiplicative-expression = unary-expression
                            {("*" | "/" | "%") unary-expression}
```

`is` remains a specialized type or presence test. It is not identity equality,
does not accept an ordinary expression on the right, and does not add `is not`
syntax. Negation uses explicit grouping:

```ska
value is Item && ready
optional is some || use_default()
!(value is Item)
```

The following require explicit grouping or are invalid:

```ska
first < second < third       // invalid chain
value is Item == flag        // invalid chain
(value is Item) == flag      // valid bool equality
!optional is some            // parsed as (!optional) is some; invalid by type
```

Bitwise operators bind above comparison so the common form
`flags & mask == expected` means `(flags & mask) == expected`.

## O4 — Evaluation and temporary lifetimes

**Question:** When do operands execute and when do their temporaries end?

**Recommended decision:**

- a unary operand evaluates once before its operator;
- eager binary operands evaluate exactly once, left then right;
- `&&` evaluates its left operand first and evaluates the right only when the
  left result is `true`;
- `||` evaluates its left operand first and evaluates the right only when the
  left result is `false`;
- a skipped operand performs no calls, allocation, ownership operations,
  checks, panic, or cleanup;
- every temporary completed on the selected path remains live until the
  enclosing existing full-expression boundary; and
- completed temporaries are cleaned in reverse completion order at that
  boundary.

In particular, a temporary completed while evaluating the left operand of a
logical expression remains live while an evaluated right operand runs. A
temporary belonging only to a skipped right operand never becomes live. This
extends the existing full-expression contract to path-dependent expression
evaluation rather than introducing operand-local cleanup.

When a logical expression is used as an `if`, `elif`, or `while` condition, the
complete selected expression path is cleaned before control enters either
successor, under the existing condition boundary.

## O5 — Integer overflow

**Question:** What happens when an integer arithmetic result is outside its
mathematical range?

**Recommended decision:** `i64`, `u64`, and `u8` arithmetic wraps modulo the
type width:

- `i64` retains the low 64 bits and interprets them as two's-complement;
- `u64` retains the low 64 bits as unsigned;
- `u8` retains the low 8 bits and remains canonical in `0..=255`.

This rule applies to addition, subtraction, and multiplication for every
integer type, and to unary negation for `i64`. Thus `-i64::MIN` is
`i64::MIN`. It also applies to the exceptional division result described
below and to high bits discarded by left shift.

Overflow does not panic, produce an invalid value, depend on build mode, or
expose a target overflow flag. Compile-time evaluation must use explicit
wrapping operations rather than host-language overflow.

## O6 — Integer division and remainder

**Question:** How do signed rounding, zero divisors, and
`i64::MIN / -1` behave?

**Recommended decision:**

- unsigned division uses the ordinary nonnegative quotient and remainder;
- signed division rounds the mathematical quotient toward negative infinity;
- signed remainder satisfies `r = dividend - quotient * divisor` and is zero
  or has the divisor's sign;
- integer `/` and `%` panic when the divisor is zero;
- `i64::MIN / -1` wraps to `i64::MIN`; and
- `i64::MIN % -1` is zero.

Representative signed results are:

| Expression | Result |
|---|---|
| `7 / 3` | `2` |
| `7 % 3` | `1` |
| `-7 / 3` | `-3` |
| `-7 % 3` | `2` |
| `7 / -3` | `-3` |
| `7 % -3` | `-2` |
| `-7 / -3` | `2` |
| `-7 % -3` | `-1` |
| `i64::MIN / -1` | `i64::MIN` |
| `i64::MIN % -1` | `0` |

Both operands complete in source order before the divisor check and operation.
The wrapping overflow pair must be handled deliberately before a target signed
divide instruction that would otherwise trap. Raw hardware divide faults are
not valid implementation of the source panic contract.

## O7 — Bitwise and shift semantics

**Question:** What bit widths and count rules govern bitwise operations and
shifts?

**Recommended decision:**

- `&`, `|`, `^`, and `~` operate on the exact fixed-width representation of
  `i64`, `u64`, or `u8`;
- `<<` shifts zero bits into the low end and discards high bits;
- `>>` is arithmetic for `i64` and copies the sign bit;
- `>>` is logical for `u64` and `u8` and shifts zero bits into the high end;
- the right operand is exactly `u64`;
- counts `0u..63u` are valid for `i64` and `u64`;
- counts `0u..7u` are valid for `u8`; and
- a count at or above the left width panics.

The language never masks an excessive count to the target instruction's
accepted low bits. A valid `u8` result is canonicalized after an operation.

## O8 — Floating-point semantics

**Question:** What observable binary64 behavior is frozen for `/` and
comparison?

**Recommended decision:** `f64` unary negation and `+`, `-`, `*`, and `/`
follow IEEE-754 binary64 operations in the existing round-to-nearest,
ties-to-even environment. Floating division by zero does not panic; it produces
the applicable signed infinity or NaN. Overflow, underflow, signed zero,
subnormal, infinity, and NaN results follow the corresponding binary64
operation.

Floating equality and ordering use unordered IEEE comparison:

| Condition when either operand is NaN | Result |
|---|---|
| `left == right` | `false` |
| `left != right` | `true` |
| `left < right` | `false` |
| `left <= right` | `false` |
| `left > right` | `false` |
| `left >= right` | `false` |

Positive and negative zero compare equal. Infinities participate in ordinary
numeric ordering. Operators do not provide a total order over binary64 values.

The language does not promise a particular NaN sign or payload, preservation
of signaling NaN state, or source-visible floating exception flags. An
implementation must not replace the specified unordered result with whatever
single target condition happens to be convenient.

Floating `%` is not defined by this proposal.

## O9 — Boolean operators

**Question:** Which operations exist for `bool`, and are conjunction and
disjunction eager?

**Recommended decision:** `bool` supports:

- prefix `!`;
- equality `==` and inequality `!=`;
- short-circuit conjunction `&&`; and
- short-circuit disjunction `||`.

It supports no ordering, arithmetic, shifts, or bitwise `&`, `|`, `^`, and `~`.
There is no truthiness conversion from numeric, optional, owner, array, class,
interface, `Obj`, or `unit` values.

`&&` and `||` are control-flow expressions. Treating them as eager scalar
instructions, even when both operands are statically pure, is not a valid
baseline lowering. An optimizer may simplify them only after proving that
evaluation, panic, ownership, cleanup, and result behavior remain unchanged.

## O10 — Panic and diagnostic boundaries

**Question:** How are invalid runtime operands reported, and what compile-time
diagnostics are part of the language contract?

**Recommended decision:** Integer zero divisors and excessive shift counts are
compiler-known source-reachable failures under the existing non-returning,
non-unwinding panic contract. They must carry distinct target-independent
semantic reasons through verified IR and use the common panic reporter.

Before promotion, the closed panic catalog must be extended with confirmed
exact static messages. Recommended messages are:

| Failure | Recommended static message |
|---|---|
| Integer division by zero | `integer division by zero` |
| Integer remainder by zero | `integer remainder by zero` |
| Shift count at or above operand width | `shift count out of range` |

No remaining source-level cleanup is guaranteed after reporting begins.
Floating division by zero and integer wrapping overflow do not use panic.

Compile-time rejection should identify the operator and incompatible operand
types. The language continues not to freeze a diagnostic code, exact wording,
follow-on count, or ordering between independent errors. Operator and operand
spans, deterministic recovery, and stable syntax/resolved/HIR/MIR dumps remain
compiler quality obligations.

## O11 — Compiler representation boundary

**Question:** Which internal invariants must be settled before implementation
can be divided among roadmaps?

**Recommended decision:**

- source syntax retains exact operator and operand spans;
- resolved form preserves operator identity without selecting target
  instructions;
- typed HIR selects one exact primitive operation, operand type, result type,
  and failure capability;
- eager unary and binary primitive operations remain explicit typed values;
- short-circuit boolean operations remain structured in HIR and lower to
  ordinary MIR control flow with one selected result;
- MIR represents eager scalar operations and compiler-known failure reasons
  target-independently;
- MIR verification proves operand/result types, valid operation flavors,
  canonical `bool`/`u8` results, and well-formed short-circuit control flow;
- constant folding and other optimizations use the same wrapping, division,
  shift, NaN, panic, and short-circuit rules as runtime execution;
- generated target code implements verified MIR mechanically and must not
  recover source semantics from enum names, source text, or host behavior; and
- no operator requires a new public runtime ABI entry point beyond the existing
  common panic reporter.

The exact Rust module organization, enum names, CFG block numbering,
instruction sequences, register choices, branch shapes, and optimization
algorithms remain private implementation decisions.

The path-dependent temporary state introduced by `&&` and `||` is a material
representation prerequisite. The eventual compiler design must merge selected
paths while retaining every completed temporary until the enclosing
full-expression cleanup. A roadmap must not avoid this obligation by
restricting logical operands to effect-free primitive expressions.

## O12 — Promotion and delivery boundary

**Question:** When may implementation roadmaps begin, and how may they divide
the work?

**Recommended decision:** No implementation roadmap begins until O1 through
O12 are confirmed, contradictions are resolved, and the selected rules are
promoted into living contracts.

Promotion should update, at minimum:

- the operator, overflow, comparison, and conversion boundaries in
  [Types, Values, and Expressions](../language/TYPES_AND_VALUES.md);
- the planned grammar and precedence authority in
  [the grammar](../language/GRAMMAR.md);
- evaluation order and full-expression cleanup in
  [Functions and Control Flow](../language/FUNCTIONS_AND_CONTROL_FLOW.md);
- the compiler-known panic catalog in
  [Errors and Exceptional Control Flow](../language/ERRORS.md);
- feature maturity and the implementation boundary in
  [the status matrix](../language/STATUS.md);
- phase and representation invariants in
  [Phases and Intermediate Representations](../compiler/PHASES_AND_IR.md);
- target and panic realization boundaries in
  [the backend contract](../compiler/BACKEND.md) and
  [runtime ABI](../compiler/RUNTIME_ABI.md); and
- language and compiler documentation indexes.

After promotion, separate roadmaps may be created for cohesive vertical
families such as:

- wrapping `i64` arithmetic reconciliation;
- boolean negation, equality, and short-circuit expressions;
- integer and floating division plus integer remainder;
- integer bitwise operations and checked shifts; and
- floating and boolean comparisons.

That list is not an implementation plan and does not establish order. Each
roadmap must inspect current ownership and deliver its selected family
source-to-native without silently claiming that every other frozen operator is
implemented.

## Decisions required before roadmap work

The proposal is ready for promotion only after all of the following have been
handled explicitly:

- [ ] Confirm the complete initial unary and binary operator matrix.
- [ ] Confirm that no operator performs an implicit cast or promotion, shifts
      are the only mixed-type exception, operators cannot observe cast
      provenance, and future explicit casts complete independently before
      operator selection.
- [ ] Confirm the full precedence ladder, one non-associative comparison tier,
      and contextual `is` placement and meaning.
- [ ] Confirm left-to-right evaluation and path-dependent full-expression
      temporary lifetime for `&&` and `||`.
- [ ] Confirm wrapping two's-complement `i64`, wrapping `u64`, and canonical
      wrapping `u8` arithmetic.
- [ ] Confirm floor signed division, divisor-sign remainder, zero-divisor
      panic, and the `i64::MIN / -1` and remainder results.
- [ ] Confirm bitwise width, signed/unsigned right shift, exact `u64` counts,
      and excessive-count panic.
- [ ] Confirm IEEE binary64 division, NaN comparison, signed-zero, infinity,
      and floating `%` exclusion.
- [ ] Confirm exact boolean equality, logical negation, mandatory
      short-circuiting, and absence of boolean bitwise operations.
- [ ] Confirm new compiler-known panic reasons and their exact static catalog
      messages.
- [ ] Confirm the HIR/MIR split between eager values and structured
      short-circuit control flow, including verifier and optimization
      invariants.
- [ ] Check the complete proposal for contradictions with existing optional,
      shared-owner, object-cast, temporary, panic, and evaluation-order
      contracts.
- [ ] Promote every confirmed source and representation rule into living
      documentation so implementation roadmaps consume authoritative contracts
      rather than this proposal.
- [ ] Validate documentation links and indexes, then archive this proposal as
      the historical decision record.

## Deliberately deferred decisions

The following are not freeze prerequisites and must not be decided by this
proposal or absorbed opportunistically into its later implementation
roadmaps:

- power/exponentiation syntax, token, precedence, operand types, negative
  exponents, overflow, and floating behavior;
- operators on `Str`, other class values, interfaces, `Obj`, shared owners,
  optionals, arrays, or future value families;
- user-defined operator declarations, overload resolution, protocols, traits,
  or compiler-selected method names;
- implicit numeric promotions, mixed-type arithmetic or comparison, contextual
  literal typing, and conversions involving `bool`;
- expansion of explicit casts beyond the implemented integer matrix, provided
  it preserves O2's frozen boundary between explicit conversion and operator
  selection;
- floating remainder, including a choice among truncating remainder, IEEE
  remainder, Euclidean modulo, or a library-only operation;
- total floating-point ordering, NaN sorting, NaN construction and inspection,
  payload preservation, signaling NaN behavior, and floating exception flags;
- checked, saturating, overflowing-result, arbitrary-precision, or selectable
  overflow modes;
- rotations, population counts, leading/trailing-zero counts, and other
  integer bit utilities;
- compound assignment, prefix/postfix increment and decrement, and assignment
  expressions;
- `is not`, pattern matching, type-test bindings, and new flow-sensitive
  narrowing semantics;
- identity equality or value equality for object and ownership types;
- null-coalescing, conditional expressions, pipeline operators, ranges, and
  other unrelated expression forms; and
- SIMD, vector, atomic, volatile, concurrency, or memory-model operators.

These areas may receive independent design proposals later. Nothing here
reserves their syntax or implies that eventual designs must reuse primitive
operator spelling.

## Test obligations after promotion

Later implementation roadmaps should allocate tests to the owning layer, but
the frozen design establishes these eventual coverage families:

- lexer longest-match coverage and invalid-token recovery for every spelling;
- parser precedence, associativity, grouping, prefix/postfix ambiguity, and
  complete comparison-chain rejection;
- exact operand/result type matrices and mixed-type diagnostics;
- left-to-right effects and skipped logical right operands;
- full-expression cleanup on every short-circuit path;
- wrapping boundaries for every integer width and operation;
- complete signed division/remainder sign matrices, zero divisors, and the
  signed-minimum overflow pair;
- valid and excessive shift counts at every width;
- binary64 finite, zero, infinity, and NaN comparisons;
- compiler-known panic reasons, exact reporter output, and absence of raw
  target faults;
- HIR/MIR dumps and verifier mutation tests for every operation flavor;
- constant-folded and dynamic executions producing identical results and
  failures;
- deterministic assembly, assembler acceptance, and native execution; and
- no unplanned runtime ABI symbol or version change.

Every implementation roadmap must use the repository's documented focused
tests and complete `make check` gate, plus the supported-toolchain gate when
Rust, manifests, or source syntax change.

## Promotion criteria

This proposal may be frozen and archived only when:

- O1 through O12 have explicit confirmed decisions;
- every item under
  [Decisions required before roadmap work](#decisions-required-before-roadmap-work)
  is complete;
- all deliberate deferrals remain outside the promoted contract except as
  explicit exclusions;
- living documentation contains the complete authoritative behavior without
  requiring readers to consult this proposal;
- the status matrix distinguishes frozen design from implemented availability;
- no implementation roadmap has started from an unresolved decision; and
- documentation links, indexes, and terminology have been validated.

After promotion, this file becomes a historical decision record under
`docs/archive/`. Selected operator implementation roadmaps may then be written
and scheduled independently.

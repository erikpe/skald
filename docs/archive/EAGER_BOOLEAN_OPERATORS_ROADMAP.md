# Eager Boolean Operators Roadmap

Status: complete; EB0 through EB2 are implemented and verified.

This roadmap implements the eager part of Skald's frozen boolean operator
profile: prefix logical negation and exact boolean equality and inequality. It
creates a complete source-to-native path while establishing a comparison
representation that can serve more than the already implemented integer
family without introducing short-circuit control flow.

The completed slice accepts expressions such as:

```ska
!ready
left == right
left != right
!optional_flag!
```

with the precedence, exact-type selection, evaluation order, and canonical
boolean results required by the frozen
[primitive operator profile](../language/TYPES_AND_VALUES.md#frozen-primitive-operator-profile).

## Scope and invariants

- Prefix `!` accepts exactly `bool` and produces `bool`.
- `==` and `!=` accept two `bool` operands and produce `bool`; both operands
  must already have that identical static type.
- A unary operand evaluates exactly once. Equality operands evaluate exactly
  once from left to right.
- Prefix `!` remains distinct from existing postfix optional unwrap by
  position. Postfix parsing binds first, so `!optional_flag!` means logical
  negation of the unwrapped value.
- Longest-match tokenization keeps `!=` distinct from `!` followed by `=`.
- Boolean equality remains in the existing non-associative comparison tier.
  This roadmap does not permit comparison chains.
- HIR and MIR carry selected target-independent boolean operations rather than
  source punctuation or target condition codes.
- MIR represents these operations as ordinary eager scalar rvalues. They add
  no blocks, branches, cleanup edges, termination reasons, or runtime calls.
- Every result is a canonical Skald `bool`, including results stored, returned,
  passed, printed, or consumed as a condition.
- Existing integer comparison semantics, signedness, validation, dumps, and
  native results remain unchanged.
- Source support is not advertised as implemented until syntax, type checking,
  HIR, verified MIR, x86-64 lowering, native execution, diagnostics, and living
  documentation agree.
- The public runtime ABI and its version remain unchanged.
- Short-circuit `&&` and `||` are explicitly excluded. They require structured
  HIR, expression-level CFG lowering, selected-path lifetime handling, and a
  separate roadmap.
- Floating equality and ordering, boolean casts, truthiness, object or identity
  equality, optional equality, user-defined operators, constant folding, and
  every other frozen but unimplemented operator are excluded.

### Settled representation boundary

The current integer-only comparison carrier is generalized into one cohesive
primitive-comparison operation in HIR and MIR. It carries:

- the comparison predicate;
- an explicit operand kind, initially the three integer kinds or `bool`; and
- the exact operand and result types derived from that kind.

Integer operand kinds retain all six comparison predicates. The `bool` operand
kind admits only equality and inequality. Type checking constructs only valid
operations, and MIR verification independently rejects invalid predicate/type
combinations. No future floating operand variant is added until floating
comparisons are implemented.

Logical negation is an exact boolean variant of the existing eager unary
operation family in HIR and MIR. It is not represented as equality with
`false`, a branch, a call, or an eager binary logical operation.

The compiler may share private target instruction-selection machinery for
integer and boolean equality, but target-independent IR retains the semantic
operand kind. Existing integer comparison dump vocabulary may be preserved or
migrated deliberately, but every dump must identify the predicate and exact
operand kind deterministically.

Niflheim confirms the usefulness of distinguishing logical negation, boolean
comparison, and short-circuit logical operations semantically. Skald keeps its
own frozen exact-type rules and uses its existing typed HIR, verified MIR, and
canonical scalar boundaries rather than copying Niflheim's implementation
structure.

## Progress

- [x] EB0 — Establish verified eager-boolean IR operations
- [x] EB1 — Enable eager boolean source expressions end to end
- [x] EB2 — Harden and promote eager boolean operators

## PR-sized implementation sequence

### EB0 — Establish verified eager-boolean IR operations

**Purpose:** Settle and exercise the reusable target-independent operation
boundary before source selection depends on it, while preserving the currently
accepted source language.

- [x] Generalize the integer-only HIR comparison carrier into the settled
      primitive-comparison shape, initially covering `i64`, `u64`, `u8`, and
      `bool`, without encoding target condition codes or register widths.
- [x] Generalize the corresponding MIR operation and rvalue while preserving
      integer predicate, signedness, operand-type, result-type, and
      definition-before-use invariants.
- [x] Add exact logical-negation variants to the HIR and MIR unary operation
      families with `bool` operand and result types.
- [x] Lower both operation families as pure, block-local scalar rvalues with no
      control-effect classification, failure capability, cleanup action, or
      exceptional edge.
- [x] Extend MIR verification to accept boolean equality and inequality,
      reject boolean ordering and mismatched operand/result types, and retain
      deterministic mutation diagnostics.
- [x] Extend x86-64 legality and instruction selection so logical negation and
      boolean equality produce canonical zero-or-one results through private
      target operations.
- [x] Keep source construction of the new boolean operations disabled in this
      task; direct IR and backend tests establish the complete downstream path
      before parser and type-check activation.
- [x] Update HIR/MIR facades, deterministic dumps, and debugging vocabulary for
      the cohesive comparison representation without leaking implementation
      helpers as new public API.
- [x] Preserve the runtime header, runtime archive, ABI version, frame model,
      deterministic labels, and all existing integer comparison observations.

**Tests:** Existing eighteen-case integer comparison matrices and native
boundaries; direct MIR construction for boolean equality, inequality, and
logical negation truth tables; verifier mutations for boolean ordering,
non-boolean unary operands, non-boolean results, mismatched comparison
operands, and use-before-definition; backend legality and instruction-selection
tests; canonical result storage; deterministic HIR/MIR dumps; system assembler
acceptance; `cargo test --locked -p skald-compiler`, `make check`, and
`git diff --check`.

**Exit criteria:** HIR, verified MIR, and x86-64 can represent and execute exact
boolean negation and equality operations with canonical results, existing
integer comparisons remain unchanged, and no newly valid source expression
depends on an incomplete downstream path.

### EB1 — Enable eager boolean source expressions end to end

**Purpose:** Connect the settled operations to source syntax and exact type
selection as one complete user-visible feature.

- [x] Parse `!` in the right-associative prefix tier while retaining postfix
      unwrap precedence, cast precedence, syntax nesting limits, source spans,
      recovery, and `!=` longest-match behavior.
- [x] Preserve logical-negation identity and operand shape through AST and
      resolved IR without selecting semantic or target operations early.
- [x] Select logical negation only for a `bool` operand and issue a focused
      type error for numeric, optional, class, array, shared-owner, object-view,
      and `unit` operands.
- [x] Extend equality selection so `bool == bool` and `bool != bool` produce
      the settled primitive comparison while existing same-type integer
      equality and ordering continue to select their current semantics.
- [x] Reject mixed boolean/numeric equality, boolean ordering, and every
      unsupported equality family before HIR; diagnostics identify the
      operator and both relevant actual types without implying a cast or
      truthiness conversion.
- [x] Preserve unary evaluation-once and eager equality's left-to-right,
      exactly-once evaluation through HIR and MIR lowering.
- [x] Add deterministic AST, resolved, HIR, and MIR dumps for nested negation,
      boolean equality, grouping, calls, and prefix/postfix `!` compositions.
- [x] Add source-to-native and compile-failure goldens covering truth tables,
      variables, fields, parameters, calls, returns, assignments, conditional
      use, optional-boolean unwrap followed by negation, precedence, and
      rejected type combinations.
- [x] Update the implemented grammar, types and values, status matrix,
      compiler phase/IR contract, backend contract, and relevant testing or
      debugging guidance in the same change that enables the behavior.
- [x] Keep short-circuit syntax, floating comparisons, boolean casts, runtime
      support, and unrelated operator tokens outside the change.

**Tests:** Lexer longest-match regression for `!`, `!=`, and `=`; parser,
recovery, nesting, precedence, grouping, prefix/postfix, AST, and resolved-dump
tests; unary and equality type matrices; invalid-operand diagnostics; HIR/MIR
selection and dump tests; call-based evaluation-once and left-to-right tests;
all logical-negation and boolean equality truth-table cases; canonical results
through storage, calls, returns, external boolean output, `if`, and `while`;
compile-failure and native goldens; `make check`, `make msrv-check`, and
`git diff --check`.

**Exit criteria:** Every frozen eager boolean expression has one accepted,
typed, verified, and executable source-to-native path; every excluded operand
combination fails before HIR; prefix and postfix `!` compose without ambiguity;
and living documentation reports the feature as implemented without implying
short-circuit support.

### EB2 — Harden and promote eager boolean operators

**Purpose:** Close the slice with complete boundary coverage, deterministic
observations, and no rollout-only structure before a short-circuit roadmap
builds on boolean expressions.

- [x] Complete valid and invalid operator matrices across literals, bindings,
      fields, calls, grouping, explicit integer casts around unrelated
      operands, optional unwrap, assignments, conditions, and returns.
- [x] Prove prefix/postfix and precedence boundaries including `!!flag`,
      `!optional_flag!`, `!left == right`, `left == !right`,
      `!(left == right)`, contextual `is`, and ungrouped chain rejection.
- [x] Add independent-process determinism coverage for diagnostics and phase
      dumps plus stable assembly/native observations where not already owned by
      focused tests.
- [x] Audit touched lexer, syntax, resolution, type-check, HIR, MIR, verifier,
      backend, facade, dump, and test owners by responsibility; resolve
      high-priority hotspots and place any material out-of-scope findings in
      an indexed discoveries document.
- [x] Remove roadmap codes and rollout language from living code, tests, and
      general documentation; retain them only in this historical roadmap.
- [x] Confirm documentation links, status wording, ABI non-change, repository
      artifact cleanliness, and the boundary reserving `&&` and `||` for
      structured short-circuit work.
- [x] Mark this roadmap complete, move it to `docs/archive/`, update the active
      and archive indexes, and repair incoming relative links.

**Tests:** Complete eager-boolean source and IR matrices; compile-failure and
native golden suites; independent-process diagnostic, dump, and assembly
determinism; focused documentation and link checks; `make check`,
`make msrv-check`, `make robustness-long`, `git diff --check`, and an
artifact-free final full gate.

**Exit criteria:** Prefix boolean negation and exact boolean equality are
implemented deterministic contracts from source through native x86-64,
existing operator behavior remains intact, all living documentation is
current, no short-circuit behavior has entered incidentally, and the completed
roadmap is archived.

## Ordering and dependencies

The representation task comes first because boolean equality is the second
semantic consumer of the current integer-only comparison path, and source
activation should not force an incomplete or duplicated downstream design.
The source task follows only after verified MIR and target lowering can execute
every newly selectable operation. Broad matrix and determinism hardening closes
the slice after the behavior is observable end to end.

This roadmap depends on the completed boolean/conditional-control-flow and
primitive-integer-operation roadmaps, the frozen primitive operator profile,
existing `Bang` and `BangEqual` tokenization, exact-type expression checking,
canonical boolean storage and ABI boundaries, deterministic phase dumps,
verified block-local MIR values, and x86-64 condition selection.

It does not depend on short-circuit logical expressions, path-dependent
temporary ownership, new panic reasons, runtime changes, floating comparisons,
or expansion of the explicit cast matrix. A later short-circuit roadmap may
consume the completed logical-negation and boolean-comparison expressions but
must introduce its own structured HIR and CFG/lifetime design.

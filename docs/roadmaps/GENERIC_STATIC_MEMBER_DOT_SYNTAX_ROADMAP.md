# Generic Static-Member Dot Syntax Roadmap

Status: in progress; GSD0 is complete and GSD1 is next.

Replace the generic-class static-member spelling `Class<T>::member` with
`Class<T>.member`. This makes `::` exclusively a module-path separator and
uses `.` consistently for class-member selection, whether the class is
ordinary or a closed generic specialization. The migration preserves the
existing generic specialization, static storage, call, function-value,
lifecycle, HIR, MIR, backend, and ABI contracts.

The source contract is settled by this roadmap:

```ska
Cache<i64>.count = 1;
var count: i64 = Cache<i64>.count;
var result: i64 = Factory<i64>.apply(42);
var callback: fn(i64) -> i64 = Factory<i64>.apply;
var imported: i64 = dep::Cache<i64>.count;
```

## Scope and invariants

- `.` selects every static field or static method from an ordinary or generic
  class spelling. Reading, writing, direct calling, and forming a function
  reference use the same separator.
- `::` remains the separator between module-path components. A qualified
  generic selection therefore has the form `module::Class<T>.member`.
- `Class<T>::member` is rejected. It is not retained as an alias, compatibility
  mode, or deprecated spelling.
- The parser continues to produce the dedicated
  `GenericStaticSelectionExpr`; a closed generic application is a type-level
  selection head, not a runtime receiver wrapped in ordinary member access.
- The selection node retains the exact `.` span so diagnostics and dumps do
  not lose source provenance.
- Generic construction remains `Class<T>(arguments)` and allocation remains
  `new Class<T>(arguments)`. Instance selection from constructed, returned, or
  stored values remains ordinary `.` or `->` behavior and is not changed.
- Nested generic closers remain distinct from expression `>`, `>=`, and `>>`.
  Recognizing `.member` after a closed generic head must not reinterpret
  comparisons or shifts.
- Static member kind checks, visibility, declaring-class privacy, inheritance,
  specialization requests, definition-site lookup, and per-specialization
  static identity remain unchanged.
- Closed generic static method references retain their exact specialized
  `MethodId` and canonical `FunctionTypeId`; only their source punctuation
  changes.
- Evaluation order, ownership, cleanup, static effects, body retention, panic
  traces, generated symbols, HIR, MIR, and x86-64 lowering remain unchanged.
- The lexer keeps both `Dot` and `DoubleColon`: `DoubleColon` is still required
  for module paths and is not removed from the language.
- Archived proposals and completed roadmaps remain historical records. Living
  grammar, language, compiler, development, standard-library, benchmark, and
  test sources describe or use only the new spelling.
- The change adds no public runtime surface and does not change runtime ABI
  version 9.

## Progress

- [x] GSD0 — Adopt dot selection throughout the source frontend
- [ ] GSD1 — Harden composition and close the syntax migration

## PR-sized implementation sequence

### GSD0 — Adopt dot selection throughout the source frontend

**Purpose:** Change the accepted source contract atomically while preserving
the existing dedicated generic-static-selection representation and all
downstream semantics.

- [x] Change generic-expression lookahead and parsing so a closed generic
      application followed by `.identifier` produces
      `GenericStaticSelectionExpr` with the exact dot span.
- [x] Keep module qualification in the generic head, accepting forms such as
      `dep::Factory<i64>.apply` without treating the final dot as part of the
      declaration path or as an object receiver operation.
- [x] Add a recovery-only path for `Class<T>::member` that emits one focused
      replacement diagnostic and consumes enough input to prevent misleading
      comparison, call, or missing-semicolon cascades. Recovery must not make
      the legacy spelling a valid program.
- [x] Preserve the existing generic selection node and resolver entry points
      for static-field reads, static-field assignments, direct static calls,
      static-method function references, template-dependent selections, and
      specialization requests; avoid duplicating these paths through ordinary
      `MemberAccessExpr` handling.
- [x] Migrate Skald standard-library source, benchmarks, golden fixtures, and
      Rust-embedded Skald programs in the same change so every checked source
      uses `.` and the repository remains buildable at the task boundary.
- [x] Update focused syntax, recovery, resolution, specialization, type-check,
      static-lifecycle, MIR, backend, and runtime-trace expectations affected
      by the punctuation or its source span without changing semantic output.
- [x] Update the implemented grammar and living generic-class, static-member,
      function-value, compiler, testing, and debugging documentation wherever
      they specify or demonstrate generic static selection. State explicitly
      that `::` separates module paths and `.` selects class members.

**Tests:** Focused parser and syntax-dump tests for unqualified,
module-qualified, parameter-bearing, nested-generic, read, assignment, call,
and reference forms; parser recovery tests for the rejected legacy separator;
focused resolution and specialization tests for each semantic consumer;
affected static-lifecycle, function-value, MIR, backend, standard-vector, and
generic-class goldens; `cargo fmt --all -- --check`; `make docs-check`;
`make check`; and `git diff --check`.

**Exit criteria:** `.` is the only accepted generic static-member separator;
all existing read, write, call, and reference behavior reaches the same
semantic operations and identities; module qualification and expression
operators remain intact; legacy syntax receives one useful diagnostic; all
in-tree executable sources and living documentation use the new spelling; and
the complete ordinary repository gate passes.

### GSD1 — Harden composition and close the syntax migration

**Purpose:** Prove the new punctuation across complete programs and hostile
syntax boundaries, remove stale current-facing examples, and close the change
without leaving a second spelling or downstream special case.

- [ ] Add or consolidate an end-to-end golden matrix covering static-field
      reads and writes, direct static calls, static-method function references,
      uses inside generic templates, distinct closed specializations, inherited
      members, privacy, and module-qualified generic heads.
- [ ] Add exact compile-failure coverage for `Class<T>::field` and
      `Class<T>::method`, including assignment, call, and value-reference
      positions, and assert the focused `.` replacement diagnostic without
      cascades.
- [ ] Exercise nested closers such as `Outer<Inner<i64>>.member` beside `<`,
      `>`, `>=`, and `>>` expressions; preserve generic construction followed
      by instance selection and existing malformed-angle recovery.
- [ ] Verify syntax, resolved, HIR, and MIR dumps remain deterministic and that
      closed generic function references still name the exact specialized
      method while equal signatures continue to share canonical function-type
      identity.
- [ ] Audit `.ska` files and living documentation for legacy `>::identifier`
      spellings. Exclude archived historical records and avoid mistaking Rust
      turbofish syntax such as `Vec::<T>` for Skald source.
- [ ] Confirm the standard `Vec<T>` implementation, its native/golden suites,
      and the generic-vector benchmark use and execute the new spelling.
- [ ] Recheck that no runtime header, archive, compatibility marker, ABI
      version, MIR representation, backend calling convention, or generated
      symbol spelling changed.
- [ ] Remove roadmap task codes and rollout wording from living code and docs,
      complete every checkbox, set the roadmap status to complete, move it to
      `docs/archive/`, update both roadmap indexes, and repair incoming links.

**Tests:** The focused generic-class, static-field, function-value, standard-
vector, syntax-failure, and module-qualified golden groups in full determinism
mode; cross-process compiler determinism tests covering generic selections;
`make generic-vec-benchmark`; an artifact-free `make check`;
`make golden-determinism-test`; `make msrv-check`; `make docs-check`;
`cargo fmt --all -- --check`; and `git diff --check`.

**Exit criteria:** Complete native programs prove the dot spelling in every
generic static-member role, the old separator is deterministically rejected,
operator and nested-angle parsing has not regressed, no current source or
living documentation retains the wart, deterministic compiler products and
the standard library remain sound, the runtime/ABI boundary is unchanged, and
the completed roadmap is archived.

## Ordering and dependencies

GSD0 is an atomic source migration because changing the parser without
migrating the standard library and embedded test programs would leave the
repository unable to pass its ordinary gate. It reuses the implemented generic
class, static field, static method, module, function-value, and specialization
contracts; none requires redesign.

GSD1 follows only after the source frontend and corpus agree on the new
spelling. It adds broad composition and rejection evidence against the stable
parser boundary, then performs the archive and documentation audit. No other
active roadmap blocks either task, and neither task changes runtime ABI or
backend representation.

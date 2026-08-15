# Produced-Object Field Reads Roadmap

Status: in progress; PFR3 is next.

This roadmap lets an expression that produces one exact inline class expose
its fields directly without a source-level staging local. It makes
`values[index]._length` inside `Str` obey the same hidden-temporary ownership
model already used by produced read-only method receivers, while preserving
Skald's place/value distinction, deterministic cleanup, and read-only access
boundary.

Niflheim demonstrates the source-level usefulness of reading fields from call,
cast, and indexed results. Skald remains authoritative for ownership: every
accepted producer is materialized once in caller-owned full-expression
storage, every projected field is consumed under its existing type-specific
rules, and the complete produced root is destroyed only after the field result
or immediate consumer has been secured.

## Scope and invariants

- Accept the same exact-class producer families as produced read-only method
  receivers: construction, canonical exact-class literals, exact-class results
  from direct, static, instance, interface, and structural getter calls, and
  grouping around those forms.
- Treat a produced root as read-only. Permit direct field reads and chains of
  canonical base and exact inline-class field projections; do not permit field
  writes, mutable methods, `mut ref` arguments, whole-object replacement, or
  mutation through the unnamed inline value.
- Preserve ordinary field typing and consumption. A final primitive field is a
  scalar value. A class field remains an object subplace usable only by the
  existing read-only receiver, `ref`, checked-view, copy, owning argument,
  assignment-source, and return-copy contexts. Optional, array, shared-owner,
  optional-owner, and inline-array fields retain their existing value, alias,
  transfer, copy, and explicit-dereference rules.
- Evaluate the complete producer exactly once at its ordinary source position.
  Construct it directly in one hidden temporary, register cleanup only after
  successful completion, and never synthesize a source binding or an extra
  copy merely to read a field.
- Keep the complete produced root live through the immediate field consumer
  and the enclosing full expression. Load scalar results and secure copied,
  transferred, anchored, optional, array, or exact-class results before root
  cleanup; destroy all completed temporaries exactly once in reverse completion
  order.
- Preserve selected-path behavior and failure ordering. A skipped logical path
  creates no produced storage or cleanup; a failed producer exposes no field;
  and a later failing consumer retains the existing non-unwinding boundary for
  already completed owners.
- Preserve the producer's exact complete-object origin and dynamic class while
  projecting to inherited declarations and nested inline fields. Projection
  never slices the hidden object and never changes declaring-class privacy or
  virtual/interface selection.
- Reuse `ResolvedObjectReceiver::Produced`, `HirObjectReceiver::View`,
  `HirViewSource::Produced`, ordinary field-place lowering, and the existing
  full-expression object-temporary helper. Do not add a fake binding, a second
  produced-field carrier, or target-specific lowering.
- Keep produced optional containers, produced arrays, and raw produced shared
  owners from becoming implicit dot roots. They retain their family-specific
  unwrap, indexing, ownership, and explicit-dereference syntax.
- Add no grammar form, storable reference, local alias, escape path, backend
  representation, calling-convention rule, runtime service, or runtime ABI
  version change.
- Keep dumps, diagnostics, evaluation order, identities, layout, runtime-call
  surface, and assembly deterministic.

## Progress

- [x] PFR0 — Freeze the produced-field read contract
- [x] PFR1 — Admit produced roots and nested field projections in resolution
- [x] PFR2 — Type and lower primitive and inline-object field consumers
- [ ] PFR3 — Secure owning and guarded field categories
- [ ] PFR4 — Verify lifetime, control flow, and rejection boundaries
- [ ] PFR5 — Prove native composition and publish the feature

Every Rust implementation task runs its focused phase and golden tests,
followed by `make check` and `make msrv-check`. Documentation-only PFR0 runs
`make docs-check`. The Makefile remains the repository automation interface;
this roadmap adds no repository CI.

## PR-sized implementation sequence

### PFR0 — Freeze the produced-field read contract

**Purpose:** Define the source-visible access, projection, ownership, and
lifetime rules before removing the current resolver rejection.

- [x] Update the functions/control-flow, classes/lifecycle, types/values, and
      status contracts to distinguish produced read-only field roots from
      ordinary source places.
- [x] Define the accepted exact-class producer families, grouping, structural
      getter results, inherited base projection, nested inline-field
      projection, closed-generic specialization, and declaring-class privacy.
- [x] Define the final-field category matrix: primitive load, exact-class
      object context, optional value, inline and shared array, shared owner,
      optional shared owner, and inline array.
- [x] Freeze exactly-once production, source evaluation order, completion
      before projection, full-expression lifetime, result securing, reverse
      cleanup, selected-path behavior, and failure ordering.
- [x] State the read-only boundary and retain explicit rejection of direct and
      nested field writes, mutable methods, `mut ref`, implicit shared
      dereference, and all escape-shaped uses.
- [x] Reconcile compiler phase/IR, testing, debugging, grammar notes, backend,
      and runtime ABI documentation with a frozen-but-unavailable feature.

**Tests:** `make docs-check`; audit living documentation with
`rg -n "produced.*field|temporary field|produced.*receiver|object place" docs -g '*.md'`.

**Exit criteria:** One authoritative living contract answers which producers
and fields are eligible, which consumers may use them, when the hidden owner
lives and dies, how each owning field result is secured, and which neighboring
temporary features remain excluded; compiler behavior is unchanged.

Completed 2026-08-15. The living language contract now defines the complete
producer, projection, field-category, lifetime, securing, and read-only
boundary. Compiler-phase, testing, debugging, grammar, backend, and runtime
documentation agree that the design is frozen but unavailable: the resolver
continues to report `RES009`, so compiler behavior and the runtime ABI remain
unchanged. `make docs-check`, the prescribed living-documentation audit, and
the whitespace/error diff check pass.

### PFR1 — Admit produced roots and nested field projections in resolution

**Purpose:** Remove the blanket source rejection and make resolved provenance
represent every accepted field path exactly once before typing or lowering it.

- [x] Replace the direct produced-field rejection in ordinary member-value
      resolution with a `ResolvedFieldAccessExpr` carrying the existing
      produced receiver and selected field identity.
- [x] Implement `ResolvedObjectReceiver::Produced` field projection by
      appending the canonical `ObjectProjection::Field`, retaining the complete
      `exact_class`, terminal `class`, source span, and producer once.
- [x] Apply inherited declaring-base projection and nested exact-class field
      selection without slicing, duplicated resolution, or fake binding roots.
- [x] Preserve ordinary nearest-member lookup, declaring-class privacy,
      malformed-member recovery, static-member diagnostics, and source labels.
- [x] Keep produced optional, array, shared-owner, primitive, and `unit` roots
      on their current family-specific diagnostics.
- [x] Resolve write-shaped paths far enough to preserve the existing read-only
      access diagnostic in type checking; do not accidentally accept mutation
      by sharing the new read projection helper.
- [x] Update resolved dumps and focused resolver helpers with semantic
      produced-field vocabulary and deterministic projection order.

**Tests:** Focused resolver tests for construction, literal, every call-result
producer, structural getter, grouping, inherited/private fields, nested class
fields, closed generics, exact producer identity, deterministic dumps, invalid
root families, and write-shaped sources; then `make check` and
`make msrv-check`.

**Exit criteria:** Every eligible direct or nested field path resolves to one
produced receiver plus canonical projections, every excluded root retains its
specific diagnostic, writes remain unaccepted, and no source expression is
resolved twice.

Completed 2026-08-15. Eligible construction, literal, direct, static,
instance, interface, structural-getter, grouped, inherited, nested, private,
and closed-generic reads now retain one produced receiver with deterministic
canonical projections and a separate final field identity. Member assignment
syntax retains expression receivers for semantic classification, and produced
writes reach the ordinary read-only receiver diagnostic. Unsupported root
families retain their existing diagnostics. Focused syntax/resolver tests, the
produced-receiver golden group, `make check`, `make msrv-check`, documentation
validation, and diff hygiene pass.

### PFR2 — Type and lower primitive and inline-object field consumers

**Purpose:** Establish the first source-to-verified-MIR vertical slice for
field reads while reusing the normalized receiver carrier and ordinary object
place machinery.

- [x] Type direct primitive field reads as ordinary `HirFieldPlace` reads with
      one read-only `HirObjectReceiver::View` whose source is produced and whose
      inspection place is absent.
- [x] Type nested primitive reads through inherited bases and inline class
      fields without treating the projected subobject as an ordinary scalar.
- [x] Permit a produced-root class field in every already-supported read-only
      object context: method receiver, `ref` argument, checked view, explicit
      copy, owning value argument, assignment source, and return copy.
- [x] Preserve exact complete-object origin and dynamic class for root dispatch
      while retaining the statically selected nested field class for its
      immediate consumer.
- [x] Lower the receiver through the common produced-object temporary helper,
      project the ordinary MIR field path, load or consume it once, and reuse
      existing direct, virtual, interface, copy, and alias ABI paths.
- [x] Include produced field roots and their producer expressions in
      control-effect discovery so earlier scalar state is spilled whenever
      production or consumption can change control flow.
- [x] Keep mutable methods, `mut ref`, field assignment, and whole-object
      replacement rejected through existing access diagnostics.

**Tests:** Focused type-check, HIR dump, MIR dump, verifier, copy/lifecycle,
alias, checked-cast, virtual/interface, generic, structural-indexing, and
diagnostic tests; native smoke tests for a direct primitive field and nested
inline-field method/copy consumers; then `make check` and `make msrv-check`.

**Exit criteria:** Primitive and inline-object produced-field consumers compile
through verified MIR and execute natively with one producer temporary, correct
projection and origin, no receiver copy, no fake binding, and unchanged
read-only diagnostics.

Completed 2026-08-15. Primitive endpoints now use ordinary field reads over a
single read-only produced view without an inspection binding. Exact
inline-class endpoints feed the existing method, alias, checked-view, copy,
owning-argument, assignment-source, and return-copy paths. MIR reuses the
common produced temporary, gives nested field subobjects their correct exact
origin and dynamic class, and retains ordinary direct, virtual, interface,
copy, and alias operations. Focused HIR/MIR, verifier, lifecycle, diagnostic,
generic, structural, and dispatch tests plus native produced-field coverage,
`make check`, `make msrv-check`, documentation validation, and diff hygiene
pass.

### PFR3 — Secure owning and guarded field categories

**Purpose:** Make resolver acceptance uniform across the complete implemented
field type surface without allowing the produced root to die before an owning
or guarded field consumer is safe.

- [ ] Route primitive and class optional fields through existing presence,
      unwrap, copy, assignment, argument, and result machinery while keeping
      the produced root live through every bounded payload consumer.
- [ ] Route inline array, shared array, optional shared array, and inline-array
      fields through their existing receiver, anchor, alias, slice, element,
      deep-copy, transfer, and result semantics.
- [ ] Route shared and optional-shared object fields through ordinary secure
      owner copies, produced/replaceable-field anchors, explicit dereference,
      casts, and optional-box guards before releasing the produced root.
- [ ] Prove that a copied or transferred field result outlives root cleanup,
      while a borrowed field or payload view remains bounded by its immediate
      consumer and cannot escape.
- [ ] Preserve source order when field securing itself allocates, retains,
      copies, checks, calls lifecycle code, or fails; do not reload or
      re-evaluate the producer for sizing, applicability, or lowering.
- [ ] Reuse existing typed source and MIR owner categories. Factor a helper
      only where produced-view field sources expose a repeated responsibility;
      do not create one helper per field type.

**Tests:** Focused optional, array, shared-ownership, optional-box, inline-array,
copy, assignment, argument/result, alias-anchor, guard-conflict, allocation
failure, and reverse-cleanup tests. Include named versus produced equivalence,
later-argument replacement, self-overlap where legal, and deterministic HIR/MIR
dumps; then `make check` and `make msrv-check`.

**Exit criteria:** Every implemented readable field category either uses its
ordinary secured/guarded consumer path from a produced root or retains a
documented family-specific rejection; no owner, backing, payload view, or
inline subobject is used after root cleanup.

### PFR4 — Verify lifetime, control flow, and rejection boundaries

**Purpose:** Mechanically prove the full-expression contract across branches,
loops, failures, and malformed IR before broad source adoption.

- [ ] Verify that produced storage begins before construction, becomes owned
      only after complete production, remains live through all field
      projections and consumers, and is cleaned exactly once afterward.
- [ ] Prove result securing precedes root destruction for scalars, exact-class
      copies, owners, arrays, and optionals; prove nested temporaries clean in
      reverse completion order.
- [ ] Cover selected and skipped short-circuit paths, `if`/`elif`, loop epochs,
      return expressions, nested produced-field chains, later consumer effects,
      and production or consumer failure.
- [ ] Add verifier mutations for missing, duplicate, premature, wrong-path, and
      wrong-order cleanup; use-before-initialization; post-cleanup projection;
      invalid complete origin or projection; and mutable access through a
      produced field root.
- [ ] Preserve abrupt non-unwinding behavior without inventing cleanup on a
      terminating path, and preserve all prior method-receiver verifier tests.
- [ ] Keep diagnostics deterministic for direct and nested writes, mutable
      methods, `mut ref`, unsupported root families, private fields, invalid
      member kinds, and escape-shaped uses.

**Tests:** Focused MIR lowering and verifier suites, logical-boundary and
full-expression owner tests, lifecycle trace natives, compile-failure goldens,
and `./scripts/golden.sh --determinism full --filter 'produced_receivers/**'`;
then `make check` and `make msrv-check`.

**Exit criteria:** Verified MIR rejects every lifetime, origin, projection,
access, and cleanup corruption; valid selected paths execute with exactly-once
production and cleanup; invalid source forms fail before MIR with stable
diagnostics.

### PFR5 — Prove native composition and publish the feature

**Purpose:** Exercise the feature across real language composition, remove
staging locals where clarity improves, and make living documentation describe
only the implemented boundary.

- [ ] Add native conformance covering construction, literals, all direct and
      dispatched producer families, inherited/private selection, nested inline
      fields, closed generics, structural getters, every supported final-field
      category, ABI pressure, and observable lifecycle order.
- [ ] Prove that produced-field chains compose with later arguments, checked
      casts, interface calls, optionals, arrays, shared owners, loops, logical
      expressions, copying, returns, and panic/runtime traces without backend
      or runtime special cases.
- [ ] Simplify `Str.join` to read `Vec<Str>` result fields directly where that
      is clearer, and remove only staging locals whose sole purpose was to turn
      a produced object into a readable place.
- [ ] Replace the temporary-field compile-failure golden with focused success
      and read-only failure coverage; update testing guidance and deterministic
      phase-product coverage.
- [ ] Publish current behavior in functions/control flow, classes/lifecycle,
      types/values, aliases/ownership, compiler phase/IR, debugging, testing,
      generic/vector/string, and status documentation. Remove stale claims
      that produced exact-class fields always require a source local.
- [ ] Audit touched modules by responsibility and record non-trivial follow-up
      opportunities in an indexed discoveries document rather than expanding
      this roadmap.
- [ ] Confirm unchanged grammar, internal/external ABI, backend runtime-call
      surface, runtime ABI version, and deterministic assembly.
- [ ] Run focused deterministic native/runtime gates, `make check`, and
      `make msrv-check` from an artifact-free snapshot; then archive this
      completed roadmap and update both roadmap indexes.

**Tests:** Complete produced-field conformance and rejection matrices;
`./scripts/golden.sh --determinism full --filter 'produced_receivers/**'`;
`./scripts/golden.sh --determinism full --filter 'primitive_strings/**'`;
focused pipeline determinism and runtime-trace checks; `make check`;
`make msrv-check`; documentation link and diff hygiene checks.

**Exit criteria:** Produced exact-class field reads are an implemented,
documented, deterministic source-to-native feature across the full supported
field surface; standard-library staging is reduced; all exclusions remain
tested; no target/runtime special case exists; the roadmap is archived with no
unindexed actionable discovery.

## Ordering and dependencies

PFR0 freezes ownership and access before source acceptance. PFR1 makes
resolution and projection provenance explicit without inventing later-phase
representations. PFR2 establishes primitive and inline-object behavior on the
existing produced-view path. PFR3 then extends the same root across owning and
guarded categories whose securing rules require separate lifecycle scrutiny.
PFR4 hardens the complete behavior against path, lifetime, origin, and cleanup
corruption before adoption. PFR5 broadens native composition, simplifies real
source, publishes the implemented contract, and closes the roadmap.

The completed produced-object alias, produced exact-class method receiver,
object-cast, optional, array, shared-ownership, generic-class, and structural
indexing roadmaps are implementation dependencies and baselines, not work to
repeat. PFR0 is the only next task; later tasks depend on every earlier task.

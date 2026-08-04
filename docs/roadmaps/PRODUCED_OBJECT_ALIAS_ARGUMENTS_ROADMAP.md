# Produced Object Alias Arguments Roadmap

Status: in progress; PAA0 through PAA2 are complete, and PAA3 is next.

This roadmap generalizes class alias arguments so a produced exact-class
object can bind directly to a read-only `ref` parameter. The compiler
materializes the value in hidden caller-owned storage, keeps that complete
object alive through the call, and destroys it with the enclosing
full-expression temporaries. The feature removes source-only staging locals
such as the one currently needed for `text.equals("NaN")` without weakening
the language's alias non-escape or ownership rules.

The completed first three tasks publish the source-visible contract, accept
the source through type checking and HIR, and prove its temporary lifetime in
MIR. This roadmap records that frozen contract and the remaining native-
execution and adoption order; verified MIR alone does not publish the feature
as an implemented language contract.

## Scope and invariants

- Permit any expression that produces one exact-class inline object to bind
  directly to a compatible read-only `ref` class, interface, or `Obj`
  parameter. This includes construction, object-returning calls, canonical
  class literals such as `Str` literals, grouping, and supported checked casts
  whose selected result is such a produced object.
- Apply the rule uniformly to ordinary functions, static and instance
  methods, interface calls, and initializer overloads. No callable or
  standard-library type receives a special exemption.
- Evaluate the producer exactly once at its ordinary left-to-right argument
  position. A method receiver is still selected before explicit arguments,
  and later arguments run only after the temporary has been completed.
- Materialize one hidden caller-owned exact-class temporary rather than
  copy-constructing an alias parameter. Preserve the complete object, exact
  dynamic class, base projections, interface witness selection, and `Obj`
  identity used by existing non-owning views.
- Keep the temporary alive through the complete dynamic call, including all
  later argument effects and any nested forwarding performed by the callee.
  Clean it at the enclosing full-expression boundary in reverse completion
  order with other owning temporaries.
- Bind a produced object only to read-only `ref`. A `mut ref` parameter still
  requires an existing mutable place. This makes the implicit relaxation
  observational rather than an invitation to mutate and immediately discard
  an unnamed value; a future explicit mutable-temporary feature would need
  its own syntax and motivation.
- Retain the existing shallow access model: read-only access restricts
  operations through that alias but does not promise global immutability or
  exclusivity. Existing alias overlap and source-order behavior remain
  unchanged.
- Retain alias non-escape. The callee receives no owner, performs no cleanup,
  cannot rebind the parameter, and cannot store or return the alias. Copying
  the designated object into an owning result or destination still creates a
  distinct object before the temporary dies.
- Preserve current class compatibility: an exact class may view itself, an
  ancestor, any implemented interface, or `Obj`, without slicing. Unrelated
  targets, implicit downcasts, and unsupported interface conversions remain
  errors.
- Do not extend this rule to primitives, produced optional containers,
  produced arrays, raw shared handles, or implicit shared dereference.
  Existing optional-, array-, and shared-alias source and anchoring rules stay
  authoritative.
- Existing zero-default static fields remain ordinary identity-based places.
  Their primitive, inline-optional, optional-owner, array, and projection alias
  behavior requires no produced-object temporary and is outside this
  roadmap's source relaxation. A produced object passed alongside a static
  alias still follows the ordinary receiver-before-arguments, left-to-right,
  and full-expression ordering defined here.
- Add no local alias values, reference type, lifetime syntax, user-visible
  storage handle, external alias ABI, runtime service, or grammar form. The
  ordinary expression grammar already admits the source spelling.
- Preserve current failure behavior. If production or a checked conversion
  terminates, the call is not entered. Successfully completed temporaries on
  paths that reach a full-expression boundary follow the existing conditional
  cleanup contract.

## Compiler ownership model

Syntax and resolution continue to represent an ordinary argument expression.
Type checking owns the new source classification and read-only restriction.
Typed HIR records an object-producing view rather than pretending the source
was already a place. MIR lowering owns the hidden storage, construction
point, source-ordered effects, full-expression registration, and final view.
The MIR verifier proves the storage and view lifetime. Backends consume the
same verified internal alias argument representation used for existing
places, so the target calling convention and runtime ABI do not change.

The existing `HirViewSource::Produced`, `HirObjectOrigin::Produced`, and
full-expression object-temporary lowering are the implementation baseline.
They are already exercised by produced receivers, casts, and owning object
consumers. This feature should reuse and, where useful, factor that machinery
rather than introduce a second temporary or alias pipeline.

## Progress

- [x] PAA0 — Freeze the produced read-only alias contract
- [x] PAA1 — Accept and represent produced object alias arguments
- [x] PAA2 — Verify temporary lifetime and source-ordered lowering
- [ ] PAA3 — Prove polymorphic native execution and diagnostics
- [ ] PAA4 — Adopt the feature and publish the implemented boundary

Every implementation task runs focused type-check, HIR, MIR, verifier, and
native tests appropriate to its layer, then `make check` and
`make msrv-check`. Documentation-only PAA0 runs `make docs-check`. The
Makefile remains the repository automation interface.

## PR-sized implementation sequence

### PAA0 — Freeze the produced read-only alias contract

**Purpose:** Replace the current blanket “existing place” restriction with a
precise contract for hidden owning temporaries before compiler behavior or
standard-library source relies on it.

- [x] Update the alias, call, lifecycle, and status documentation to
      distinguish existing-place aliases from produced read-only alias
      arguments.
- [x] Define the accepted exact-class producer families, class/interface/`Obj`
      conversions, grouping and checked-cast composition, and the explicit
      exclusions for `mut ref`, optional containers, arrays, and shared
      handles.
- [x] Freeze exactly-once evaluation, receiver-before-arguments and
      left-to-right order, completion before the call, lifetime through later
      arguments and forwarding, and reverse full-expression cleanup.
- [x] State that alias binding performs no copy, preserves the complete
      dynamic object, creates no ownership in the callee, and leaves the
      internal and external ABI boundaries unchanged.
- [x] Specify diagnostics that identify an incompatible object producer
      separately from a `mut ref` argument that requires an existing mutable
      place.
- [x] Reconcile the grammar's semantic note and compiler phase documentation
      while retaining “frozen but unavailable” status wording.

**Tests:** `make docs-check`; review non-archived matches from
`rg -n "alias argument|existing.*place|produced.*object|mut ref" docs -g '*.md'`.

**Exit criteria:** One living contract answers which produced expressions may
bind, why only `ref` is relaxed, when their owner starts and ends, how
polymorphic identity behaves, and which neighboring alias features remain
excluded; executable behavior is unchanged.

### PAA1 — Accept and represent produced object alias arguments

**Purpose:** Make typed call selection recognize the new source category and
carry its ownership provenance explicitly into HIR.

- [x] Extend object-view source checking so an exact-class producer is
      eligible for an alias argument when the selected parameter is
      read-only, without weakening place validation for mutable aliases.
- [x] Reuse ordinary object-producer checking for constructions, literals,
      exact-class-returning calls, grouping, and checked casts; do not
      duplicate expression typing or evaluate a source during applicability
      probing.
- [x] Apply existing static class projection and interface-conformance rules
      to the produced source, retaining its exact dynamic-class origin and
      complete-object identity.
- [x] Emit a produced `HirObjectView` call argument with read-only access.
      Keep the distinction from a place view visible and deterministic in HIR
      dumps.
- [x] Keep overload applicability and final checking consistent for ordinary
      initializer candidates so speculative checks neither consume identities
      nor emit duplicate diagnostics.
- [x] Add focused success tests for direct, static, method, interface, and
      initializer calls, including `Str` literal to `Obj`, exact class,
      ancestor, interface, grouping, and one checked-cast composition.
- [x] Add focused failures for unrelated targets, implicit downcasts,
      `mut ref` producers, primitives, raw shared handles, arrays, and produced
      optional containers, with stable parameter-site context.

**Tests:** Focused resolver/type-check/HIR tests and dump snapshots;
compile-failure diagnostic goldens; `make check`; `make msrv-check`.

**Exit criteria:** Every supported call form selects a produced read-only
alias through one explicit HIR representation, every excluded family fails at
the type-check boundary, mutable alias acceptance remains place-based, and no
source expression is checked or represented twice.

**Completion summary (2026-08-04):** Object alias checking now classifies
ordinary exact-class producers through the shared producer pipeline and emits
one read-only `HirViewSource::Produced` view with exact dynamic-class origin.
Initializer applicability admits that category only for `ref`, including
through checked casts, while `mut ref` remains place-only and is rejected
before producer checking. Focused HIR and standard-library literal tests cover
all producer/call and static-view families; diagnostic tests and goldens cover
the excluded families with parameter-site context. `make check` and
`make msrv-check` pass.

### PAA2 — Verify temporary lifetime and source-ordered lowering

**Purpose:** Turn the HIR producer view into one caller-owned temporary whose
construction, use, and destruction are mechanically proven.

- [x] Lower the object producer directly into hidden `Temporary` storage at
      its argument position and use that same complete place as the alias-view
      source; do not add a copy-construction step.
- [x] Reuse the common produced-object lowering used by receivers, casts, and
      owning consumers, extracting a cohesive helper if that removes
      source-specific lifetime logic.
- [x] Begin storage lifetime before initialization, register cleanup ownership
      only after successful completion, keep the object live through later
      argument evaluation and the outer call, and destroy completed temporaries
      in reverse order at the enclosing full-expression boundary.
- [x] Preserve selected-path behavior in short-circuit expressions and
      conditions. A skipped producer creates no storage, effects, view, or
      cleanup obligation.
- [x] Compose checked casts so their bounded carrier ends before the owning
      produced temporary, while static projections continue to avoid an
      unnecessary carrier.
- [x] Strengthen MIR verification where necessary to prove one storage
      lifetime, initialization before view use, read-only alias access,
      full-expression registration, reverse cleanup, and absence of use after
      cleanup or duplicate destruction.
- [x] Add mutation-based verifier tests for premature cleanup, missing
      cleanup, mutable produced views, incorrect construction order, invalid
      complete-object origins, and duplicate cleanup.
- [x] Add deterministic MIR tests with multiple produced aliases, mixed scalar
      and alias arguments, nested forwarding, later argument calls, checked
      casts, and selected-path control flow.

**Tests:** Focused MIR lowering, dump, full-expression tracker, and verifier
tests including mutation cases; `make check`; `make msrv-check`.

**Exit criteria:** Verified MIR visibly constructs each producer once at the
correct source position, the alias always designates live owning storage for
the complete call, every completed temporary has exactly one correctly
ordered cleanup, and malformed lifetime/access variants are rejected before
backend lowering.

**Completion summary (2026-08-04):** Produced views now share one cohesive
object-temporary materialization helper that lowers directly into hidden
caller storage and registers cleanup only after successful completion. HIR
retains static ancestor projections separately from exact complete-object
provenance, and MIR preserves left-to-right construction, forwarding,
selected-path ownership, checked-carrier ordering, and reverse
full-expression cleanup without copying. Verification rejects reused
temporary epochs, mutable produced views, invalid origins, early or missing
cleanup, checked carriers that outlive their owner, and duplicate destruction.
Focused deterministic and mutation tests pass; `make check` and
`make msrv-check` pass.

### PAA3 — Prove polymorphic native execution and diagnostics

**Purpose:** Demonstrate that the existing internal alias ABI faithfully
carries produced complete objects across every supported static view without
introducing a target or runtime special case.

- [ ] Exercise exact-class, ancestor, interface, and `Obj` alias targets from
      produced derived objects, including virtual/interface dispatch and
      type/identity observations that prove the object was not sliced or
      copied.
- [ ] Cover direct, static, method, interface, and initializer calls on the
      native target, with receiver and later-argument side effects that expose
      any evaluation-order regression.
- [ ] Trace construction and destruction for several produced arguments and
      nested calls, proving that callee return precedes reverse
      full-expression cleanup.
- [ ] Prove that an owning copy or result made from the alias remains valid
      after the temporary is destroyed and has its own single lifecycle.
- [ ] Retain native compile failures for produced `mut ref` arguments and the
      excluded value families, matching the type-check diagnostics rather
      than failing in MIR or code generation.
- [ ] Audit backend lowering and assembly to confirm it consumes ordinary
      `MirArgument::View` data and requires no new calling-convention branch,
      runtime symbol, layout rule, or ABI-version change.
- [ ] Add deterministic HIR, MIR, assembly, stdout, and destruction-trace
      comparisons where each phase product materially proves the contract.

**Tests:** Focused source-to-native success and compile-failure goldens;
deterministic phase and assembly comparisons; runtime-symbol audit;
`make check`; `make msrv-check`.

**Exit criteria:** Produced aliases execute correctly for every supported call
and polymorphic target, lifecycle traces match the frozen order, owning copies
outlive their sources, diagnostics stop invalid programs before MIR, and the
backend/runtime boundary remains unchanged.

### PAA4 — Adopt the feature and publish the implemented boundary

**Purpose:** Remove temporary source workarounds, make the new rule the single
documented current behavior, and close the roadmap with repository-wide
evidence.

- [ ] Replace standard-library staging locals that exist only to pass a
      produced object to `ref`, beginning with direct special-string
      comparisons in `Str.to_f64`; retain locals that clarify logic or serve
      another lifetime purpose.
- [ ] Add a compact conformance example showing a literal, construction, and
      object-returning call passed directly to `ref`, plus the diagnostic for
      the corresponding `mut ref` case.
- [ ] Update the language status, aliases, functions, lifecycle,
      polymorphism, strings, compiler phase/IR, backend, testing, and debugging
      documentation to describe implemented behavior and inspection points.
- [ ] Audit living documentation, standard-library code, compiler
      diagnostics, and tests for stale claims that every object alias argument
      must already be a source-level place.
- [ ] Record any non-trivial optional-container, array, mutable-temporary, or
      local-alias opportunity in a discovery file rather than expanding this
      roadmap's implemented surface.
- [ ] Run the complete repository, MSRV, documentation-link, deterministic
      process, native, and runtime gates; then archive the completed roadmap
      and advance the active-roadmap index.

**Tests:** Focused standard-library/native regression for direct
`Str.equals("NaN")`, the complete PAA conformance matrix, `make docs-check`,
`make check`, and `make msrv-check`.

**Exit criteria:** Repository source uses direct produced `ref` arguments
where they improve clarity, all living contracts and diagnostics agree on the
implemented boundary, no workaround remains solely because producers were
previously rejected, all gates pass, and the roadmap is archived.

## Ordering and dependencies

PAA0 freezes the source contract before acceptance changes. PAA1 reuses the
existing produced-view HIR vocabulary and must preserve overload-checking
determinism. PAA2 proves the hidden owner and its lifetime before native
behavior relies on it. PAA3 establishes that polymorphism and the internal ABI
need no special target path. PAA4 migrates standard-library source and
publishes implemented status only after the full compiler pipeline is proven.

The roadmap depends on the implemented alias, object value temporary,
full-expression cleanup, polymorphism, checked object-cast, and internal call
contracts. Implemented static-field aliases are an independent existing-place
source and do not expand the produced source categories or lifetime proposed
here. It does not depend on the unfinished floating formatter in the
primitive string conversions roadmap, though PAA4 should coordinate its
`Str.to_f64` cleanup with that roadmap's active source changes.

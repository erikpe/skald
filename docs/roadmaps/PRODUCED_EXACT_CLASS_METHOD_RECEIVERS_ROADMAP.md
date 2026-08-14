# Produced Exact-Class Method Receivers Roadmap

Status: in progress; PER0 through PER4 are complete and PER5 is next.

This roadmap lets an expression that produces one exact inline class serve
directly as a read-only instance-method receiver. It removes staging locals
from expressions such as `"item-".concat(Str.from_i64(index))` and
`values.last().byte(index)` while preserving Skald's deterministic evaluation,
inline ownership, access, and full-expression cleanup rules.

The implementation reuses the completed produced-object alias machinery: one
hidden caller-owned exact-class temporary, one non-owning view of that
temporary, and one reverse-ordered cleanup obligation. It does not introduce
a receiver-specific object representation, calling convention, or runtime
service.

## Scope and invariants

- Accept an exact-class construction, canonical class literal, or exact-class
  result from a direct, static, instance, or interface call as an instance-
  method receiver. Grouping preserves the same eligibility.
- Apply the rule to ordinary direct and virtual methods and to read-only
  interface requirements selected through closed generic bounds. A closed
  generic method result, including `Vec<Str>.last()`, is an ordinary exact-
  class producer after specialization and receives no special implementation.
- Permit only read-only `fn` methods on a produced receiver. A `mut fn` method
  continues to require an existing mutable object place. This matches the
  implemented produced-`ref` rule and does not create implicit mutation of an
  unnamed value.
- Evaluate the receiver producer exactly once before every explicit argument.
  Complete it directly in one hidden caller-owned temporary; do not add a
  copy-construction step merely to borrow it as the receiver.
- Register cleanup ownership only after successful production. Keep the
  completed receiver alive through later argument effects, the complete
  direct/virtual/interface call, result securing, and the enclosing full-
  expression boundary. Destroy completed temporaries in reverse completion
  order.
- Preserve exact complete-object provenance and dynamic class. Inherited
  method selection may project the temporary to its declaring base without
  slicing, and virtual/interface dispatch observes the same complete object.
- Preserve selected-path behavior. A receiver in a skipped short-circuit path
  creates no storage, effects, view, or cleanup; a receiver on an evaluated
  path follows ordinary path-conditioned full-expression cleanup.
- Preserve failure ordering. If receiver production terminates, no explicit
  argument or outer call runs and the incomplete receiver owns no cleanup. If
  a later argument terminates, every already completed full-expression owner
  retains the existing abrupt-termination behavior.
- Keep explicit shared access unchanged. A produced `shared Class` remains an
  owner and continues to require `->` or explicit `*`; this roadmap does not
  make raw shared handles valid dot receivers.
- Do not generalize temporary field reads or writes, mutable temporary
  receivers, optional or array receiver families, independently storable
  references, or escaping aliases. These require separate motivation and
  contracts.
- Add no grammar form. The existing postfix expression grammar already parses
  both target spellings.
- Add no runtime operation, object-layout change, target-specific receiver
  representation, external ABI surface, or runtime ABI-version change.
- Keep resolved, HIR, and MIR dumps deterministic and explicit about produced
  receiver provenance. Never encode a compiler-owned temporary as a fake
  source binding.
- Do not add a third parallel `produced_view` option beside the existing
  shared- and optional-backed receiver carriers. Normalize receiver provenance
  behind one discriminated carrier or one general object-view slot before
  adding the new source category.

## Progress

- [x] PER0 — Freeze the produced read-only receiver contract
- [x] PER1 — Normalize receiver provenance carriers
- [x] PER2 — Accept and lower produced exact-class receivers
- [x] PER3 — Verify lifetime, control flow, and failure behavior
- [x] PER4 — Prove dispatch, generics, and native execution
- [ ] PER5 — Adopt the feature and publish the implemented boundary

Every implementation task runs focused tests for its owning phase, followed
by `make check` and `make msrv-check`. Documentation-only PER0 runs
`make docs-check`. The Makefile remains the local and external automation
interface; this roadmap adds no repository CI.

## PR-sized implementation sequence

### PER0 — Freeze the produced read-only receiver contract

**Purpose:** Make the source-visible access, evaluation, lifetime, dispatch,
failure, and exclusion rules authoritative before compiler acceptance changes.

- [x] Update the functions/control-flow and class/lifecycle contracts to
      define produced exact-class method receivers as hidden caller-owned
      read-only places.
- [x] Define the accepted producer families, grouping behavior, inherited and
      polymorphic method selection, and closed-generic composition.
- [x] Freeze receiver-before-arguments order, exactly-once production,
      completion before the call, result securing, and reverse full-expression
      cleanup.
- [x] State the read-only restriction and retain explicit rejection of
      `mut fn` on a produced receiver.
- [x] Reconcile string examples, compiler phase/IR documentation, testing
      guidance, and the status matrix with a frozen-but-unavailable boundary.
- [x] Record the unchanged grammar, internal/external ABI, backend, runtime,
      and runtime-version boundaries.

**Tests:** `make docs-check`; audit living matches with
`rg -n "method receiver|produced.*receiver|object place|call result" docs -g '*.md'`.

**Exit criteria:** One living contract answers which producers may be
receivers, which methods they may call, when the hidden owner lives and dies,
how dispatch sees it, and which neighboring temporary features remain
excluded; executable compiler behavior is unchanged.

Completed 2026-08-13. The language contract now owns producer eligibility,
read-only access, dispatch, evaluation, failure, lifetime, and exclusions.
Compiler, string, grammar, backend, runtime-ABI, status, and testing documents
freeze the corresponding unavailable implementation boundary. `make docs-check`,
the prescribed living-document audit, and `git diff --check` pass; no
executable compiler or test behavior changed.

### PER1 — Normalize receiver provenance carriers

**Purpose:** Remove the parallel optional provenance shape in typed receiver
data before adding produced sources, keeping impossible carrier combinations
unrepresentable and lowering ownership cohesive.

- [x] Replace the independent shared-backed and optional-backed receiver view
      slots in checked and HIR method-receiver data with one discriminated
      carrier or general `HirObjectView` path.
- [x] Apply the same carrier boundary where field and interface receiver
      plumbing shares the representation, without broadening accepted source
      syntax.
- [x] Keep checked casts and array-element receivers distinct only where their
      bounded guards or checked addressing require different lowering.
- [x] Update HIR dumps, control-effect discovery, receiver access queries, MIR
      lowering, and local helper APIs to exhaustively match the normalized
      carrier rather than coordinating optional fields.
- [x] Preserve all existing shared-anchor, optional-presence-guard,
      array-anchor, exact-origin, virtual/interface dispatch, and cleanup
      behavior byte-for-byte where phase dumps are contractual.
- [x] Keep implementation in the existing cohesive resolver, type-check,
      HIR-object, and MIR-call owners; do not widen facade visibility or create
      source-category-specific lowering modules.

**Tests:** Focused object-place, checked-cast, shared-anchor, optional-view,
array-receiver, virtual/interface-call, HIR-dump, MIR-dump, and control-effect
tests; `make check`; `make msrv-check`.

**Exit criteria:** Existing source behavior is unchanged, each typed receiver
has exactly one explicit provenance carrier, dumps remain deterministic, and a
produced `HirObjectView` can later occupy the general view path without adding
another optional field or fake binding.

Completed 2026-08-13. Checked and HIR method, field, and interface receivers
now use one exhaustive provenance carrier with distinct place, checked-cast,
general-view, and array-element variants. Existing source acceptance, access,
guards, anchors, origins, dispatch, cleanup, and contractual dump text remain
unchanged. Focused carrier and lowering coverage, `make check`, and
`make msrv-check` pass. The general view variant can represent a future
produced receiver without another parallel slot or synthesized source binding.

### PER2 — Accept and lower produced exact-class receivers

**Purpose:** Add the source-to-verified-MIR vertical slice by reusing ordinary
object producers and the normalized receiver-view path.

- [x] Add an explicit resolved produced-receiver variant carrying the resolved
      producer once, its exact class, source span, and inherited base
      projections.
- [x] Recognize canonical class literals, constructions, and exact-class
      results from direct, static, instance, and interface calls. Continue to
      route produced shared owners to the existing explicit-dereference
      diagnostic.
- [x] Resolve the receiver before explicit arguments without resolving,
      checking, or representing its producer twice. Retain focused diagnostics
      for non-class results and invalid member kinds.
- [x] Reuse ordinary object-producer checking to build a read-only produced
      `HirObjectView` with exact complete-object provenance. Do not synthesize
      a source binding or copy construction.
- [x] Enforce read-only receiver access through the existing mutable-method
      diagnostic, including virtual methods and interface requirements.
- [x] Lower the view through the common produced-object temporary helper, then
      reuse the ordinary direct, virtual, and interface receiver ABI.
- [x] Include receiver production in control-effect classification so nested
      calls, checks, and path changes spill earlier scalar state correctly.
- [x] Add deterministic resolved, HIR, and MIR tests for literal, constructor,
      and each call-result producer family, including grouping, inherited
      selection, explicit arguments, and `Vec<Str>.last().byte(...)`.
- [x] Add focused failures for primitive/unit/optional/array/shared results and
      mutable methods, preserving useful source and declaration labels.

**Tests:** Focused resolver, type-check, HIR, MIR-lowering, dump, and diagnostic
tests plus one native smoke test for each motivating expression; `make check`;
`make msrv-check`.

**Exit criteria:** Both motivating expressions compile through verified MIR
and execute natively, every accepted producer is represented once as a
read-only produced view, invalid categories stop before MIR, and no backend or
runtime special case is introduced.

Completed 2026-08-13. Resolution now retains one explicit produced receiver
for literals, constructions, and direct, static, instance, or interface call
results, including grouping and inherited base projections. Type checking
turns it into one read-only produced `HirObjectView` without an inspection
place or copy, and existing object-producer temporary lowering supplies the
ordinary receiver ABI and cleanup. Focused deterministic phase, diagnostic,
control-effect, generic `Vec<Str>`, and native motivating-expression coverage
passes, as do `make check` and `make msrv-check`. Excluded fields, mutable
methods, non-class results, and shared owners stop before MIR with their
frontend diagnostics; no backend or runtime special case was added.

### PER3 — Verify lifetime, control flow, and failure behavior

**Purpose:** Mechanically prove that nested produced receivers obey Skald's
full-expression ownership contract on every normal selected path.

- [x] Prove storage lifetime begins before production, object initialization
      completes before receiver use, cleanup ownership begins only after
      completion, and the view never outlives its temporary.
- [x] Prove the receiver temporary survives all later arguments, nested calls,
      dynamic dispatch, and result securing, then cleans exactly once in
      reverse full-expression order.
- [x] Cover chains in which one object-returning method result becomes the
      receiver of the next read-only method without reevaluation or an
      intermediate copy.
- [x] Cover selected and skipped `&&`/`||` operands, `if`/`elif` conditions,
      repeated `while` condition epochs, return expressions, and nested
      full-expression owners.
- [x] Cover producer checks or calls that terminate before publication and
      later argument failure after receiver completion, retaining the existing
      non-unwinding abrupt-termination contract.
- [x] Strengthen MIR verification where necessary for produced receiver
      storage kind, initialization, exact origin, read-only access, projection,
      use-before-cleanup, and exactly-once cleanup.
- [x] Add verifier mutations for missing or premature cleanup, mutable view,
      wrong complete-object origin, use after cleanup, duplicate production or
      cleanup, invalid projection, and path-condition leakage.
- [x] Add deterministic lifetime traces proving receiver-before-arguments and
      reverse cleanup with several receiver and argument temporaries.

**Tests:** Focused full-expression tracker, MIR lowering/dump, verifier
mutation, logical-path, loop-epoch, and failure-order tests; `make check`;
`make msrv-check`.

**Exit criteria:** Verified MIR proves exactly-once production, live receiver
use, path-correct ownership, and one correctly ordered cleanup for every
completed produced receiver; malformed access and lifetime variants are
rejected before backend lowering.

Completed 2026-08-14. MIR now retains source-granted method-receiver access
and explicit ordinary-versus-produced view provenance independently from the
mutability of backing storage. Verification ties a produced marker to one
exact complete `Temporary`, requires read-only access, and composes with the
existing path-sensitive storage and owner analyses. Focused lowering,
mutation, closed-generic interface, and native lifecycle tests prove chained
receivers, receiver-before-argument order, result preservation, selected and
skipped logical paths, `if`/`elif`, repeated `while` epochs, return
expressions, reverse cleanup, and non-unwinding producer or argument failure.
Malformed storage kind, origin, projection, access, production, cleanup,
post-cleanup use, and path leakage are rejected before backend lowering.
`make check` and `make msrv-check` pass.

### PER4 — Prove dispatch, generics, and native execution

**Purpose:** Demonstrate that produced receivers compose with every existing
method-selection and closed-specialization path through the unchanged native
receiver ABI.

- [x] Exercise exact, inherited, virtual, and interface calls from produced
      derived objects, proving complete-object identity is preserved without
      slicing or copy construction.
- [x] Exercise exact-class results from direct, static, instance, and interface
      producers under register and stack pressure, recursion, and nested call
      chains.
- [x] Exercise canonical string literals and string-producing factories as
      receivers, including embedded zero/high bytes, concatenation, slicing,
      parsing, and byte observation where those operations add distinct
      ownership pressure.
- [x] Exercise closed generic classes whose methods return exact-class values,
      including `Vec<Str>`, a nested generic result, and a generic-bound
      interface call.
- [x] Add native lifecycle traces with later argument effects and owning
      results that outlive receiver cleanup.
- [x] Keep compile-fail goldens for mutable methods and excluded receiver
      families, matching frontend diagnostics rather than MIR/backend errors.
- [x] Audit assembly and runtime symbols to confirm ordinary receiver
      marshaling, no layout change, no runtime addition, and no ABI-version
      change.
- [x] Add cross-process phase, diagnostic, assembly, stdout, status, and
      lifecycle determinism where each observation materially proves the
      contract.

**Tests:** Focused backend/native tests; successful and compile-failure golden
tests; pipeline and golden determinism; runtime-symbol audit; `make check`;
`make msrv-check`.

**Exit criteria:** Every supported producer, dispatch form, and generic result
executes correctly on x86-64; diagnostics reject excluded access before MIR;
lifecycle traces match the frozen order; and the backend/runtime boundary is
unchanged.

Completed 2026-08-14. Focused backend and native tests now exercise exact,
inherited, devirtualized virtual, and closed-bound interface selection while
preserving the produced derived object's complete identity. Direct, static,
instance, and interface result producers execute through recursive nested
chains under mixed register and stack pressure. Canonical strings cover
embedded zero and high bytes, concatenation, slicing, parsing, and byte
observation; closed specialization covers `Vec<Str>`, nested generic results,
and interface-bound calls. Owning-result traces prove later argument order and
that secured results outlive receiver cleanup. Feature-owned success and
compile-failure goldens retain native output, lifecycle order, process status,
mutable access, and excluded-family diagnostics across repeated processes.
Cross-process phase products are deterministic, and assembly/runtime audits
confirm the ordinary receiver convention, unchanged layouts and runtime-call
surface, ABI version 9, and `ska_rt_abi_v9`. Focused checks, `make check`, and
`make msrv-check` pass.

### PER5 — Adopt the feature and publish the implemented boundary

**Purpose:** Remove receiver-only staging workarounds, make direct produced
receivers ordinary current behavior, and close the roadmap with repository-
wide evidence.

- [ ] Rewrite the involved `Vec<Str>` golden exercise to use both
      `"item-".concat(Str.from_i64(index))` and
      `snapshot.last().byte(byte_index)` directly while retaining its growth,
      copy-independence, loop, and cleanup coverage.
- [ ] Audit standard-library and test source for locals that exist only to turn
      an exact-class producer into a read-only method receiver; remove only
      those whose deletion improves clarity.
- [ ] Add compact conformance examples for literal, construction, direct call,
      method chain, generic result, inherited/virtual dispatch, and the mutable
      receiver diagnostic.
- [ ] Update language status, functions/control flow, classes/lifecycle,
      strings, generics/vectors, compiler phase/IR, testing, and debugging
      documentation from frozen-unavailable to implemented behavior.
- [ ] Audit living documentation, diagnostics, dumps, source, and tests for
      stale claims that every exact-class method receiver must be a source-
      level place or that exact-class call results cannot be receivers.
- [ ] Record any non-trivial mutable-temporary, produced-field, optional,
      array, or shared-owner opportunity in a separately indexed discoveries
      document rather than expanding this roadmap.
- [ ] Run documentation links, the complete repository gate, MSRV, focused
      determinism, native, and runtime checks; then archive the completed
      roadmap and update both roadmap indexes.

**Tests:** The complete produced-receiver conformance matrix;
`./scripts/golden.sh --filter 'standard_vec/**'`; `make docs-check`;
`make check`; `make msrv-check`; focused deterministic process and runtime
gates identified by the implementation.

**Exit criteria:** Direct produced read-only receivers are used where they
improve source clarity, all living contracts and diagnostics agree on the
implemented boundary, all supported gates pass, actionable out-of-scope work
is indexed separately, and this completed roadmap is archived.

## Ordering and dependencies

PER0 freezes semantics before acceptance. PER1 makes the typed carrier
maintainable without changing the language. PER2 adds one complete basic
source-to-native path so no intermediate repository state accepts syntax that
cannot lower. PER3 hardens the ownership and control-flow proof before broad
native use. PER4 closes dispatch, generic-specialization, diagnostic, target,
and determinism coverage. PER5 removes workarounds and publishes implemented
status only after the complete pipeline is evidenced.

The roadmap depends on the completed exact-class value/result, produced
read-only alias, full-expression cleanup, polymorphism, checked-cast, string,
generic-class, and `Vec<T>` contracts. It has no dependency on another active
roadmap. Niflheim demonstrates the value of direct literal and call-result
method chaining, but Skald remains authoritative and implements the feature
through inline caller-owned temporaries rather than Niflheim's object model.

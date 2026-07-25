# Explicit Optional Values Roadmap

Status: in progress; OP1 is next.

This roadmap adds explicit optional values without weakening Skald's central
guarantee that every ordinary inline value, alias, and shared owner is present
and valid. Inline `T?` stores either no payload or one live `T`; `shared? T`
stores either no owner or one ordinary non-null `shared T` owner. Presence
tests are non-failing, postfix `!` performs checked access, and every absent
unwrap or attempt to remove a dynamically pinned payload terminates before it
can produce an invalid value or dangling view.

The work also creates focused language and compiler documentation for
optionals and updates the implemented grammar, type, lifecycle, alias,
ownership, failure, IR, backend, runtime-ABI, debugging, and testing
documentation as each corresponding contract becomes executable. Archived
documents remain historical and are not migration targets.

## Scope and invariants

### Source type model

- A plain `T` remains an always-present value. No optional operation changes
  the representation or validity rules of plain primitive, inline class,
  alias, or `shared T` values.
- `T?` is an inline optional value with exactly two source-visible states:
  absent, with no live `T`, and present, with exactly one complete valid `T`.
- The first profile permits `T?` when `T` is a primitive or exact inline class
  type. `unit?`, bare optional interface/`Obj` views, nested `T??`, and
  optional function or array types remain invalid.
- `shared? T` is an optional shared-owner value. Its present payload is one
  ordinary non-null `shared T`, where `T` may be any currently valid shared
  class, interface, or `Obj` target. Absence belongs to the optional wrapper
  and never becomes a value of type `shared T`.
- `shared T?` and `shared? T?` are deliberately recognized and rejected with
  focused diagnostics. They reserve the possible future meanings “non-null
  shared box containing `T?`” and “optional owner of such a box” without
  implementing generalized shared boxes in this roadmap.
- `shared?` consists of the contextual `shared` word followed by the `?`
  punctuation token. Ordinary trivia may separate tokens, while documentation
  and dumps use `shared? T` as the canonical spelling.
- `ref value: T?` and `mut ref value: T?` borrow an always-present optional
  container. They do not form optional references. `ref?`, first-class
  references, escaping aliases, and optional alias values remain invalid.
- Inline optional containment reserves storage for the complete payload.
  Consequently `class Node { next: Node?; }` remains recursively infinite and
  invalid; optionality does not break inline-containment cycles. A
  `shared? Node` edge remains finite.

### Construction, conversion, and inspection

- `none` is a reserved empty-optional expression. Type checking gives it the
  expected optional type at a local, field, argument, assignment, or return
  boundary; using it without one unambiguous optional expectation is invalid.
- An ordinary `T` source may initialize, assign, satisfy an argument, or
  return an expected `T?`. This is the sole new implicit value conversion.
- An ordinary `shared T` source may likewise satisfy an expected `shared? T`;
  named owners are copied, produced owners are adopted or moved, and compatible
  shared up-views retain their existing ownership behavior.
- No optional value implicitly converts to its payload. Optional-to-optional
  compatibility only lifts a conversion already legal for the complete
  payload or shared target; it introduces no new class, interface, `Obj`, or
  primitive relation.
- Exact non-optional overload matches outrank optional injection. `none`
  admits only optional candidates and contributes no payload-type specificity;
  ordinary overload rules must still select one unique candidate.
- `value is some` and `value is none` produce exact `bool` values. Optionals do
  not acquire truthiness, and the tests do not bind or copy a payload.
- Postfix `value!` evaluates its source once. Absence takes an explicit
  non-returning failure edge; success produces the ordinary payload value or a
  bounded checked payload place according to the payload's normal value/place
  category.
- Every `!` remains semantically checked. Dominating presence tests may remove
  redundant machine checks only as a behavior-preserving optimization; source
  correctness never relies on flow-sensitive narrowing.
- `!` participates in the postfix chain with calls, `.`, and `->`.
  `value!.member` accesses an inline payload member, while
  `(owner!)->member` or the equivalent accepted postfix chain first obtains a
  valid `shared T` and then crosses its shared edge.
- There is no implicit optional chaining, coalescing, propagation, member
  forwarding, dereference, cast, operator lifting, or failed-cast-to-optional
  conversion in this profile.

### Lifecycle and evaluation

- A newly initialized optional is always initialized explicitly with `none` or
  a present payload. `var value: T?;` remains invalid, and optional fields
  remain subject to the enclosing initializer's exactly-once field rules.
- Fresh exact `T(arguments)` construction into a newly initialized `T?`
  constructs directly in the optional payload destination. This is a specified
  destination-placement rule, not optional copy elision.
- Assignment evaluates and secures the complete source before changing the
  destination. The runtime transition matrix is:
  - absent from absent: no payload operation;
  - absent from present: initialize or copy-construct the payload;
  - present from absent: destroy or release the old payload;
  - present from present: perform ordinary payload assignment.
- Direct assignment from a non-optional `T` follows the corresponding present
  source row. Shared-owner assignment secures the incoming owner before
  releasing any old owner.
- Copy construction copies presence and conditionally copy-constructs or
  retains one payload. Destruction conditionally destroys or releases exactly
  one present payload. No operation runs a lifecycle member for absent bytes.
- Optional payload temporaries follow the existing full-expression order.
  Results are secured before argument, checked-view, and other temporary
  cleanup; locals and value parameters retain deterministic reverse cleanup.
- Runtime presence is encapsulated inside one initialized optional storage
  state. CFG joins do not require both predecessors to have the same dynamic
  presence, but MIR verification must prove that every conditional payload
  operation is attached to compatible initialized optional storage.

### Checked payload places and dynamic presence guards

- Primitive unwrapping copies the primitive payload. Optional shared-owner
  unwrapping secures an ordinary non-null owner. Neither operation needs a
  continuing presence pin after the copy is complete.
- An inline-class unwrap used by an owning copy consumer pins only through
  completion of that copy. An unwrap used as a field place, method receiver,
  alias argument, cast/type-test source, or other non-owning object-place
  consumer pins the payload for that complete immediate consumer.
- A presence pin is a dynamic lifetime guard, not a source lock or promise of
  exclusive access. Payload fields and methods may mutate the still-present
  `T`; only clearing, replacing, or destroying its optional container is
  forbidden while one or more checked payload views remain active.
- Nested and overlapping checked views are supported by a count or equivalent
  backend-private state. Beginning a view from an absent optional, overflowing
  the guard count, or clearing/replacing/destroying a pinned optional takes an
  unrecoverable failure edge.
- A checked view begins at its source evaluation position. For a receiver or
  alias argument it remains pinned through later left-to-right argument
  evaluation and the complete call, then ends in reverse temporary order.
- A checked payload place is not a storable or returnable alias. It remains
  bounded by the same immediate-consumer discipline as checked object casts.
- Keeping the optional container's storage alive and keeping its payload
  present are separate obligations. A checked optional field reached through
  replaceable or produced shared storage uses the existing shared owner anchor
  as well as the optional presence guard.
- Normal completion ends every guard exactly once. Current unrecoverable
  failure does not return and need not unpin; future recoverable exceptions
  must add exceptional guard cleanup before they can cross this feature.

### Failure, representation, and ABI boundary

- Absent unwrap, presence-guard overflow, and mutation or destruction of a
  pinned optional are source-level unrecoverable runtime failures. They
  terminate without producing an invalid value, returning to Skald, or
  guaranteeing remaining source-level cleanup, matching checked-cast failure.
- MIR represents optional failure reasons explicitly and requires their
  failure edges to end in the matching non-returning termination. The x86-64
  backend lowers those verified edges to the existing illegal-instruction trap
  boundary; no C runtime helper or runtime ABI version change is expected.
- The portable language does not freeze size, padding, tag values, guard
  encoding, register classification, or the null-niche optimization.
- The initial x86-64 compiler contract uses aligned conditional payload
  storage plus state sufficient for absence, presence, nested presence guards,
  and safe lifecycle transitions. The existing presence state may encode the
  guard count; a separate source-visible lock does not exist.
- The initial x86-64 representation of `shared? T` is one machine word: zero
  represents the optional's absence and a non-zero word is an ordinary
  canonical shared header handle. Retain, release, dereference, cast, metadata,
  and finalization operations receive only verified non-zero handles.
- Internal optional parameters and results use one documented compiler-private
  calling convention. External declarations continue to reject every optional
  parameter and result until a separate foreign ABI is designed.

### Documentation and exclusions

- A new `docs/language/OPTIONAL_VALUES.md` becomes the focused authority for
  source optionality, and a new `docs/compiler/OPTIONAL_VALUES.md` becomes the
  focused authority for target-independent IR, verification, presence guards,
  x86-64 representation, and internal ABI behavior.
- Every task updates the affected living, non-archived documentation together
  with behavior. The final task audits all non-archived documentation for
  obsolete exploratory or contradictory optional wording.
- Arrays of optionals, default array element construction, strings, function
  values, generics, first-class references, optional references, generalized
  shared boxes, optional casts, optional equality, optional chaining,
  coalescing, propagation, nested optionals, recoverable exceptions,
  concurrency semantics, and external optional ABI mappings are non-goals.

## Progress

- [x] OP0 — Freeze language and compiler optional-value contracts
- [ ] OP1 — Add optional syntax and resolved type identities
- [ ] OP2 — Execute primitive optional locals and checked inspection
- [ ] OP3 — Carry primitive optionals through stored and callable boundaries
- [ ] OP4 — Implement inline-class optional lifecycle
- [ ] OP5 — Enforce checked payload views and dynamic presence guards
- [ ] OP6 — Implement optional shared owners
- [ ] OP7 — Complete alias, overload, conversion, and polymorphism integration
- [ ] OP8 — Harden diagnostics, documentation, and end-to-end behavior

## PR-sized implementation sequence

### OP0 — Freeze language and compiler optional-value contracts

**Purpose:** Establish one authoritative source contract and one compiler
contract before syntax or representation code depends on them.

- [x] Add `docs/language/OPTIONAL_VALUES.md` with the complete type grid,
      `none`, injection, presence tests, checked unwrap, lifecycle matrix,
      direct payload construction, presence guards, failure behavior,
      declaration positions, and exclusions above.
- [x] Add `docs/compiler/OPTIONAL_VALUES.md` with phase ownership, proposed
      resolved/HIR/MIR distinctions, initialized optional storage invariants,
      conditional ownership, checked-view/anchor interaction, verifier
      obligations, x86-64 layouts, internal ABI direction, trap lowering, and
      the unchanged C runtime ABI boundary.
- [x] Update `docs/README.md`, `docs/language/README.md`,
      `docs/language/STATUS.md`, and `docs/language/TYPES_AND_VALUES.md` to link
      the focused frozen design while stating that compiler support is still
      planned.
- [x] Update the optional-facing future sections in aliases, lifecycle,
      functions/control flow, shared ownership, object casts, polymorphism,
      errors, modules/interoperation, compiler overview, phases/IR, backend,
      runtime ABI, debugging, and testing documentation so none contradicts
      the frozen design or claims implementation.
- [x] Record `shared T?`, `shared? T?`, optional casts, arrays, exceptions,
      concurrency, and external ABI mappings as explicit exclusions rather
      than unresolved syntax.

**Tests:** Run `make docs-check` and review every non-archived match from
`rg -n "optional|nullable|none|some|shared\\?" docs -g '*.md'
-g '!docs/archive/**'`.

**Exit criteria:** Focused language and compiler contracts are linked from the
documentation indexes, all living documents agree on frozen versus implemented
status, documentation validation passes, and no compiler behavior has been
misrepresented.

### OP1 — Add optional syntax and resolved type identities

**Purpose:** Preserve every optional spelling, span, precedence boundary, and
resolved identity before executable semantics are introduced.

- [ ] Add `?` and postfix `!` punctuation tokens and reserve `none`; keep
      `shared` and the presence-test word `some` contextual where the grammar
      requires contextual recognition.
- [ ] Extend type syntax for primitive/exact-class `T?` and `shared? T`,
      retaining separate spans for the payload, `shared`, and each optional
      marker.
- [ ] Parse `none`, `value is some`, `value is none`, and postfix unwrap in the
      existing expression precedence hierarchy, including mixed `!`, `.`,
      `->`, calls, grouping, casts, unary operators, and malformed chains.
- [ ] Recognize and diagnose `unit?`, bare interface/`Obj` optionals, `T??`,
      `shared T?`, `shared? T?`, `ref?`, missing targets, and repeated markers
      without losing later declaration or statement recovery.
- [ ] Add non-recursive resolved optional target identities that preserve the
      distinction between inline optional payloads and optional shared owners
      without making all existing type enums recursively allocated.
- [ ] Update syntax and resolved dumps with one deterministic canonical type
      spelling and explicit nodes for absence, presence tests, and unwrap.
- [ ] Reject otherwise well-formed optional execution at the type-checking
      boundary with one temporary “not implemented” diagnostic until the
      responsible executable task lands.
- [ ] Update the implemented grammar, phases/IR documentation, feature status,
      and debugging dump examples to match the newly accepted source and
      resolved shapes without claiming executable support.

**Tests:** Lexer/parser/resolver unit tests for accepted spellings, precedence,
spans, dumps, contextual words, nesting budget, UTF-8 adjacency, and recovery;
diagnostic tests for every reserved exclusion; `make check`; and
`make msrv-check`.

**Exit criteria:** Every supported and reserved spelling has deterministic AST
and resolved behavior, invalid forms recover without panics, later phases see
explicit optional identities, and all unsupported execution is rejected before
HIR.

### OP2 — Execute primitive optional locals and checked inspection

**Purpose:** Deliver the smallest complete source-to-machine optional slice and
establish reusable HIR, MIR, verification, control-flow, and backend machinery.

- [ ] Type check primitive `T?` locals initialized from `none`, the exact
      primitive `T`, or another exact `T?`; preserve source evaluation once.
- [ ] Type check `is some`, `is none`, and primitive postfix unwrap as exact
      `bool` and primitive value expressions without optional truthiness.
- [ ] Introduce explicit HIR for empty construction, present injection,
      optional copy/assignment, presence tests, and checked unwrap.
- [ ] Add initialized optional storage and explicit MIR operations for
      absent/present initialization, primitive payload copy/store, runtime
      presence tests, and unwrap success/failure control flow.
- [ ] Add a distinct optional-access termination reason, require every dynamic
      unwrap failure edge to end in that reason, and preserve deterministic MIR
      dumps.
- [ ] Verify compatible optional storage, initialization exactly once, legal
      tag/payload operations, terminated failure edges, and identical
      initialized-wrapper state at CFG joins without pretending dynamic
      presence is statically known.
- [ ] Lay out primitive optionals on x86-64 with one documented state plus
      aligned payload, and lower presence branches and failure to verified
      comparisons and `ud2`.
- [ ] Remove the temporary execution diagnostic for primitive optional locals
      and update types/values, functions/control flow, errors, phases/IR,
      backend, and debugging documentation for this exact implemented slice.

**Tests:** Focused type-checker, HIR dump, MIR lowering/dump/verifier, backend
layout/instruction, and native-execution tests for every primitive, absent and
present branches, unchecked successful access, absent failure, assignment,
grouping, repeated checks, and CFG joins; compile-failure goldens for
truthiness and implicit unwrap; `make check`; and `make msrv-check`.

**Exit criteria:** Primitive optional locals initialize, inspect, assign, and
terminate on absent unwrap end to end, while malformed MIR and unsupported
optional positions remain rejected.

### OP3 — Carry primitive optionals through stored and callable boundaries

**Purpose:** Make primitive optionals ordinary owning values across fields and
internal calls before class payload lifecycle or shared ownership adds further
conditional resources.

- [ ] Permit primitive optional fields and enforce explicit exactly-once
      initialization with `none`, `T`, or exact `T?`.
- [ ] Implement absent/present field reads and assignment using the lifecycle
      matrix, including receiver-before-source evaluation and overlapping
      source/destination safety.
- [ ] Include primitive optional fields in synthesized class construction,
      copy construction, copy assignment, and destruction plans without
      reading absent payload bytes.
- [ ] Permit primitive optional value parameters and results for internal
      functions, methods, interfaces, initializers, and virtual overrides under
      one documented exact-signature rule.
- [ ] Implement the documented x86-64 internal parameter/result convention,
      including register/stack classification, result preservation, call
      marshaling, and field/temporary layout.
- [ ] Contextually type `none` at arguments and returns. Extend initializer
      applicability/specificity so exact `T` beats injected `T?`, `none`
      admits only optional candidates, and ambiguous optional candidates
      remain deterministic errors.
- [ ] Reject every optional external parameter/result before MIR and keep the
      runtime ABI surface and version unchanged.
- [ ] Update classes/lifecycle, functions/control flow, polymorphism,
      modules/interoperation, backend, runtime ABI, phases/IR, and testing
      documentation alongside the new positions.

**Tests:** Declaration, initializer-overload, override/interface, field
lifecycle, HIR/MIR call, verifier, ABI classifier, register/stack pressure,
result, and native-execution tests; negative extern and signature tests;
goldens for field/call/return behavior and lifecycle order; `make check`; and
`make msrv-check`.

**Exit criteria:** Every primitive optional stored or internal callable
boundary executes through one verified ABI, lifecycle synthesis is correct,
and external optionals remain diagnosed before backend lowering.

### OP4 — Implement inline-class optional lifecycle

**Purpose:** Add conditional exact-class ownership while preserving existing
construction, copy, assignment, destruction, destination, and containment
semantics.

- [ ] Extend type and layout queries with aligned inline-class optional storage
      containing state plus reserved exact payload bytes; retain base-prefix
      and field alignment rules.
- [ ] Initialize class optionals from `none`, exact live places, produced
      objects, calls, and fresh construction. Construct an ungrouped fresh
      exact `T(arguments)` directly in a new optional payload destination.
- [ ] Implement optional copy construction and destruction by branching on
      source presence and conditionally invoking exactly one existing payload
      lifecycle operation.
- [ ] Implement the complete assignment matrix, securing the source and any
      checked view before destroying/releasing or assigning the destination.
- [ ] Carry class optionals through locals, fields, value parameters, results,
      temporaries, initializer overloads, synthesized lifecycle, base/field
      recursion, normal return, and reverse cleanup.
- [ ] Extend copy/assignment capability analysis through optional class fields
      and report the existing failing payload path when a required operation is
      unavailable.
- [ ] Preserve recursive containment rejection: an optional inline class edge
      contributes the same payload-layout edge as its non-optional class.
- [ ] Add MIR conditional payload construction/copy/assignment/destruction
      operations and verification for reserved storage, published presence,
      cleanup registration, result transfer, temporaries, and CFG joins.
- [ ] Lower optional class destinations, calls, and cleanup on x86-64 without
      addressing absent payload bytes or inventing backend lifecycle policy.
- [ ] Update optional values, classes/lifecycle, types/values, phases/IR,
      backend, and testing documentation with the implemented object lifecycle
      and direct-payload construction rule.

**Tests:** Capability, containment, constructor/copy/assignment/destructor
ordering, nested optional field, parameter/result, temporary, early return,
MIR verification, backend layout, and native-execution tests using
side-effect-visible lifecycle members; compile-failure tests for recursive
containment and unavailable payload operations; `make check`; and
`make msrv-check`.

**Exit criteria:** Inline class optionals own zero or one complete payload
through every ordinary value boundary and perform exactly the specified
lifecycle operations, while non-owning payload access remains limited to the
next task.

### OP5 — Enforce checked payload views and dynamic presence guards

**Purpose:** Make `value!` useful as an inline object place without allowing
re-entrant clearing, replacement, destruction, or alias overlap to create a
dangling payload view.

- [ ] Classify each successful inline unwrap as an owning copy consumer or a
      bounded non-owning checked payload place with its exact root, access, and
      immediate consumer recorded in HIR.
- [ ] Support checked payload places for primitive field access/mutation,
      direct/virtual/interface methods, `ref`/`mut ref` arguments, casts, type
      tests, inline copy consumers, and nested inline projections.
- [ ] Add explicit MIR begin/end optional-view operations and storage state
      capable of representing absence, presence, and nested active guards.
- [ ] Pin before later evaluation can invalidate the payload and unpin after
      the complete consumer, preserving receiver-before-arguments,
      left-to-right arguments, result securing, and reverse temporary cleanup.
- [ ] Reject or terminate clearing, replacing, destroying, or guard-count
      overflow while pinned, while continuing to permit ordinary mutation of
      the present payload.
- [ ] Add distinct optional-pinned-mutation and optional-guard-overflow
      termination reasons and require well-formed non-returning failure edges.
- [ ] Combine a presence guard with the existing stable/copied/adopted shared
      owner anchor whenever the optional container is reached through shared
      storage; end the checked payload view before releasing its anchor.
- [ ] Verify begin/end pairing, compatible payload roots, active-guard
      liveness, legal mutation transitions, anchor coverage, normal-exit guard
      exhaustion, and identical guard state at CFG joins.
- [ ] Lower state/count checks and updates without a runtime helper. Preserve
      one existing optional state word where the documented encoding permits,
      and trap before a forbidden transition changes payload lifetime.
- [ ] Update optional values, aliases/ownership, object casts, polymorphism,
      functions/control flow, errors, phases/IR, backend, debugging, and
      testing documentation with the implemented checked-view boundary.

**Tests:** Type-checker access matrices; HIR/MIR dumps; mutated-MIR verification
for missing, mismatched, leaked, or reordered guards; backend/native tests for
nested views, overlapping aliases, later-argument clearing, re-entrant method
clearing, allowed payload mutation, shared-root anchoring, guard overflow, and
failure traps; `make check`; and `make msrv-check`.

**Exit criteria:** Every successful inline payload place remains present and
its containing storage remains alive through its verified immediate consumer;
every invalidating transition fails before ending the payload lifetime.

### OP6 — Implement optional shared owners

**Purpose:** Add `shared? T` as conditional ownership while keeping every
ordinary `shared T` non-null and reusing the existing allocation, metadata,
anchor, cast, and finalization model.

- [ ] Type check `shared? T` locals, fields, parameters, results, temporaries,
      assignments, and exact virtual/interface signatures for every current
      class/interface/`Obj` shared target.
- [ ] Construct absence from `none`; inject named `shared T` by owner copy and
      produced allocation/call/cast results by existing adopt or move rules.
- [ ] Lift only currently valid shared target compatibility and owner-preserving
      casts through optionality, preserving exact produced-allocation
      provenance and overload specificity.
- [ ] Implement absent/present owner copy, secure-before-release assignment,
      conditional release, result transfer, field lifecycle, temporary
      cleanup, and dynamic last-owner destruction.
- [ ] Make `owner!` secure one ordinary non-null `shared T` before its
      consumer. Continue to require ordinary `*` or `->` pointee selection
      after unwrap, and diagnose direct optional-owner dereference or member
      access.
- [ ] Add explicit HIR/MIR optional-owner operations and verification that
      absence accounts for no strong owner, presence accounts for exactly one,
      zero is never passed to a normal shared operation, and CFG joins retain
      consistent initialized optional-owner storage.
- [ ] Implement the one-word x86-64 zero-niche representation, guarded
      retain/release/metadata/dereference operations, register/stack/result
      classification, and normal trap failures without changing allocation
      headers or the C runtime ABI.
- [ ] Reuse existing shared anchors after unwrap for pointee places reached from
      replaceable or produced owners; do not add a presence guard once the
      non-null owner has been secured.
- [ ] Update optional values, language/compiler shared ownership, aliases,
      object casts, polymorphism, classes/lifecycle, phases/IR, backend,
      runtime ABI, errors, and testing documentation with the implemented
      optional-owner contract and unchanged plain-owner guarantees.

**Tests:** Owner copy/adopt/move/release, field/parameter/result, direct and
allocation-alias self-assignment, up-view, interface/`Obj`, owner cast,
checked-pointee anchor, nested shared field, last-release destruction, cycle,
zero-guard, ABI pressure, verifier mutation, and native/golden failure tests;
`make check`; and `make msrv-check`.

**Exit criteria:** `shared? T` executes in every internal owning position,
absence never enters ordinary shared machinery, every present handle has one
verified ownership account, and `shared T` remains unchanged and non-null.

### OP7 — Complete alias, overload, conversion, and polymorphism integration

**Purpose:** Close cross-feature semantic gaps after both inline and shared
optional payload families have executable ownership and checked-access rules.

- [ ] Permit `ref value: T?` and `mut ref value: T?` for supported inline
      optional containers, preserving read-only/mutable access, non-exclusivity,
      call-scoped lifetime, and the prohibition on optional references.
- [ ] Allow read-only aliases to test and checked-access payloads; allow mutable
      aliases additionally to set, clear, or replace an unpinned container.
      Apply dynamic presence guards to unwrapped payload aliases.
- [ ] Complete optional compatibility at locals, fields, arguments, returns,
      assignments, initializer candidates, overrides, interface requirements,
      checked object-place consumers, and shared up-views without implicit
      unwrapping.
- [ ] Freeze and implement overload ranking for exact payloads, optional
      injection, `none`, compatible shared targets, and otherwise ambiguous
      optional candidates; preserve source-ordered diagnostics.
- [ ] Ensure `is some`/`is none` remains distinct from object type tests and
      that postfix unwrap composes with existing plain/shared casts only after
      producing the checked payload or secured owner.
- [ ] Diagnose optional values used through truthiness, raw `.`, raw `->`, raw
      `*`, arithmetic, implicit casts, unsupported extern boundaries, `ref?`,
      `shared T?`, `shared? T?`, and nested optional shapes with focused
      alternatives where one exists.
- [ ] Complete syntax/resolved/HIR/MIR dump coverage and deterministic naming
      for hidden optional destinations, temporaries, views, guards, and
      optional shared-owner storage.
- [ ] Update all affected language and compiler documents for the completed
      declaration, overload, alias, conversion, and polymorphism matrices.

**Tests:** Complete type/access/overload matrices, alias overlap, optional
container forwarding, exact override/interface matching, static/dynamic object
view composition, shared up-view and cast composition, diagnostics, dumps,
goldens, and generative frontend robustness; `make check`; and
`make msrv-check`.

**Exit criteria:** Every supported source boundary agrees on optional
compatibility and access, aliases cannot outlive or invalidate checked payload
views, and all intentionally excluded type combinations receive deterministic
diagnostics.

### OP8 — Harden diagnostics, documentation, and end-to-end behavior

**Purpose:** Validate the complete feature as one coherent language/compiler
profile, remove rollout wording, and leave living documentation describing
only current behavior and explicit future exclusions.

- [ ] Add end-to-end golden programs covering every primitive, inline class,
      optional shared target, storage position, presence transition, checked
      access consumer, lifecycle effect, owner transfer, call boundary,
      polymorphic view, and normal cleanup order.
- [ ] Add compile-failure goldens for every unsupported optional type,
      declaration, conversion, implicit access, boxing form, external
      signature, containment cycle, and malformed punctuation chain.
- [ ] Add native failure goldens for absent unwrap, pinned clearing,
      replacement/destruction, and guard overflow, requiring non-success
      without promising an exact signal, status, or diagnostic string.
- [ ] Run deterministic robustness coverage across the lexer, parser,
      resolver, type checker, HIR/MIR dumps, verifier, and backend legality for
      malformed and deeply nested optional syntax and mutated optional MIR.
- [ ] Audit responsibilities and size of the new optional modules and split
      cohesive owners behind facades where needed; do not leave optional logic
      scattered as repeated matches across unrelated phases.
- [ ] Audit every non-archived documentation file. Promote optional values to
      **implemented contract** in the status matrix; make the focused optional
      documents authoritative; update overview, grammar, types, functions,
      classes, aliases, shared ownership, casts, polymorphism, errors,
      interoperation, compiler architecture, phases/IR, backend, runtime ABI,
      debugging, and testing links and wording.
- [ ] Remove roadmap task codes and rollout terminology from living code,
      tests, dumps, diagnostics, and non-roadmap documentation. Preserve only
      semantic optional vocabulary outside roadmap/archive files.
- [ ] Confirm the C runtime symbol set and ABI version remain unchanged, or,
      if implementation evidence required a new runtime boundary, document and
      test one coordinated ABI version bump rather than an undocumented helper.
- [ ] Run the complete repository and supported-toolchain gates from an
      artifact-free snapshot and verify final link, status, and diff hygiene.

**Tests:** Focused optional suites; `make robustness-long`; `make check`;
`make msrv-check`; non-archived optional-term audit; documentation link/index
validation; and a final clean-snapshot golden run.

**Exit criteria:** The complete source-to-native optional profile is covered by
positive, negative, failure, lifecycle, ownership, verifier, ABI, and
robustness tests; all living documentation describes implemented behavior
without stale exploratory language; and the roadmap is ready for the standard
completion/archive procedure.

## Ordering and dependencies

OP0 freezes source and representation contracts before implementation. OP1
then gives every later phase stable syntax, spans, resolved identities, and
diagnostics. OP2 establishes a small executable optional core and its explicit
failure/control-flow model. OP3 extends that core across fields and calls so
later resource-bearing payloads can reuse stable storage and ABI boundaries.

OP4 depends on primitive optional storage and existing class lifecycle. OP5
depends on OP4's conditional inline payload lifetime and reuses the existing
checked-place and shared-anchor machinery. OP6 also depends on the core storage
and call boundaries, but its conditional shared ownership is otherwise
independent of inline-class presence guards and may proceed alongside OP4/OP5
after OP3 when repository integration permits. OP7 waits for both inline and
shared optional families so it can close one final compatibility, alias, and
polymorphism matrix without placeholders. OP8 is the only broad hardening and
documentation-audit task.

Every implementation task keeps its behavior and living documentation in the
same change. `make check` is the ordinary repository gate; every Rust or
supported-syntax change also runs `make msrv-check`. The extended deterministic
robustness suite and artifact-free final validation belong to OP8 rather than
being substituted for focused tests during earlier tasks.

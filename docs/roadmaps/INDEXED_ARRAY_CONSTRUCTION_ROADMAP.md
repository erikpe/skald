# Indexed Array Construction Roadmap

Status: in progress; IA0 through IA5 are complete and IA6 is next.

This roadmap implements the frozen
[indexed array construction contract](../language/ARRAYS.md#indexed-array-construction)
and its archived
[design record](../archive/INDEXED_ARRAY_CONSTRUCTION_DESIGN_PROPOSAL.md).
It adds typed `T[](length; index => expression)` and
`new T[](length; index => expression)` construction from source through
verified MIR and native x86-64, then uses that ordinary language feature to
implement `Vec<T>.to_array()`.

## Scope and invariants

- Accept explicit typed inline and shared-outer indexed construction. The
  existing array type remains the sole source of element identity.
- Evaluate the length once as an exact `u64`, allocate unpublished backing
  before element effects, and evaluate elements in increasing index order.
- Bind one immutable exact `i64` index whose scope is only the element
  expression. The length expression cannot observe that binding.
- Treat each selected element slot as a previously uninitialized owning
  destination. Never implement the form as default construction followed by
  assignment.
- Reuse ordinary primitive, exact-class, optional, nested-array, shared-owner,
  and optional-owner destination initialization, including named copy versus
  produced adoption.
- Require only the selected source-to-destination operation. Indexed
  construction does not imply element default construction or assignment.
- Bound each iteration's temporaries and anchors to a per-element cleanup
  epoch, complete that cleanup before the next index, and retain only values
  adopted by the destination slot.
- Represent the initialized extent as a verified dynamic prefix. Advance it
  only after an element is completely initialized and publish only when the
  prefix equals the requested length.
- Preserve completed arrays' existing copy, assignment, destruction, slicing,
  indexing, and ownership behavior independently of how they were built.
- Preserve deterministic syntax, resolved, HIR, MIR, assembly, and diagnostic
  output plus structured non-panicking rejection at every staged boundary.
- Keep target layout private and retain runtime ABI version 9 with no new C
  entry point, metadata field, or `Vec` compiler special case.
- Do not add callback values, fill/repetition constructors, iterable
  collection, comprehensions, builder syntax, type inference, parallel
  evaluation, or generalized placement expressions.
- Maintain facade-oriented recursive Rust modules. Substantial syntax,
  typing, MIR, verification, or backend logic belongs behind its owning array
  facade rather than in orchestration modules.

The implemented element-list construction pipeline supplies the static-prefix
and destination-initialization foundation. The dynamic-prefix proof is the
principal new compiler responsibility. No other roadmap blocks IA0.

## Progress

- [x] IA0 — Retain indexed construction through syntax and resolution
- [x] IA1 — Select typed repeated destination initialization
- [x] IA2 — Execute verified primitive dynamic-prefix construction
- [x] IA3 — Execute exact-class destination placement and copying
- [x] IA4 — Compose optional and nested-array elements
- [x] IA5 — Compose shared and optional-shared owner elements
- [ ] IA6 — Add `Vec<T>.to_array()`, harden, and publish

## PR-sized implementation sequence

### IA0 — Retain indexed construction through syntax and resolution

**Purpose:** Establish the complete source and name-binding contract before
semantic or executable representations depend on it.

- [x] Tokenize `=>` by longest match without changing `=` or `>` behavior,
      and include it in deterministic token dumps and implemented punctuation
      documentation.
- [x] Add distinct inline and shared-outer indexed construction syntax that
      retains `new`, exact array type, length, semicolon, binding name, arrow,
      element expression, parentheses, and complete spans.
- [x] Parse multiline and nested forms in every expression position without
      confusing ordinary array construction, calls, indexing, generic
      punctuation, or explicit element lists.
- [x] Diagnose missing length, semicolon, binding, arrow, expression, and
      closing parenthesis once, with recovery at stable expression, statement,
      argument, initializer, and body boundaries.
- [x] Include both expressions in syntax depth, dependency scanning, generic
      request discovery, visitors, and source-order traversal.
- [x] Resolve the length before introducing one local binding, resolve the
      element expression under that binding, and retain a non-forgeable binding
      identity rather than matching later references by spelling.
- [x] Reject duplicate/conflicting source shapes structurally while deferring
      index type, immutability, and destination compatibility to IA1.
- [x] Extend AST and resolved dumps and public phase accessors without exposing
      parser-private representation.
- [x] Add one tested semantic availability gate so accepted source cannot
      panic or leak malformed HIR before IA1.
- [x] Promote only accepted syntax from the frozen grammar section; keep the
      status matrix explicit that indexed construction is not executable yet.

**Primary implementation areas:** lexer punctuation, expression parser and
array syntax facade, recovery and nesting, source visitors, resolver scopes and
expression IR, phase dumps, diagnostics, and focused documentation.

**Tests:** Token adjacency; inline/shared, multiline, nested, postfix-consumed,
and expression-position parsing; every malformed delimiter and recovery case;
binding shadowing and out-of-scope rejection; depth and generic request scans;
AST/resolved independent-process determinism; explicit semantic-gate tests.

**Gates:** Focused lexer, parser, resolver, robustness, and dump tests;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Every frozen source shape has one deterministic syntax and
resolved representation, the index identity is scoped exactly to the element
expression, malformed input recovers without compiler panics, and no indexed
construction reaches HIR except through the deliberate gate.

### IA1 — Select typed repeated destination initialization

**Purpose:** Resolve length, index, and element ownership semantics in HIR so
lower layers need no type, capability, or provenance inference.

- [x] Add an indexed array construction HIR mode with exact array identity,
      inline/shared outer ownership, length expression, immutable index local,
      element expression, and one reusable destination initialization plan.
- [x] Require the length expression to be exact `u64` and the generated index
      local to be exact `i64`; reject conversions and index mutation through
      the ordinary local-assignment diagnostics.
- [x] Type-check the length outside the binding scope and the element under
      the expected exact element destination type.
- [x] Reuse the stored-value destination initialization selector introduced by
      element-list construction for primitive, exact-class, optional,
      nested-array, shared-owner, and optional-owner sources.
- [x] Retain initializer/copy identities, access authorization, nested
      `ArrayTypeId`, shared target, named-versus-produced provenance, and all
      spans required below HIR.
- [x] Require neither a default plan nor assignment merely because the length
      is dynamic; preserve the completed array type's independent lifecycle
      table.
- [x] Diagnose failures at the length, binding mutation, or element expression
      that owns them, including unavailable copy and invalid owner target.
- [x] Extend HIR effect, reachability, storage-use, dependency, and
      deterministic dump traversal in the specified evaluation order.
- [x] Replace the semantic gate with an explicit executable-lowering gate
      until IA2 supplies verified dynamic-prefix MIR.

**Primary implementation areas:** resolved-to-HIR array lowering, local
binding and assignment validation, reusable destination initialization plans,
generic capability collection, HIR effects/dependencies/dumps, and staging.

**Tests:** Exact length/index types; index visibility and immutability; every
element family; no-default classes; named/produced provenance; inaccessible
initializer; unavailable copy without false default/assignment requirements;
generic specialization; stable HIR dumps and lowering-gate diagnostics.

**Gates:** Focused type, HIR, generic, effect, dependency, and dump tests;
`make compiler-test`; `make docs-check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Valid HIR carries one exact repeated destination plan and
all identities required for lowering, invalid programs fail at their owning
source construct, generic requirements contain no invented default or
assignment capability, and later phases remain protected by one structured
gate.

### IA2 — Execute verified primitive dynamic-prefix construction

**Purpose:** Establish the canonical loop, per-element cleanup epoch, dynamic
initialized-prefix proof, and native vertical slice with trivial element
lifecycle.

- [x] Extend target-independent MIR with the minimum array-construction state
      needed to retain requested length and dynamic initialized prefix without
      exposing target layout.
- [x] Lower length evaluation, checked allocation, zero-prefix initialization,
      loop header, `i64` index materialization, element evaluation, direct slot
      initialization, prefix advance, cleanup, backedge, and publication in
      the frozen order.
- [x] Make the zero-length path publish without evaluating the element and
      prove allocation failure precedes every element effect.
- [x] Reuse existing full-expression machinery within a new per-element epoch
      so all non-adopted temporaries and anchors are cleaned before the
      backedge.
- [x] Verify canonical CFG shape, exact types, prefix monotonicity and bounds,
      slot/prefix correspondence, no use before initialization, complete
      publication, and single backing consumption.
- [x] Add verifier mutations for skipped/duplicate/out-of-order stores,
      advance-before-initialize, missing/excess advance, escaped epoch values,
      invalid backedges, incomplete publication, and leaked/duplicated backing.
- [x] Extend storage/lifetime use accounting, cleanup verification, MIR dumps,
      backend legality, reachability, and final-MIR transforms for the new
      state without weakening proof seals.
- [x] Execute inline and shared-outer arrays of every primitive element type on
      x86-64 using existing allocation, addressing, store, publication, and
      release machinery.
- [x] Remove the execution gate for primitive elements while retaining
      structured staging for lifecycle-bearing families.
- [x] Prove runtime headers, symbols, allocator/panic interfaces, and ABI
      version remain unchanged.

**Primary implementation areas:** array HIR-to-MIR lowering, construction
state and verifier, cleanup epochs, MIR transforms/dumps, x86-64 array lowering,
ABI assertions, and primitive golden fixtures.

**Tests:** Zero/one/many lengths; inline/shared outer; every primitive; index-
dependent and side-effectful expressions; allocation-before-effect; per-index
cleanup traces; ordinary destinations and postfix use; exhaustive verifier
mutations; exact MIR/assembly dumps; source/native equivalence and determinism;
ABI compatibility.

**Gates:** Focused MIR, verifier, cleanup, optimizer, backend, and ABI tests;
indexed-construction goldens; `make check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Primitive indexed construction executes through verified
MIR in both outer ownership modes, evaluates the requested length and every
index exactly as specified, publishes only a complete dynamic prefix, cleans
each iteration before the next, and adds no runtime ABI.

### IA3 — Execute exact-class destination placement and copying

**Purpose:** Apply the proven dynamic-prefix protocol to observable class
construction, copying, adoption, and destruction.

- [x] Supply each slot as the final destination for eligible ungrouped exact-
      class construction and exact-class-returning calls, with no default
      object, assignment, or unnecessary temporary.
- [x] Copy-construct from named places and otherwise materialized sources with
      the exact selected operation and existing checked-source rules.
- [x] Preserve grouping: grouped fresh construction materializes, requires the
      applicable copy, initializes the slot from it, and destroys the
      temporary in the current per-element epoch.
- [x] Enforce initializer privacy at the indexed construction site and
      diagnose unavailable copy only for source shapes that require it.
- [x] Advance the prefix only after initializer, result placement, or copy
      construction completes normally; clean all other iteration state before
      the backedge.
- [x] Extend MIR class-initialization, lifetime, path-state, and cleanup
      verification for one logical slot destination reused across epochs.
- [x] Reuse x86-64 initializer, result, copy, destructor, and aligned element-
      place machinery without aggregate byte copying.
- [x] Preserve constructor, copy-constructor, destructor, source effect, and
      reverse completed-array destruction order in all owning consumers.

**Tests:** Fresh, named, grouped, call-result, private-initializer, explicit-
copy, no-default, unavailable-copy, conditional source, user lifecycle,
alignment, local/field/argument/result/assignment, and inline/shared-outer
matrices; exact HIR/MIR dumps; class/prefix verifier mutations; native traces.

**Gates:** Focused class construction, MIR, verifier, cleanup, backend, and
native tests; indexed class goldens; `make check`; `make msrv-check`; and
`git diff --check`.

**Exit criteria:** Exact-class elements retain ordinary destination-directed
construction and copy distinctions at every dynamic index, lifecycle effects
occur once in the frozen order, per-element state cannot leak across the
backedge, and no default or assignment operation is introduced.

### IA4 — Compose optional and nested-array elements

**Purpose:** Prove that the dynamic-prefix protocol composes with conditional
payload lifetime and recursively owning array values.

- [x] Initialize optional primitive and exact-class slots from absence,
      injection, optional sources, and conditional expressions using existing
      payload destination plans.
- [x] Publish optional presence only after its payload is complete and advance
      the outer array prefix only after the complete optional value is live.
- [x] Direct eligible fresh class values into present payload destinations
      while preserving named/materialized copy and grouping behavior.
- [x] Initialize nested inline-array elements through exact array copy or
      produced-backing adoption, including inner indexed construction with an
      independent requested length, prefix, loop, and cleanup epoch.
- [x] Preserve jagged lengths, nested source/effect order, backing ownership,
      and reverse recursive cleanup without rectangular-shape inference.
- [x] Extend verifier nesting rules so optional state, inner prefixes, and
      outer prefix advancement cannot be confused or cross-consumed.
- [x] Reuse existing optional and nested-array x86-64 layout, guards, transfer,
      copy, destruction, and publication machinery.

**Tests:** Absent/present primitive and class optionals; fresh/named/grouped
payloads; unavailable conditional copy; nested empty/nonempty/jagged arrays;
indexed-inside-indexed construction; named copy versus produced adoption;
optional/prefix/nesting verifier mutations; lifecycle and failure traces in
both outer ownership modes.

**Gates:** Focused optional, nested-array, MIR, verifier, cleanup, backend, and
native tests; indexed optional/nested goldens; `make check`; `make msrv-check`;
and `git diff --check`.

**Exit criteria:** Optional and nested-array element destinations compose with
the exact existing conditional and ownership semantics, every nested prefix
has independent proof state, and each outer prefix advances only after its
entire element is live.

### IA5 — Compose shared and optional-shared owner elements

**Purpose:** Complete the element matrix with polymorphic retained ownership,
the category required by the motivating generic vector adopter.

- [x] Initialize shared exact-class, base-class, interface, `Obj`, shared-array,
      and optional-shared element destinations through their selected retain,
      transfer, conversion, and absence plans.
- [x] Preserve named-source retention versus produced-owner adoption without
      allocating default pointees or requiring an exact polymorphic target.
- [x] Keep temporary owners and receiver anchors within the current element
      epoch unless ownership is transferred into the slot.
- [x] Verify owner target compatibility, reference-count responsibility,
      optional presence, prefix advancement, publication, and cleanup on
      normal loop paths.
- [x] Exercise shared outer arrays independently from shared element values so
      backing ownership and element-owner counts cannot be conflated.
- [x] Extend reachability, dependency, lifetime, ownership, optimizer, and
      backend handling without introducing a special shared indexed path.

**Tests:** Exact/base/interface/`Obj` and array targets; named retention,
produced adoption, conditionals, optional absence/presence, returned owners,
receiver anchors, nested shared outer arrays, zero/one/many lengths, ownership
verifier mutations, reference-count and destructor traces, leak checks, and
source/native equivalence.

**Gates:** Focused shared ownership, optional box, MIR, verifier, optimizer,
backend, and native tests; indexed shared-owner goldens; `make check`;
`make msrv-check`; and `git diff --check`.

**Exit criteria:** Every existing shared and optional-shared element category
constructs correctly at a dynamic index, retained and transferred owners have
one exact responsibility, backing and element ownership remain distinct, and
polymorphic targets require no default object.

### IA6 — Add `Vec<T>.to_array()`, harden, and publish

**Purpose:** Exercise the complete language feature through its motivating
ordinary-library adopter and close documentation, determinism, and regression
coverage.

- [ ] Add public `Vec<T>.to_array() -> T[]` using indexed construction over the
      live prefix of private optional capacity storage, with no compiler or
      runtime knowledge of `Vec`.
- [ ] Validate `to_array()` for primitive, non-defaultable exact class, nested
      array, shared exact/base/interface/`Obj`, optional, and optional-shared
      element families, including empty, spare-capacity, and grown vectors.
- [ ] Prove conversion preserves the vector and uses ordinary source-to-
      destination copying or owner retention; document that consuming/draining
      conversion is outside this roadmap.
- [ ] Remove all remaining family gates and audit every expression position,
      owning destination, generic specialization, module boundary, cleanup
      exit, and postfix consumer.
- [ ] Add malformed/deep source generation, verifier mutation matrices,
      allocation-failure and lifecycle stress, independent-process dump and
      diagnostic determinism, optimizer selection, and source/native
      equivalence coverage.
- [ ] Confirm final-MIR optimization never folds away observable per-index
      effects or weakens requested-length, prefix, epoch, ownership, or
      publication proof obligations.
- [ ] Promote indexed construction from frozen to implemented across grammar,
      status, array, vector, generic, compiler, backend, runtime ABI, testing,
      and debugging documentation.
- [ ] Record ABI version 9 compatibility and archive this roadmap only after
      the complete standard and replacement-standard-library test matrix
      passes.

**Primary implementation areas:** `std/std/vec.ska`, standard-library vector
goldens, full frontend/HIR/MIR/verifier/backend regression suites, optimizer
and determinism tests, living documentation, and roadmap/archive indexes.

**Tests:** Complete indexed-construction family matrix; `Vec<T>.to_array()` for
all existing vector categories and capacities; no-default and polymorphic
shared regression cases; mutation/depth/recovery/failure stress; phase and
assembly determinism; source/native parity; ABI symbol/version assertions.

**Gates:** Focused standard-vector and indexed-construction goldens;
`make check`; `make msrv-check`; `make docs-check`; repeated independent-
process determinism checks; and `git diff --check`.

**Exit criteria:** Indexed construction is fully implemented and documented
for every frozen element and ownership category, `Vec<T>.to_array()` is an
ordinary generic library method with complete coverage, all staged gates are
gone, native output and artifacts are deterministic, runtime ABI version 9 is
unchanged, and no roadmap-specific discovery remains unresolved.

## Ordering and dependencies

1. IA0 owns source identity and scoped binding; IA1 must not infer either from
   syntax text.
2. IA1 owns all type, capability, access, and provenance selection; MIR and
   the backend consume those decisions without repeating them.
3. IA2 establishes the dynamic-prefix and cleanup-epoch trust boundary before
   any lifecycle-bearing element category executes.
4. IA3 validates observable direct placement and copying before optional,
   nested, or shared composition adds conditional ownership.
5. IA4 and IA5 complete independent conditional/nested and retained-owner
   dimensions on the same verified prefix protocol.
6. IA6 adopts only the fully implemented language surface in `std::vec`, then
   closes documentation and regression coverage.

If implementation reveals a conflict with a confirmed design decision, record
it in a dedicated indexed-array-construction discoveries document and resolve
the contract explicitly. Do not silently broaden an IA task or change the
frozen design through implementation detail.

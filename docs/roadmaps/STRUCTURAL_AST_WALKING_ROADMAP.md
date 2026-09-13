# Structural AST Walking Roadmap

Status: in progress; W01 through W03 are complete and W04 is next.

This roadmap implements
[cleanup finding A13](CODEBASE_CLEANUP_AUDIT.md#a13--share-structural-ast-walking-where-responsibilities-repeat)
through the accepted
[structural AST walking design](../archive/STRUCTURAL_AST_WALKING_DESIGN_PROPOSAL.md).
It gives source-shaped structural traversal one private syntax owner, proves
the contract with non-semantic consumers, restores the resolver's stated
source-order guarantee, and then migrates specialization discovery without
moving semantic authority into syntax.

The completed work will leave `syntax::walk` responsible only for immutable
node relationships, source order, balanced enter/leave events, and
continue/prune control. Parser depth policy, module dependencies, resolver type
closing, specialization identities, diagnostics, scopes, and every transformed
phase product remain with their current owners.

## Scope and invariants

- Add one crate-private iterative syntax walker. Do not expose it through the
  workspace-facing public syntax API.
- Keep the syntax facade concise, with substantial work-stack and child-order
  implementation in a cohesive private file and focused tests beside it.
- Borrow AST nodes immutably. The walker cannot mutate syntax, resolve names,
  allocate identities, emit diagnostics, or build later-phase products.
- Emit children in source spelling order, retain stored vector order, and omit
  absent optional children.
- Emit one balanced leave event for every enter event, including when a
  consumer prunes the node's descendants. Pruning cannot suppress a later
  sibling.
- Treat complete `TypeSyntax` and `NamedTypeSyntax` occurrences as opaque leaf
  events. Their recursive meaning stays with existing type owners.
- Preserve A01's bounded expression construction, root-depth definition,
  logical-depth definition, structured rejection, recovery, and process-level
  stack-safety tests. Shared traversal must never recurse through callbacks.
- Preserve compiler dependency kinds, canonical module selection, token-based
  string/general-iteration evidence, exact range-operator spans, and evidence
  order.
- Preserve specialization module order, template pruning, closing,
  deduplication, provenance, diagnostics, and deterministic identity
  allocation after W03 deliberately fixes the local annotation/initializer
  discrepancy.
- Keep mechanical migrations byte-for-byte stable in AST and resolved dumps,
  diagnostics, reports, HIR, MIR, assembly, native output, and ownership
  traces. W03's explicitly reviewed identity/dump ordering delta is the sole
  intended observable change.
- Do not migrate the AST dumper, ordinary resolution, generic-template body
  resolution, type checking, semantic range discovery, HIR lowering, or later
  IR traversal without separate evidence that structure can be shared without
  hiding semantic control flow.
- Do not add mutable visitors, rewriting, folds, parent pointers, arenas,
  caches, registration APIs, generated walkers, or a visitor hierarchy shared
  by compiler IRs.

## Progress

- [x] W01 — Establish the iterative syntax traversal contract
- [x] W02 — Migrate non-semantic structural consumers
- [x] W03 — Restore and freeze specialization source order
- [ ] W04 — Migrate specialization discovery
- [ ] W05 — Audit the boundary and close A13

## PR-sized implementation sequence

### W01 — Establish the iterative syntax traversal contract

**Purpose:** create and characterize the shared structural boundary before a
production consumer depends on it.

- [x] Add private `syntax::walk` facade and implementation modules. Selectively
  re-export only `SyntaxNode`, `WalkControl`, `SyntaxVisitor`, and the traversal
  entry point as `pub(crate)` through `syntax`.
- [x] Represent compilation units, imports, declarations, class members,
  blocks, statements, expressions, type occurrences, and named-type
  occurrences as borrowed copyable node events.
- [x] Implement traversal with an explicit heap work stack. Emit enter, push
  leave, then push children in reverse so observations remain in declared
  forward order.
- [x] Make `Continue` and `Prune` behavior explicit. Always emit leave for an
  entered node and make leave return unit so pruning decisions occur only on
  enter.
- [x] Keep `TopLevelDeclaration`, `ClassMember`, `Statement`, `ForInSource`,
  `Expression`, call-argument, array-construction, optional-initializer, and
  bracket-bound child matches exhaustive. Do not use a wildcard that lets a
  child-bearing variant silently become a leaf.
- [x] Emit imports, complete types, and complete named types as opaque leaves.
  Preserve declaration, member, statement, expression, conditional-arm,
  argument, element, endpoint, and projection source order exactly as frozen
  by the accepted design.
- [x] Add a parsed all-shapes fixture covering every current child-bearing
  statement and expression family, both `for-in` sources, each array
  constructor and call-argument form, both optional-box initializers, and
  index/slice projections.
- [x] Add focused tests for exact compact event/source-slice order, source
  spans, balanced nesting, pruning at multiple node levels, continuation with
  later siblings, and absent optional children.
- [x] Document the private structural owner and its non-semantic boundary in
  [`PHASES_AND_IR.md`](../compiler/PHASES_AND_IR.md), linking to living syntax
  and phase contracts rather than copying the variant inventory.

**Tests:** Run `cargo test --locked -p skald-compiler syntax::walk`, the full
syntax unit suite, AST process-determinism cases, `cargo fmt --all -- --check`,
Clippy with warnings denied, `make check`, `make msrv-check`, and `git diff
--check`.

**Exit criteria:** one tested crate-private iterative engine defines the shared
syntax-node and child-order contract; it has no production consumer yet; every
current statement/expression family and pruning rule is covered; no semantic
type or policy enters the module; and all existing outputs remain unchanged.

#### W01 delivery record

The syntax facade now exposes a crate-private borrowed node vocabulary,
continue/prune control, balanced visitor events, and one iterative traversal
engine. Exhaustive source-shaped child enumeration lives in the private
`syntax::walk` implementation; complete types remain opaque and no production
consumer has migrated early.

Six focused walker tests cover the parsed all-shapes inventory, exact event and
span order, composite child order, opaque types, absent optionals, and balanced
local pruning at declaration, member, block, and expression boundaries. The
231 syntax tests and all 53 independent-process phase determinism tests pass.
`make check` passes with 3,154 compiler unit tests and all 629 selected golden
executions, and `make msrv-check` passes with Rust 1.82.0.

### W02 — Migrate non-semantic structural consumers

**Purpose:** prove the walker serves both expression-root and whole-unit
observation while preserving parser robustness and module dependency meaning.

- [x] Replace the private expression-child match in parser depth measurement
  with a visitor over one expression root. Keep current and maximum expression
  and logical depths as parser-owned state updated on paired expression
  enter/leave events.
- [x] Demonstrate exact equivalence at root depth one and across unary, postfix,
  binary, logical, call, construction, optional, and projection shapes before
  deleting the old pending-expression implementation.
- [x] Replace declaration/member/block/statement recursion in compiler
  dependency range collection with a whole-unit visitor. Match the enclosing
  `ForIn` statement to record `RangeForSource`, prune expression subtrees, and
  ignore type/import leaves.
- [x] Keep lexer-token collection for `StringLiteral` and `GeneralIteration`
  unchanged. Do not infer those dependencies from AST events.
- [x] Add module-graph coverage for ordered direct range sources inside
  functions, every body-bearing class member, generic class bodies,
  conditionals, loops, and explicit nested blocks.
- [x] Remove only the superseded structural recursion and imports. Keep the
  `Depths` result, dependency map, kinds, spans, canonical paths, and consumers
  in their existing modules.
- [x] Remove the transition-only dead-code and unused-re-export allowances from
  the syntax facade once the new traversal API has production consumers.
- [x] Update living parser or module documentation only if implementation
  reveals a structural guarantee not already stated by the accepted design and
  phase documentation.

**Tests:** Run syntax nesting and short-circuit depth tests, `cargo test
--locked -p skald-compiler --test expression_depth_robustness`, its documented
release-profile invocation, module-graph compiler-dependency tests, AST/module
process-determinism cases, and relevant range/iteration module tests. Then run
`make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** parser depth measurement and compiler-dependency range
collection use the shared iterative engine; no duplicated statement or
expression child enumeration remains in either consumer; A01 boundaries and
all dependency evidence are unchanged; and the shared module still owns no
consumer policy.

#### W02 delivery record

Parser expression-depth measurement now keeps its current and maximum depth
state in a visitor over one expression root. Exact focused assertions cover
leaf, unary, postfix, binary, logical, call, allocation, optional-box, array,
cast, and projection forms at root depth one. Both debug and release external
process watchdogs retain the bounded rejection and stack-safety guarantee.

Compiler dependency collection now observes `ForIn` statements through one
whole-unit traversal, prunes expression subtrees, and continues to derive
string-literal and general-iteration evidence exclusively from lexer tokens.
Focused graph coverage proves exact range-operator spans and source order
through functions, all five body-bearing class members, a generic class,
conditional arms, `while` and `for` loops, and an explicit nested block. The
superseded consumer recursions, imports, and syntax-facade transition
allowances have been removed.

Focused validation passed 22 syntax-nesting tests, six short-circuit parser
tests, two exact depth-equivalence tests, both debug and release process
watchdogs, 35 module-graph tests, 24 resolver iteration tests, 19 type-checker
iteration tests, and nine selected module/range/iteration/short-circuit process
determinism cases. The full `make check` gate passed with 3,157 compiler unit
tests, all 53 process-determinism cases, and all 629 golden executions;
`make msrv-check` passed with Rust 1.82.0.

### W03 — Restore and freeze specialization source order

**Purpose:** isolate the resolver's one intentional ordering correction before
its recursive scanner is mechanically replaced.

- [x] Add a focused specialization fixture with distinct closed generic class
  and interface applications in a local annotation and its initializer. Record
  current keys, allocated identities, provenance spans, diagnostics, and
  resolved dump order.
- [x] Confirm all other source-request locations already follow source spelling
  order, including parameters/results, class headers, fields/static
  initializers, casts/tests, allocations, arrays, calls, conditionals, both
  loop sources, assignments, and projections.
- [x] Change the existing scanner so a local type annotation is closed before
  its initializer is scanned. Keep local bindings out of their own initializer;
  semantic range inference already belongs to the later semantic request pass.
- [x] Update only assertions or deterministic snapshots whose identity order
  demonstrably follows from that correction. Preserve accepted programs,
  diagnostics and labels, provenance contents, HIR/MIR, native behavior, and
  ownership traces.
- [x] State the corrected type-before-initializer rule in the specialization
  scanner's module documentation and the existing generic compiler contract,
  without turning internal identity numbers into a language promise.
- [x] Keep this change on the existing recursive scanner. Do not adopt the
  shared walker in the same PR.

**Tests:** Run focused specialization coordinator, declaration, body, interface,
generic-class, generic-interface, and range-request tests; compare exact
resolved dumps; run generic/range process-determinism and relevant golden
suites; then run `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** specialization discovery follows source spelling order for
local annotations and initializers; the intended identity/dump delta is
explicit and fully characterized; every other result remains unchanged; and
the old scanner remains otherwise structurally intact for a clean W04
comparison.

#### W03 delivery record

The existing recursive source-request scanner now closes a local's declared
type before scanning its initializer. Its module documentation and the generic
compiler contract state that written order, keep binding scope with ordinary
body resolution, and describe numeric specialization identities as internal
compiler details. No shared-walker migration was included.

A focused fixture freezes the complete ordering delta for distinct nested
generic class and interface applications: canonical template/argument keys,
allocated class and interface identities, exact application-origin byte
ranges, empty diagnostics, and resolved-dump specialization order. Before the
correction, the initializer reserved `View<bool>` as `i0` and `Derived<...>` as
`c0`; afterward, the annotation reserves `View<i64>` as `i0` and `Base<...>` as
`c0`, with the initializer applications following as `i1` and `c1`.
Provenance contents and spans are unchanged. A second valid fixture proves that
the corrected class order continues through type checking to verified
preliminary MIR. Review of every remaining scanner branch confirmed source
spelling order for parameters and results, class headers, fields and static
initializers, casts and tests, allocations, arrays, calls, conditionals,
iterable and range loops, assignments, and projections.

Focused validation passed all 55 specialization tests, 26 generic-class tests,
22 interface tests, 25 range-language-item tests, and six generic/range
cross-process determinism cases. Full determinism passed for 36 selected
generic-class, generic-interface, and range golden leaves. No existing
expectation required an identity, diagnostic, HIR, MIR, native-behavior, or
ownership-trace update. The full `make check` gate passed with 3,159 compiler
unit tests, all 53 process-determinism cases, and all 629 golden executions;
`make msrv-check` passed with Rust 1.82.0.

### W04 — Migrate specialization discovery

**Purpose:** remove the final repeated full AST walk after its
identity-sensitive semantic contract and the shared traversal contract are
both stable.

- [ ] Implement the shared visitor at the specialization request owner and
  start it once for each `ModuleUnit` in the existing canonical module order.
- [ ] On generic class or interface declaration entry, retain the current rule
  that applications inside an unrequested template are discovered only after
  substitution closes that template, and prune the complete declaration.
- [ ] Close each emitted `TypeSyntax` or `NamedTypeSyntax` occurrence once with
  `SyntaxTypeCloser`. Keep lookup, diagnostic suppression/reporting choices,
  interning, recursion handling, request activation, and provenance in
  resolution.
- [ ] Preserve corrected source order across declarations, class members,
  statements, expressions, arguments, conditional arms, loop sources, and
  projections. Preserve module order separately from within-unit order.
- [ ] Remove superseded `visit_declaration`, `visit_member`, `visit_block`,
  `visit_statement`, `visit_expression`, call-argument, and expression-list
  recursion. Remove no semantic closing or coordinator logic.
- [ ] Keep ordinary resolver, generic-template body, and semantic range walks
  explicit. Do not route them through events as part of this migration.
- [ ] Update specialization owner documentation and focused debugging/testing
  guidance for the final boundary where useful.

**Tests:** Run the all-shapes walker fixture together with specialization tests
covering every type-bearing source location, nested generic arguments,
deduplication, repeated origins, recursive requests, failed requests,
unrequested-template pruning, semantic range extension, and module
permutations. Run exact AST/resolved/diagnostic process-determinism checks and
generic, interface, array, range, function-value, static-field, and object-cast
golden suites. Then run `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** explicit closed-application discovery uses the shared walker
and owns only semantic reactions to node events; its old structural recursion
is gone; exact request order, identities, origins, diagnostics, and downstream
behavior match the W03 baseline; and no resolver authority has moved into
syntax.

### W05 — Audit the boundary and close A13

**Purpose:** verify the shared abstraction reduced the intended maintenance
surface without attracting unrelated semantic traversal, then leave accurate
living and historical documentation.

- [ ] Inventory remaining production matches over syntax declarations,
  statements, expressions, call arguments, array arguments, optional
  initializers, and bracket bounds. Classify each as structural observation,
  semantic transformation/query, rendering, or parser construction.
- [ ] Migrate a remaining observer only when it repeats the accepted immutable
  event contract and the change is small. Keep transformations, scoped
  semantic walks, short-circuit queries, and renderers explicit.
- [ ] Confirm `syntax::walk` has one concise facade, cohesive work-stack
  implementation, exhaustive structural matches, narrow visibility, and no
  obsolete adapters or parallel child enumerators for migrated consumers.
- [ ] Confirm the all-shapes fixture covers the current AST and that adding a
  child-bearing statement or expression variant makes the central structural
  match require an explicit decision.
- [ ] Update [`PHASES_AND_IR.md`](../compiler/PHASES_AND_IR.md), the testing and
  debugging guides, and source module documentation to describe only the final
  living boundary and focused commands.
- [ ] Record any substantial additional candidate in a clearly named indexed
  discoveries document with evidence, owner, priority, and boundary. Do not
  widen the closure task to absorb it.
- [ ] Mark A13 complete with delivered behavior and validation, mark every
  roadmap checkbox complete, archive this roadmap, update active/archive
  indexes, and repair all relative links.

**Tests:** Run all focused syntax, depth, module-graph, resolver
specialization, generic, range, and determinism suites. From an artifact-free
snapshot or clean checkout run `make check`, `make msrv-check`, the documented
release expression-depth robustness check, `make docs-check`, and `git diff
--check`.

**Exit criteria:** one private iterative syntax owner supplies structural
walking to every intended A13 consumer; consumer semantics and phase
boundaries remain explicit; the source-order correction is the only accepted
observable delta; no superseded traversal remains in the migrated owners; all
quality gates pass; A13 is complete; and the accepted design and completed
roadmap are archived and indexed.

## Ordering and dependencies

W01 settles node vocabulary, event nesting, pruning, source order, and stack
behavior before production code depends on them. W02 validates that contract
against two simple but different observers and retains A01's robustness
evidence. W03 then changes the one known identity-sensitive ordering rule while
the old scanner still provides a direct comparison. W04 becomes a mechanical
migration against the corrected baseline. W05 audits adoption and closes the
finding only after every intended owner is stable.

A13 depends on the completed A01 expression-depth work and the stable resolver
publication, semantic-range, and language-item boundaries delivered by
A09–A12. It does not depend on another active roadmap. Unrelated standard
library, optional representation, optimization, and tooling work may proceed
independently, but syntax additions during W01–W04 must update both the accepted
walker contract and whichever migration baseline is current.

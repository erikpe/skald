# Cleanup Retrospective Review

Status: complete (2026-09-12). R01–R04 evidence, decisions, verification and
final reconciliation are accepted. This is an archived historical record;
the active audit tracks remaining work. The publication follow-up is complete
in the [archived publication ownership roadmap](PUBLICATION_OWNERSHIP_ROADMAP.md).

Reviewed: 2026-09-12, revision `64b6da73b41ed2ec6afe0e1401b3735484847e6d`.
The working tree was clean before this documentation task. Source inspection
therefore describes that revision, not uncommitted compiler changes.

This is the evidence record for the
[retrospective roadmap](CLEANUP_RETROSPECTIVE_ROADMAP.md). Original promises
remain in the [cleanup audit](../roadmaps/CODEBASE_CLEANUP_AUDIT.md); current behavior is
owned by the [phase contracts](../compiler/PHASES_AND_IR.md). Tests below were
inspected as evidence, not rerun as part of R01. A named test covers its fixture
and assertions, not all possible programs or future product additions.

## History and attribution

| Change | Historical evidence | Distinction from pre-existing behavior |
| --- | --- | --- |
| A07 | `45d15431` | Entry selection moved to module ownership with driver compatibility exports; ordinary provider and graph semantics were retained. |
| A08 | `a84e0f4b` | Extended an existing category-predicate service with closed queries and resolved lifecycle facts; HIR plan construction already existed and remains in type checking. |
| A09 and A43 | `312008ee` | Despite its dependency-check commit title, this commit also introduces stage products and candidate publication. The parent validator already restored ordinary class tables and cleared generated/body/dispatch products; A09 relocates that policy and adds independent-class survival coverage. |
| A10 | `dee47aca` | Semantic range probing and specialization coordination already existed. Added named deltas/completion, work counters, explicit growth checks, and the no-range fast path. |
| A11 | `c19bf4cb` | Existing expression-type prediction moved to a named provisional query with explicit states; authoritative type checking and lexical scope storage remain. |
| A12 | `e044178a` | Canonical paths and collection implementation were consolidated; validators and dependency provenance behavior already existed. Body environments now reuse a constructed context. |
| Supporting measurements | `3fc0d426` | Added the reproducible cleanup harness. Its existence does not establish a before/after speedup for subsequent changes. |

History was inspected with `git log` for the relevant owners and targeted
`git show`/`git diff` comparisons, including publication against `312008ee^`.
The later `8f64a5b3` function-field-array crash fix is present in the reviewed
revision but is outside this retrospective's scope.

## Outcome traceability

Disposition meanings: **fulfilled** means the inspected implementation meets
the stated bounded outcome; **deliberately narrowed** identifies a defensible
smaller endpoint needing explicit acceptance; **outstanding** means the original
architectural obligation is not yet established. These were proposals at R01; the final R04 section records their disposition.

| Finding and original outcome | Current owner/product and evidence below | Proposed disposition | Limitation or residual risk |
| --- | --- | --- | --- |
| A08: remove resolver dependence on type checking | Closed capability queries over resolved IR; capability and publication tests | Fulfilled | Source guard does not cover the neutral service as an owner. |
| A08: neutral lifecycle facts; preserve HIR plans and diagnostic paths | Resolved lifecycle availability plus type-check-owned concrete plans; parity tests | Fulfilled | Parallel availability/plan algorithms can drift; fixture parity is not a proof over every recursive graph. |
| A08: ordinary/generic eligibility and atomic rejection | Shared categories, closed requirement query, publication tests | Fulfilled for bounded migration | Publication completeness remains R02's separate obligation; no exhaustive ordinary/generic cross-product test was established. |
| A09: named collection and body products with stable identities | `CollectedDeclarations`, `ResolvedBodies`, `BodyResolutionStage`; determinism tests | Fulfilled | Orchestration still relies on local ordering and mutable interning. |
| A09: separate candidate publication so rollback need not remember every table | Consuming `CandidateProgram` and saved `OrdinaryProgramProducts` | Outstanding | R02 requires owned selection and exhaustive assembly in the publication ownership roadmap; manual restoration remains an interim implementation. |
| A09: preserve failed-dependency behavior and successful dumps | Ordered class/interface validation; publication and permutation assertions | Fulfilled for covered cases | Current-run determinism is not byte comparison against the pre-cleanup compiler; no complete field/rejection matrix yet. |
| A10: explicit delta, termination contract, and measured work | `SemanticRangeRequestDelta`, `SemanticRangeCompletion`, counters | Fulfilled | Growth is a continuation condition, not independently a proof that the key universe is finite; existing specialization recursion controls remain essential. |
| A10: optimize repeated work only with evidence | No-range fast path; probe-local inputs and state | Deliberately narrowed | Work counters establish avoided operations. No comparable before/after timing or RSS result was established by R01; affected-body scheduling remains unimplemented and unjustified here. |
| A11: explicit provisional query and consumer distinctions | `ProvisionalExpressionType`; operator and receiver tests | Fulfilled | `.known()` intentionally collapses Unknown and Invalid for many consumers; future consumers must choose deliberately. |
| A11: cache binding facts | Query still scans lexical scope values by selected binding identity | Deliberately narrowed | A25 owns indexed lookup; this was documented deferral, not delivered caching. |
| A12: canonical catalog and common diagnostic-origin collection | `CanonicalModule`, requirement and declaration origin products | Fulfilled | Separate requirement families still traverse the graph; collection is shared code, not a fused traversal. |
| A12: separate validators, one body context, IDs in later phases | Existing structural validators and shared stage constructor | Fulfilled | Probe context deliberately has no string selection. Cross-feature origin ordering needs evidence beyond catalog mapping tests. |

## A08 — Neutral capability service

[Closed queries](../../crates/skald-compiler/src/type_capabilities/closed.rs)
consume `ResolvedProgram`, close class/interface requirement subjects, and
lazily compute lifecycle facts through a query-local `OnceCell`.
[Lifecycle availability](../../crates/skald-compiler/src/type_capabilities/lifecycle.rs)
separates constructor and assignment facts, follows base/inline dependencies,
uses visit states for recursion, and iterates array-dependent invalidation.
Results are booleans and identity-based failure paths, not HIR plans.
[Type-check capabilities](../../crates/skald-compiler/src/typeck/capabilities.rs)
retain concrete plan construction. This is an earlier-IR semantic service,
not representation-independent infrastructure or a relocated whole type checker.

Exact evidence in [closed-query tests](../../crates/skald-compiler/src/type_capabilities/tests/closed.rs):

- `declaration_roles_delegate_to_stored_alias_optional_array_and_shared_owners`
- `recursive_lifecycle_queries_follow_optional_array_and_shared_plans`
- `effective_contract_allows_alias_and_shared_interface_but_rejects_inline_optional_interface`

In [type-check capability tests](../../crates/skald-compiler/src/typeck/tests/capabilities.rs),
`phase_neutral_lifecycle_facts_match_hir_plan_availability` compares constructor,
assignment, failure paths, and canonical array availability across every entry
in its fixture, including deliberately unavailable operations. This direct
parity fixture does not cover every recursive cycle. Supporting tests include
`recursive_synthesis_terminates_and_marks_the_capability_unavailable` and
`shared_edges_do_not_require_the_pointee_copy_capability`.
Publication tests below cover repeated generic failures and nested lifecycle
diagnostic paths. They support migration compatibility, not exhaustive parity.

## A09 — Stages and publication

[Stages](../../crates/skald-compiler/src/resolve/resolver/program/stages.rs)
name collection, completed bodies, and body context. The
[resolver](../../crates/skald-compiler/src/resolve/resolver/program/resolver.rs)
still orchestrates declaration validation, specialization, hierarchy/dispatch
construction, authoritative bodies, interner completion, and publication in a
long ordered method. Named products improve navigation without making every
ordering constraint a type-level invariant.

[Publication](../../crates/skald-compiler/src/resolve/resolver/program/specialization/publication.rs)
consumes a candidate and invokes immutable validators, then applies rejection.
Class rejection fails generated class identities, restores classes/hierarchy,
and clears class definitions, function definitions, and virtual families.
Interface rejection fails interface specializations and restores interfaces.
The class policy rejects the generated class family together; it is not
per-class salvage. Class validation precedes interface validation. This list
describes inspected behavior, not R02's still-pending full consistency proof.

Exact evidence in [specialization tests](../../crates/skald-compiler/src/resolve/resolver/program/specialization/tests.rs):

- `contextual_requirement_failures_reject_declaration_publication_after_identity_discovery`
  asserts failed specialization state, one requirement diagnostic, and no classes.
- `repeated_failed_keys_emit_once_and_restore_coherent_specialization_products`
  asserts repeated-use evidence, ordinary `Plain` preservation, failed generated
  identities, and cleared bodies/virtual families.
- `invalid_interface_candidate_preserves_independent_class_publication` asserts
  surviving ordinary declarations, completed `Box<i64>`, failed interface
  specializations, and retained class/function bodies.
- `mixed_interface_and_class_recursion_does_not_publish_a_failed_dependency_graph`
  covers mixed recursion failure.
- `lifecycle_requirement_diagnostics_include_the_existing_field_path` covers
  requirement provenance; `cross_module_reuse_and_source_permutation_have_identical_dumps`
  covers order stability.

R02 must inventory every retained field and rejection combination, including
references to rejected identities. Existing assertions do not automatically
cover a new table added to `ResolvedProgram`. No new dangling-reference defect
is asserted by this review. That is the central unanswered acceptance question.

## A10 — Semantic range discovery

[Range completion](../../crates/skald-compiler/src/resolve/resolver/program/semantic_range_requests.rs)
skips work without a validated range item or concise syntax. Each round clones
the interner, collects requests with local diagnostics/address-taken state,
and applies them through the existing specialization coordinator. Only strict
class-specialization count growth permits another round; equal counts stop.
Probe diagnostics are discarded; extension diagnostics belong to the
authoritative coordinator. Provisional declaration setup precedes the cloned
probe interner, so isolation should not be described as forbidding all shared
interning during the entire completion operation.

Exact evidence in [range tests](../../crates/skald-compiler/src/resolve/tests/range_language_item.rs):

- `ordinary_expressions_skip_semantic_range_discovery`: validated range item
  but no range syntax gives zero rounds, revisits, and copies.
- `semantic_range_delta_follows_local_endpoint_bindings`: two of each counter
  and one selected range source.
- `deeply_nested_concise_ranges_discover_new_endpoint_types_to_a_fixpoint`:
  four of each counter and three endpoint types.
- `deferred_generic_ranges_select_distinct_keys_for_the_same_source_span` and
  `specialized_generic_bodies_discover_deferred_and_nested_range_requests`:
  generated-body discovery and source-span reuse.
- `semantic_range_request_probe_does_not_publish_body_diagnostics`: two missing
  member diagnostics, without duplicate probe diagnostics or range cascade.
- `failed_outer_range_does_not_request_ranges_from_its_unresolved_body`:
  failure limits nested discovery.

These are structural work baselines and behavioral assertions. The
[measurement procedure](../development/CLEANUP_MEASUREMENTS.md) specifies
comparable profiles, inputs, repetitions, and dispersion. R01 did not locate
and evaluate a paired pre/post report; no wall-time or memory gain is accepted.
Counter improvements alone do not justify a resumption scheduler.

## A11 — Provisional expression types

[The query](../../crates/skald-compiler/src/resolve/resolver/body/provisional_type.rs)
returns Known, Unknown, or Invalid. Contextual optional constructors are
unknown; a uniquely selected overload contributes its output; failed selection
is invalid. Primitive inference remains provisional. Binding lookup still
scans scope values, and [A25](../roadmaps/CODEBASE_CLEANUP_AUDIT.md#a25--use-identity-indexed-lookup-for-resolved-bindings)
owns its replacement. Operator selection handles unknown contextual operands
and invalid selection separately; consumers needing only a candidate use the
named conversion to `Option`.

Colocated `operator_output_is_known_only_for_one_selected_candidate` and
`only_known_provisional_types_convert_to_an_option` pin the result conversion.
[Operator tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs)
provide behavior-level evidence:

- `failed_operator_selection_used_as_a_receiver_keeps_resolution_diagnostics`
- `selected_operator_output_does_not_bypass_receiver_carrier_rules`
- `contextual_optional_rhs_does_not_supply_a_provisional_receiver_type`
- `generic_operator_selection_supports_structural_rhs_and_output`
- `primitive_precedence_remains_the_existing_hir_with_reachable_protocols`

The receiver cases assert a single resolver diagnostic; the carrier test also
pins its message. This does not exhaust every member/call/bracket/iteration
consumer or prove multi-error ordering for all Unknown/Invalid combinations.
R03 should select missing behavior only after identifying a specific invariant.

## A12 — Language-item discovery

[The module catalog](../../crates/skald-compiler/src/module/language_items.rs)
owns seven canonical module roles and dependency-kind mappings.
[Origin collection](../../crates/skald-compiler/src/resolve/resolver/program/language_item_sources.rs)
shares collection helpers, retains explicit spans before compiler-dependency
spans per edge, and falls back to a canonical entry span only when no requiring
spans exist. String literals use compiler-dependency spans alone. Declaration
collection selects canonical modules while leaving protocol-specific validation
separate. Four requirement collections still traverse the graph separately.
The audit's repeated-scan wording is corrected in this task.

The body stage constructs one environment for authoritative bodies. Range
probes use the same constructor per round, intentionally without a validated
string product. Validators continue selecting identities before lower phases.

Exact [module graph tests](../../crates/skald-compiler/src/module/graph/tests.rs):

- `explicit_and_synthetic_std_str_dependencies_coalesce_without_losing_kind`
- `for_in_adds_typed_std_iter_evidence_and_reuses_an_explicit_edge`
- `direct_range_sources_alone_add_typed_std_range_evidence`
- `operator_punctuation_never_creates_a_std_ops_dependency`
- `canonical_iteration_dependency_uses_ordinary_missing_and_ambiguity_rules`
- `synthetic_std_str_dependency_uses_ordinary_missing_ambiguity_and_case_rules`
- `canonical_error_module_reaches_string_module_through_an_ordinary_import`
- `synthetic_string_dependencies_may_participate_in_cycles`

Validator evidence includes
`canonical_iteration_module_is_dependency_free_and_valid_as_an_entry` and
`canonical_declaration_failure_labels_the_bad_component_and_requirement_site`
in [iteration tests](../../crates/skald-compiler/src/resolve/tests/iteration.rs);
`concise_syntax_validates_replacement_canonical_range_declarations` and
`malformed_range_class_components_are_rejected_structurally` in range tests;
`validates_the_exact_language_item_and_allocates_source_ordered_literal_data`
and `rejects_every_structural_language_item_mismatch_before_hir` in
[string tests](../../crates/skald-compiler/src/resolve/tests/strings.rs); and
`rejects_malformed_replacement_io_intrinsic_declarations` in
[intrinsic tests](../../crates/skald-compiler/src/resolve/tests/intrinsics.rs).
Missing-provider fixtures support missing canonical module behavior, but do not
alone establish every driver-level disabled-stdlib option path. Graph-edge tests
also do not prove exact cross-feature ordering of final validator diagnostics.
Those are verification questions for R03, not demonstrated regressions.

## Supporting boundaries and determinism

A07's [module entry owner](../../crates/skald-compiler/src/module/entry.rs)
and driver compatibility export keep selection vocabulary out of the driver
dependency direction. `positional_and_logical_selection_intern_the_same_rooted_module`
in graph tests observes selection equivalence.

[Phase-boundary tests](../../crates/skald-compiler/tests/phase_boundaries.rs)
include `production_phase_dependencies_follow_the_forward_pipeline` and
`policy_rejects_reverse_edges_and_accepts_lowering_inputs`. The scanner handles
direct/grouped crate paths and relative paths leaving a phase root, excludes
comments/literals and conventionally named test files, and has one file-scoped
MIR retention exception. It is not a Rust name resolver, macro expansion engine,
transitive dependency proof, or semantic ownership check. In particular,
`type_capabilities` is absent from the scanned owner policies. The inspected
service currently uses resolved IR, but the guard does not prevent a future
reverse dependency inside it. Follow-up is recorded in
[discoveries](../roadmaps/CLEANUP_RETROSPECTIVE_DISCOVERIES.md).

[Process determinism tests](../../crates/skald-compiler/tests/pipeline_determinism.rs)
include `generic_module_phase_products_are_deterministic_across_processes`,
`generic_interface_phase_products_are_deterministic_across_processes`,
`generic_operator_phase_products_are_deterministic_across_processes`,
`range_phase_products_are_deterministic_across_processes`, and
`string_language_item_diagnostics_are_deterministic_across_processes`.
Their fixture permutations compare current outputs; they do not compare the
pre-cleanup and post-cleanup binaries. Historical golden passes are useful
compatibility evidence but expected-output changes require separate review.

## Verification record and next decisions

Historical full `make check`, golden, determinism, and MSRV results remain in
the audit's delivery records. They have not been rerun or reclassified as
current results by R01. No production code or tests changed in this task.

Current R01 checks: `make docs-check` and `git diff --check` passed for the
documentation changes on the reviewed revision. Read-only source/history
inspection supplies the structural evidence above. No benchmark was run.

R01's outcome proposals and explicit gaps are complete. R02 must settle the
publication boundary and enumerate retained products before acceptance; R03
must decide which uncovered assertions warrant tests; R04 must reconcile
statuses and readiness. The guard follow-up can remain separate after this
roadmap. Indexed bindings and performance work remain with their existing
audit owners, avoiding duplicate backlog entries.


## R02 — Publication acceptance decision

Reviewed 2026-09-12 at `72424ff2f73b7430697d0d75597f9792a969f949`, with a clean
working tree before this task. R01's historical record remains unchanged.
This section records a design decision, not an implemented representation change.

**Decision:** retain the current rejection policy as compatibility behavior,
but do not accept its manual rollback implementation as A09's final endpoint.
Require explicit owned product selection and exhaustive final assembly through
[the publication ownership roadmap](PUBLICATION_OWNERSHIP_ROADMAP.md). This is
an outstanding maintainability obligation, not a newly demonstrated compiler
correctness failure. R03 may verify today's contract before that follow-up.

### Complete field disposition inventory

The authoritative field list is
[`ResolvedProgram`](../../crates/skald-compiler/src/resolve/ir/declarations.rs).
The following covers all 31 fields. **Keep** means retain the candidate field,
not prove that every referenced declaration remains published. **Restore**
means select the saved ordinary snapshot. **Clear** means replace with an empty
table. Success keeps every field. Combined rejection applies the class column
first and then the interface column to the resulting product.

| Field | Owner/dependencies and retention rationale | Class rejection | Interface rejection |
| --- | --- | --- | --- |
| `modules` | Request module identities and provenance | Keep | Keep |
| `external_links` | Ordinary external function linkage plan | Keep | Keep |
| `module_bindings` | Module import identities | Keep | Keep |
| `ordinary_bindings` | Source-selected ordinary names; diagnostic identity evidence | Keep | Keep |
| `module_declarations` | Source declaration indexes, including template identity slots | Keep | Keep |
| `class_templates` | Original generic syntax identity inventory | Keep | Keep |
| `interface_templates` | Original interface template inventory | Keep | Keep |
| `interface_template_semantics` | Definition-site requirements and diagnostic origins | Keep | Keep |
| `type_parameters` | Template parameter identities used by retained evidence | Keep | Keep |
| `template_semantics` | Class template requirements and body-origin evidence | Keep | Keep |
| `generic_specializations` | Closed keys, reserved class IDs, provenance and state transitions | Mark all entries with class identities failed | Keep |
| `generic_interface_specializations` | Closed interface keys, type uses and transitions; inspected after class rejection | Keep until interface validation | Mark entries with interface identities failed |
| `function_types` | Interned signatures can mention rejected types; preserve numbering/evidence | Keep | Keep |
| `address_taken_callables` | Accumulated body facts can reference cleared candidate bodies | Keep as partial evidence | Keep as partial evidence |
| `array_types` | Interned element kinds can mention rejected types | Keep | Keep |
| `optional_types` | Interned payload kinds can mention rejected types | Keep | Keep |
| `optional_box_types` | Interned optional/object references can mention rejected types | Keep | Keep |
| `iterable_language_item` | Canonical template/requirement identities; not a closed application | Keep | Keep |
| `operator_language_item` | Canonical template/requirement bundle | Keep | Keep |
| `range_language_item` | Canonical templates and declaration slots, not generated range classes | Keep | Keep |
| `string_language_item` | Canonical ordinary string class and field IDs | Keep; verify snapshot correspondence in R03 | Keep |
| `literal_data` | Source-ordered bytes and spans independent of body publication | Keep | Keep |
| `declarations` | Ordinary function signatures may contain rejected generated types | Keep as partial evidence | Keep as partial evidence |
| `definitions` | Completed ordinary and generated-dependent function bodies | Clear all, including ordinary bodies | Keep |
| `classes` | Ordinary snapshot or candidate ordinary/generated class declarations | Restore | Keep |
| `interfaces` | Ordinary snapshot or candidate interface declarations | Keep until interface validation | Restore |
| `hierarchy` | Derived class hierarchy aligned with class snapshot | Restore | Keep |
| `virtual_families` | Dispatch derived from candidate classes and interfaces | Clear | Keep |
| `class_definitions` | Completed class bodies dependent on closed declarations | Clear all | Keep |
| `entry_function` | Ordinary selected function ID, not a promise its body survives | Keep | Keep |
| `span` | Entry source span | Keep | Keep |

The snapshot is not a completely independent ordinary program: ordinary
signatures and claims can already refer to generated identities. The class
snapshot is captured after ordinary interface claims; its hierarchy is built
at that point. The interface snapshot is captured before specialization.
These capture points in the resolver are part of the compatibility contract.
Moving them earlier or rebuilding an ordinary program would change behavior.

Related state outside these fields: `OrdinaryProgramProducts` holds only the
three saved tables; the type interner is finished before candidate assembly;
body-local lookup state and probe state do not become publication owners.
Diagnostics and resolution measurements belong to `ResolveOutput` and are not
rolled back. Class `fail_class` and interface `fail_all` preserve keys, reserved
identities, origins and prior transitions while recording failure; they do not
compact or recycle identities.

### Success, rejection and consumer boundaries

1. Success: both validators return true and the candidate is returned intact.
   Other resolution diagnostics can still exist; successful specialization
   validation does not alone make the whole compilation valid.
2. Class failure: class validation emits its diagnostics, all generated class
   entries with identities become failed, the ordinary class/hierarchy snapshot
   is restored, and all bodies/virtual families are cleared. Independently valid
   generated classes are not salvaged. This all-class-family policy predates A09.
3. Interface validation follows the class decision. It first checks closed type
   uses with `type_is_fully_published`; missing class/interface declarations in
   nested signatures, arrays, optionals or shared targets cause rejection
   without another interface diagnostic. Otherwise contextual and bound checks
   emit their own diagnostics. This is a dependency check over those uses,
   not a verifier for every reference in `ResolvedProgram`.
4. Interface-only failure restores ordinary interfaces and marks generated
   interface entries failed. It preserves the class product and bodies, as the
   independent-class regression requires. It does not revalidate all class claims
   against the restored interfaces. Such cross-links remain partial error evidence.
5. Combined failure performs both actions in that order. Independent interface
   candidates can survive class rejection if their checks succeed; if one
   interface candidate fails the family is rejected, not salvaged per key.

The driver checks `diagnostics.has_errors()` before calling type checking in
[`finish_compilation`](../../crates/skald-compiler/src/driver/pipeline.rs).
Thus error-bearing resolved output is inspectable diagnostic state, not a
closed program ready for HIR. The public resolved structure itself has no type
seal enforcing that precondition. Retained interned/signature references must
not be represented as a promise that all their targets survive rejection.
No new consumer may lower or execute these partial products. A separate public
success/error IR redesign is outside this decision.

### Alternatives and selected representation

| Alternative | Extension cost and intermediate states | Copy/migration implications | Decision |
| --- | --- | --- | --- |
| Keep manual mutation plus this inventory and tests | One mutation list must remember every new dependent field; inventory can drift; partially restored candidate exists between validators | Smallest migration; current ordinary snapshots are cloned again on rejection | Useful interim contract, insufficient for original A09 maintainability goal |
| Own selection products and assemble once exhaustively | New fields must be assigned to a product and explicitly destructured/assembled; coupled body/dispatch fields move together; validators receive an explicit selected view | Keep existing snapshot timing; move selected owned snapshots at rejection to avoid the extra restore clones where feasible; moderate private refactor | Selected |
| Rebuild a fully closed ordinary program on failure | Eliminates some partial references but introduces another resolution/re-interning policy and changes diagnostic products | High semantic and identity risk, potentially repeated frontend work | Excluded |

The selected private design separates retained request/declaration evidence,
class declaration/hierarchy selection, interface selection, and completed
bodies/dispatch. Class rejection selects ordinary class/hierarchy plus empty
bodies/dispatch; interface rejection selects ordinary interfaces without
implicitly clearing class bodies. The interface validator must see the
class-selected view before its own decision. Explicit owned selection and
exhaustive destructuring (without `..` or a default-filled assembly) force a
new field to receive a disposition. This cannot prove semantic classification
correct: tests and review still check which product owns a new field.

Preserve success IDs, dumps, diagnostic ordering, all-class-family rejection,
independent-class survival, and partial diagnostic evidence. Do not implement
per-specialization salvage, rebuild interning, silently clear retained evidence,
or claim that error output is closed. Snapshot copying before candidate
mutation remains necessary unless separately redesigned; no speedup is claimed.

### Test obligations and gaps

Existing tests are linked in the A09 section above. Their exact assertions map
as follows; a blank area is an obligation for R03, not an assumed failure.

| Contract | Existing named evidence | Remaining verification |
| --- | --- | --- |
| Success keys, identities and dumps | `cross_module_reuse_and_source_permutation_have_identical_dumps`; process determinism suites | Preserve equivalent products when migrating selection; current tests alone are not historical byte comparisons |
| Ordinary classes and repeated failure origins | `repeated_failed_keys_emit_once_and_restore_coherent_specialization_products` | Assert restored hierarchy correspondence, retained ordinary signatures/entry, and the expected partial nature of their type references |
| Class-dependent interface rejection | `mixed_interface_and_class_recursion_does_not_publish_a_failed_dependency_graph` | Add a non-recursive contextual class failure with dependent and independent interface applications to distinguish dependency rejection from recursion failure |
| Interface failure preserves valid class bodies | `invalid_interface_candidate_preserves_independent_class_publication` | Exercise claims/dispatch that mention an invalid interface; establish safe diagnostic inspection and blocked lowering, not false closure |
| Class failure clears dependent executable products | `repeated_failed_keys_emit_once_and_restore_coherent_specialization_products` | Fixture currently lacks a populated virtual family; use one that can observe dispatch clearing |
| Retained canonical and interned evidence | No combined rejection assertion established in R01/R02 | Include string/literal metadata, optional/array/function types and address-taken evidence where a meaningful consumer boundary can be asserted |
| Diagnostic ordering | Repeated-key and field-path tests; interface validators inspected | Combined rejection should retain causal diagnostics without cascades, and be stable under supported module permutations |

### Acceptance and blocking scope

R02 is complete as a design decision and inventory. A09's publication ownership
outcome remains outstanding pending the separate roadmap. R03 can characterize
current rejection behavior now; R04 can close this retrospective with that
explicit prerequisite still open. Changes that add candidate-dependent fields,
change rejection/publication, or assume closed failed output must wait for the
ownership follow-up or explicitly incorporate its contract. Independent MIR
work and indexed binding lookup need not wait if they preserve this boundary.
The discovered risk is owned by resolver publication, priority P1, and indexed
in the discoveries record. No production defect has been demonstrated by this
source review, and no production fix is silently bundled here.

R02 validation: field names were checked against the current struct; existing
assertions and rejection/helper implementations were inspected. `make docs-check`
and `git diff --check` passed. No compiler tests or benchmarks were run for
this documentation-only design task; R03 owns execution and added coverage.


## R03 — Verification progress and blocker

Started from `f50e5596a04b6fb1a5483b4fc9faf1f8fe32fe59` with a clean tree.
The focused [publication test module](../../crates/skald-compiler/src/resolve/resolver/program/specialization/publication_tests.rs)
adds four behavioral cases, without changing production compiler code:

| Test | Result and obligation |
| --- | --- |
| `class_rejection_clears_populated_dispatch_and_bodies_but_retains_evidence` | Passes. A valid control has populated dispatch/bodies; rejection clears them, restores the ordinary base relation, preserves entry/signatures and interned optional/array/function evidence, and remains dumpable. |
| `contextual_class_rejection_rejects_dependent_interface_family_without_cascade` | Passes. Non-recursive contextual class failure rejects both dependent and independent members of the interface family and emits only the causal requirement diagnostic. |
| `interface_rejection_preserves_class_dispatch_and_module_ordered_diagnostics` | Passes. Interface-only rejection preserves ordinary class dispatch/bodies and yields identical diagnostic/debug and resolved dumps under module source creation permutations. This does not establish closure of claims mentioning rejected interfaces. |
| `class_rejection_preserves_canonical_string_fields_and_literal_bytes` | Fails with a compiler panic during semantic range probing, before publication. Intended string field/literal assertions remain enabled and unchanged. |

The last case is a demonstrated failure, not an accepted partial-product
behavior. Its backtrace and bounded repair scope are described in
[discoveries](../roadmaps/CLEANUP_RETROSPECTIVE_DISCOVERIES.md#prevent-range-probing-from-consuming-absent-class-declarations).
The public compiler's internal test loader uses the real canonical library;
no artificial missing class was injected. The source's invalid generic
application should produce a diagnostic rather than panic.

Focused execution: `cargo test --locked -p skald-compiler publication_tests --lib`
reported three passed and one failed. The failing test was rerun with
`RUST_BACKTRACE=1` to locate the probe/class-body/member-selection path.
An initial fixture spelling error (`:` instead of `extends`) was corrected
before these results; it was not a compiler regression.

R03 remains incomplete. The capability, range, provisional-consumer and
provenance acceptance questions remain subject to existing coverage and the
remaining verification after repair. No benchmark or performance claim was
added. Publication ownership migration and retrospective closure are blocked
by the reproduced panic; independent fixes remain possible.


Repository validation after correcting a needless-borrow Clippy finding:
`make check` passed formatting, workspace all-target compilation, Clippy and
documentation validation, then failed in compiler unit tests with **3,103 passed,
1 failed, 0 ignored**. The sole failing compiler test is the retained probe-panic
regression. Later integration/golden/runtime targets were not reached by this
invocation. `make msrv-check` passed on Rust 1.82. Documentation and
`git diff --check` also pass. These are current results, not inherited delivery
claims; a green full gate and R03 acceptance still require the scoped repair.


### Repair of the reproduced materialization failure

The scoped repair follows the R03 discovery without implementing the separate
publication ownership redesign. Investigation found two consumers of the
incomplete declaration family: range probes and authoritative body resolution.
The latter could also panic after the probe was skipped. Lifecycle requirement
queries subsequently encountered missing generated classes in retained types.

Both body stages now require successful generated declaration materialization.
Structural closed requirements still report the original source error;
default-construction, copy and assignment requirement checks are deferred when
reserved class identities lack materialized declarations. Their absence is not
interpreted as proof of unavailable lifecycle operations. The same policy is
used for class and interface requirement validation. Publication's rejection
policy remains unchanged.

This intentionally changes early materialization-error output: address-taken
facts and body-only diagnostics are not collected against missing declarations.
The existing regression assertion was changed from nonempty address-taken
metadata to empty metadata for this stage-specific reason. Valid compilation
and errors discovered after successful materialization still run bodies.
The string regression now includes a valid control exercising actual range
probing, then verifies zero probe rounds/copies on failed materialization while
retaining its exact single-diagnostic and canonical-field/literal assertions.

R03 acceptance remains a separate verification task; repairing this blocker
does not complete its remaining review obligations or the ownership roadmap.


Repair validation (2026-09-12): `make check` passed, including 3,104 compiler
unit tests, 53 process-determinism tests, runtime/doc checks, and all 629 golden
cases. `make msrv-check` passed on Rust 1.82. Formatting, Clippy, documentation
links and diff hygiene passed. The discovered panic is resolved; these results
supersede the earlier failing execution for the repair, without retroactively
changing the R03 discovery record. Remaining R03 acceptance review is pending.


## R03 — Final coverage reconciliation

The remaining review started at `4c3495e163e0601a38203a3a961d7ebfe6c6c87c`
with a clean working tree, after the scoped panic repair. This section
supersedes the earlier pending-gap statements for R03; the earlier failure
record is retained as history. No further production behavior changes were
needed during this final verification task.

### Publication obligations

The tests below are in [publication tests](../../crates/skald-compiler/src/resolve/resolver/program/specialization/publication_tests.rs)
unless another owner is linked. The disposition is acceptance of the tested
current behavior, not completion of the planned ownership redesign.

| R02 obligation | Evidence and final disposition |
| --- | --- |
| Success identity/dump equivalence | Existing `cross_module_reuse_and_source_permutation_have_identical_dumps` and process determinism tests remain the current-behavior baseline. Historical pre-cleanup binary equivalence is not claimed. Before/after equivalence for the future representation migration belongs to its P01/P02 tasks, not a prerequisite to accept this test-only retrospective. |
| Ordinary classes, hierarchy, signatures, entry and partial type evidence | `class_rejection_clears_populated_dispatch_and_bodies_but_retains_evidence` compares a valid control with rejection, checks restored base identity, signature count and entry, retained interned types, and safe dumping. Accepted. |
| Non-recursive dependent and independent interface rejection | `contextual_class_rejection_rejects_dependent_interface_family_without_cascade` verifies the family-level policy and the single causal diagnostic. Accepted; individual interface salvage is not promised. |
| Rejected interface claims in retained class products | New `rejected_interface_claims_remain_dumpable_partial_class_evidence` proves the retained claim names an absent interface while class bodies/dispatch remain and dumping succeeds. The added case in [driver reporting](../../crates/skald-compiler/src/driver/tests/reporting.rs) `singleton_source_failures_stop_after_the_owning_frontend_phase` verifies compilation stops at failed resolution without entering type checking, MIR or backend phases. Accepted as partial diagnostic evidence, explicitly not closed IR. |
| Clearing populated dispatch and bodies | The valid control in `class_rejection_clears_populated_dispatch_and_bodies_but_retains_evidence` establishes these tables were populated before testing rejection behavior. Accepted. |
| Canonical fields/literals, interned types and address-taken evidence | Repaired `class_rejection_preserves_canonical_string_fields_and_literal_bytes` checks exact retained canonical field IDs and literal bytes. The valid control probes ranges; rejected materialization does not. Early-failure address-taken facts are intentionally empty under the repaired contract, while the valid control's facts are populated. Existing interface-only rejection tests retain completed bodies. Accepted; no guarantee of arbitrary closed references in error output is inferred. |
| Combined rejection diagnostic ownership and order across modules | New `dependent_rejection_diagnostics_and_products_ignore_module_creation_order` verifies exact diagnostic debug evidence and resolved dumps across source creation permutations, rejected class/interface tables and no cascade. `interface_rejection_preserves_class_dispatch_and_module_ordered_diagnostics` covers the interface-only case. Accepted. |

The new driver case intentionally uses the same small rejected-claim scenario
as the owner test: one observes the retained resolved product, the other the
public compilation phase boundary. Neither calls type checking directly on an
error-bearing program. The tests do not require arbitrary lower phases to
accept diagnostic-only state.

### Other frontend obligations

| R01 question | Evidence and disposition |
| --- | --- |
| Recursive neutral lifecycle/HIR parity | Extended [capability tests](../../crates/skald-compiler/src/typeck/tests/capabilities.rs) `recursive_synthesis_terminates_and_marks_the_capability_unavailable` to invoke the existing parity helper. It now compares both operations and diagnostic paths for the recursive fixture, alongside the existing optional/array aggregate parity fixture. Accepted as focused parity coverage; exhaustive graph enumeration is not required. |
| Ordinary/generic eligibility and provisional consumers | Reused closed-query role tests and the operator suite, including `generic_operator_bound_closes_to_primitive_intrinsic`, `failed_operator_selection_used_as_a_receiver_keeps_resolution_diagnostics`, `selected_operator_output_does_not_bypass_receiver_carrier_rules`, and `contextual_optional_rhs_does_not_supply_a_provisional_receiver_type`. R01 identified a limit to exhaustive coverage, not a specific untested semantic distinction beyond these cases. No additional mini type-checker or duplicated fixture matrix is warranted. |
| Range termination, work and probe isolation | Existing range tests cover zero-work ordinary syntax, local bindings, nested fixed points, generated bodies, repeated source spans and probe-diagnostic suppression. The repaired publication fixture adds failed materialization versus a valid standard-library control. Accepted. The finite-work contract still relies on the existing specialization recursion controls; no new performance or universal termination proof is claimed. |
| Explicit/implicit dependency provenance and cross-feature ordering | New [operator language-item test](../../crates/skald-compiler/src/resolve/tests/operator_language_item.rs) `malformed_protocol_diagnostics_preserve_requirement_order_across_modules` combines malformed iteration and operator protocols. It checks protocol diagnostic order, the explicit iteration import taking precedence over implicit `for` evidence, and identical diagnostic evidence across module creation order. Malformed iteration diagnostics intentionally label only the first requirement origin, not every origin. Existing graph tests cover retention of all edge evidence. Accepted. |
| Disabled/replacement standard library and string/error cycle | Existing [driver pipeline test](../../crates/skald-compiler/src/driver/tests/pipeline.rs) `canonical_standard_library_cycle_obeys_default_replacement_and_disabled_selection` tests the actual driver selection options, supplementing graph/provider fixtures. This closes R01's distinction between absent-provider fixtures and disabled-library configuration. |
| Automated neutral-service dependency guarding | The existing source scanner's limitation remains the explicit, separately scoped P2 discovery. Manual service inspection plus semantic tests support current ownership; extending the guard is future prevention work, not a missing behavior fix required by R03. |

### Acceptance boundaries

No demonstrated correctness blocker remains from this retrospective. The
publication ownership implementation and neutral-service guard remain in their
existing indexed follow-up records. A25's binding index and measurement-driven
optimizations remain with their audit owners; they are not test-hardening tasks.
R04 still owns final audit statuses, readiness and archival. Completing R03 does
not declare all original architectural ambitions implemented.

Focused checks passed: six publication tests, the new protocol provenance test,
the recursive parity test, and the driver frontend-stop test. The initial
provenance assertion was corrected to the inspected validator contract (one
selected requirement label); it did not expose or conceal a production defect.
Final validation (2026-09-12): `make check` passed with **3,107 compiler unit
tests**, **53 process-determinism tests**, documentation/runtime checks and
**629 golden cases**. The additional explicit retained-signature reference
assertion was subsequently checked by its focused test; final `make static-check`
and `make msrv-check` passed on that source state. Documentation and diff checks
pass. No ignored/failing regression remains, and no production changes were
introduced in this final coverage pass.

**R03 is complete.** All detail checkboxes and the progress checkbox are now
checked. R04 is next; acceptance of the remaining architectural follow-ups is
not implied by these passing tests.


## R04 — Final acceptance and readiness

Final decisions, 2026-09-12: A08 and A12 are fulfilled for their reviewed
ownership contracts. A10 and A11 are accepted as deliberately bounded outcomes:
range resumption scheduling lacks justification, and indexed binding facts
remain with A25. A09 is partially complete; the owned publication selection
obligation remains outstanding in its implementation roadmap. The audit's
summary, inventory and detailed entries now reflect these distinctions.
Earlier proposed dispositions and failed-run records are historical evidence,
not competing current statuses. No known correctness failure from this
retrospective remains unresolved.

The review of remaining hotspots is by responsibility: the long resolver
orchestrator is still ordered mutable work, and publication still enumerates
rejection products manually. The P1 ownership roadmap explicitly addresses that
extension hazard. Query scope scans belong to A25; the neutral-service source
guard remains a P2 follow-up. This closure introduces no unrelated cleanup or
new abstraction and does not equate shorter files with better ownership.

| Next work | Accepted prerequisite and disposition | Next step |
| --- | --- | --- |
| Publication ownership | Candidate/error behavior is characterized, including absent retained references and driver error gating. Original ownership goal remains outstanding. | P01 in the publication ownership roadmap; preserve R03 fixtures and characterize any further representation changes before migration. |
| A25 indexed binding lookup | Provisional query ownership and diagnostic distinctions accepted. Independent of publication representation if IDs and selection behavior stay fixed. | Design callable-local identity facts, then a bounded implementation with shadowing, substitutions and invalid-ID tests. |
| Other resolver consumers, including A14 | Current body environment and partial error contract accepted. Adding candidate-dependent tables or altering rejection is blocked on publication ownership; local consumers preserving those contracts may proceed. | State the boundary in each design; route publication changes through the existing roadmap. |
| A17 capability reconstruction | Neutral eligibility/HIR-plan split and parity accepted. No measured performance gain inferred. Neutral-service guard extension is needed before expanding its dependencies. | Capture a comparable baseline and design immutable fact reuse; include the guard prerequisite when changing dependencies. |
| A18 structural graph queries | Independent MIR contract work, not dependent on frontend publication redesign. | Design the graph/query ownership and preserve deterministic traversal and verifier independence. |
| A19 analysis reuse | Depends on A18 and a stable immutable MIR snapshot/invalidation contract. | Establish snapshot lifetime, cache validity and measurements before implementation. |
| A15 optional/place representation | Inventory/design still required; retrospective acceptance does not select a replacement representation. | Review consumers and lifecycle/evaluation invariants, compare alternatives, then stage migration. |
| A22 target virtual registers; A23 effect/alias work | Separate larger projects; no authorization or design is implied by cleanup completion. | Select a measured objective and define target/lifetime or alias-semantics contracts before roadmapping implementation. |
| Narrow fixes and independent tool/MIR work | May proceed without publication redesign when existing contracts are preserved. | Record the bounded purpose, invariants, focused tests and repository gates. |

Future workflow: use a bounded task for a narrow demonstrated fix. Use a short
written design for cross-phase, representation, or substantial algorithm work:
current behavior, desired endpoint, non-goals, alternatives, invariant changes,
and observable acceptance. Resolve those decisions before scheduling dependent
PR-sized tasks. Every task names an owner, tests, and exit criteria. Extra
findings go to an indexed discoveries record; they do not silently enlarge the
active task. No retrospective documents need to be manufactured for each early
small fix.

### Closure validation

A separate local shared clone at
`/tmp/skald-retrospective-closure-xpnao43e/checkout` was checked out at
`b572baf5bdb8fa7b7ca021465ca747b92844db12`. It began clean, with neither `target/`
nor `build/` present; no build artifacts were copied from the working repository.
Commands are `make check` followed by `make msrv-check` inside that checkout.
The final archival/link edits are documentation-only and receive separate
working-tree documentation and diff checks after the move. This procedure
validates the delivered Rust changes from a clean snapshot without discarding
workspace artifacts or modifying the user's checkout state.


Closure results: clean-snapshot `make check` passed with **3,107 compiler unit
tests**, **53 process-determinism tests**, documentation/runtime checks and
**629 golden cases**. Clean-snapshot `make msrv-check` passed on Rust 1.82.
Both commands exited zero. The snapshot revision is the delivered code baseline;
only final documentation reconciliation and archival were added afterward.
Working-tree `make docs-check` and `git diff --check` validate those final edits.

R04 is complete. The retrospective roadmap and this review are archived;
publication ownership and the neutral-service guard remain indexed active
follow-ups. Their remaining work is explicit and does not invalidate this
bounded retrospective's closure.

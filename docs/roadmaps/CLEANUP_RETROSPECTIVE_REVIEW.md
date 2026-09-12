# Cleanup Retrospective Review

Status: in progress; R01 evidence and proposed dispositions recorded. R02 owns
publication acceptance, R03 owns verification gaps, and R04 owns final status
reconciliation. These proposals do not change the audit's delivery statuses.

Reviewed: 2026-09-12, revision `64b6da73b41ed2ec6afe0e1401b3735484847e6d`.
The working tree was clean before this documentation task. Source inspection
therefore describes that revision, not uncommitted compiler changes.

This is the evidence record for the
[retrospective roadmap](CLEANUP_RETROSPECTIVE_ROADMAP.md). Original promises
remain in the [cleanup audit](CODEBASE_CLEANUP_AUDIT.md); current behavior is
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
architectural obligation is not yet established. All are proposals until R04.

| Finding and original outcome | Current owner/product and evidence below | Proposed disposition | Limitation or residual risk |
| --- | --- | --- | --- |
| A08: remove resolver dependence on type checking | Closed capability queries over resolved IR; capability and publication tests | Fulfilled | Source guard does not cover the neutral service as an owner. |
| A08: neutral lifecycle facts; preserve HIR plans and diagnostic paths | Resolved lifecycle availability plus type-check-owned concrete plans; parity tests | Fulfilled | Parallel availability/plan algorithms can drift; fixture parity is not a proof over every recursive graph. |
| A08: ordinary/generic eligibility and atomic rejection | Shared categories, closed requirement query, publication tests | Fulfilled for bounded migration | Publication completeness remains R02's separate obligation; no exhaustive ordinary/generic cross-product test was established. |
| A09: named collection and body products with stable identities | `CollectedDeclarations`, `ResolvedBodies`, `BodyResolutionStage`; determinism tests | Fulfilled | Orchestration still relies on local ordering and mutable interning. |
| A09: separate candidate publication so rollback need not remember every table | Consuming `CandidateProgram` and saved `OrdinaryProgramProducts` | Outstanding | Rejection still manually enumerates restored and cleared fields. R02 must decide whether to accept a narrowed contract or require stronger ownership. |
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
scans scope values, and [A25](CODEBASE_CLEANUP_AUDIT.md#a25--use-identity-indexed-lookup-for-resolved-bindings)
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
[discoveries](CLEANUP_RETROSPECTIVE_DISCOVERIES.md).

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

# Operator-Overloading Conformance Matrix

Status: authoritative traceability map for the implemented interface-based
operator-overloading language and compiler contracts. The contracts define
behavior; this document identifies the executable evidence that protects it.

Test names below are stable semantic descriptions rather than delivery-task
identifiers. Owner-local tests prove narrow phase contracts, while the
[operator golden suite](../../tests/golden/operators/README.md) proves complete
source-to-native composition. The canonical declaration audit in
`canonical_operator_bundle_retains_exact_protocol_identities` mechanically
compares the declaration block in the
[language contract](../language/OPERATOR_OVERLOADING.md#canonical-stdops-protocols)
with `std/std/ops.ska`; exhaustive selection and primitive-registry tests then
carry the same surface through compiler realization without a second hand-
maintained primitive table.

## Canonical surface, selection, and primitives

| Contract rule | Primary executable evidence |
|---|---|
| All seventeen canonical interfaces have exact names, parameter order, requirement signatures, visibility, dependency freedom, and fixed identities; documentation and installed source agree byte-for-byte on the declarations. | `canonical_operator_bundle_retains_exact_protocol_identities` and `canonical_operator_module_is_dependency_free_and_valid_as_an_entry` in [operator language-item tests](../../crates/skald-compiler/src/resolve/tests/operator_language_item.rs). |
| Every overloadable punctuation maps to exactly one canonical protocol; prefix `!`, `&&`, and `||` have no protocol mapping, and `!=` maps to `OpEq`. | `every_overloadable_punctuation_maps_exhaustively_to_its_canonical_protocol`, `predicates_select_direct_protocols_and_not_equal_negates_one_equality_call`, and `logical_syntax_never_consults_operator_protocols` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs). |
| The compiler registry contains exactly the sixty supported primitive cells, each satisfies its exact canonical bound, and each lowers identically to its direct primitive operation. | `every_primitive_operator_cell_satisfies_its_exact_canonical_bound` and `primitive_registry_matches_the_complete_direct_operation_matrix` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs). |
| Unsupported primitive cells, wrong `Rhs` or `Output`, foreign same-named protocols, and unrelated bounds do not gain evidence. Primitive evidence creates no member, interface view, cast, type-test relation, witness, or object metadata. | `primitive_evidence_rejects_unsupported_wrong_and_noncanonical_bounds` and `primitive_evidence_does_not_create_members_views_or_type_test_relations` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs). |
| Exact primitive syntax keeps priority and its existing HIR even when `std::ops` is reachable. Primitive equality and all four ordering predicates retain the direct primitive and IEEE-754 behavior. | `primitive_precedence_remains_the_existing_hir_with_reachable_protocols`, `primitive_predicates_retain_exact_comparison_hir_with_reachable_protocols`, and the [primitive operator native profile](../../tests/golden/operators/primitive_operator_profile.ska). |

## Class selection, aliases, outputs, and dispatch

| Contract rule | Primary executable evidence |
|---|---|
| Direct, inherited, specialized-generic, and exact canonical interface receivers select nominal applications and erase to ordinary interface calls. Same-named methods alone do not authorize punctuation. | `every_value_protocol_selects_once_and_erases_to_an_interface_call`, `inherited_closed_generic_and_exact_interface_receivers_are_eligible`, and `same_named_methods_and_missing_rhs_applications_do_not_authorize_punctuation` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs). |
| Locals, fields, statics, `self`, aliases, produced values, checked views, explicit shared dereference, explicit optional unwrap, array elements, and exact interface views reuse ordinary receiver carriers. Raw shared/optional values and unrelated `Obj`, interface, array, and function values do not cross implicitly. | `operator_receivers_reuse_the_complete_ordinary_carrier_matrix` and `ineligible_left_types_do_not_gain_implicit_operator_crossings` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs), plus the [call-equivalence native program](../../tests/golden/operators/call_equivalence_operator_overloading.ska). |
| Binary `Rhs` uses ordinary read-only alias compatibility, including base/interface views and produced primitive scalar storage. Exact-match or expected-output ranking never selects a candidate; explicit casts may change the static RHS first. `mut ref` remains place-only. | `predicate_rhs_uses_ordinary_base_and_interface_view_compatibility`, `expected_results_do_not_select_and_explicit_rhs_casts_do`, and `generic_operator_rhs_incompatibility_has_ordered_bound_evidence` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs), together with produced-alias owner tests documented in [Aliases and Ownership](../language/ALIASES_AND_OWNERSHIP.md#implemented-produced-primitive-read-only-alias-arguments). |
| Primitive, exact-class, shared, optional, array, function, and specialized-generic outputs reuse ordinary capabilities, assignment/argument/return paths, ownership, and cleanup. | `operator_outputs_reuse_every_ordinary_result_capability` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs) and the [result-capability native program](../../tests/golden/operators/result_operator_overloading.ska). |
| Eager operands evaluate once left-to-right; calls, failures, result securing, aliases, anchors, and temporaries retain ordinary reverse full-expression cleanup and exact interface dispatch. | [Call-equivalence](../../tests/golden/operators/call_equivalence_operator_overloading.ska), [value-operator](../../tests/golden/operators/value_operator_overloading.ska), and [failure-trace](../../tests/golden/operators/operator_overloading_failures.ska) native programs. |

## Generics, modules, equality, and failure

| Contract rule | Primary executable evidence |
|---|---|
| Definition-site generic selection freezes one exact bound application. Specialization realizes it as either an ordinary class witness call or an existing primitive intrinsic, including manual bound calls, with no concrete-type reselection. | `generic_operator_bound_closes_to_primitive_intrinsic`, `generic_operator_bound_closes_to_class_witness_without_reselection`, and `generic_operator_selection_supports_structural_rhs_and_output` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs), plus the [generic native program](../../tests/golden/operators/generic_operator_overloading.ska). |
| Unary, algebraic, equality, and ordering bounds compose; missing, ambiguous, foreign, incompatible-RHS, and unsupported primitive applications fail at their owning boundary. | `generic_operator_bounds_cover_unary_algebraic_equality_and_ordering`, `generic_operator_ambiguity_and_missing_bound_fail_at_definition_site`, `generic_operator_rhs_incompatibility_has_ordered_bound_evidence`, and `unsupported_primitive_operator_bound_fails_without_a_fake_witness` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs). |
| `OpEq<Rhs>` remains typed and independent of dynamic `Equatable.equals(ref Obj)`; overloaded `!=` performs one equality call and boolean negation. The four ordering protocols remain direct, including heterogeneous RHS orientation. | `dynamic_equatable_and_typed_operator_equality_remain_independent`, `predicates_select_direct_protocols_and_not_equal_negates_one_equality_call`, and `predicate_rhs_uses_ordinary_base_and_interface_view_compatibility` in [operator type-check tests](../../crates/skald-compiler/src/typeck/tests/operator_overloading.rs), plus the [predicate native program](../../tests/golden/operators/predicate_operator_overloading.ska). |
| `std::ops` is reachable only through explicit protocol references or direct entry selection. A complete replacement bundle works through ordinary provider rules; punctuation does not add reachability; primitive-only source works with `--no-stdlib`. | `valid_replacement_bundle_and_explicit_protocol_use_follow_ordinary_interfaces`, `qualified_protocol_use_authorizes_value_producing_class_punctuation`, and `punctuation_does_not_make_the_operator_module_reachable` in [operator resolution/type-check tests](../../crates/skald-compiler/src/resolve/tests/operator_language_item.rs), plus `assembly_output_runs_the_real_pipeline_through_the_binary` in [CLI integration tests](../../crates/skac/tests/cli.rs). |
| Provider errors precede bundle validation, which precedes generic and closed selection, capability, and ordinary lifecycle diagnostics. Malformed bundles and candidate origins remain canonically ordered. | `simultaneous_bundle_defects_are_reported_in_canonical_order`, `canonical_bundle_failures_precede_generic_operator_selection_failures`, `multiple_resolved_applications_are_diagnosed_before_hir_completion`, and `malformed_resolved_operator_mapping_is_diagnosed_before_interface_erasure` in [operator owner tests](../../crates/skald-compiler/src/resolve/tests/operator_language_item.rs). |

## Lowering, artifacts, and determinism

| Contract rule | Primary executable evidence |
|---|---|
| Completed HIR contains only ordinary exact interface calls or existing primitive operations; preliminary and final MIR add no operator node and reject corrupt calls, operations, aliases, cleanup, or realization evidence through existing verifiers. | [operator realization MIR tests](../../crates/skald-compiler/src/mir/tests/operator_realization.rs). |
| Static effects, reachable bodies, interface dispatch, panic traces, and native code match explicit-call or primitive-intrinsic behavior. Public symbols and runtime ABI references remain equal to the pre-feature baseline. | `generic_operator_phase_products_are_deterministic_across_processes` in [pipeline determinism](../../crates/skald-compiler/tests/pipeline_determinism.rs) and the operator native programs linked above. |
| Resolved identities, candidates, diagnostics, generated bodies, HIR/MIR, target retention, assembly, and native observations are stable across module/provider order and independent compiler processes. | `operator_language_item_identities_ignore_source_creation_order`, `operator_selection_is_independent_of_module_source_creation_order`, the generic operator pipeline-determinism case, and full golden determinism. |
| Malformed chains, nested generic applications, aliases, comments, delimiters, and recovery remain bounded and non-panicking. | Operator mutation families in [frontend robustness](../../crates/skald-compiler/tests/generative_robustness.rs). |

## Confirmed exclusions

| Excluded feature | Evidence that it remains absent or unaffected |
|---|---|
| Prefix `!`, `&&`, `||`, truthiness, or short-circuit protocols | `logical_syntax_never_consults_operator_protocols`; ordinary boolean and short-circuit goldens retain their exact behavior. |
| `Equatable` as punctuation, or an implicit bridge between dynamic and typed equality | `dynamic_equatable_and_typed_operator_equality_remain_independent`. |
| Ordinary method overloading, structural same-name lookup, associated/default methods, or implicit conversion/ranking | Same-name, ambiguity, and expected-result tests in the operator type-check owner; ordinary interface conformance tests retain exact method uniqueness. |
| Unsupported primitive cells, source-defined primitive implementations, primitive interface objects, or direct primitive protocol members | Primitive evidence rejection and no-view/member tests listed above. |
| Implicit shared dereference, optional unwrap, owner creation, or unrelated view crossing | Receiver eligibility and complete carrier-matrix tests listed above. |
| Compound assignment, increment/decrement, ranges, `Range<T>`, or iteration changes | The implemented grammar contains no such operator protocol extension; [general iteration](../language/ITERATION.md) remains an independent consumer of `std::iter`. |

## Closure gates

The complete profile is accepted only when `make check`, `make check-long`,
the two representative operator native cases, documentation validation, and
`git diff --check` pass. The pipeline determinism test additionally pins the
pre-feature public-symbol and runtime-ABI surface.

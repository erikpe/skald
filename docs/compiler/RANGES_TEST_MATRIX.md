# Generic-Range Conformance Matrix

Status: authoritative traceability map for the implemented generic-range
language and compiler contracts. The contracts define behavior; this document
identifies the executable evidence that protects it.

Owner-local tests prove the narrow phase contract. The
[range golden suite](../../tests/golden/ranges/README.md) proves complete
source-to-diagnostic and source-to-native composition.

## Source forms and semantics

| Contract rule | Primary executable evidence |
|---|---|
| Direct `for-in` range sources accept exact `u8`, `u64`, and `i64` endpoints and opted-in exact classes. | `concise_ranges` in the [range goldens](../../tests/golden/ranges/concise_ranges.ska), `concise_integer_range_activates_canonical_module_and_retains_resolved_evidence`, and `concise_class_range_selects_nominal_ordering_and_successor_witnesses` in [range resolution tests](../../crates/skald-compiler/src/resolve/tests/range_language_item.rs). |
| Explicit `Range<T>` construction remains an ordinary iterable expression in direct, stored, argument, and result positions for primitive and class endpoints. | [Explicit primitive](../../tests/golden/ranges/explicit_primitives.ska) and [explicit class](../../tests/golden/ranges/explicit_classes.ska) goldens, plus `explicit_range_values_and_direct_range_sources_share_ordinary_semantics` in [range type-check tests](../../crates/skald-compiler/src/typeck/tests/explicit_ranges.rs). |
| Parentheses may group either endpoint or an ordinary iterable, but parentheses around the complete concise range make it invalid. | `grouped_endpoints_and_parenthesized_iterables_are_valid_but_grouped_ranges_are_not` in [range syntax tests](../../crates/skald-compiler/src/syntax/tests/ranges.rs), the grouped endpoint cases in `concise_ranges`, and the grouped-range case in [direct-source-only diagnostics](../../tests/golden/ranges/direct_source_only.golden.toml). |
| `..` is rejected in stored-value, argument, result, and grouped-complete-range contexts with one `PAR017` diagnostic that recommends explicit `Range` construction without guessing a type argument. | `range_syntax_is_rejected_in_every_general_expression_context` in [range syntax tests](../../crates/skald-compiler/src/syntax/tests/ranges.rs) and all four exact diagnostics in [direct-source-only diagnostics](../../tests/golden/ranges/direct_source_only.golden.toml). |
| Missing endpoints and chained operators recover once at the `for-in` header boundary; later statements and declarations remain parseable. | `malformed_direct_ranges_report_once_and_recover_at_for_header_boundaries` and `rejected_value_ranges_recover_into_later_statements_and_declarations` in [range syntax tests](../../crates/skald-compiler/src/syntax/tests/ranges.rs). |
| Endpoints evaluate once from lower to upper; class comparison and successor effects remain ordered, with successor occurring before body entry. | `concise_ranges`, [explicit class execution](../../tests/golden/ranges/explicit_classes.ska), and `fused_increment_precedes_the_first_body_operation` in [range MIR tests](../../crates/skald-compiler/src/mir/tests/range_iteration.rs). |
| Equal and descending bounds are empty; representable maximum bounds remain half-open without wrapping during traversal. | `concise_ranges`, [explicit primitive execution](../../tests/golden/ranges/explicit_primitives.ska), and [successor execution](../../tests/golden/ranges/successor.ska). |
| Normal completion, nesting, `continue`, `break`, and `return` preserve item, state, receiver, and body-local lifecycle. | `concise_ranges`, `explicit_classes`, `verifier_rejects_fused_storage_operation_and_lifetime_mutations` in [range MIR tests](../../crates/skald-compiler/src/mir/tests/range_iteration.rs), and ordinary iteration lifecycle coverage. |
| Body runtime failure retains ordinary source attribution and adds no range runtime service. | [Concise range failure](../../tests/golden/ranges/concise_range_failure.golden.toml), range MIR absence assertions, and backend runtime-symbol assertions. |

## Pipeline, determinism, and target behavior

| Contract rule | Primary executable evidence |
|---|---|
| Canonical declarations, static primitive successor evidence, exact endpoint typing, class witnesses, and structural loop-source identities are resolved deterministically. | [Range resolution tests](../../crates/skald-compiler/src/resolve/tests/range_language_item.rs) and `range_phase_products_are_deterministic_across_processes` in [pipeline determinism](../../crates/skald-compiler/tests/pipeline_determinism.rs). |
| Direct primitive sources select structured fusion; direct classes and every explicit/stored/generic/interface boundary retain ordinary protocol execution. | `immediate_integer_ranges_select_the_fused_structured_plan`, `concise_class_ranges_retain_ordinary_witness_and_lifecycle_plans`, and `fusion_excludes_every_non_immediate_or_nonprimitive_iteration_boundary` in [range type-check tests](../../crates/skald-compiler/src/typeck/tests/explicit_ranges.rs). |
| Tokens, AST, module graph, resolved program, HIR, preliminary/planned/final MIR, assembly, retained artifacts, and diagnostics remain stable across independent processes and source/provider permutations. | Both range cases in [pipeline determinism](../../crates/skald-compiler/tests/pipeline_determinism.rs), full golden determinism, and backend artifact-retention tests. |
| Mutation and deep-source recovery stay bounded around the direct `for-in` grammar boundary; invalid punctuation stops at syntax. | `exercise_range_syntax_mutations` and `bounded_deep_direct_range_sources_respect_the_for_header_boundary` in [frontend robustness](../../crates/skald-compiler/tests/generative_robustness.rs). |
| Fused `u8`, `u64`, and `i64` loops contain only scalar range state, one comparison, and one increment. | `fused_integer_matrix_contains_only_scalar_loop_machinery` in [range MIR tests](../../crates/skald-compiler/src/mir/tests/range_iteration.rs). |
| Each fused integer loop matches its handwritten `while` instruction profile, has no call or range runtime symbol, and prunes fused-only canonical artifacts. Explicit and direct-class paths retain their ordinary artifacts. | `immediate_integer_ranges_match_handwritten_while_instruction_shapes`, `ordinary_range_execution_retains_canonical_artifacts`, and `direct_class_range_execution_retains_canonical_artifacts` in [x86-64 iteration tests](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/iteration.rs). |
| Ranges add no lower-MIR operation, target ABI rule, public runtime symbol, or runtime ABI revision. | Existing MIR vocabulary and verifier tests, the x86-64 range tests' ABI-version assertion, and the independent [runtime contract](../../tests/runtime/test_runtime_contract.c). |

## Exclusions and supporting evidence

| Excluded behavior | Evidence |
|---|---|
| Reusable concise range values, range chaining, inclusive/stepped syntax, inferred range result types, or an overloadable range operator | [Direct-source-only diagnostics](../../tests/golden/ranges/direct_source_only.golden.toml), malformed syntax tests, and the implemented [grammar](../language/GRAMMAR.md). |
| Mixed endpoint types, unsupported primitive successors, structural class opt-in, or noncanonical lookalikes | [Range failure goldens](../../tests/golden/ranges/failures.golden.toml), range resolution diagnostics, and exact canonical-evidence mutations. |
| Fusion of explicit, stored, class, generic-dependent, inherited, interface-view, or lookalike sources | `fusion_excludes_every_non_immediate_or_nonprimitive_iteration_boundary` and backend artifact-retention tests. |
| Wall-clock timing as a correctness gate | The [range-loop performance procedure](../development/RANGE_LOOP_PERFORMANCE.md) records supporting measurements; deterministic MIR and assembly shape remain the gates. |

## Closure gates

The complete profile is accepted only when `make check`,
`make golden-determinism-test`, `make msrv-check`, the bounded robustness
suite, `make docs-check`, and `git diff --check` pass. The range benchmark is
rerun only when deterministic loop shape changes or existing timing evidence
falls outside its documented acceptance band.

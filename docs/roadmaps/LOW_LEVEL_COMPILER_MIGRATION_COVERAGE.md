# Low-Level Compiler Migration Coverage

Status: phase architecture preparation complete, 2026-09-17. Cumulative review
covers `495debd3..472bce66` and the closing metric guard/documentation changes.
Current-boundary witnesses and the durable baseline are qualified; all
new-pipeline delivery is **pending**. The archived preparation roadmap records
validation and artifact dispositions.
Program implementation baseline: `495debd3`.

This is the continuing migration/handoff record for the
[architecture program](LOW_LEVEL_COMPILER_ARCHITECTURE_DESIGN_PROPOSAL.md).
The [frozen phase design](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_DESIGN_PROPOSAL.md) owns
architectural decisions; the [preparation roadmap](../archive/LOW_LEVEL_PHASE_ARCHITECTURE_ROADMAP.md)
owns current-boundary tests and baseline evidence. Update this inventory when
schemas or helpers change, retain it through foundation adoption, and archive
only after all migration obligations have a disposition.

## Review scope and inventory rules

The reviewed public input remains sealed final MIR. Reconciliation covered:

- [Instruction definitions](../../crates/skald-compiler/src/mir/model/instruction.rs):
  42 `MirInstruction` variants, including nested array and I/O operations.
- [Control flow](../../crates/skald-compiler/src/mir/model/control_flow.rs):
  20 `MirTerminator` variants and all 12 `MirTerminationReason` variants.
- [Array operations](../../crates/skald-compiler/src/mir/model/array.rs):
  29 `MirArrayInstruction` variants.
- [Value/place definitions](../../crates/skald-compiler/src/mir/model/value.rs):
  19 `MirRvalueKind` variants, nine place bases and six projections.
  `PathCondition` is proof-rich-only and must remain unavailable at this boundary.
- [I/O](../../crates/skald-compiler/src/mir/model/io.rs): five `MirIoOperation` variants.
- Exhaustive instruction dispatch in
  [lowering](../../crates/skald-compiler/src/backend/x86_64_sysv/lower.rs),
  assignment dispatch and every `select_*_terminator` owner, helper constructors,
  [symbol families](../../crates/skald-compiler/src/backend/x86_64_sysv/symbol.rs),
  planning/trace/data/retention owners, and named tests linked below.
- The [current backend contract](../compiler/BACKEND.md), runtime/lifecycle
  documents linked by the frozen design, [reporting](../compiler/REPORTING.md),
  [test ownership](../development/TESTING.md), and measurement implementation.
  The sibling Niflheim backend transition/specification informed review of
  explicit CFG and migration parity; its GC and non-SSA choices are not Skald contracts.

To repeat review, search `pub enum MirInstruction`, `pub enum MirTerminator`,
`pub enum MirArrayInstruction`, `pub enum MirRvalueKind`, and `pub enum
MirIoOperation` in the model; reconcile their members with the tables and with
`MirInstruction::`, `MirRvalueKind::`, `MirArrayInstruction::`, and
`MirTerminator::` matches throughout the target. Search `AssemblyFunction`,
`lower_helpers`, `lower_all`, `lower_finalizers`, and symbol constructors for
work not represented by a MIR body. Follow typed instruction/data references
through `artifacts::dependency_graph`, not emitted assembly strings.

Variant names in tables are exact within their stated enum. Each variant is
listed once in its enum's inventory. Test references use the evidence registry;
they establish the named behavior, not exhaustive new-pipeline coverage. A dash
in the gap column means no additional *current-behavior* witness was identified
for that family; downstream verifier/native parity work remains mandatory.

Delivery notation: `LA03` is the complete scalar/CFG/call pilot, `LA04` is full
language/helper migration, and `LA05` is consolidation/default adoption. Every
row remains pending even where current compiler tests already exist. The
future shared owner expands semantic work into explicit operations; target
selection realizes those operations using checked layout/ABI facts.

## Current evidence registry

Backend test names below are relative to `backend::x86_64_sysv::tests`.
Runtime-trace names are relative to `backend::x86_64_sysv::runtime_trace`.
Retain the semantic expectations while adapting owners during migration.

| Key | Current test owner and exact witness identifiers |
| --- | --- |
| E01 | [Instruction selection](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/instruction_selection.rs): `storage_lifetime_markers_emit_no_machine_instructions`, `selects_every_supported_arithmetic_operation_and_storage_copy`, `selects_every_integer_comparison_with_exact_signedness_and_canonical_results`, `selects_every_integer_bitwise_operation_and_canonicalizes_u8_results` |
| E02 | [Calls](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/calls.rs): `canonicalizes_u8_arithmetic_parameters_calls_and_returns`, `mixed_scalar_layout_independently_exhausts_register_classes`, `lowers_register_and_stack_arguments_at_the_abi_boundary`, `unit_calls_and_returns_do_not_move_a_fictitious_result`, `external_calls_use_the_declared_symbol_without_emitting_a_body` |
| E03 | [Division](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/integer_division.rs): `native_signed_division_and_remainder_follow_floor_semantics_without_traps`, `native_signed_property_samples_match_the_floor_model`, `zero_divisors_report_the_exact_operation_specific_panic`; [shifts](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/shifts.rs) and [primitive casts](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/primitive_casts.rs): `checked_float_to_integer_successes_cover_every_target_boundary`, `checked_float_to_integer_failures_report_the_exact_frozen_message` |
| E04 | [Control flow](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/control_flow.rs): `lowers_a_diamond_with_branch_local_calls_and_a_storage_join`, `jumps_to_a_non_first_entry_before_emitting_blocks_in_id_order`, `normalized_path_activation_is_a_representation_only_backend_refinement` |
| E05 | [Objects](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/objects.rs): `lays_out_and_addresses_deep_source_subobjects_from_every_storage_base`, `lowers_exhausted_receiver_alias_and_sse_arguments_through_ordered_stack_slots`; [object results](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/object_results.rs): `return_destination_precedes_receiver_and_explicit_arguments`; [produced receivers](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/produced_receivers.rs): `every_call_result_producer_survives_recursion_and_register_stack_pressure` |
| E06 | [Copy](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/copy.rs): `lowers_user_and_synthesized_copy_in_mir_defined_field_order`, `preserves_live_aliases_and_scalar_homes_across_copy_calls`; [destruction](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/destruction.rs): `cleanup_preserves_the_precomputed_return_value_without_aggregate_runtime_operations`, `floating_return_values_reload_into_xmm0_after_cleanup_calls` |
| E07 | [Shared ownership](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/shared_ownership.rs): `generated_dynamic_finalizer_executes_derived_then_base_and_frees_once`, `checked_retain_reports_exhaustion_but_invalid_state_is_a_silent_hard_trap`, `synthesized_shared_field_copy_and_self_assignment_lower_to_balanced_owners`, `shared_call_anchor_survives_later_argument_replacement_until_call_completion` |
| E08 | [Optional values](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/optional_values.rs): `nested_optional_class_and_shared_payload_lifecycles_execute_recursively`, `optional_container_alias_cannot_clear_a_checked_payload`, `nested_optionals_cross_dispatch_recursion_initializers_and_abi_pressure`; [boxes](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/shared_optional_boxes.rs): `owner_replacement_keeps_the_old_box_alive_and_frees_exact_bases_once`, `optional_box_array_inner_allocation_failure_does_not_publish_the_next_slot` |
| E09 | [Arrays](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/arrays.rs): `primitive_inline_array_helpers_are_deterministic_and_layout_specialized`, `nontrivial_and_nested_inline_array_lifecycle_reaches_native_lowering`, `detached_element_alias_keeps_old_backing_while_descriptor_alias_observes_replacement`, `copied_slices_and_checked_slice_assignment_execute_with_snapshot_semantics`, `invalid_slice_bounds_and_length_mismatch_terminate_before_writes` |
| E10 | [Element lists](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/array_element_lists.rs): `primitive_element_list_expressions_run_once_in_left_to_right_order`, `exact_class_element_lists_preserve_lifecycle_order_and_reverse_destruction`; [indexed construction](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/indexed_array_construction.rs): `zero_length_skips_the_element_expression`, `shared_owner_indexed_arrays_release_every_allocation_after_normal_cleanup` |
| E11 | [Strings](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/strings.rs): `emits_pooled_aligned_immutable_literal_backings_with_exact_bytes`, `repeated_literals_copy_assign_pass_and_return_with_immortal_backing`, `panic_extracts_the_exact_descriptor_slice_and_uses_the_common_reporter`; [I/O](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/io.rs): `verified_io_selects_each_exact_runtime_symbol_and_assembles`, `io_calls_preserve_alignment_result_homes_and_backing_anchor_lifetimes` |
| E12 | [Type operations](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/type_operations.rs): `shared_cast_terminator_owns_runtime_trace_attribution_in_an_empty_block`, `checked_cast_places_execute_through_owning_copy_operations`; [virtual dispatch](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/virtual_dispatch.rs): `permits_an_absent_body_in_an_unused_virtual_table_slot`; [interface dispatch](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/interface_dispatch.rs): `interface_object_results_reuse_the_hidden_destination_path` |
| E13 | [Function values](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/function_values.rs): `preserves_mixed_register_classes_stack_pressure_and_function_results`, `reuses_alias_aggregate_optional_shared_and_function_result_conventions`, `emits_address_taken_bodies_without_a_direct_or_indirect_call_edge` |
| E14 | [Static initialization](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/static_initialization.rs): `coordinator_uses_dependency_order_and_wrapper_runs_it_before_entry`; [shutdown](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/static_shutdown.rs): `lowers_exact_reverse_shutdown_and_preserves_the_entry_result`, `entry_panic_does_not_attempt_static_unwinding`; [planning](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/static_planning.rs): `backend_storage_planning_distinguishes_active_and_dead_body_fallback_slots` |
| E15 | [Retained domain](../../crates/skald-compiler/src/backend/x86_64_sysv/tests/retained_domain.rs): `sparse_backend_planning_visits_only_physically_retained_definitions`, `sparse_complete_and_artifact_retained_emission_never_resurrect_absent_bodies`, `equal_retained_closures_emit_identical_runtime_trace_metadata`; [artifact closure](../../crates/skald-compiler/src/backend/x86_64_sysv/artifacts.rs): `prunes_unreachable_artifacts_from_every_section`, `retains_transitive_function_and_data_dependencies` |
| E16 | [Trace frames/metadata](../../crates/skald-compiler/src/backend/x86_64_sysv/runtime_trace/tests.rs): `runtime_trace_metadata_omission_never_requests_or_emits_trace_data`, `runtime_trace_frame_mixed_returns_preserve_results_and_restore_null`; [locations](../../crates/skald-compiler/src/backend/x86_64_sysv/runtime_trace/location_tests.rs): `runtime_trace_location_precedes_every_explicit_call_target_after_marshalling`, `runtime_trace_location_is_failure_only_and_immediately_precedes_reporters`; [attribution](../../crates/skald-compiler/src/backend/x86_64_sysv/runtime_trace/attribution_tests.rs): `runtime_trace_attribution_native_generated_copy_and_finalizer_chains_omit_helpers` |
| E17 | [Facade](../../crates/skald-compiler/src/backend/mod.rs): raw/proof-rich input, private edit authority and attaching omitted sources compile-fail examples, `backend_input_exposes_sources_only_for_enabled_tracing`; [final seal](../../crates/skald-compiler/src/passes/pipeline/seal.rs): seal forgery, read-only program and private invalidation compile-fail examples; [phase guards](../../crates/skald-compiler/tests/phase_boundaries.rs): `production_compiler_dependencies_follow_owned_boundaries`, `policy_rejects_reverse_edges_and_accepts_lowering_inputs`, `backend_policy_rejects_frontend_state_and_accepts_verified_input_services` |
| E18 | [Reporting observers](../../crates/skald-compiler/src/driver/tests/reporting/observers.rs): `observation_preserves_success_artifacts_and_failure_diagnostics`, `independent_observers_do_not_share_events_across_repeated_or_parallel_calls`; [cross-process pipeline tests](../../crates/skald-compiler/tests/pipeline_determinism.rs); golden determinism/release Make targets |
| E19 | [Live integers](../../tests/golden/operators/live_integer_inputs.ska), `operators/arithmetic::live_integer_inputs`; [aggregate pressure](../../tests/golden/calls/aggregate_pressure.ska), `calls/functions::aggregate_pressure`; exact variants/runs below |
| E20 | [Strong-count native probe](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/ownership/count/tests.rs): `release_frees_original_header_after_finalizer_changes_owner_and_clobbers_callers` |
| E21 | [Generated retain ABI probe](../../crates/skald-compiler/src/backend/x86_64_sysv/lower/ownership/helpers/tests.rs): `generated_retain_helper_aligns_the_stack_before_reporting_exhaustion`; demonstrated the unaligned overflow reporter call before its correction |

Feature-owned native/failure goldens additionally protect
[calls](../../tests/golden/calls/functions.golden.toml),
[shared graphs](../../tests/golden/shared_ownership/graphs.golden.toml),
[optional lifecycle](../../tests/golden/optionals/lifecycle.golden.toml),
[static fields](../../tests/golden/static_fields/fields.golden.toml), and
[panic reporting](../../tests/golden/runtime/panic.golden.toml).
Exact selected leaves and their mode coverage are recorded below.

Inventory references G01–G05 identify the original readiness gaps, now all
closed by E17/E19/E20 and the selected mode table. Retain those references to
connect the audited families to their added protection; they do not mark
pending current-behavior work. New-pipeline delivery remains pending separately.

## MIR instruction inventory

Current owners below are relative to
`crates/skald-compiler/src/backend/x86_64_sysv/`. They are source paths, not
proposed module declarations. `lower.rs` dispatches all 42 variants.

| `MirInstruction` members | Current owner | Future shared lowering obligation / invariant | Evidence | Gap | Delivery (pending) |
| --- | --- | --- | --- | --- | --- |
| `StorageLive`, `StorageDead` | `lower.rs`, `frame.rs` | Preserve lifetime information for addressable objects; current emission is a no-op, not machine liveness or permission to reuse storage | E01 | — | LA03 |
| `Assign` | `lower/assignment.rs` and scalar/type/optional/array selectors | See exhaustive rvalue inventory; widths and canonical forms survive home removal | E01, E03 | G01, G02 | LA03 scalar; LA04 remaining rvalues |
| `Store` | `lower/value.rs` | Explicit place address and width-correct store; final-MIR authorization is not rechecked as source policy | E05 | — | LA03 scalar; LA04 complex places |
| `Call` | `lower/call.rs`, `lower/call/*` | Stabilized target and ordered role-based components; ordinary/direct/static/indirect/method/interface forms; see call inventory | E02, E05, E13 | G03 | LA03 direct/indirect scalar; LA04 remaining shapes |
| `Initialize` | `lower/call.rs` | Call the selected initializer into its final destination; receiver origin preserved | E05 | — | LA04 |
| `CopyConstruct`, `CopyAssign` | `lower/copy.rs` | User/synthesized selection, base/field order, shared retain-before-release and alias-safe self-assignment | E06, E07 | — | LA04 |
| `Cleanup`, `EndFullExpression` | `lower/cleanup.rs`, `lower/copy.rs` | Destruction plan and recorded reverse completion order; preserve already-computed result | E06 | — | LA04 |
| `BindCheckedView`, `EndCheckedView` | `lower/type_operations.rs`, `lower.rs` | Materialize static/complete/metadata components; current end is a verified lifetime no-op | E12 | — | LA04 |
| `SharedAllocate`, `SharedInitialize`, `SharedPublish` | `lower/ownership.rs`, `lower/call.rs` | Checked allocation, unpublished payload, selected constructor/copy and publish after completion | E07, E08 | — | LA04 |
| `SharedStatic` | `lower/strings.rs` | Static/immortal provenance and backing reference; no dynamic retain/free of immortal data | E11 | — | LA04 |
| `SharedAdopt`, `SharedMove` | `lower/ownership.rs` | Transfer owner exactly once; no extra retain; source disposition preserved | E07 | — | LA04 |
| `SharedCopy`, `SharedFieldCopy` | `lower/ownership.rs`, `lower/ownership/count.rs` | Null/zero invalid-state hard trap, immortal no-op, checked count overflow | E07 | — | LA04 |
| `SharedCast` | `lower/ownership.rs`, `lower/type_operations.rs` | Preserve named-retain versus produced-transfer semantics and selected view metadata | E07, E12 | — | LA04 |
| `SharedRelease` | `lower/ownership.rs`, `lower/ownership/count.rs` | Last-owner graph, finalizer call and free original header; calls/clobbers visible before placement | E07, E08 | G04 | LA04 |
| `SharedFieldInitialize`, `SharedFieldReplace` | `lower/ownership.rs` | Initialize or replace edge; retain secured replacement before old-owner release | E07 | — | LA04 |
| `StringInitialize` | `lower/strings.rs`, `literal_data.rs` | Exact byte slice/descriptor with pooled immutable backing and planned initializer shape | E11 | — | LA04 |
| `OptionalInitialize`, `OptionalAssign` | `lower/optional/scalar.rs` | Exact primitive presence/payload representation; assignment preserves absence semantics | E08 | — | LA04 |
| `AggregateOptionalInitialize`, `AggregateOptionalAssign`, `AggregateOptionalPublish`, `AggregateOptionalCleanup` | `lower/optional/aggregate.rs` | Recursive state/payload layout, publish after initialization, conditional cleanup and pin checks | E08 | — | LA04 |
| `ClassOptionalInitialize`, `ClassOptionalAssign`, `ClassOptionalPublish`, `ClassOptionalCleanup` | `lower/optional/inline_class.rs` | Aligned inline payload, copy/destruction order, explicit publication state and pinned-mutation behavior | E08 | — | LA04 |
| `EndOptionalView`, `EndOptionalBoxView` | `lower/optional/access.rs` | Real guard decrement with underflow hard trap; not equivalent to `EndCheckedView` | E08 | — | LA04 |
| `OptionalSharedInitialize`, `OptionalSharedAssign`, `OptionalSharedCleanup` | `lower/optional/shared_owner.rs` | Zero-niche absent owner, conditional retain/release and balanced self-assignment | E08 | G04 for release path | LA04 |
| `Array` | `lower/array.rs` and siblings | All 29 members below; explicit lifecycle, checks, backing anchors and loops | E09, E10 | — | LA04 |
| `Io` | `lower/io.rs` | All five runtime operations below; raw buffer pointers/lengths with anchored owner | E11 | — | LA04 |

## Rvalues, operations and places

| `MirRvalueKind` members | Current owner | Future disposition / invariant | Evidence | Gap | Delivery (pending) |
| --- | --- | --- | --- | --- | --- |
| `ConstantI64`, `ConstantU64`, `ConstantU8`, `ConstantF64Bits`, `ConstantBool` | `lower/assignment.rs` | Explicit scalar width/raw float bits; canonical bytes/booleans | E01, E02 | — | LA03 |
| `CallableAddress` | `lower/assignment.rs`, `symbol.rs` | Typed code reference to exact eligible callable, not string resolution | E13, E15 | — | LA03 |
| `PathCondition` | `mir/verify/contract.rs`; rejected at selector boundary | Proof-rich-only: normalized to executable storage/load before final MIR; never add lowered LIR support for leaked proof state | E04, E17 | Future seal negative test | LA02 |
| `Load` | `lower/assignment.rs`, `lower/value.rs` | Address plus aligned scalar read; alias/heap/static objects remain memory | E05 | — | LA03 scalar; LA04 complex places |
| `Unary`, `Binary` | `lower/assignment.rs` | Exact operation inventory below; preserve wrapping/float meaning and live inputs | E01 | G01 | LA03 |
| `IntegerDivision` | `lower/integer_division.rs` | Quotient/remainder, signed floor rounding and defined minimum pair; selected correction CFG explicit | E03 | G02 | LA03 |
| `Shift` | `lower/shift.rs` | Left/right, arithmetic/logical right by integer kind; checked width, no host masking semantics | E03 | — | LA03 |
| `PrimitiveComparison` | `lower/assignment.rs` | Six shared predicates, integer signedness, floating unordered behavior and canonical bool | E01, E03 | — | LA03 |
| `PrimitiveCast`, `CheckedF64ToInteger` | `lower/primitive_cast.rs` | Exact source/target cells; checked finite post-truncation range; preserve bits/canonical forms | E03 | — | LA03 |
| `TypeTest` | `lower/type_operations.rs` | Runtime membership against planned metadata; exact static outcome already realized in MIR | E12 | — | LA04 |
| `OptionalPresence`, `OptionalBoxPresence` | `lower/optional/scalar.rs`, `lower/optional/access.rs` | Exact layer/kind, planned tag or niche, secured box owner | E08 | — | LA04 |
| `ArrayLength` | `lower/array.rs` | Length from exact planned descriptor/header representation | E09 | — | LA04 |

Scalar operation members are not additional opaque lifecycle operations:

- Unary: `NegateI64`, `NegateF64`, `LogicalNotBool`, `BitwiseComplement`.
- Binary: `AddI64`, `SubtractI64`, `MultiplyI64`, `AddU64`, `SubtractU64`,
  `MultiplyU64`, `AddU8`, `SubtractU8`, `MultiplyU8`, `AddF64`, `SubtractF64`,
  `MultiplyF64`, `DivideF64`, `IntegerBitwise`; bitwise `And`, `Or`, `Xor`.
- Division: `Quotient`, `Remainder`; shift directions `Left`, `Right` and right
  flavors `Arithmetic`, `Logical`. Comparison predicates: `Equal`, `NotEqual`,
  `LessThan`, `LessEqual`, `GreaterThan`, `GreaterEqual` for the verified operand kind.
  LA02 freezes concrete lowered operations; LA03 owns target recipes/constraints.
- Primitive cast kinds: `Identity`, `IntegerBits`, `ToBool`, `ToF64`,
  `FromBool`, `BitReinterpretation`, `CheckedF64ToInteger`. Legal source/target
  cells remain owned by the primitive cast contract, not backend inference.

Place bases: `StaticField`, `StaticLifecycleDestination`, `Storage`,
`AliasParameter`, `CheckedView`, `ArrayAlias`, `SharedPointee`,
`SharedAllocationPayload`, `OptionalBoxPayload`. Projections: `Base`, `Field`,
`OptionalPayload`, `AggregateOptionalPayload`, `CheckedOptionalPayload`,
`ArrayElement`. Current owners are `lower/value.rs`, `lower/object_abi.rs`,
`lower/array.rs`, and optional access helpers. Shared lowering materializes
their addresses and origin components using checked plan facts; target selection
must not follow nominal IDs back into MIR. E05/E08/E09/E12 cover these families;
all complex-place delivery is pending LA04, with scalar storage/static addressing
in the LA03 pilot. Zero-size/metadata-only types require an explicit LA02
disposition; `Obj`/interface views and `unit` do not imply owning object slots.

## Terminators and failures

Current owner paths are again relative to the x86 target. Specialized selectors
handle checked terminators before the basic `lower/terminator.rs` fallback.

| `MirTerminator` members | Current owner | Future shared lowering obligation / invariant | Evidence | Gap | Delivery (pending) |
| --- | --- | --- | --- | --- | --- |
| `Return` | `lower/terminator.rs`, `lower/call/*` | Unit/scalar or previously materialized aggregate destination; result survives cleanup and trace pop | E02, E05, E06, E16 | G03 | LA03 scalar; LA04 aggregate |
| `ReturnShared`, `ReturnOptionalShared` | `lower/terminator.rs` | Transfer live owner or zero niche to caller, no hidden aggregate destination | E07, E08 | — | LA04 |
| `Goto`, `Branch` | `lower/terminator.rs` | Explicit target CFG and canonical condition; stable entry/block ordering | E04 | G02 | LA03 |
| `ShiftCountCheck` | `lower/shift.rs` | Success/failure edges before operation; reject count at selected width | E03 | — | LA03 |
| `IntegerDivisorCheck` | `lower/integer_division.rs` | Matching divisor check and operation-specific failure attribution | E03 | G02 | LA03 |
| `PrimitiveCastRangeCheck` | `lower/primitive_cast.rs` | Matching finite/range relation with success-only conversion | E03 | — | LA03 |
| `CheckedCast`, `SharedCast` | `lower/type_operations.rs` | Runtime membership and success-only carrier/owner, exact failure edge | E12 | — | LA04 |
| `OptionalUnwrap`, `OptionalSharedUnwrap` | `lower/optional/access.rs` | Success-only payload/owner transfer, absent failure with exact layer | E08 | — | LA04 |
| `BeginOptionalView`, `BeginOptionalBoxView` | `lower/optional/access.rs` | Success, absence and overflow are separate edges; pin exact state/owner | E08 | — | LA04 |
| `CheckOptionalMutation` | `lower/optional/access.rs` | Pinned mutation reports; invalid internal pin state hard-traps | E08 | — | LA04 |
| `ArrayPositionCheck`, `ArrayOperationCheck`, `ArrayLoop` | `lower/array.rs` | Valid position/failure relation, selected failure result, forward/reverse counted loop with explicit index state | E09, E10 | — | LA04 |
| `Panic`, `Terminate` | `lower/terminator.rs` | Exact message/reason/location; ordered reporter, no unwind/extra cleanup | E03, E11, E16 | G05 mode handoff | LA03 scalar failures; LA04 remaining |

All `MirTerminationReason` members are migration obligations:
`ObjectCastFailure`, `OptionalAccessFailure`, `OptionalGuardOverflow`,
`OptionalPinnedMutation`, `ArrayAllocationFailure`, `ArrayIndexOutOfBounds`,
`ArrayInvalidSliceBounds`, `ArraySliceLengthMismatch`, `ShiftCountOutOfRange`,
`IntegerDivisionByZero`, `IntegerRemainderByZero`, `PrimitiveCastOutOfRange`.
Preserve current canonical messages and operation spans. Ownership-count
exhaustion and shared allocation failure are also generated reporter paths;
they are not additional `MirTerminationReason` variants. Invalid owner/header,
guard underflow and violated verified helper preconditions remain hard defects.
LA03/LA04 implement their respective failure paths and LA05 checks complete parity.

## Array and I/O suboperations

All array rows are pending LA04 shared lowering; target layout/ordinary selection
is pending LA03/LA04. `lower/array.rs` dispatches every member; specialized
siblings own anchors, slices, shared elements and generated helpers. E09/E10
are current evidence, with no additional LP02 family gap identified.

| `MirArrayInstruction` members | Current specialized owner | Shared expansion invariant |
| --- | --- | --- |
| `Allocate`, `AllocateElements` | `lower/array.rs` | Overflow/allocation check before construction; inline/shared backing representation and prefix zero |
| `BeginIndexed`, `BindIndexed`, `InitializeIndexedElement`, `AdvanceIndexedElement` | `lower/array.rs` | Explicit prefix/length/binding state; element epoch evaluates once; advance only after completion |
| `EndIndexedElement`, `CompleteIndexed` | `lower/array.rs` | Verified cleanup/backedge/publication markers currently no-op; preserve disposition rather than invent executable effects |
| `InitializeElement`, `CompleteElement` | `lower/array.rs` | Ordered element-list evaluation and exact initialized-prefix advancement |
| `InitializeNext`, `CopyNext` | `lower/array.rs`, `lower/array/lifecycle.rs`, `lower/array/shared_elements.rs` | Selected primitive/class/nested array/shared/optional element operations; recursive helpers explicit |
| `Publish`, `PublishShared` | `lower/array.rs` | Publish descriptor or shared owner after completed initialization; planned header/metadata |
| `Adopt`, `Replace` | `lower/array.rs` | Transfer produced backing; release old backing safely; no unintended extra clone |
| `ElementAssign`, `DestroyNext`, `Release` | `lower/array.rs`, `lower/array/lifecycle.rs` | Exact element assignment/destruction plan, reverse cleanup and balanced count/free |
| `AnchorBegin`, `AnchorEnd`, `AliasBind` | `lower/array/anchors.rs`, `lower/array.rs` | Secure backing across replacement; distinguish descriptor alias from detached element alias |
| `Normalize`, `Offset`, `Boundary` | `lower/array.rs`, `lower/array/slices.rs` | Signed/unsigned position normalization and boundary semantics; no unchecked element address before guard |
| `SliceCopy`, `SliceAssignNext` | `lower/array/slices.rs`, `lower/array.rs` | Snapshot/overlap semantics, selected element lifecycle, ordered source/destination indices |
| `SliceLengthCheck`, `SliceBoundsCheck` | `lower/array/slices.rs`, `lower/array.rs` | Exact failure reason before mutation or result publication |

Array element plan members are explicit migration obligations:

- Default: `Primitive`, `OptionalAbsent`, `Class`, `ArrayEmpty`, `SharedClass`,
  `SharedArrayEmpty`, `SharedOptionalBoxAbsent`.
- Copy and assignment each: `Primitive`, `OptionalPrimitive`, `Class`,
  `OptionalClass`, `Array`, `Shared`, `OptionalShared`, `Optional`.
- Destruction: `Trivial`, `Class`, `OptionalClass`, `Array`, `Shared`,
  `OptionalShared`, `Optional`.
- Ownership: `Inline`, `Shared`; loop kinds: `Ordinary`, `Indexed`;
  anchor kinds: `InlineOwner`, `InlineBacking`, `StableSharedOwner`,
  `CopiedSharedOwner`, `AdoptedSharedOwner`, `SecuredOptionalSharedOwner`.
- Positions: `Element`, `SliceBound`, `RangeOffset`; boundaries: `Start`, `End`;
  failure results: `AllocationSize`, `IndexOutOfBounds`, `InvalidSliceBounds`,
  `SliceLengthMismatch`.

LA04 must split family rows by concrete element/optional shape when recording
implemented coverage. The current helpers cover recursive class/array graphs;
their calls cannot be replaced with opaque legacy assembly snippets.

I/O members `StandardHandle`, `Open`, `Read`, `Write`, `Close` are dispatched in
`lower/io.rs`. Shared lowering forms anchored buffers and ordered runtime calls;
target selection assigns physical scalar/address arguments. E11 covers every
exact runtime symbol, pointer/length handling and alignment. Delivery is pending
LA04, with no additional current-behavior witness identified.

## Calls, generated bodies, runtime services and data

Call targets are `Direct`, `Static`, `Indirect`, `Method`, `Interface`;
method selection is `Direct` or `Virtual`. Receiver forms are `Method` and
`Interface`; arguments are `Value`, `Place`, `View`, `OwnedPlace`, `SharedOwner`.
Object origins are `Exact`, `Forwarded`, `Shared`, preserving static view,
complete-object address and dynamic metadata. These are logical components,
not a universal ABI register list. Current `abi.rs`/`lower/call/marshal.rs`
spill/marshal them alongside hidden result storage; future target planning
classifies shapes, shared lowering materializes values, selection assigns ABI
locations, and realization moves them. E02/E05/E07/E12/E13 are evidence;
E19 closes G03's combined call witness gap. Delivery remains pending for
LA03's pilot and LA04's full surface.

| Generated/external family | Current construction/domain | Future owner and invariant | Evidence / gap | Delivery (pending) |
| --- | --- | --- | --- | --- |
| Ordinary source callable bodies | `lower.rs::lower_definition`; `executable_definitions()` includes function, static initializer, initializer, copy constructor, copy assignment, destructor, method bodies | Shared lowered callable; never recreate physically absent bodies from dense declarations | E15; — | LA03 pilot, LA04 full |
| Six helpers per array ID: element initializer, element copier, clone, element destroyer, release, shared finalizer | `lower/array/helpers/{initialization,copy,destruction}.rs`; currently all declared array types | Shared helper LIR/worklist, explicit recursive calls and retain/free effects; one owner per symbol | E09, E10; — | LA04 |
| Raw-address complete class copy wrappers | `lower/array/lifecycle.rs::lower_class_copy_helpers`; class dependency closure from array element shapes | Shared helper construction with checked component shapes; recursion broken by ordinary calls | E06, E09; — | LA04 |
| Shared handle retain/release helpers | `lower/ownership/helpers.rs`; array element and static shutdown needs | Shared lowered graphs, inherited attribution and visible call clobbers; no source trace frame | E07, E16; G04 | LA04 |
| Complete class finalizers | `lower/finalize.rs::lower_all`; currently all classes | Shared expansion of destruction plan: `UserBody`, `Field`, `SharedField`, `OptionalSharedField`, `OptionalClassField`, `OptionalField`, `ArrayField`, `Base`; free remains with releasing caller | E06, E07; — | LA04 |
| Exact optional-box finalizers | `lower/optional_box.rs`, `lower/finalize/optional.rs`; `exact_optional` box types | Shared recursive optional payload cleanup; same ordinary lifecycle/call pipeline | E08; G04 for caller | LA04 |
| Program initializer/finalizer | `lower/static_lifecycle.rs`; verified activation/shutdown coordinator | Shared lowered callables; `ZeroDefault` versus `Explicit` activation, exact reverse shutdown; transitions require no target runtime state | E14; — | LA04 |
| Static cleanup shapes | `lower/static_lifecycle.rs`; `None`, `CompleteObject`, `OptionalClass`, `Shared`, `OptionalShared`, `AggregateOptional`, `Array` | Shared lowering realizes certified plan only; no backend activation discovery | E08, E14, E15; — | LA04 |
| Exported `main` entry wrapper | `lower.rs::entry_wrapper` | Target entry ABI plus explicit runtime-marker/initializer/entry/finalizer calls; preserve entry result across shutdown; no synthetic trace frame | E02, E14; — | LA03 minimal, LA04 lifecycle |
| External declared callees | `symbol.rs`, `lower/call.rs`; external link inventory | Symbol reference/signature only, no body; existing scalar C ABI and runtime marker preserved | E02, E11; — | LA03/LA04 by shape |

Runtime call inventory is `ska_rt_alloc`, `ska_rt_free`, `ska_rt_panic`,
`ska_rt_io_standard_handle`, `ska_rt_io_open`, `ska_rt_io_read`,
`ska_rt_io_write`, `ska_rt_io_close`, and the facade's current
`ska_rt_abi_v9` link guard. `ska_rt_trace_top` is TLS data, not a callable.
Allocation failure reporting belongs to shared lowering; free/invalid helper
preconditions retain hard-defect handling; I/O returns status to ordinary
checked library code. User external symbols remain their own reviewed C ABI.
No new runtime service or marker revision is implied by this migration.

| Artifact family/root | Current owner | Future responsibility / invariant | Evidence | Delivery (pending) |
| --- | --- | --- | --- | --- |
| Required runtime entities: `ClassDispatch`, `VirtualFamily`, `InterfaceRequirement`, `FunctionType`, `ArrayLifecycle`, `OptionalLifecycle`, `OptionalBoxLayout`, `StaticStorage`, `LiteralBacking` | `backend/retained_domain.rs`, `planning.rs` | Checked plan consumes narrow certified queries, never analysis internals or mutable certificates | E15, E17 | LA02 planning contract, LA04 consumption |
| Class dispatch/interface witnesses, shared-array and optional-box descriptors | `dispatch.rs`, `layout.rs` | Shared plan identities/dependencies, target encoding; stable dense slots with null only for verified-unused selections | E08, E12, E15 | LA04 |
| Writable static slots, including complete-mode inactive-field fallback | `static_fields.rs` | Certified active domain; fallback only for references in present complete-mode bodies; closure removes dead body/slot together | E14, E15 | LA04 |
| Immutable literal backings, including empty backing | `literal_data.rs`, `lower/strings.rs` | Canonical byte pooling/provenance and explicit metadata references | E11 | LA04 |
| Panic-message constants | `lower/terminator.rs::PanicMessagePool` | Exact reason byte strings and reference inventory; source panic also uses ordinary string slice | E03, E11 | LA03/LA04 |
| Trace strings, contexts, locations | `runtime_trace/{metadata,names}.rs` | Enabled-only source lookup; canonical retained ordering and pooling | E15, E16 | LA04 |
| Callable/data references and artifact closure | `artifacts.rs`, `machine.rs` | Current graph follows typed instruction variants and data initializers carrying symbol strings; future references gain typed identities. Roots are exported bodies; external symbols have no generated body. Complete mode skips closure; reachable mode requires it | E15 | LA04, LA05 adoption |

Dense declarations/layout planning are not executable-body authority. The current
backend deliberately constructs some helper/table families from complete type
inventories, then reachable mode removes unused artifacts. Migrating those
families must not accidentally change complete-mode observability or semantic
retention. Future demand-driven construction needs an explicit disposition and
equivalence evidence; it is not a hidden prerequisite for LA01.

## Trace, observation and error handoffs

| Current responsibility | Current owner/evidence | Planned handoff and required check | Delivery (pending) |
| --- | --- | --- | --- |
| Source visibility, frame eligibility and initial location | `runtime_trace/activation.rs`, E16 | Shared checked plan/lowering; only eligible source bodies push; helpers inherit outer attribution | LA04 |
| Push/pop, call-site replacement, reporting-edge replacement | `runtime_trace/instrumentation.rs`, E16 | Shared ordered trace actions; selection expands explicit TLS/address operations, temporaries and clobbers before placement | LA02 action schema, LA03 pilot, LA04 full |
| Omitted tracing | Facade sources isolation and metadata early omission, E17/E16 | No action/record/lookup/metadata/TLS references; report selection cannot alter policy | LP02 guards, LA04 parity |
| Call attribution: `SourceOperation`, `InheritedSourceOperation`, `SourceBodyFromOmittedHelper`, `NonReporting`, `HardDefectOnly`, `ProcessBoundary` | `lower/call/emission.rs`, E16 | Preserve attribution in lowered calls/actions. Current constructor audits it but discards the enum; do not treat physical `Call` as carrying that evidence today | LA02 metadata, LA04 lowering |
| Backend planning visits | `planning.rs`, E15/E14 | Current callable phases: `ArrayLegality`, `Legality`, `RuntimeTraceActivation`, `Frame`, `InstructionSelection`; static phases: `Declared`, `Active`, `Initializer`, `Finalizer`, `ConservativeFallback`, `Retained`, `Emitted`. Future phase-local observers preserve exact domains without driving semantics | LA02/LA03 interfaces, LA05 observations |
| Request-local reporting and dump inspection | `reporting`, driver orchestration, E18 | Typed new phase/events and immutable inspection adapters; dumps remain phase-owned; no timings in deterministic output; requested-only metrics | LA02/LA03 adapters, LA05 production |
| Source diagnostics and driver failure categories | Driver/reporting contract, E18 | Source errors remain frontend-owned; structured backend errors retain target/callable context; no error hidden by filtering or fallback | LA03/LA04 integration |
| Invalid generated product, target mismatch, stale placement | No current LIR equivalent | Private stage/origin/local-location invariant failure; verifier-owned seals; not a new source error or emitted runtime trap | LA02 virtual verification, LA03 placement/physical checks |

The new observation schema must distinguish checked planning, shared lowering,
selection, placement, realization, verification and closure. The driver currently
observes backend assembly emission as one phase; detailed LIR events do not yet
exist. Do not add public mutable products just for inspection.

## Phase publication and authority handoff

This table records responsibilities; names are conceptual, not new Rust types.
Private checked construction and full lowered callable publication are implemented.
Complete lowered-program closure and parent-bound target declarations are implemented.
Selected authority and native consumption remain downstream work.

| Product | Producer and permitted inputs | Publication obligations | Consumer authority | Observation/error obligation | Implementation owner |
| --- | --- | --- | --- | --- | --- |
| Checked target plan | Backend planning from `BackendInput` and immutable target facts | Valid runtime obligations/layout/component signatures; exact target/profile; fixed layout/signatures; generated inventories have explicit finalization owners | Narrow shared-lowering/selection views, no mutable MIR/certificate access | Planning domains and structured target/layout rejection | LA02/LA03 joint contract; LA04 full coverage |
| Verified lowered LIR | Shared MIR/lifecycle/helper lowering plus checked plan | Closed callable CFG, single definitions/dominance/edge signatures, memory/effects/trace/artifact references; no opaque lifecycle operation or physical resource; final helper worklist before complete-program publication | Selection consumes immutable executable meaning without reading MIR; edits consume seal and reverify | Canonical dump, requested metrics, stage/origin verifier failure | LA02 schema/seals; LA03 pilot; LA04 full construction |
| Verified selected LIR | Target selector plus lowered product and plan | All introduced CFG/calls/temporaries/clobbers/fixed/tied/resource constraints explicit; target/context bound; target-specific thunks equally verified | Placement consumes graph/operands/resources, no nominal source semantics | Canonical selected dump and target-selection/verification context | LA02 structural APIs; LA03 target schema/verifier |
| Checked placement result | Baseline placer from exact selected callable and target resource descriptions | Independent legality/value-flow/transfer checking, exact snapshot binding, legal scratch/object/save requirements | Realization consumes placements and declared recipes; no reinterpretation of semantic operations | Placement dump, counts and stage/local-location defects | LA03 |
| Verified physical code | Target realization from checked placement, frame requirements and target facts | No remaining virtual/frame references; legal instructions/offsets/edges, alignment/balance/saves and artifact references | Closure/rendering consume code/data references only; no selection, allocation or MIR queries | Frame/code metrics, encoding limits and physical verifier defects | LA03 pilot; LA04 full; LA05 integration |

Analyses borrow exact immutable products or belong to phase-local sessions;
mutation invalidates them. IDs alone do not certify freshness. Shared immutable
context and streaming observations avoid retaining all intermediate bodies.
These are accepted constraints for LA02 storage/ID decisions, not an instruction
to introduce global caches or a general pass manager.

## Acceptance witnesses and bounded follow-up

| Frozen walkthrough | Current evidence and readiness disposition | Downstream acceptance obligation |
| --- | --- | --- |
| Live arithmetic input with x86 tie versus AArch64 three-address form | E01 selection plus E19 runtime-loaded inputs reused after arithmetic result and a call; G01 closed | LA03 selected tie test plus independent placement corruption/value-flow test; synthetic three-address/resource view test, no AArch64 execution claim |
| Loop, join and checked signed division | E03 boundary/property/failure coverage plus E19 signed division/remainder through both joins and loop epochs with original inputs live; G02 closed | LA02 dominance/block-parameter/edge negatives; LA03 all selection-created correction blocks visible and transfers checked |
| Object result, receiver triple, integer/float pressure and indirect target | E05 hidden destination/receiver plus E19 direct/interface object results with seven integer/nine floating arguments and later argument calls; G03 closed | LA03 joint role-based shape/ABI contract; LA04 full method/virtual/interface result native parity; AArch64 `x8` witness remains proposed private mapping |
| Original allocation across finalizer and free | E07/E08 lifecycle integration plus E20 finalizer-before-free, mutable-owner replacement, all caller-saved integer/SIMD registers clobbered, original header freed once; G04 closed | LA04 lowered header value survives call, metadata/helper checks remain hard defects, source destructor gets its own trace frame |
| Width overlap, preserved resources and platform reservations | E02/E05 exercise current scalar x86 ABI; no machine resource model exists yet | LA03 synthetic overlap/partial-preservation/link-register/resource tests and real x86 assembler/C ABI probes; full AArch64 remains separate |

LP02 closes G01–G05 as current-behavior readiness obligations. No wrong-code
defect was found. Fixed homes still hide the future destructive-tie hazard;
native parity does not replace downstream constraint/placement verification.
Canonical byte/boolean coverage remains E01/E02; division property/zero-failure
coverage remains E03. Existing ABI-position/encoding assertions remain owned by
LA03; new semantic witnesses do not depend on temporary registers or offsets.

The live-integer spec has six runs: `negative_dividend`, `negative_divisor`,
`both_negative`, `both_positive`, `minimum_over_negative_one`, and
`maximum_over_two`. Operands and independent expected results come from process
arguments, preventing compile-time folding of the operations under test.
The aggregate spec has one run, `hidden_result_receiver_and_mixed_argument_pressure`;
weighted integer and exact floating results detect lost/permuted components.
Native lifecycle probes use ABI header identity and order, not assembly-text
matches. The count probe invokes the real current release selector; synthetic
finalizer/free bodies are enduring test doubles, never production scaffolding.

### Selected mode coverage (G05 closed)

| Witness / exact selection | Established dimension | Deliberate scope |
| --- | --- | --- |
| `operators/arithmetic::live_integer_inputs::{default,optimization-none,omit-runtime-trace}::<six runs above>` | Default/minimum MIR profiles, enabled/omitted tracing; native arithmetic, signed floor correction, minimum/-1 and maximum/2, joins/loops | Three builds, 18 executions; production reachable artifacts, reporting off |
| `calls/functions::aggregate_pressure::{default,optimization-none,omit-runtime-trace}::hidden_result_receiver_and_mixed_argument_pressure` | Same profile/trace variants; hidden result, receiver triple, integer/SIMD register and stack pressure, direct/indirect calls | Three builds/executions; production reachable artifacts, reporting off |
| E20 count probe; E07 `generated_dynamic_finalizer_executes_derived_then_base_and_frees_once` | Original header across destructive finalizer; ordinary generated dynamic-finalizer integration | Count probe bypasses MIR/traces deliberately to isolate the call-clobber obligation; E16 owns source/helper attribution |
| E17 `backend_input_exposes_sources_only_for_enabled_tracing`, facade/final-seal doctests, `backend_policy_rejects_frontend_state_and_accepts_verified_input_services` | Enabled-only source access; caller cannot attach omitted sources; raw/proof-rich input and mutation authority rejected; frontend phase roots forbidden, source/MIR/seals allowed | Current public visibility and direct source-reference policy only; no internal LIR phase isolation claim |
| E15 `sparse_backend_planning_visits_only_physically_retained_definitions`, `equal_retained_closures_emit_identical_runtime_trace_metadata` | Complete versus sparse bodies with tracing enabled; full/sparse reachable artifact closures identical | Reuses current target planning/trace witnesses rather than duplicating native fixtures |
| E15 `sparse_complete_and_artifact_retained_emission_never_resurrect_absent_bodies` | Complete/reachable artifact modes on sparse MIR with tracing omitted; deterministic repeated emission | Assembler acceptance and absent-body guards; no claim every native case is crossed with both artifact modes |
| E16 `runtime_trace_metadata_omission_never_requests_or_emits_trace_data`, `runtime_trace_frame_mixed_returns_preserve_results_and_restore_null`, `runtime_trace_attribution_native_generated_copy_and_finalizer_chains_omit_helpers` | Enabled/omitted metadata isolation; native trace push/pop and helper/finalizer attribution | Complements the new goldens' result parity without copying their scenarios |
| E18 `observation_preserves_success_artifacts_and_failure_diagnostics` | Quiet, reporting Off and reporting Trace emit identical success artifacts; quiet/Trace failures preserve diagnostics | Existing driver witness; native pressure cases need not also multiply reporting variants |

Run the new native witnesses with `scripts/golden.sh --filter
'**live_integer_inputs**' --filter '**aggregate_pressure**'`. The ordinary
repository gate includes all selected/reused witnesses. No scanner or policy
implementation change, target API extraction or orchestration change was needed.

Future negative-test ownership: LA02 owns unfinished CFG, duplicate definitions,
dominance, edge argument type/arity, forbidden payloads/effects, target/context
binding and seal privacy. LA03 owns missing clobbers, illegal fixed/tied/overlap
constraints, edge cycles, insufficient declared scratch, corrupt/stale placement,
large offsets and physical stack/save violations. LA04 owns helper recursion,
trace omission/attribution, sparse bodies and full artifact closure parity.
LA05 owns reporting/determinism/default adoption and fallback removal.

## Joint design questions and measurement readiness

The [frozen model design](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#joint-review-and-promotion-checkpoint)
settles the common LA02 contracts. Its roadmap tests their readiness; concrete
LA03 counterparts below must be resolved before dependent native implementation.
They refine accepted boundaries; they do not reopen shared lowering or
single-definition values:

### Common model readiness (LI01)

Assessed against `f1053782` and the preparation commits through `027af94f`.
The frozen common contracts represent the inherited walkthroughs; no conflict
requiring a design amendment was found. Declaration/identity readiness was the initial result; the task records below
cover subsequent program and selected delivery. Native evidence remains with
its scheduled owner. The present checker validates supplied facts;
it does not certify a production final-MIR projection.

| Joint question | Common schema agreement / present evidence | Remaining owner and required evidence |
| --- | --- | --- |
| Logical shape versus physical ABI | Explicit convention, scalar/address component roles and return shape; positive hidden-destination, receiver/origin and seven-integer/nine-float fixtures on both profile shapes | LI03 delivered logical call checks; LA03 freezes actual incoming/outgoing bindings and indirect-target timing; LA04 migrates full native pressure witness |
| Selected representation and operands | Distinct stage/local IDs and checked owner arenas; frozen ordered opcode-derived operand/tie/clobber interface can express fixed division resources, destructive add and three-address shapes | LI07 delivered synthetic payload/resource/timing contracts; LI08 independently verifies them; LA03 confirms early-clobber, call-result ordering and canonicalization in real x86 recipes |
| Flags and edges | Frozen atomic flag bundles and explicit edge occurrences/arguments represent current conditional labels and simultaneous loop transfers without hiding branches | LI02 constructed loop/diamond/parallel-edge drafts; LI04 delivered independent CFG/definition/dominance checks; LI07 delivered explicit atomic bundle edges; LI08 checks correction blocks and bundle consistency; LA03 implements critical-edge policy and independent swap/cycle transfer checking |
| Inventories and publication | Typed source/helper/coordinator/entry/thunk keys, canonical declaration lookup, absent-source rejection and context-bound handles; equal live plans do not share authority | LI05 delivered genuine callable seals/receipts; LI06 delivered worklists, program closure and parent-bound extensions; LA03 freezes streaming target discovery and verified thunk construction |
| Symbolic objects and scratch | Checked addressable/zero-size versus elided layouts; no value home or frame offset; frozen role separation represents semantic/trace/ABI and later placement requirements | LI02 delivered size-zero/elided objects and checked lifetime sites; LI07 delivered scratch/ABI descriptions; LI08 verifies their consistency; LA03 freezes actual slot shapes and bounded frame legalization |
| Inspection and errors | Immutable borrowed fact views, stable structural declaration failure reasons and public private-path compile-fail tests; omitted policy rejects trace inventory | LI03 delivered effects and trace construction checks; LI04 delivered structured graph failures; LI05 delivered full callable failures; LI10 supplies canonical phase dumps; LA03/LA05 wire real observations and native error conversion |

The model compiles in ordinary builds, with item-scoped non-test allowances for
delivered interfaces that currently have only regression consumers. The
[artifact ledger](LOW_LEVEL_IR_MODEL_ROADMAP.md#temporary-artifacts-and-discoveries)
assigns removal to each first real native consumer in LA03 (or its full-migration
owner in LA04). Child closure must transfer outstanding symbols and obligations;
neither local commits nor archival discharge them. No production gate,
placeholder verifier/placer, compatibility alias or exploratory lowering path
was introduced.

### Lowered draft construction (LI02)

Implemented against the user's LI01 commit `ff12421d`. Private `backend::lir`
now supplies callable/block/value/object tables and context-bound draft mutations,
scalar/address/memory/lifetime schemas and finite checks/evidence. The builder
preserves explicit entry and definition/result order, reserves forward references
and distinguishes parallel edge occurrences. Tests cover nonzero entry indices,
loop swaps, diamonds, all primitive cast cells, binary64 bits, symbolic zero-size
versus elided storage, widths, stride/size overflow, metadata and lifetime sites.

All 16 owner tests, maintained boundary/privacy regressions, ordinary validation
(3,194 compiler unit tests and 650 golden observations) and Rust 1.82.0 checking
passed. These are model/construction witnesses, not native operation delivery.
Independent guard protection, exact constant evidence, effects and memory extents
were delivered by LI05; structural dominance is delivered below. Complete-program publication, selected interfaces,
receipts and native planning/lowering/placement are still pending. The inherited
native coverage rows and preserved measurements are unchanged.

### Observable lowered execution (LI03)

Implemented against the user's LI02 commit `43df9ce6`. Calls carry checked
logical components, direct or signature-typed indirect targets and explicit
attribution. Memory effects track known objects/statics conservatively; unknown
accesses may alias either, and optional summaries can only widen mandatory effects.
Runtime services have checked signatures and reviewed effects. Trace plans/actions
respect policy and frame eligibility; reported failures, nonreturning calls and
hard traps are separate terminals. Compiler failure bytes share the native catalog.

Nine new owner tests cover these contracts and the ordinary shared-release graph
under enabled/omitted tracing, including freeing the original header after its
finalizer. `make check` passed (3,203 compiler unit tests, 12 boundary tests and
650 golden observations); Rust 1.82.0 checking passed. Drafts remain unsealed:
guard/effect/provenance and local trace verification were delivered by LI05; native trace-path parity remains pending; structural
dominance is delivered below. Native migration and preserved measurement evidence are unchanged.

### Independent graph/value-flow checking (LI04)

Implemented against the user's LI03 commit `68dec8ed`. The shared graph owner
checks structural descriptions for definitions, exact result/input/edge types,
entry/terminator rules and use positions before constructing CFG, reachability
and dominance once. The lowered adapter checks stored arena contexts and local
references independently of builder history. Shared selected-shape fixtures use
the same algorithm, without introducing selected payload storage or publication.

Seventeen new tests challenge malformed tables, foreign contexts/owners and
bounds, duplicate/unresolved definitions, ordering, successor-only and
nondominating uses, unreachable/newly reachable blocks, parallel critical edges
and simultaneous loop swaps. Earlier lowered loop/diamond/trace/release fixtures
also pass graph checking. Analysis borrows the exact draft; cross-block dominance
involving unreachable blocks is unknown. Graph success cannot mint a seal or
receipt. `make check` passed serially (3,220 compiler unit tests, 12 boundary tests,
runtime tests and 650 golden observations); Rust 1.82.0 checking passed.
Full callable verification/publication is delivered below. Complete-program
publication is delivered below; native delivery and preserved measurements are unchanged.

### Full lowered callable verification/publication (LI05)

The private `backend::lir::verify_callable` consumes a draft only after structural,
scalar/cast, finite domain, memory/provenance, effect, signature/service,
trace-association and typed-reference checks succeed. Shared borrowed schema rules
avoid a second operation legality catalog; derived facts are recomputed from the
stored graph. `VerifiedCallable` exposes immutable storage and genuine
`CompletionReceipt` witnesses bind exact snapshots to their live owners without
retaining predecessor bodies. Graph success alone cannot create either product.

Regressions exercise secured values versus reloads, entry/failure bypasses,
check-rooted dead/disconnected regions, exact constant and float bounds, forged
provenance, extents/alignment, narrowed effects, signatures, omitted/orphan trace
records, trace associations, absent source symbols, forward address definitions,
snapshot freshness and private diagnostic conversion. Shared-release and enabled
trace fixtures now pass full verification. Public compile-fail examples protect
the private publication paths.

This is supplied-model authority, not certification of real MIR lowering or
native trace-path/recipe equivalence. Complete-program inventory reconciliation
is delivered below; native planning/selection/emission remain LA03/LA04. Production public
paths and preserved measurements are unchanged.

### Finalized inventories and parent-bound declarations (LI06)

The private lowered program owner supplies canonical declared/building/verified
worklists, genuine chosen-snapshot completion registration and consuming
complete-program finalization. All required bodies and data initializers must
complete; reserved recursive helper references never reenter construction.
Receipts retain witnesses/dependencies without retaining predecessor graphs;
input reconciliation rejects stale replacements, other owners and foreign live
contexts/targets. Explicit byte/zero/address data definitions check extent,
category, addend, executable/static domain and intrinsic failure catalog bytes.
Authorized externals/runtime services and null dispatch slots require no body.

Target declaration extensions borrow the exact finalized parent inventory and
freeze target thunk signatures/constant layouts from existing immutable pools.
They cannot overwrite parent declarations or create new layout/signature facts,
semantic bodies or trace policy. This is declaration/data freeze, not a selected
seal. LI07 delivered resource/ABI catalogs and selected thunk drafts; selected
thunk completion is delivered by LI08.

Owner regressions cover recursive helpers, randomized arrival, missing/conflicting
completion, streaming receipt retention, stale/foreign witnesses, data cycles,
invalid categories/addends/initializers, active/inactive statics, legal external
and null dispositions, and parent-bound target extensions. Program and extension
public-path compile-fail examples preserve privacy. Production helper/data/target
discovery, native lowering/selection, physical relocations and adoption remain
LA03/LA04/LA05. LA03 must specify discovery/freeze and selection order relative
to input-body release; the model proves streaming bookkeeping, not reconstruction
of a released input or an implemented native streaming pipeline. Preserved
measurements and public backend APIs are unchanged.

| Decision to specify | Primary owner / required counterpart | Concrete required output |
| --- | --- | --- |
| Signature shape versus physical ABI locations | LA03 with LA02 call schema | Same caller/callee component plan; destinations/receivers/origins and indirect targets representable without fake scalar arguments |
| Helper/data requests and inventory freeze | LA02 with LA03 target thunk generation | Typed deterministic identities/worklist, recursion handling, context binding and staged publication; target requests cannot mutate frozen layout/signatures |
| Selected values, edge parameters and introduced blocks | LA02 with LA03 recipes/transfers | Construction/remapping APIs; simultaneous edge transfers and critical-edge policy; no private labels concealing CFG |
| Resource width/overlap, ties, flags and operand timing | LA03 with LA02 selected payload interface | Narrow structural descriptions sufficient for baseline placement and later allocation, including partial preservation and call clobbers |
| Frame scratch and bounded late legalization | LA03 with LA02 symbolic-object model | Distinct object/value/save/ABI/trace requirements; declared scratch and terminating large-offset recipes; no late semantic calls/failures |
| Trace, artifact and error/inspection metadata | LA02 with LA03 and driver/reporting owners | Immutable adapters, ordered attributed effects, typed dependencies, stage/origin/local-location errors; stable deterministic dumps |

The existing [measurement harness](../../scripts/measure_cleanup_baseline.py)
already records compiler/runtime/source identities, three compile samples by
default, native warmups/repeats, compiler wall/RSS summaries, repeated assembly
hashes, assembly byte size, total executable bytes, pass/analysis observations,
and native result digests. Native enabled/omitted workload pairs alternate order.
The complete pre-migration baseline at `9e3cebb1` is now
[durably retained](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md#reviewed-pre-migration-baseline).
Its equivalent-build cost comparison remains inconclusive for eleven short
compile timings; all required metrics and correctness/identity evidence are
complete. Future architecture adoption still needs cost clearance.

LP03 closes the six collection/comparison gaps identified by LP01. The
[foundation protocol](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md) is the
authoritative procedure; collection extends the same cleanup entry point.

| Original capability gap | Implemented disposition |
| --- | --- |
| Frozen corpus and historical-binary identity | Version-1 manifest, hashed source/stdlib/expected bytes; explicit binary commit/dirty/profile/toolchain/flags attestations and hashes, separate collecting-checkout identity |
| Paired compiler measurements | Compile and native warmups/repetitions alternate baseline/candidate order; raw ordered events retained |
| Required counts and repeat protocol | Defaults/minima five compile/nine native, warmups; comparison requires distinct paired captures; explicit smoke/subset captures cannot qualify adoption |
| Target code/frame metrics | ELF text-section sizes, fixed frame bytes, peak explicit stack reservations and direct static frame-memory operands; supported recipes have small fixtures, unknown shapes are unsupported |
| Raw samples/provenance/reporting and retention | Full reports, copied manifest, repeated assembly, untimed reporting equivalence/observations, tool/host/input/harness identities; checked-in raw reports/manifest/build record/untimed output/comparison plus hashes and untimed verification |
| Classification | Per-workload incompatibility, semantic/report/determinism invalidity, missing/noisy evidence, repeated 10% timing/15% RSS/text review gates and strict twice-larger-MAD rule; frame changes separately visible |

An additional runtime-input scalar/call/division kernel covers the material
pilot gap left by the existing recursion/range/trace workloads. Inputs
`17 1000000` and independent checksum `-993156` are frozen in the manifest.
No standard-library refactoring is required. E21 fixes the generated retain
helper's exhaustion-call alignment defect found while checking frame recipes;
The measured revision `9e3cebb1` includes that correction. Two independent full
captures use the same preserved binary with 15 compile/31 native samples per
role/workload, warmups and reversed alternating order. All 21 configurations'
artifact/static metrics agree across roles/captures; native digests match the
manifest and reporting emits equivalent assembly. No metrics or semantic evidence
are absent. Noisy compile classifications are retained without changing policy;
they remain an adoption obligation. The measurement guide owns exact results,
retention, host limits and requalification instructions.


## Contract reconciliation and transition disposition

The frozen design is a future contract. Current fixed homes, physical scratch
choices, target-owned lifecycle expansion, discarded call-attribution enum and
helper generation outside ordinary body lowering are expected migration work,
not discrepancies requiring a silent design amendment. The current artifact
graph uses symbol strings inside typed machine variants; typed artifact identity
is a later representation improvement, and assembly parsing is not introduced.

Preparation required no frozen-contract amendment, Rust extraction, production
bridge, rollout gate, scanner exception or exploratory phase product. The retain
helper's exhaustion-call alignment correction and private native probe are
enduring ABI protection. Boundary/privacy tests, live-input/aggregate goldens,
the opt-in measurement protocol and retained raw evidence have continuing owners.
The original cleanup workload definitions are unchanged. Existing lowering
remains production code until LA05 retires it; baseline stack placement is a
future implementation, not a bridge due for removal now.

Future changes must record their introducing commit and removal/transfer owner
in the child and program ledgers, even after local commits. This record is
maintained through LA05, which reconciles its obligations and archives it.

### Consuming edit and analysis readiness (LI09)

Implemented against LI08's committed endpoint `73b1fafe`. Both stages have
consuming editors for instruction/terminal uses, individual edge arguments,
block splits and explicit compact arena remaps. Complete-program edits consume
the chosen inventory and exact callable, preserve other completions/data, and
require fresh verified completion before reclosure. Direct callable editor entry
is restricted to its owning phase. New selected inventory input reconciliation
checks the chosen snapshot, matching the lowered contract.

| Contract | Delivered evidence | Remaining native obligation |
| --- | --- | --- |
| Definitions and remaps | Explicit partial maps, source receipt binding, coherent regenerated sites and preserved origins; duplicate/deleted IDs and faulty target definition rewrites rejected | Real target callback coverage for every opcode, object and indexed annotation |
| Guard / trace / reference coherence | Moved check evidence and trace associations relocated; changed divisor/check operands rejected; inherited attribution boundaries retained in selected dependencies | Actual selection evidence propagation and trace/frame recipes |
| CFG changes | Swaps and correction-graph splits/permutations republish; newly reachable nondominating uses and malformed edge arity fail | Semantics-preserving native transformations and independently checked transfer realization |
| Snapshot authority | Fresh receipts and reclosed inventories reject stale completions; static assertions enforce consuming signatures and non-cloneable callable/program/analysis authority | Downstream placement/realization must borrow or reconcile the exact selected snapshot |
| Read-only analyses | Borrowed graph sessions identify the actual immutable draft, with reachability/dominance and ordered edge occurrences | No global cache or pass manager is introduced; canonical inspection remains LI10 |

Unpublished editor drafts may have dirty derived metadata until rebuild/finish;
no editor analysis or provisional publication authority is provided. Full
reverification recomputes current facts and dependencies. This establishes
structural transformation contracts, not semantic equivalence, native ABI
preservation or optimization cost clearance. LA03 must implement concrete target
rewriters and verify actual payload/frame/transfer behavior. Scoped non-test
allowances remain ledgered for first native consumers.

### Selected verification readiness (LI08)

Shared descriptor checking and a mandatory target hook jointly publish immutable
selected callable snapshots. Completion receipts bind the exact lower input,
selection context/frozen extension and selected snapshot; declared target thunks
carry no fabricated input. Selected inventory closure rejects missing, stale,
duplicate and foreign-context completions.

| Synthetic witness | Delivered evidence | Concrete target obligation (LA03) |
| --- | --- | --- |
| Destructive / three-address add | Separate input/result IDs; tied input remains a later call use; both target shapes publish | Actual instruction constraints, canonicalization and preservation of the live input |
| Division correction diamond | Explicit guard, quotient/remainder temporaries, correction block and join; unsecured division rejected by target hook | Zero and overflow handling, signed-floor recipes, fixed registers and remapping |
| Loop / swap / parallel / critical edges | Shared graph checks ordered simultaneous arguments and individual edge occurrences | Independent cycle-breaking transfers and real critical-edge policy |
| Hidden destination / receiver / mixed banks | Twenty ordered components, independent banks and secured signature-typed indirect target outside arguments | SysV/AAPCS entry/call/return assignment, bank exhaustion and marshalling interference |
| Release and trace | Explicit count load and ownership branches; original header remains the free argument after finalizer; omitted trace effects/objects rejected; enabled dependencies and inherited helper attribution retained | Real count/header loads and ownership paths, service footprints and frame/location updates |
| Extended resources and thunks | Overlapping narrow/wide views, partial unit preservation, frozen thunk publication and context/snapshot-bound receipts | Actual resource catalog, reservations, preservation, native thunk discovery and legalization |

Private mutations challenge slot/tie/timing, fixed width, scratch, flow, ABI,
reference/effect and clobber mistakes independently of checked append. Wrong
profiles, explicit target rejection and malformed edge arguments fail. Lowered
IDs cannot inhabit selected operand storage. Earlier construction regressions
remain useful; they certify drafts only. No native target registration, placer,
checker or physical-preservation claim is introduced. Consuming edits and snapshot borrowing are delivered by LI09; canonical
inspection remains LI10.

### Selected contract readiness (LI07)

Implemented against the user's LI06 commit `ddc5a97d`, with the model baseline
`f1053782`. `backend::selected` compiles in ordinary builds and gives shared graph
consumers fresh arenas and borrowed opcode-derived descriptions. Construction
alone confers no selected seal or placement authority; no native selection switch
or physical instruction enum enters the shared owner.

| Joint counterpart | Present structural evidence | Remaining obligation |
| --- | --- | --- |
| Logical versus physical ABI | Exact component/representation checks; hidden destination and receiver bindings; fixed resources and checked symbolic slots | LA03 freezes real entry/call/return areas and indirect-target timing |
| Representations and operands | Nonzero widths, signature-qualified addresses, opcode-derived use/def slots, destructive tie with separate IDs, three-address shape and frozen event iteration | LA03 confirms actual opcodes and canonicalization |
| Flags and edges | Atomic synthetic terminal bundle names two actual stored edges; shared CFG/dominance checking consumes target descriptions | LA03 supplies actual flag recipes/transfers |
| Inventories and publication | Source draft admission checks finalized parent receipts; thunk drafts resolve only frozen extension declarations; distinct selection contexts reject foreign handles | LA03 supplies native discovery |
| Objects, resources and scratch | Semantic/trace/ABI symbolic object roles, explicit origin maps, bounded recipe scratch; same/cross-bank overlap, reservation and partial-width preservation tests | LA03 supplies real footprints, preservation and frame legalization |
| Inspection and failures | Shared `GraphView` projection over selected storage; structured construction errors; private-path compile-fail and selected-core dependency guard | LI10 canonicalizes phase inspection; LA03/LA05 wire production observations |

The construction interface is now independently checked by LI08. Native preservation and allocator
correctness are not established by these witnesses. No separate discovery or
accepted-design amendment was required; scoped non-test allowances remain
tracked in the model artifact ledger and expire with native consumers.

## Preparation handoff and next designs

The [LA02 model/construction/verification design](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md)
is accepted, frozen and promoted. Its [implementation roadmap](LOW_LEVEL_IR_MODEL_ROADMAP.md)
is in progress; LI01–LI09 are complete and LI10 is next. The
[common readiness record](#common-model-readiness-li01) covers the inherited
walkthroughs and the implemented declaration/identity foundation. LA03's
concrete target-selection and physical-realization
agreements remain pending before dependent native implementation.
The phase design remains the inherited frozen authority; its preparation
roadmap is complete, while the architecture program and cleanup audit's A22
remain in progress. The model's
[joint contract checkpoint](LOW_LEVEL_IR_MODEL_DESIGN_PROPOSAL.md#joint-review-and-promotion-checkpoint)
records common contracts and required native counterparts. Promotion and roadmap
creation do not mark any new-pipeline delivery complete.

Carry these accepted constraints into both designs:

- Layout-specialized shared lowering expands lifecycle work before publication.
  Checked planning, lowered LIR, selected LIR, placement and physical realization
  have separate payloads and verification authority bound to one context/target
  and exact input snapshot.
- Machine values have one definition, dominance and simultaneous block-edge
  arguments. Semantic MIR locals remain memory; SSA conversion and scalar
  promotion are separate work. Lowered LIR contains no physical registers,
  target instructions or concrete frame offsets.
- Selection exposes complete CFG, ABI components, effects, clobbers and trace
  requirements. Target realization owns physical ABI assignment, frames,
  declared scratch and bounded legalization. Baseline stack placement needs an
  independent checker and must be complete without register allocation.
- Final-MIR authority, sparse executable bodies, certified statics, stable
  dispatch, trace omission and typed artifact dependencies remain explicit.
  There is no second semantic reachability calculation or assembly dependency
  inference. AArch64 witnesses guide contracts without claiming a second backend.

| Owner | Remaining deliverable / exit obligation |
| --- | --- |
| LA02 | Checked declarations/identities and complete lowered execution vocabulary delivered by LI01–LI03, independent graph/value-flow verification by LI04 and full callable scalar/memory/effect/trace verification, immutable publication and genuine receipts by LI05; program closure and parent-bound target declarations delivered by LI06; remapping, selected interfaces, editing and phase inspection/dumps still pending |
| LA03 | Target resources and overlap/tie/operand timing, ABI plans and complete selection, parallel transfers, checked stack placement, symbolic frames and physical legalization; end-to-end scalar/control-flow/call pilot plus native x86 and synthetic AArch64 contract witnesses |
| LA04 | Full operation/helper migration and native parity for lifecycle, objects, optionals, arrays, I/O, traces, entry/statics and complete/reachable artifacts; update each inventory row with delivery evidence |
| LA05 | One production LIR pipeline, reporting/determinism parity, legacy-path removal, portability/cumulative review and frozen foundation cost acceptance |
| LA06 | Separate allocation design, implementation/checking and measured adoption after foundation consolidation |

Use the existing named witnesses, acceptance walkthroughs and future negative-test
ownership above when scheduling tests; current-backend success is not evidence
that a nonexistent LIR verifier, selection constraint or placement checker passes.

The [pre-migration record](../development/LOW_LEVEL_COMPILER_MEASUREMENTS.md#reviewed-pre-migration-baseline)
at `9e3cebb1` retains all 21 supported configurations, exact observations,
deterministic artifacts and two full equivalent-build captures. Eleven compile
timings remain inconclusive under the frozen policy; input readiness does not
grant future adoption cost clearance. Improve compatible measurement support or
host control before resolving those uncertainties.

Closing review made stack-register writes in the second operand of `xchg`/`xadd`
and unsupported `loop` control flow conservatively unsupported. Untimed review
of all 4,396 role/capture callable observations found no change to retained
metrics. Original evidence and its historical harness identity remain untouched.
The guard changes the current harness fingerprint: future paired comparisons
must recapture the preserved baseline and candidate with one compatible harness,
as the measurement procedure requires. No timing recapture is needed for this
preparation closure.

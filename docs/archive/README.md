# Archived Implementation Roadmaps

This directory contains completed implementation plans. They preserve the
reasoning, task breakdown, and acceptance criteria used while building the
current compiler, but they are historical records rather than descriptions of
the current architecture or language surface.

For current information, use:

- [the language overview](../language/README.md);
- [language and compiler status](../language/STATUS.md);
- [the implemented grammar](../language/GRAMMAR.md);
- [the module-system language contract](../language/MODULES_AND_INTEROP.md);
- [the compiler architecture](../compiler/README.md);
- [compiler phases and IR](../compiler/PHASES_AND_IR.md);
- [the module-system compiler contract](../compiler/MODULE_SYSTEM.md);
- [the backend and target contract](../compiler/BACKEND.md);
- [the runtime ABI](../compiler/RUNTIME_ABI.md);
- [driver and artifacts](../compiler/DRIVER_AND_ARTIFACTS.md);
- [structured compiler reporting](../compiler/REPORTING.md);
- [the development workflow](../development/README.md);
- [testing](../development/TESTING.md);
- [debugging the compiler](../development/DEBUGGING.md);
- [active and planned roadmaps](../roadmaps/README.md).

Archived plans:

- [First vertical slice](FIRST_VERTICAL_SLICE_ROADMAP.md)
- [`i64` output and golden-test observability](I64_OUTPUT_ROADMAP.md)
- [`bool` and conditional control flow](BOOL_CONDITIONALS_ROADMAP.md)
- [Remaining primitive types](PRIMITIVE_TYPES_ROADMAP.md)
- [Compiler implementation cleanup](IMPLEMENTATION_CLEANUP_ROADMAP.md)
- [First inline objects](INLINE_OBJECTS_ROADMAP.md)
- [Alias parameters](ALIAS_PARAMETERS_ROADMAP.md)
- [Class-typed inline object fields](INLINE_OBJECT_FIELDS_ROADMAP.md)
- [Deterministic destruction](DETERMINISTIC_DESTRUCTION_ROADMAP.md)
- [Object value semantics](OBJECT_VALUE_SEMANTICS_ROADMAP.md)
- [Compiler maintainability cleanup](MAINTAINABILITY_ROADMAP.md)
- [Documentation overhaul](DOCUMENTATION_OVERHAUL_ROADMAP.md)
- [Restricted polymorphism](POLYMORPHISM_ROADMAP.md)
- [Object casts](OBJECT_CASTS_ROADMAP.md)
- [Constructor overloads and explicit copy construction](CONSTRUCTOR_SEMANTICS_ROADMAP.md)
- [Shared ownership and heap allocation](SHARED_OWNERSHIP_ROADMAP.md)
- [Explicit shared dereference](EXPLICIT_SHARED_DEREFERENCE_ROADMAP.md)
- [Intel-syntax x86-64 assembly](INTEL_ASSEMBLY_SYNTAX_ROADMAP.md)
- [Explicit optional values](OPTIONAL_VALUES_ROADMAP.md)
- [Arrays](ARRAYS_ROADMAP.md)
- [Initial whole-program module system](MODULE_SYSTEM_ROADMAP.md)
- [Private members and static methods](PRIVATE_AND_STATIC_MEMBERS_ROADMAP.md)
- [Primitive integer casts and comparisons](PRIMITIVE_INTEGER_OPERATIONS_ROADMAP.md)
- [String types](STRINGS_ROADMAP.md)
- [Private ordinary initializers](PRIVATE_INITIALIZERS_ROADMAP.md)
- [Primitive local reassignment](PRIMITIVE_LOCAL_REASSIGNMENT_ROADMAP.md)
- [Panic and unrecoverable failure reporting](PANIC_ROADMAP.md)
- [Panic runtime traces](PANIC_RUNTIME_TRACE_ROADMAP.md)
- [While loops and loop exits](WHILE_LOOPS_ROADMAP.md)
- [Cyclic module imports](CYCLIC_IMPORTS_ROADMAP.md)
- [Eager boolean operators](EAGER_BOOLEAN_OPERATORS_ROADMAP.md)
- [Short-circuit boolean expressions](SHORT_CIRCUIT_BOOLEAN_EXPRESSIONS_ROADMAP.md)
- [Equivalent path-state compaction](PATH_STATE_COMPACTION_ROADMAP.md)
- [Optional initialization responsibility split](OPTIONAL_INITIALIZATION_RESPONSIBILITY_ROADMAP.md)
- [Shared ownership verifier responsibility split](SHARED_OWNERSHIP_VERIFIER_RESPONSIBILITY_ROADMAP.md)
- [Integer bitwise operators and checked shifts](INTEGER_BITWISE_AND_SHIFT_OPERATORS_ROADMAP.md)
- [Integer division and remainder](INTEGER_DIVISION_AND_REMAINDER_ROADMAP.md)
- [Remaining floating-point operators](REMAINING_FLOATING_POINT_OPERATORS_ROADMAP.md)
- [Complete primitive cast matrix](PRIMITIVE_CAST_MATRIX_ROADMAP.md)
- [Standard I/O](STANDARD_IO_ROADMAP.md)
- [Primitive string conversions](PRIMITIVE_STRING_CONVERSIONS_ROADMAP.md)
- [Zero-default static fields](STATIC_FIELDS_ROADMAP.md)
- [Static field initialization and shutdown](STATIC_FIELD_INITIALIZATION_ROADMAP.md)
- [Compact Ryū binary64 formatting](COMPACT_RYU_F64_FORMATTING_ROADMAP.md)
- [Efficient binary64 parsing](EFFICIENT_F64_PARSING_ROADMAP.md)
- [Integer string helper modules](INTEGER_STRING_HELPER_MODULES_ROADMAP.md)
- [Hexadecimal integer and byte literals](HEXADECIMAL_INTEGER_AND_BYTE_LITERALS_ROADMAP.md)
- [Produced object alias arguments](PRODUCED_OBJECT_ALIAS_ARGUMENTS_ROADMAP.md)
- [Program arguments](PROGRAM_ARGUMENTS_ROADMAP.md)
- [Spec-driven parallel golden test runner](GOLDEN_TEST_RUNNER_ROADMAP.md)
- [Golden stream matcher lists](GOLDEN_STREAM_MATCHER_LISTS_ROADMAP.md)
- [Explicit array element-list construction](ARRAY_ELEMENT_LIST_CONSTRUCTION_ROADMAP.md)
- [Compositional optional types](COMPOSITIONAL_OPTIONAL_TYPES_ROADMAP.md)
- [Shared optional boxes](SHARED_OPTIONAL_BOXES_ROADMAP.md)
- [Generic classes](GENERIC_CLASSES_ROADMAP.md)
- [Generic interfaces](GENERIC_INTERFACES_ROADMAP.md)
- [General iteration](GENERAL_ITERATION_ROADMAP.md)
- [Interface-based operator overloading](OPERATOR_OVERLOADING_ROADMAP.md)
- [Generic ranges and tight range loops](GENERIC_RANGES_ROADMAP.md)
- [Produced exact-class method receivers](PRODUCED_EXACT_CLASS_METHOD_RECEIVERS_ROADMAP.md)
- [Structural indexing and slicing](STRUCTURAL_INDEXING_AND_SLICING_ROADMAP.md)
- [Produced-object field reads](PRODUCED_OBJECT_FIELD_READS_ROADMAP.md)
- [Capture-free function values](FUNCTION_VALUES_ROADMAP.md)
- [Generic static-member dot syntax](GENERIC_STATIC_MEMBER_DOT_SYNTAX_ROADMAP.md)
- [Private cell fields](PRIVATE_CELL_FIELDS_ROADMAP.md)
- [`Str` cached-hash migration](STR_CACHED_HASH_MIGRATION_ROADMAP.md)
- [Primitive box classes](PRIMITIVE_BOX_CLASSES_ROADMAP.md)
- [Final fields](FINAL_FIELDS_ROADMAP.md)

Resolved string-design inputs:

- [String types design proposal](STRINGS_DESIGN_PROPOSAL.md)

Resolved loop-design inputs:

- [While loops design proposal](WHILE_LOOPS_DESIGN_PROPOSAL.md)
- [General iteration design proposal](GENERAL_ITERATION_DESIGN_PROPOSAL.md)

Resolved range-design inputs:

- [Generic ranges and tight range loops design proposal](GENERIC_RANGES_DESIGN_PROPOSAL.md)

Resolved operator-design inputs:

- [Primitive operator semantics design proposal](OPERATORS_DESIGN_PROPOSAL.md)
- [Interface-based operator overloading design proposal](OPERATOR_OVERLOADING_DESIGN_PROPOSAL.md)

Resolved array-design inputs:

- [Explicit array element-list construction design proposal](ARRAY_ELEMENT_LIST_CONSTRUCTION_DESIGN_PROPOSAL.md)
- [Structural indexing and slicing design proposal](STRUCTURAL_INDEXING_AND_SLICING_DESIGN_PROPOSAL.md)

Resolved module-system design inputs:

- [Niflheim module-system audit](MODULE_SYSTEM_NIFLHEIM_AUDIT.md)
- [Initial Skald module-system design record](SKALD_INITIAL_MODULE_SYSTEM_PROPOSAL.md)

Resolved golden-test-runner design inputs:

- [Spec-driven parallel golden test runner design](GOLDEN_TEST_RUNNER_DESIGN_PROPOSAL.md)

Resolved panic runtime-trace inputs:

- [Panic runtime trace design record](PANIC_RUNTIME_TRACE_DESIGN_PROPOSAL.md)
- [Panic runtime trace investigation](PANIC_RUNTIME_TRACE_INVESTIGATION.md)

Resolved compiler-reporting design inputs:

- [Structured compiler reporting design proposal](STRUCTURED_REPORTING_DESIGN_PROPOSAL.md)

Supporting records for the documentation overhaul:

- [migration inventory](DOCUMENTATION_OVERHAUL_INVENTORY.md)
- [resolved discoveries](DOCUMENTATION_OVERHAUL_DISCOVERIES.md)

Resolved compiler-maintainability follow-ups:

- [maintainability discoveries](MAINTAINABILITY_DISCOVERIES.md)
- [polymorphism maintainability discoveries](POLYMORPHISM_DISCOVERIES.md)

Resolved object-cast follow-ups:

- [object-cast discoveries](OBJECT_CASTS_DISCOVERIES.md)

Resolved shared-ownership follow-ups:

- [shared-ownership maintainability discoveries](SHARED_OWNERSHIP_DISCOVERIES.md)

Resolved optional-values follow-ups:

- [optional-values maintainability discoveries](OPTIONAL_VALUES_DISCOVERIES.md)
- [compositional optional type discoveries](COMPOSITIONAL_OPTIONAL_TYPES_DISCOVERIES.md)

Resolved shared-optional-box design inputs:

- [shared optional boxes design record](SHARED_OPTIONAL_BOXES_DESIGN_PROPOSAL.md)

Resolved generic-class design inputs:

- [generic classes design record](GENERIC_CLASSES_DESIGN_PROPOSAL.md)

Resolved generic-interface design inputs:

- [generic interfaces design record](GENERIC_INTERFACES_DESIGN_PROPOSAL.md)

Resolved generic-class follow-ups:

- [generic classes discoveries](GENERIC_CLASSES_DISCOVERIES.md)
- [generic array copy lifecycle discovery](GENERIC_ARRAY_COPY_LIFECYCLE_DISCOVERY.md)

Resolved function-value design inputs:

- [capture-free function values design record](FUNCTION_VALUES_DESIGN_PROPOSAL.md)

Resolved private-cell design inputs:

- [private cell fields design record](PRIVATE_CELL_FIELDS_DESIGN_PROPOSAL.md)

Resolved final-field design inputs:

- [final fields design record](FINAL_FIELDS_DESIGN_PROPOSAL.md)

Resolved string cached-hash follow-ups:

- [`Str` cached-hash migration discovery](STR_CACHED_HASH_MIGRATION_DISCOVERY.md)

Resolved shared-optional-box follow-ups:

- [shared optional boxes discoveries](SHARED_OPTIONAL_BOXES_DISCOVERIES.md)

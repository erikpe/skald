use crate::mir::test_fixtures::{
    add_body_storage_lifetimes as fixture_add_body_storage_lifetimes, assign as fixture_assign,
    block as fixture_block, call as fixture_call,
    checked_integer_division_program as fixture_checked_integer_division_program,
    checked_primitive_cast_program as fixture_checked_primitive_cast_program,
    checked_shift_program as fixture_checked_shift_program,
    conditional_full_expression_cleanup_program as fixture_conditional_cleanup_program,
    function_declaration as fixture_function_declaration,
    function_definition as fixture_function_definition,
    integer_bitwise_program as fixture_integer_bitwise_program, io_program as fixture_io_program,
    member_definition as fixture_member_definition, parameter as fixture_parameter,
    receiver_storage as fixture_receiver_storage,
    standard_io_program as fixture_standard_io_program,
    standard_io_program_with_additional_bodies as fixture_standard_io_program_with_additional_bodies,
    storage as fixture_storage, storage_dead as fixture_storage_dead,
    storage_live as fixture_storage_live, store as fixture_store, value as fixture_value,
    OneBlockDefinition,
};
use crate::{
    backend::{emit_assembly, Target},
    identity::{
        BindingId, ClassId, CopyConstructorId, FieldId, FunctionId, InitializerId, LocalId,
        MethodId, ParameterId,
    },
    mir::{
        verify_mir, BlockId, MirAliasAccess, MirArgument, MirAssignment, MirBasicBlock,
        MirBinaryOperation, MirBody, MirCall, MirCallTarget, MirClassDeclaration,
        MirClassDeclarationTable, MirCleanup, MirComparisonOperand, MirComparisonPredicate,
        MirCopyCapability, MirCopyConstruction, MirDestructionPlan, MirEndFullExpression,
        MirFieldDeclaration, MirFunctionDeclaration, MirFunctionDeclarationTable,
        MirFunctionDefinition, MirFunctionDefinitionTable, MirFunctionLinkage, MirInitialize,
        MirInitializerDeclaration, MirInstruction, MirIntegerBitwiseOperation, MirIntegerType,
        MirMemberDefinition, MirMemberDefinitionTable, MirMethodCallTarget, MirMethodDeclaration,
        MirMethodKind, MirMethodReceiver, MirObjectOrigin, MirParameter, MirParameterMode,
        MirPlace, MirPrimitiveCast, MirPrimitiveComparison, MirPrimitiveType, MirProgram,
        MirReceiverAccess, MirRvalue, MirRvalueKind, MirSelectedCopyOperation, MirSharedCopy,
        MirSharedRelease, MirSharedTarget, MirStorage, MirStorageKind, MirTerminationReason,
        MirTerminator, MirType, MirUnaryOperation, MirValue, MirViewTarget, MirVirtualFamilyTable,
        StorageId, ValueId,
    },
    source::SourceDatabase,
    test_support::{lower_source_to_assembly, lower_source_to_mir},
};

mod source_support;
use source_support::*;
mod scalar_fixtures;
use scalar_fixtures::*;
mod primitive_cast_fixtures;
use primitive_cast_fixtures::*;
mod control_flow_fixtures;
mod primitive_cast_oracle;
use control_flow_fixtures::*;
mod object_fixtures;
use object_fixtures::*;
mod alias_fixtures;
use alias_fixtures::*;
mod native_support;
use native_support::*;

mod arrays;
mod assembler;
mod calls;
mod control_flow;
mod copy;
mod destruction;
mod floating_comparisons;
mod floating_division;
mod instruction_selection;
mod integer_division;
mod interface_dispatch;
mod io;
mod legality;
mod native_execution;
mod object_results;
mod objects;
mod optional_values;
mod primitive_binding_assignment;
mod primitive_casts;
mod shared_ownership;
mod shifts;
mod static_inheritance;
mod strings;
mod termination;
mod type_operations;
mod value_parameters;
mod virtual_dispatch;
use objects::println_i64_stub;

use crate::mir::test_fixtures::{
    assign as fixture_assign, block as fixture_block, call as fixture_call,
    function_declaration as fixture_function_declaration,
    function_definition as fixture_function_definition,
    member_definition as fixture_member_definition, parameter as fixture_parameter,
    receiver_storage as fixture_receiver_storage, storage as fixture_storage,
    store as fixture_store, value as fixture_value, OneBlockDefinition,
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
        MirClassDeclarationTable, MirCleanup, MirCopyCapability, MirCopyConstruction,
        MirDestructionPlan, MirEndFullExpression, MirFieldDeclaration, MirFunctionDeclaration,
        MirFunctionDeclarationTable, MirFunctionDefinition, MirFunctionDefinitionTable,
        MirFunctionLinkage, MirInitialize, MirInitializerDeclaration, MirInstruction,
        MirMemberDefinition, MirMemberDefinitionTable, MirMethodCallTarget, MirMethodDeclaration,
        MirMethodKind, MirMethodReceiver, MirObjectOrigin, MirParameter, MirParameterMode,
        MirPlace, MirProgram, MirReceiverAccess, MirRvalue, MirRvalueKind,
        MirSelectedCopyOperation, MirSharedCopy, MirSharedRelease, MirSharedTarget, MirStorage,
        MirStorageKind, MirTerminator, MirType, MirUnaryOperation, MirValue, MirViewTarget,
        MirVirtualFamilyTable, StorageId, ValueId,
    },
    source::SourceDatabase,
    test_support::{lower_source_to_assembly, lower_source_to_mir},
};

mod source_support;
use source_support::*;
mod scalar_fixtures;
use scalar_fixtures::*;
mod control_flow_fixtures;
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
mod instruction_selection;
mod interface_dispatch;
mod legality;
mod native_execution;
mod object_results;
mod objects;
mod optional_values;
mod shared_ownership;
mod static_inheritance;
mod strings;
mod type_operations;
mod value_parameters;
mod virtual_dispatch;
use objects::println_i64_stub;

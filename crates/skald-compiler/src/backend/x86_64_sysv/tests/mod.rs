use std::{
    io::Write,
    process::{Command, Stdio},
};

use crate::mir::test_fixtures::{
    assign as fixture_assign, block as fixture_block, call as fixture_call,
    function_declaration as fixture_function_declaration,
    function_definition as fixture_function_definition,
    member_definition as fixture_member_definition, parameter as fixture_parameter,
    receiver_storage as fixture_receiver_storage, storage as fixture_storage,
    store as fixture_store, value as fixture_value, OneBlockDefinition,
};
use crate::{
    backend::{emit_assembly, Target, RUNTIME_ABI_MARKER_SYMBOL},
    identity::{
        BindingId, ClassId, FieldId, FunctionId, InitializerId, LocalId, MethodId, ParameterId,
    },
    mir::{
        verify_mir, BlockId, MirAliasAccess, MirArgument, MirAssignment, MirBasicBlock,
        MirBinaryOperation, MirBody, MirCall, MirCallTarget, MirClassDeclaration,
        MirClassDeclarationTable, MirCleanup, MirCopyCapability, MirCopyConstruction,
        MirDestructionPlan, MirEndFullExpression, MirFieldDeclaration, MirFunctionDeclaration,
        MirFunctionDeclarationTable, MirFunctionDefinition, MirFunctionDefinitionTable,
        MirFunctionLinkage, MirInitialize, MirInitializerDeclaration, MirInstruction,
        MirMemberDefinition, MirMemberDefinitionTable, MirMethodDeclaration, MirParameter,
        MirParameterMode, MirPlace, MirProgram, MirReceiverAccess, MirRvalue, MirRvalueKind,
        MirSelectedCopyOperation, MirStorage, MirStorageKind, MirTerminator, MirType,
        MirUnaryOperation, MirValue, StorageId, ValueId,
    },
    source::SourceDatabase,
    test_support::{lower_source_to_assembly, lower_source_to_mir, TemporaryFile},
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

mod assembler;
mod calls;
mod control_flow;
mod copy;
mod destruction;
mod instruction_selection;
mod legality;
mod native_execution;
mod object_results;
mod objects;
mod static_inheritance;
mod value_parameters;
use objects::println_i64_stub;

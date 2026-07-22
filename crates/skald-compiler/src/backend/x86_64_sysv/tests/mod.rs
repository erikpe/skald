use std::{
    io::Write,
    process::{Command, Stdio},
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
        MirSelectedCopyOperation, MirStorage, MirStorageKind, MirStore, MirTerminator, MirType,
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
mod value_parameters;
use objects::println_i64_stub;

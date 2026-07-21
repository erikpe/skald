use std::{
    io::Write,
    process::{Command, Stdio},
};

use crate::{
    backend::{emit_assembly, Target},
    identity::{
        BindingId, ClassId, FieldId, FunctionId, InitializerId, LocalId, MethodId, ParameterId,
    },
    mir::{
        verify_mir, BlockId, MirAliasAccess, MirArgument, MirAssignment, MirBasicBlock,
        MirBinaryOperation, MirBody, MirCall, MirCallTarget, MirClassDeclaration,
        MirClassDeclarationTable, MirDestructionPlan, MirFieldDeclaration, MirFunctionDeclaration,
        MirFunctionDeclarationTable, MirFunctionDefinition, MirFunctionDefinitionTable,
        MirFunctionLinkage, MirInitialize, MirInitializerDeclaration, MirInstruction,
        MirMemberDefinition, MirMemberDefinitionTable, MirMethodDeclaration, MirParameter,
        MirParameterMode, MirPlace, MirProgram, MirReceiverAccess, MirRvalue, MirRvalueKind,
        MirStorage, MirStorageKind, MirStore, MirTerminator, MirType, MirUnaryOperation, MirValue,
        StorageId, ValueId,
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
mod instruction_selection;
mod legality;
mod native_execution;
mod objects;
use objects::println_i64_stub;

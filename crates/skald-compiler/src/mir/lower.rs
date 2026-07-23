//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        HirBlock, HirCallArgument, HirExpression, HirExpressionKind, HirLocal, HirLocalInitializer,
        HirObjectProducer, HirObjectReturn, HirObjectSource, HirParameter, HirParameterMode,
        HirProgram, HirReturnValue, HirSelectedCopyOperation, HirStatement, Type,
    },
    identity::{BindingId, CallableId, ClassId},
    source::Span,
};
use std::fmt;

use super::{build::MirBodyBuilder, model::*};

mod call;
mod cleanup;
mod control_flow;
mod expression;
mod object_values;
mod places;
mod program;
mod statement;

use cleanup::CleanupPlanner;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirLoweringError {
    /// PM17 represents type operations in HIR; PM18 will define their MIR.
    TypeOperationsUnsupported { span: Span },
}

impl fmt::Display for HirLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TypeOperationsUnsupported { .. } => {
                formatter.write_str("type tests and checked narrowing are not lowered to MIR yet")
            }
        }
    }
}

impl std::error::Error for HirLoweringError {}

/// Lowers every currently representable HIR operation into executable MIR.
///
pub fn lower_hir(hir: &HirProgram) -> Result<MirProgram, HirLoweringError> {
    if let Some(span) = first_type_operation(hir) {
        return Err(HirLoweringError::TypeOperationsUnsupported { span });
    }
    let mir = program::lower_program(hir);

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    Ok(mir)
}

fn first_type_operation(program: &HirProgram) -> Option<Span> {
    program
        .definitions
        .iter()
        .find_map(|definition| block_type_operation(&definition.body))
        .or_else(|| {
            program.class_definitions.iter().find_map(|class| {
                std::iter::once(&class.initializer)
                    .chain(class.copy_constructor.iter())
                    .chain(class.copy_assignment.iter())
                    .chain(class.destructor.iter())
                    .chain(class.methods.iter())
                    .find_map(|definition| block_type_operation(&definition.body))
            })
        })
}

fn block_type_operation(block: &HirBlock) -> Option<Span> {
    block.statements.iter().find_map(statement_type_operation)
}

fn statement_type_operation(statement: &HirStatement) -> Option<Span> {
    match statement {
        HirStatement::Narrowing(narrowing) => Some(narrowing.span),
        HirStatement::BaseInitialization(initialization) => {
            arguments_type_operation(&initialization.arguments)
        }
        HirStatement::Local(local) => match &local.initializer {
            HirLocalInitializer::Value(expression) => expression_type_operation(expression),
            HirLocalInitializer::Object(initialization) => {
                producer_type_operation(&initialization.producer)
            }
            HirLocalInitializer::Copy(copy) => source_type_operation(&copy.source),
        },
        HirStatement::Return(statement) => statement.value.as_ref().and_then(|value| match value {
            HirReturnValue::Scalar(expression) => expression_type_operation(expression),
            HirReturnValue::Object(HirObjectReturn::Copy { source, .. }) => {
                source_type_operation(source)
            }
            HirReturnValue::Object(HirObjectReturn::Construct { construction, .. }) => {
                arguments_type_operation(&construction.arguments)
            }
        }),
        HirStatement::Call(statement) => expression_type_operation(&statement.call),
        HirStatement::Conditional(conditional) => conditional
            .arms
            .iter()
            .find_map(|arm| {
                expression_type_operation(&arm.condition)
                    .or_else(|| block_type_operation(&arm.body))
            })
            .or_else(|| {
                conditional
                    .else_block
                    .as_ref()
                    .and_then(block_type_operation)
            }),
        HirStatement::Block(block) => block_type_operation(block),
        HirStatement::FieldAssignment(assignment) => expression_type_operation(&assignment.value),
        HirStatement::FieldConstruction(construction) => {
            arguments_type_operation(&construction.construction.arguments)
        }
        HirStatement::FieldCopyConstruction(construction) => {
            source_type_operation(&construction.source)
        }
        HirStatement::FieldCopyAssignment(assignment) => source_type_operation(&assignment.source),
        HirStatement::CopyAssignment(assignment) => source_type_operation(&assignment.source),
    }
}

fn expression_type_operation(expression: &HirExpression) -> Option<Span> {
    match &expression.kind {
        HirExpressionKind::TypeTest(_) => Some(expression.span),
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            expression_type_operation(operand)
        }
        HirExpressionKind::Binary { left, right, .. } => {
            expression_type_operation(left).or_else(|| expression_type_operation(right))
        }
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::MethodCall { arguments, .. }
        | HirExpressionKind::InterfaceCall { arguments, .. } => arguments_type_operation(arguments),
        HirExpressionKind::Binding(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_)
        | HirExpressionKind::FieldRead(_) => None,
    }
}

fn arguments_type_operation(arguments: &[HirCallArgument]) -> Option<Span> {
    arguments.iter().find_map(|argument| match argument {
        HirCallArgument::Value(expression) => expression_type_operation(expression),
        HirCallArgument::Copy(copy) => source_type_operation(&copy.source),
        HirCallArgument::Place(_) | HirCallArgument::View(_) => None,
    })
}

fn source_type_operation(source: &HirObjectSource) -> Option<Span> {
    match source {
        HirObjectSource::Produced(producer) => producer_type_operation(producer),
        HirObjectSource::Slice(slice) => source_type_operation(&slice.source),
        HirObjectSource::Place(_) => None,
    }
}

fn producer_type_operation(producer: &HirObjectProducer) -> Option<Span> {
    match producer {
        HirObjectProducer::Construct(construction) => {
            arguments_type_operation(&construction.arguments)
        }
        HirObjectProducer::Call(call) => arguments_type_operation(&call.arguments),
    }
}

fn lower_selected_copy_operation<I>(
    operation: HirSelectedCopyOperation<I>,
) -> MirSelectedCopyOperation<I> {
    match operation {
        HirSelectedCopyOperation::User(id) => MirSelectedCopyOperation::User(id),
        HirSelectedCopyOperation::Synthesized(class) => {
            MirSelectedCopyOperation::Synthesized(class)
        }
    }
}

struct BodyLoweringInput<'hir> {
    callable: CallableId,
    parameters: &'hir [HirParameter],
    locals: &'hir [HirLocal],
    source_body: &'hir HirBlock,
    return_type: Type,
    receiver_class: Option<ClassId>,
}

struct LoweredBody {
    return_storage: Option<StorageId>,
    receiver: Option<StorageId>,
    parameters: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBody,
}

struct BodyLowerer<'hir> {
    input: BodyLoweringInput<'hir>,
    return_storage: Option<StorageId>,
    receiver_storage: Option<StorageId>,
    parameter_storage: Vec<StorageId>,
    local_storage: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBodyBuilder,
    cleanup: CleanupPlanner,
    full_expression_temporaries: Vec<MirCleanup>,
}

impl<'hir> BodyLowerer<'hir> {
    fn lower(input: BodyLoweringInput<'hir>) -> LoweredBody {
        let mut lowerer = Self {
            parameter_storage: Vec::with_capacity(input.parameters.len()),
            local_storage: Vec::with_capacity(input.locals.len()),
            storage: Vec::with_capacity(
                input.parameters.len()
                    + input.locals.len()
                    + usize::from(input.receiver_class.is_some()),
            ),
            values: Vec::new(),
            body: MirBodyBuilder::new(input.callable, input.source_body.span),
            cleanup: CleanupPlanner::new(),
            full_expression_temporaries: Vec::new(),
            return_storage: None,
            receiver_storage: None,
            input,
        };
        lowerer.allocate_storage();
        lowerer.cleanup.enter_scope();
        for (parameter, storage) in lowerer
            .input
            .parameters
            .iter()
            .zip(&lowerer.parameter_storage)
        {
            if let (HirParameterMode::Value, Type::Class(class)) = (parameter.mode, parameter.ty) {
                lowerer.cleanup.register_owned(*storage, class);
            }
        }
        lowerer.lower_block(lowerer.input.source_body);
        if !lowerer.body.is_current_terminated() && lowerer.input.return_type == Type::Unit {
            lowerer.emit_cleanups(
                lowerer
                    .cleanup
                    .for_current_scope(lowerer.input.source_body.span),
            );
            lowerer.terminate(MirTerminator::Return {
                value: None,
                span: lowerer.input.source_body.span,
            });
        }
        assert!(
            lowerer.body.is_current_terminated(),
            "type-checked callable must lower to a terminated entry block"
        );
        lowerer.cleanup.leave_scope();
        LoweredBody {
            return_storage: lowerer.return_storage,
            receiver: lowerer.receiver_storage,
            parameters: lowerer.parameter_storage,
            storage: lowerer.storage,
            values: lowerer.values,
            body: lowerer.body.finish(),
        }
    }

    fn allocate_storage(&mut self) {
        if let Type::Class(class) = self.input.return_type {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.return_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: None,
                name: "return".to_owned(),
                kind: MirStorageKind::Return,
                ty: MirType::Class(class),
                span: self.input.source_body.span,
            });
        }
        if let Some(class) = self.input.receiver_class {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.receiver_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: Some(BindingId::Receiver(self.input.callable)),
                name: "self".to_owned(),
                kind: MirStorageKind::Receiver,
                ty: MirType::Class(class),
                span: self.input.source_body.span,
            });
        }
        for parameter in self.input.parameters {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.parameter_storage.push(id);
            self.storage.push(MirStorage {
                id,
                source: Some(BindingId::Parameter(parameter.id)),
                name: parameter.name.clone(),
                kind: match parameter.mode {
                    HirParameterMode::Value => MirStorageKind::Parameter,
                    HirParameterMode::ReadOnlyAlias => {
                        MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly)
                    }
                    HirParameterMode::MutableAlias => {
                        MirStorageKind::AliasParameter(MirAliasAccess::Mutable)
                    }
                },
                ty: lower_type(parameter.ty),
                span: parameter.span,
            });
        }
        for local in self.input.locals {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.local_storage.push(id);
            self.storage.push(MirStorage {
                id,
                source: Some(BindingId::Local(local.id)),
                name: local.name.clone(),
                kind: MirStorageKind::Local,
                ty: lower_type(local.ty),
                span: local.span,
            });
        }
    }

    fn emit(&mut self, instruction: MirInstruction) {
        self.body
            .push_instruction(instruction)
            .expect("HIR lowering must not emit after a terminator");
    }

    fn emit_cleanups(&mut self, cleanups: Vec<MirCleanup>) {
        for cleanup in cleanups {
            self.emit(MirInstruction::Cleanup(cleanup));
        }
    }

    fn terminate(&mut self, terminator: MirTerminator) {
        self.body
            .terminate(terminator)
            .expect("HIR lowering must terminate each block exactly once");
    }

    fn new_value(&mut self, ty: MirType, span: crate::source::Span) -> ValueId {
        assert!(
            ty.is_scalar_value(),
            "typed HIR lowering must not materialize a non-scalar MIR value"
        );
        let result = ValueId::new(self.input.callable, self.values.len());
        self.values.push(MirValue {
            id: result,
            ty,
            span,
        });
        result
    }
}

const fn lower_type(ty: Type) -> MirType {
    match ty {
        Type::I64 => MirType::I64,
        Type::U64 => MirType::U64,
        Type::U8 => MirType::U8,
        Type::F64 => MirType::F64,
        Type::Bool => MirType::Bool,
        Type::Unit => MirType::Unit,
        Type::Obj => MirType::Obj,
        Type::Class(class) => MirType::Class(class),
        Type::Interface(interface) => MirType::Interface(interface),
    }
}

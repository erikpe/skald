//! Deterministic lowering from typed HIR to MIR.

use crate::{
    hir::{
        HirBlock, HirLocal, HirParameter, HirParameterMode, HirProgram, HirSelectedCopyOperation,
        Type,
    },
    identity::{BindingId, CallableId, ClassId},
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
    StaticInheritanceNotRepresentable { class: ClassId },
    StaticObjectViewsNotRepresentable,
}

impl fmt::Display for HirLoweringError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StaticInheritanceNotRepresentable { class } => write!(
                formatter,
                "HIR for {class} uses static inheritance, which is not representable in MIR yet"
            ),
            Self::StaticObjectViewsNotRepresentable => formatter
                .write_str("HIR uses static object views, which are not representable in MIR yet"),
        }
    }
}

impl std::error::Error for HirLoweringError {}

/// Lowers every currently representable HIR operation into executable MIR.
///
/// Static inheritance is already meaningful in HIR but does not yet have MIR
/// places or lifecycle instructions, so that valid staged feature returns a
/// structured error instead of being discarded or reaching a backend panic.
pub fn lower_hir(hir: &HirProgram) -> Result<MirProgram, HirLoweringError> {
    if let Some(class) = hir
        .classes
        .iter()
        .find(|class| class.direct_base.is_some())
        .map(|class| class.id)
    {
        return Err(HirLoweringError::StaticInheritanceNotRepresentable { class });
    }
    if hir_uses_obj_views(hir) {
        return Err(HirLoweringError::StaticObjectViewsNotRepresentable);
    }
    let mir = program::lower_program(hir);

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    Ok(mir)
}

fn hir_uses_obj_views(hir: &HirProgram) -> bool {
    hir.declarations
        .iter()
        .flat_map(|declaration| {
            declaration
                .parameters
                .iter()
                .map(|parameter| parameter.ty)
                .chain(std::iter::once(declaration.return_type))
        })
        .chain(hir.classes.iter().flat_map(|class| {
            class
                .methods
                .iter()
                .flat_map(|method| {
                    method
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty)
                        .chain(std::iter::once(method.return_type))
                })
                .chain(
                    class
                        .initializer
                        .parameters
                        .iter()
                        .map(|parameter| parameter.ty),
                )
        }))
        .any(|ty| ty == Type::Obj)
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
        Type::Obj => panic!("Obj views are rejected before MIR type lowering"),
        Type::Class(class) => MirType::Class(class),
    }
}

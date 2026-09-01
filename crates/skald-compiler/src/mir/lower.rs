//! Deterministic lowering from typed HIR to MIR.

use super::{build::MirBodyBuilder, model::*};
use crate::{
    hir::{
        HirArrayPlace, HirBlock, HirExpression, HirFieldPlace, HirFieldWriteAuthorization,
        HirLocal, HirOptionalStorage, HirParameter, HirParameterMode, HirProgram,
        HirSelectedCopyOperation, Type,
    },
    identity::{BindingId, CallableId, ClassId, OptionalTypeId},
};

mod array;
mod call;
mod cleanup;
mod control_effect;
mod control_flow;
mod expression;
mod full_expression;
mod integer_division;
mod io;
mod iteration;
mod logical;
#[allow(dead_code)]
mod loop_context;
mod loop_flow;
mod object_values;
mod optional;
mod optional_box;
mod optional_box_types;
mod optional_types;
mod places;
mod primitive;
mod program;
mod range_iteration;
mod shared;
mod shift;
mod statement;
mod static_initializer;
mod type_operations;

use cleanup::CleanupPlanner;
use full_expression::FullExpressionTracker;
use loop_context::LoopContextStack;

/// Lowers a fully type-checked HIR program to MIR.
pub fn lower_hir(hir: &HirProgram) -> MirProgram {
    let preliminary = lower_preliminary_hir(hir);
    let mir = preliminary
        .try_into_final()
        .unwrap_or_else(|_| panic!("typed static initializers require lifecycle planning"));

    #[cfg(debug_assertions)]
    if let Err(errors) = super::verify_mir(&mir) {
        panic!("HIR lowering produced invalid MIR:\n{errors}");
    }
    mir
}

/// Lowers complete typed HIR into the closed-world product consumed by static
/// lifecycle analysis. This product cannot be passed directly to a backend.
pub fn lower_preliminary_hir(hir: &HirProgram) -> PreliminaryMirProgram {
    let program = program::lower_program(hir);
    let (static_fields, static_initializers) =
        static_initializer::lower_static_initializers(hir, program.string_language_item);
    let preliminary = PreliminaryMirProgram::new(program, static_fields, static_initializers);

    #[cfg(debug_assertions)]
    if let Err(errors) = super::check_preliminary_mir(&preliminary) {
        panic!("HIR lowering produced invalid preliminary MIR:\n{errors}");
    }
    preliminary
}

fn invalid_array_hir() -> ! {
    unreachable!("typed array HIR violates the lowering contract")
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

fn lower_cell_write_authorization(place: &HirFieldPlace) -> Option<MirCellWriteAuthorization> {
    (place.write_authorization == Some(HirFieldWriteAuthorization::DeclaringClassCell))
        .then_some(MirCellWriteAuthorization { field: place.field })
}

fn lower_final_write_authorization(place: &HirFieldPlace) -> Option<MirFinalWriteAuthorization> {
    match place.write_authorization {
        Some(HirFieldWriteAuthorization::DeclaringClassFinalAssignment(operation)) => {
            Some(MirFinalWriteAuthorization {
                field: place.field,
                operation,
            })
        }
        _ => None,
    }
}

fn lower_optional_cell_write_authorization(
    storage: &HirOptionalStorage,
) -> Option<MirCellWriteAuthorization> {
    match storage {
        HirOptionalStorage::Field(place) => lower_cell_write_authorization(place),
        HirOptionalStorage::Binding(_)
        | HirOptionalStorage::Static(_)
        | HirOptionalStorage::ArrayElement(_)
        | HirOptionalStorage::SharedPointee(_) => None,
    }
}

fn lower_optional_final_write_authorization(
    storage: &HirOptionalStorage,
) -> Option<MirFinalWriteAuthorization> {
    match storage {
        HirOptionalStorage::Field(place) => lower_final_write_authorization(place),
        HirOptionalStorage::Binding(_)
        | HirOptionalStorage::Static(_)
        | HirOptionalStorage::ArrayElement(_)
        | HirOptionalStorage::SharedPointee(_) => None,
    }
}

fn lower_array_cell_write_authorization(
    place: &HirArrayPlace,
) -> Option<MirCellWriteAuthorization> {
    match place {
        HirArrayPlace::Field { place, .. } => lower_cell_write_authorization(place),
        HirArrayPlace::Binding { .. }
        | HirArrayPlace::Static { .. }
        | HirArrayPlace::Element(_) => None,
    }
}

fn lower_array_final_write_authorization(
    place: &HirArrayPlace,
) -> Option<MirFinalWriteAuthorization> {
    match place {
        HirArrayPlace::Field { place, .. } => lower_final_write_authorization(place),
        HirArrayPlace::Binding { .. }
        | HirArrayPlace::Static { .. }
        | HirArrayPlace::Element(_) => None,
    }
}

struct BodyLoweringInput<'hir> {
    callable: CallableId,
    parameters: &'hir [HirParameter],
    locals: &'hir [HirLocal],
    source_body: &'hir HirBlock,
    return_type: Type,
    receiver_class: Option<ClassId>,
    string_language_item: Option<MirStringLanguageItem>,
    literal_data: &'hir crate::hir::HirLiteralDataTable,
    array_types: &'hir crate::hir::HirArrayTypeTable,
    optional_types: &'hir crate::hir::HirOptionalTypeTable,
    optional_box_types: &'hir crate::hir::HirOptionalBoxTypeTable,
}

struct LoweredBody {
    return_storage: Option<StorageId>,
    receiver: Option<StorageId>,
    parameters: Vec<StorageId>,
    storage: Vec<MirStorage>,
    values: Vec<MirValue>,
    body: MirBody,
}

#[derive(Clone)]
enum FullExpressionTemporary {
    Inline(MirCleanup),
    Shared(StorageId),
    ClassOptional(crate::mir::MirClassOptionalCleanup),
    OptionalShared(crate::mir::MirOptionalSharedCleanup),
    AggregateOptional(crate::mir::MirAggregateOptionalCleanup),
    Array {
        storage: StorageId,
        array: crate::identity::ArrayTypeId,
    },
    ArrayAnchor(StorageId),
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
    loop_contexts: LoopContextStack,
    full_expression: FullExpressionTracker,
    next_optional_guard: usize,
    active_optional_guards: Vec<ActiveOptionalGuard>,
}

#[derive(Clone)]
enum ActiveOptionalGuard {
    Inline {
        guard: OptionalGuardId,
        source: MirPlace,
        optional: OptionalTypeId,
        payload: MirType,
    },
    Box {
        guard: OptionalGuardId,
        owner: StorageId,
        target: crate::identity::OptionalBoxTypeId,
        layer: usize,
    },
}

impl<'hir> BodyLowerer<'hir> {
    fn lower_type(&self, ty: Type) -> MirType {
        optional_types::lower_type(ty)
    }

    fn lower(input: BodyLoweringInput<'hir>) -> LoweredBody {
        let mut lowerer = Self::new(input);
        lowerer.allocate_storage();
        lowerer.cleanup.enter_scope();
        for (parameter, storage) in lowerer
            .input
            .parameters
            .iter()
            .zip(&lowerer.parameter_storage)
        {
            if parameter.mode == HirParameterMode::Value {
                match parameter.ty {
                    Type::Class(class) => lowerer.cleanup.register_owned(*storage, class),
                    Type::Shared(_) => lowerer.cleanup.register_shared(*storage),
                    Type::Optional(optional) => match lowerer
                        .input
                        .optional_types
                        .get(optional)
                        .expect("typed optional identity must have metadata")
                        .storage
                    {
                        crate::hir::HirOptionalStorageCategory::InlineClass(class) => lowerer
                            .cleanup
                            .register_class_optional(*storage, optional, class),
                        crate::hir::HirOptionalStorageCategory::SharedOwner(target) => {
                            lowerer.cleanup.register_optional_shared(
                                *storage,
                                optional,
                                lower_shared_target(target),
                            )
                        }
                        crate::hir::HirOptionalStorageCategory::Scalar => {}
                        crate::hir::HirOptionalStorageCategory::Nested(_)
                        | crate::hir::HirOptionalStorageCategory::InlineArray(_) => lowerer
                            .cleanup
                            .register_aggregate_optional(*storage, optional),
                    },
                    Type::Array(array) => lowerer.cleanup.register_array(*storage, array),
                    _ => {}
                }
            }
        }
        lowerer.lower_block(lowerer.input.source_body);
        if !lowerer.body.is_current_terminated() && lowerer.input.return_type == Type::Unit {
            lowerer.emit_scope_exit(
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

    fn new(input: BodyLoweringInput<'hir>) -> Self {
        Self {
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
            loop_contexts: LoopContextStack::new(),
            full_expression: FullExpressionTracker::default(),
            next_optional_guard: 0,
            active_optional_guards: Vec::new(),
            return_storage: None,
            receiver_storage: None,
            input,
        }
    }

    fn allocate_storage(&mut self) {
        if let Type::Array(array) = self.input.return_type {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.return_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: None,
                name: "array-return".to_owned(),
                kind: MirStorageKind::Return,
                ty: MirType::Array(array),
                span: self.input.source_body.span,
            });
        } else if let Type::Class(class) = self.input.return_type {
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
        } else if let Type::Shared(target) = self.input.return_type {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.return_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: None,
                name: "shared-return".to_owned(),
                kind: MirStorageKind::Return,
                ty: self.lower_type(Type::Shared(target)),
                span: self.input.source_body.span,
            });
        } else if matches!(self.input.return_type, Type::Optional(_)) {
            let id = StorageId::new(self.input.callable, self.storage.len());
            self.return_storage = Some(id);
            self.storage.push(MirStorage {
                id,
                source: None,
                name: "optional-return".to_owned(),
                kind: MirStorageKind::Return,
                ty: self.lower_type(self.input.return_type),
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
                ty: self.lower_type(parameter.ty),
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
                ty: self.lower_type(local.ty),
                span: local.span,
            });
        }
    }

    fn emit(&mut self, instruction: MirInstruction) {
        self.body
            .push_instruction(instruction)
            .expect("HIR lowering must not emit after a terminator");
    }

    fn emit_scope_exit(&mut self, exit: cleanup::PlannedScopeExit) {
        for action in exit.actions {
            let cleanup = match action {
                cleanup::PlannedScopeAction::Cleanup(cleanup) => cleanup,
                cleanup::PlannedScopeAction::StorageDead(storage) => {
                    self.end_storage_lifetime(storage, exit.span);
                    continue;
                }
            };
            match cleanup {
                cleanup::PlannedCleanup::Inline(cleanup) => {
                    self.emit(MirInstruction::Cleanup(cleanup))
                }
                cleanup::PlannedCleanup::Shared(release) => {
                    self.emit(MirInstruction::SharedRelease(release))
                }
                cleanup::PlannedCleanup::ClassOptional(cleanup) => {
                    self.emit_class_optional_cleanup(cleanup)
                }
                cleanup::PlannedCleanup::OptionalShared(cleanup) => {
                    self.emit(MirInstruction::OptionalSharedCleanup(cleanup))
                }
                cleanup::PlannedCleanup::AggregateOptional(cleanup) => {
                    self.emit_aggregate_optional_cleanup(cleanup)
                }
                cleanup::PlannedCleanup::Array {
                    storage,
                    array,
                    span,
                } => self.emit(MirInstruction::Array(MirArrayInstruction::Release {
                    owner: MirPlace::base(storage),
                    array,
                    span,
                })),
                cleanup::PlannedCleanup::IterationReceiver(resources) => {
                    self.emit_iteration_receiver_resources(resources, exit.span)
                }
            }
        }
    }

    fn begin_storage_lifetime(&mut self, storage: StorageId, span: crate::source::Span) {
        self.emit(MirInstruction::StorageLive(MirStorageLive {
            storage,
            span,
        }));
    }

    fn end_storage_lifetime(&mut self, storage: StorageId, span: crate::source::Span) {
        self.emit(MirInstruction::StorageDead(MirStorageDead {
            storage,
            span,
        }));
    }

    fn track_full_expression_storage(&mut self, storage: StorageId, span: crate::source::Span) {
        self.begin_storage_lifetime(storage, span);
        self.full_expression.register_storage(storage);
    }

    fn extend_storage_beyond_full_expression(&mut self, storage: StorageId) {
        self.full_expression.remove_storage(storage);
    }

    /// Give the current full expression ownership of one selected MIR path.
    fn register_full_expression_condition(
        &mut self,
        condition: MirPathCondition,
    ) -> PathConditionId {
        let id = self.body.register_path_condition(condition.clone());
        self.full_expression.register_condition(condition);
        id
    }

    /// Select the path condition inherited by subsequently completed resources.
    fn select_full_expression_condition(&mut self, condition: Option<PathConditionId>) {
        self.full_expression.select_condition(condition);
    }

    fn current_full_expression_condition(&self) -> Option<PathConditionId> {
        self.full_expression.current_condition()
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

fn lower_shared_target(target: crate::hir::HirSharedTarget) -> MirSharedTarget {
    match target {
        crate::hir::HirSharedTarget::Obj => MirSharedTarget::Obj,
        crate::hir::HirSharedTarget::Class(class) => MirSharedTarget::Class(class),
        crate::hir::HirSharedTarget::Interface(interface) => MirSharedTarget::Interface(interface),
        crate::hir::HirSharedTarget::Array(array) => MirSharedTarget::Array(array),
        crate::hir::HirSharedTarget::OptionalBox(target) => MirSharedTarget::OptionalBox(target),
    }
}

fn lower_array_default_element(
    operation: crate::hir::HirArrayDefaultElement,
) -> MirArrayDefaultElement {
    use crate::hir::HirArrayDefaultElement as H;
    match operation {
        H::Primitive => MirArrayDefaultElement::Primitive,
        H::OptionalAbsent => MirArrayDefaultElement::OptionalAbsent,
        H::Class { class, initializer } => MirArrayDefaultElement::Class { class, initializer },
        H::ArrayEmpty(array) => MirArrayDefaultElement::ArrayEmpty(array),
        H::SharedClass { class, initializer } => {
            MirArrayDefaultElement::SharedClass { class, initializer }
        }
        H::SharedArrayEmpty(array) => MirArrayDefaultElement::SharedArrayEmpty(array),
        H::SharedOptionalBoxAbsent(target) => {
            MirArrayDefaultElement::SharedOptionalBoxAbsent(target)
        }
    }
}

fn lower_array_copy_element(operation: crate::hir::HirArrayCopyElement) -> MirArrayCopyElement {
    use crate::hir::HirArrayCopyElement as H;
    match operation {
        H::Primitive => MirArrayCopyElement::Primitive,
        H::OptionalPrimitive => MirArrayCopyElement::OptionalPrimitive,
        H::Class { class, operation } => MirArrayCopyElement::Class {
            class,
            operation: lower_selected_copy_operation(operation),
        },
        H::OptionalClass { class, operation } => MirArrayCopyElement::OptionalClass {
            class,
            operation: lower_selected_copy_operation(operation),
        },
        H::Array(array) => MirArrayCopyElement::Array(array),
        H::Shared(target) => MirArrayCopyElement::Shared(lower_shared_target(target)),
        H::OptionalShared(target) => {
            MirArrayCopyElement::OptionalShared(lower_shared_target(target))
        }
        H::Optional(optional) => MirArrayCopyElement::Optional(optional),
    }
}

fn lower_array_assign_element(
    operation: crate::hir::HirArrayAssignElement,
) -> MirArrayAssignElement {
    use crate::hir::HirArrayAssignElement as H;
    match operation {
        H::Primitive => MirArrayAssignElement::Primitive,
        H::OptionalPrimitive => MirArrayAssignElement::OptionalPrimitive,
        H::Class { class, operation } => MirArrayAssignElement::Class {
            class,
            operation: lower_selected_copy_operation(operation),
        },
        H::OptionalClass {
            class,
            copy_constructor,
            copy_assignment,
        } => MirArrayAssignElement::OptionalClass {
            class,
            copy_constructor: lower_selected_copy_operation(copy_constructor),
            copy_assignment: lower_selected_copy_operation(copy_assignment),
        },
        H::Array(array) => MirArrayAssignElement::Array(array),
        H::Shared(target) => MirArrayAssignElement::Shared(lower_shared_target(target)),
        H::OptionalShared(target) => {
            MirArrayAssignElement::OptionalShared(lower_shared_target(target))
        }
        H::Optional(optional) => MirArrayAssignElement::Optional(optional),
    }
}

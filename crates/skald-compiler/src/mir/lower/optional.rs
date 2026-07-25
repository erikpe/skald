//! Primitive optional storage and checked-access lowering.

use crate::{
    hir::{HirOptionalPlace, HirOptionalSource, HirPresenceTestKind},
    mir::{
        MirInstruction, MirOptionalAssign, MirOptionalInitialize, MirOptionalSource,
        MirPresenceTestKind, MirPrimitiveType, MirRvalueKind, MirStorage, MirStorageKind,
        MirTerminationReason, MirTerminator, MirType, StorageId,
    },
};

use super::BodyLowerer;

impl BodyLowerer<'_> {
    pub(super) fn lower_optional_initialize(
        &mut self,
        destination: StorageId,
        source: &HirOptionalSource,
        span: crate::source::Span,
    ) {
        let source = self.lower_optional_source(source);
        self.emit(MirInstruction::OptionalInitialize(MirOptionalInitialize {
            destination,
            source,
            span,
        }));
    }

    pub(super) fn lower_optional_assignment(
        &mut self,
        assignment: &crate::hir::HirOptionalAssignment,
    ) {
        let destination = self.storage_for_binding(assignment.destination);
        let source = self.lower_optional_source(&assignment.source);
        self.emit(MirInstruction::OptionalAssign(MirOptionalAssign {
            destination,
            source,
            span: assignment.span,
        }));
    }

    pub(super) fn lower_presence_test(
        &mut self,
        expression: &crate::hir::HirExpression,
        source: HirOptionalPlace,
        kind: HirPresenceTestKind,
    ) -> crate::mir::ValueId {
        let source = self.storage_for_binding(source.binding);
        self.assign(
            MirRvalueKind::OptionalPresence {
                source,
                kind: match kind {
                    HirPresenceTestKind::Some => MirPresenceTestKind::Some,
                    HirPresenceTestKind::None => MirPresenceTestKind::None,
                },
            },
            MirType::Bool,
            expression.span,
        )
    }

    pub(super) fn lower_optional_unwrap(
        &mut self,
        expression: &crate::hir::HirExpression,
        source: HirOptionalPlace,
    ) -> crate::mir::ValueId {
        let source_storage = self.storage_for_binding(source.binding);
        let payload = lower_primitive_type(source.payload);
        let destination = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id: destination,
            source: None,
            name: format!("unwrap{}", destination.index()),
            kind: MirStorageKind::OptionalUnwrap,
            ty: payload.payload_type(),
            span: expression.span,
        });

        let success_target = self.body.allocate_block(expression.span);
        let failure_target = self.body.allocate_block(expression.span);
        self.terminate(MirTerminator::OptionalUnwrap {
            source: source_storage,
            destination,
            success_target,
            failure_target,
            span: expression.span,
        });

        self.body
            .select_block(failure_target)
            .expect("allocated optional failure block must be selectable");
        self.terminate(MirTerminator::Terminate {
            reason: MirTerminationReason::OptionalAccessFailure,
            span: expression.span,
        });

        self.body
            .select_block(success_target)
            .expect("allocated optional success block must be selectable");
        self.assign(
            MirRvalueKind::Load(destination.into()),
            payload.payload_type(),
            expression.span,
        )
    }

    fn lower_optional_source(&mut self, source: &HirOptionalSource) -> MirOptionalSource {
        match source {
            HirOptionalSource::Absent { .. } => MirOptionalSource::Absent,
            HirOptionalSource::Present(expression) => MirOptionalSource::Present(
                self.lower_expression(expression)
                    .expect("typed primitive optional payload must produce a scalar value"),
            ),
            HirOptionalSource::Copy(place) => {
                MirOptionalSource::Copy(self.storage_for_binding(place.binding))
            }
        }
    }
}

pub(super) const fn lower_primitive_type(
    payload: crate::hir::HirPrimitiveType,
) -> MirPrimitiveType {
    match payload {
        crate::hir::HirPrimitiveType::I64 => MirPrimitiveType::I64,
        crate::hir::HirPrimitiveType::U64 => MirPrimitiveType::U64,
        crate::hir::HirPrimitiveType::U8 => MirPrimitiveType::U8,
        crate::hir::HirPrimitiveType::F64 => MirPrimitiveType::F64,
        crate::hir::HirPrimitiveType::Bool => MirPrimitiveType::Bool,
    }
}

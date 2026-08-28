//! Structured protocol and immediate-range iteration plans retained until MIR lowering.

use crate::{
    identity::{
        ArrayTypeId, ClassId, CopyConstructorId, InterfaceId, InterfaceRequirementId, LocalId,
        LoopId, OptionalTypeId,
    },
    source::Span,
};

use super::{
    HirAccess, HirArrayCopyElement, HirBinaryOperation, HirBlock, HirCanonicalRangeOrigin,
    HirControlEffects, HirExpression, HirIntegerType, HirObjectView, HirOptionalCopyPlan,
    HirOptionalDestructionPlan, HirOptionalPresenceTestPlan, HirOptionalUnwrapPlan,
    HirPrimitiveComparison, HirSelectedCopyOperation, HirSharedTarget, HirViewTarget, Type,
};

/// One completely selected and typed `for-in` statement.
///
/// The receiver is deliberately stored once. Both call plans refer to the
/// same loop-duration receiver implicitly, preventing later phases from
/// accidentally evaluating or acquiring the iterable twice.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirForIn {
    pub loop_id: LoopId,
    pub binding: LocalId,
    pub plan: HirForInPlan,
    pub body: HirBlock,
    pub effects: HirControlEffects,
    pub spans: HirIterationSpans,
}

impl HirForIn {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        loop_id: LoopId,
        binding: LocalId,
        protocol: HirIterationProtocol,
        receiver: HirIterationReceiver,
        state: HirIterationStatePlan,
        result: HirIterationResultPlan,
        item: HirIterationItemPlan,
        body: HirBlock,
        spans: HirIterationSpans,
    ) -> Self {
        assert_eq!(binding.callable(), loop_id.callable());
        assert_eq!(protocol.iter_state.interface(), protocol.interface);
        assert_eq!(protocol.iter_next.interface(), protocol.interface);
        assert_eq!(
            receiver.carrier.target(),
            HirViewTarget::Interface(protocol.interface)
        );
        assert_eq!(receiver.carrier.access(), HirAccess::ReadOnly);
        assert_eq!(state.value.ty, protocol.state);
        assert_eq!(state.initialize.receiver_access, HirAccess::ReadOnly);
        assert_eq!(state.initialize.result, protocol.state);
        assert_eq!(state.initialize.target.interface, protocol.interface);
        assert_eq!(state.initialize.target.requirement, protocol.iter_state);
        assert_eq!(state.advance.result, Type::Optional(protocol.result));
        assert_eq!(state.advance.target.interface, protocol.interface);
        assert_eq!(state.advance.target.requirement, protocol.iter_next);
        assert_eq!(state.advance.receiver_access, HirAccess::ReadOnly);
        assert_eq!(state.advance.state_alias.ty, protocol.state);
        assert_eq!(state.advance.state_alias.access, HirAccess::Mutable);
        assert_eq!(result.optional, protocol.result);
        assert_eq!(result.payload, protocol.item);
        assert_eq!(item.binding, binding);
        assert_eq!(item.value.ty, protocol.item);
        assert!(item.value.copy.is_some());
        assert_eq!(item.access, HirAccess::ReadOnly);

        let effects = body.effects.clone().through_loop(loop_id);
        Self {
            loop_id,
            binding,
            plan: HirForInPlan::Protocol(Box::new(HirProtocolIterationPlan {
                protocol,
                receiver,
                state,
                result,
                item,
            })),
            body,
            effects,
            spans,
        }
    }

    pub fn new_primitive_range(
        loop_id: LoopId,
        binding: LocalId,
        plan: HirPrimitiveRangeIterationPlan,
        body: HirBlock,
        spans: HirIterationSpans,
    ) -> Self {
        assert_eq!(binding.callable(), loop_id.callable());
        assert_eq!(plan.item.binding, binding);
        assert_eq!(plan.item.access, HirAccess::ReadOnly);
        assert_eq!(plan.item.value.ty, plan.integer.operand_type());
        assert_eq!(plan.item.value.copy, Some(HirIterationValueCopy::Trivial));
        assert_eq!(
            plan.item.value.destruction,
            HirIterationValueDestruction::Trivial
        );
        plan.assert_valid();

        let effects = body.effects.clone().through_loop(loop_id);
        Self {
            loop_id,
            binding,
            plan: HirForInPlan::PrimitiveRange(Box::new(plan)),
            body,
            effects,
            spans,
        }
    }

    pub const fn protocol_plan(&self) -> Option<&HirProtocolIterationPlan> {
        match &self.plan {
            HirForInPlan::Protocol(plan) => Some(plan),
            HirForInPlan::PrimitiveRange(_) => None,
        }
    }

    pub fn protocol_plan_mut(&mut self) -> Option<&mut HirProtocolIterationPlan> {
        match &mut self.plan {
            HirForInPlan::Protocol(plan) => Some(plan),
            HirForInPlan::PrimitiveRange(_) => None,
        }
    }

    pub const fn primitive_range_plan(&self) -> Option<&HirPrimitiveRangeIterationPlan> {
        match &self.plan {
            HirForInPlan::Protocol(_) => None,
            HirForInPlan::PrimitiveRange(plan) => Some(plan),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirForInPlan {
    Protocol(Box<HirProtocolIterationPlan>),
    PrimitiveRange(Box<HirPrimitiveRangeIterationPlan>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProtocolIterationPlan {
    pub protocol: HirIterationProtocol,
    pub receiver: HirIterationReceiver,
    pub state: HirIterationStatePlan,
    pub result: HirIterationResultPlan,
    pub item: HirIterationItemPlan,
}

/// One immediate canonical integer range lowered without materializing the
/// ordinary `Range<T>` value or its general-iteration protocol traffic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirPrimitiveRangeIterationPlan {
    pub origin: HirCanonicalRangeOrigin,
    pub lower: HirExpression,
    pub upper: HirExpression,
    pub integer: HirIntegerType,
    pub comparison: HirPrimitiveComparison,
    pub increment: HirBinaryOperation,
    pub item: HirIterationItemPlan,
}

impl HirPrimitiveRangeIterationPlan {
    fn assert_valid(&self) {
        let ty = self.integer.operand_type();
        assert_eq!(self.origin.endpoint_type, ty);
        assert_eq!(self.lower.ty, ty);
        assert_eq!(self.upper.ty, ty);
        assert_eq!(
            self.comparison,
            HirPrimitiveComparison {
                predicate: super::HirComparisonPredicate::LessThan,
                operand: super::HirComparisonOperand::Integer(self.integer),
            }
        );
        assert_eq!(self.increment, range_increment(self.integer));
        assert_eq!(
            self.origin.ordering.realization,
            super::HirRangeProtocolRealization::PrimitiveIntrinsic(ty)
        );
        assert_eq!(
            self.origin.successor.realization,
            super::HirRangeProtocolRealization::PrimitiveIntrinsic(ty)
        );
    }
}

const fn range_increment(integer: HirIntegerType) -> HirBinaryOperation {
    match integer {
        HirIntegerType::I64 => HirBinaryOperation::AddI64,
        HirIntegerType::U64 => HirBinaryOperation::AddU64,
        HirIntegerType::U8 => HirBinaryOperation::AddU8,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationProtocol {
    pub interface: InterfaceId,
    pub iter_state: InterfaceRequirementId,
    pub iter_next: InterfaceRequirementId,
    pub item: Type,
    pub state: Type,
    pub result: OptionalTypeId,
}

/// The single acquisition that remains valid from before `iter_state` until
/// after state cleanup on every outer exit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIterationReceiver {
    pub iterable: Type,
    pub carrier: HirIterationReceiverCarrier,
    pub lifetime: HirIterationReceiverLifetime,
}

/// The non-owning receiver acquired once before an iteration starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HirIterationReceiverCarrier {
    View(HirObjectView),
    Checked(Box<super::HirCheckedObjectView>),
}

impl HirIterationReceiverCarrier {
    pub const fn target(&self) -> HirViewTarget {
        match self {
            Self::View(view) => view.target,
            Self::Checked(view) => view.consumer_target,
        }
    }

    pub const fn access(&self) -> HirAccess {
        match self {
            Self::View(view) => view.access,
            Self::Checked(view) => view.consumer_access,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirIterationReceiverLifetime {
    LoopDuration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirIterationStatePlan {
    pub value: HirIterationStoredValuePlan,
    pub initialize: HirIterationStateCallPlan,
    pub advance: HirIterationNextCallPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationStateCallPlan {
    pub target: HirIterationCallTarget,
    pub receiver_access: HirAccess,
    pub result: Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationNextCallPlan {
    pub target: HirIterationCallTarget,
    pub receiver_access: HirAccess,
    pub state_alias: HirIterationStateAlias,
    pub result: Type,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationCallTarget {
    pub interface: InterfaceId,
    pub requirement: InterfaceRequirementId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationStateAlias {
    pub ty: Type,
    pub access: HirAccess,
}

/// Canonical outer-optional operations. `payload` is exactly one layer below
/// `optional`, so an optional item remains optional rather than becoming the
/// termination sentinel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationResultPlan {
    pub optional: OptionalTypeId,
    pub payload: Type,
    pub presence: HirOptionalPresenceTestPlan,
    pub unwrap: HirOptionalUnwrapPlan,
    pub destruction: HirOptionalDestructionPlan,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationItemPlan {
    pub binding: LocalId,
    pub access: HirAccess,
    pub value: HirIterationStoredValuePlan,
}

/// Source-independent stored-value lifecycle selected for a call result or
/// extracted payload. MIR supplies the hidden destination and concrete source.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationStoredValuePlan {
    pub ty: Type,
    /// Copying is required when a yielded payload becomes an independent item.
    /// State initialization adopts the call result directly and therefore does
    /// not require this capability.
    pub copy: Option<HirIterationValueCopy>,
    pub destruction: HirIterationValueDestruction,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirIterationValueCopy {
    Trivial,
    Class {
        class: ClassId,
        operation: HirSelectedCopyOperation<CopyConstructorId>,
    },
    Array {
        array: ArrayTypeId,
        operation: HirArrayCopyElement,
    },
    Shared(HirSharedTarget),
    Optional {
        optional: OptionalTypeId,
        operation: HirOptionalCopyPlan,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirIterationValueDestruction {
    Trivial,
    Class(ClassId),
    Array(ArrayTypeId),
    Shared(HirSharedTarget),
    Optional {
        optional: OptionalTypeId,
        plan: HirOptionalDestructionPlan,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirIterationSpans {
    pub for_span: Span,
    pub binding_span: Span,
    pub annotation_span: Option<Span>,
    pub in_span: Span,
    pub iterable_span: Span,
    pub span: Span,
}

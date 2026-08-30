//! Compact semantic fixtures shared by reachability contract tests.

use crate::{
    identity::{
        ArrayTypeId, CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId,
        FunctionId, FunctionTypeId, InitializerId, InterfaceId, InterfaceRequirementId,
        LiteralDataId, MethodId, OptionalBoxTypeId, OptionalTypeId, StaticFieldId,
        StaticInitializerId, VirtualFamilyId,
    },
    mir::{MirArrayLifecycleOperation, MirClassLifecycleOperation, MirExecutionNode},
    source::{SourceDatabase, Span},
};

use super::MirRuntimeEntity;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ReachabilityIdentityFixture {
    pub(super) class: ClassId,
    pub(super) static_field: StaticFieldId,
    pub(super) ordinary_function: CallableId,
    pub(super) static_initializer: CallableId,
    pub(super) initializer: CallableId,
    pub(super) copy_constructor: CallableId,
    pub(super) copy_assignment: CallableId,
    pub(super) destructor: CallableId,
    pub(super) direct_method: CallableId,
    pub(super) virtual_method: CallableId,
    pub(super) interface_method: CallableId,
    pub(super) function_value_target: CallableId,
    pub(super) virtual_family: VirtualFamilyId,
    pub(super) interface_requirement: InterfaceRequirementId,
    pub(super) function_type: FunctionTypeId,
    pub(super) array: ArrayTypeId,
    pub(super) optional: OptionalTypeId,
    pub(super) optional_box: OptionalBoxTypeId,
    pub(super) literal: LiteralDataId,
    pub(super) span: Span,
}

impl ReachabilityIdentityFixture {
    pub(super) fn callable_nodes(&self) -> Vec<MirExecutionNode> {
        [
            self.ordinary_function,
            self.static_initializer,
            self.initializer,
            self.copy_constructor,
            self.copy_assignment,
            self.destructor,
            self.direct_method,
            self.virtual_method,
            self.interface_method,
            self.function_value_target,
        ]
        .into_iter()
        .map(MirExecutionNode::callable)
        .collect()
    }

    pub(super) fn lifecycle_nodes(&self) -> Vec<MirExecutionNode> {
        [
            MirExecutionNode::class(self.class, MirClassLifecycleOperation::CopyConstructor),
            MirExecutionNode::class(self.class, MirClassLifecycleOperation::CopyAssignment),
            MirExecutionNode::class(self.class, MirClassLifecycleOperation::CompleteFinalizer),
            MirExecutionNode::array(self.array, MirArrayLifecycleOperation::Default),
            MirExecutionNode::array(self.array, MirArrayLifecycleOperation::Copy),
            MirExecutionNode::array(self.array, MirArrayLifecycleOperation::Assignment),
            MirExecutionNode::array(self.array, MirArrayLifecycleOperation::Destruction),
        ]
        .into_iter()
        .collect()
    }

    pub(super) fn runtime_entities(&self) -> Vec<MirRuntimeEntity> {
        vec![
            MirRuntimeEntity::ClassDispatch(self.class),
            MirRuntimeEntity::VirtualFamily(self.virtual_family),
            MirRuntimeEntity::InterfaceRequirement(self.interface_requirement),
            MirRuntimeEntity::FunctionType(self.function_type),
            MirRuntimeEntity::ArrayLifecycle(self.array),
            MirRuntimeEntity::OptionalLifecycle(self.optional),
            MirRuntimeEntity::OptionalBoxLayout(self.optional_box),
            MirRuntimeEntity::StaticStorage(self.static_field),
            MirRuntimeEntity::LiteralBacking(self.literal),
        ]
    }
}

pub(super) fn reachability_identity_fixture() -> ReachabilityIdentityFixture {
    let class = ClassId::new(3);
    let static_field = StaticFieldId::new(class, 1);
    let interface = InterfaceId::new(2);
    let span = fixture_spans()[0];

    ReachabilityIdentityFixture {
        class,
        static_field,
        ordinary_function: FunctionId::new(0).into(),
        static_initializer: StaticInitializerId::from(static_field).into(),
        initializer: InitializerId::new(class, 0).into(),
        copy_constructor: CopyConstructorId::new(class, 0).into(),
        copy_assignment: CopyAssignmentId::new(class, 0).into(),
        destructor: DestructorId::new(class, 0).into(),
        direct_method: MethodId::new(class, 0).into(),
        virtual_method: MethodId::new(class, 1).into(),
        interface_method: MethodId::new(class, 2).into(),
        function_value_target: FunctionId::new(1).into(),
        virtual_family: VirtualFamilyId::new(4),
        interface_requirement: InterfaceRequirementId::new(interface, 1),
        function_type: FunctionTypeId::new(5),
        array: ArrayTypeId::new(6),
        optional: OptionalTypeId::new(7),
        optional_box: OptionalBoxTypeId::new(8),
        literal: LiteralDataId::new(9),
        span,
    }
}

pub(super) fn fixture_spans() -> [Span; 4] {
    let mut sources = SourceDatabase::new();
    let first = sources.add("first.ska", "0123456789");
    let second = sources.add("second.ska", "0123456789");
    [
        Span::empty(first, 1),
        Span::empty(first, 7),
        Span::empty(second, 0),
        Span::empty(second, 3),
    ]
}

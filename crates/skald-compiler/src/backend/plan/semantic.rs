//! Checked semantic layout, lifecycle, object-view and dispatch declarations.
//!
//! These records deliberately contain stable identities and physical facts,
//! but no MIR nodes or legacy target planner types.  Shared lowering can use
//! them without returning to the semantic program or recomputing layout.

use crate::identity::{
    ArrayTypeId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId,
    FunctionTypeId, InitializerId, InterfaceId, InterfaceRequirementId, MethodId,
    OptionalBoxTypeId, OptionalTypeId, VirtualFamilyId, VirtualSlotId,
};

use super::{LayoutId, LirCallableId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SemanticType {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Function(FunctionTypeId),
    Array(ArrayTypeId),
    Class(ClassId),
    Interface(InterfaceId),
    Obj,
    Shared(SharedTarget),
    Optional(OptionalTypeId),
    Unit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(ArrayTypeId),
    OptionalBox(OptionalBoxTypeId),
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ObjectViewTarget {
    Class(ClassId),
    Interface(InterfaceId),
    Obj,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TypeLayoutBinding {
    pub ty: SemanticType,
    pub layout: LayoutId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SharedHeaderLayout {
    pub handle_layout: LayoutId,
    pub owner_count_offset: usize,
    pub dynamic_metadata_offset: usize,
    pub header_size: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SharedAllocationLayout {
    pub byte_count: u64,
    pub payload_offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct BaseLayoutFact {
    pub class: ClassId,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct FieldLayoutFact {
    pub field: FieldId,
    pub ty: SemanticType,
    pub layout: LayoutId,
    pub offset: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum DestructionStepFact {
    UserBody(DestructorId),
    Field(FieldId),
    SharedField(FieldId),
    OptionalSharedField(FieldId),
    OptionalClassField(FieldId),
    OptionalField {
        field: FieldId,
        optional: OptionalTypeId,
    },
    ArrayField(FieldId),
    Base(ClassId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ClassLayoutFact {
    pub class: ClassId,
    pub exact_layout: LayoutId,
    pub complete_layout: LayoutId,
    pub base: Option<BaseLayoutFact>,
    pub fields: Vec<FieldLayoutFact>,
    pub shared_allocation: SharedAllocationLayout,
    pub destruction: Vec<DestructionStepFact>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum OptionalStorageFact {
    Scalar,
    InlineClass(ClassId),
    InlineArray(ArrayTypeId),
    SharedOwner(SharedTarget),
    Nested(OptionalTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct OptionalLayoutFact {
    pub optional: OptionalTypeId,
    pub payload: SemanticType,
    pub storage: OptionalStorageFact,
    pub layout: LayoutId,
    pub payload_layout: LayoutId,
    pub state_offset: Option<usize>,
    pub payload_offset: usize,
    pub nullable_niche: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum SelectedCopy<I> {
    User(I),
    Synthesized(ClassId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArrayDefaultElementFact {
    Primitive,
    OptionalAbsent,
    Class {
        class: ClassId,
        initializer: InitializerId,
    },
    ArrayEmpty(ArrayTypeId),
    SharedClass {
        class: ClassId,
        initializer: InitializerId,
    },
    SharedArrayEmpty(ArrayTypeId),
    SharedOptionalBoxAbsent(OptionalBoxTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArrayCopyElementFact {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: SelectedCopy<CopyConstructorId>,
    },
    OptionalClass {
        class: ClassId,
        operation: SelectedCopy<CopyConstructorId>,
    },
    Array(ArrayTypeId),
    Shared(SharedTarget),
    OptionalShared(SharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArrayAssignElementFact {
    Primitive,
    OptionalPrimitive,
    Class {
        class: ClassId,
        operation: SelectedCopy<CopyAssignmentId>,
    },
    OptionalClass {
        class: ClassId,
        copy_constructor: SelectedCopy<CopyConstructorId>,
        copy_assignment: SelectedCopy<CopyAssignmentId>,
    },
    Array(ArrayTypeId),
    Shared(SharedTarget),
    OptionalShared(SharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ArrayDestroyElementFact {
    Trivial,
    Class(ClassId),
    OptionalClass(ClassId),
    Array(ArrayTypeId),
    Shared(SharedTarget),
    OptionalShared(SharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ArrayLayoutFact {
    pub array: ArrayTypeId,
    pub descriptor_layout: LayoutId,
    pub element: SemanticType,
    pub element_layout: LayoutId,
    pub element_offset: usize,
    pub shared_element_offset: usize,
    pub stride: usize,
    pub maximum_length: u64,
    pub shared_maximum_length: u64,
    pub default: Option<ArrayDefaultElementFact>,
    pub copy: Option<ArrayCopyElementFact>,
    pub assignment: Option<ArrayAssignElementFact>,
    pub destruction: ArrayDestroyElementFact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct OptionalBoxLayoutFact {
    pub optional_box: OptionalBoxTypeId,
    pub exact_optional: Option<OptionalTypeId>,
    pub exact_dynamic_class: Option<ClassId>,
    pub object_view: Option<ObjectViewTarget>,
    pub layer_offsets: Vec<usize>,
    pub payload_offset: Option<usize>,
    pub allocation: Option<SharedAllocationLayout>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum ObjectComponent {
    StaticAddress,
    CompleteAddress,
    DynamicMetadata,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ObjectViewFact {
    pub target: ObjectViewTarget,
    pub components: Vec<ObjectComponent>,
    pub members: Vec<ClassId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct VirtualFamilyFact {
    pub family: VirtualFamilyId,
    pub slot: VirtualSlotId,
    pub root: MethodId,
    pub members: Vec<MethodId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct RequirementImplementationFact {
    pub requirement: InterfaceRequirementId,
    pub method: MethodId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct InterfaceRequirementFact {
    pub requirement: InterfaceRequirementId,
    pub signature: super::SignatureId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct InterfaceFact {
    pub interface: InterfaceId,
    pub requirements: Vec<InterfaceRequirementFact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ConformanceFact {
    pub class: ClassId,
    pub interface: InterfaceId,
    pub implementations: Vec<RequirementImplementationFact>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum MethodSlot {
    Virtual(VirtualFamilyId),
    Interface(InterfaceRequirementId),
    Finalizer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct MethodSlotFact {
    pub slot: MethodSlot,
    pub index: usize,
    pub byte_offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct ClassDispatchFact {
    pub class: ClassId,
    pub targets: Vec<Option<LirCallableId>>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct SemanticFacts {
    pub types: Vec<TypeLayoutBinding>,
    pub shared_header: Option<SharedHeaderLayout>,
    pub classes: Vec<ClassLayoutFact>,
    pub optionals: Vec<OptionalLayoutFact>,
    pub optional_boxes: Vec<OptionalBoxLayoutFact>,
    pub arrays: Vec<ArrayLayoutFact>,
    pub object_views: Vec<ObjectViewFact>,
    pub virtual_families: Vec<VirtualFamilyFact>,
    pub interfaces: Vec<InterfaceFact>,
    pub conformances: Vec<ConformanceFact>,
    pub method_slots: Vec<MethodSlotFact>,
    pub dispatch_tables: Vec<ClassDispatchFact>,
}

#[cfg_attr(not(test), allow(dead_code))]
impl SemanticFacts {
    pub(in crate::backend) fn layout(&self, ty: SemanticType) -> Option<LayoutId> {
        self.types
            .iter()
            .find_map(|binding| (binding.ty == ty).then_some(binding.layout))
    }

    pub(in crate::backend) fn class(&self, id: ClassId) -> Option<&ClassLayoutFact> {
        self.classes.get(id.index()).filter(|fact| fact.class == id)
    }

    pub(in crate::backend) fn field(&self, id: FieldId) -> Option<FieldLayoutFact> {
        self.class(id.class())?
            .fields
            .get(id.index())
            .copied()
            .filter(|fact| fact.field == id)
    }

    pub(in crate::backend) fn optional(&self, id: OptionalTypeId) -> Option<&OptionalLayoutFact> {
        self.optionals
            .get(id.index())
            .filter(|fact| fact.optional == id)
    }

    pub(in crate::backend) fn optional_box(
        &self,
        id: OptionalBoxTypeId,
    ) -> Option<&OptionalBoxLayoutFact> {
        self.optional_boxes
            .get(id.index())
            .filter(|fact| fact.optional_box == id)
    }

    pub(in crate::backend) fn array(&self, id: ArrayTypeId) -> Option<&ArrayLayoutFact> {
        self.arrays.get(id.index()).filter(|fact| fact.array == id)
    }

    pub(in crate::backend) fn object_view(
        &self,
        target: ObjectViewTarget,
    ) -> Option<&ObjectViewFact> {
        self.object_views.iter().find(|fact| fact.target == target)
    }

    pub(in crate::backend) fn virtual_family(
        &self,
        id: VirtualFamilyId,
    ) -> Option<&VirtualFamilyFact> {
        self.virtual_families
            .get(id.index())
            .filter(|fact| fact.family == id)
    }

    pub(in crate::backend) fn interface(&self, id: InterfaceId) -> Option<&InterfaceFact> {
        self.interfaces
            .get(id.index())
            .filter(|fact| fact.interface == id)
    }

    pub(in crate::backend) fn interface_requirement(
        &self,
        id: InterfaceRequirementId,
    ) -> Option<InterfaceRequirementFact> {
        self.interface(id.interface())?
            .requirements
            .get(id.index())
            .copied()
            .filter(|fact| fact.requirement == id)
    }

    pub(in crate::backend) fn conformance(
        &self,
        class: ClassId,
        interface: InterfaceId,
    ) -> Option<&ConformanceFact> {
        self.conformances
            .iter()
            .find(|fact| fact.class == class && fact.interface == interface)
    }

    pub(in crate::backend) fn method_slot(&self, slot: MethodSlot) -> Option<MethodSlotFact> {
        self.method_slots
            .iter()
            .find(|fact| fact.slot == slot)
            .copied()
    }

    pub(in crate::backend) fn dispatch_table(&self, class: ClassId) -> Option<&ClassDispatchFact> {
        self.dispatch_tables
            .get(class.index())
            .filter(|fact| fact.class == class)
    }
}

//! Narrow projection of verified target-independent reachability for targets.

use crate::{
    identity::{
        ArrayTypeId, ClassId, FunctionTypeId, InterfaceRequirementId, LiteralDataId,
        OptionalBoxTypeId, OptionalTypeId, StaticFieldId, VirtualFamilyId,
    },
    passes::reachability::MirRuntimeEntity,
};

use super::BackendInput;

/// Target-independent runtime obligation projected for backend planning.
///
/// This backend-owned vocabulary prevents target implementations from
/// depending on the reachability analysis representation or its pass policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BackendRequiredRuntimeEntity {
    ClassDispatch(ClassId),
    VirtualFamily(VirtualFamilyId),
    InterfaceRequirement(InterfaceRequirementId),
    FunctionType(FunctionTypeId),
    ArrayLifecycle(ArrayTypeId),
    OptionalLifecycle(OptionalTypeId),
    OptionalBoxLayout(OptionalBoxTypeId),
    StaticStorage(StaticFieldId),
    LiteralBacking(LiteralDataId),
}

impl<'input> BackendInput<'input> {
    /// Returns the exact certified static-storage domain in canonical identity
    /// order without exposing lifecycle reports or mutable certificate state.
    pub(crate) fn active_static_fields(self) -> &'input [StaticFieldId] {
        self.program()
            .static_lifecycle
            .as_ref()
            .map_or(&[], |coordinator| {
                coordinator.lifecycle().proof().activation().fields()
            })
    }

    /// Iterates required target-independent runtime entities in canonical
    /// identity order without exposing the analysis product to a backend.
    pub(crate) fn required_runtime_entities(
        self,
    ) -> impl ExactSizeIterator<Item = BackendRequiredRuntimeEntity> + 'input {
        self.verified
            .reachability()
            .runtime_entities()
            .iter()
            .copied()
            .map(BackendRequiredRuntimeEntity::from)
    }

    pub(crate) fn reachable_callables(self) -> &'input [crate::identity::CallableId] {
        self.verified.reachability().reachable_callables()
    }

    /// Returns static slots referenced by any physically retained body, not
    /// only the reachable semantic domain. Complete artifact planning uses the
    /// difference to allocate inert zero storage without promoting lifecycle.
    pub(crate) fn retained_static_fields(
        self,
    ) -> Result<Vec<StaticFieldId>, crate::backend::BackendError> {
        let extraction = crate::passes::reachability::extract_final_dependencies(self.program())
            .map_err(|error| {
                crate::backend::BackendError::new(
                    crate::backend::Target::X86_64SysV,
                    None,
                    format!("cannot project retained static dependencies: {error}"),
                )
            })?;
        let mut fields = extraction
            .static_accesses()
            .iter()
            .map(|access| access.target())
            .collect::<Vec<_>>();
        fields.sort_unstable();
        fields.dedup();
        Ok(fields)
    }

    pub(crate) fn uses_virtual_family(self, family: VirtualFamilyId) -> bool {
        self.verified
            .reachability()
            .used_virtual_families()
            .binary_search(&family)
            .is_ok()
    }

    pub(crate) fn uses_interface_requirement(self, requirement: InterfaceRequirementId) -> bool {
        self.verified
            .reachability()
            .used_interface_requirements()
            .binary_search(&requirement)
            .is_ok()
    }
}

impl From<MirRuntimeEntity> for BackendRequiredRuntimeEntity {
    fn from(entity: MirRuntimeEntity) -> Self {
        match entity {
            MirRuntimeEntity::ClassDispatch(class) => Self::ClassDispatch(class),
            MirRuntimeEntity::VirtualFamily(family) => Self::VirtualFamily(family),
            MirRuntimeEntity::InterfaceRequirement(requirement) => {
                Self::InterfaceRequirement(requirement)
            }
            MirRuntimeEntity::FunctionType(function_type) => Self::FunctionType(function_type),
            MirRuntimeEntity::ArrayLifecycle(array) => Self::ArrayLifecycle(array),
            MirRuntimeEntity::OptionalLifecycle(optional) => Self::OptionalLifecycle(optional),
            MirRuntimeEntity::OptionalBoxLayout(box_type) => Self::OptionalBoxLayout(box_type),
            MirRuntimeEntity::StaticStorage(field) => Self::StaticStorage(field),
            MirRuntimeEntity::LiteralBacking(data) => Self::LiteralBacking(data),
        }
    }
}

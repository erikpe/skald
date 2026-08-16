//! Shallow access granted when complete field or static storage is aliased.

use crate::{
    hir::{HirAccess, HirFieldPlace},
    identity::{FieldId, StaticFieldId},
    resolve::ResolvedExpression,
};

use super::super::function::CallableChecker;

impl CallableChecker<'_, '_> {
    pub(in crate::typeck) fn rebinding_field_alias_access(
        &self,
        field: FieldId,
        receiver_access: HirAccess,
    ) -> HirAccess {
        if self
            .program
            .field(field)
            .expect("resolved field use must reference a declaration")
            .final_span
            .is_some()
        {
            HirAccess::ReadOnly
        } else {
            receiver_access
        }
    }

    pub(in crate::typeck) fn rebinding_field_place_alias_access(
        &self,
        place: &HirFieldPlace,
    ) -> HirAccess {
        self.rebinding_field_alias_access(place.field, place.receiver.access())
    }

    pub(in crate::typeck) fn rebinding_static_field_alias_access(
        &self,
        field: StaticFieldId,
    ) -> HirAccess {
        if self
            .program
            .static_field(field)
            .expect("resolved static-field use must reference a declaration")
            .final_span
            .is_some()
        {
            HirAccess::ReadOnly
        } else {
            HirAccess::Mutable
        }
    }

    pub(in crate::typeck) fn rebinding_storage_alias_access(
        &self,
        expression: &ResolvedExpression,
        fallback: HirAccess,
    ) -> HirAccess {
        match expression {
            ResolvedExpression::Grouped(grouped) => {
                self.rebinding_storage_alias_access(&grouped.expression, fallback)
            }
            ResolvedExpression::FieldAccess(access) => {
                self.rebinding_field_alias_access(access.field, fallback)
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                self.rebinding_static_field_alias_access(access.field)
            }
            _ => fallback,
        }
    }
}

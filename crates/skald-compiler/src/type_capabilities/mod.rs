//! Target-independent contextual type-capability predicates.
//!
//! Resolution and type checking use different type representations. Both
//! phases adapt those representations to this deliberately small category
//! vocabulary so contextual eligibility rules remain single-sourced without
//! either phase depending on the other's IR.

mod closed;
mod lifecycle;

pub(crate) use closed::{
    failed_interface_specialization_requirements, failed_specialization_requirements,
};
pub(crate) use lifecycle::{LifecyclePathElement, ResolvedLifecycleCapabilities};

use crate::{
    identity::ClassId,
    resolve::{
        GenericInterfaceSpecializationTable, GenericSpecializationTable, ResolvedArrayTypeTable,
        ResolvedClassDeclaration, ResolvedClassDeclarationTable,
        ResolvedClassTemplateSemanticTable, ResolvedInterfaceTemplateSemanticTable,
        ResolvedOptionalBoxTypeTable, ResolvedOptionalTypeTable, ResolvedProgram,
    },
};

/// Resolved declaration and type facts needed by closed capability queries.
///
/// Publication implements this over borrowed candidate products so validation
/// does not need a temporary `ResolvedProgram`. The public resolved program
/// remains the ordinary adapter for later consumers.
pub(crate) trait ResolvedCapabilityView {
    fn classes(&self) -> &ResolvedClassDeclarationTable;
    fn generic_specializations(&self) -> &GenericSpecializationTable;
    fn generic_interface_specializations(&self) -> &GenericInterfaceSpecializationTable;
    fn template_semantics(&self) -> &ResolvedClassTemplateSemanticTable;
    fn interface_template_semantics(&self) -> &ResolvedInterfaceTemplateSemanticTable;
    fn array_types(&self) -> &ResolvedArrayTypeTable;
    fn optional_types(&self) -> &ResolvedOptionalTypeTable;
    fn optional_box_types(&self) -> &ResolvedOptionalBoxTypeTable;

    fn class(&self, id: ClassId) -> Option<&ResolvedClassDeclaration> {
        self.classes().get(id)
    }
}

impl ResolvedCapabilityView for ResolvedProgram {
    fn classes(&self) -> &ResolvedClassDeclarationTable {
        &self.classes
    }

    fn generic_specializations(&self) -> &GenericSpecializationTable {
        &self.generic_specializations
    }

    fn generic_interface_specializations(&self) -> &GenericInterfaceSpecializationTable {
        &self.generic_interface_specializations
    }

    fn template_semantics(&self) -> &ResolvedClassTemplateSemanticTable {
        &self.template_semantics
    }

    fn interface_template_semantics(&self) -> &ResolvedInterfaceTemplateSemanticTable {
        &self.interface_template_semantics
    }

    fn array_types(&self) -> &ResolvedArrayTypeTable {
        &self.array_types
    }

    fn optional_types(&self) -> &ResolvedOptionalTypeTable {
        &self.optional_types
    }

    fn optional_box_types(&self) -> &ResolvedOptionalBoxTypeTable {
        &self.optional_box_types
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TypeCategory {
    Primitive,
    Unit,
    Obj,
    Class,
    Interface,
    Function,
    Shared,
    Optional,
    Array,
}

pub(crate) const fn resolved_type_category(kind: crate::resolve::ResolvedTypeKind) -> TypeCategory {
    use crate::resolve::ResolvedTypeKind;
    match kind {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool => TypeCategory::Primitive,
        ResolvedTypeKind::Unit => TypeCategory::Unit,
        ResolvedTypeKind::Obj => TypeCategory::Obj,
        ResolvedTypeKind::Class(_) => TypeCategory::Class,
        ResolvedTypeKind::Interface(_) => TypeCategory::Interface,
        ResolvedTypeKind::Function(_) => TypeCategory::Function,
        ResolvedTypeKind::Shared(_) => TypeCategory::Shared,
        ResolvedTypeKind::Optional(_) => TypeCategory::Optional,
        ResolvedTypeKind::Array(_) => TypeCategory::Array,
    }
}

pub(crate) const fn supports_stored_value(category: TypeCategory) -> bool {
    !matches!(
        category,
        TypeCategory::Unit | TypeCategory::Obj | TypeCategory::Interface
    )
}

pub(crate) const fn supports_value_result(category: TypeCategory) -> bool {
    !matches!(category, TypeCategory::Obj | TypeCategory::Interface)
}

pub(crate) const fn supports_alias_target(
    category: TypeCategory,
    optional_payload_supports_alias: bool,
) -> bool {
    match category {
        TypeCategory::Primitive
        | TypeCategory::Obj
        | TypeCategory::Class
        | TypeCategory::Interface
        | TypeCategory::Shared
        | TypeCategory::Array => true,
        TypeCategory::Optional => optional_payload_supports_alias,
        TypeCategory::Unit | TypeCategory::Function => false,
    }
}

pub(crate) const fn supports_optional_payload(category: TypeCategory) -> bool {
    matches!(
        category,
        TypeCategory::Primitive
            | TypeCategory::Class
            | TypeCategory::Shared
            | TypeCategory::Optional
            | TypeCategory::Array
    )
}

pub(crate) const fn supports_array_element(category: TypeCategory) -> bool {
    supports_stored_value(category)
}

pub(crate) const fn supports_shared_target(category: TypeCategory) -> bool {
    supports_direct_shared_target(category) || matches!(category, TypeCategory::Optional)
}

pub(crate) const fn supports_direct_shared_target(category: TypeCategory) -> bool {
    matches!(
        category,
        TypeCategory::Obj | TypeCategory::Class | TypeCategory::Interface | TypeCategory::Array
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATEGORIES: [TypeCategory; 9] = [
        TypeCategory::Primitive,
        TypeCategory::Unit,
        TypeCategory::Obj,
        TypeCategory::Class,
        TypeCategory::Interface,
        TypeCategory::Function,
        TypeCategory::Shared,
        TypeCategory::Optional,
        TypeCategory::Array,
    ];

    fn supported(predicate: impl Fn(TypeCategory) -> bool) -> Vec<TypeCategory> {
        CATEGORIES
            .into_iter()
            .filter(|category| predicate(*category))
            .collect()
    }

    #[test]
    fn stored_value_and_array_element_categories_stay_aligned() {
        let expected = vec![
            TypeCategory::Primitive,
            TypeCategory::Class,
            TypeCategory::Function,
            TypeCategory::Shared,
            TypeCategory::Optional,
            TypeCategory::Array,
        ];
        assert_eq!(supported(supports_stored_value), expected);
        assert_eq!(supported(supports_array_element), expected);
    }

    #[test]
    fn value_result_categories_include_unit_but_not_views() {
        assert_eq!(
            supported(supports_value_result),
            vec![
                TypeCategory::Primitive,
                TypeCategory::Unit,
                TypeCategory::Class,
                TypeCategory::Function,
                TypeCategory::Shared,
                TypeCategory::Optional,
                TypeCategory::Array,
            ]
        );
    }

    #[test]
    fn optional_payload_and_shared_target_categories_are_explicit() {
        assert_eq!(
            supported(supports_optional_payload),
            vec![
                TypeCategory::Primitive,
                TypeCategory::Class,
                TypeCategory::Shared,
                TypeCategory::Optional,
                TypeCategory::Array,
            ]
        );
        assert_eq!(
            supported(supports_shared_target),
            vec![
                TypeCategory::Obj,
                TypeCategory::Class,
                TypeCategory::Interface,
                TypeCategory::Optional,
                TypeCategory::Array,
            ]
        );
        assert_eq!(
            supported(supports_direct_shared_target),
            vec![
                TypeCategory::Obj,
                TypeCategory::Class,
                TypeCategory::Interface,
                TypeCategory::Array,
            ]
        );
    }

    #[test]
    fn optional_aliases_follow_their_payload_capability() {
        let direct = supported(|category| supports_alias_target(category, false));
        assert_eq!(
            direct,
            vec![
                TypeCategory::Primitive,
                TypeCategory::Obj,
                TypeCategory::Class,
                TypeCategory::Interface,
                TypeCategory::Shared,
                TypeCategory::Array,
            ]
        );
        assert!(!supports_alias_target(TypeCategory::Optional, false));
        assert!(supports_alias_target(TypeCategory::Optional, true));
    }
}

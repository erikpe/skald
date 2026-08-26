//! Target-independent contextual type-capability predicates.
//!
//! Resolution and type checking use different type representations. Both
//! phases adapt those representations to this deliberately small category
//! vocabulary so contextual eligibility rules remain single-sourced without
//! either phase depending on the other's IR.

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

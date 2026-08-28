use super::*;

#[test]
fn registry_is_complete_unique_and_integer_only() {
    assert_eq!(
        validate_primitive_successor_registry(primitive_successor_registry()),
        Ok(())
    );
    assert_eq!(primitive_successor_registry().len(), 3);
}

#[test]
fn registry_validation_rejects_missing_duplicate_and_unsupported_entries() {
    let registry = primitive_successor_registry();
    assert!(matches!(
        validate_primitive_successor_registry(&registry[..2]),
        Err(PrimitiveSuccessorRegistryError::WrongEntryCount { actual: 2 })
    ));

    let mut duplicate = registry.to_vec();
    duplicate[1] = duplicate[0];
    assert!(matches!(
        validate_primitive_successor_registry(&duplicate),
        Err(PrimitiveSuccessorRegistryError::DuplicatePrimitive { .. })
    ));

    let mut unsupported = registry.to_vec();
    unsupported[0].primitive = ResolvedPrimitiveType::F64;
    assert!(matches!(
        validate_primitive_successor_registry(&unsupported),
        Err(PrimitiveSuccessorRegistryError::UnsupportedPrimitive { index: 0 })
    ));
}

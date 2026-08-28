use super::*;

#[test]
fn closed_registry_is_complete_unique_and_semantically_consistent() {
    assert_eq!(
        validate_primitive_operator_registry(primitive_operator_registry()),
        Ok(())
    );
    assert_eq!(primitive_operator_registry().len(), 60);
}

#[test]
fn registry_validation_rejects_missing_duplicate_wrong_and_unsupported_cells() {
    let registry = primitive_operator_registry();
    assert!(matches!(
        validate_primitive_operator_registry(&registry[..registry.len() - 1]),
        Err(PrimitiveOperatorRegistryError::WrongEntryCount { actual: 59 })
    ));

    let mut duplicate = registry.to_vec();
    duplicate[1] = duplicate[0];
    assert!(matches!(
        validate_primitive_operator_registry(&duplicate),
        Err(PrimitiveOperatorRegistryError::DuplicateKey { .. })
    ));

    let mut wrong_output = registry.to_vec();
    wrong_output[0].output = ResolvedPrimitiveType::Bool;
    assert!(matches!(
        validate_primitive_operator_registry(&wrong_output),
        Err(PrimitiveOperatorRegistryError::OperationMismatch { index: 0 })
    ));

    let mut unsupported = registry.to_vec();
    unsupported[0].operation =
        ResolvedPrimitiveOperatorOperation::Negate(ResolvedPrimitiveType::U64);
    unsupported[0].receiver = ResolvedPrimitiveType::U64;
    unsupported[0].output = ResolvedPrimitiveType::U64;
    assert!(matches!(
        validate_primitive_operator_registry(&unsupported),
        Err(PrimitiveOperatorRegistryError::UnsupportedCell { index: 0 })
    ));
}

#[test]
fn registry_cells_follow_every_frozen_protocol_shape() {
    for entry in primitive_operator_registry() {
        assert_eq!(
            entry.protocol().shape(),
            match entry.rhs() {
                None => CanonicalOperatorProtocolShape::Unary,
                Some(_)
                    if entry.output() == ResolvedPrimitiveType::Bool
                        && matches!(
                            entry.protocol(),
                            CanonicalOperatorProtocol::Eq
                                | CanonicalOperatorProtocol::Less
                                | CanonicalOperatorProtocol::LessEq
                                | CanonicalOperatorProtocol::Greater
                                | CanonicalOperatorProtocol::GreaterEq
                        ) =>
                    CanonicalOperatorProtocolShape::Predicate,
                Some(_) => CanonicalOperatorProtocolShape::Binary,
            }
        );
    }
}

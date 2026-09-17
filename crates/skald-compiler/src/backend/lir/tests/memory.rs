use super::*;

#[test]
fn symbolic_objects_distinguish_zero_size_and_elided_views_and_check_lifetime_sites() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    for role in [
        ObjectRole::SemanticStorage,
        ObjectRole::AggregateResult,
        ObjectRole::AggregateTemporary,
    ] {
        let object: ObjectHandle<'_> = builder
            .declare_object(object(
                LayoutDisposition::Addressable,
                0,
                1,
                role,
                LifetimeDisposition::Sites(2),
            ))
            .unwrap();
        builder
            .append(entry, Operation::ObjectAddress(object))
            .unwrap();
        for marker in [
            LifetimeMarker::Start,
            LifetimeMarker::End,
            LifetimeMarker::Start,
            LifetimeMarker::End,
        ] {
            builder
                .append(
                    entry,
                    Operation::Lifetime {
                        marker,
                        object,
                        site: 1,
                    },
                )
                .unwrap();
        }
        assert_eq!(
            err(builder.append(
                entry,
                Operation::Lifetime {
                    marker: LifetimeMarker::Start,
                    object,
                    site: 2
                }
            )),
            BuildError::InvalidLifetime
        );
    }
    for disposition in [
        LayoutDisposition::ElidedUnit,
        LayoutDisposition::ElidedMetadata,
    ] {
        let object = builder
            .declare_object(object(
                disposition,
                0,
                1,
                ObjectRole::SemanticStorage,
                LifetimeDisposition::WholeCallable,
            ))
            .unwrap();
        assert_eq!(
            err(builder.append(entry, Operation::ObjectAddress(object))),
            BuildError::InvalidObject
        );
        assert_eq!(
            err(builder.append(
                entry,
                Operation::Lifetime {
                    marker: LifetimeMarker::Start,
                    object,
                    site: 0
                }
            )),
            BuildError::InvalidLifetime
        );
    }
    assert_eq!(
        err(builder.declare_object(object(
            LayoutDisposition::Addressable,
            1,
            3,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable
        ))),
        BuildError::InvalidObject
    );
    assert_eq!(
        err(builder.declare_object(object(
            LayoutDisposition::Addressable,
            usize::MAX,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable
        ))),
        BuildError::SizeOverflow
    );
    assert_eq!(
        err(builder.declare_object(object(
            LayoutDisposition::ElidedUnit,
            1,
            1,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::WholeCallable
        ))),
        BuildError::InvalidObject
    );
    assert_eq!(
        err(builder.declare_object(object(
            LayoutDisposition::Addressable,
            1,
            1,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::Sites(0)
        ))),
        BuildError::InvalidLifetime
    );
    assert_eq!(
        err(builder.declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::TraceRecord,
            LifetimeDisposition::WholeCallable
        ))),
        BuildError::Plan(PlanError::OmittedTrace)
    );
    let draft = builder.finish();
    assert_eq!(draft.objects().len(), 5);
    assert_eq!(
        draft
            .objects()
            .take(3)
            .map(|(_, object)| object.role)
            .collect::<Vec<_>>(),
        [
            ObjectRole::SemanticStorage,
            ObjectRole::AggregateResult,
            ObjectRole::AggregateTemporary
        ]
    );
    assert!(draft.objects().all(|(_, object)| object.origin.is_none()));
    let mut traced = facts();
    traced.runtime_trace = crate::backend::RuntimeTracePolicy::Enabled;
    let plan = CheckedPlan::check(traced).unwrap();
    let mut builder = DraftBuilder::new(plan.view().callable(source(0)).unwrap()).unwrap();
    let trace = builder
        .declare_object(object(
            LayoutDisposition::Addressable,
            8,
            8,
            ObjectRole::TraceRecord,
            LifetimeDisposition::WholeCallable,
        ))
        .unwrap();
    assert_eq!(
        builder.finish().object(trace).unwrap().role,
        ObjectRole::TraceRecord
    );
}

#[test]
fn memory_representation_and_address_arithmetic_keep_widths_and_strides_explicit() {
    assert_eq!(
        AddressStride::new(usize::MAX, 2),
        Err(BuildError::SizeOverflow)
    );
    assert_eq!(AddressStride::new(0, usize::MAX).unwrap().bytes(), 0);
    assert_eq!(AddressStride::new(8, 3).unwrap().bytes(), 24);
    let plan = CheckedPlan::check(facts()).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    let object = builder
        .declare_object(object(
            LayoutDisposition::Addressable,
            32,
            8,
            ObjectRole::SemanticStorage,
            LifetimeDisposition::Sites(1),
        ))
        .unwrap();
    let address = builder
        .append(entry, Operation::ObjectAddress(object))
        .unwrap()[0];
    let index = constant(&mut builder, entry, Constant::U64(2));
    let offset = builder
        .append(
            entry,
            Operation::ByteOffset {
                base: address,
                offset: index,
            },
        )
        .unwrap()[0];
    let scaled = builder
        .append(
            entry,
            Operation::ScaledIndex {
                base: offset,
                index,
                stride: AddressStride::new(8, 2).unwrap(),
            },
        )
        .unwrap()[0];
    for scalar in [
        ScalarType::U8,
        ScalarType::Bool,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::F64,
        ScalarType::DataAddress,
    ] {
        let representation = MemoryRepresentation {
            scalar,
            bytes: 8,
            alignment: 8,
        };
        let value = builder
            .append(
                entry,
                Operation::Load {
                    address: scaled,
                    representation,
                },
            )
            .unwrap()[0];
        assert!(builder
            .append(
                entry,
                Operation::Store {
                    address,
                    value,
                    representation
                }
            )
            .unwrap()
            .is_empty());
    }
    for scalar in [ScalarType::U8, ScalarType::Bool] {
        builder
            .append(
                entry,
                Operation::Load {
                    address,
                    representation: MemoryRepresentation {
                        scalar,
                        bytes: 1,
                        alignment: 1,
                    },
                },
            )
            .unwrap();
    }
    for representation in [
        MemoryRepresentation {
            scalar: ScalarType::U64,
            bytes: 1,
            alignment: 1,
        },
        MemoryRepresentation {
            scalar: ScalarType::U8,
            bytes: 2,
            alignment: 1,
        },
        MemoryRepresentation {
            scalar: ScalarType::Bool,
            bytes: 1,
            alignment: 0,
        },
        MemoryRepresentation {
            scalar: ScalarType::F64,
            bytes: 8,
            alignment: 3,
        },
    ] {
        assert_eq!(
            err(builder.append(
                entry,
                Operation::Load {
                    address,
                    representation
                }
            )),
            BuildError::InvalidMemory
        );
    }
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Store {
                address,
                value: index,
                representation: MemoryRepresentation {
                    scalar: ScalarType::U8,
                    bytes: 1,
                    alignment: 1
                }
            }
        )),
        BuildError::InvalidMemory
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Load {
                address: index,
                representation: MemoryRepresentation {
                    scalar: ScalarType::U64,
                    bytes: 8,
                    alignment: 8
                }
            }
        )),
        BuildError::InvalidMemory
    );
    let draft = builder.finish();
    assert!(draft.block(entry).unwrap().instructions.iter().any(|instruction| matches!(instruction.operation, Operation::ScaledIndex { stride, .. } if stride.bytes() == 16)));
}

#[test]
fn symbol_addresses_require_declared_categories_and_exact_code_signatures() {
    use crate::backend::plan::{ArtifactDeclaration, ArtifactId, DataKey};
    let mut supplied = facts();
    let signature = supplied.callables[0].signature;
    let layout = supplied
        .add_layout(LayoutFact {
            size: 0,
            alignment: 1,
            disposition: LayoutDisposition::Addressable,
        })
        .unwrap();
    let data = ArtifactId::Data(DataKey::Table(4));
    supplied.artifacts.push(ArtifactDeclaration {
        key: data,
        signature: None,
        layout: Some(layout),
    });
    let other_signature = supplied
        .add_signature(supplied.signatures[0].clone())
        .unwrap();
    let plan = CheckedPlan::check(supplied).unwrap();
    let mut builder = builder(&plan);
    let entry = entry_block(&mut builder);
    builder
        .append(
            entry,
            Operation::SymbolAddress {
                symbol: data,
                ty: ScalarType::DataAddress,
            },
        )
        .unwrap();
    builder
        .append(
            entry,
            Operation::SymbolAddress {
                symbol: ArtifactId::Callable(source(0)),
                ty: ScalarType::CodeAddress(signature),
            },
        )
        .unwrap();
    assert_eq!(
        err(builder.append(
            entry,
            Operation::SymbolAddress {
                symbol: data,
                ty: ScalarType::CodeAddress(signature)
            }
        )),
        BuildError::InvalidScalar
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::SymbolAddress {
                symbol: ArtifactId::Callable(source(0)),
                ty: ScalarType::DataAddress
            }
        )),
        BuildError::InvalidScalar
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::SymbolAddress {
                symbol: ArtifactId::Callable(source(0)),
                ty: ScalarType::CodeAddress(other_signature)
            }
        )),
        BuildError::InvalidScalar
    );
    let first = constant(
        &mut builder,
        entry,
        Constant::Null(ScalarType::CodeAddress(signature)),
    );
    let second = constant(
        &mut builder,
        entry,
        Constant::Null(ScalarType::CodeAddress(other_signature)),
    );
    assert_eq!(
        err(builder.append(
            entry,
            Operation::Compare {
                predicate: Predicate::Equal,
                left: first,
                right: second
            }
        )),
        BuildError::InvalidScalar
    );
}

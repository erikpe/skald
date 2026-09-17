use super::*;

#[test]
fn target_freeze_is_bound_to_the_exact_parent_inventory_and_preserves_parent_facts() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let parent = program(&plan);
    let different = program(&plan);
    let layout = plan.view().layout_id(0).unwrap();
    let layout_id = facts().add_layout(facts().layouts[0]).unwrap();
    // Obtain a valid parent pool ID through a declaration rather than forge one.
    let signature = plan.view().callables().next().unwrap().signature;
    let mut extension = TargetDeclarations::new(&parent);
    extension
        .declare(ArtifactDeclaration {
            key: thunk(),
            signature: Some(signature),
            layout: None,
        })
        .unwrap();
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: ArtifactId::Callable(source(0)),
            signature: Some(signature),
            layout: None
        })),
        ProgramError::DuplicateDefinition
    );
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Table(0)),
            signature: None,
            layout: Some(layout_id)
        })),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    let frozen: TargetExtension<'_, '_> = extension.freeze().unwrap();
    frozen.require_parent(&parent).unwrap();
    assert_eq!(
        error(frozen.require_parent(&different)),
        ProgramError::Plan(PlanError::WrongContext)
    );
    assert_eq!(frozen.declarations().len(), 1);
    assert_eq!(
        frozen
            .artifact(thunk(), ArtifactCategory::Callable)
            .unwrap(),
        None
    );
    assert_eq!(frozen.data().len(), 0);
    assert_eq!(plan.view().layout(layout).unwrap().size, 8);
}

#[test]
fn target_constants_cannot_replace_parent_data_or_mutate_frozen_catalogs() {
    let plan = data_plan();
    let mut work = ProgramBuilder::new(plan.view());
    complete_all(&mut work, &plan);
    for key in [DataKey::Table(0), DataKey::Table(1)] {
        work.define_data(definition(key, DataInitializer::Bytes(vec![0; 8])))
            .unwrap();
    }
    let parent = work.finish().unwrap();
    let layout = plan
        .view()
        .artifacts()
        .find(|a| a.key == ArtifactId::Data(DataKey::Table(0)))
        .unwrap()
        .layout
        .unwrap();
    let decl = ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Table(2)),
        signature: None,
        layout: Some(layout),
    };
    let mut extension = TargetDeclarations::new(&parent);
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Table(0)),
            ..decl
        })),
        ProgramError::DuplicateDefinition
    );
    extension.declare(decl).unwrap();
    assert_eq!(
        error(extension.declare(decl)),
        ProgramError::DuplicateDefinition
    );
    assert_eq!(
        error(extension.define_data(definition(DataKey::Table(0), DataInitializer::Zero(8)))),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    extension
        .define_data(definition(
            DataKey::Table(2),
            DataInitializer::Address {
                target: ArtifactId::Data(DataKey::Table(0)),
                category: ArtifactCategory::Data,
                addend: 0,
            },
        ))
        .unwrap();
    let frozen = extension.freeze().unwrap();
    assert_eq!(frozen.data().len(), 1);
    assert_eq!(
        frozen
            .artifact(ArtifactId::Data(DataKey::Table(2)), ArtifactCategory::Data)
            .unwrap(),
        Some(8)
    );
    assert_eq!(
        error(frozen.artifact(thunk(), ArtifactCategory::Data)),
        ProgramError::Plan(PlanError::ArtifactCategoryMismatch)
    );
    let mut missing = TargetDeclarations::new(&parent);
    missing.declare(decl).unwrap();
    assert_eq!(
        error(missing.freeze()),
        ProgramError::MissingDefinition(decl.key)
    );
}

#[test]
fn target_extensions_reject_new_signatures_trace_inventory_and_semantic_bodies() {
    let plan = CheckedPlan::check(facts()).unwrap();
    let parent = program(&plan);
    let mut f = facts();
    let signature = f.add_signature(f.signatures[0].clone()).unwrap();
    let mut extension = TargetDeclarations::new(&parent);
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: thunk(),
            signature: Some(signature),
            layout: None
        })),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: ArtifactId::Callable(source(99)),
            signature: Some(signature),
            layout: None
        })),
        ProgramError::Plan(PlanError::InvalidDomain)
    );
    assert_eq!(
        error(extension.declare(ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::TraceContext(0)),
            signature: None,
            layout: None
        })),
        ProgramError::Plan(PlanError::InvalidDomain)
    );
    assert_eq!(extension.freeze().unwrap().declarations().len(), 0);
}

#[test]
fn target_constant_cycles_are_predeclared_and_sorted_independently_of_arrival() {
    let plan = data_plan();
    let mut work = ProgramBuilder::new(plan.view());
    complete_all(&mut work, &plan);
    for key in [DataKey::Table(0), DataKey::Table(1)] {
        work.define_data(definition(key, DataInitializer::Zero(8)))
            .unwrap();
    }
    let parent = work.finish().unwrap();
    let layout = parent
        .parent()
        .artifacts()
        .find(|a| a.key == ArtifactId::Data(DataKey::Table(0)))
        .unwrap()
        .layout
        .unwrap();
    for order in [[3, 2], [2, 3]] {
        let mut extension = TargetDeclarations::new(&parent);
        for key in order {
            extension
                .declare(ArtifactDeclaration {
                    key: ArtifactId::Data(DataKey::Table(key)),
                    signature: None,
                    layout: Some(layout),
                })
                .unwrap();
        }
        for (key, target) in [(3, 2), (2, 3)] {
            extension
                .define_data(definition(
                    DataKey::Table(key),
                    DataInitializer::Address {
                        target: ArtifactId::Data(DataKey::Table(target)),
                        category: ArtifactCategory::Data,
                        addend: 0,
                    },
                ))
                .unwrap();
        }
        assert_eq!(
            extension
                .freeze()
                .unwrap()
                .declarations()
                .map(|a| a.key)
                .collect::<Vec<_>>(),
            vec![
                ArtifactId::Data(DataKey::Table(2)),
                ArtifactId::Data(DataKey::Table(3))
            ]
        );
    }
}

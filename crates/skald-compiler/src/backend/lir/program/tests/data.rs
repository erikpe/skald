use super::*;

#[test]
fn data_cycles_have_explicit_dependencies_and_exact_checked_addends() {
    let plan = data_plan();
    let mut work = ProgramBuilder::new(plan.view());
    complete_all(&mut work, &plan);
    assert_eq!(
        error(work.define_data(definition(DataKey::Table(0), DataInitializer::Zero(7)))),
        ProgramError::InvalidInitializer
    );
    assert_eq!(
        error(work.define_data(definition(
            DataKey::Table(0),
            DataInitializer::Address {
                target: ArtifactId::Data(DataKey::Table(1)),
                category: ArtifactCategory::Callable,
                addend: 0
            }
        ))),
        ProgramError::Plan(PlanError::ArtifactCategoryMismatch)
    );
    for addend in [-1, 9] {
        assert_eq!(
            error(work.define_data(definition(
                DataKey::Table(0),
                DataInitializer::Address {
                    target: ArtifactId::Data(DataKey::Table(1)),
                    category: ArtifactCategory::Data,
                    addend
                }
            ))),
            ProgramError::InvalidAddend
        );
    }
    assert_eq!(
        error(work.define_data(definition(
            DataKey::Table(0),
            DataInitializer::Address {
                target: ArtifactId::Callable(source(0)),
                category: ArtifactCategory::Callable,
                addend: 1
            }
        ))),
        ProgramError::InvalidAddend
    );
    for (key, target, addend) in [(0, 1, 8), (1, 0, 0)] {
        work.define_data(definition(
            DataKey::Table(key),
            DataInitializer::Address {
                target: ArtifactId::Data(DataKey::Table(target)),
                category: ArtifactCategory::Data,
                addend,
            },
        ))
        .unwrap();
    }
    assert_eq!(
        error(work.define_data(definition(DataKey::Table(0), DataInitializer::Zero(8)))),
        ProgramError::DuplicateDefinition
    );
    assert_eq!(work.finish().unwrap().data().len(), 2);
}

#[test]
fn missing_initializers_unknown_targets_absent_sources_and_size_overflow_are_rejected() {
    let plan = data_plan();
    let mut work = ProgramBuilder::new(plan.view());
    complete_all(&mut work, &plan);
    assert_eq!(
        error(work.define_data(definition(DataKey::Table(99), DataInitializer::Zero(8)))),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    assert_eq!(
        error(work.define_data(definition(
            DataKey::Table(0),
            DataInitializer::Address {
                target: ArtifactId::Data(DataKey::Table(99)),
                category: ArtifactCategory::Data,
                addend: 0
            }
        ))),
        ProgramError::Plan(PlanError::UnknownDeclaration)
    );
    assert_eq!(
        error(work.define_data(DataDefinition {
            key: DataKey::Table(0),
            initializers: vec![DataInitializer::Zero(usize::MAX), DataInitializer::Zero(1)]
        })),
        ProgramError::SizeOverflow
    );
    assert_eq!(
        error(work.finish()),
        ProgramError::MissingDefinition(ArtifactId::Data(DataKey::Table(0)))
    );
    let mut f = facts();
    f.callables[1].body = BodyDisposition::Absent;
    if let LirCallableId::Source(key) = source(1) {
        f.executable_sources.remove(&key);
    }
    let layout = f.add_layout(f.layouts[0]).unwrap();
    f.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Table(0)),
        layout: Some(layout),
        signature: None,
    });
    let plan = CheckedPlan::check(f).unwrap();
    let mut work = ProgramBuilder::new(plan.view());
    assert_eq!(
        error(work.define_data(definition(
            DataKey::Table(0),
            DataInitializer::Address {
                target: ArtifactId::Callable(source(1)),
                category: ArtifactCategory::Callable,
                addend: 0
            }
        ))),
        ProgramError::Plan(PlanError::AbsentBody)
    );
}

#[test]
fn runtime_external_null_dispatch_and_inactive_static_dispositions_need_no_body() {
    let mut f = facts();
    runtime_declarations(&mut f);
    let mut external = f.signatures[0].clone();
    external.convention = Convention::ExternC;
    let signature = f.add_signature(external).unwrap();
    f.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::External(crate::identity::ExternalLinkId::new(0)),
        signature: Some(signature),
        layout: None,
    });
    f.dispatch.push(DispatchSlot {
        family: crate::identity::VirtualFamilyId::new(0),
        index: 0,
        target: None,
    });
    let layout = f.add_layout(f.layouts[0]).unwrap();
    let field = crate::identity::StaticFieldId::new(crate::identity::ClassId::new(0), 0);
    f.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(DataKey::Static(field)),
        signature: None,
        layout: Some(layout),
    });
    let plan = CheckedPlan::check(f).unwrap();
    let mut work = ProgramBuilder::new(plan.view());
    assert_eq!(
        error(work.define_data(definition(DataKey::Static(field), DataInitializer::Zero(8)))),
        ProgramError::Plan(PlanError::InvalidDomain)
    );
    complete_all(&mut work, &plan);
    assert_eq!(work.finish().unwrap().parent().dispatch()[0].target, None);
}

#[test]
fn intrinsic_failure_data_must_match_the_shared_message_catalog() {
    let mut f = facts();
    let reason = crate::backend::failure::FailureMessage::IntegerDivisionByZero;
    let key = DataKey::FailureMessage(reason);
    let mut layout = f.layouts[0];
    layout.size = reason.bytes().len();
    layout.alignment = 1;
    let layout = f.add_layout(layout).unwrap();
    f.artifacts.push(ArtifactDeclaration {
        key: ArtifactId::Data(key),
        signature: None,
        layout: Some(layout),
    });
    let plan = CheckedPlan::check(f).unwrap();
    let mut work = ProgramBuilder::new(plan.view());
    assert_eq!(
        error(work.define_data(definition(key, DataInitializer::Zero(reason.bytes().len())))),
        ProgramError::InvalidInitializer
    );
    work.define_data(definition(
        key,
        DataInitializer::Bytes(reason.bytes().to_vec()),
    ))
    .unwrap();
    complete_all(&mut work, &plan);
    assert_eq!(work.finish().unwrap().data().len(), 1);
}

#[test]
fn active_statics_require_initializers_in_both_artifact_policies() {
    for policy in [
        crate::backend::plan::ArtifactPolicy::Complete,
        crate::backend::plan::ArtifactPolicy::Reachable,
    ] {
        let mut f = facts();
        f.artifact_policy = policy;
        let field = crate::identity::StaticFieldId::new(crate::identity::ClassId::new(0), 0);
        f.active_statics.insert(field);
        let layout = f.add_layout(f.layouts[0]).unwrap();
        f.artifacts.push(ArtifactDeclaration {
            key: ArtifactId::Data(DataKey::Static(field)),
            signature: None,
            layout: Some(layout),
        });
        let plan = CheckedPlan::check(f).unwrap();
        let mut work = ProgramBuilder::new(plan.view());
        complete_all(&mut work, &plan);
        assert_eq!(
            error(work.finish()),
            ProgramError::MissingDefinition(ArtifactId::Data(DataKey::Static(field)))
        );
        let mut work = ProgramBuilder::new(plan.view());
        complete_all(&mut work, &plan);
        work.define_data(definition(DataKey::Static(field), DataInitializer::Zero(8)))
            .unwrap();
        assert_eq!(work.finish().unwrap().data().len(), 1);
    }
}

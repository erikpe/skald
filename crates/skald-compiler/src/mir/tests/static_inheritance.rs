use crate::identity::{BindingId, ClassId, FunctionId, LocalId};

use super::*;

fn messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .unwrap_err()
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

#[test]
fn lowering_retains_hierarchy_views_slices_and_lifecycle_plans() {
    let program = static_inheritance_mir();
    verify_mir(&program).expect("static inheritance MIR must verify");

    let base = ClassId::new(0);
    let middle = ClassId::new(1);
    let derived = ClassId::new(2);
    assert_eq!(program.direct_base(derived), Some(middle));
    assert_eq!(program.direct_base(middle), Some(base));
    assert!(program.is_ancestor(base, derived));

    let MirCopyCapability::Synthesized(copy) = &program.class(derived).unwrap().copy_constructor
    else {
        panic!("derived class must synthesize copy construction");
    };
    assert_eq!(copy.base.as_ref().map(|step| step.base), Some(middle));
    assert_eq!(
        program.class(derived).unwrap().destruction.steps.last(),
        Some(&MirDestructionStep::Base(middle))
    );

    let dump = dump_mir(&program);
    assert!(dump.contains("DirectBase c1"));
    assert!(dump.contains("Base c1 via synthesized c1"));
    assert!(dump.contains(".base(c1).base(c0)"));
    assert!(dump.contains("-> class c0 readonly"));
    assert!(dump.contains("-> Obj readonly"));
    assert!(dump.contains("copy-construct"));
}

#[test]
fn verifier_rejects_corrupt_hierarchy_and_base_projection_metadata() {
    let mut hierarchy = static_inheritance_mir();
    let derived_span = hierarchy.class(ClassId::new(2)).unwrap().span;
    hierarchy.classes.entries_mut_for_test()[0].direct_base = Some(MirDirectBase {
        class: ClassId::new(2),
        span: derived_span,
    });
    assert!(messages(&hierarchy)
        .iter()
        .any(|message| message.contains("direct-base chain contains a cycle")));

    let mut projection = static_inheritance_mir();
    let relay = projection
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let view = relay.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &mut call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .unwrap();
    view.source.projections[0] = MirPlaceProjection::Base(ClassId::new(0));
    assert!(messages(&projection)
        .iter()
        .any(|message| message.contains("is not the declared direct base")));
}

#[test]
fn verifier_rejects_corrupt_view_access_and_targets() {
    let mut access = static_inheritance_mir();
    let relay = access
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let call = relay.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    let MirArgument::View(view) = &mut call.arguments[0] else {
        panic!("ancestor alias argument must be a view");
    };
    view.access = MirAliasAccess::Mutable;
    assert!(messages(&access)
        .iter()
        .any(|message| message.contains("view grants mutable access")));

    let mut target = static_inheritance_mir();
    let relay = target
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let call = relay.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    let MirArgument::View(view) = &mut call.arguments[0] else {
        panic!("ancestor alias argument must be a view");
    };
    view.target = MirViewTarget::Class(ClassId::new(2));
    assert!(messages(&target)
        .iter()
        .any(|message| message.contains("view target type mismatch")));

    let mut liveness = static_inheritance_mir();
    let relay = liveness
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .unwrap();
    let storage = StorageId::new(FunctionId::new(2), relay.storage.len());
    relay.storage.push(crate::mir::test_fixtures::storage(
        storage,
        Some(BindingId::Local(LocalId::new(FunctionId::new(2), 0))),
        "uninitialized",
        MirStorageKind::Local,
        MirType::Class(ClassId::new(2)),
        relay.span,
    ));
    let call = relay.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    let MirArgument::View(view) = &mut call.arguments[0] else {
        panic!("ancestor alias argument must be a view");
    };
    view.source = MirPlace::base(storage)
        .project_base(ClassId::new(1))
        .project_base(ClassId::new(0));
    assert!(messages(&liveness)
        .iter()
        .any(|message| message.contains("object view source is not live")));
}

#[test]
fn verifier_rejects_corrupt_base_copy_destruction_and_slice_overlap() {
    let mut copy_plan = static_inheritance_mir();
    let MirCopyCapability::Synthesized(copy) =
        &mut copy_plan.classes.entries_mut_for_test()[2].copy_constructor
    else {
        panic!("derived class must synthesize copy construction");
    };
    copy.base = None;
    assert!(messages(&copy_plan)
        .iter()
        .any(|message| message.contains("invalid direct-base step")));

    let mut destruction = static_inheritance_mir();
    destruction.classes.entries_mut_for_test()[2]
        .destruction
        .steps
        .pop();
    assert!(messages(&destruction)
        .iter()
        .any(|message| message.contains("then its direct base")));

    let mut overlap = static_inheritance_mir();
    let main = overlap
        .definitions
        .get_mut_for_test(overlap.entry_function)
        .unwrap();
    let copy = main.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::CopyConstruct(copy)
                if copy
                    .source
                    .projections
                    .iter()
                    .any(|projection| matches!(projection, MirPlaceProjection::Base(_))) =>
            {
                Some(copy)
            }
            _ => None,
        })
        .unwrap();
    copy.destination = copy.source.clone();
    assert!(messages(&overlap)
        .iter()
        .any(|message| message.contains("must not overlap")));
}

use super::*;
use crate::typeck::expression::object_view::CheckedSharedPointee;
use crate::{
    hir::{HirObjectOrigin, HirStaticPlace, HirViewSource},
    identity::{BindingId, ClassId, FunctionId, InterfaceId, LocalId, StaticFieldId},
    object_path::ObjectPath,
    test_support::resolve_source,
};

const VIEW_GRAPH: &str = "\
interface Marker { fn mark() -> unit; }\n\
interface Extra { fn extra() -> unit; }\n\
class Root implements Marker { init() {} fn mark() -> unit {} }\n\
class Middle extends Root { init() { super(); } }\n\
class Leaf extends Middle implements Extra {\n\
  init() { super(); }\n\
  fn extra() -> unit {}\n\
}\n\
class Other { init() {} }\n\
fn main() -> i64 { return 0; }\n";

#[test]
fn direct_relations_distinguish_static_failure_and_checked_narrowing() {
    let program = resolved_view_graph();
    let root = ClassId::new(0);
    let leaf = ClassId::new(2);

    let incompatible = plan_object_view(
        &program,
        exact_class_source(&program, root, HirAccess::Mutable),
        immediate(HirViewTarget::Class(leaf), HirAccess::ReadOnly),
    );
    assert!(matches!(
        incompatible,
        Err(ObjectViewProblem::IncompatibleTarget(..))
    ));

    let span = program.class(root).unwrap().span;
    let runtime = plan_object_view(
        &program,
        ObjectViewSource::Obj {
            binding: binding(0),
            access: HirAccess::Mutable,
            span,
        },
        immediate(HirViewTarget::Class(leaf), HirAccess::ReadOnly),
    );
    assert!(matches!(
        runtime,
        Err(ObjectViewProblem::RequiresExplicitCheckedOperation(..))
    ));

    let interface = planned(plan_object_view(
        &program,
        exact_class_source(&program, leaf, HirAccess::Mutable),
        immediate(
            HirViewTarget::Interface(InterfaceId::new(1)),
            HirAccess::ReadOnly,
        ),
    ))
    .into_view();
    assert_eq!(
        interface.target,
        HirViewTarget::Interface(InterfaceId::new(1))
    );
}

#[test]
fn planning_checks_access_before_constructing_a_view() {
    let program = resolved_view_graph();
    let result = plan_object_view(
        &program,
        exact_class_source(&program, ClassId::new(2), HirAccess::ReadOnly),
        immediate(HirViewTarget::Obj, HirAccess::Mutable),
    );
    assert!(matches!(
        result,
        Err(ObjectViewProblem::InsufficientAccess(..))
    ));
}

#[test]
fn class_and_shared_plans_preserve_base_projection_order() {
    let program = resolved_view_graph();
    let root = ClassId::new(0);
    let middle = ClassId::new(1);
    let leaf = ClassId::new(2);
    let expected = [ObjectProjection::Base(middle), ObjectProjection::Base(root)];

    let class_view = planned(plan_object_view(
        &program,
        exact_class_source(&program, leaf, HirAccess::Mutable),
        immediate(HirViewTarget::Class(root), HirAccess::ReadOnly),
    ))
    .into_view();
    let HirViewSource::Place(place) = class_view.source else {
        panic!("class plans must retain their object place")
    };
    assert_eq!(place.projections(), expected);

    let shared_view = planned(plan_object_view(
        &program,
        stable_shared_source(&program, leaf),
        immediate(HirViewTarget::Class(root), HirAccess::ReadOnly),
    ))
    .into_view();
    let HirViewSource::Shared { projections, .. } = shared_view.source else {
        panic!("immediate stable shared plans must borrow the binding")
    };
    assert_eq!(projections, expected);
}

#[test]
fn static_plans_preserve_complete_origin_and_append_target_projections() {
    let program = resolved_view_graph();
    let root = ClassId::new(0);
    let middle = ClassId::new(1);
    let leaf = ClassId::new(2);
    let span = program.class(leaf).unwrap().span;
    let place = HirStaticPlace {
        field: StaticFieldId::new(leaf, 0),
        span,
    };

    let view = planned(plan_object_view(
        &program,
        ObjectViewSource::Static {
            place,
            dynamic_class: leaf,
            class: leaf,
            projections: Vec::new(),
            span,
        },
        immediate(HirViewTarget::Class(root), HirAccess::ReadOnly),
    ))
    .into_view();

    assert!(matches!(
        *view.origin,
        HirObjectOrigin::Static {
            place: origin_place,
            dynamic_class,
        } if origin_place == place && dynamic_class == leaf
    ));
    assert!(matches!(
        view.source,
        HirViewSource::Static { place: source_place, ref projections }
            if source_place == place
                && projections == &[ObjectProjection::Base(middle), ObjectProjection::Base(root)]
    ));
}

#[test]
fn retention_selects_the_shared_anchor_before_hir_construction() {
    let program = resolved_view_graph();
    let leaf = ClassId::new(2);
    let target = HirViewTarget::Class(leaf);

    let immediate = planned(plan_object_view(
        &program,
        stable_shared_source(&program, leaf),
        ObjectViewRequest::new(
            target,
            HirAccess::ReadOnly,
            ObjectViewRetention::ImmediateConsumer,
        ),
    ))
    .into_view();
    assert!(matches!(immediate.source, HirViewSource::Shared { .. }));

    let loop_body = planned(plan_object_view(
        &program,
        stable_shared_source(&program, leaf),
        ObjectViewRequest::new(target, HirAccess::ReadOnly, ObjectViewRetention::LoopBody),
    ))
    .into_view();
    assert!(matches!(
        loop_body.source,
        HirViewSource::AnchoredShared { .. }
    ));
}

#[test]
fn forwarded_obj_and_interface_sources_keep_their_static_view_facts() {
    let program = resolved_view_graph();
    let span = program.class(ClassId::new(0)).unwrap().span;
    let cases = [
        (
            ObjectViewSource::Obj {
                binding: binding(0),
                access: HirAccess::Mutable,
                span,
            },
            HirViewTarget::Obj,
        ),
        (
            ObjectViewSource::Interface {
                binding: binding(1),
                interface: InterfaceId::new(0),
                access: HirAccess::ReadOnly,
                span,
            },
            HirViewTarget::Interface(InterfaceId::new(0)),
        ),
    ];

    for (source, target) in cases {
        let view = planned(plan_object_view(
            &program,
            source,
            immediate(target, HirAccess::ReadOnly),
        ))
        .into_view();
        let HirViewSource::Forwarded {
            target: source_target,
            ..
        } = view.source
        else {
            panic!("forwarded sources must remain forwarded in HIR")
        };
        assert_eq!(source_target, target);
        assert_eq!(view.target, target);
    }
}

#[test]
fn closed_world_overlap_does_not_create_a_new_implicit_interface_conversion() {
    let output = resolve_source(
        "interface Left { fn left() -> unit; }\n\
         interface Right { fn right() -> unit; }\n\
         class Both implements Left, Right {\n\
           init() {}\n\
           fn left() -> unit {}\n\
           fn right() -> unit {}\n\
         }\n\
         fn main() -> i64 { return 0; }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let span = output.program.class(ClassId::new(0)).unwrap().span;
    let result = plan_object_view(
        &output.program,
        ObjectViewSource::Interface {
            binding: binding(0),
            interface: InterfaceId::new(0),
            access: HirAccess::ReadOnly,
            span,
        },
        immediate(
            HirViewTarget::Interface(InterfaceId::new(1)),
            HirAccess::ReadOnly,
        ),
    );
    assert!(matches!(
        result,
        Err(ObjectViewProblem::RequiresExplicitCheckedOperation(..))
    ));
}

fn resolved_view_graph() -> ResolvedProgram {
    let output = resolve_source(VIEW_GRAPH);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    output.program
}

fn immediate(target: HirViewTarget, access: HirAccess) -> ObjectViewRequest {
    ObjectViewRequest::new(target, access, ObjectViewRetention::ImmediateConsumer)
}

fn planned(result: Result<ObjectViewPlan, ObjectViewProblem>) -> ObjectViewPlan {
    match result {
        Ok(plan) => plan,
        Err(_) => panic!("test source and request must produce a direct view plan"),
    }
}

fn exact_class_source(
    program: &ResolvedProgram,
    class: ClassId,
    access: HirAccess,
) -> ObjectViewSource {
    let place = class_place(program, class, access);
    ObjectViewSource::Class {
        origin: HirObjectOrigin::Exact {
            complete: place.clone(),
            dynamic_class: class,
        },
        place,
    }
}

fn class_place(program: &ResolvedProgram, class: ClassId, access: HirAccess) -> HirObjectPlace {
    let span = program.class(class).expect("test class must exist").span;
    HirObjectPlace {
        path: ObjectPath::root(binding(0), class, span),
        access,
    }
}

fn stable_shared_source(program: &ResolvedProgram, class: ClassId) -> ObjectViewSource {
    let span = program.class(class).expect("test class must exist").span;
    ObjectViewSource::Shared(CheckedSharedPointee::stable(
        binding(0),
        HirViewTarget::Class(class),
        HirAccess::Mutable,
        Vec::new(),
        span,
    ))
}

fn binding(index: usize) -> BindingId {
    BindingId::Local(LocalId::new(FunctionId::new(0), index))
}

use super::*;
use crate::{identity::InterfaceId, test_support::resolve_source};

const VIEW_GRAPH: &str = "\
interface Marker { fn mark() -> unit; }\n\
interface Extra { fn extra() -> unit; }\n\
class Root implements Marker {\n\
  init() {}\n\
  fn mark() -> unit {}\n\
}\n\
class Derived extends Root implements Extra {\n\
  init() { super(); }\n\
  fn extra() -> unit {}\n\
}\n\
class Peer implements Marker {\n\
  init() {}\n\
  fn mark() -> unit {}\n\
}\n\
class Other { init() {} }\n\
fn main() -> i64 { return 0; }\n";

#[test]
fn exact_classes_resolve_same_upcast_interface_and_impossible_targets_statically() {
    let program = resolved_view_graph();
    let derived = ClassId::new(1);

    assert_relation(
        &program,
        ObjectViewSource::ExactClass(derived),
        HirViewTarget::Class(derived),
        ObjectViewRelation::StaticSuccess,
    );
    assert_relation(
        &program,
        ObjectViewSource::ExactClass(derived),
        HirViewTarget::Class(ClassId::new(0)),
        ObjectViewRelation::StaticSuccess,
    );
    assert_relation(
        &program,
        ObjectViewSource::ExactClass(derived),
        HirViewTarget::Interface(InterfaceId::new(0)),
        ObjectViewRelation::StaticSuccess,
    );
    assert_relation(
        &program,
        ObjectViewSource::ExactClass(ClassId::new(0)),
        HirViewTarget::Class(derived),
        ObjectViewRelation::StaticFailure,
    );
    assert_relation(
        &program,
        ObjectViewSource::ExactClass(ClassId::new(3)),
        HirViewTarget::Interface(InterfaceId::new(0)),
        ObjectViewRelation::StaticFailure,
    );
}

#[test]
fn forwarded_class_interface_and_obj_views_use_the_closed_declared_class_set() {
    let program = resolved_view_graph();
    let marker = HirViewTarget::Interface(InterfaceId::new(0));
    let extra = HirViewTarget::Interface(InterfaceId::new(1));

    assert_relation(
        &program,
        ObjectViewSource::Dynamic(HirViewTarget::Class(ClassId::new(0))),
        HirViewTarget::Class(ClassId::new(1)),
        ObjectViewRelation::Runtime,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(HirViewTarget::Class(ClassId::new(0))),
        marker,
        ObjectViewRelation::StaticSuccess,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(marker),
        marker,
        ObjectViewRelation::StaticSuccess,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(marker),
        extra,
        ObjectViewRelation::Runtime,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(extra),
        HirViewTarget::Class(ClassId::new(2)),
        ObjectViewRelation::StaticFailure,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(HirViewTarget::Obj),
        HirViewTarget::Class(ClassId::new(0)),
        ObjectViewRelation::Runtime,
    );
    assert_relation(
        &program,
        ObjectViewSource::Dynamic(HirViewTarget::Obj),
        HirViewTarget::Obj,
        ObjectViewRelation::StaticSuccess,
    );
}

fn resolved_view_graph() -> ResolvedProgram {
    let output = resolve_source(VIEW_GRAPH);
    assert!(
        output.diagnostics.is_empty(),
        "view graph must resolve: {:?}",
        output.diagnostics
    );
    output.program
}

fn assert_relation(
    program: &ResolvedProgram,
    source: ObjectViewSource,
    target: HirViewTarget,
    expected: ObjectViewRelation,
) {
    assert_eq!(
        classify_object_view_relation(program, source, target),
        expected
    );
}

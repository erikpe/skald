use super::*;
use crate::{
    identity::{ClassId, ClassTemplateId, InterfaceId, InterfaceTemplateId},
    resolve::{
        dump_resolved, GenericInterfaceApplicationOrigin, GenericInterfaceSpecializationState,
        GenericSpecializationState, ResolvedSharedTarget, ResolvedTypeKind,
    },
    source::Span,
    test_support::resolve_source,
};

const LOCAL_SPECIALIZATION_ORDER_SOURCE: &str = "interface View<T> {}\n\
     class Base<T> { init() {} }\n\
     class Derived<Marker> extends Base<shared View<i64>> {\n\
       init() { super(); }\n\
     }\n\
     fn main() -> i64 {\n\
       var value: shared Base<shared View<i64>> = new Derived<shared View<bool>>();\n\
       return 0;\n\
     }\n";

const TYPE_BEARING_SOURCE_SHAPES: &str = concat!(
    "class Box<T> {\n",
    "  init() {}\n",
    "  static fn ping() -> unit {}\n",
    "  fn index_get(key: T) -> i64 { return 0; }\n",
    "  fn slice_get(start: T?, end: T?) -> i64 { return 0; }\n",
    "}\n",
    "class Base<T> { init() {} }\n",
    "interface Face<T> {}\n",
    "class Hidden<T> { hidden: Box<bool>; }\n",
    "extern fn external(parameter: Box<i64>) -> Box<i64>;\n",
    "class Uses extends Base<Box<i64>> implements Face<Box<i64>> {\n",
    "  field: Box<i64>;\n",
    "  static stored: Box<i64> = Box<i64>();\n",
    "  init(parameter: Box<i64>) {\n",
    "    super();\n",
    "    var local: Box<i64> = Box<i64>();\n",
    "    local is Box<i64>;\n",
    "    (Box<i64>) local;\n",
    "    new Box<i64>();\n",
    "    new Box<i64>?();\n",
    "    Box<i64>[]();\n",
    "    Box<i64>.ping();\n",
    "    consume(Box<i64>());\n",
    "    if (Box<i64>()) { Box<i64>(); } elif (Box<i64>()) { Box<i64>(); } else { Box<i64>(); }\n",
    "    while (Box<i64>()) { Box<i64>(); break; }\n",
    "    for (item: Box<i64> in Box<i64>()) { Box<i64>(); }\n",
    "    for (item: Box<i64> in Box<i64>() .. Box<i64>()) { Box<i64>(); }\n",
    "    self.field = Box<i64>();\n",
    "    local = Box<i64>();\n",
    "    Box<i64>()[Box<i64>()];\n",
    "    Box<i64>()[Box<i64>():Box<i64>()];\n",
    "  }\n",
    "  fn inspect(parameter: Box<i64>) -> Box<i64> { return Box<i64>(); }\n",
    "}\n",
    "fn consume(parameter: Box<i64>) -> unit {}\n",
    "fn produce() -> Box<i64> { return Box<i64>(); }\n",
);

#[test]
fn local_annotation_specializations_precede_initializer_specializations() {
    let output = resolve_source(LOCAL_SPECIALIZATION_ORDER_SOURCE);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let classes = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(classes.len(), 2);
    assert_eq!(classes[0].key.template, ClassTemplateId::new(0));
    assert_eq!(classes[1].key.template, ClassTemplateId::new(1));
    assert_eq!(
        classes[0].state,
        GenericSpecializationState::Complete(ClassId::new(0))
    );
    assert_eq!(
        classes[1].state,
        GenericSpecializationState::Complete(ClassId::new(1))
    );
    assert!(matches!(
        classes[0].key.arguments.as_slice(),
        [ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(interface))]
            if *interface == InterfaceId::new(0)
    ));
    assert!(matches!(
        classes[1].key.arguments.as_slice(),
        [ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(interface))]
            if *interface == InterfaceId::new(1)
    ));
    assert_eq!(
        class_origin_texts(&classes[0].provenance.origins),
        ["Base<shared View<i64>>"]
    );
    assert_eq!(
        class_origin_texts(&classes[1].provenance.origins),
        ["Derived<shared View<bool>>"]
    );
    assert_eq!(
        class_origin_ranges(&classes[0].provenance.origins),
        [(163, 185)]
    );
    assert_eq!(
        class_origin_ranges(&classes[1].provenance.origins),
        [(192, 218)]
    );

    let interfaces = output
        .program
        .generic_interface_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(interfaces.len(), 2);
    assert_eq!(interfaces[0].key.template, InterfaceTemplateId::new(0));
    assert_eq!(interfaces[1].key.template, InterfaceTemplateId::new(0));
    assert_eq!(interfaces[0].key.arguments, [ResolvedTypeKind::I64]);
    assert_eq!(interfaces[1].key.arguments, [ResolvedTypeKind::Bool]);
    assert_eq!(
        interfaces[0].state,
        GenericInterfaceSpecializationState::Complete(InterfaceId::new(0))
    );
    assert_eq!(
        interfaces[1].state,
        GenericInterfaceSpecializationState::Complete(InterfaceId::new(1))
    );
    assert_eq!(
        interface_origin_texts(&interfaces[0].provenance.origins),
        ["View<i64>", "View<i64>"]
    );
    assert_eq!(
        interface_origin_texts(&interfaces[1].provenance.origins),
        ["View<bool>"]
    );
    assert_eq!(
        interface_origin_ranges(&interfaces[0].provenance.origins),
        [(91, 100), (175, 184)]
    );
    assert_eq!(
        interface_origin_ranges(&interfaces[1].provenance.origins),
        [(207, 217)]
    );

    let dump = dump_resolved(&output.program);
    let specialization_headers = dump
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("Specialization "))
        .collect::<Vec<_>>();
    assert_eq!(
        specialization_headers,
        [
            "Specialization View<i64> interface i0 state complete i0 @0..20",
            "Specialization View<bool> interface i1 state complete i1 @0..20",
            "Specialization Base<shared View<i64>> class c0 state complete c0 @21..48",
            "Specialization Derived<shared View<bool>> class c1 state complete c1 @49..125",
        ]
    );
    assert_eq!(dump, dump_resolved(&output.program));
}

#[test]
fn corrected_local_class_order_reaches_verified_preliminary_mir() {
    let output = resolve_source(
        "class Base<T> { init() {} }\n\
         class Derived<T> extends Base<T> { init() { super(); } }\n\
         fn main() -> i64 {\n\
           var value: shared Base<i64> = new Derived<i64>();\n\
           return 0;\n\
         }\n",
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let classes = output
        .program
        .generic_specializations
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(classes.len(), 2);
    assert_eq!(classes[0].key.template, ClassTemplateId::new(0));
    assert_eq!(classes[1].key.template, ClassTemplateId::new(1));

    let checked = crate::typeck::type_check(&output.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let preliminary = crate::mir::lower_preliminary_hir(
        &checked.hir.expect("successful type checking produces HIR"),
    );
    crate::mir::verify_preliminary_mir(preliminary)
        .expect("local specialization order must produce valid preliminary MIR");
}

#[test]
fn shared_walker_closes_every_type_bearing_source_occurrence_once() {
    let output = resolve_source(TYPE_BEARING_SOURCE_SHAPES);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let box_specialization = output
        .program
        .generic_specializations
        .iter()
        .find(|entry| entry.key.template == ClassTemplateId::new(0))
        .expect("Box<i64> should be discovered");
    assert_eq!(box_specialization.key.arguments, [ResolvedTypeKind::I64]);
    assert!(output.program.generic_specializations.iter().all(|entry| {
        entry.key.template != ClassTemplateId::new(0)
            || entry.key.arguments != [ResolvedTypeKind::Bool]
    }));

    let actual = class_origin_ranges(&box_specialization.provenance.origins);
    let expected = TYPE_BEARING_SOURCE_SHAPES
        .match_indices("Box<i64>")
        .map(|(start, text)| (start, start + text.len()))
        .collect::<Vec<_>>();
    assert_eq!(actual, expected);
}

fn class_origin_texts(origins: &[GenericApplicationOrigin]) -> Vec<&str> {
    origins
        .iter()
        .map(|origin| source_text(origin.span))
        .collect()
}

fn class_origin_ranges(origins: &[GenericApplicationOrigin]) -> Vec<(usize, usize)> {
    origins
        .iter()
        .map(|origin| {
            let range = origin.span.range();
            (range.start(), range.end())
        })
        .collect()
}

fn interface_origin_texts(origins: &[GenericInterfaceApplicationOrigin]) -> Vec<&str> {
    origins
        .iter()
        .map(|origin| source_text(origin.span))
        .collect()
}

fn interface_origin_ranges(origins: &[GenericInterfaceApplicationOrigin]) -> Vec<(usize, usize)> {
    origins
        .iter()
        .map(|origin| {
            let range = origin.span.range();
            (range.start(), range.end())
        })
        .collect()
}

fn source_text(span: Span) -> &'static str {
    &LOCAL_SPECIALIZATION_ORDER_SOURCE[span.range().start()..span.range().end()]
}

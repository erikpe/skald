use super::*;
use crate::{
    hir::{
        HirAccess, HirCallArgument, HirExpressionKind, HirObjectOrigin, HirObjectProducer,
        HirObjectView, HirParameterMode, HirStatement, HirViewSource, HirViewTarget,
    },
    identity::{ClassId, FunctionId},
    resolve::{resolve_module_graph, ResolvedParameterBindingMode, ResolvedTypeKind},
    test_support::load_module_sources_with_standard_library,
};

#[test]
fn checks_aliases_across_calls_owners_forwarding_grouping_and_overlap() {
    let output = check_text(concat!(
        "class Counter {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn get() -> i64 { return self.value; }\n",
        "  mut fn add(delta: i64) -> unit { self.value = self.value + delta; }\n",
        "  fn compare(ref other: Counter) -> i64 { return read(other); }\n",
        "  fn observe_self() -> i64 { return read(self); }\n",
        "  mut fn mutate_self() -> unit { mutate(self, 1); }\n",
        "}\n",
        "class Snapshot {\n",
        "  value: i64;\n",
        "  init(ref source: Counter) { self.value = source.value; }\n",
        "}\n",
        "fn read(ref counter: Counter) -> i64 { return counter.get(); }\n",
        "fn mutate(mut ref counter: Counter, amount: i64) -> unit {\n",
        "  counter.value = counter.value + amount;\n",
        "  counter.add(amount);\n",
        "}\n",
        "fn forward(ref source: Counter, mut ref target: Counter, amount: i64) -> i64 {\n",
        "  mutate((target), amount);\n",
        "  return read(source) + read(target);\n",
        "}\n",
        "fn overlap(mut ref left: Counter, mut ref right: Counter) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var first: Counter = Counter(40);\n",
        "  var second: Counter = Counter(1);\n",
        "  var snapshot: Snapshot = Snapshot(first);\n",
        "  overlap(first, first);\n",
        "  var compared: i64 = first.compare(second);\n",
        "  return forward(first, second, snapshot.value) + compared;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let read = hir.declarations.get(FunctionId::new(0)).unwrap();
    assert_eq!(read.parameters[0].mode, HirParameterMode::ReadOnlyAlias);
    assert_eq!(read.parameters[0].ty, Type::Class(ClassId::new(0)));
    let mutate = hir.declarations.get(FunctionId::new(1)).unwrap();
    assert_eq!(mutate.parameters[0].mode, HirParameterMode::MutableAlias);
    assert_eq!(mutate.parameters[1].mode, HirParameterMode::Value);
    let signature = hir.callable_signature(mutate.id.into()).unwrap();
    assert_eq!(signature.parameters, mutate.parameters);

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Return(return_statement) = main.body.statements.last().unwrap() else {
        panic!("expected final return");
    };
    let HirReturnValue::Scalar(value) = return_statement.value.as_ref().unwrap() else {
        panic!("expected scalar return");
    };
    let HirExpressionKind::Binary { left, .. } = &value.kind else {
        panic!("expected final addition");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &left.kind else {
        panic!("expected forwarding call");
    };
    assert!(matches!(arguments[0], HirCallArgument::View(_)));
    assert!(matches!(arguments[1], HirCallArgument::View(_)));
    assert!(matches!(arguments[2], HirCallArgument::Value(_)));
    let (_, first) = class_alias_view(&arguments[0]);
    assert_eq!(first.access, HirAccess::Mutable);
}

#[test]
fn enforces_read_only_alias_field_method_and_forwarding_restrictions() {
    let output = check_text(concat!(
        "class Counter {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
        "}\n",
        "fn mutate(mut ref counter: Counter) -> unit {}\n",
        "fn misuse(ref counter: Counter) -> unit {\n",
        "  counter.value = 1;\n",
        "  counter.add(1);\n",
        "  mutate(counter);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            READ_ONLY_RECEIVER,
            READ_ONLY_RECEIVER,
            INSUFFICIENT_ALIAS_ACCESS
        ]
    );
}

#[test]
fn checks_nominal_type_and_rejects_non_object_alias_sources() {
    let output = check_text(concat!(
        "class Left { value: i64; init() { self.value = 0; } }\n",
        "class Right { value: i64; init() { self.value = 0; } }\n",
        "fn take(ref value: Left) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var right: Right = Right();\n",
        "  var scalar: i64 = 1;\n",
        "  take(right);\n",
        "  take(scalar);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(codes, [TYPE_MISMATCH, INVALID_ALIAS_ARGUMENT]);
}

#[test]
fn accepts_exact_class_producers_for_read_only_aliases_across_call_forms() {
    let output = check_text(concat!(
        "interface Named {}\n",
        "interface Producer { fn make() -> Derived; }\n",
        "class Base { init() {} }\n",
        "class Derived extends Base implements Named { init() { super(); } }\n",
        "class Factory implements Producer {\n",
        "  init() {}\n",
        "  static fn make_static() -> Derived { return Derived(); }\n",
        "  fn make() -> Derived { return Derived(); }\n",
        "}\n",
        "class Consumer { init(ref value: Derived) {} }\n",
        "fn make_direct() -> Derived { return Derived(); }\n",
        "fn take_exact(ref value: Derived) -> unit {}\n",
        "fn take_base(ref value: Base) -> unit {}\n",
        "fn take_named(ref value: Named) -> unit {}\n",
        "fn take_obj(ref value: Obj) -> unit {}\n",
        "fn from_interface(ref producer: Producer) -> unit { take_exact(producer.make()); }\n",
        "fn main() -> i64 {\n",
        "  var factory: Factory = Factory();\n",
        "  take_exact(Derived());\n",
        "  take_base((Derived()));\n",
        "  take_named(Factory.make_static());\n",
        "  take_obj(factory.make());\n",
        "  from_interface(factory);\n",
        "  take_base((Base) Derived());\n",
        "  var consumer: Consumer = Consumer(make_direct());\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let dump = dump_hir(&hir);
    assert_eq!(dump.matches("ProducedView").count(), 7, "{dump}");
    assert!(dump.contains("ObjectCall function"), "{dump}");
    assert!(dump.contains("ObjectCall static"), "{dump}");
    assert!(dump.contains("ObjectCall method"), "{dump}");
    assert!(dump.contains("ObjectCall interface"), "{dump}");

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Call(exact) = &main.body.statements[1] else {
        panic!("expected direct alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &exact.call.kind else {
        panic!("expected direct alias call expression");
    };
    let HirCallArgument::View(view) = &arguments[0] else {
        panic!("expected produced alias view");
    };
    assert_eq!(view.access, HirAccess::ReadOnly);
    assert_eq!(view.target, HirViewTarget::Class(ClassId::new(1)));
    assert!(matches!(view.source, HirViewSource::Produced { .. }));
    assert!(matches!(
        view.origin.as_ref(),
        HirObjectOrigin::Produced {
            dynamic_class,
            ..
        } if *dynamic_class == ClassId::new(1)
    ));

    let HirStatement::Call(base) = &main.body.statements[2] else {
        panic!("expected produced ancestor alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &base.call.kind else {
        panic!("expected direct ancestor alias call");
    };
    let HirCallArgument::View(HirObjectView {
        source: HirViewSource::Produced { projections, .. },
        ..
    }) = &arguments[0]
    else {
        panic!("expected projected produced alias view");
    };
    assert_eq!(
        projections,
        &[crate::object_path::ObjectProjection::Base(ClassId::new(0))]
    );
}

#[test]
fn accepts_string_literal_producers_as_obj_aliases() {
    let (_workspace, graph) = load_module_sources_with_standard_library(
        "app",
        &[(
            "app.ska",
            concat!(
                "fn inspect(ref value: Obj) -> i64 { return 1; }\n",
                "fn main() -> i64 { return inspect(\"temporary\"); }\n",
            ),
        )],
    );
    let resolved = resolve_module_graph(&graph);
    assert!(!resolved.has_errors(), "{:?}", resolved.diagnostics);
    let output = type_check(&resolved.program);
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirExpressionKind::DirectCall { arguments, .. } = &returned_expression(main).kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::View(view) = &arguments[0] else {
        panic!("expected produced string view");
    };
    assert_eq!(view.target, HirViewTarget::Obj);
    assert_eq!(view.access, HirAccess::ReadOnly);
    assert!(matches!(
        &view.source,
        HirViewSource::Produced { producer, .. }
            if matches!(&**producer, HirObjectProducer::StringLiteral(_))
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("ViewArgument -> Obj readonly"), "{dump}");
    assert!(dump.contains("ProducedView @"), "{dump}");
    assert!(dump.contains("StringLiteral"), "{dump}");
}

#[test]
fn produced_objects_cannot_bind_mutable_aliases_even_through_casts() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn mutate(mut ref value: Value) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  mutate(Value(1));\n",
        "  mutate((Value) Value());\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_ALIAS_ARGUMENT));
    assert!(diagnostics
        .iter()
        .all(|diagnostic| diagnostic.message.contains("existing object place")));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic
        .labels
        .iter()
        .any(|label| label.message == "mutable alias declared here")));
}

#[test]
fn rejects_excluded_producer_families_with_parameter_context() {
    let output = check_text(concat!(
        "class Base { init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "class Other { init() {} }\n",
        "fn unrelated(ref value: Other) -> unit {}\n",
        "fn downcast(ref value: Derived) -> unit {}\n",
        "fn primitive(ref value: i64) -> unit {}\n",
        "fn array(ref value: i64[]) -> unit {}\n",
        "fn optional(ref value: Base?) -> unit {}\n",
        "fn make_optional() -> Base? { return Base(); }\n",
        "fn main() -> i64 {\n",
        "  unrelated(Base());\n",
        "  downcast(Base());\n",
        "  primitive(1);\n",
        "  array(i64[]());\n",
        "  optional(make_optional());\n",
        "  var owner: shared Base = new Base();\n",
        "  unrelated(owner);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 6, "{diagnostics:#?}");
    assert_eq!(diagnostics[0].code, TYPE_MISMATCH);
    assert_eq!(diagnostics[1].code, TYPE_MISMATCH);
    assert_eq!(diagnostics[2].code, INVALID_ALIAS_ARGUMENT);
    assert_eq!(diagnostics[3].code, INVALID_ALIAS_ARGUMENT);
    assert_eq!(diagnostics[4].code, INVALID_ALIAS_ARGUMENT);
    assert_eq!(diagnostics[5].code, IMPLICIT_SHARED_DEREFERENCE);
    assert!(
        diagnostics.iter().all(|diagnostic| diagnostic
            .labels
            .iter()
            .any(|label| label.message.contains("declared here"))),
        "{diagnostics:#?}"
    );
}

#[test]
fn aliases_are_copy_sources_but_remain_invalid_as_scalar_values_and_returns() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn scalar(value: i64) -> unit {}\n",
        "fn misuse(ref value: Value) -> i64 {\n",
        "  scalar(value);\n",
        "  var copy: Value = value;\n",
        "  return value;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(codes, [INVALID_OBJECT_CONTEXT, INVALID_OBJECT_CONTEXT]);
}

#[test]
fn rejects_initializer_self_as_an_alias_source() {
    let output = check_text(concat!(
        "class Value { value: i64; init() { self.value = read(self); } }\n",
        "fn read(ref value: Value) -> i64 { return value.value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_ALIAS_ARGUMENT));
}

#[test]
fn diagnoses_wrong_arity_for_alias_calls() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn take(ref value: Value, amount: i64) -> unit {}\n",
        "fn main() -> i64 { var value: Value = Value(); take(value); return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        WRONG_ARGUMENT_COUNT
    );
}

#[test]
fn rejects_external_aliases_and_accepts_primitive_alias_signatures() {
    let external = check_text(concat!(
        "class Value { init() {} }\n",
        "extern fn imported(ref value: Value) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(external.hir.is_none());
    assert_eq!(external.diagnostics.len(), 1);
    assert_eq!(
        external.diagnostics.iter().next().unwrap().code,
        INVALID_EXTERNAL_DECLARATION
    );
    assert!(external
        .diagnostics
        .iter()
        .next()
        .unwrap()
        .message
        .contains("alias parameters"));

    let mut resolved = resolve_text(concat!(
        "class Value { init() {} }\n",
        "fn take(ref value: Value) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let take = &mut resolved.declarations.entries_mut_for_test()[0];
    assert!(matches!(
        take.parameters[0].binding_mode,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. }
    ));
    take.parameters[0].type_syntax.kind = ResolvedTypeKind::I64;
    let corrupt = type_check(&resolved);
    assert!(corrupt.diagnostics.is_empty(), "{:?}", corrupt.diagnostics);
    assert!(corrupt.hir.is_some());
}

#[test]
fn alias_hir_dump_is_exact_and_records_modes_arguments_and_access() {
    let output = check_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn read(ref box: Box) -> i64 { return box.value; }\n",
        "fn main() -> i64 { var box: Box = Box(7); return read((box)); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..182\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f1\n",
            "  Classes\n",
            "    Class c0 module m0 \"Box\" @0..66\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" : i64 @12..23\n",
            "      Initializers\n",
            "        Initializer c0:init0 @24..64\n",
            "          Parameter c0:init0:p0 \"value\" value : i64 @29..39\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "          Scalar c0:field0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "          Scalar c0:field0\n",
            "      Methods\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..66\n",
            "      MemberDefinition c0:init0 @24..64\n",
            "        Locals\n",
            "        Block @41..64\n",
            "          FieldAssignment @43..62\n",
            "            FieldPlace c0:field0 @43..62\n",
            "              ObjectPlace c0:init0:self : class c0 mutable @43..47\n",
            "            Binding c0:init0:p0 : i64 @56..61\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"read\" internal @67..117\n",
            "      Parameters\n",
            "        Parameter f0:p0 \"box\" ref : class c0 @75..87\n",
            "      ReturnType i64\n",
            "    Declaration f1 module m0 \"main\" internal @118..181\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @67..117\n",
            "      Locals\n",
            "      Block @96..117\n",
            "        Return @98..115\n",
            "          FieldRead c0:field0 : i64 @105..114\n",
            "            ObjectPlace f0:p0 : class c0 readonly @105..108\n",
            "    Definition f1 @118..181\n",
            "      Locals\n",
            "        Local f1:l0 \"box\" : class c0 @137..159\n",
            "      Block @135..181\n",
            "        LocalDeclaration f1:l0 @137..159\n",
            "          ObjectInitialization @152..158\n",
            "            ObjectPlace f1:l0 : class c0 mutable @141..144\n",
            "            Construct c0 via c0:init0 @152..158\n",
            "              ValueArgument @156..157\n",
            "                Integer 7 : i64 @156..157\n",
            "            ElidedCopy\n",
            "              Operation Synthesized c0\n",
            "        Return @160..179\n",
            "          DirectCall f0 : i64 @167..178\n",
            "            ViewArgument -> class c0 readonly @172..177\n",
            "              ObjectPlace f1:l0 : class c0 mutable @172..177\n",
            "              Origin Exact dynamic c0\n",
            "                ObjectPlace f1:l0 : class c0 mutable @172..177\n",
        )
    );
}

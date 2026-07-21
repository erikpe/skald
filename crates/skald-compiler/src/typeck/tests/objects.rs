use super::*;
use crate::{
    diagnostics::Diagnostics,
    hir::{HirAccess, HirCallArgument, HirLocalInitializer},
    identity::{BindingId, ClassId, FieldId, FunctionId, InitializerId, MethodId},
    typeck::function::{CallableChecker, MemberCheckContext, ReceiverContext},
};

#[test]
fn checks_construction_fields_methods_and_all_callable_owners() {
    let output = check_text(concat!(
        "class Counter {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
        "  fn get() -> i64 { return self.value; }\n",
        "}\n",
        "fn main() -> i64 { var counter: Counter = Counter(40); counter.add(2); return counter.get(); }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(class.fields[0].id, FieldId::new(class.id, 0));
    assert_eq!(class.initializer.id, InitializerId::new(class.id, 0));
    assert_eq!(class.methods[0].id, MethodId::new(class.id, 0));
    assert_eq!(class.methods[0].receiver_access, HirAccess::Mutable);
    assert_eq!(class.methods[1].receiver_access, HirAccess::ReadOnly);

    let initializer = hir.member_definition(class.initializer.id.into()).unwrap();
    let HirStatement::FieldAssignment(assignment) = &initializer.body.statements[0] else {
        panic!("expected typed field initialization");
    };
    assert_eq!(assignment.place.field, class.fields[0].id);
    assert_eq!(
        assignment.place.receiver.root(),
        BindingId::Receiver(initializer.callable)
    );

    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert_eq!(main.locals[0].ty, Type::Class(class.id));
    let HirStatement::Local(local) = &main.body.statements[0] else {
        panic!("expected object local");
    };
    let HirLocalInitializer::Construct(construction) = &local.initializer else {
        panic!("expected destination construction");
    };
    assert_eq!(construction.class, class.id);
    assert_eq!(construction.initializer, class.initializer.id);
}

#[test]
fn diagnoses_missing_duplicate_and_premature_field_initialization() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  first: i64; second: i64; missing: i64;\n",
        "  init() {\n",
        "    self.first = self.second;\n",
        "    self.second = 1;\n",
        "    self.second = 2;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let field_errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == FIELD_INITIALIZATION)
        .collect();
    assert_eq!(field_errors.len(), 3);
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("before initialization")));
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("more than once")));
    assert!(field_errors
        .iter()
        .any(|error| error.message.contains("not initialized")));
}

#[test]
fn enforces_initializer_shape_and_field_types() {
    let output = check_text(concat!(
        "class Broken {\n",
        "  value: i64;\n",
        "  init() { if (true) { self.value = 1; } }\n",
        "}\n",
        "class WrongType { value: i64; init() { self.value = true; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == FIELD_INITIALIZATION));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
}

#[test]
fn requires_one_explicit_initializer_even_for_empty_classes() {
    let output = check_text("class Empty {} fn main() -> i64 { return 0; }");

    assert!(output.hir.is_none());
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, INVALID_OBJECT_DECLARATION);
    assert!(diagnostic.message.contains("explicit initializer"));
}

#[test]
fn rejects_object_bearing_value_parameters_and_results_at_the_type_boundary() {
    let mut resolved = resolve_text(concat!(
        "class Other { init() {} }\n",
        "class Value {\n",
        "  field: i64;\n",
        "  init(value: i64) { self.field = value; }\n",
        "  fn get() -> i64 { return self.field; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let class = &mut resolved.classes.entries_mut_for_test()[1];
    class.fields[0].type_syntax.kind = crate::resolve::ResolvedTypeKind::Class(ClassId::new(0));
    class.initializer.as_mut().unwrap().parameters[0]
        .type_syntax
        .kind = crate::resolve::ResolvedTypeKind::Class(class.id);
    class.methods[0].return_type.kind = crate::resolve::ResolvedTypeKind::Class(class.id);

    let output = type_check(&resolved);

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_DECLARATION)
            .count(),
        2
    );
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code != RECURSIVE_INLINE_CONTAINMENT));
}

#[test]
fn lowers_nested_object_places_with_one_root_capability_and_identity_path() {
    let resolved = resolve_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } fn read() -> i64 { return self.value; } }\n",
        "class Link { leaf: Leaf; init() {} }\n",
        "class Root { link: Link; init() {} fn nested() -> i64 { return self.link.leaf.value; } }\n",
        "fn take(ref leaf: Leaf) -> i64 { return leaf.value; }\n",
        "fn inspect(ref root: Root) -> i64 { return take(((root.link).leaf)); }\n",
        "fn inspect_mut(mut ref root: Root) -> i64 { return root.link.leaf.read(); }\n",
        "fn local() -> i64 { var root: Root = Root(); return root.link.leaf.read(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let expected = [
        FieldId::new(ClassId::new(2), 0),
        FieldId::new(ClassId::new(1), 0),
    ];

    let inspect_declaration = resolved.declarations.get(FunctionId::new(1)).unwrap();
    let inspect_definition = resolved.definitions.get(FunctionId::new(1)).unwrap();
    let grouped_span = match &inspect_definition.body.statements[0] {
        crate::resolve::ResolvedStatement::Return(statement) => match &statement.value {
            Some(crate::resolve::ResolvedExpression::DirectCall(call)) => call.arguments[0].span(),
            _ => panic!("expected forwarding call"),
        },
        _ => panic!("expected return statement"),
    };
    let mut diagnostics = Diagnostics::new();
    let inspect = CallableChecker::new(
        &resolved,
        inspect_declaration,
        inspect_definition,
        &mut diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::DirectCall { arguments, .. } = &returned_expression(&inspect).kind
    else {
        panic!("expected typed forwarding call");
    };
    let HirCallArgument::Place(place) = &arguments[0] else {
        panic!("expected projected alias place");
    };
    assert_eq!(
        place.root(),
        BindingId::Parameter(inspect_declaration.parameters[0].id)
    );
    assert_eq!(place.projections(), expected);
    assert_eq!(place.class(), ClassId::new(0));
    assert_eq!(place.access, HirAccess::ReadOnly);
    assert_eq!(place.span(), grouped_span);

    let mutable_declaration = resolved.declarations.get(FunctionId::new(2)).unwrap();
    let mutable_definition = resolved.definitions.get(FunctionId::new(2)).unwrap();
    let mut diagnostics = Diagnostics::new();
    let mutable = CallableChecker::new(
        &resolved,
        mutable_declaration,
        mutable_definition,
        &mut diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::MethodCall { receiver, .. } = &returned_expression(&mutable).kind else {
        panic!("expected nested method receiver");
    };
    assert_eq!(receiver.projections(), expected);
    assert_eq!(receiver.access, HirAccess::Mutable);

    let local_declaration = resolved.declarations.get(FunctionId::new(3)).unwrap();
    let local_definition = resolved.definitions.get(FunctionId::new(3)).unwrap();
    let mut diagnostics = Diagnostics::new();
    let local = CallableChecker::new(
        &resolved,
        local_declaration,
        local_definition,
        &mut diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::MethodCall { receiver, .. } = &returned_expression(&local).kind else {
        panic!("expected local nested method receiver");
    };
    assert_eq!(receiver.projections(), expected);
    assert_eq!(receiver.root(), BindingId::Local(local.locals[0].id));
    assert_eq!(receiver.access, HirAccess::Mutable);

    let class = resolved.classes.get(ClassId::new(2)).unwrap();
    let method = &class.methods[0];
    let definition = &resolved.class_definitions.get(class.id).unwrap().methods[0];
    let mut diagnostics = Diagnostics::new();
    let member = CallableChecker::new_member(
        &resolved,
        MemberCheckContext {
            callable: method.id.into(),
            parameters: &method.parameters,
            definition,
            return_type: Type::I64,
            receiver: ReceiverContext {
                class: class.id,
                access: HirAccess::ReadOnly,
                initializer: false,
            },
            callable_name: "method `nested`".to_owned(),
        },
        &mut diagnostics,
    )
    .check_member();
    assert!(diagnostics.is_empty());
    let HirStatement::Return(statement) = &member.body.statements[0] else {
        panic!("expected member return");
    };
    let HirExpressionKind::FieldRead(field) = &statement.value.as_ref().unwrap().kind else {
        panic!("expected nested self field read");
    };
    assert_eq!(field.receiver.projections(), expected);
    assert_eq!(field.receiver.root(), BindingId::Receiver(method.id.into()));
    assert_eq!(field.receiver.access, HirAccess::ReadOnly);
}

#[test]
fn class_field_selection_does_not_create_an_object_rvalue() {
    let resolved = resolve_text(concat!(
        "class Leaf { init() {} }\n",
        "class Root { leaf: Leaf; init() {} }\n",
        "fn invalid(ref root: Root) -> i64 { var value: i64 = root.leaf; return value; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let declaration = resolved.declarations.get(FunctionId::new(0)).unwrap();
    let definition = resolved.definitions.get(FunctionId::new(0)).unwrap();
    let mut diagnostics = Diagnostics::new();

    let _ = CallableChecker::new(&resolved, declaration, definition, &mut diagnostics).check();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("is not a value")));
}

#[test]
fn validates_exact_direct_construction_and_constructor_arguments() {
    let output = check_text(concat!(
        "class Left { init(value: i64) {} }\n",
        "class Right { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var wrong: Left = Right();\n",
        "  var grouped: Left = (Left(1));\n",
        "  var arity: Left = Left();\n",
        "  var typed: Left = Left(true);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == WRONG_ARGUMENT_COUNT));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
}

#[test]
fn rejects_object_values_copying_and_general_construction() {
    let output = check_text(concat!(
        "class Value { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var first: Value = Value();\n",
        "  var copy: Value = first;\n",
        "  var scalar: i64 = Value();\n",
        "  return first;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT)
            .count(),
        2
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION));
}

#[test]
fn enforces_read_only_and_mutable_receiver_access() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init() { self.field = 0; }\n",
        "  mut fn set(value: i64) -> unit { self.field = value; }\n",
        "  fn write() -> unit { self.field = 1; }\n",
        "  fn forward() -> unit { self.set(2); }\n",
        "}\n",
        "fn main() -> i64 { var value: Value = Value(); value.set(3); return value.field; }\n",
    ));

    assert!(output.hir.is_none());
    let errors: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER)
        .collect();
    assert_eq!(errors.len(), 2);
}

#[test]
fn initializer_cannot_call_a_method_before_the_receiver_is_live() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init() { self.field = self.get(); }\n",
        "  fn get() -> i64 { return 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INITIALIZER_BODY));
}

#[test]
fn methods_reuse_structured_definite_return_analysis() {
    let output = check_text(concat!(
        "class Value {\n",
        "  init() {}\n",
        "  fn complete(flag: bool) -> i64 { if (flag) { return 1; } else { return 2; } }\n",
        "  fn missing(flag: bool) -> i64 { if (flag) { return 1; } }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    let missing: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == MISSING_RETURN)
        .collect();
    assert_eq!(missing.len(), 1);
    assert!(missing[0].message.contains("method `missing`"));
}

#[test]
fn lowers_alias_signatures_for_every_internal_owner() {
    let output = check_text(concat!(
        "class Thing {\n",
        "  init(ref other: Thing) {}\n",
        "  fn inspect(mut ref other: Thing) -> unit {}\n",
        "}\n",
        "fn take(ref thing: Thing) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert_eq!(
        hir.declarations.get(FunctionId::new(0)).unwrap().parameters[0].mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    let class = hir.class(ClassId::new(0)).unwrap();
    assert_eq!(
        class.initializer.parameters[0].mode,
        crate::hir::HirParameterMode::ReadOnlyAlias
    );
    assert_eq!(
        class.methods[0].parameters[0].mode,
        crate::hir::HirParameterMode::MutableAlias
    );
}

#[test]
fn object_hir_dump_is_exact_and_identity_based() {
    let output = check_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Box = Box(1); return value.get(); }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..172\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 \"Box\" @0..105\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" : i64 @12..23\n",
            "      Initializer c0:init0 @24..64\n",
            "        Parameter c0:init0:p0 \"value\" value : i64 @29..39\n",
            "      Methods\n",
            "        Method c0:method0 \"get\" readonly -> i64 @65..103\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..105\n",
            "      MemberDefinition c0:init0 @24..64\n",
            "        Locals\n",
            "        Block @41..64\n",
            "          FieldAssignment @43..62\n",
            "            FieldPlace c0:field0 @43..62\n",
            "              ObjectPlace c0:init0:self : class c0 mutable @43..47\n",
            "            Binding c0:init0:p0 : i64 @56..61\n",
            "      MemberDefinition c0:method0 @65..103\n",
            "        Locals\n",
            "        Block @81..103\n",
            "          Return @83..101\n",
            "            FieldRead c0:field0 : i64 @90..100\n",
            "              ObjectPlace c0:method0:self : class c0 readonly @90..94\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @106..171\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @106..171\n",
            "      Locals\n",
            "        Local f0:l0 \"value\" : class c0 @125..149\n",
            "      Block @123..171\n",
            "        LocalDeclaration f0:l0 @125..149\n",
            "          Construct c0 via c0:init0 @142..148\n",
            "            ValueArgument @146..147\n",
            "              Integer 1 : i64 @146..147\n",
            "        Return @150..169\n",
            "          MethodCall c0:method0 : i64 @157..168\n",
            "            ObjectPlace f0:l0 : class c0 mutable @157..162\n",
        )
    );
}

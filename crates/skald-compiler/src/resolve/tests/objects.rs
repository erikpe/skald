use super::*;
use crate::identity::FieldId;

fn class(output: &ResolveOutput, index: usize) -> &ResolvedClassDeclaration {
    output
        .program
        .classes
        .get(ClassId::new(index))
        .expect("expected resolved class")
}

#[test]
fn resolves_forward_classes_members_construction_and_all_callable_owners() {
    let output = resolve_text(concat!(
        "fn main() -> i64 {\n",
        "    var counter: Counter = Counter(40);\n",
        "    counter.add(2);\n",
        "    return counter.forwarded();\n",
        "}\n",
        "class Counter {\n",
        "    value: i64;\n",
        "    init(value: i64) { self.value = value; }\n",
        "    mut fn add(delta: i64) -> unit {\n",
        "        var next: i64 = self.value + delta;\n",
        "        self.value = next;\n",
        "    }\n",
        "    fn get() -> i64 { return self.value; }\n",
        "    fn forwarded() -> i64 { return self.get(); }\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let counter = class(&output, 0);
    assert_eq!(counter.id, ClassId::new(0));
    assert_eq!(counter.fields[0].id, FieldId::new(counter.id, 0));
    assert_eq!(
        counter.initializer.as_ref().unwrap().id,
        InitializerId::new(counter.id, 0)
    );
    assert_eq!(counter.methods[0].id, MethodId::new(counter.id, 0));
    assert_eq!(counter.methods[1].id, MethodId::new(counter.id, 1));
    assert_eq!(counter.methods[2].id, MethodId::new(counter.id, 2));
    assert_eq!(
        counter.methods[0].receiver_access,
        ResolvedReceiverAccess::Mutable
    );
    assert_eq!(
        counter.methods[1].receiver_access,
        ResolvedReceiverAccess::ReadOnly
    );

    let class_definition = output.program.class_definitions.get(counter.id).unwrap();
    let initializer = class_definition.initializer.as_ref().unwrap();
    assert_eq!(
        initializer.callable,
        counter.initializer.as_ref().unwrap().id.into()
    );
    let ResolvedStatement::FieldAssignment(initial_assignment) = &initializer.body.statements[0]
    else {
        panic!("expected initializer field assignment");
    };
    assert_eq!(initial_assignment.field, counter.fields[0].id);
    assert_eq!(
        initial_assignment.receiver.binding,
        BindingId::Receiver(initializer.callable)
    );

    let add = &class_definition.methods[0];
    assert_eq!(add.locals[0].id.callable(), add.callable);
    let forwarded = &class_definition.methods[2];
    let ResolvedExpression::MethodCall(forwarded_call) =
        return_value(&forwarded.body.statements[0])
    else {
        panic!("expected self method call");
    };
    assert_eq!(forwarded_call.method, counter.methods[1].id);
    assert_eq!(
        forwarded_call.receiver.binding,
        BindingId::Receiver(forwarded.callable)
    );

    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    assert_eq!(
        main.locals[0].type_syntax.kind,
        ResolvedTypeKind::Class(counter.id)
    );
    let ResolvedExpression::Construct(construct) = local_initializer(&main.body.statements[0])
    else {
        panic!("expected construction");
    };
    assert_eq!(construct.class, counter.id);
    assert_eq!(
        construct.initializer,
        counter.initializer.as_ref().unwrap().id
    );
}

#[test]
fn top_level_and_member_namespaces_reject_cross_kind_duplicates() {
    let output = resolve_text(concat!(
        "class Same { init() {} }\n",
        "fn Same() -> unit {}\n",
        "class Members {\n",
        "    value: i64;\n",
        "    fn value() -> i64 { return 0; }\n",
        "    init() {}\n",
        "    init(value: i64) {}\n",
        "    fn get() -> i64 { return 1; }\n",
        "    fn get(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            DUPLICATE_TOP_LEVEL,
            DUPLICATE_MEMBER,
            DUPLICATE_MEMBER,
            DUPLICATE_MEMBER,
        ]
    );
    assert_eq!(output.program.declarations.len(), 1);
    assert_eq!(output.program.classes.len(), 2);
    let members = class(&output, 1);
    assert_eq!(members.fields.len(), 1);
    assert!(members.methods.iter().any(|method| method.name == "get"));
    assert!(!members.methods.iter().any(|method| method.name == "value"));
}

#[test]
fn identical_member_names_are_independent_between_owners() {
    let output = resolve_text(concat!(
        "class Left { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "class Right { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var value: Right = Right(1); return value.get(); }\n",
    ));

    assert!(!output.has_errors());
    assert_eq!(class(&output, 0).fields[0].id.class(), ClassId::new(0));
    assert_eq!(class(&output, 1).fields[0].id.class(), ClassId::new(1));
}

#[test]
fn resolves_named_field_types_through_the_top_level_class_table() {
    let output = resolve_text(concat!(
        "class Outer { child: Inner; init() {} }\n",
        "class Inner { init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let outer = class(&output, 0);
    let inner = class(&output, 1);
    assert_eq!(
        outer.fields[0].type_syntax.kind,
        ResolvedTypeKind::Class(inner.id)
    );
    assert!(dump_resolved(&output.program).contains(concat!(
        "Field c0:field0 \"child\"",
        " @14..27\n",
        "          Type Class c1 @21..26"
    )));
}

#[test]
fn diagnoses_unknown_and_non_class_named_field_types() {
    let unknown = resolve_text(concat!(
        "class Holder { value: Missing; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(unknown.diagnostics.len(), 1);
    assert_eq!(
        unknown.diagnostics.iter().next().unwrap().code,
        UNKNOWN_TYPE
    );

    let function = resolve_text(concat!(
        "fn NotAClass() -> i64 { return 0; }\n",
        "class Holder { value: NotAClass; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert_eq!(function.diagnostics.len(), 1);
    let diagnostic = function.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, UNKNOWN_TYPE);
    assert!(diagnostic.message.contains("does not name a class"));
}

#[test]
fn diagnoses_unknown_types_members_and_wrong_owner_selection() {
    let unknown_type = resolve_text("fn main() -> i64 { var missing: Missing = 0; return 0; }");
    assert_eq!(unknown_type.diagnostics.len(), 1);
    assert_eq!(
        unknown_type.diagnostics.iter().next().unwrap().code,
        UNKNOWN_TYPE
    );

    let output = resolve_text(concat!(
        "class Left { left: i64; init() { self.left = 0; } }\n",
        "class Right { right: i64; init() { self.right = 0; } fn wrong() -> i64 { return self.left; } }\n",
        "fn main() -> i64 { var value: Left = Left(); return value.missing; }\n",
    ));
    assert_eq!(output.diagnostics.len(), 2);
    assert!(output
        .diagnostics
        .iter()
        .all(|diagnostic| diagnostic.code == UNKNOWN_MEMBER));
}

#[test]
fn self_is_scoped_to_initializers_and_methods() {
    let output = resolve_text("fn main() -> i64 { return self.value; }");
    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        SELF_OUTSIDE_MEMBER
    );
}

#[test]
fn local_bindings_shadow_callable_class_names_but_not_type_names() {
    let output = resolve_text(concat!(
        "class Counter { init() {} }\n",
        "fn main() -> i64 {\n",
        "    var Counter: i64 = 0;\n",
        "    var value: Counter = Counter();\n",
        "    return 0;\n",
        "}\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        INVALID_CALL_TARGET
    );
    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    assert_eq!(
        main.locals[1].type_syntax.kind,
        ResolvedTypeKind::Class(ClassId::new(0))
    );
}

#[test]
fn diagnoses_invalid_member_kinds_receivers_and_missing_initializers() {
    let output = resolve_text(concat!(
        "class Empty {}\n",
        "class Value { field: i64; init() { self.field = 0; } fn method() -> i64 { return self.field; } }\n",
        "fn main() -> i64 {\n",
        "    var scalar: i64 = 0;\n",
        "    var value: Value = Value();\n",
        "    value.field();\n",
        "    var method: i64 = value.method;\n",
        "    var missing: Empty = Empty();\n",
        "    return scalar.field;\n",
        "}\n",
    ));

    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert_eq!(
        codes,
        [
            INVALID_CALL_TARGET,
            INVALID_MEMBER_SELECTION,
            INVALID_CONSTRUCTION_TARGET,
            INVALID_MEMBER_SELECTION,
        ]
    );
}

#[test]
fn resolved_object_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "class Box { value: i64; init(value: i64) { self.value = value; } fn get() -> i64 { return self.value; } }\n",
        "fn main() -> i64 { var box: Box = Box(1); return box.get(); }\n",
    ));
    assert!(!output.has_errors());

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..168\n",
            "  Entry f0\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Box\" @0..105\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" @12..23\n",
            "          Type I64 @19..22\n",
            "      Initializer\n",
            "        Initializer c0:init0 @24..64\n",
            "          Parameters\n",
            "            Parameter c0:init0:p0 \"value\" @29..39\n",
            "              Binding Value\n",
            "              Type I64 @36..39\n",
            "      Methods\n",
            "        Method c0:method0 readonly \"get\" @65..103\n",
            "          Parameters\n",
            "          ReturnType\n",
            "            Type I64 @77..80\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @106..167\n",
            "      Parameters\n",
            "      ReturnType\n",
            "        Type I64 @119..122\n",
            "  Definitions\n",
            "    Definition f0 @106..167\n",
            "      Locals\n",
            "        Local f0:l0 \"box\" @125..147\n",
            "          Type Class c0 @134..137\n",
            "      Block @123..167\n",
            "        LocalDeclaration f0:l0 @125..147\n",
            "          Construct c0 with c0:init0 @140..146\n",
            "            Integer \"1\" @144..145\n",
            "        Return @148..165\n",
            "          MethodCall c0:method0 @155..164\n",
            "            Receiver f0:l0 class c0 @155..158\n",
            "            Arguments\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..105\n",
            "      MemberDefinition c0:init0 @24..64\n",
            "        Locals\n",
            "        Block @41..64\n",
            "          FieldAssignment c0:field0 @43..62\n",
            "            Receiver c0:init0:self class c0 @43..47\n",
            "            Equal @54..55\n",
            "            Value\n",
            "              Binding c0:init0:p0 @56..61\n",
            "      MemberDefinition c0:method0 @65..103\n",
            "        Locals\n",
            "        Block @81..103\n",
            "          Return @83..101\n",
            "            FieldAccess c0:field0 @90..100\n",
            "              Receiver c0:method0:self class c0 @90..94\n",
        )
    );
}

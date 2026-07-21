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
        initial_assignment.receiver.root,
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
        forwarded_call.receiver.root,
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
fn resolves_copy_lifecycle_slots_to_stable_owner_qualified_identities() {
    let output = resolve_text(concat!(
        "class Value {\n",
        "    value: i64;\n",
        "    init(ref other: Value) { self.value = other.value; }\n",
        "    init(value: i64) { self.value = value; }\n",
        "    assign(ref source: Value) { self.value = source.value; }\n",
        "}\n",
        "class Empty { init() {} }\n",
        "fn main() -> i64 { var value: Value = Value(1); return value.value; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let value = class(&output, 0);
    let ordinary = value.initializer.as_ref().unwrap();
    let copy_constructor = value.copy_constructor_declaration.as_ref().unwrap();
    let copy_assignment = value.copy_assignment_declaration.as_ref().unwrap();
    assert_eq!(copy_constructor.id, InitializerId::new(value.id, 0));
    assert_eq!(ordinary.id, InitializerId::new(value.id, 1));
    assert_eq!(copy_assignment.id, CopyAssignmentId::new(value.id, 0));
    assert_eq!(
        value.copy_constructor,
        ResolvedCopyOperation::User(copy_constructor.id)
    );
    assert_eq!(
        value.copy_assignment,
        ResolvedCopyOperation::User(copy_assignment.id)
    );

    let definition = output.program.class_definitions.get(value.id).unwrap();
    let copy_constructor_body = definition.copy_constructor.as_ref().unwrap();
    let copy_assignment_body = definition.copy_assignment.as_ref().unwrap();
    assert_eq!(copy_constructor_body.callable, copy_constructor.id.into());
    assert_eq!(copy_assignment_body.callable, copy_assignment.id.into());
    let ResolvedStatement::FieldAssignment(assignment) = &copy_assignment_body.body.statements[0]
    else {
        panic!("expected the copy-assignment body to resolve");
    };
    assert_eq!(
        assignment.receiver.root,
        BindingId::Receiver(copy_assignment.id.into())
    );
    let ResolvedExpression::FieldAccess(source) = &assignment.value else {
        panic!("expected source field access");
    };
    assert_eq!(
        source.receiver.root,
        BindingId::Parameter(copy_assignment.parameter.id)
    );

    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedExpression::Construct(construct) = local_initializer(&main.body.statements[0])
    else {
        panic!("expected ordinary construction");
    };
    assert_eq!(construct.initializer, ordinary.id);

    let empty = class(&output, 1);
    assert_eq!(
        empty.copy_constructor,
        ResolvedCopyOperation::Synthesized(empty.id)
    );
    assert_eq!(
        empty.copy_assignment,
        ResolvedCopyOperation::Synthesized(empty.id)
    );

    let dump = dump_resolved(&output.program);
    let identity_lines: Vec<_> = dump
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("User c0:") || line.starts_with("MemberDefinition c0:"))
        .map(|line| line.split(" @").next().unwrap())
        .collect();
    assert_eq!(
        identity_lines,
        [
            "User c0:init0",
            "User c0:assign0",
            "MemberDefinition c0:init1",
            "MemberDefinition c0:init0",
            "MemberDefinition c0:assign0",
        ]
    );
}

#[test]
fn diagnoses_malformed_and_duplicate_copy_lifecycle_slots_deterministically() {
    let output = resolve_text(concat!(
        "class Other { init() {} }\n",
        "class Duplicate {\n",
        "    init() {}\n",
        "    init(value: i64) {}\n",
        "    init(ref first: Duplicate) {}\n",
        "    init(ref second: Duplicate) {}\n",
        "    assign(ref first: Duplicate) {}\n",
        "    assign(ref second: Duplicate) {}\n",
        "}\n",
        "class MissingSource { init() {} assign() {} }\n",
        "class MutableSource { init() {} assign(mut ref other: MutableSource) {} }\n",
        "class WrongSource { init() {} assign(ref other: Other) {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let diagnostics: Vec<_> = output.diagnostics.iter().collect();
    assert_eq!(diagnostics.len(), 6);
    assert_eq!(diagnostics[0].code, DUPLICATE_MEMBER);
    assert_eq!(
        diagnostics[0].message,
        "duplicate ordinary initializer in class `Duplicate`"
    );
    assert_eq!(diagnostics[1].code, DUPLICATE_MEMBER);
    assert_eq!(
        diagnostics[1].message,
        "duplicate copy constructor in class `Duplicate`"
    );
    assert_eq!(diagnostics[2].code, DUPLICATE_MEMBER);
    assert_eq!(
        diagnostics[2].message,
        "duplicate copy assignment in class `Duplicate`"
    );
    assert!(diagnostics[0..3]
        .iter()
        .all(
            |diagnostic| diagnostic.labels[0].message == "redeclared here"
                && diagnostic.labels[1].message == "first declared here"
        ));
    assert!(diagnostics[3..]
        .iter()
        .all(|diagnostic| diagnostic.code == INVALID_LIFECYCLE_SIGNATURE));
    for class_index in 2..=4 {
        assert_eq!(
            class(&output, class_index).copy_assignment,
            ResolvedCopyOperation::Unavailable
        );
    }
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
fn resolves_nested_object_places_from_locals_self_and_alias_roots() {
    let output = resolve_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } fn read() -> i64 { return self.value; } }\n",
        "class Link { next: Leaf; init() {} }\n",
        "class Root { next: Link; init() {} fn read() -> i64 { return ((self.next).next).value; } }\n",
        "fn take(ref leaf: Leaf) -> i64 { return leaf.value; }\n",
        "fn inspect(ref root: Root) -> i64 { return root.next.next.read(); }\n",
        "fn forward(mut ref root: Root) -> i64 { return take(((root.next).next)); }\n",
        "fn main() -> i64 { var root: Root = Root(); return root.next.next.value; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let expected = [
        FieldId::new(ClassId::new(2), 0),
        FieldId::new(ClassId::new(1), 0),
    ];

    let root_definition = &output
        .program
        .class_definitions
        .get(ClassId::new(2))
        .unwrap()
        .methods[0];
    let ResolvedExpression::FieldAccess(self_access) =
        return_value(&root_definition.body.statements[0])
    else {
        panic!("expected nested self field access");
    };
    assert_eq!(
        self_access.receiver.root,
        BindingId::Receiver(root_definition.callable)
    );
    assert_eq!(self_access.receiver.projections, expected);
    assert_eq!(self_access.receiver.class, ClassId::new(0));

    let inspect = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let ResolvedExpression::MethodCall(call) = return_value(&inspect.body.statements[0]) else {
        panic!("expected nested method call");
    };
    assert_eq!(
        call.receiver.root,
        BindingId::Parameter(
            output
                .program
                .declarations
                .get(FunctionId::new(1))
                .unwrap()
                .parameters[0]
                .id,
        )
    );
    assert_eq!(call.receiver.projections, expected);

    let forward = output.program.definitions.get(FunctionId::new(2)).unwrap();
    let ResolvedExpression::DirectCall(call) = return_value(&forward.body.statements[0]) else {
        panic!("expected forwarding call");
    };
    let ResolvedExpression::Grouped(grouped) = &call.arguments[0] else {
        panic!("expected grouped alias argument");
    };
    let ResolvedExpression::FieldAccess(argument) = &*grouped.expression else {
        panic!("expected class-field endpoint");
    };
    assert_eq!(argument.receiver.projections, expected[..1]);
    assert_eq!(grouped.span, call.arguments[0].span());

    let main = output
        .program
        .definitions
        .get(output.program.entry_function.unwrap())
        .unwrap();
    let ResolvedExpression::FieldAccess(local_access) = return_value(&main.body.statements[1])
    else {
        panic!("expected nested local field access");
    };
    assert_eq!(
        local_access.receiver.root,
        BindingId::Local(main.locals[0].id)
    );
    assert_eq!(local_access.receiver.projections, expected);

    let dump = dump_resolved(&output.program);
    let nested_receivers: Vec<_> = dump
        .lines()
        .filter(|line| line.contains("-> c1:field0 class c0"))
        .map(str::trim)
        .collect();
    assert_eq!(
        nested_receivers,
        [
            "Receiver f1:p0 -> c2:field0 -> c1:field0 class c0 @333..347",
            "Receiver f3:l0 -> c2:field0 -> c1:field0 class c0 @484..498",
            "Receiver c2:method0:self -> c2:field0 -> c1:field0 class c0 @206..224",
        ]
    );
}

#[test]
fn diagnoses_invalid_intermediate_and_terminal_nested_members() {
    let output = resolve_text(concat!(
        "class Leaf { value: i64; init(value: i64) { self.value = value; } fn read() -> i64 { return self.value; } }\n",
        "class Root { child: Leaf; init() {} }\n",
        "fn first(ref root: Root) -> i64 { return root.child.value.missing; }\n",
        "fn second(ref root: Root) -> i64 { return root.child.missing; }\n",
        "fn third(ref root: Root) -> i64 { return root.child.read.value; }\n",
        "fn fourth(ref root: Root) -> i64 { return root.child.value(); }\n",
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
            INVALID_MEMBER_SELECTION,
            UNKNOWN_MEMBER,
            INVALID_MEMBER_SELECTION,
            INVALID_CALL_TARGET,
        ]
    );
    assert!(output.diagnostics.iter().any(|diagnostic| diagnostic
        .message
        .contains("cannot be used as an object place")));
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
fn resolves_destructor_identity_receiver_body_and_forward_references() {
    let output = resolve_text(concat!(
        "class Resource {\n",
        "    value: i64;\n",
        "    init() { self.value = 0; }\n",
        "    destroy {\n",
        "        var saved: i64 = self.value;\n",
        "        self.reset();\n",
        "        observe(self);\n",
        "    }\n",
        "    mut fn reset() -> unit { self.value = 0; }\n",
        "    fn destroy() -> unit {}\n",
        "}\n",
        "fn observe(ref resource: Resource) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let resource = class(&output, 0);
    let destructor = resource.destructor.as_ref().unwrap();
    assert_eq!(destructor.id, DestructorId::new(resource.id, 0));
    let definition = output
        .program
        .member_definition(destructor.id.into())
        .unwrap();
    assert_eq!(definition.callable, CallableId::Destructor(destructor.id));
    assert_eq!(definition.locals[0].id, LocalId::new(destructor.id, 0));

    let ResolvedStatement::Local(saved) = &definition.body.statements[0] else {
        panic!("expected destructor-local declaration");
    };
    let ResolvedExpression::FieldAccess(value) = &saved.initializer else {
        panic!("expected a field read through destructor `self`");
    };
    assert_eq!(
        value.receiver.root,
        BindingId::Receiver(CallableId::Destructor(destructor.id))
    );

    let ResolvedStatement::Expression(reset) = &definition.body.statements[1] else {
        panic!("expected destructor method call");
    };
    let ResolvedExpression::MethodCall(reset) = &reset.expression else {
        panic!("expected resolved method call");
    };
    assert_eq!(reset.method, resource.methods[0].id);
    assert_eq!(
        reset.receiver.root,
        BindingId::Receiver(CallableId::Destructor(destructor.id))
    );
    assert_eq!(resource.methods[1].name, "destroy");
}

#[test]
fn special_destructor_is_not_an_explicit_method_call_target() {
    let output = resolve_text(concat!(
        "class Resource { init() {} destroy { self.destroy(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, UNKNOWN_MEMBER);
    assert!(diagnostic.message.contains("has no member `destroy`"));
}

#[test]
fn copy_lifecycle_slots_are_not_explicit_method_call_targets() {
    let output = resolve_text(concat!(
        "class Value {\n",
        "  init() {}\n",
        "  init(ref other: Value) {}\n",
        "  assign(ref other: Value) { self.assign(other); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, UNKNOWN_MEMBER);
    assert!(diagnostic.message.contains("has no member `assign`"));
}

#[test]
fn duplicate_destructors_are_diagnosed_in_source_order() {
    let output = resolve_text(concat!(
        "class Duplicate {\n",
        "    init() {}\n",
        "    destroy {}\n",
        "    destroy { return; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    let diagnostic = output.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, DUPLICATE_MEMBER);
    assert_eq!(
        diagnostic.message,
        "duplicate destructor in class `Duplicate`"
    );
    assert_eq!(diagnostic.labels[0].message, "redeclared here");
    assert_eq!(diagnostic.labels[1].message, "first declared here");

    let duplicate = class(&output, 0);
    assert_eq!(duplicate.destructor.as_ref().unwrap().id.index(), 0);
    assert_eq!(
        output
            .program
            .class_definitions
            .get(duplicate.id)
            .unwrap()
            .destructor
            .as_ref()
            .unwrap()
            .body
            .statements
            .len(),
        0
    );
}

#[test]
fn resolved_destructor_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "class Empty { init() {} destroy { return; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..77\n",
            "  Entry f0\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Empty\" @0..45\n",
            "      Fields\n",
            "      OrdinaryInitializer\n",
            "        Initializer c0:init0 @14..23\n",
            "          Parameters\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Destructor\n",
            "        Destructor c0:destroy0 @24..43\n",
            "      Methods\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @46..76\n",
            "      Parameters\n",
            "      ReturnType\n",
            "        Type I64 @59..62\n",
            "  Definitions\n",
            "    Definition f0 @46..76\n",
            "      Locals\n",
            "      Block @63..76\n",
            "        Return @65..74\n",
            "          Integer \"0\" @72..73\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..45\n",
            "      MemberDefinition c0:init0 @14..23\n",
            "        Locals\n",
            "        Block @21..23\n",
            "      MemberDefinition c0:destroy0 @24..43\n",
            "        Locals\n",
            "        Block @32..43\n",
            "          Return @34..41\n",
        )
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
            "      OrdinaryInitializer\n",
            "        Initializer c0:init0 @24..64\n",
            "          Parameters\n",
            "            Parameter c0:init0:p0 \"value\" @29..39\n",
            "              Binding Value\n",
            "              Type I64 @36..39\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "      Destructor\n",
            "        <none>\n",
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

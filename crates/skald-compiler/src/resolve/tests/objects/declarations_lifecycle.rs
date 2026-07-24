use super::*;

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
        initial_assignment.receiver.root().unwrap(),
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
        forwarded_call.receiver.root().unwrap(),
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
    assert_eq!(copy_constructor.id, CopyConstructorId::new(value.id, 0));
    assert_eq!(ordinary.id, InitializerId::new(value.id, 0));
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
        assignment.receiver.root().unwrap(),
        BindingId::Receiver(copy_assignment.id.into())
    );
    let ResolvedExpression::FieldAccess(source) = &assignment.value else {
        panic!("expected source field access");
    };
    assert_eq!(
        source.receiver.root().unwrap(),
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
            "User c0:copy0",
            "User c0:assign0",
            "MemberDefinition c0:init0",
            "MemberDefinition c0:copy0",
            "MemberDefinition c0:assign0",
        ]
    );
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
        value.receiver.root().unwrap(),
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
        reset.receiver.root().unwrap(),
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

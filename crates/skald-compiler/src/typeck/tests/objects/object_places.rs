use super::*;

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
        ObjectProjection::Field(FieldId::new(ClassId::new(2), 0)),
        ObjectProjection::Field(FieldId::new(ClassId::new(1), 0)),
    ];
    let copy_capabilities = CopyCapabilities::compute(&resolved);

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
    let mut lowering_diagnostics = Diagnostics::new();
    let inspect = CallableChecker::new(
        &resolved,
        &copy_capabilities,
        inspect_declaration,
        inspect_definition,
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::DirectCall { arguments, .. } = &returned_expression(&inspect).kind
    else {
        panic!("expected typed forwarding call");
    };
    let (view, place) = class_alias_view(&arguments[0]);
    assert_eq!(
        place.root(),
        BindingId::Parameter(inspect_declaration.parameters[0].id)
    );
    assert_eq!(place.projections(), expected);
    assert_eq!(place.class(), ClassId::new(0));
    assert_eq!(place.access, HirAccess::ReadOnly);
    assert_eq!(place.span(), grouped_span);
    assert!(matches!(
        view.origin.as_ref(),
        crate::hir::HirObjectOrigin::Exact {
            dynamic_class,
            ..
        } if *dynamic_class == ClassId::new(0)
    ));

    let mutable_declaration = resolved.declarations.get(FunctionId::new(2)).unwrap();
    let mutable_definition = resolved.definitions.get(FunctionId::new(2)).unwrap();
    let mut diagnostics = Diagnostics::new();
    let mut lowering_diagnostics = Diagnostics::new();
    let mutable = CallableChecker::new(
        &resolved,
        &copy_capabilities,
        mutable_declaration,
        mutable_definition,
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::MethodCall { receiver, .. } = &returned_expression(&mutable).kind else {
        panic!("expected nested method receiver");
    };
    let receiver = receiver_place(receiver);
    assert_eq!(receiver.projections(), expected);
    assert_eq!(receiver.access, HirAccess::Mutable);

    let local_declaration = resolved.declarations.get(FunctionId::new(3)).unwrap();
    let local_definition = resolved.definitions.get(FunctionId::new(3)).unwrap();
    let mut diagnostics = Diagnostics::new();
    let mut lowering_diagnostics = Diagnostics::new();
    let local = CallableChecker::new(
        &resolved,
        &copy_capabilities,
        local_declaration,
        local_definition,
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check();
    assert!(diagnostics.is_empty());
    let HirExpressionKind::MethodCall { receiver, .. } = &returned_expression(&local).kind else {
        panic!("expected local nested method receiver");
    };
    let receiver = receiver_place(receiver);
    assert_eq!(receiver.projections(), expected);
    assert_eq!(receiver.root(), BindingId::Local(local.locals[0].id));
    assert_eq!(receiver.access, HirAccess::Mutable);

    let class = resolved.classes.get(ClassId::new(2)).unwrap();
    let method = &class.methods[0];
    let definition = &resolved.class_definitions.get(class.id).unwrap().methods[0];
    let mut diagnostics = Diagnostics::new();
    let mut lowering_diagnostics = Diagnostics::new();
    let member = CallableChecker::new_member(
        &resolved,
        &copy_capabilities,
        MemberCheckContext {
            callable: method.id.into(),
            owner: class.id,
            parameters: &method.parameters,
            definition,
            return_type: Type::I64,
            receiver: Some(ReceiverContext {
                class: class.id,
                access: HirAccess::ReadOnly,
            }),
            body_kind: MemberBodyKind::MethodOrDestructor,
            callable_name: "method `nested`".to_owned(),
        },
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check_member();
    assert!(diagnostics.is_empty());
    let HirStatement::Return(statement) = &member.body.statements[0] else {
        panic!("expected member return");
    };
    let HirReturnValue::Scalar(value) = statement.value.as_ref().unwrap() else {
        panic!("expected scalar return");
    };
    let HirExpressionKind::FieldRead(field) = &value.kind else {
        panic!("expected nested self field read");
    };
    let receiver = receiver_place(&field.receiver);
    assert_eq!(receiver.projections(), expected);
    assert_eq!(receiver.root(), BindingId::Receiver(method.id.into()));
    assert_eq!(receiver.access, HirAccess::ReadOnly);
}

#[test]
fn checks_a_class_owned_body_with_explicitly_absent_receiver_context() {
    let resolved = resolve_text(concat!(
        "class Tools {\n",
        "    init() {}\n",
        "    fn answer() -> i64 { return 42; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let copy_capabilities = CopyCapabilities::compute(&resolved);
    let class = resolved.classes.get(ClassId::new(0)).unwrap();
    let method = &class.methods[0];
    let definition = &resolved.class_definitions.get(class.id).unwrap().methods[0];
    let mut diagnostics = Diagnostics::new();
    let mut lowering_diagnostics = Diagnostics::new();

    let member = CallableChecker::new_member(
        &resolved,
        &copy_capabilities,
        MemberCheckContext {
            callable: method.id.into(),
            owner: class.id,
            parameters: &method.parameters,
            definition,
            return_type: Type::I64,
            receiver: None,
            body_kind: MemberBodyKind::MethodOrDestructor,
            callable_name: "receiverless class body".to_owned(),
        },
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check_member();

    assert!(diagnostics.is_empty());
    assert_eq!(member.class_owner, class.id);
    assert_eq!(member.receiver_class, None);
    assert!(matches!(member.body.statements[0], HirStatement::Return(_)));
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
    let mut lowering_diagnostics = Diagnostics::new();
    let copy_capabilities = CopyCapabilities::compute(&resolved);

    let _ = CallableChecker::new(
        &resolved,
        &copy_capabilities,
        declaration,
        definition,
        &mut diagnostics,
        &mut lowering_diagnostics,
    )
    .check();

    assert!(diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("is not a value")));
}

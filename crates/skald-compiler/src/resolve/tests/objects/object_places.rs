use super::*;

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
        ObjectProjection::Field(FieldId::new(ClassId::new(2), 0)),
        ObjectProjection::Field(FieldId::new(ClassId::new(1), 0)),
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

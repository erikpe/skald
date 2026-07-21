use super::*;
use crate::{identity::FieldId, resolve::ResolvedParameterBindingMode};

#[test]
fn resolves_alias_modes_nominal_types_and_parameter_ids_for_every_internal_owner() {
    let output = resolve_text(concat!(
        "fn inspect(ref dog: Dog, mut ref target: Dog, count: i64) -> i64 {\n",
        "    target.age = dog.age;\n",
        "    return count;\n",
        "}\n",
        "class Dog {\n",
        "    age: i64;\n",
        "    init(ref source: Dog) { self.age = source.age; }\n",
        "    fn compare(ref other: Dog) -> i64 { return other.age; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let inspect = output
        .program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "inspect")
        .unwrap();
    assert!(matches!(
        inspect.parameters[0].binding_mode,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. }
    ));
    assert!(matches!(
        inspect.parameters[1].binding_mode,
        ResolvedParameterBindingMode::MutableAlias { .. }
    ));
    assert_eq!(
        inspect.parameters[2].binding_mode,
        ResolvedParameterBindingMode::Value
    );
    for (index, parameter) in inspect.parameters.iter().enumerate() {
        assert_eq!(parameter.id, ParameterId::new(inspect.id, index));
    }

    let dog = output.program.classes.get(ClassId::new(0)).unwrap();
    assert_eq!(
        inspect.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Class(dog.id)
    );
    let initializer = dog.initializer.as_ref().unwrap();
    assert!(matches!(
        initializer.parameters[0].binding_mode,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. }
    ));
    assert_eq!(
        initializer.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Class(dog.id)
    );
    assert!(matches!(
        dog.methods[0].parameters[0].binding_mode,
        ResolvedParameterBindingMode::ReadOnlyAlias { .. }
    ));
}

#[test]
fn alias_parameters_are_object_place_bases_and_grouping_preserves_argument_shape() {
    let output = resolve_text(concat!(
        "class Dog { age: i64; init() { self.age = 0; } fn look() -> i64 { return self.age; } }\n",
        "fn inspect(ref dog: Dog) -> unit {}\n",
        "fn relay(ref dog: Dog) -> i64 { inspect((dog)); return (dog).look(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let relay = output
        .program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "relay")
        .unwrap();
    let definition = output.program.definitions.get(relay.id).unwrap();
    let ResolvedStatement::Expression(statement) = &definition.body.statements[0] else {
        panic!("expected call statement");
    };
    let ResolvedExpression::DirectCall(call) = &statement.expression else {
        panic!("expected direct call");
    };
    let ResolvedExpression::Grouped(grouped) = &call.arguments[0] else {
        panic!("alias call argument must retain its source grouping");
    };
    let ResolvedExpression::Binding(binding) = grouped.expression.as_ref() else {
        panic!("grouped argument must contain the alias binding");
    };
    assert_eq!(
        binding.binding,
        BindingId::Parameter(relay.parameters[0].id)
    );

    let ResolvedExpression::MethodCall(call) = return_value(&definition.body.statements[1]) else {
        panic!("expected grouped method receiver");
    };
    assert_eq!(
        call.receiver.binding,
        BindingId::Parameter(relay.parameters[0].id)
    );
    assert_eq!(call.receiver.class, ClassId::new(0));
}

#[test]
fn alias_places_keep_lexical_shadowing_and_class_member_namespaces_independent() {
    let output = resolve_text(concat!(
        "class Left { id: i64; init() { self.id = 1; } }\n",
        "class Right { id: i64; init() { self.id = 2; } }\n",
        "fn choose(ref value: Left) -> i64 {\n",
        "    { var value: Right = Right(); value.id; }\n",
        "    return value.id;\n",
        "}\n",
        "fn right(ref value: Right) -> i64 { return value.id; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let choose = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let definition = output.program.definitions.get(choose.id).unwrap();
    let ResolvedStatement::Block(nested) = &definition.body.statements[0] else {
        panic!("expected nested block");
    };
    let ResolvedStatement::Expression(inner_statement) = &nested.statements[1] else {
        panic!("expected inner field access");
    };
    let ResolvedExpression::FieldAccess(inner) = &inner_statement.expression else {
        panic!("expected inner field access");
    };
    assert_eq!(
        inner.receiver.binding,
        BindingId::Local(definition.locals[0].id)
    );
    assert_eq!(inner.field, FieldId::new(ClassId::new(1), 0));

    let ResolvedExpression::FieldAccess(outer) = return_value(&definition.body.statements[1])
    else {
        panic!("expected outer field access");
    };
    assert_eq!(
        outer.receiver.binding,
        BindingId::Parameter(choose.parameters[0].id)
    );
    assert_eq!(outer.field, FieldId::new(ClassId::new(0), 0));

    let right = output.program.declarations.get(FunctionId::new(1)).unwrap();
    let right_definition = output.program.definitions.get(right.id).unwrap();
    let ResolvedExpression::FieldAccess(right_access) =
        return_value(&right_definition.body.statements[0])
    else {
        panic!("expected right field access");
    };
    assert_eq!(right_access.field, FieldId::new(ClassId::new(1), 0));
}

#[test]
fn diagnoses_unknown_members_selected_through_alias_places() {
    let output = resolve_text(concat!(
        "class Dog { init() {} }\n",
        "fn inspect(ref dog: Dog) -> i64 { return dog.missing; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(output.diagnostics.len(), 1);
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        UNKNOWN_MEMBER
    );
}

#[test]
fn resolved_alias_dump_is_exact_and_identity_based() {
    let output = resolve_text(concat!(
        "fn inspect(ref dog: Dog, mut ref target: Dog) -> unit { target.age = dog.age; }\n",
        "class Dog { age: i64; init() { self.age = 0; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    assert_eq!(
        dump_resolved(&output.program),
        concat!(
            "ResolvedProgram @0..160\n",
            "  Entry f1\n",
            "  ClassDeclarations\n",
            "    Class c0 \"Dog\" @80..128\n",
            "      Fields\n",
            "        Field c0:field0 \"age\" @92..101\n",
            "          Type I64 @97..100\n",
            "      Initializer\n",
            "        Initializer c0:init0 @102..126\n",
            "          Parameters\n",
            "      Methods\n",
            "  Declarations\n",
            "    Declaration f0 \"inspect\" internal @0..79\n",
            "      Parameters\n",
            "        Parameter f0:p0 \"dog\" @11..23\n",
            "          Binding ReadOnlyAlias\n",
            "            Ref @11..14\n",
            "          Type Class c0 @20..23\n",
            "        Parameter f0:p1 \"target\" @25..44\n",
            "          Binding MutableAlias\n",
            "            Mut @25..28\n",
            "            Ref @29..32\n",
            "          Type Class c0 @41..44\n",
            "      ReturnType\n",
            "        Type Unit @49..53\n",
            "    Declaration f1 \"main\" internal @129..159\n",
            "      Parameters\n",
            "      ReturnType\n",
            "        Type I64 @142..145\n",
            "  Definitions\n",
            "    Definition f0 @0..79\n",
            "      Locals\n",
            "      Block @54..79\n",
            "        FieldAssignment c0:field0 @56..77\n",
            "          Receiver f0:p1 class c0 @56..62\n",
            "          Equal @67..68\n",
            "          Value\n",
            "            FieldAccess c0:field0 @69..76\n",
            "              Receiver f0:p0 class c0 @69..72\n",
            "    Definition f1 @129..159\n",
            "      Locals\n",
            "      Block @146..159\n",
            "        Return @148..157\n",
            "          Integer \"0\" @155..156\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @80..128\n",
            "      MemberDefinition c0:init0 @102..126\n",
            "        Locals\n",
            "        Block @109..126\n",
            "          FieldAssignment c0:field0 @111..124\n",
            "            Receiver c0:init0:self class c0 @111..115\n",
            "            Equal @120..121\n",
            "            Value\n",
            "              Integer \"0\" @122..123\n",
        )
    );
}

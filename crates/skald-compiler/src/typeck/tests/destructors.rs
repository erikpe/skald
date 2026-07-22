use super::*;
use crate::{
    hir::{HirAccess, HirCallArgument, HirExpressionKind, HirParameterMode, HirStatement},
    identity::{CallableId, ClassId, DestructorId, FieldId, FunctionId},
};

#[test]
fn checks_destructor_bodies_as_mutable_complete_object_members() {
    let output = check_text(concat!(
        "class Leaf {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  mut fn increment() -> unit { self.value = self.value + 1; }\n",
        "}\n",
        "class Branch { leaf: Leaf; init(value: i64) { self.leaf = Leaf(value); } }\n",
        "class Owner {\n",
        "  branch: Branch; observed: i64;\n",
        "  init(value: i64) { self.branch = Branch(value); self.observed = 0; }\n",
        "  destroy {\n",
        "    var value: i64 = self.branch.leaf.read();\n",
        "    self.branch.leaf.increment();\n",
        "    inspect(self.branch.leaf);\n",
        "    mutate(self.branch.leaf);\n",
        "    if (true) { { self.observed = value; } } else { return; }\n",
        "  }\n",
        "}\n",
        "fn inspect(ref leaf: Leaf) -> unit {}\n",
        "fn mutate(mut ref leaf: Leaf) -> unit { leaf.increment(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let owner = hir.class(ClassId::new(2)).unwrap();
    let destructor = owner.destructor.as_ref().unwrap();
    assert_eq!(destructor.id, DestructorId::new(owner.id, 0));
    assert_eq!(destructor.receiver_access, HirAccess::Mutable);
    let signature = hir
        .callable_signature(CallableId::Destructor(destructor.id))
        .unwrap();
    assert!(signature.parameters.is_empty());
    assert_eq!(signature.return_type, Type::Unit);
    assert_eq!(
        hir.declarations.get(FunctionId::new(0)).unwrap().parameters[0].mode,
        HirParameterMode::ReadOnlyAlias
    );
    assert_eq!(
        hir.declarations.get(FunctionId::new(1)).unwrap().parameters[0].mode,
        HirParameterMode::MutableAlias
    );

    let definition = hir
        .member_definition(CallableId::Destructor(destructor.id))
        .unwrap();
    assert_eq!(definition.locals.len(), 1);
    assert_eq!(definition.locals[0].ty, Type::I64);
    assert_eq!(definition.body.statements.len(), 5);

    let HirStatement::Call(readonly_call) = &definition.body.statements[2] else {
        panic!("expected read-only alias call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &readonly_call.call.kind else {
        panic!("expected direct call");
    };
    let HirCallArgument::Place(place) = &arguments[0] else {
        panic!("expected object place argument");
    };
    assert_eq!(place.access, HirAccess::Mutable);
    assert_eq!(
        place.path.projections.as_slice(),
        &[FieldId::new(owner.id, 0), FieldId::new(ClassId::new(1), 0)]
    );

    let HirStatement::Conditional(conditional) = &definition.body.statements[4] else {
        panic!("expected conditional");
    };
    assert_eq!(conditional.flow, BlockFlow::FallsThrough);
    assert_eq!(
        conditional.else_block.as_ref().unwrap().flow,
        BlockFlow::Terminates
    );
}

#[test]
fn rejects_value_returns_from_implicit_unit_destructors() {
    let output = check_text(concat!(
        "class Resource { init() {} destroy { return 1; } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_RETURN
            && diagnostic.message == "destructor for class `Resource` cannot return a value"
    }));
}

#[test]
fn permits_copying_and_bounded_constructor_temporaries_in_destructor_bodies() {
    let output = check_text(concat!(
        "class Leaf { init() {} }\n",
        "class Owner {\n",
        "  leaf: Leaf;\n",
        "  init() { self.leaf = Leaf(); }\n",
        "  destroy { self.leaf = Leaf(); var copy: Leaf = self.leaf; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
}

#[test]
fn destructor_hir_dump_is_exact_and_identity_based() {
    let output = check_text(concat!(
        "class Resource {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  destroy { self.value = self.value + 1; return; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let dump = dump_hir(&output.hir.unwrap());
    assert_eq!(
        dump,
        concat!(
            "HirProgram @0..158\n",
            "  Entry f0\n",
            "  Classes\n",
            "    Class c0 \"Resource\" @0..126\n",
            "      Fields\n",
            "        Field c0:field0 \"value\" : i64 @19..30\n",
            "      Initializer c0:init0 @33..73\n",
            "        Parameter c0:init0:p0 \"value\" value : i64 @38..48\n",
            "      CopyConstructor\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      CopyAssignment\n",
            "        Synthesized c0\n",
            "          Primitive c0:field0\n",
            "      Destructor c0:destroy0 mutable -> unit @76..124\n",
            "      Methods\n",
            "  ClassDefinitions\n",
            "    ClassDefinition c0 @0..126\n",
            "      MemberDefinition c0:init0 @33..73\n",
            "        Locals\n",
            "        Block @50..73\n",
            "          FieldAssignment @52..71\n",
            "            FieldPlace c0:field0 @52..71\n",
            "              ObjectPlace c0:init0:self : class c0 mutable @52..56\n",
            "            Binding c0:init0:p0 : i64 @65..70\n",
            "      MemberDefinition c0:destroy0 @76..124\n",
            "        Locals\n",
            "        Block @84..124\n",
            "          FieldAssignment @86..114\n",
            "            FieldPlace c0:field0 @86..114\n",
            "              ObjectPlace c0:destroy0:self : class c0 mutable @86..90\n",
            "            Binary AddI64 : i64 @99..113\n",
            "              FieldRead c0:field0 : i64 @99..109\n",
            "                ObjectPlace c0:destroy0:self : class c0 mutable @99..103\n",
            "              Integer 1 : i64 @112..113\n",
            "          Return @115..122\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @127..157\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @127..157\n",
            "      Locals\n",
            "      Block @144..157\n",
            "        Return @146..155\n",
            "          Integer 0 : i64 @153..154\n",
        )
    );
}

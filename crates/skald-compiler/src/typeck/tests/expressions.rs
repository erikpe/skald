use super::*;

#[test]
fn checks_the_demonstration_program_into_fully_typed_hir() {
    let output = check_text(concat!(
        "fn twice(value: i64) -> i64 { return value * 2; }\n",
        "fn main() -> i64 {\n",
        "  var result: i64 = twice(20);\n",
        "  return result + 2;\n",
        "}\n",
    ));
    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    assert_eq!(hir.entry_function.index(), 1);
    assert_eq!(hir.declarations.len(), 2);
    assert_eq!(hir.definitions.len(), 2);

    for declaration in hir.declarations.iter() {
        assert_eq!(declaration.return_type, Type::I64);
        for parameter in &declaration.parameters {
            assert_eq!(parameter.ty, Type::I64);
        }
        let definition = hir.definitions.get(declaration.id).unwrap();
        for local in &definition.locals {
            assert_eq!(local.ty, Type::I64);
        }
        for statement in &definition.body.statements {
            match statement {
                HirStatement::Local(local) => {
                    let HirLocalInitializer::Value(initializer) = &local.initializer else {
                        panic!("expected scalar local initializer");
                    };
                    assert_expression_is_fully_typed(initializer);
                }
                HirStatement::Return(statement) => {
                    if let Some(value) = &statement.value {
                        let HirReturnValue::Scalar(value) = value else {
                            panic!("expected scalar return");
                        };
                        assert_expression_is_fully_typed(value);
                    }
                }
                HirStatement::Call(statement) => assert_expression_is_fully_typed(&statement.call),
                HirStatement::Conditional(_) => {}
                HirStatement::Block(_) => {}
                HirStatement::BaseInitialization(_)
                | HirStatement::FieldAssignment(_)
                | HirStatement::FieldConstruction(_)
                | HirStatement::FieldCopyConstruction(_)
                | HirStatement::FieldCopyAssignment(_)
                | HirStatement::CopyAssignment(_)
                | HirStatement::SharedFieldWrite(_)
                | HirStatement::SharedAssignment(_)
                | HirStatement::OptionalAssignment(_) => {}
                HirStatement::ClassOptionalAssignment(_) => {}
            }
        }
    }

    let main = hir.definitions.get(hir.entry_function).unwrap();
    let HirStatement::Local(local) = &main.body.statements[0] else {
        panic!("expected local declaration");
    };
    let HirLocalInitializer::Value(initializer) = &local.initializer else {
        panic!("expected scalar local initializer");
    };
    let HirExpressionKind::DirectCall {
        function,
        arguments,
    } = &initializer.kind
    else {
        panic!("expected typed direct call");
    };
    assert_eq!(function.index(), 0);
    assert_eq!(arguments.len(), 1);

    let HirExpressionKind::Binary { operation, .. } = &returned_expression(main).kind else {
        panic!("expected typed addition");
    };
    assert_eq!(*operation, HirBinaryOperation::AddI64);
}

#[test]
fn checks_unit_functions_returns_and_call_statements() {
    let output = check_text(concat!(
        "fn explicit() -> unit { return; }\n",
        "fn implicit() -> unit {}\n",
        "fn main() -> i64 { (explicit()); implicit(); return 0; }\n",
    ));

    assert!(!output.has_errors());
    let hir = output.hir.unwrap();
    assert_eq!(
        hir.declarations
            .get(FunctionId::new(0))
            .unwrap()
            .return_type,
        Type::Unit
    );
    assert_eq!(
        hir.declarations
            .get(FunctionId::new(1))
            .unwrap()
            .return_type,
        Type::Unit
    );
    let explicit = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Return(statement) = &explicit.body.statements[0] else {
        panic!("expected explicit unit return");
    };
    assert!(statement.value.is_none());
    let main = hir.definitions.get(hir.entry_function).unwrap();
    assert!(main.body.statements[..2].iter().all(|statement| {
        matches!(
            statement,
            HirStatement::Call(call) if call.call.ty == Type::Unit
        )
    }));
    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("ReturnType unit"));
    assert!(dump.contains("CallStatement"));
    assert!(dump.contains("DirectCall f0 : unit"));
}

#[test]
fn checks_invalid_binary_operands_in_source_order() {
    let source = concat!(
        "class Left { init() {} }\n",
        "class Right { init() {} }\n",
        "fn main() -> i64 { return Left() + Right(); }\n",
    );
    let output = check_text(source);
    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION)
        .collect();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].labels[0].span.range().start(),
        source.find("Left() +").unwrap()
    );
    assert_eq!(
        diagnostics[1].labels[0].span.range().start(),
        source.find("Right();").unwrap()
    );
}

#[test]
fn checks_excluded_construction_arguments_before_their_owner() {
    let source = concat!(
        "class Inner { init() {} }\n",
        "class Outer { init(value: i64) {} }\n",
        "fn main() -> i64 { return Outer(Inner()); }\n",
    );
    let output = check_text(source);
    let diagnostics: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_CONSTRUCTION)
        .collect();

    assert_eq!(diagnostics.len(), 2);
    assert_eq!(
        diagnostics[0].labels[0].span.range().start(),
        source.find("Inner());").unwrap()
    );
    assert_eq!(
        diagnostics[1].labels[0].span.range().start(),
        source.find("Outer(Inner())").unwrap()
    );
}

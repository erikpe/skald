use crate::{identity::LoopId, test_support::type_check_source};

use super::{
    dump_hir, HirBlock, HirControlEffects, HirExpression, HirExpressionKind, HirStatement,
    HirWhile, Type,
};

#[test]
fn dumps_manually_constructed_structured_while_deterministically() {
    let mut hir = type_check_source("fn main() -> i64 { return 0; }\n")
        .hir
        .unwrap();
    let entry = hir.entry_function;
    let definition = hir.definitions.get_mut_for_test(entry).unwrap();
    let span = definition.body.span;
    let loop_id = LoopId::new(entry, 0);
    let body = HirBlock {
        statements: vec![],
        effects: HirControlEffects::fallthrough(),
        span,
    };
    definition.body.statements.insert(
        0,
        HirStatement::While(HirWhile::new(
            loop_id,
            HirExpression {
                kind: HirExpressionKind::Boolean(true),
                ty: Type::Bool,
                span,
            },
            body,
            span,
        )),
    );

    let dump = dump_hir(&hir);
    let lines: Vec<_> = dump
        .lines()
        .filter(|line| {
            line.contains("While ")
                || line.trim_start().starts_with("Condition ")
                || line.trim_start().starts_with("Boolean ")
        })
        .map(|line| line.split(" @").next().unwrap().trim())
        .collect();
    assert_eq!(
        lines,
        ["While f0:loop0", "Condition", "Boolean true : bool",]
    );
}

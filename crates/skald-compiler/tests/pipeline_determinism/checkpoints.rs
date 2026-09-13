use skald_compiler::{
    lexer::lex,
    mir::{dump_mir, lower_hir},
    passes::{run_mir_pipeline_inspected, MirPipelineCheckpoint},
    resolve::resolve,
    source::SourceDatabase,
    syntax::parse,
    typeck::type_check,
};

pub(crate) fn mir_pipeline_checkpoint_dump() -> String {
    let text = "fn removed_target() -> i64 { return 99; }\n\
                fn identity(value: i64) -> i64 { return value + 0; }\n\
                fn main() -> i64 {\n\
                    if (1 + 1 == 2) { return identity(6 * 7); }\n\
                    return removed_target();\n\
                }\n";
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("checkpoint-determinism.ska", text);
    let source = sources.get(source_id).unwrap();
    let lexed = lex(source);
    assert!(lexed.diagnostics.is_empty());
    let parsed = parse(source, &lexed.tokens);
    assert!(parsed.diagnostics.is_empty());
    let resolved = resolve(&parsed.ast);
    assert!(resolved.diagnostics.is_empty());
    let checked = type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty());

    let mut checkpoints = Vec::new();
    let mut inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
        let label = checkpoint.label().to_string();
        match checkpoint {
            MirPipelineCheckpoint::ProofRich(checkpoint) => {
                checkpoints.push((label, dump_mir(checkpoint.verified()), None));
            }
            MirPipelineCheckpoint::Final(checkpoint) => checkpoints.push((
                label,
                dump_mir(checkpoint.verified()),
                Some(checkpoint.reachability_dump()),
            )),
        }
    };
    run_mir_pipeline_inspected(lower_hir(&checked.hir.unwrap()), &mut inspector).unwrap();

    checkpoints
        .into_iter()
        .map(|(label, mir, reachability)| {
            format!(
                "CHECKPOINT {label}\n{mir}{}",
                reachability.unwrap_or_default()
            )
        })
        .collect()
}

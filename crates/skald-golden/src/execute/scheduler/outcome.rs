use super::{
    coordinator::{CoordinatorOutput, TaskResult},
    graph::ExecutionGraph,
};
use crate::{
    execute::sequential::compilation_status, BuildExecution, CompilationExecution, CompilationKind,
    LeafExecution, LinkExecution, PlannedLeaf, PlannedLeafKind, RuntimeExecution, SelectedPlan,
    SequentialExecution, SequentialOptions, StageStatus,
};
use std::collections::BTreeMap;

pub(super) fn assemble(
    selected: &SelectedPlan<'_>,
    options: &SequentialOptions,
    graph: &ExecutionGraph,
    mut output: CoordinatorOutput,
) -> SequentialExecution {
    let runtime = graph.runtime().map(|node| {
        take_runtime(&mut output.results[node]).unwrap_or_else(|| {
            RuntimeExecution::new(
                options.runtime().command().clone(),
                options.runtime().archive().to_path_buf(),
                None,
                cancelled_status(&output, node),
            )
        })
    });
    let grouped = group_leaves(selected.leaves());
    let mut builds = Vec::new();
    let mut leaves = Vec::new();

    for (build_id, selected_leaves) in grouped {
        let compile_node = graph.compile(build_id);
        let compilation_cancelled = output.results[compile_node].is_none();
        let compilation =
            take_compilation(&mut output.results[compile_node]).unwrap_or_else(|| {
                let kind = match selected_leaves[0].kind() {
                    PlannedLeafKind::Run(_) => CompilationKind::Success,
                    PlannedLeafKind::Compile(_) => CompilationKind::CompileFail,
                };
                CompilationExecution::cancelled(
                    build_id.to_owned(),
                    kind,
                    cancellation_message(&output, compile_node),
                )
            });

        match selected_leaves[0].kind() {
            PlannedLeafKind::Compile(_) => {
                let status = if compilation_cancelled {
                    cancelled_status(&output, compile_node)
                } else {
                    compilation_status(&compilation)
                };
                for leaf in selected_leaves {
                    leaves.push(LeafExecution::new(
                        leaf.id().to_owned(),
                        Vec::new(),
                        status.clone(),
                    ));
                }
                builds.push(BuildExecution::new(
                    build_id.to_owned(),
                    compilation,
                    None,
                    status,
                ));
            }
            PlannedLeafKind::Run(_) => {
                let link_node = graph.link(build_id).expect("native build must have link");
                let link = take_link(&mut output.results[link_node]).unwrap_or_else(|| {
                    LinkExecution::new(
                        selected
                            .plan()
                            .build(build_id)
                            .expect("selected build must exist")
                            .artifact_directory()
                            .join("program"),
                        None,
                        None,
                        cancelled_status(&output, link_node),
                    )
                });
                let build_status = if compilation_cancelled {
                    cancelled_status(&output, compile_node)
                } else if compilation.passed() {
                    link.status().clone()
                } else {
                    compilation_status(&compilation)
                };
                for leaf in selected_leaves {
                    let run_node = graph
                        .run(leaf.id())
                        .expect("native leaf must have run node");
                    let execution = take_run(&mut output.results[run_node]).unwrap_or_else(|| {
                        LeafExecution::new(
                            leaf.id().to_owned(),
                            Vec::new(),
                            cancelled_status(&output, run_node),
                        )
                    });
                    leaves.push(execution);
                }
                builds.push(BuildExecution::new(
                    build_id.to_owned(),
                    compilation,
                    Some(link),
                    build_status,
                ));
            }
        }
    }

    builds.sort_by(|left, right| left.build_id().cmp(right.build_id()));
    leaves.sort_by(|left, right| left.leaf_id().cmp(right.leaf_id()));
    SequentialExecution::new(
        runtime,
        builds,
        leaves,
        output.scheduler_failure,
        output.elapsed,
    )
}

fn cancellation_message(output: &CoordinatorOutput, node: usize) -> String {
    format!(
        "scheduler cancelled this stage because of {}",
        output
            .cancellations
            .get(&node)
            .map(String::as_str)
            .unwrap_or("an unavailable prerequisite")
    )
}

fn cancelled_status(output: &CoordinatorOutput, node: usize) -> StageStatus {
    StageStatus::Cancelled {
        dependency: output
            .cancellations
            .get(&node)
            .cloned()
            .unwrap_or_else(|| "unavailable prerequisite".to_owned()),
    }
}

fn group_leaves<'a>(leaves: &[&'a PlannedLeaf]) -> BTreeMap<&'a str, Vec<&'a PlannedLeaf>> {
    let mut grouped = BTreeMap::new();
    for leaf in leaves {
        grouped
            .entry(leaf.build_id())
            .or_insert_with(Vec::new)
            .push(*leaf);
    }
    grouped
}

fn take_runtime(result: &mut Option<TaskResult>) -> Option<RuntimeExecution> {
    match result.take() {
        Some(TaskResult::Runtime(value)) => Some(value),
        other => {
            *result = other;
            None
        }
    }
}

fn take_compilation(result: &mut Option<TaskResult>) -> Option<CompilationExecution> {
    match result.take() {
        Some(TaskResult::Compilation(value)) => Some(value),
        other => {
            *result = other;
            None
        }
    }
}

fn take_link(result: &mut Option<TaskResult>) -> Option<LinkExecution> {
    match result.take() {
        Some(TaskResult::Link(value)) => Some(value),
        other => {
            *result = other;
            None
        }
    }
}

fn take_run(result: &mut Option<TaskResult>) -> Option<LeafExecution> {
    match result.take() {
        Some(TaskResult::Run(value)) => Some(value),
        other => {
            *result = other;
            None
        }
    }
}

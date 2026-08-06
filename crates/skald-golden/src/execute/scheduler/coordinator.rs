use super::{
    graph::{CompilationPurpose, ExecutionGraph, NodeKind},
    outcome::assemble,
};
use crate::{
    compile::{compile_build, CompilationPurpose as BorrowedCompilationPurpose},
    execute::sequential::{execute_native_leaf, link_build, prepare_runtime},
    CompilationExecution, LeafExecution, LinkExecution, PlannedLeaf, RuntimeExecution,
    SchedulerFailure, SchedulerOptions, SelectedPlan, SequentialExecution, SequentialOptions,
};
use std::{
    any::Any,
    collections::{BTreeMap, BTreeSet},
    panic::{catch_unwind, AssertUnwindSafe},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Instant,
};

/// Executes an immutable selected plan through a bounded fixed worker pool.
pub fn execute_parallel(
    selected: &SelectedPlan<'_>,
    stage_options: &SequentialOptions,
    scheduler_options: SchedulerOptions,
) -> SequentialExecution {
    execute_with(selected, stage_options, scheduler_options, &execute_task)
}

pub(super) fn execute_with<F>(
    selected: &SelectedPlan<'_>,
    stage_options: &SequentialOptions,
    scheduler_options: SchedulerOptions,
    executor: &F,
) -> SequentialExecution
where
    F: Fn(ScheduledTask, &SequentialOptions) -> TaskResult + Sync,
{
    let started = Instant::now();
    let graph = ExecutionGraph::new(selected);
    let mut output = coordinate(&graph, stage_options, scheduler_options, executor);
    output.elapsed = started.elapsed();
    assemble(selected, stage_options, &graph, output)
}

fn coordinate<F>(
    graph: &ExecutionGraph,
    stage_options: &SequentialOptions,
    scheduler_options: SchedulerOptions,
    executor: &F,
) -> CoordinatorOutput
where
    F: Fn(ScheduledTask, &SequentialOptions) -> TaskResult + Sync,
{
    let mut states = vec![NodeState::Pending; graph.nodes().len()];
    let mut remaining_dependencies = graph
        .nodes()
        .iter()
        .map(|node| node.dependencies().len())
        .collect::<Vec<_>>();
    let mut ready = graph
        .nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| node.dependencies().is_empty())
        .map(|(index, node)| (node.sort_key().to_owned(), index))
        .collect::<BTreeSet<_>>();
    let mut results = std::iter::repeat_with(|| None)
        .take(graph.nodes().len())
        .collect::<Vec<_>>();
    let mut cancellations = BTreeMap::new();
    let mut active = BTreeMap::<usize, ActiveLock>::new();
    let mut held_resources = BTreeSet::new();
    let mut scheduler_failure = None;
    let mut failure_observed = false;

    thread::scope(|scope| {
        let (work_sender, work_receiver) = mpsc::channel::<WorkItem>();
        let work_receiver = Arc::new(Mutex::new(work_receiver));
        let (result_sender, result_receiver) = mpsc::channel::<WorkerMessage>();

        for _ in 0..scheduler_options.jobs().get() {
            let work_receiver = Arc::clone(&work_receiver);
            let result_sender = result_sender.clone();
            scope.spawn(move || loop {
                let work = match work_receiver.lock() {
                    Ok(receiver) => receiver.recv(),
                    Err(_) => return,
                };
                let Ok(work) = work else {
                    return;
                };
                let outcome = catch_unwind(AssertUnwindSafe(|| executor(work.task, stage_options)));
                let message = match outcome {
                    Ok(result) => WorkerMessage::Completed {
                        node: work.node,
                        result: Box::new(result),
                    },
                    Err(payload) => WorkerMessage::Panicked {
                        node: work.node,
                        message: panic_message(payload),
                    },
                };
                if result_sender.send(message).is_err() {
                    return;
                }
            });
        }
        drop(result_sender);

        loop {
            let may_schedule =
                scheduler_failure.is_none() && !(scheduler_options.fail_fast() && failure_observed);
            while may_schedule && active.len() < scheduler_options.jobs().get() {
                let candidate = ready
                    .iter()
                    .find(|(_, index)| can_start(graph.node(*index), &active, &held_resources))
                    .cloned();
                let Some((sort_key, node)) = candidate else {
                    break;
                };
                ready.remove(&(sort_key, node));
                let task = materialize_task(graph, node, &results);
                if work_sender.send(WorkItem { node, task }).is_err() {
                    scheduler_failure = Some(failure_snapshot(
                        graph,
                        &states,
                        &active,
                        "worker command channel closed unexpectedly",
                    ));
                    cancel_all_pending(
                        graph,
                        &mut states,
                        &mut ready,
                        &mut cancellations,
                        "scheduler-internal",
                    );
                    break;
                }
                acquire(graph.node(node), &mut held_resources);
                active.insert(node, ActiveLock::from_node(graph.node(node)));
                states[node] = NodeState::Active;
            }

            if active.is_empty() {
                let unfinished = states
                    .iter()
                    .any(|state| matches!(state, NodeState::Pending | NodeState::Active));
                if !unfinished {
                    break;
                }
                if scheduler_failure.is_none()
                    && !(scheduler_options.fail_fast() && failure_observed)
                {
                    scheduler_failure = Some(failure_snapshot(
                        graph,
                        &states,
                        &active,
                        "scheduler has pending nodes but no runnable work",
                    ));
                }
                cancel_all_pending(
                    graph,
                    &mut states,
                    &mut ready,
                    &mut cancellations,
                    "scheduler-internal",
                );
                break;
            }

            let message = match result_receiver.recv() {
                Ok(message) => message,
                Err(_) => {
                    scheduler_failure = Some(failure_snapshot(
                        graph,
                        &states,
                        &active,
                        "worker result channel closed with active nodes",
                    ));
                    for node in active.keys().copied().collect::<Vec<_>>() {
                        states[node] = NodeState::Cancelled;
                        cancellations.insert(node, "scheduler-internal".to_owned());
                    }
                    cancel_all_pending(
                        graph,
                        &mut states,
                        &mut ready,
                        &mut cancellations,
                        "scheduler-internal",
                    );
                    break;
                }
            };
            let node = message.node();
            if let Some(lock) = active.remove(&node) {
                release(&lock, &mut held_resources);
            }

            match message {
                WorkerMessage::Completed { node, result } => {
                    let result = *result;
                    let passed = result.passed();
                    results[node] = Some(result);
                    states[node] = NodeState::Finished;
                    if passed {
                        release_dependents(
                            graph,
                            node,
                            &states,
                            &mut remaining_dependencies,
                            &mut ready,
                        );
                    } else {
                        failure_observed = true;
                        cancel_dependents(
                            graph,
                            node,
                            &mut states,
                            &mut ready,
                            &mut cancellations,
                            graph.node(node).id(),
                        );
                        if scheduler_options.fail_fast() {
                            cancel_all_pending(
                                graph,
                                &mut states,
                                &mut ready,
                                &mut cancellations,
                                "fail-fast",
                            );
                        }
                    }
                }
                WorkerMessage::Panicked { node, message } => {
                    states[node] = NodeState::Cancelled;
                    cancellations.insert(node, "scheduler-internal".to_owned());
                    if scheduler_failure.is_none() {
                        scheduler_failure = Some(panic_failure_snapshot(
                            graph,
                            &states,
                            &active,
                            node,
                            format!(
                                "worker panicked while executing {}: {message}",
                                graph.node(node).id()
                            ),
                        ));
                    }
                    cancel_all_pending(
                        graph,
                        &mut states,
                        &mut ready,
                        &mut cancellations,
                        "scheduler-internal",
                    );
                }
            }
        }
        drop(work_sender);
    });

    CoordinatorOutput {
        results,
        cancellations,
        scheduler_failure,
        elapsed: std::time::Duration::ZERO,
    }
}

fn panic_failure_snapshot(
    graph: &ExecutionGraph,
    states: &[NodeState],
    active: &BTreeMap<usize, ActiveLock>,
    panicked: usize,
    message: impl Into<String>,
) -> SchedulerFailure {
    let mut active_nodes = vec![graph.node(panicked).id().to_owned()];
    active_nodes.extend(active.keys().map(|node| graph.node(*node).id().to_owned()));
    active_nodes.sort();
    let pending_nodes = states
        .iter()
        .enumerate()
        .filter(|(_, state)| **state == NodeState::Pending)
        .map(|(node, _)| graph.node(node).id().to_owned())
        .collect();
    SchedulerFailure::new(message, active_nodes, pending_nodes)
}

fn materialize_task(
    graph: &ExecutionGraph,
    node: usize,
    results: &[Option<TaskResult>],
) -> ScheduledTask {
    match graph.node(node).kind() {
        NodeKind::Runtime => ScheduledTask::Runtime,
        NodeKind::Compile { build, purpose } => ScheduledTask::Compile {
            build: build.clone(),
            purpose: purpose.clone(),
        },
        NodeKind::Link { build } => {
            let compilation = results[graph.compile(build.id())]
                .as_ref()
                .and_then(TaskResult::compilation)
                .expect("ready link must have a successful compilation")
                .clone();
            ScheduledTask::Link {
                build: build.clone(),
                compilation,
            }
        }
        NodeKind::Run { leaf } => {
            let link = results[graph
                .link(leaf.build_id())
                .expect("native run must have a link node")]
            .as_ref()
            .and_then(TaskResult::link)
            .expect("ready run must have a successful link");
            ScheduledTask::Run {
                leaf: leaf.clone(),
                executable: link.executable().to_path_buf(),
            }
        }
    }
}

fn execute_task(task: ScheduledTask, options: &SequentialOptions) -> TaskResult {
    match task {
        ScheduledTask::Runtime => TaskResult::Runtime(prepare_runtime(options)),
        ScheduledTask::Compile { build, purpose } => {
            let purpose = match &purpose {
                CompilationPurpose::Success => BorrowedCompilationPurpose::Success,
                CompilationPurpose::CompileFail(expectation) => {
                    BorrowedCompilationPurpose::CompileFail(expectation)
                }
            };
            TaskResult::Compilation(compile_build(
                &build,
                purpose,
                options.compiler(),
                options.determinism(),
            ))
        }
        ScheduledTask::Link { build, compilation } => {
            TaskResult::Link(link_build(&build, &compilation, options))
        }
        ScheduledTask::Run { leaf, executable } => {
            TaskResult::Run(execute_native_leaf(&leaf, &executable, options))
        }
    }
}

fn release_dependents(
    graph: &ExecutionGraph,
    completed: usize,
    states: &[NodeState],
    remaining: &mut [usize],
    ready: &mut BTreeSet<(String, usize)>,
) {
    for dependent in graph.node(completed).dependents() {
        if states[*dependent] != NodeState::Pending {
            continue;
        }
        remaining[*dependent] -= 1;
        if remaining[*dependent] == 0 {
            ready.insert((graph.node(*dependent).sort_key().to_owned(), *dependent));
        }
    }
}

fn cancel_dependents(
    graph: &ExecutionGraph,
    failed: usize,
    states: &mut [NodeState],
    ready: &mut BTreeSet<(String, usize)>,
    cancellations: &mut BTreeMap<usize, String>,
    dependency: &str,
) {
    let mut pending = graph.node(failed).dependents().to_vec();
    while let Some(node) = pending.pop() {
        if states[node] != NodeState::Pending {
            continue;
        }
        states[node] = NodeState::Cancelled;
        ready.remove(&(graph.node(node).sort_key().to_owned(), node));
        cancellations.insert(node, dependency.to_owned());
        pending.extend(graph.node(node).dependents());
    }
}

fn cancel_all_pending(
    graph: &ExecutionGraph,
    states: &mut [NodeState],
    ready: &mut BTreeSet<(String, usize)>,
    cancellations: &mut BTreeMap<usize, String>,
    reason: &str,
) {
    for (node, state) in states.iter_mut().enumerate() {
        if *state == NodeState::Pending {
            *state = NodeState::Cancelled;
            cancellations.insert(node, reason.to_owned());
        }
    }
    ready.clear();
    debug_assert_eq!(states.len(), graph.nodes().len());
}

fn can_start(
    node: &super::graph::Node,
    active: &BTreeMap<usize, ActiveLock>,
    held_resources: &BTreeSet<String>,
) -> bool {
    if node.serial() {
        return active.is_empty();
    }
    if active.values().any(|lock| lock.serial) {
        return false;
    }
    node.resources()
        .iter()
        .all(|resource| !held_resources.contains(resource))
}

fn acquire(node: &super::graph::Node, held_resources: &mut BTreeSet<String>) {
    for resource in node.resources() {
        assert!(held_resources.insert(resource.clone()));
    }
}

fn release(lock: &ActiveLock, held_resources: &mut BTreeSet<String>) {
    for resource in &lock.resources {
        assert!(held_resources.remove(resource));
    }
}

fn failure_snapshot(
    graph: &ExecutionGraph,
    states: &[NodeState],
    active: &BTreeMap<usize, ActiveLock>,
    message: impl Into<String>,
) -> SchedulerFailure {
    let active_nodes = active
        .keys()
        .map(|node| graph.node(*node).id().to_owned())
        .collect();
    let pending_nodes = states
        .iter()
        .enumerate()
        .filter(|(_, state)| **state == NodeState::Pending)
        .map(|(node, _)| graph.node(node).id().to_owned())
        .collect();
    SchedulerFailure::new(message, active_nodes, pending_nodes)
}

fn panic_message(payload: Box<dyn Any + Send>) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|message| (*message).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string panic payload".to_owned())
}

#[derive(Clone)]
pub(super) enum ScheduledTask {
    Runtime,
    Compile {
        build: crate::PlannedBuild,
        purpose: CompilationPurpose,
    },
    Link {
        build: crate::PlannedBuild,
        compilation: CompilationExecution,
    },
    Run {
        leaf: PlannedLeaf,
        executable: std::path::PathBuf,
    },
}

pub(super) enum TaskResult {
    Runtime(RuntimeExecution),
    Compilation(CompilationExecution),
    Link(LinkExecution),
    Run(LeafExecution),
}

impl TaskResult {
    fn passed(&self) -> bool {
        match self {
            Self::Runtime(result) => result.status().passed(),
            Self::Compilation(result) => result.passed(),
            Self::Link(result) => result.status().passed(),
            Self::Run(result) => result.status().passed(),
        }
    }

    fn compilation(&self) -> Option<&CompilationExecution> {
        match self {
            Self::Compilation(result) => Some(result),
            _ => None,
        }
    }

    fn link(&self) -> Option<&LinkExecution> {
        match self {
            Self::Link(result) => Some(result),
            _ => None,
        }
    }
}

pub(super) struct CoordinatorOutput {
    pub(super) results: Vec<Option<TaskResult>>,
    pub(super) cancellations: BTreeMap<usize, String>,
    pub(super) scheduler_failure: Option<SchedulerFailure>,
    pub(super) elapsed: std::time::Duration,
}

struct WorkItem {
    node: usize,
    task: ScheduledTask,
}

enum WorkerMessage {
    Completed {
        node: usize,
        result: Box<TaskResult>,
    },
    Panicked {
        node: usize,
        message: String,
    },
}

impl WorkerMessage {
    fn node(&self) -> usize {
        match self {
            Self::Completed { node, .. } | Self::Panicked { node, .. } => *node,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NodeState {
    Pending,
    Active,
    Finished,
    Cancelled,
}

struct ActiveLock {
    serial: bool,
    resources: Vec<String>,
}

impl ActiveLock {
    fn from_node(node: &super::graph::Node) -> Self {
        Self {
            serial: node.serial(),
            resources: node.resources().to_vec(),
        }
    }
}

use crate::{PlannedBuild, PlannedLeaf, PlannedLeafKind, ResolvedCompileExpectation, SelectedPlan};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(super) struct ExecutionGraph {
    nodes: Vec<Node>,
    runtime: Option<usize>,
    compile_by_build: BTreeMap<String, usize>,
    link_by_build: BTreeMap<String, usize>,
    run_by_leaf: BTreeMap<String, usize>,
}

impl ExecutionGraph {
    pub(super) fn new(selected: &SelectedPlan<'_>) -> Self {
        let grouped = group_leaves(selected.leaves());
        let needs_runtime = selected
            .leaves()
            .iter()
            .any(|leaf| matches!(leaf.kind(), PlannedLeafKind::Run(_)));
        let mut graph = Self {
            nodes: Vec::new(),
            runtime: None,
            compile_by_build: BTreeMap::new(),
            link_by_build: BTreeMap::new(),
            run_by_leaf: BTreeMap::new(),
        };

        if needs_runtime {
            graph.runtime = Some(graph.push(Node::new(
                "runtime",
                "0::runtime",
                NodeKind::Runtime,
                Vec::new(),
                false,
                Vec::new(),
            )));
        }

        for (build_id, leaves) in grouped {
            let build = selected
                .plan()
                .build(build_id)
                .expect("selected leaf must reference a planned build")
                .clone();
            let purpose = match leaves[0].kind() {
                PlannedLeafKind::Run(_) => CompilationPurpose::Success,
                PlannedLeafKind::Compile(expectation) => {
                    CompilationPurpose::CompileFail(expectation.clone())
                }
            };
            let compile = graph.push(Node::new(
                format!("{}::<compiler>", build.id()),
                format!("1::{}", build.id()),
                NodeKind::Compile {
                    build: build.clone(),
                    purpose,
                },
                Vec::new(),
                build.serial(),
                build.resources().to_vec(),
            ));
            graph
                .compile_by_build
                .insert(build.id().to_owned(), compile);

            if matches!(leaves[0].kind(), PlannedLeafKind::Run(_)) {
                let mut dependencies = vec![compile];
                dependencies.extend(graph.runtime);
                let link = graph.push(Node::new(
                    format!("{}::<link>", build.id()),
                    format!("2::{}", build.id()),
                    NodeKind::Link {
                        build: build.clone(),
                    },
                    dependencies,
                    build.serial(),
                    build.resources().to_vec(),
                ));
                graph.link_by_build.insert(build.id().to_owned(), link);

                for leaf in leaves {
                    let PlannedLeafKind::Run(run) = leaf.kind() else {
                        unreachable!("one build cannot mix run and compile-fail leaves");
                    };
                    let run_node = graph.push(Node::new(
                        leaf.id(),
                        format!("3::{}", leaf.id()),
                        NodeKind::Run { leaf: leaf.clone() },
                        vec![link],
                        run.serial(),
                        run.resources().to_vec(),
                    ));
                    graph.run_by_leaf.insert(leaf.id().to_owned(), run_node);
                }
            }
        }

        let edges = graph
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(dependent, node)| {
                node.dependencies
                    .iter()
                    .copied()
                    .map(move |dependency| (dependency, dependent))
            })
            .collect::<Vec<_>>();
        for (dependency, dependent) in edges {
            graph.nodes[dependency].dependents.push(dependent);
        }
        graph
    }

    fn push(&mut self, node: Node) -> usize {
        let index = self.nodes.len();
        self.nodes.push(node);
        index
    }

    pub(super) fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub(super) fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    pub(super) fn runtime(&self) -> Option<usize> {
        self.runtime
    }

    pub(super) fn compile(&self, build_id: &str) -> usize {
        self.compile_by_build[build_id]
    }

    pub(super) fn link(&self, build_id: &str) -> Option<usize> {
        self.link_by_build.get(build_id).copied()
    }

    pub(super) fn run(&self, leaf_id: &str) -> Option<usize> {
        self.run_by_leaf.get(leaf_id).copied()
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

#[derive(Clone)]
pub(super) struct Node {
    id: String,
    sort_key: String,
    kind: NodeKind,
    dependencies: Vec<usize>,
    dependents: Vec<usize>,
    serial: bool,
    resources: Vec<String>,
}

impl Node {
    fn new(
        id: impl Into<String>,
        sort_key: impl Into<String>,
        kind: NodeKind,
        dependencies: Vec<usize>,
        serial: bool,
        mut resources: Vec<String>,
    ) -> Self {
        resources.sort();
        resources.dedup();
        Self {
            id: id.into(),
            sort_key: sort_key.into(),
            kind,
            dependencies,
            dependents: Vec::new(),
            serial,
            resources,
        }
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn sort_key(&self) -> &str {
        &self.sort_key
    }

    pub(super) fn kind(&self) -> &NodeKind {
        &self.kind
    }

    pub(super) fn dependencies(&self) -> &[usize] {
        &self.dependencies
    }

    pub(super) fn dependents(&self) -> &[usize] {
        &self.dependents
    }

    pub(super) fn serial(&self) -> bool {
        self.serial
    }

    pub(super) fn resources(&self) -> &[String] {
        &self.resources
    }
}

#[derive(Clone)]
pub(super) enum NodeKind {
    Runtime,
    Compile {
        build: PlannedBuild,
        purpose: CompilationPurpose,
    },
    Link {
        build: PlannedBuild,
    },
    Run {
        leaf: PlannedLeaf,
    },
}

#[derive(Clone)]
pub(super) enum CompilationPurpose {
    Success,
    CompileFail(ResolvedCompileExpectation),
}

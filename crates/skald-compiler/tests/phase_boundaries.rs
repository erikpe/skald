//! Source-level guards for compiler phase dependency direction.

#[path = "phase_boundaries/source_scan.rs"]
mod source_scan;

use std::{collections::BTreeSet, fs, path::Path};

use source_scan::{crate_root_references, root_references};

const PHASE_ROOTS: &[&str] = &[
    "source",
    "lexer",
    "syntax",
    "module",
    "resolve",
    "hir",
    "typeck",
    "mir",
    "passes",
    "backend",
    "reporting",
    "driver",
];

const SUPPORTING_SERVICE_ROOTS: &[&str] = &["type_capabilities"];

struct DependencyPolicy {
    root: &'static str,
    allowed: &'static [&'static str],
}

const POLICIES: &[DependencyPolicy] = &[
    DependencyPolicy {
        root: "source",
        allowed: &[],
    },
    DependencyPolicy {
        root: "lexer",
        allowed: &["source"],
    },
    DependencyPolicy {
        root: "syntax",
        allowed: &["source", "lexer"],
    },
    DependencyPolicy {
        root: "module",
        allowed: &["source", "lexer", "syntax"],
    },
    DependencyPolicy {
        root: "resolve",
        allowed: &["source", "lexer", "syntax", "module", "type_capabilities"],
    },
    DependencyPolicy {
        root: "hir",
        allowed: &["source", "module", "resolve"],
    },
    DependencyPolicy {
        root: "typeck",
        allowed: &["source", "resolve", "hir", "type_capabilities"],
    },
    DependencyPolicy {
        root: "mir",
        allowed: &["source", "module", "resolve", "hir"],
    },
    DependencyPolicy {
        root: "passes",
        allowed: &["source", "mir"],
    },
    DependencyPolicy {
        root: "backend",
        allowed: &["source", "mir", "passes"],
    },
    DependencyPolicy {
        root: "reporting",
        allowed: &["passes"],
    },
    DependencyPolicy {
        root: "driver",
        allowed: PHASE_ROOTS,
    },
    DependencyPolicy {
        root: "type_capabilities",
        allowed: &["source", "resolve"],
    },
];

struct TemporaryException {
    owner: &'static str,
    dependency: &'static str,
    path: &'static str,
}

// MIR retention currently consumes the sealed whole-world reachability result
// owned by the pass pipeline. Keep this exact reverse edge visible until that
// authority boundary can move without exposing a caller-selected identity set.
const TEMPORARY_EXCEPTIONS: &[TemporaryException] = &[TemporaryException {
    owner: "mir",
    dependency: "passes",
    path: "mir/retain/mod.rs",
}];

#[test]
fn production_compiler_dependencies_follow_owned_boundaries() {
    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();
    let mut observed_exceptions = BTreeSet::new();

    for policy in POLICIES {
        let directory = source_root.join(policy.root);
        let file = source_root.join(format!("{}.rs", policy.root));
        let phase_root = if directory.is_dir() {
            &directory
        } else {
            &file
        };
        visit_rust_sources(phase_root, &mut |path| {
            let relative = path.strip_prefix(&source_root).unwrap();
            if is_test_source(relative) {
                return;
            }
            let source = fs::read_to_string(path).expect("Rust source is valid UTF-8");
            for reference in root_references(&source, module_depth(relative)) {
                if let Some(exception) = temporary_exception(policy, relative, &reference.root) {
                    observed_exceptions.insert((
                        exception.owner,
                        exception.dependency,
                        exception.path,
                    ));
                }
                if !dependency_allowed(policy, relative, &reference.root) {
                    violations.push(format!(
                        "{}:{}: `{}` may not depend on `{}`",
                        relative.display(),
                        reference.line,
                        policy.root,
                        reference.root,
                    ));
                }
            }
        });
    }
    for exception in TEMPORARY_EXCEPTIONS {
        if !observed_exceptions.contains(&(exception.owner, exception.dependency, exception.path)) {
            violations.push(format!(
                "stale temporary exception: `{}` -> `{}` at {}",
                exception.owner, exception.dependency, exception.path
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "compiler phase dependency violations:\n{}",
        violations.join("\n")
    );
}

fn module_depth(path: &Path) -> usize {
    let components = path.components().count();
    if path.file_stem().is_some_and(|stem| stem == "mod") {
        components - 1
    } else {
        components
    }
}

fn dependency_allowed(policy: &DependencyPolicy, path: &Path, dependency: &str) -> bool {
    dependency == policy.root
        || !is_governed_root(dependency)
        || policy.allowed.contains(&dependency)
        || temporary_exception(policy, path, dependency).is_some()
}

fn is_governed_root(root: &str) -> bool {
    PHASE_ROOTS.contains(&root) || SUPPORTING_SERVICE_ROOTS.contains(&root)
}

fn temporary_exception(
    policy: &DependencyPolicy,
    path: &Path,
    dependency: &str,
) -> Option<&'static TemporaryException> {
    TEMPORARY_EXCEPTIONS.iter().find(|exception| {
        exception.owner == policy.root
            && exception.dependency == dependency
            && path == Path::new(exception.path)
    })
}

fn is_test_source(path: &Path) -> bool {
    path.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "tests" || name == "test_fixtures"
    }) || path.file_stem().is_some_and(|stem| {
        let stem = stem.to_string_lossy();
        stem == "tests"
            || stem.ends_with("_tests")
            || stem == "test_support"
            || stem == "test_fixtures"
    })
}

fn visit_rust_sources(directory: &Path, visit: &mut impl FnMut(&Path)) {
    if directory.is_file() {
        visit(directory);
        return;
    }
    for entry in fs::read_dir(directory).expect("phase source directory is readable") {
        let path = entry.expect("phase source entry is readable").path();
        if path.is_dir() {
            visit_rust_sources(&path, visit);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            visit(&path);
        }
    }
}

#[test]
fn test_source_detection_matches_the_repository_conventions() {
    for path in [
        "resolve/tests.rs",
        "resolve/tests/classes.rs",
        "mir/verify/shape_tests.rs",
        "backend/test_support.rs",
        "mir/test_fixtures.rs",
        "mir/test_fixtures/program.rs",
    ] {
        assert!(is_test_source(Path::new(path)), "did not exclude {path}");
    }
    for path in [
        "resolve/resolver.rs",
        "passes/static_lifecycle/verify.rs",
        "backend/x86_64_sysv/lower.rs",
    ] {
        assert!(!is_test_source(Path::new(path)), "excluded {path}");
    }
}

#[test]
fn policy_rejects_reverse_edges_and_accepts_lowering_inputs() {
    let resolve = POLICIES
        .iter()
        .find(|policy| policy.root == "resolve")
        .unwrap();
    let synthetic = "use crate::{syntax::CompilationUnit, typeck::TypeCheckOutput};";
    let violations = crate_root_references(synthetic)
        .into_iter()
        .filter(|reference| {
            !dependency_allowed(resolve, Path::new("resolve/resolver.rs"), &reference.root)
        })
        .map(|reference| reference.root)
        .collect::<Vec<_>>();
    assert_eq!(violations, ["typeck"]);

    let backend = POLICIES
        .iter()
        .find(|policy| policy.root == "backend")
        .unwrap();
    assert!(dependency_allowed(
        backend,
        Path::new("backend/x86_64_sysv/lower.rs"),
        "mir"
    ));

    let mir = POLICIES.iter().find(|policy| policy.root == "mir").unwrap();
    assert!(dependency_allowed(
        mir,
        Path::new("mir/retain/mod.rs"),
        "passes"
    ));
    assert!(!dependency_allowed(
        mir,
        Path::new("mir/lower.rs"),
        "passes"
    ));
}

#[test]
fn neutral_capability_policy_accepts_resolved_inputs_and_rejects_later_products() {
    let capabilities = POLICIES
        .iter()
        .find(|policy| policy.root == "type_capabilities")
        .unwrap();
    let synthetic = "use crate::{\n\
        source::Span,\n\
        resolve::ResolvedProgram,\n\
        hir::HirProgram,\n\
        typeck::TypeCheckOutput,\n\
        mir::MirProgram,\n\
        passes::OptimizationLevel,\n\
        backend::Backend,\n\
    };";
    let violations = crate_root_references(synthetic)
        .into_iter()
        .filter(|reference| {
            !dependency_allowed(
                capabilities,
                Path::new("type_capabilities/mod.rs"),
                &reference.root,
            )
        })
        .map(|reference| reference.root)
        .collect::<Vec<_>>();

    assert_eq!(violations, ["hir", "typeck", "mir", "passes", "backend"]);
}

#[test]
fn neutral_capability_consumers_are_explicit() {
    for root in ["resolve", "typeck"] {
        let policy = POLICIES.iter().find(|policy| policy.root == root).unwrap();
        assert!(dependency_allowed(
            policy,
            Path::new("synthetic.rs"),
            "type_capabilities"
        ));
    }

    for root in [
        "source",
        "lexer",
        "syntax",
        "module",
        "hir",
        "mir",
        "passes",
        "backend",
        "reporting",
        "driver",
    ] {
        let policy = POLICIES.iter().find(|policy| policy.root == root).unwrap();
        assert!(!dependency_allowed(
            policy,
            Path::new("synthetic.rs"),
            "type_capabilities"
        ));
    }
}

#[test]
fn every_governed_root_has_one_policy() {
    for root in PHASE_ROOTS.iter().chain(SUPPORTING_SERVICE_ROOTS) {
        assert_eq!(
            POLICIES
                .iter()
                .filter(|policy| policy.root == *root)
                .count(),
            1,
            "expected exactly one dependency policy for `{root}`"
        );
    }
}

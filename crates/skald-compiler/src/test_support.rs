//! Test-only compiler pipelines and temporary resources.
//!
//! Each source helper asserts success only for phases before the boundary its
//! name exposes. Tests remain responsible for checking the requested phase.

use std::{
    fs::{self, OpenOptions},
    io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    backend::{emit_assembly, BackendError, Target},
    lexer::{lex, LexOutput},
    mir::{lower_hir, MirProgram},
    resolve::{resolve, ResolveOutput},
    source::{SourceDatabase, SourceId},
    syntax::{parse, ParseOutput},
    typeck::{type_check, TypeCheckOutput},
};

static NEXT_TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

pub(crate) const INLINE_FIELD_SOURCE: &str = concat!(
    "class Root {\n",
    "  flag: bool; left: Branch; right: Branch;\n",
    "  init(left: i64, right: i64) {\n",
    "    self.right = Branch(right);\n",
    "    self.flag = true;\n",
    "    self.left = Branch(left);\n",
    "  }\n",
    "  fn total() -> i64 { return self.left.leaf.value + self.right.leaf.read(); }\n",
    "  mut fn adjust() -> i64 {\n",
    "    self.left.leaf.value = self.left.leaf.value + 1;\n",
    "    return mutate(self.right.leaf, 5) + self.left.leaf.read();\n",
    "  }\n",
    "}\n",
    "class Empty { init() {} }\n",
    "class Leaf {\n",
    "  small: u8; value: i64;\n",
    "  init(value: i64) { self.value = value; self.small = 1u8; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  mut fn add(delta: i64) -> i64 { self.value = self.value + delta; return self.value; }\n",
    "}\n",
    "class Branch {\n",
    "  tag: u8; empty: Empty; leaf: Leaf; tail: u8;\n",
    "  init(value: i64) {\n",
    "    self.leaf = Leaf(value); self.tag = 2u8; self.empty = Empty(); self.tail = 3u8;\n",
    "  }\n",
    "}\n",
    "fn read(ref leaf: Leaf) -> i64 { return leaf.read(); }\n",
    "fn mutate(mut ref leaf: Leaf, delta: i64) -> i64 { return leaf.add(delta); }\n",
    "fn forward(mut ref root: Root) -> i64 {\n",
    "  return mutate(root.left.leaf, 3) + read(root.right.leaf);\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  var root: Root = Root(10, 20);\n",
    "  return forward(root) + root.adjust() + root.total();\n",
    "}\n",
);

pub(crate) fn lex_source(text: impl Into<String>) -> (SourceDatabase, SourceId, LexOutput) {
    let mut sources = SourceDatabase::new();
    let source_id = sources.add("test.ska", text);
    let output = lex(sources
        .get(source_id)
        .expect("test source was just inserted"));
    (sources, source_id, output)
}

pub(crate) fn parse_source(text: impl Into<String>) -> (SourceDatabase, ParseOutput) {
    let (sources, source_id, lexed) = lex_source(text);
    assert_phase_succeeded("lexing", &lexed.diagnostics);
    let parsed = parse(
        sources
            .get(source_id)
            .expect("test source was just inserted"),
        &lexed.tokens,
    );
    (sources, parsed)
}

pub(crate) fn resolve_source(text: impl Into<String>) -> ResolveOutput {
    let (_, parsed) = parse_source(text);
    assert_phase_succeeded("parsing", &parsed.diagnostics);
    resolve(&parsed.ast)
}

pub(crate) fn type_check_source(text: impl Into<String>) -> TypeCheckOutput {
    let resolved = resolve_source(text);
    assert_phase_succeeded("resolution", &resolved.diagnostics);
    type_check(&resolved.program)
}

pub(crate) fn lower_source_to_mir(text: impl Into<String>) -> MirProgram {
    let checked = type_check_source(text);
    assert_phase_succeeded("type checking", &checked.diagnostics);
    lower_hir(
        &checked
            .hir
            .expect("successful type checking must produce typed HIR"),
    )
    .expect("test source must use HIR representable in MIR")
}

pub(crate) fn lower_source_to_assembly(
    text: impl Into<String>,
    target: Target,
) -> Result<String, BackendError> {
    emit_assembly(target, &lower_source_to_mir(text))
}

fn assert_phase_succeeded(phase: &str, diagnostics: &crate::diagnostics::Diagnostics) {
    assert!(
        diagnostics.is_empty(),
        "test source failed during {phase}: {:?}",
        diagnostics
    );
}

#[derive(Debug)]
pub(crate) struct TemporaryDirectory {
    path: PathBuf,
}

impl TemporaryDirectory {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        loop {
            let path = temporary_path(label, "dir");
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn join(&self, path: impl AsRef<Path>) -> PathBuf {
        self.path.join(path)
    }
}

impl Drop for TemporaryDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
pub(crate) struct TemporaryFile {
    path: PathBuf,
}

impl TemporaryFile {
    pub(crate) fn new(label: &str) -> io::Result<Self> {
        loop {
            let path = temporary_path(label, "file");
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    drop(file);
                    return Ok(Self { path });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TemporaryFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn temporary_path(label: &str, kind: &str) -> PathBuf {
    let id = NEXT_TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "skald-test-{kind}-{}-{id}-{label}",
        std::process::id()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_helpers_leave_the_requested_boundary_for_the_test() {
        let (_, _, lexed) = lex_source("@");
        assert!(lexed.has_errors());

        let (_, parsed) = parse_source("fn");
        assert!(parsed.has_errors());

        let resolved = resolve_source("fn main() -> i64 { return missing; }");
        assert!(resolved.has_errors());

        let checked = type_check_source("fn main() -> i64 { return true; }");
        assert!(checked.has_errors());

        let mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
        assert!(!mir.definitions.is_empty());
        assert!(
            lower_source_to_assembly("fn main() -> i64 { return 0; }", Target::X86_64SysV).is_ok()
        );
    }

    #[test]
    fn temporary_directories_are_unique_and_removed_on_drop() {
        let first = TemporaryDirectory::new("unique").unwrap();
        let second = TemporaryDirectory::new("unique").unwrap();
        let first_path = first.path().to_owned();
        let second_path = second.path().to_owned();

        assert_ne!(first_path, second_path);
        assert!(first_path.is_dir());
        assert!(second_path.is_dir());
        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }

    #[test]
    fn temporary_files_are_unique_and_removed_on_drop() {
        let first = TemporaryFile::new("unique").unwrap();
        let second = TemporaryFile::new("unique").unwrap();
        let first_path = first.path().to_owned();
        let second_path = second.path().to_owned();

        assert_ne!(first_path, second_path);
        assert!(first_path.is_file());
        assert!(second_path.is_file());
        drop(first);
        drop(second);
        assert!(!first_path.exists());
        assert!(!second_path.exists());
    }

    #[test]
    fn temporary_resources_are_removed_during_assertion_unwinding() {
        let directory = TemporaryDirectory::new("unwind").unwrap();
        let file = TemporaryFile::new("unwind").unwrap();
        let directory_path = directory.path().to_owned();
        let file_path = file.path().to_owned();

        let result = std::panic::catch_unwind(move || {
            let _directory = directory;
            let _file = file;
            panic!("simulated failed assertion");
        });

        assert!(result.is_err());
        assert!(!directory_path.exists());
        assert!(!file_path.exists());
    }
}

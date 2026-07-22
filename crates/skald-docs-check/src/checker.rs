use crate::markdown::{self, Document};
use std::{
    collections::{HashMap, HashSet},
    fmt, fs, io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Diagnostic {
    path: PathBuf,
    line: Option<usize>,
    message: String,
}

impl Diagnostic {
    fn link(path: PathBuf, line: usize, message: impl Into<String>) -> Self {
        Self {
            path,
            line: Some(line),
            message: message.into(),
        }
    }

    fn index(path: PathBuf, message: impl Into<String>) -> Self {
        Self {
            path,
            line: None,
            message: message.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}", self.path.display())?;
        if let Some(line) = self.line {
            write!(formatter, ":{line}")?;
        }
        write!(formatter, ": {}", self.message)
    }
}

pub fn check_repository(root: impl AsRef<Path>) -> io::Result<Vec<Diagnostic>> {
    let root = root.as_ref().canonicalize()?;
    let mut markdown_paths = Vec::new();
    collect_markdown(&root, &root, &mut markdown_paths)?;
    markdown_paths.sort();

    let mut documents = HashMap::new();
    for path in &markdown_paths {
        documents.insert(path.clone(), parse_file(path)?);
    }

    let mut diagnostics = Vec::new();
    for path in &markdown_paths {
        let links = documents[path].links.clone();
        for link in &links {
            check_link(
                &root,
                path,
                link.line,
                &link.destination,
                &mut documents,
                &mut diagnostics,
            )?;
        }
    }

    check_indexes(&root, &documents, &mut diagnostics);
    diagnostics.sort_by(|left, right| {
        (&left.path, left.line, &left.message).cmp(&(&right.path, right.line, &right.message))
    });
    Ok(diagnostics)
}

fn collect_markdown(root: &Path, directory: &Path, output: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            let name = entry.file_name();
            if name.to_string_lossy().starts_with('.')
                || matches!(name.to_str(), Some("build" | "target"))
            {
                continue;
            }
            collect_markdown(root, &path, output)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("md")
        {
            output.push(path);
        }
    }
    debug_assert!(output.iter().all(|path| path.starts_with(root)));
    Ok(())
}

fn parse_file(path: &Path) -> io::Result<Document> {
    fs::read_to_string(path).map(|source| markdown::parse(&source))
}

fn check_link(
    root: &Path,
    source_path: &Path,
    line: usize,
    destination: &str,
    documents: &mut HashMap<PathBuf, Document>,
    diagnostics: &mut Vec<Diagnostic>,
) -> io::Result<()> {
    if is_external(destination) {
        return Ok(());
    }
    let (path_part, fragment) = split_destination(destination);
    let decoded_path = match percent_decode(path_part) {
        Ok(path) => path,
        Err(message) => {
            diagnostics.push(Diagnostic::link(relative(root, source_path), line, message));
            return Ok(());
        }
    };
    let Some(target) = resolve_target(root, source_path, &decoded_path) else {
        return Ok(());
    };

    if !target.is_file() {
        diagnostics.push(Diagnostic::link(
            relative(root, source_path),
            line,
            format!("missing file `{}`", path_part),
        ));
        return Ok(());
    }

    if let Some(fragment) = fragment {
        if target.extension().and_then(|value| value.to_str()) != Some("md") {
            return Ok(());
        }
        let decoded_fragment = match percent_decode(fragment) {
            Ok(fragment) => fragment,
            Err(message) => {
                diagnostics.push(Diagnostic::link(relative(root, source_path), line, message));
                return Ok(());
            }
        };
        if !documents.contains_key(&target) {
            documents.insert(target.clone(), parse_file(&target)?);
        }
        if !documents[&target].anchors.contains(&decoded_fragment) {
            diagnostics.push(Diagnostic::link(
                relative(root, source_path),
                line,
                format!(
                    "missing anchor `#{decoded_fragment}` in `{}`",
                    relative(root, &target).display()
                ),
            ));
        }
    }

    Ok(())
}

fn split_destination(destination: &str) -> (&str, Option<&str>) {
    let (before_fragment, fragment) = destination
        .split_once('#')
        .map_or((destination, None), |(path, fragment)| {
            (path, Some(fragment))
        });
    (
        before_fragment
            .split_once('?')
            .map_or(before_fragment, |(path, _)| path),
        fragment,
    )
}

fn is_external(destination: &str) -> bool {
    if destination.starts_with("//") {
        return true;
    }
    let Some((scheme, _)) = destination.split_once(':') else {
        return false;
    };
    !scheme.is_empty()
        && scheme.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_alphabetic()
                || (index > 0 && (byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.')))
        })
}

fn resolve_target(root: &Path, source: &Path, decoded_path: &str) -> Option<PathBuf> {
    let joined = if decoded_path.is_empty() {
        source.to_owned()
    } else if let Some(path) = decoded_path.strip_prefix('/') {
        root.join(path)
    } else {
        source.parent()?.join(decoded_path)
    };
    let normalized = normalize(&joined);
    normalized.starts_with(root).then_some(normalized)
}

fn normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

fn percent_decode(value: &str) -> Result<String, String> {
    let bytes = value.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] != b'%' {
            output.push(bytes[index]);
            index += 1;
            continue;
        }
        if index + 2 >= bytes.len() {
            return Err(format!("invalid percent encoding in `{value}`"));
        }
        let Some(high) = hex_value(bytes[index + 1]) else {
            return Err(format!("invalid percent encoding in `{value}`"));
        };
        let Some(low) = hex_value(bytes[index + 2]) else {
            return Err(format!("invalid percent encoding in `{value}`"));
        };
        output.push(high * 16 + low);
        index += 3;
    }
    String::from_utf8(output).map_err(|_| format!("percent encoding in `{value}` is not UTF-8"))
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn check_indexes(
    root: &Path,
    documents: &HashMap<PathBuf, Document>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for directory in ["docs", "docs/roadmaps", "docs/archive"] {
        let directory = root.join(directory);
        let index = directory.join("README.md");
        let Some(index_document) = documents.get(&index) else {
            continue;
        };
        let linked = indexed_files(root, &index, index_document);
        let mut required: Vec<_> = documents
            .keys()
            .filter(|path| path.parent() == Some(directory.as_path()) && *path != &index)
            .collect();
        required.sort();
        for path in required {
            if !linked.contains(path) {
                diagnostics.push(Diagnostic::index(
                    relative(root, &index),
                    format!(
                        "missing required index entry for `{}`",
                        relative(root, path).display()
                    ),
                ));
            }
        }
    }
}

fn indexed_files(root: &Path, index: &Path, document: &Document) -> HashSet<PathBuf> {
    document
        .links
        .iter()
        .filter_map(|link| {
            let (path, _) = split_destination(&link.destination);
            let decoded = percent_decode(path).ok()?;
            resolve_target(root, index, &decoded)
        })
        .collect()
}

fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_owned()
}

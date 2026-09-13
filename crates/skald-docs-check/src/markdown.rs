use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Link {
    pub(crate) line: usize,
    pub(crate) destination: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Issue {
    pub(crate) line: usize,
    pub(crate) message: String,
}

#[derive(Debug, Default)]
pub(crate) struct Document {
    pub(crate) anchors: HashSet<String>,
    pub(crate) links: Vec<Link>,
    pub(crate) issues: Vec<Issue>,
}

#[derive(Clone, Copy)]
struct Fence {
    marker: u8,
    width: usize,
}

pub(crate) fn parse(source: &str) -> Document {
    let mut document = Document::default();
    let mut anchor_counts = HashMap::<String, usize>::new();
    let mut definitions = HashMap::<String, String>::new();
    let mut content_lines = Vec::new();
    let mut fence = None;

    for (index, line) in source.lines().enumerate() {
        let line_number = index + 1;
        if let Some(active) = fence {
            if closes_fence(line, active) {
                fence = None;
            }
            continue;
        }
        if let Some(opening) = opens_fence(line) {
            fence = Some(opening);
            continue;
        }

        if let Some(heading) = atx_heading(line) {
            let base = slugify(heading);
            let count = anchor_counts.entry(base.clone()).or_default();
            let anchor = if *count == 0 {
                base
            } else {
                format!("{base}-{count}")
            };
            *count += 1;
            document.anchors.insert(anchor);
        }

        if let Some((label, destination)) = reference_definition(line) {
            definitions.entry(label).or_insert(destination);
        } else {
            content_lines.push((line_number, line));
        }
    }

    for (line_number, line) in content_lines {
        scan_links(
            line,
            line_number,
            &definitions,
            &mut document.links,
            &mut document.issues,
        );
    }

    document
}

fn opens_fence(line: &str) -> Option<Fence> {
    let trimmed = markdown_line(line)?;
    let bytes = trimmed.as_bytes();
    let marker = *bytes.first()?;
    if !matches!(marker, b'`' | b'~') {
        return None;
    }
    let width = bytes.iter().take_while(|byte| **byte == marker).count();
    if width < 3 || marker == b'`' && bytes[width..].contains(&b'`') {
        return None;
    }
    Some(Fence { marker, width })
}

fn closes_fence(line: &str, fence: Fence) -> bool {
    let Some(trimmed) = markdown_line(line) else {
        return false;
    };
    let bytes = trimmed.as_bytes();
    let width = bytes
        .iter()
        .take_while(|byte| **byte == fence.marker)
        .count();
    width >= fence.width
        && bytes[width..]
            .iter()
            .all(|byte| matches!(byte, b' ' | b'\t'))
}

fn markdown_line(line: &str) -> Option<&str> {
    let indentation = line.bytes().take_while(|byte| *byte == b' ').count();
    (indentation <= 3).then(|| &line[indentation..])
}

fn atx_heading(line: &str) -> Option<&str> {
    let trimmed = markdown_line(line)?;
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with([' ', '\t']) {
        return None;
    }
    Some(trim_heading_closing_sequence(rest.trim()))
}

fn trim_heading_closing_sequence(heading: &str) -> &str {
    let hashes = heading
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'#')
        .count();
    if hashes == 0 {
        return heading;
    }
    let before_hashes = &heading[..heading.len() - hashes];
    if before_hashes.ends_with([' ', '\t']) {
        before_hashes.trim_end()
    } else {
        heading
    }
}

fn reference_definition(line: &str) -> Option<(String, String)> {
    let trimmed = markdown_line(line)?;
    let bytes = trimmed.as_bytes();
    if bytes.first() != Some(&b'[') {
        return None;
    }
    let label_end = matching_bracket(bytes, 0)?;
    if bytes.get(label_end + 1) != Some(&b':') {
        return None;
    }
    let label = &trimmed[1..label_end];
    if label.is_empty() || label.starts_with('^') {
        return None;
    }
    let destination = link_destination(trimmed[label_end + 2..].trim_start())?;
    let label = normalize_reference_label(label);
    (!label.is_empty()).then_some((label, destination))
}

fn scan_links(
    line: &str,
    line_number: usize,
    definitions: &HashMap<String, String>,
    links: &mut Vec<Link>,
    issues: &mut Vec<Issue>,
) {
    let bytes = line.as_bytes();
    let mut index = 0;
    let mut code_delimiter = 0;

    while index < bytes.len() {
        if bytes[index] == b'`' && !is_escaped(bytes, index) {
            let count = bytes[index..]
                .iter()
                .take_while(|byte| **byte == b'`')
                .count();
            if code_delimiter == 0 {
                code_delimiter = count;
            } else if code_delimiter == count {
                code_delimiter = 0;
            }
            index += count;
            continue;
        }
        if code_delimiter != 0 || bytes[index] != b'[' || is_escaped(bytes, index) {
            index += 1;
            continue;
        }

        let Some(label_end) = matching_bracket(bytes, index) else {
            break;
        };
        let label = &line[index + 1..label_end];
        match bytes.get(label_end + 1) {
            Some(b'(') => {
                let Some(destination_end) = matching_parenthesis(bytes, label_end + 1) else {
                    index = label_end + 1;
                    continue;
                };
                if let Some(destination) =
                    link_destination(line[label_end + 2..destination_end].trim())
                {
                    links.push(Link {
                        line: line_number,
                        destination,
                    });
                }
                index = destination_end + 1;
            }
            Some(b'[') => {
                let reference_start = label_end + 1;
                let Some(reference_end) = matching_bracket(bytes, reference_start) else {
                    index = label_end + 1;
                    continue;
                };
                let explicit_label = &line[reference_start + 1..reference_end];
                let reference_label = if explicit_label.is_empty() {
                    label
                } else {
                    explicit_label
                };
                resolve_reference(reference_label, line_number, definitions, links, issues);
                index = reference_end + 1;
            }
            _ => {
                let normalized = normalize_reference_label(label);
                if let Some(destination) = definitions.get(&normalized) {
                    links.push(Link {
                        line: line_number,
                        destination: destination.clone(),
                    });
                }
                index = label_end + 1;
            }
        }
    }
}

fn resolve_reference(
    label: &str,
    line: usize,
    definitions: &HashMap<String, String>,
    links: &mut Vec<Link>,
    issues: &mut Vec<Issue>,
) {
    let normalized = normalize_reference_label(label);
    if let Some(destination) = definitions.get(&normalized) {
        links.push(Link {
            line,
            destination: destination.clone(),
        });
    } else {
        issues.push(Issue {
            line,
            message: format!("undefined reference link `[{label}]`"),
        });
    }
}

fn matching_bracket(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0;
    for index in open..bytes.len() {
        if is_escaped(bytes, index) {
            continue;
        }
        match bytes[index] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn matching_parenthesis(bytes: &[u8], open: usize) -> Option<usize> {
    let mut depth = 0;
    let mut in_angle = false;
    for index in open..bytes.len() {
        if is_escaped(bytes, index) {
            continue;
        }
        match bytes[index] {
            b'<' if depth == 1 => in_angle = true,
            b'>' if depth == 1 => in_angle = false,
            b'(' if !in_angle => depth += 1,
            b')' if !in_angle => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn link_destination(raw: &str) -> Option<String> {
    let destination = if let Some(rest) = raw.strip_prefix('<') {
        let end = rest
            .as_bytes()
            .iter()
            .enumerate()
            .find(|(index, byte)| **byte == b'>' && !is_escaped(rest.as_bytes(), *index))?
            .0;
        &rest[..end]
    } else {
        let end = raw
            .char_indices()
            .find(|(_, character)| character.is_whitespace())
            .map_or(raw.len(), |(index, _)| index);
        &raw[..end]
    };
    (!destination.is_empty()).then(|| markdown_unescape(destination))
}

fn normalize_reference_label(label: &str) -> String {
    let mut normalized = String::new();
    let mut characters = label.chars().peekable();
    let mut pending_space = false;

    while let Some(character) = characters.next() {
        let character = if character == '\\' {
            match characters.peek() {
                Some(next) if next.is_ascii_punctuation() => characters.next().unwrap(),
                _ => character,
            }
        } else {
            character
        };
        if character.is_whitespace() {
            pending_space = !normalized.is_empty();
            continue;
        }
        if pending_space {
            normalized.push(' ');
            pending_space = false;
        }
        normalized.extend(character.to_lowercase());
    }

    normalized
}

fn markdown_unescape(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut characters = value.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\'
            && characters
                .peek()
                .is_some_and(|next| next.is_ascii_punctuation())
        {
            output.push(characters.next().unwrap());
        } else {
            output.push(character);
        }
    }
    output
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let slashes = bytes[..index]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count();
    slashes % 2 == 1
}

fn slugify(heading: &str) -> String {
    let mut slug = String::new();
    let mut in_tag = false;

    for character in heading.chars().flat_map(char::to_lowercase) {
        match character {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if in_tag => {}
            '-' | '_' => slug.push(character),
            _ if character.is_alphanumeric() => slug.push(character),
            _ if character.is_whitespace() => slug.push('-'),
            _ => {}
        }
    }

    slug
}

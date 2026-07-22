use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Eq, PartialEq)]
pub(crate) struct Link {
    pub(crate) line: usize,
    pub(crate) destination: String,
}

#[derive(Debug, Default)]
pub(crate) struct Document {
    pub(crate) anchors: HashSet<String>,
    pub(crate) links: Vec<Link>,
}

pub(crate) fn parse(source: &str) -> Document {
    let mut document = Document::default();
    let mut anchor_counts = HashMap::<String, usize>::new();
    let mut fence = None;

    for (index, line) in source.lines().enumerate() {
        if let Some(marker) = fence_marker(line) {
            if fence == Some(marker) {
                fence = None;
            } else if fence.is_none() {
                fence = Some(marker);
            }
            continue;
        }
        if fence.is_some() {
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

        if let Some(link) = reference_definition(line, index + 1) {
            document.links.push(link);
        }
        document.links.extend(inline_links(line, index + 1));
    }

    document
}

fn reference_definition(line: &str, line_number: usize) -> Option<Link> {
    let trimmed = line.trim_start_matches(' ');
    if line.len() - trimmed.len() > 3 || !trimmed.starts_with('[') {
        return None;
    }
    let label_end = trimmed.find("]: ").or_else(|| trimmed.find("]:"))?;
    if trimmed[1..label_end].starts_with('^') {
        return None;
    }
    let raw = trimmed[label_end + 2..].trim_start();
    let destination = link_destination(raw)?;
    Some(Link {
        line: line_number,
        destination: destination.to_owned(),
    })
}

fn fence_marker(line: &str) -> Option<char> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~') || trimmed.chars().take_while(|c| *c == marker).count() < 3 {
        return None;
    }
    Some(marker)
}

fn atx_heading(line: &str) -> Option<&str> {
    let trimmed = line
        .strip_prefix("   ")
        .unwrap_or_else(|| line.trim_start_matches(' '));
    if line.len() - trimmed.len() > 3 {
        return None;
    }
    let hashes = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    Some(rest.trim().trim_end_matches('#').trim_end())
}

fn inline_links(line: &str, line_number: usize) -> Vec<Link> {
    let bytes = line.as_bytes();
    let mut links = Vec::new();
    let mut index = 0;
    let mut code_delimiter = 0;

    while index < bytes.len() {
        if bytes[index] == b'`' {
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

        let Some(label_end) = find_unescaped(bytes, index + 1, b']') else {
            break;
        };
        if bytes.get(label_end + 1) != Some(&b'(') {
            index = label_end + 1;
            continue;
        }
        let Some(destination_end) = matching_parenthesis(bytes, label_end + 1) else {
            index = label_end + 1;
            continue;
        };
        let raw = line[label_end + 2..destination_end].trim();
        if let Some(destination) = link_destination(raw) {
            links.push(Link {
                line: line_number,
                destination: destination.to_owned(),
            });
        }
        index = destination_end + 1;
    }

    links
}

fn is_escaped(bytes: &[u8], index: usize) -> bool {
    let slashes = bytes[..index]
        .iter()
        .rev()
        .take_while(|byte| **byte == b'\\')
        .count();
    slashes % 2 == 1
}

fn find_unescaped(bytes: &[u8], start: usize, needle: u8) -> Option<usize> {
    (start..bytes.len()).find(|index| bytes[*index] == needle && !is_escaped(bytes, *index))
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

fn link_destination(raw: &str) -> Option<&str> {
    if let Some(rest) = raw.strip_prefix('<') {
        return rest.split_once('>').map(|(destination, _)| destination);
    }
    raw.split_whitespace()
        .next()
        .filter(|value| !value.is_empty())
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

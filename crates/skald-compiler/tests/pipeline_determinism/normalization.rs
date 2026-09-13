use std::path::Path;

/// Canonicalizes identities that deliberately depend on a temporary module fixture.
///
/// Module and provider permutations can change the temporary root, selected
/// path spelling, and global source offsets used by rendered spans. Semantic
/// names and all other output remain unchanged so the determinism comparison
/// still observes meaningful differences.
pub(crate) fn normalize_module_fixture_output(fixture: &Path, output: String) -> String {
    let path_normalized = output.replace(fixture.to_str().unwrap(), "<fixture>");
    path_normalized
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("display ") {
                format!(
                    "{}display <spelling>",
                    &line[..line.len() - line.trim_start().len()]
                )
            } else {
                normalize_spans(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn normalize_spans(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut output = String::with_capacity(line.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'@' {
            let start = index;
            index += 1;
            let first_digits = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > first_digits && bytes.get(index..index + 2) == Some(b"..") {
                index += 2;
                let second_digits = index;
                while index < bytes.len() && bytes[index].is_ascii_digit() {
                    index += 1;
                }
                if index > second_digits {
                    output.push_str("@<span>");
                    continue;
                }
            }
            output.push_str(&line[start..index]);
        } else {
            let character = line[index..].chars().next().unwrap();
            output.push(character);
            index += character.len_utf8();
        }
    }
    output
}

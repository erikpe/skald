//! Deliberately limited Rust source scanner for crate-root dependency paths.

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct RootReference {
    pub(super) root: String,
    pub(super) line: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Token {
    kind: TokenKind,
    offset: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TokenKind {
    Identifier(String),
    PathSeparator,
    OpenBrace,
    CloseBrace,
    Comma,
    Other,
}

pub(super) fn crate_root_references(source: &str) -> Vec<RootReference> {
    let tokens = tokenize(source);
    let mut references = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if !matches!(&token.kind, TokenKind::Identifier(identifier) if identifier == "crate")
            || !matches!(
                tokens.get(index + 1).map(|token| &token.kind),
                Some(TokenKind::PathSeparator)
            )
        {
            continue;
        }
        collect_root(&tokens, index + 2, source, &mut references);
    }
    references
}

pub(super) fn root_references(source: &str, module_depth: usize) -> Vec<RootReference> {
    let tokens = tokenize(source);
    let mut references = crate_root_references(source);
    for (start, token) in tokens.iter().enumerate() {
        if !matches!(&token.kind, TokenKind::Identifier(identifier) if identifier == "super")
            || matches!(
                start
                    .checked_sub(1)
                    .and_then(|index| tokens.get(index))
                    .map(|token| &token.kind),
                Some(TokenKind::PathSeparator)
            )
        {
            continue;
        }
        let mut index = start;
        let mut super_count = 0usize;
        while matches!(
            tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Identifier(identifier)) if identifier == "super"
        ) && matches!(
            tokens.get(index + 1).map(|token| &token.kind),
            Some(TokenKind::PathSeparator)
        ) {
            super_count += 1;
            index += 2;
        }
        if super_count >= module_depth {
            collect_root(&tokens, index, source, &mut references);
        }
    }
    references
}

fn collect_root(tokens: &[Token], index: usize, source: &str, references: &mut Vec<RootReference>) {
    let Some(token) = tokens.get(index) else {
        return;
    };
    match &token.kind {
        TokenKind::Identifier(root) if root != "self" => references.push(RootReference {
            root: root.clone(),
            line: line_number(source, token.offset),
        }),
        TokenKind::OpenBrace => collect_group_roots(tokens, index + 1, source, references),
        _ => {}
    }
}

fn collect_group_roots(
    tokens: &[Token],
    mut index: usize,
    source: &str,
    references: &mut Vec<RootReference>,
) {
    let mut depth = 1usize;
    let mut item_start = true;
    while let Some(token) = tokens.get(index) {
        match &token.kind {
            TokenKind::OpenBrace => depth += 1,
            TokenKind::CloseBrace => {
                depth -= 1;
                if depth == 0 {
                    return;
                }
            }
            TokenKind::Comma if depth == 1 => item_start = true,
            TokenKind::Identifier(root) if depth == 1 && item_start => {
                if root != "self" {
                    references.push(RootReference {
                        root: root.clone(),
                        line: line_number(source, token.offset),
                    });
                }
                item_start = false;
            }
            TokenKind::Other | TokenKind::PathSeparator if depth == 1 && item_start => {}
            _ => {}
        }
        index += 1;
    }
}

fn line_number(source: &str, offset: usize) -> usize {
    source.as_bytes()[..offset]
        .iter()
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            index += 1;
        } else if bytes[index..].starts_with(b"//") {
            index = skip_line_comment(bytes, index + 2);
        } else if bytes[index..].starts_with(b"/*") {
            index = skip_block_comment(bytes, index + 2);
        } else if let Some(end) = skip_raw_string(bytes, index) {
            index = end;
        } else if bytes[index] == b'"' {
            index = skip_quoted(bytes, index + 1, b'"');
        } else if bytes[index] == b'\'' {
            index = skip_character_or_lifetime(bytes, index);
        } else if bytes[index..].starts_with(b"::") {
            tokens.push(Token {
                kind: TokenKind::PathSeparator,
                offset: index,
            });
            index += 2;
        } else if bytes[index] == b'{' {
            tokens.push(Token {
                kind: TokenKind::OpenBrace,
                offset: index,
            });
            index += 1;
        } else if bytes[index] == b'}' {
            tokens.push(Token {
                kind: TokenKind::CloseBrace,
                offset: index,
            });
            index += 1;
        } else if bytes[index] == b',' {
            tokens.push(Token {
                kind: TokenKind::Comma,
                offset: index,
            });
            index += 1;
        } else if let Some(end) = identifier_end(bytes, index) {
            let raw_prefix = bytes[index..end].starts_with(b"r#");
            let start = index + usize::from(raw_prefix) * 2;
            tokens.push(Token {
                kind: TokenKind::Identifier(source[start..end].to_owned()),
                offset: index,
            });
            index = end;
        } else {
            tokens.push(Token {
                kind: TokenKind::Other,
                offset: index,
            });
            index += 1;
        }
    }
    tokens
}

fn identifier_end(bytes: &[u8], index: usize) -> Option<usize> {
    let mut end = index;
    if bytes[index..].starts_with(b"r#") {
        end += 2;
    }
    if !bytes.get(end).is_some_and(u8::is_ascii_alphabetic) && bytes.get(end) != Some(&b'_') {
        return None;
    }
    end += 1;
    while bytes
        .get(end)
        .is_some_and(|byte| byte.is_ascii_alphanumeric() || *byte == b'_')
    {
        end += 1;
    }
    Some(end)
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(|byte| *byte != b'\n') {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize) -> usize {
    let mut depth = 1usize;
    while index < bytes.len() && depth != 0 {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
        } else {
            index += 1;
        }
    }
    index
}

fn skip_raw_string(bytes: &[u8], index: usize) -> Option<usize> {
    let marker = if bytes[index..].starts_with(b"br") {
        index + 2
    } else if bytes[index] == b'r' {
        index + 1
    } else {
        return None;
    };
    let mut quote = marker;
    while bytes.get(quote) == Some(&b'#') {
        quote += 1;
    }
    if bytes.get(quote) != Some(&b'"') {
        return None;
    }
    let hashes = quote - marker;
    let mut end = quote + 1;
    while end < bytes.len() {
        if bytes[end] == b'"'
            && bytes
                .get(end + 1..end + 1 + hashes)
                .is_some_and(|suffix| suffix.iter().all(|byte| *byte == b'#'))
        {
            return Some(end + 1 + hashes);
        }
        end += 1;
    }
    Some(bytes.len())
}

fn skip_quoted(bytes: &[u8], mut index: usize, quote: u8) -> usize {
    while index < bytes.len() {
        if bytes[index] == b'\\' {
            index = (index + 2).min(bytes.len());
        } else if bytes[index] == quote {
            return index + 1;
        } else {
            index += 1;
        }
    }
    index
}

fn skip_character_or_lifetime(bytes: &[u8], index: usize) -> usize {
    if bytes.get(index + 1) == Some(&b'\\') {
        return skip_quoted(bytes, index + 2, b'\'');
    }
    let search_end = (index + 7).min(bytes.len());
    if let Some(relative) = bytes[index + 1..search_end]
        .iter()
        .position(|byte| *byte == b'\'')
    {
        return index + relative + 2;
    }
    index + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_direct_qualified_and_grouped_crate_roots() {
        let source = "use crate::{resolve::ResolvedProgram, typeck::{self, TypeCheckOutput}};\n\
                      fn lower(value: crate::hir::HirProgram) { crate :: mir::consume(value); }";
        assert_eq!(
            crate_root_references(source),
            [
                RootReference {
                    root: "resolve".to_owned(),
                    line: 1,
                },
                RootReference {
                    root: "typeck".to_owned(),
                    line: 1,
                },
                RootReference {
                    root: "hir".to_owned(),
                    line: 2,
                },
                RootReference {
                    root: "mir".to_owned(),
                    line: 2,
                },
            ]
        );
    }

    #[test]
    fn ignores_non_code_and_keeps_lifetimes() {
        let source = r####"
            // crate::driver::one
            /* crate::{backend::two, /* crate::passes::three */ typeck::four} */
            const A: &str = "crate::reporting::five";
            const B: &str = r###"crate::module::six"###;
            const C: u8 = b'c';
            fn allowed<'a>(_: &'a str) -> crate::resolve::ResolvedProgram { todo!() }
        "####;
        assert_eq!(
            crate_root_references(source),
            [RootReference {
                root: "resolve".to_owned(),
                line: 7,
            }]
        );
    }

    #[test]
    fn accepts_raw_identifiers_and_group_self_entries() {
        assert_eq!(
            crate_root_references("use crate::{self, r#typeck::Thing};"),
            [RootReference {
                root: "typeck".to_owned(),
                line: 1,
            }]
        );
    }

    #[test]
    fn resolves_relative_paths_that_leave_the_owning_phase_root() {
        let source = "use super::local;\n\
                      use super::super::{typeck::TypeCheckOutput, identity::ClassId};";
        assert_eq!(
            root_references(source, 2),
            [
                RootReference {
                    root: "typeck".to_owned(),
                    line: 2,
                },
                RootReference {
                    root: "identity".to_owned(),
                    line: 2,
                },
            ]
        );
    }
}

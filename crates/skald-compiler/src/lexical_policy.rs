//! Source-level lexical predicates shared across compiler phases.
//!
//! This module contains language policy only. It deliberately owns no scanner
//! state, tokens, diagnostics, or MIR concepts.

pub(crate) const fn is_identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

pub(crate) const fn is_identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

pub(crate) fn is_source_identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters.next().is_some_and(is_identifier_start) && characters.all(is_identifier_continue)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_identifiers_use_one_ascii_policy() {
        for valid in ["a", "Z", "_", "_name", "name_2"] {
            assert!(is_source_identifier(valid), "rejected `{valid}`");
        }
        for invalid in ["", "2name", "na-me", "näme", "name!"] {
            assert!(!is_source_identifier(invalid), "accepted `{invalid}`");
        }
    }
}

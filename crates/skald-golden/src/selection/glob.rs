pub(super) fn matches(pattern: &str, value: &str) -> bool {
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let mut memo = vec![vec![None; value.len() + 1]; pattern.len() + 1];
    matches_from(pattern, value, 0, 0, &mut memo)
}

fn matches_from(
    pattern: &[u8],
    value: &[u8],
    pattern_index: usize,
    value_index: usize,
    memo: &mut [Vec<Option<bool>>],
) -> bool {
    if let Some(result) = memo[pattern_index][value_index] {
        return result;
    }
    let result = if pattern_index == pattern.len() {
        value_index == value.len()
    } else if pattern[pattern_index..].starts_with(b"**") {
        matches_from(pattern, value, pattern_index + 2, value_index, memo)
            || (value_index < value.len()
                && matches_from(pattern, value, pattern_index, value_index + 1, memo))
    } else if pattern[pattern_index] == b'*' {
        matches_from(pattern, value, pattern_index + 1, value_index, memo)
            || (value_index < value.len()
                && !is_separator(value[value_index])
                && matches_from(pattern, value, pattern_index, value_index + 1, memo))
    } else {
        value_index < value.len()
            && pattern[pattern_index] == value[value_index]
            && matches_from(pattern, value, pattern_index + 1, value_index + 1, memo)
    };
    memo[pattern_index][value_index] = Some(result);
    result
}

fn is_separator(byte: u8) -> bool {
    matches!(byte, b'/' | b':')
}

#[cfg(test)]
mod tests {
    use super::matches;

    #[test]
    fn literal_and_component_globs_match_whole_values() {
        assert!(matches("language/arrays", "language/arrays"));
        assert!(matches("language/*", "language/arrays"));
        assert!(!matches("language/*", "language/arrays/basic"));
        assert!(!matches("language/*", "language/arrays::basic"));
        assert!(!matches("arrays", "language/arrays"));
    }

    #[test]
    fn recursive_globs_cross_path_and_identity_components() {
        assert!(matches("language/**", "language/arrays/basic"));
        assert!(matches(
            "language/**",
            "language/arrays::basic::default::run"
        ));
        assert!(matches("**::optimized::*", "a/b::test::optimized::run"));
        assert!(!matches("**::optimized::*", "a/b::test::default::run"));
    }

    #[test]
    fn stars_may_match_empty_components() {
        assert!(matches("a*", "a"));
        assert!(matches("a**b", "ab"));
        assert!(matches("**", ""));
    }
}

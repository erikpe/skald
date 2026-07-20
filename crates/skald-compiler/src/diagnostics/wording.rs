//! Small, deterministic helpers for wording shared by diagnostics.

pub(crate) fn format_type_list(type_names: &[&str]) -> String {
    assert!(
        !type_names.is_empty(),
        "a diagnostic type list must not be empty"
    );

    let quoted: Vec<_> = type_names.iter().map(|name| format!("`{name}`")).collect();
    match quoted.as_slice() {
        [only] => only.clone(),
        [left, right] => format!("{left} or {right}"),
        [head @ .., last] => format!("{}, or {last}", head.join(", ")),
        [] => unreachable!("empty type lists are rejected above"),
    }
}

#[cfg(test)]
mod tests {
    use super::format_type_list;

    #[test]
    fn formats_supported_types_consistently() {
        assert_eq!(format_type_list(&["i64"]), "`i64`");
        assert_eq!(format_type_list(&["i64", "f64"]), "`i64` or `f64`");
        assert_eq!(
            format_type_list(&["i64", "u64", "u8", "f64"]),
            "`i64`, `u64`, `u8`, or `f64`"
        );
    }
}

//! Canonical standard-library source closure shared by compiler tests.

pub const CANONICAL_STR_SOURCE: &str = include_str!("../../../std/std/str.ska");
pub const CANONICAL_STR_FORMAT_INTEGER_SOURCE: &str =
    include_str!("../../../std/std/str/format_integer.ska");
pub const CANONICAL_STR_FORMAT_F64_SOURCE: &str =
    include_str!("../../../std/std/str/format_f64.ska");
pub const CANONICAL_STR_PARSE_INTEGER_SOURCE: &str =
    include_str!("../../../std/std/str/parse_integer.ska");
pub const CANONICAL_STR_PARSE_F64_SOURCE: &str = include_str!("../../../std/std/str/parse_f64.ska");
pub const CANONICAL_ERROR_SOURCE: &str = include_str!("../../../std/std/error.ska");
pub const CANONICAL_F64_SOURCE: &str = include_str!("../../../std/std/f64.ska");
pub const CANONICAL_HASH_SOURCE: &str = include_str!("../../../std/std/hash.ska");
pub const CANONICAL_IO_SOURCE: &str = include_str!("../../../std/std/io.ska");
pub const CANONICAL_LANG_SOURCE: &str = include_str!("../../../std/std/lang.ska");
pub const CANONICAL_PROCESS_SOURCE: &str = include_str!("../../../std/std/process.ska");
pub const CANONICAL_TEST_SOURCE: &str = include_str!("../../../std/std/test.ska");
pub const CANONICAL_VEC_SOURCE: &str = include_str!("../../../std/std/vec.ska");

const CANONICAL_SOURCES: [(&str, &str); 13] = [
    ("std/str.ska", CANONICAL_STR_SOURCE),
    (
        "std/str/format_integer.ska",
        CANONICAL_STR_FORMAT_INTEGER_SOURCE,
    ),
    ("std/str/format_f64.ska", CANONICAL_STR_FORMAT_F64_SOURCE),
    (
        "std/str/parse_integer.ska",
        CANONICAL_STR_PARSE_INTEGER_SOURCE,
    ),
    ("std/str/parse_f64.ska", CANONICAL_STR_PARSE_F64_SOURCE),
    ("std/error.ska", CANONICAL_ERROR_SOURCE),
    ("std/f64.ska", CANONICAL_F64_SOURCE),
    ("std/hash.ska", CANONICAL_HASH_SOURCE),
    ("std/io.ska", CANONICAL_IO_SOURCE),
    ("std/lang.ska", CANONICAL_LANG_SOURCE),
    ("std/process.ska", CANONICAL_PROCESS_SOURCE),
    ("std/test.ska", CANONICAL_TEST_SOURCE),
    ("std/vec.ska", CANONICAL_VEC_SOURCE),
];

/// Returns every canonical standard-library module in stable fixture order.
///
/// Overrides must name an existing canonical module exactly once. Callers may
/// reorder the returned vector when source-creation order is part of a test.
pub fn canonical_standard_library_sources<'a>(
    overrides: &[(&str, &'a str)],
) -> Vec<(&'static str, &'a str)> {
    for (index, (path, _)) in overrides.iter().enumerate() {
        assert!(
            CANONICAL_SOURCES
                .iter()
                .any(|(canonical_path, _)| canonical_path == path),
            "standard-library override `{path}` is not canonical"
        );
        assert!(
            !overrides[..index]
                .iter()
                .any(|(previous, _)| previous == path),
            "standard-library module `{path}` is overridden more than once"
        );
    }

    CANONICAL_SOURCES
        .iter()
        .map(|(path, canonical)| {
            let source = overrides
                .iter()
                .find_map(|(override_path, source)| (override_path == path).then_some(*source))
                .unwrap_or(*canonical);
            (*path, source)
        })
        .collect()
}

//! Portable compiler-diagnostic bytes derived from raw process output.

/// Stderr bytes used for compile-fail comparison and reporting.
///
/// `Raw` borrows the owning process observation when path normalization is an
/// identity operation. `Normalized` owns the derived bytes without replacing
/// the captured child output.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum CompilerStderrView {
    #[default]
    Raw,
    Normalized(Vec<u8>),
}

impl CompilerStderrView {
    pub(super) fn from_raw(raw: &[u8], diagnostic_path_prefix: Option<&[u8]>) -> Self {
        let Some(prefix) = diagnostic_path_prefix.filter(|prefix| !prefix.is_empty()) else {
            return Self::Raw;
        };
        let Some(first) = find_bytes(raw, prefix) else {
            return Self::Raw;
        };

        let mut normalized = Vec::with_capacity(raw.len() - prefix.len());
        normalized.extend_from_slice(&raw[..first]);
        let mut remaining = &raw[first + prefix.len()..];
        while let Some(index) = find_bytes(remaining, prefix) {
            normalized.extend_from_slice(&remaining[..index]);
            remaining = &remaining[index + prefix.len()..];
        }
        normalized.extend_from_slice(remaining);
        Self::Normalized(normalized)
    }

    pub(super) fn bytes<'a>(&'a self, raw: &'a [u8]) -> &'a [u8] {
        match self {
            Self::Raw => raw,
            Self::Normalized(normalized) => normalized,
        }
    }
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

#[cfg(test)]
mod tests;

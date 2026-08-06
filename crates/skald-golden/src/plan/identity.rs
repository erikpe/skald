use super::PlanError;
use std::path::Path;

pub(super) fn spec_id(relative_path: &Path) -> Result<String, PlanError> {
    let relative = slash_path(relative_path)?;
    relative
        .strip_suffix(".golden.toml")
        .map(str::to_owned)
        .ok_or_else(|| PlanError::at_path(relative_path, "spec path lacks .golden.toml suffix"))
}

pub(super) fn slash_path(path: &Path) -> Result<String, PlanError> {
    let mut components = Vec::new();
    for component in path.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(PlanError::at_path(
                path,
                "stable relative paths may contain only normal components",
            ));
        };
        let component = component.to_str().ok_or_else(|| {
            PlanError::at_path(path, "stable identities require UTF-8 path components")
        })?;
        components.push(component);
    }
    Ok(components.join("/"))
}

pub(super) fn artifact_name(build_id: &str) -> String {
    let mut prefix = String::with_capacity(48);
    let mut previous_separator = false;
    for character in build_id.chars() {
        let character = if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
            previous_separator = false;
            character
        } else if previous_separator {
            continue;
        } else {
            previous_separator = true;
            '_'
        };
        if prefix.len() + character.len_utf8() > 48 {
            break;
        }
        prefix.push(character);
    }
    let prefix = prefix.trim_matches('_');
    let prefix = if prefix.is_empty() { "case" } else { prefix };
    format!("{prefix}-{:016x}", stable_hash(build_id.as_bytes()))
}

// FNV-1a is deliberately implemented here instead of using DefaultHasher,
// whose output is not a stable persistence contract.
fn stable_hash(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{artifact_name, stable_hash};

    #[test]
    fn pins_the_stable_hash_algorithm() {
        assert_eq!(stable_hash(b""), 0xcbf29ce484222325);
        assert_eq!(stable_hash(b"skald"), 0xe968394216993ad6);
    }

    #[test]
    fn flattened_prefix_collisions_retain_distinct_hashes() {
        let left = artifact_name("language/a::b_c::default");
        let right = artifact_name("language/a_b::c::default");
        assert_ne!(left, right);
        assert!(left.starts_with("language_a_b_c_default-"));
        assert!(right.starts_with("language_a_b_c_default-"));
    }
}

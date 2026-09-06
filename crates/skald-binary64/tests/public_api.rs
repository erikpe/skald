use std::{fs, path::Path};

use skald_binary64::{Binary64, Binary64Comparison};

#[test]
fn facade_is_usable_without_exposing_the_apfloat_implementation() {
    let value = Binary64::from_bits(0xbff0_0000_0000_0000);
    let copied = value;

    assert_eq!(copied.to_bits(), 0xbff0_0000_0000_0000);
    assert!(copied.is_finite());
    assert!(copied.is_negative());
    assert!(copied.same_bits(value));

    let sum = copied.add(Binary64::from_bits(0x4000_0000_0000_0000));
    assert_eq!(sum.to_bits(), 0x3ff0_0000_0000_0000);
    assert_eq!(
        sum.compare(Binary64::from_bits(0x4000_0000_0000_0000)),
        Binary64Comparison::Less
    );
}

#[test]
fn rustc_apfloat_is_confined_to_this_crate() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let crates_dir = crate_dir
        .parent()
        .expect("crate has a workspace crates directory");

    for entry in fs::read_dir(crates_dir).expect("workspace crates directory is readable") {
        let entry = entry.expect("workspace crate entry is readable");
        let path = entry.path();
        if path == crate_dir || !path.is_dir() {
            continue;
        }

        assert_crate_tree_does_not_name_apfloat(&path);
    }
}

fn assert_crate_tree_does_not_name_apfloat(directory: &Path) {
    for entry in fs::read_dir(directory).expect("source directory is readable") {
        let entry = entry.expect("source entry is readable");
        let path = entry.path();
        if path.is_dir() {
            assert_crate_tree_does_not_name_apfloat(&path);
        } else if path.extension().is_some_and(|extension| extension == "rs")
            || path.file_name().is_some_and(|name| name == "Cargo.toml")
        {
            assert_file_does_not_name_apfloat(&path);
        }
    }
}

fn assert_file_does_not_name_apfloat(path: &Path) {
    let source = fs::read_to_string(path).expect("source file is readable as UTF-8");
    assert!(
        !source.contains("rustc_apfloat"),
        "{} bypasses the skald-binary64 dependency boundary",
        path.display()
    );
}

use super::*;

#[test]
fn selects_every_supported_arithmetic_operation_and_storage_copy() {
    let output = assembly(concat!(
        "fn helper(a: i64) -> i64 { return -a; }\n",
        "fn main() -> i64 { ",
        "var x: i64 = 9; return helper(x * 3 - 4 + 2); }",
    ));

    assert!(output.contains("neg rax"));
    assert!(output.contains("imul rax, rcx"));
    assert!(output.contains("sub rax, rcx"));
    assert!(output.contains("add rax, rcx"));
    assert!(output.contains("call .Lska_fn_0"));
    assert!(output.contains("mov qword ptr [rbp - 8], rax"));
}

#[test]
fn selects_every_integer_comparison_with_exact_signedness_and_canonical_results() {
    let mut source = String::new();
    for type_name in ["i64", "u64", "u8"] {
        for (name, spelling) in [
            ("eq", "=="),
            ("ne", "!="),
            ("lt", "<"),
            ("le", "<="),
            ("gt", ">"),
            ("ge", ">="),
        ] {
            source.push_str(&format!(
                "fn {name}_{type_name}(left: {type_name}, right: {type_name}) -> bool {{ \
                 return left {spelling} right; }}\n"
            ));
        }
    }
    source.push_str("fn main() -> i64 { return 0; }\n");

    let output = assembly(&source);
    assert_eq!(output, assembly(&source));

    for (mnemonic, expected_count) in [
        ("sete al", 3),
        ("setne al", 3),
        ("setl al", 1),
        ("setle al", 1),
        ("setg al", 1),
        ("setge al", 1),
        ("setb al", 2),
        ("setbe al", 2),
        ("seta al", 2),
        ("setae al", 2),
    ] {
        assert_eq!(
            output
                .lines()
                .filter(|line| line.trim() == mnemonic)
                .count(),
            expected_count,
            "unexpected selection count for `{mnemonic}`"
        );
    }

    let lines: Vec<_> = output.lines().map(str::trim).collect();
    for index in lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.starts_with("set").then_some(index))
    {
        assert_eq!(lines[index + 1], "movzx rax, al");
        assert!(lines[index + 2].starts_with("mov qword ptr [rbp"));
        assert!(lines[index + 2].ends_with(", rax"));
    }
    assert_eq!(
        lines.iter().filter(|line| **line == "cmp rax, rcx").count(),
        18
    );
    assert!(output.contains("call ska_rt_abi_v5"));
    assert!(!output.contains("ska_rt_compare"));
}

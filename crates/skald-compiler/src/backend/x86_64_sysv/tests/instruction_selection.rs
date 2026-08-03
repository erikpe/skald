use super::*;

#[test]
fn storage_lifetime_markers_emit_no_machine_instructions() {
    let mut without_markers = lower_source_to_mir("fn main() -> i64 { return 7; }");
    let function = without_markers
        .definitions
        .get_mut_for_test(without_markers.entry_function)
        .unwrap();
    let span = function.span;
    let unused = StorageId::new(function.function, function.storage.len());
    function.storage.push(MirStorage {
        id: unused,
        source: Some(BindingId::Local(LocalId::new(function.function, 0))),
        name: "unused".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::I64,
        span,
    });
    verify_mir(&without_markers).expect("unused dead storage is valid");

    let mut with_markers = without_markers.clone();
    let function = with_markers
        .definitions
        .get_mut_for_test(with_markers.entry_function)
        .unwrap();
    let block = &mut function.body.blocks[0];
    block
        .instructions
        .insert(0, fixture_storage_live(unused, span));
    block.instructions.push(fixture_storage_dead(unused, span));
    verify_mir(&with_markers).expect("balanced unused lifetime markers are valid");

    assert_eq!(
        emit_assembly(Target::X86_64SysV, &without_markers).unwrap(),
        emit_assembly(Target::X86_64SysV, &with_markers).unwrap()
    );
}

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
    assert!(output.contains("call .Lska.fn.main.helper.f0"));
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
    assert!(output.contains("call ska_rt_abi_v8"));
    assert!(!output.contains("ska_rt_compare"));
}

#[test]
fn selects_eager_boolean_operations_with_canonical_results() {
    let output = emit_assembly(Target::X86_64SysV, &eager_boolean_program()).unwrap();
    assert_eq!(
        output,
        emit_assembly(Target::X86_64SysV, &eager_boolean_program()).unwrap()
    );

    let lines: Vec<_> = output.lines().map(str::trim).collect();
    assert_eq!(
        lines
            .iter()
            .filter(|line| **line == "test rax, rax")
            .count(),
        8
    );
    assert_eq!(
        lines.iter().filter(|line| **line == "cmp rax, rcx").count(),
        4
    );
    assert_eq!(lines.iter().filter(|line| **line == "sete al").count(), 4);
    assert_eq!(lines.iter().filter(|line| **line == "setne al").count(), 2);

    for index in lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| line.starts_with("set").then_some(index))
    {
        assert_eq!(lines[index + 1], "movzx rax, al");
        assert!(lines[index + 2].starts_with("mov qword ptr [rbp"));
        assert!(lines[index + 2].ends_with(", rax"));
    }
    assert!(!output.contains("ska_rt_boolean"));
    assert!(!output.contains("ska_rt_compare"));
}

#[test]
fn selects_every_integer_bitwise_operation_and_canonicalizes_u8_results() {
    let output = emit_assembly(Target::X86_64SysV, &integer_bitwise_program()).unwrap();
    assert_eq!(
        output,
        emit_assembly(Target::X86_64SysV, &integer_bitwise_program()).unwrap()
    );

    let function = function_assembly(&output, ".Lska.fn.main.main.f0");
    let lines: Vec<_> = function.lines().map(str::trim).collect();
    for mnemonic in ["not rax", "and rax, rcx", "or rax, rcx", "xor rax, rcx"] {
        assert_eq!(
            lines.iter().filter(|line| **line == mnemonic).count(),
            3,
            "unexpected selection count for `{mnemonic}`"
        );
    }
    let canonicalized_operations = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| {
            matches!(
                **line,
                "not rax" | "and rax, rcx" | "or rax, rcx" | "xor rax, rcx"
            )
        })
        .filter(|(index, _)| lines.get(index + 1) == Some(&"movzx rax, al"))
        .count();
    assert_eq!(canonicalized_operations, 4);
    assert!(!function.contains("call "));
    assert!(!output.contains("ska_rt_bitwise"));
}

#[test]
fn selects_every_integer_cast_through_canonical_scalar_moves() {
    let mut source = String::new();
    let mut functions = Vec::new();
    for source_type in ["i64", "u64", "u8"] {
        for target_type in ["i64", "u64", "u8"] {
            functions.push((source_type, target_type));
            source.push_str(&format!(
                "fn cast_{source_type}_to_{target_type}(value: {source_type}) -> {target_type} {{ \
                 return ({target_type}) value; }}\n"
            ));
        }
    }
    source.push_str("fn main() -> i64 { return 0; }\n");

    let output = assembly(&source);
    assert_eq!(output, assembly(&source));
    for (index, (source_type, target_type)) in functions.into_iter().enumerate() {
        let function = function_assembly(
            &output,
            &format!(".Lska.fn.main.cast_{source_type}_to_{target_type}.f{index}"),
        );
        assert!(function.contains("mov rax, qword ptr [rbp"));
        assert!(!function.contains("ud2"));
        assert!(!function.contains("cmp "));
        assert!(!function.contains("set"));
        assert!(!function.contains("call "));

        if target_type == "u8" {
            assert!(
                function.contains("movzx rax, al"),
                "{source_type}-to-{target_type} must retain and canonicalize the low byte"
            );
        }
        if source_type != "u8" && target_type != "u8" {
            assert!(
                !function.contains("movzx"),
                "{source_type}-to-{target_type} must preserve all 64 bits"
            );
        }
        if source_type == "u8" && target_type != "u8" {
            assert!(
                function.contains("movzx rax, al"),
                "{source_type}-to-{target_type} must consume a canonical zero-extended source"
            );
        }
    }
}

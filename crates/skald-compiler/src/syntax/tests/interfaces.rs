use super::*;

#[test]
fn parses_interface_requirements_and_ordered_class_claims() {
    let (_, output) = parse_text(
        "interface Readable { fn read(offset: u64) -> u8; }\n\
         interface Writable { mut fn write(value: u8) -> unit; }\n\
         class Buffer implements Readable, Writable {}",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let TopLevelDeclaration::Interface(readable) = &output.ast.declarations[0] else {
        panic!("expected interface");
    };
    assert_eq!(readable.requirements[0].name.text, "read");
    let TopLevelDeclaration::Class(buffer) = &output.ast.declarations[2] else {
        panic!("expected class");
    };
    assert_eq!(
        buffer
            .implemented_interfaces
            .iter()
            .map(|name| name.text.as_str())
            .collect::<Vec<_>>(),
        ["Readable", "Writable"]
    );
}

#[test]
fn recovers_after_an_invalid_interface_member() {
    let (_, output) =
        parse_text("interface Broken { value: u64; fn ok() -> unit; }\nfn main() -> unit {}");
    assert!(output.has_errors());
    assert!(matches!(
        output.ast.declarations.last(),
        Some(TopLevelDeclaration::Function(_))
    ));
}

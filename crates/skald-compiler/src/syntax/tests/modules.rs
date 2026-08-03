use super::*;

#[test]
fn parses_imports_aliases_visibility_and_qualified_declaration_uses() {
    let (_, output) = parse_text(
        "import std::Str;\n\
         import app::text as Text;\n\
         from std::Str import Str, StrBuf as Buffer;\n\
         public extern fn make(value: std::Str::Str) -> unit;\n\
         public class Child extends app::Base implements api::Display {}\n\
         public interface View {}\n\
         public fn main() -> unit {\n\
           var value: std::Str::Str = std::Str::Str();\n\
           var owned: shared std::Str::Str = new std::Str::Str();\n\
           value is api::View;\n\
           (api::View) value;\n\
           return;\n\
         }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.ast.imports.len(), 3);
    let ImportDeclaration::Module(first) = &output.ast.imports[0] else {
        panic!("expected a module import");
    };
    assert_eq!(first.module.text, "std::Str");
    assert_eq!(first.module.components().count(), 2);
    assert_eq!(first.module.separator_spans().len(), 1);
    let ImportDeclaration::Selective(selective) = &output.ast.imports[2] else {
        panic!("expected a selective import");
    };
    assert_eq!(selective.items.len(), 2);
    assert_eq!(selective.comma_spans.len(), 1);
    assert_eq!(
        selective.items[1]
            .alias
            .as_ref()
            .map(|alias| alias.text.as_str()),
        Some("Buffer")
    );
    assert!(output
        .ast
        .declarations
        .iter()
        .all(|declaration| match declaration.visibility() {
            Visibility::Public { span } => {
                declaration.span().range().start() == span.range().start()
            }
            Visibility::Private => false,
        }));

    let TopLevelDeclaration::Class(class) = &output.ast.declarations[1] else {
        panic!("expected the class");
    };
    assert_eq!(class.direct_base.as_ref().unwrap().text, "app::Base");
    assert_eq!(class.implemented_interfaces[0].text, "api::Display");

    let dump = dump_ast(&output.ast);
    assert!(dump.contains("ModuleImport"));
    assert!(dump.contains("SelectiveImport"));
    assert!(dump.contains("Component \"std\""));
    assert!(dump.contains("Separator"));
    assert!(dump.contains("Public"));
}

#[test]
fn module_words_remain_contextual_identifiers() {
    let (_, output) = parse_text(
        "class as {}\n\
         fn import(from: i64) -> i64 {\n\
           var public: i64 = from;\n\
           return public;\n\
         }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.ast.declarations[0].name().text, "as");
    assert_eq!(output.ast.declarations[1].name().text, "import");
}

#[test]
fn primitive_type_spellings_can_name_module_namespaces() {
    let (_, output) = parse_text(
        "import std::f64;\n\
         fn main() -> i64 {\n\
           std::f64::to_bits(1.0);\n\
           std::f64::from_bits(0u);\n\
           return 0;\n\
         }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let ImportDeclaration::Module(import) = &output.ast.imports[0] else {
        panic!("expected a module import");
    };
    assert_eq!(import.module.text, "std::f64");
    let dump = dump_ast(&output.ast);
    assert!(dump.contains("Component \"f64\""));
    assert!(dump.contains("Identifier \"std::f64::to_bits\""));
    assert!(dump.contains("Identifier \"std::f64::from_bits\""));
}

#[test]
fn rejects_invalid_import_forms_and_recovers_to_later_declarations() {
    for source in [
        "from std::Str import *; fn later() -> unit {}",
        "import .std::Str; fn later() -> unit {}",
        "import std::::Str; fn later() -> unit {}",
        "import std::Str as alias::nested; fn later() -> unit {}",
        "from std::Str import Str,; fn later() -> unit {}",
    ] {
        let (_, output) = parse_text(source);
        assert!(output.has_errors(), "source unexpectedly parsed: {source}");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, INVALID_IMPORT | EXPECTED_TOKEN)));
        assert_eq!(
            output
                .ast
                .declarations
                .last()
                .map(TopLevelDeclaration::name)
                .map(|name| name.text.as_str()),
            Some("later"),
            "failed to recover for {source}"
        );
    }
}

#[test]
fn rejects_imports_after_declarations_but_keeps_parsing() {
    let (_, output) = parse_text(
        "fn first() -> unit {}\n\
         import std::Str;\n\
         fn later() -> unit {}\n",
    );

    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == MISPLACED_IMPORT));
    assert!(output.ast.imports.is_empty());
    assert_eq!(output.ast.declarations.len(), 2);
}

#[test]
fn malformed_qualified_uses_recover_to_later_declarations() {
    let (_, output) = parse_text(
        "extern fn broken(value: std::) -> unit;\n\
         fn later() -> unit {}\n",
    );

    assert!(output.has_errors());
    assert_eq!(
        output
            .ast
            .declarations
            .last()
            .map(TopLevelDeclaration::name)
            .map(|name| name.text.as_str()),
        Some("later")
    );
}

#[test]
fn very_long_qualified_paths_are_parsed_iteratively() {
    let path = std::iter::repeat_n("component", 4_096)
        .collect::<Vec<_>>()
        .join("::");
    let source = format!("import {path};\nfn later() -> unit {{}}\n");
    let (_, output) = parse_text(&source);

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let ImportDeclaration::Module(import) = &output.ast.imports[0] else {
        panic!("expected a module import");
    };
    assert_eq!(import.module.components().count(), 4_096);
}

use super::*;
use crate::{
    hir::{dump_hir, HirObjectProducer},
    identity::LiteralDataId,
    test_support::load_module_sources,
    typeck::{type_check, PRIVATE_INITIALIZER_ACCESS},
};

const VALID_STR: &str = concat!(
    "public class Str {\n",
    "  private _storage: shared u8[];\n",
    "  private _start: i64;\n",
    "  private _length: u64;\n",
    "  init() {\n",
    "    self._storage = new u8[]();\n",
    "    self._start = 0;\n",
    "    self._length = 0u;\n",
    "  }\n",
    "}\n",
);
const CANONICAL_STR: &str = include_str!("../../../../../std/std/str.ska");

fn resolve_modules(entry: &str, sources: &[(&str, &str)]) -> ResolveOutput {
    let (_workspace, graph) = load_module_sources(entry, sources);
    resolve_module_graph(&graph)
}

fn source_with_str(app: &str) -> ResolveOutput {
    resolve_modules("app", &[("app.ska", app), ("std/str.ska", VALID_STR)])
}

#[test]
fn canonical_standard_library_surface_resolves_and_type_checks_as_ordinary_members() {
    let resolved = resolve_modules(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from std::str import Str;\n",
                    "fn main() -> i64 {\n",
                    "  var bytes: u8[] = u8[](2u);\n",
                    "  var value: Str = Str.from_bytes(bytes);\n",
                    "  var part: Str = value.slice(0, 2);\n",
                    "  var copy: u8[] = part.to_bytes();\n",
                    "  var combined: Str = value.concat(part);\n",
                    "  return (i64) combined.len();\n",
                    "}\n",
                ),
            ),
            ("std/str.ska", CANONICAL_STR),
        ],
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "canonical library must resolve: {:?}",
        resolved.diagnostics
    );
    let class = resolved
        .program
        .classes
        .iter()
        .find(|class| class.name == "Str")
        .expect("canonical library must declare Str");
    assert_eq!(class.initializers.len(), 2);
    assert_eq!(
        class.initializers[0].visibility,
        ResolvedMemberVisibility::Public
    );
    assert!(matches!(
        class.initializers[1].visibility,
        ResolvedMemberVisibility::Private { .. }
    ));
    assert!(class.initializers[0].parameters.is_empty());
    assert_eq!(class.initializers[1].parameters.len(), 3);
    assert!(class.fields.iter().all(|field| field.name.starts_with('_')));
    assert_eq!(class.fields[1].type_syntax.kind, ResolvedTypeKind::I64);
    assert_eq!(class.fields[2].type_syntax.kind, ResolvedTypeKind::U64);
    assert!(class
        .methods
        .iter()
        .filter(|method| method.visibility.private_span().is_some())
        .all(|method| method.name.starts_with('_')));
    let byte = class
        .methods
        .iter()
        .find(|method| method.name == "byte")
        .expect("canonical library must expose byte access");
    assert_eq!(byte.parameters.len(), 1);
    assert_eq!(byte.parameters[0].type_syntax.kind, ResolvedTypeKind::I64);
    let slice = class
        .methods
        .iter()
        .find(|method| method.name == "slice")
        .expect("canonical library must expose slicing");
    assert_eq!(
        slice
            .parameters
            .iter()
            .map(|parameter| parameter.type_syntax.kind)
            .collect::<Vec<_>>(),
        [ResolvedTypeKind::I64, ResolvedTypeKind::I64]
    );
    assert!(!class
        .methods
        .iter()
        .any(|method| method.name == "_from_fresh_storage"));
    assert!(!class
        .methods
        .iter()
        .any(|method| method.name == "_slice_trusted"));
    assert!(!class
        .methods
        .iter()
        .any(|method| method.name == "_normalize_position"));
    let descriptor_initializer = class.initializers[1].id;

    let checked = type_check(&resolved.program);
    assert!(
        checked.diagnostics.is_empty(),
        "canonical library API must type-check: {:?}",
        checked.diagnostics
    );
    let dump = dump_hir(checked.hir.as_ref().unwrap());
    assert_eq!(
        dump.matches(&format!(
            "Construct {} via {descriptor_initializer}",
            class.id
        ))
        .count(),
        3,
        "{dump}"
    );
}

#[test]
fn literal_materialization_does_not_select_private_initializers_or_method_names() {
    let renamed = concat!(
        "public class Str {\n",
        "  private _storage: shared u8[];\n",
        "  private _start: i64;\n",
        "  private _length: u64;\n",
        "  private init() { self._storage = new u8[](); self._start = 0; self._length = 0u; }\n",
        "  fn renamed_observer() -> u64 { return self._length; }\n",
        "  private static fn _renamed_helper(ref source: Str) -> Str { return Str(copy source); }\n",
        "}\n",
    );
    let resolved = resolve_modules(
        "app",
        &[
            (
                "app.ska",
                "from std::str import Str;\nfn main() -> i64 { var value: Str = \"names are ordinary\"; return 0; }\n",
            ),
            ("std/str.ska", renamed),
        ],
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "renamed ordinary methods must not affect the language item: {:?}",
        resolved.diagnostics
    );
    assert!(type_check(&resolved.program).diagnostics.is_empty());
}

#[test]
fn canonical_descriptor_initializer_is_not_source_accessible() {
    let resolved = resolve_modules(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "from std::str import Str;\n",
                    "fn main() -> i64 {\n",
                    "  var empty: Str = Str();\n",
                    "  var storage: shared u8[] = new u8[]();\n",
                    "  var forbidden: Str = Str(storage, 0, 0u);\n",
                    "  return 0;\n",
                    "}\n",
                ),
            ),
            ("std/str.ska", CANONICAL_STR),
        ],
    );
    assert!(
        resolved.diagnostics.is_empty(),
        "canonical library must resolve: {:?}",
        resolved.diagnostics
    );

    let checked = type_check(&resolved.program);
    assert_eq!(
        checked
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == PRIVATE_INITIALIZER_ACCESS)
            .count(),
        1
    );
    assert!(checked.hir.is_none());
}

#[test]
fn validates_the_exact_language_item_and_allocates_source_ordered_literal_data() {
    let output = source_with_str(concat!(
        "import std::str;\n",
        "fn first() -> unit { var first: std::str::Str = \"a\\0\"; }\n",
        "fn main() -> i64 {\n",
        "  var second: std::str::Str = \"\\x62\";\n",
        "  var third: std::str::Str = \"c\";\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(
        output.diagnostics.is_empty(),
        "valid language item must resolve: {:?}",
        output.diagnostics
    );

    let program = output.program;
    let item = program
        .string_language_item
        .as_ref()
        .expect("literal use must select the language item");
    let class = program.class(item.class).unwrap();
    assert_eq!(class.name, "Str");
    assert_eq!(
        [item.storage_field, item.start_field, item.length_field],
        [class.fields[0].id, class.fields[1].id, class.fields[2].id]
    );
    assert_eq!(item.requiring_literal_spans.len(), 3);
    assert_eq!(
        program
            .literal_data
            .iter()
            .map(|literal| (literal.id, literal.bytes.as_slice()))
            .collect::<Vec<_>>(),
        [
            (LiteralDataId::new(0), b"a\0".as_slice()),
            (LiteralDataId::new(1), b"b".as_slice()),
            (LiteralDataId::new(2), b"c".as_slice()),
        ]
    );

    let dump = dump_resolved(&program);
    assert!(dump.contains("StringLanguageItem"));
    assert!(dump.contains("StringLanguageItem class c0 fields c0:field0 c0:field1 c0:field2"));
    assert!(dump.contains("str0 bytes=6100"));
    assert!(dump.contains("str1 bytes=62"));
    assert!(dump.contains("StringLiteral str2 class c0"));
    assert_eq!(dump, dump_resolved(&program));
}

#[test]
fn rejects_every_structural_language_item_mismatch_before_hir() {
    let cases = [
        ("missing class", "public class Other { init() {} }\n"),
        ("wrong kind", "public fn Str() -> unit {}\n"),
        (
            "private class",
            "class Str { private _storage: shared u8[]; private _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "base class",
            "class Base { init() {} }\npublic class Str extends Base { private _storage: shared u8[]; private _start: i64; private _length: u64; init() { super(); self._storage = new u8[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "missing field",
            "public class Str { private _storage: shared u8[]; private _start: i64; init() { self._storage = new u8[](); self._start = 0; } }\n",
        ),
        (
            "extra field",
            "public class Str { private _storage: shared u8[]; private _start: i64; private _length: u64; private _extra: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; self._extra = 0u; } }\n",
        ),
        (
            "reordered field",
            "public class Str { private _start: i64; private _storage: shared u8[]; private _length: u64; init() { self._start = 0; self._storage = new u8[](); self._length = 0u; } }\n",
        ),
        (
            "public storage field",
            "public class Str { _storage: shared u8[]; private _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "public start field",
            "public class Str { private _storage: shared u8[]; _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "public length field",
            "public class Str { private _storage: shared u8[]; private _start: i64; _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "wrong storage type",
            "public class Str { private _storage: shared u64[]; private _start: i64; private _length: u64; init() { self._storage = new u64[](); self._start = 0; self._length = 0u; } }\n",
        ),
        (
            "wrong start type",
            "public class Str { private _storage: shared u8[]; private _start: u64; private _length: u64; init() { self._storage = new u8[](); self._start = 0u; self._length = 0u; } }\n",
        ),
        (
            "wrong length type",
            "public class Str { private _storage: shared u8[]; private _start: i64; private _length: u8; init() { self._storage = new u8[](); self._start = 0; self._length = 0u8; } }\n",
        ),
        (
            "copy constructor",
            "public class Str { private _storage: shared u8[]; private _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } copy(ref other: Str) { self._storage = other._storage; self._start = other._start; self._length = other._length; } }\n",
        ),
        (
            "copy assignment",
            "public class Str { private _storage: shared u8[]; private _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } assign(ref other: Str) { self._storage = other._storage; self._start = other._start; self._length = other._length; } }\n",
        ),
        (
            "destructor",
            "public class Str { private _storage: shared u8[]; private _start: i64; private _length: u64; init() { self._storage = new u8[](); self._start = 0; self._length = 0u; } destroy {} }\n",
        ),
    ];

    for (name, declaration) in cases {
        let output = resolve_modules(
            "app",
            &[
                (
                    "app.ska",
                    "fn literal() -> unit { \"x\"; }\nfn main() -> i64 { return 0; }\n",
                ),
                ("std/str.ska", declaration),
            ],
        );
        assert!(
            output
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == INVALID_STRING_LANGUAGE_ITEM),
            "{name} must report {INVALID_STRING_LANGUAGE_ITEM}: {:?}",
            output.diagnostics
        );
        assert!(
            output.program.string_language_item.is_none(),
            "{name} must not publish invalid metadata"
        );
        assert!(
            output.diagnostics.has_errors(),
            "{name} must fail in resolution before HIR is requested"
        );
    }
}

#[test]
fn selects_only_the_exact_module_identity_among_equal_str_declarations() {
    let output = resolve_modules(
        "app",
        &[
            (
                "app.ska",
                "import other::str;\nimport std::str;\nfn main() -> i64 { var value: std::str::Str = \"canonical\"; return 0; }\n",
            ),
            ("other/str.ska", VALID_STR),
            ("std/str.ska", VALID_STR),
        ],
    );
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);

    let program = output.program;
    let selected = program.string_language_item.as_ref().unwrap().class;
    let equal_classes = program
        .classes
        .iter()
        .filter(|class| class.name == "Str")
        .map(|class| class.id)
        .collect::<Vec<_>>();
    assert_eq!(equal_classes.len(), 2);
    assert_eq!(selected, *equal_classes.last().unwrap());
    assert_ne!(selected, equal_classes[0]);
}

#[test]
fn intrinsic_metadata_does_not_weaken_source_private_field_access() {
    let output = source_with_str(concat!(
        "from std::str import Str;\n",
        "fn main() -> i64 {\n",
        "  var value: Str = \"private\";\n",
        "  var storage: shared u8[] = value._storage;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.program.string_language_item.is_some());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == PRIVATE_MEMBER_ACCESS));
}

#[test]
fn types_literals_as_exact_produced_values_in_object_flow_contexts() {
    let output = source_with_str(concat!(
        "from std::str import Str;\n",
        "fn consume(value: Str) -> unit {}\n",
        "fn produce() -> Str { return \"result\"; }\n",
        "class Holder {\n",
        "  value: Str;\n",
        "  init() { self.value = \"field\"; }\n",
        "  mut fn replace() -> unit { self.value = \"field assignment\"; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var local: Str = \"local\";\n",
        "  var copied: Str = local;\n",
        "  local = \"assignment\";\n",
        "  consume(\"argument\");\n",
        "  var returned: Str = produce();\n",
        "  var optional: Str? = \"optional\";\n",
        "  if (true) { var branch: Str = \"then\"; } else { var branch: Str = \"else\"; }\n",
        "  { var temporary: Str = (\"temporary\"); }\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(
        output.diagnostics.is_empty(),
        "literal program must resolve: {:?}",
        output.diagnostics
    );
    let checked = type_check(&output.program);
    assert!(
        checked.diagnostics.is_empty(),
        "literal object flows must type-check: {:?}",
        checked.diagnostics
    );
    let hir = checked.hir.expect("valid literals must produce HIR");
    let item = hir.string_language_item.as_ref().unwrap();
    assert_eq!(hir.literal_data.iter().count(), 10);
    assert!(hir
        .definitions
        .iter()
        .flat_map(|definition| definition.body.statements.iter())
        .any(|statement| match statement {
            crate::hir::HirStatement::Local(local) => matches!(
                &local.initializer,
                crate::hir::HirLocalInitializer::Copy(copy)
                    if matches!(
                        &copy.source,
                        crate::hir::HirObjectSource::Produced(
                            HirObjectProducer::StringLiteral(_)
                        )
                    )
            ),
            _ => false,
        }));

    let dump = dump_hir(&hir);
    assert!(dump.contains("StringLanguageItem"));
    assert!(dump.contains(&format!("class {}", item.class)));
    assert!(dump.contains("StringLiteral str0"));
    assert!(dump.contains("ObjectResult"));
    assert!(dump.contains("CopyAssignment"));
    assert_eq!(dump, dump_hir(&hir));
}

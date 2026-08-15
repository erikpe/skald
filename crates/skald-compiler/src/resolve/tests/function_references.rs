use super::*;
use crate::{
    identity::{FunctionTypeId, MethodId},
    test_support::{
        load_module_sources, load_module_sources_with_standard_library_overrides,
        CANONICAL_F64_SOURCE,
    },
    typeck::{type_check, FUNCTION_VALUES_NOT_YET_SUPPORTED as TYPECK_FUNCTION_GATE},
};

fn reference(expression: &ResolvedExpression) -> &ResolvedFunctionReferenceExpr {
    let ResolvedExpression::FunctionReference(reference) = expression else {
        panic!("expected a resolved function reference, got {expression:?}");
    };
    reference
}

#[test]
fn ordinary_references_preserve_exact_targets_signatures_entry_and_identity() {
    let output = resolve_text(concat!(
        "fn first(value: i64) -> bool { return true; }\n",
        "fn second(value: i64) -> bool { return false; }\n",
        "fn main() -> i64 {\n",
        "  var later: fn(i64) -> bool = second;\n",
        "  var earlier: fn(i64) -> bool = first;\n",
        "  var repeated: fn(i64) -> bool = second;\n",
        "  var recursive: fn() -> i64 = main;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);

    let main = output.program.definitions.get(FunctionId::new(2)).unwrap();
    let references = main.body.statements[..4]
        .iter()
        .map(local_initializer)
        .map(reference)
        .collect::<Vec<_>>();
    assert_eq!(
        references
            .iter()
            .map(|reference| reference.target)
            .collect::<Vec<_>>(),
        [
            CallableId::Function(FunctionId::new(1)),
            CallableId::Function(FunctionId::new(0)),
            CallableId::Function(FunctionId::new(1)),
            CallableId::Function(FunctionId::new(2)),
        ]
    );
    assert_eq!(references[0].function_type, FunctionTypeId::new(0));
    assert_eq!(references[1].function_type, references[0].function_type);
    assert_eq!(references[2].function_type, references[0].function_type);
    assert_eq!(references[3].function_type, FunctionTypeId::new(1));

    assert_eq!(
        output
            .program
            .address_taken_callables
            .iter()
            .map(|entry| (entry.target, entry.function_type))
            .collect::<Vec<_>>(),
        [
            (
                CallableId::Function(FunctionId::new(0)),
                FunctionTypeId::new(0)
            ),
            (
                CallableId::Function(FunctionId::new(1)),
                FunctionTypeId::new(0)
            ),
            (
                CallableId::Function(FunctionId::new(2)),
                FunctionTypeId::new(1)
            ),
        ]
    );
}

#[test]
fn callable_parameter_modes_are_part_of_the_reference_signature() {
    let output = resolve_text(concat!(
        "class Item {}\n",
        "fn inspect(ref item: Item, mut ref count: i64) -> bool { return true; }\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(ref Item, mut ref i64) -> bool = inspect;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let main = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let reference = reference(local_initializer(&main.body.statements[0]));
    let signature = output
        .program
        .function_types
        .get(reference.function_type)
        .unwrap();
    assert_eq!(
        signature
            .parameters
            .iter()
            .map(|parameter| parameter.mode)
            .collect::<Vec<_>>(),
        [
            ResolvedFunctionTypeParameterMode::ReadOnlyAlias,
            ResolvedFunctionTypeParameterMode::MutableAlias,
        ]
    );
}

#[test]
fn ordinary_static_methods_form_references_but_direct_calls_stay_direct() {
    let output = resolve_text(concat!(
        "class Math {\n",
        "  static fn apply(value: i64) -> i64 { return value; }\n",
        "}\n",
        "fn direct(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(i64) -> i64 = Math.apply;\n",
        "  var first: i64 = direct(1);\n",
        "  var second: i64 = Math.apply(2);\n",
        "  return first + second;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let main = output.program.definitions.get(FunctionId::new(1)).unwrap();
    let callback = reference(local_initializer(&main.body.statements[0]));
    assert_eq!(
        callback.target,
        CallableId::Method(MethodId::new(ClassId::new(0), 0))
    );
    assert!(matches!(
        local_initializer(&main.body.statements[1]),
        ResolvedExpression::DirectCall(_)
    ));
    assert!(matches!(
        local_initializer(&main.body.statements[2]),
        ResolvedExpression::StaticCall(_)
    ));
    assert_eq!(output.program.address_taken_callables.iter().len(), 1);
}

#[test]
fn private_static_references_are_eligible_inside_the_declaring_class() {
    let output = resolve_text(concat!(
        "class Hooks {\n",
        "  private static fn hidden() -> i64 { return 1; }\n",
        "  static fn expose() -> fn() -> i64 { return Hooks.hidden; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hidden = CallableId::Method(MethodId::new(ClassId::new(0), 0));
    assert!(output.program.address_taken_callables.get(hidden).is_some());
    let expose = output
        .program
        .member_definition(MethodId::new(ClassId::new(0), 1).into())
        .unwrap();
    assert_eq!(
        reference(return_value(&expose.body.statements[0])).target,
        hidden
    );
}

#[test]
fn imported_qualified_and_selective_references_reuse_canonical_identity_and_access_rules() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                concat!(
                    "import dep;\n",
                    "from dep import exported;\n",
                    "fn main() -> i64 {\n",
                    "  var qualified: fn(i64) -> i64 = dep::exported;\n",
                    "  var selected: fn(i64) -> i64 = exported;\n",
                    "  var forbidden: fn() -> i64 = dep::hidden;\n",
                    "  return 0;\n",
                    "}\n",
                ),
            ),
            (
                "dep.ska",
                concat!(
                    "public fn exported(value: i64) -> i64 { return value; }\n",
                    "fn hidden() -> i64 { return 1; }\n",
                ),
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [PRIVATE_DECLARATION]
    );

    let main_id = output.program.entry_function.unwrap();
    let main = output.program.definitions.get(main_id).unwrap();
    let qualified = reference(local_initializer(&main.body.statements[0]));
    let selected = reference(local_initializer(&main.body.statements[1]));
    assert_eq!(qualified.target, selected.target);
    assert_eq!(qualified.function_type, selected.function_type);
    assert_eq!(output.program.address_taken_callables.iter().len(), 1);
}

#[test]
fn lexical_and_field_callees_shadow_declaration_names_and_hit_the_indirect_call_gate() {
    let output = resolve_text(concat!(
        "fn target() -> i64 { return 1; }\n",
        "class Holder {\n",
        "  callback: fn() -> i64;\n",
        "  static fallback: fn() -> i64 = target;\n",
        "  fn invoke() -> i64 { return self.callback(); }\n",
        "  static fn invoke_fallback() -> i64 { return Holder.fallback(); }\n",
        "}\n",
        "fn invoke(callback: fn() -> i64) -> i64 { return callback(); }\n",
        "fn main() -> i64 {\n",
        "  var target: fn() -> i64 = target;\n",
        "  return target();\n",
        "}\n",
    ));
    assert_eq!(
        output
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [
            INDIRECT_FUNCTION_CALL_NOT_YET_SUPPORTED,
            INDIRECT_FUNCTION_CALL_NOT_YET_SUPPORTED,
            INDIRECT_FUNCTION_CALL_NOT_YET_SUPPORTED,
            INDIRECT_FUNCTION_CALL_NOT_YET_SUPPORTED,
        ]
    );
    assert_eq!(output.program.address_taken_callables.iter().len(), 1);
    assert!(output
        .program
        .address_taken_callables
        .get(CallableId::Function(FunctionId::new(0)))
        .is_some());
}

#[test]
fn ineligible_callable_families_receive_reference_specific_diagnostics() {
    let intrinsic_source =
        format!("{CANONICAL_F64_SOURCE}\nfn expose() -> fn(f64) -> u64 {{ return _to_bits; }}\n");
    let (_workspace, graph) = load_module_sources_with_standard_library_overrides(
        "app",
        &[("app.ska", "import std::f64;\nextern fn external() -> i64;\nfn main() -> i64 { var callback: fn() -> i64 = external; return 0; }\n")],
        &[("std/f64.ska", intrinsic_source.as_str())],
    );
    let output = resolve_module_graph(&graph);
    let messages = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == INVALID_FUNCTION_REFERENCE)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>();
    assert!(messages.contains(&"external function `external` cannot be used as a function value"));
    assert!(messages.contains(&"intrinsic function `_to_bits` cannot be used as a function value"));
    assert!(output.program.address_taken_callables.is_empty());
}

#[test]
fn instance_virtual_and_interface_selections_never_form_bound_references() {
    let output = resolve_text(concat!(
        "interface View { fn read() -> i64; }\n",
        "class Direct { fn read() -> i64 { return 1; } }\n",
        "class Dynamic { virtual fn read() -> i64 { return 2; } }\n",
        "fn reject(ref direct: Direct, ref dynamic: Dynamic, ref view: View) -> unit {\n",
        "  var first: fn() -> i64 = direct.read;\n",
        "  var second: fn() -> i64 = dynamic.read;\n",
        "  var third: fn() -> i64 = view.read;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(
        diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        [
            INVALID_FUNCTION_REFERENCE,
            INVALID_FUNCTION_REFERENCE,
            INVALID_FUNCTION_REFERENCE,
        ]
    );
    assert!(diagnostics[0].message.contains("instance method"));
    assert!(diagnostics[1].message.contains("virtual method"));
    assert!(diagnostics[2].message.contains("interface requirement"));
    assert!(diagnostics.iter().all(|diagnostic| diagnostic
        .notes
        .iter()
        .any(|note| note.contains("bound method"))));
    assert!(output.program.address_taken_callables.is_empty());
}

#[test]
fn generic_static_references_remain_at_the_fvi2_specialization_gate() {
    let output = resolve_text(concat!(
        "class Identity<T> { static fn apply(value: T) -> T { return value; } }\n",
        "fn raw() -> unit { var callback: fn(i64) -> i64 = Identity.apply; }\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(i64) -> i64 = Identity<i64>::apply;\n",
        "  return 0;\n",
        "}\n",
    ));
    let diagnostics = output.diagnostics.iter().collect::<Vec<_>>();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0].code, RAW_GENERIC_TYPE);
    assert_eq!(
        diagnostics[1].code,
        GENERIC_FUNCTION_REFERENCE_NOT_YET_SUPPORTED
    );
    assert!(diagnostics[1].message.contains("apply"));
    assert!(output.program.address_taken_callables.is_empty());
}

#[test]
fn dumps_expose_reference_nodes_and_target_sorted_address_metadata_before_the_lowering_gate() {
    let output = resolve_text(concat!(
        "fn first() -> i64 { return 1; }\n",
        "fn second() -> i64 { return 2; }\n",
        "fn main() -> i64 {\n",
        "  var later: fn() -> i64 = second;\n",
        "  var earlier: fn() -> i64 = first;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("FunctionReference f1 type ft0"));
    assert!(dump.contains("FunctionReference f0 type ft0"));
    let first = dump.find("AddressTaken f0 type ft0").unwrap();
    let second = dump.find("AddressTaken f1 type ft0").unwrap();
    assert!(first < second);

    let checked = type_check(&output.program);
    let diagnostic = checked.diagnostics.iter().next().unwrap();
    assert_eq!(diagnostic.code, TYPECK_FUNCTION_GATE);
    assert_eq!(
        diagnostic.labels[0].span,
        output
            .program
            .address_taken_callables
            .iter()
            .next()
            .unwrap()
            .first_reference_span
    );
}

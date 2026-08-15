use super::*;
use crate::{
    identity::{ArrayTypeId, FunctionTypeId, OptionalTypeId},
    test_support::load_module_sources,
    typeck::type_check,
};

#[test]
fn interns_exact_function_signatures_bottom_up_in_first_use_order() {
    let output = resolve_text(concat!(
        "class Item { init() {} }\n",
        "fn signatures(\n",
        "  first: fn() -> i64, same: fn() -> i64,\n",
        "  read: fn(ref Item) -> i64, write: fn(mut ref Item) -> i64,\n",
        "  higher: fn(fn() -> i64) -> fn() -> i64,\n",
        "  array: (fn() -> i64)[], optional: (fn() -> i64)?\n",
        ") -> fn() -> i64 { return 0; }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors());

    let signatures = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let kinds = signatures
        .parameters
        .iter()
        .map(|parameter| parameter.type_syntax.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds[0], ResolvedTypeKind::Function(FunctionTypeId::new(0)));
    assert_eq!(kinds[1], kinds[0]);
    assert_eq!(kinds[2], ResolvedTypeKind::Function(FunctionTypeId::new(1)));
    assert_eq!(kinds[3], ResolvedTypeKind::Function(FunctionTypeId::new(2)));
    assert_eq!(kinds[4], ResolvedTypeKind::Function(FunctionTypeId::new(3)));
    assert_eq!(kinds[5], ResolvedTypeKind::Array(ArrayTypeId::new(0)));
    assert_eq!(kinds[6], ResolvedTypeKind::Optional(OptionalTypeId::new(0)));
    assert_eq!(signatures.return_type.kind, kinds[0]);

    let table = &output.program.function_types;
    assert_eq!(table.len(), 4);
    assert!(table
        .get(FunctionTypeId::new(0))
        .unwrap()
        .parameters
        .is_empty());
    assert_eq!(
        table.get(FunctionTypeId::new(1)).unwrap().parameters[0].mode,
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias
    );
    assert_eq!(
        table.get(FunctionTypeId::new(2)).unwrap().parameters[0].mode,
        ResolvedFunctionTypeParameterMode::MutableAlias
    );
    let higher = table.get(FunctionTypeId::new(3)).unwrap();
    assert_eq!(
        higher.parameters[0].type_syntax.kind,
        ResolvedTypeKind::Function(FunctionTypeId::new(0))
    );
    assert_eq!(
        higher.result.kind,
        ResolvedTypeKind::Function(FunctionTypeId::new(0))
    );
    assert_eq!(
        output
            .program
            .array_types
            .get(ArrayTypeId::new(0))
            .unwrap()
            .element
            .kind,
        ResolvedTypeKind::Function(FunctionTypeId::new(0))
    );
    assert_eq!(
        output
            .program
            .optional_types
            .get(OptionalTypeId::new(0))
            .unwrap()
            .payload
            .kind,
        ResolvedTypeKind::Function(FunctionTypeId::new(0))
    );
}

#[test]
fn grouping_and_optional_shorthand_do_not_change_function_type_identity() {
    let output = resolve_text(concat!(
        "class Item { init() {} }\n",
        "fn compare(\n",
        "  plain: fn() -> i64, grouped: (fn() -> i64),\n",
        "  shorthand: fn(shared? Item) -> unit,\n",
        "  canonical: fn((shared Item)?) -> unit\n",
        ") -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let compare = output.program.declarations.get(FunctionId::new(0)).unwrap();
    let kinds = compare
        .parameters
        .iter()
        .map(|parameter| parameter.type_syntax.kind)
        .collect::<Vec<_>>();
    assert_eq!(kinds[0], ResolvedTypeKind::Function(FunctionTypeId::new(0)));
    assert_eq!(kinds[1], kinds[0]);
    assert_eq!(kinds[2], ResolvedTypeKind::Function(FunctionTypeId::new(1)));
    assert_eq!(kinds[3], kinds[2]);
    assert_eq!(output.program.function_types.len(), 2);
}

#[test]
fn resolved_dump_renders_ids_modes_and_recursive_source_names_deterministically() {
    let output = resolve_text(
        "class Item { init() {} }\n\
         fn use(value: fn(ref Item, mut ref i64[]) -> fn() -> bool) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let dump = dump_resolved(&output.program);
    assert_eq!(dump, dump_resolved(&output.program));
    assert!(dump.contains("FunctionTypes"));
    assert!(dump.contains("FunctionType ft0 fn() -> bool"));
    assert!(dump.contains("FunctionType ft1 fn(ref Item, mut ref i64[]) -> fn() -> bool"));
    assert!(dump.contains("ReadOnlyAlias class c0"));
    assert!(dump.contains("MutableAlias array a0"));
    assert!(dump.contains("Type Function ft1 fn(ref Item, mut ref i64[]) -> fn() -> bool"));
}

#[test]
fn source_rendering_groups_function_types_used_by_postfix_containers() {
    let output = resolve_text(
        "fn use(value: fn((fn() -> i64)[], (fn() -> i64)?) -> unit) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let dump = dump_resolved(&output.program);
    assert!(dump.contains("fn((fn() -> i64)[], (fn() -> i64)?) -> unit"));
}

#[test]
fn canonical_identity_is_shared_across_modules_after_name_resolution() {
    let (_workspace, graph) = load_module_sources(
        "app",
        &[
            (
                "app.ska",
                "import dep;\nfn use(value: fn(dep::Item) -> dep::Item) -> unit {}\nfn main() -> i64 { return 0; }\n",
            ),
            (
                "dep.ska",
                "public class Item { init() {} }\npublic fn use(value: fn(Item) -> Item) -> unit {}\n",
            ),
        ],
    );
    let output = resolve_module_graph(&graph);
    assert!(!output.has_errors());
    assert_eq!(output.program.function_types.len(), 1);
}

#[test]
fn generic_function_type_arguments_close_into_hir() {
    let output = resolve_text(concat!(
        "class Holder<T> { value: T; init(value: T) { self.value = value; } }\n",
        "fn use(value: Holder<fn(i64) -> bool>) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    assert_eq!(output.program.classes.len(), 1);
    let holder = output.program.classes.iter().next().unwrap();
    assert!(matches!(
        holder.fields[0].type_syntax.kind,
        ResolvedTypeKind::Function(_)
    ));

    let checked = type_check(&output.program);
    assert!(checked.hir.is_some(), "{:?}", checked.diagnostics);
    assert!(checked.diagnostics.is_empty());
}

#[test]
fn direct_type_check_api_lowers_resolved_function_types_without_panicking() {
    let output =
        resolve_text("fn use(value: fn(i64) -> bool) -> unit {}\nfn main() -> i64 { return 0; }\n");
    let checked = type_check(&output.program);
    let hir = checked.hir.expect("valid function types must lower to HIR");
    assert!(checked.diagnostics.is_empty());
    assert_eq!(hir.function_types.iter().count(), 1);
}

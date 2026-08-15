use super::*;
use crate::{
    hir::{
        HirDestructionStep, HirExpressionKind, HirLocalInitializer, HirScalarStorage,
        HirStoredValueInitialization, HirSynthesizedFieldCopy,
    },
    identity::{BindingId, ClassId, FunctionId},
};

#[test]
fn function_reference_hir_dump_is_exact() {
    let output = check_text(concat!(
        "fn target(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { var callback: fn(i64) -> i64 = target; return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    assert_eq!(
        dump_hir(&output.hir.unwrap()),
        concat!(
            "HirProgram @0..117\n",
            "  SelectedModule m0\n",
            "  Modules\n",
            "    Module m0 main source 0 provider provider0 package package0\n",
            "  Entry f1\n",
            "  FunctionTypes\n",
            "    FunctionType ft0 -> i64 @80..94\n",
            "      Parameter Value i64 @83..86\n",
            "  Declarations\n",
            "    Declaration f0 module m0 \"target\" internal @0..46\n",
            "      Parameters\n",
            "        Parameter f0:p0 \"value\" value : i64 @10..20\n",
            "      ReturnType i64\n",
            "    Declaration f1 module m0 \"main\" internal @47..116\n",
            "      Parameters\n",
            "      ReturnType i64\n",
            "  Definitions\n",
            "    Definition f0 @0..46\n",
            "      Locals\n",
            "      Block @29..46\n",
            "        Return @31..44\n",
            "          Binding f0:p0 : i64 @38..43\n",
            "    Definition f1 @47..116\n",
            "      Locals\n",
            "        Local f1:l0 \"callback\" : fn(i64) -> i64 @66..104\n",
            "      Block @64..116\n",
            "        LocalDeclaration f1:l0 @66..104\n",
            "          FunctionReference f0 signature ft0 : fn(i64) -> i64 @97..103\n",
            "        Return @105..114\n",
            "          Integer 0 : i64 @112..113\n",
        )
    );
}

#[test]
fn function_values_use_scalar_storage_transport_and_lifecycle_hir() {
    let output = check_text(concat!(
        "fn increment(value: i64) -> i64 { return value + 1; }\n",
        "fn forward(callback: fn(i64) -> i64) -> fn(i64) -> i64 { return callback; }\n",
        "class Holder {\n",
        "  callback: fn(i64) -> i64;\n",
        "  static fallback: fn(i64) -> i64 = increment;\n",
        "  init(callback: fn(i64) -> i64) { self.callback = callback; }\n",
        "  mut fn replace(next: fn(i64) -> i64) -> fn(i64) -> i64 {\n",
        "    self.callback = next;\n",
        "    Holder.fallback = next;\n",
        "    return self.callback;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(i64) -> i64 = increment;\n",
        "  callback = forward(callback);\n",
        "  var holder: Holder = Holder(callback);\n",
        "  var copy: Holder = holder;\n",
        "  copy = holder;\n",
        "  copy.callback = callback;\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    assert_eq!(hir.function_types.iter().count(), 1);

    let holder = hir.class(ClassId::new(0)).unwrap();
    assert!(matches!(holder.fields[0].ty, Type::Function(_)));
    assert!(holder
        .destruction
        .steps
        .iter()
        .all(|step| !matches!(step, HirDestructionStep::Field(_))));
    let crate::hir::HirCopyCapability::Synthesized(synthesized) = &holder.copy_constructor else {
        panic!("expected synthesized copy constructor");
    };
    assert!(matches!(
        synthesized.fields.as_slice(),
        [HirSynthesizedFieldCopy::Scalar { .. }]
    ));
    let static_initializer = holder.static_fields[0].initializer.as_ref().unwrap();
    assert!(matches!(
        static_initializer.value,
        HirStoredValueInitialization::Scalar(_)
    ));

    let main = hir.definitions.get(FunctionId::new(2)).unwrap();
    assert!(main.body.statements.iter().any(|statement| matches!(
        statement,
        HirStatement::ScalarAssignment(assignment)
            if assignment.destination.storage
                == HirScalarStorage::Binding(BindingId::Local(main.locals[0].id))
    )));
    let dump = dump_hir(&hir);
    assert!(dump.contains("FunctionTypes\n"));
    assert!(dump.contains("FunctionReference f0 signature ft0"));
    assert!(dump.contains("ScalarBindingAssignment"));
    assert!(dump.contains("ScalarStaticAssignment"));
}

#[test]
fn function_values_cross_every_internal_callable_signature_exactly() {
    let output = check_text(concat!(
        "fn identity(value: i64) -> i64 { return value; }\n",
        "interface Mapper { fn map(callback: fn(i64) -> i64) -> fn(i64) -> i64; }\n",
        "class Base implements Mapper {\n",
        "  init(callback: fn(i64) -> i64) {}\n",
        "  virtual fn map(callback: fn(i64) -> i64) -> fn(i64) -> i64 { return callback; }\n",
        "  static fn select(callback: fn(i64) -> i64) -> fn(i64) -> i64 { return callback; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  init(callback: fn(i64) -> i64) { super(callback); }\n",
        "  override fn map(callback: fn(i64) -> i64) -> fn(i64) -> i64 { return callback; }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var callback: fn(i64) -> i64 = Base.select(identity);\n",
        "  var value: Derived = Derived(callback);\n",
        "  var result: fn(i64) -> i64 = value.map(callback);\n",
        "  return 0;\n",
        "}\n",
    ));
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let base = hir.class(ClassId::new(0)).unwrap();
    let derived = hir.class(ClassId::new(1)).unwrap();
    assert_eq!(
        base.methods[0].parameters[0].ty,
        derived.methods[0].parameters[0].ty
    );
    assert_eq!(base.methods[0].return_type, derived.methods[0].return_type);
    assert!(matches!(base.methods[1].return_type, Type::Function(_)));
    assert_eq!(derived.conformances.len(), 1);
}

#[test]
fn generic_static_specializations_retain_distinct_targets_and_substituted_signatures() {
    let hir = check_generic_source(concat!(
        "class Identity<T> { init() {} static fn apply(value: T) -> T { return value; } }\n",
        "fn main() -> i64 {\n",
        "  var integer: fn(i64) -> i64 = Identity<i64>::apply;\n",
        "  var boolean: fn(bool) -> bool = Identity<bool>::apply;\n",
        "  return 0;\n",
        "}\n",
    ));
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let references = main
        .body
        .statements
        .iter()
        .filter_map(|statement| {
            let HirStatement::Local(local) = statement else {
                return None;
            };
            let HirLocalInitializer::Value(value) = &local.initializer else {
                return None;
            };
            let HirExpressionKind::FunctionReference(reference) = value.kind else {
                return None;
            };
            Some(reference)
        })
        .collect::<Vec<_>>();
    assert_eq!(references.len(), 2);
    assert_ne!(references[0].target, references[1].target);
    assert_ne!(references[0].function_type, references[1].function_type);
    assert!(matches!(
        references[0].target,
        crate::identity::CallableId::Method(_)
    ));
}

#[test]
fn function_value_exclusions_fail_at_existing_semantic_owners() {
    let missing_static_initializer = check_text(concat!(
        "fn target() -> i64 { return 1; }\n",
        "class Hooks { static callback: fn() -> i64; init() {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(missing_static_initializer
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_STATIC_FIELD_TYPE));

    let alias = check_text(concat!(
        "fn use(ref callback: fn() -> i64) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(alias
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_ALIAS_PARAMETER));

    let nested_alias = check_text(concat!(
        "fn use(callback: fn(ref fn() -> i64) -> unit) -> unit {}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(nested_alias
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_ALIAS_PARAMETER));

    let external = check_text(concat!(
        "extern fn install(callback: fn() -> i64) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(external
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));

    let shared = crate::test_support::resolve_source(
        "fn use(callback: shared fn() -> i64) -> unit {} fn main() -> i64 { return 0; }",
    );
    assert!(!shared.diagnostics.is_empty());
    let explicit_copy = crate::test_support::resolve_source(concat!(
        "fn target() -> i64 { return 0; } ",
        "fn consume(callback: fn() -> i64) -> unit {} ",
        "fn main() -> i64 { var callback: fn() -> i64 = target; ",
        "consume(copy callback); return 0; }",
    ));
    assert!(!explicit_copy.diagnostics.is_empty());

    for source in [
        "fn main() -> i64 { var callback: (fn() -> i64)? = none; return 0; }",
        "fn target() -> i64 { return 0; } fn main() -> i64 { var callbacks: (fn() -> i64)[] = target; return 0; }",
        "fn target() -> i64 { return 0; } fn main() -> i64 { var callback: fn() -> i64 = target; return (i64) callback; }",
        "fn target() -> i64 { return 0; } fn main() -> i64 { var callback: fn() -> i64 = target; if (callback == callback) { return 1; } return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(!output.diagnostics.is_empty());
    }
}

#[test]
fn function_value_compatibility_is_exact_at_storage_and_polymorphic_boundaries() {
    for source in [
        "fn target(value: i64) -> i64 { return value; } fn main() -> i64 { var callback: fn() -> i64 = target; return 0; }",
        "fn target(ref value: i64) -> i64 { return value; } fn main() -> i64 { var callback: fn(i64) -> i64 = target; return 0; }",
        "fn target(value: bool) -> i64 { return 0; } fn main() -> i64 { var callback: fn(i64) -> i64 = target; return 0; }",
        "fn target(value: i64) -> bool { return true; } fn main() -> i64 { var callback: fn(i64) -> i64 = target; return 0; }",
        "fn target(callback: fn() -> i64) -> i64 { return 0; } fn main() -> i64 { var callback: fn(fn(i64) -> i64) -> i64 = target; return 0; }",
        "class A { init() {} } class B { init() {} } fn target(value: A) -> i64 { return 0; } fn main() -> i64 { var callback: fn(B) -> i64 = target; return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    }

    let override_mismatch = check_text(concat!(
        "class Base { init() {} virtual fn select(callback: fn(i64) -> i64) -> unit {} }\n",
        "class Derived extends Base { init() { super(); } override fn select(callback: fn(bool) -> i64) -> unit {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(override_mismatch
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_OVERRIDE_SIGNATURE));

    let conformance_mismatch = check_text(concat!(
        "interface Select { fn select(callback: fn(i64) -> i64) -> unit; }\n",
        "class Choice implements Select { init() {} fn select(callback: fn(bool) -> i64) -> unit {} }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(conformance_mismatch
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INTERFACE_CONFORMANCE));
}

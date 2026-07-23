use super::*;
use crate::{
    hir::{
        HirAccess, HirExpressionKind, HirLocalInitializer, HirNarrowingFailure, HirNarrowingKind,
        HirTypeTestKind, HirViewSource, HirViewTarget,
    },
    identity::{ClassId, FunctionId, InterfaceId},
    mir::{lower_hir, HirLoweringError},
    resolve::{ResolvedExpression, ResolvedStatement, ResolvedTypeKind},
};

const TYPES: &str = "\
interface Tag { fn mark() -> unit; }\n\
class Base implements Tag {\n\
  init() {}\n\
  fn mark() -> unit {}\n\
}\n\
class Derived extends Base { init() { super(); } }\n\
class Other { init() {} }\n";

#[test]
fn classifies_type_tests_from_exact_and_forwarded_class_obj_and_interface_views() {
    let output = check_text(&format!(
        "{TYPES}\
         fn classify(ref base: Base, ref erased: Obj, ref tag: Tag) -> bool {{\n\
           var derived: Derived = Derived();\n\
           var other: Other = Other();\n\
           var exact_upcast: bool = derived is Base;\n\
           var exact_failure: bool = other is Base;\n\
           var guaranteed_interface: bool = base is Tag;\n\
           var dynamic_obj: bool = erased is Derived;\n\
           var nested: bool = accept((erased is Derived));\n\
           return tag is Derived;\n\
         }}\n\
         fn accept(value: bool) -> bool {{ return value; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
    let expected = [
        HirTypeTestKind::StaticSuccess,
        HirTypeTestKind::StaticFailure,
        HirTypeTestKind::StaticSuccess,
        HirTypeTestKind::Runtime,
    ];
    for (statement, expected) in definition.body.statements[2..6].iter().zip(expected) {
        let HirStatement::Local(local) = statement else {
            panic!("expected type-test local");
        };
        let HirLocalInitializer::Value(expression) = &local.initializer else {
            panic!("expected scalar initializer");
        };
        let HirExpressionKind::TypeTest(test) = &expression.kind else {
            panic!("expected explicit type-test HIR");
        };
        assert_eq!(test.kind, expected);
        assert_eq!(expression.ty, Type::Bool);
    }
    let HirStatement::Local(exact_failure) = &definition.body.statements[3] else {
        panic!("expected exact-failure local");
    };
    let HirLocalInitializer::Value(exact_failure) = &exact_failure.initializer else {
        panic!("expected scalar initializer");
    };
    let HirExpressionKind::TypeTest(exact_failure) = &exact_failure.kind else {
        panic!("expected type test");
    };
    assert_eq!(
        exact_failure.source.target,
        HirViewTarget::Class(ClassId::new(2))
    );
    assert_eq!(exact_failure.target, HirViewTarget::Class(ClassId::new(0)));

    let returned = returned_expression(definition);
    let HirExpressionKind::TypeTest(test) = &returned.kind else {
        panic!("expected interface type test");
    };
    assert_eq!(test.kind, HirTypeTestKind::Runtime);
    assert_eq!(test.target, HirViewTarget::Class(ClassId::new(1)));
    assert!(matches!(
        test.source.source,
        HirViewSource::Forwarded { .. }
    ));

    assert!(matches!(
        lower_hir(&hir),
        Err(HirLoweringError::TypeOperationsUnsupported { .. })
    ));
}

#[test]
fn checked_narrowing_records_view_identity_access_failure_and_scoped_aliases() {
    let output = check_text(&format!(
        "{TYPES}\
         fn take(ref value: Derived) -> unit {{}}\n\
         fn narrowings(ref erased: Obj, ref tag: Tag, ref derived: Derived) -> unit {{\n\
           narrow ref from_obj: Derived = (erased) {{ take((from_obj)); }}\n\
           narrow ref from_interface: Derived = tag {{ take(from_interface); }}\n\
           narrow ref as_base: Base = derived {{}}\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.definitions.get(FunctionId::new(1)).unwrap();
    assert_eq!(definition.body.statements.len(), 3);
    let expected = [
        (
            HirViewTarget::Class(ClassId::new(1)),
            HirNarrowingKind::Runtime {
                failure: HirNarrowingFailure::Terminate,
            },
        ),
        (
            HirViewTarget::Class(ClassId::new(1)),
            HirNarrowingKind::Runtime {
                failure: HirNarrowingFailure::Terminate,
            },
        ),
        (
            HirViewTarget::Class(ClassId::new(0)),
            HirNarrowingKind::Static,
        ),
    ];
    for (statement, (target, kind)) in definition.body.statements.iter().zip(expected) {
        let HirStatement::Narrowing(narrowing) = statement else {
            panic!("expected narrowing HIR");
        };
        assert_eq!(narrowing.view.target, target);
        assert_eq!(narrowing.view.access, HirAccess::ReadOnly);
        assert_eq!(narrowing.kind, kind);
    }
    let HirStatement::Narrowing(static_upcast) = &definition.body.statements[2] else {
        panic!("expected static narrowing");
    };
    assert!(matches!(
        static_upcast.view.source,
        HirViewSource::Place(ref place) if place.class() == ClassId::new(0)
    ));

    let dump = dump_hir(&hir);
    assert!(dump.contains("Narrowing f1:narrow0 runtime failure=terminate"));
    assert!(!dump.contains("TypeTest"));
    assert!(dump.contains("ObjectView -> class c1 readonly"));
    assert!(dump.contains("ForwardedView f1:p0 : Obj readonly"));
}

#[test]
fn rejects_impossible_invalid_and_access_increasing_narrowing() {
    let output = check_text(&format!(
        "{TYPES}\
         fn invalid(ref erased: Obj, ref other: Other, scalar: i64) -> unit {{\n\
           narrow mut ref mutable: Derived = erased {{}}\n\
           narrow ref impossible: Base = other {{}}\n\
           narrow ref erased_again: Obj = erased {{}}\n\
           narrow ref bad_source_narrow: Base = scalar {{}}\n\
           var bad_source: bool = scalar is Base;\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));

    assert!(output.hir.is_none());
    let codes: Vec<_> = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect();
    assert!(codes.contains(&INSUFFICIENT_ALIAS_ACCESS));
    assert_eq!(
        codes
            .iter()
            .filter(|&&code| code == INVALID_NARROWING)
            .count(),
        3
    );
    assert_eq!(
        codes
            .iter()
            .filter(|&&code| code == INVALID_TYPE_TEST)
            .count(),
        1
    );

    let mut resolved = resolve_text(&format!(
        "{TYPES}\
         fn corrupt(ref erased: Obj) -> bool {{ return erased is Derived; }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    let definition = resolved
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let ResolvedStatement::Return(returned) = &mut definition.body.statements[0] else {
        panic!("expected return");
    };
    let ResolvedExpression::TypeTest(test) = returned.value.as_mut().unwrap() else {
        panic!("expected type test");
    };
    test.target.kind = ResolvedTypeKind::I64;
    let output = type_check(&resolved);
    assert!(output.hir.is_none());
    assert_eq!(
        output.diagnostics.iter().next().unwrap().code,
        INVALID_TYPE_TEST
    );
}

#[test]
fn supports_deep_base_and_interface_targets_without_losing_static_information() {
    let output = check_text(
        "interface Marker { fn mark() -> unit; }\n\
         class Root implements Marker { init() {} fn mark() -> unit {} }\n\
         class Middle extends Root { init() { super(); } }\n\
         class Leaf extends Middle { init() { super(); } }\n\
         fn inspect(ref leaf: Leaf) -> bool { return leaf is Marker; }\n\
         fn main() -> i64 { return 0; }\n",
    );

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let test = returned_expression(hir.definitions.get(FunctionId::new(0)).unwrap());
    let HirExpressionKind::TypeTest(test) = &test.kind else {
        panic!("expected type test");
    };
    assert_eq!(test.kind, HirTypeTestKind::StaticSuccess);
    assert_eq!(test.target, HirViewTarget::Interface(InterfaceId::new(0)));
}

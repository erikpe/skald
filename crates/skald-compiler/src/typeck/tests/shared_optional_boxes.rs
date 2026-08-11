use super::*;

use crate::{
    hir::{
        dump_hir, HirLocalInitializer, HirOwnerTransfer, HirSharedProducer, HirSharedSource,
        HirSharedTarget, HirStatement,
    },
    identity::FunctionId,
    resolve::ResolvedCopyOperation,
};

#[test]
fn local_boxes_select_construction_owner_and_polymorphic_view_plans() {
    let output = check_text(
        "interface Marker { fn mark() -> i64; }\n\
         class Base { init() {} virtual fn mark() -> i64 { return 1; } }\n\
         class Derived extends Base implements Marker {\n\
           init() { super(); }\n\
           override fn mark() -> i64 { return 2; }\n\
         }\n\
         fn main() -> i64 {\n\
           var exact: shared Derived? = new Derived?(Derived());\n\
           var alias: shared Derived? = exact;\n\
           var base: shared Base? = exact;\n\
           var marker: shared Marker? = exact;\n\
           var object: shared Obj? = exact;\n\
           var maybe_marker: shared? Marker? = exact;\n\
           base = new Base?(Base());\n\
           var independent: shared Derived? = new Derived?(*exact);\n\
           var primitive: shared i64? = new i64?(41);\n\
           var maybe: shared? i64? = new i64?();\n\
           var array_box: shared i64[]? = new i64[]?(i64[]{1, 2});\n\
           var owner_box: shared (shared Base)? = new (shared Base)?(new Base());\n\
           var nested_box: shared i64?? = new i64??(some(some(7)));\n\
           var dynamic_source: shared Base = new Derived();\n\
           var checked_box: shared Derived? = new Derived?(*dynamic_source);\n\
           var optional_value: i64? = 5;\n\
           var copied_optional_box: shared i64? = new i64?(optional_value);\n\
           var produced_optional_box: shared i64? = new i64?(maybe_number());\n\
           return 0;\n\
         }\n\
         fn maybe_number() -> i64? { return some(9); }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output.hir.expect("BX1 local boxes must produce HIR");
    assert_eq!(hir.optional_box_types.iter().count(), 8);
    assert!(hir
        .optional_box_types
        .iter()
        .filter(|target| target.object_view.is_some())
        .any(|target| target.exact_optional.is_none()));

    let main = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Local(exact) = &main.body.statements[0] else {
        panic!("expected exact box local")
    };
    let HirLocalInitializer::Shared(exact) = &exact.initializer else {
        panic!("expected shared initializer")
    };
    assert_eq!(exact.operation, HirOwnerTransfer::Adopt);
    let HirSharedSource::Produced(HirSharedProducer::OptionalBoxAllocation(allocation)) =
        &exact.source
    else {
        panic!("expected typed optional-box allocation")
    };
    assert_eq!(
        allocation.exact_optional,
        allocation.initialization_type(&hir)
    );
    assert_eq!(allocation.produced_owner, HirOwnerTransfer::Adopt);
    assert!(allocation.new_span.range().start() < allocation.target_span.range().start());
    assert_eq!(allocation.publication_span, allocation.span);

    let HirStatement::Local(alias) = &main.body.statements[1] else {
        panic!("expected alias local")
    };
    let HirLocalInitializer::Shared(alias) = &alias.initializer else {
        panic!("expected shared alias initializer")
    };
    assert_eq!(alias.operation, HirOwnerTransfer::Copy);

    for statement in &main.body.statements[2..5] {
        let HirStatement::Local(local) = statement else {
            panic!("expected polymorphic box-view local")
        };
        let HirLocalInitializer::Shared(transfer) = &local.initializer else {
            panic!("expected shared box-view initializer")
        };
        assert_eq!(transfer.operation, HirOwnerTransfer::Copy);
        assert!(matches!(transfer.target, HirSharedTarget::OptionalBox(_)));
    }

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("OptionalBoxTypes"), "{dump}");
    assert!(dump.contains("OptionalBoxAllocation exact="), "{dump}");
    assert!(dump.contains("OptionalBoxPointeeCopy"), "{dump}");
    assert!(dump.contains(": shared class c1?"), "{dump}");
    assert!(dump.contains(": shared interface i0?"), "{dump}");
    assert!(dump.contains(": shared Obj?"), "{dump}");
    assert!(dump.contains(": (shared i64?)?"), "{dump}");
    assert!(dump.contains(": (shared interface i0?)?"), "{dump}");
    assert!(dump.contains("ArrayInitialization"), "{dump}");
    assert!(dump.contains("OptionalSharedInitialization"), "{dump}");
    assert!(dump.contains("AggregateOptional"), "{dump}");
    assert!(dump.contains("CheckedSource runtime-terminate"), "{dump}");
    assert!(dump.contains("OptionalCopy"), "{dump}");
    assert!(dump.contains("OptionalProduced"), "{dump}");
}

#[test]
fn absent_and_direct_class_box_construction_do_not_require_copy_capability() {
    let mut program = resolve_text(
        "class Value { init() {} }\n\
         fn main() -> i64 {\n\
           var absent: shared Value? = new Value?();\n\
           var direct: shared Value? = new Value?(Value());\n\
           return 0;\n\
         }\n",
    );
    program.classes.entries_mut_for_test()[0].copy_constructor = ResolvedCopyOperation::Unavailable;

    let output = crate::typeck::type_check(&program);
    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let dump = dump_hir(&output.hir.expect("direct plans must remain constructible"));
    assert!(
        dump.contains("ClassOptionalInitialization class c0? absent"),
        "{dump}"
    );
    assert!(
        dump.contains("ClassOptionalInitialization class c0? direct"),
        "{dump}"
    );
}

#[test]
fn independent_box_copy_requires_the_exact_optional_copy_plan() {
    let mut program = resolve_text(
        "class Value { init() {} }\n\
         fn main() -> i64 {\n\
           var source: shared Value? = new Value?();\n\
           var copy: shared Value? = new Value?(*source);\n\
           return 0;\n\
         }\n",
    );
    program.classes.entries_mut_for_test()[0].copy_constructor = ResolvedCopyOperation::Unavailable;

    let output = crate::typeck::type_check(&program);
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == crate::typeck::COPY_OPERATION_UNAVAILABLE));
}

#[test]
fn boxes_reject_invalid_exact_sources_and_invariant_conversions() {
    let invalid_object = check_text(
        "class Base { init() {} }\n\
         class Derived extends Base { init() { super(); } }\n\
         fn main() -> i64 {\n\
           var bad: shared Base? = new Derived?(Base());\n\
           return 0;\n\
         }\n",
    );
    assert!(invalid_object.hir.is_none());
    assert!(
        invalid_object
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_CONSTRUCTION),
        "{:?}",
        invalid_object.diagnostics
    );

    for source in [
        "fn main() -> i64 { var bad: shared i64? = new u64?(1u); return 0; }",
        "fn main() -> i64 { var bad: shared i64?? = new i64?(1); return 0; }",
        "class Base { init() {} } class Derived extends Base { init() { super(); } } fn main() -> i64 { var base: shared Base? = new Base?(); var bad: shared Derived? = base; return 0; }",
        "class Left { init() {} } class Right { init() {} } fn main() -> i64 { var bad: shared Left? = new Right?(); return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == crate::typeck::INVALID_SHARED_CONVERSION));
    }
}

#[test]
fn stored_box_positions_remain_focused_later_phase_gates() {
    for source in [
        "fn consume(value: shared i64?) -> unit {} fn main() -> i64 { return 0; }",
        "extern fn consume(value: shared i64?) -> unit; fn main() -> i64 { return 0; }",
        "class Holder { value: shared i64?; init(value: shared i64?) { self.value = value; } } fn main() -> i64 { return 0; }",
        "fn main() -> i64 { var boxes: (shared i64?)[] = (shared i64?)[](); return 0; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none(), "source unexpectedly produced HIR: {source}");
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == crate::typeck::SHARED_OPTIONAL_BOX_UNAVAILABLE), "source: {source}\n{:?}", output.diagnostics);
    }
}

#[test]
fn exact_box_pointees_support_explicit_optional_consumers() {
    let output = check_text(
        "class Value {\n\
           value: i64;\n\
           init(value: i64) { self.value = value; }\n\
           mut fn set(value: i64) -> unit { self.value = value; }\n\
         }\n\
         fn inspect(ref value: i64?) -> bool { return value is some; }\n\
         fn main() -> i64 {\n\
           var number: shared i64? = new i64?(41);\n\
           var copied: i64? = *number;\n\
           var present: bool = (*number) is some;\n\
           var aliased: bool = inspect(*number);\n\
           var scalar: i64 = (*number)!;\n\
           var nested: shared i64?? = new i64??(some(some(7)));\n\
           var inner: i64? = (*nested)!;\n\
           var values: shared i64[]? = new i64[]?(i64[]{1, 2});\n\
           var array: i64[] = (*values)!;\n\
           var object: shared Value? = new Value?(Value(1));\n\
           (*object)!.set(9);\n\
           return scalar + inner! + array[0] + (*object)!.value;\n\
         }\n",
    );

    assert!(output.diagnostics.is_empty(), "{:?}", output.diagnostics);
    let hir = output
        .hir
        .expect("exact boxed optional consumers must type check");
    let dump = dump_hir(&hir);
    assert!(dump.contains("OptionalBoxPointee"), "{dump}");
    assert!(dump.contains("ClassOptionalBoxPointee"), "{dump}");
}

#[test]
fn published_box_wrappers_reject_mutable_whole_value_aliases() {
    let output = check_text(
        "fn clear(mut ref value: i64?) -> unit { value = none; }\n\
         fn main() -> i64 {\n\
           var box: shared i64? = new i64?(1);\n\
           clear(*box);\n\
           return 0;\n\
         }\n",
    );
    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::typeck::INVALID_ALIAS_ARGUMENT
            && diagnostic.message.contains("cannot be mutably aliased")
    }));
}

#[test]
fn published_box_wrappers_reject_whole_pointee_assignment() {
    let output = crate::test_support::resolve_source(
        "fn main() -> i64 {\n\
           var box: shared i64? = new i64?(1);\n\
           *box = none;\n\
           return 0;\n\
         }\n",
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| { diagnostic.code == crate::resolve::INVALID_POINTEE_ASSIGNMENT }));
}

#[test]
fn box_owners_do_not_implicitly_forward_optional_operations() {
    for source in [
        "fn main() -> i64 { var box: shared i64? = new i64?(1); if (box is some) { return 1; } return 0; }",
        "fn main() -> i64 { var box: shared i64? = new i64?(1); var value: i64 = box!; return value; }",
    ] {
        let output = check_text(source);
        assert!(output.hir.is_none());
        assert!(output
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == crate::typeck::TYPE_MISMATCH));
    }

    for source in [
        "class Value { value: i64; init() { self.value = 1; } } fn main() -> i64 { var box: shared Value? = new Value?(Value()); return box.value; }",
        "class Value { value: i64; init() { self.value = 1; } } fn main() -> i64 { var box: shared Value? = new Value?(Value()); return box->value; }",
    ] {
        let output = crate::test_support::resolve_source(source);
        assert!(output.diagnostics.iter().any(|diagnostic| {
            matches!(
                diagnostic.code,
                crate::resolve::IMPLICIT_SHARED_DEREFERENCE
                    | crate::resolve::INVALID_MEMBER_SELECTION
            )
        }));
    }
}

trait OptionalBoxAllocationTestExt {
    fn initialization_type(&self, hir: &crate::hir::HirProgram) -> crate::identity::OptionalTypeId;
}

impl OptionalBoxAllocationTestExt for crate::hir::HirOptionalBoxAllocation {
    fn initialization_type(&self, hir: &crate::hir::HirProgram) -> crate::identity::OptionalTypeId {
        hir.optional_box_type(self.exact_target)
            .and_then(|target| target.exact_optional)
            .expect("exact allocation target must name optional metadata")
    }
}

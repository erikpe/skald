use super::*;
use crate::{
    hir::{
        HirExpressionKind, HirInterfaceReceiver, HirMethodDispatch, HirViewSource, HirViewTarget,
    },
    typeck::{INVALID_INTERFACE_CONFORMANCE, TYPE_MISMATCH},
};

fn returned_expression(body: &crate::hir::HirBlock) -> &crate::hir::HirExpression {
    let HirStatement::Return(returned) = body.statements.last().unwrap() else {
        panic!("expected a return statement")
    };
    let crate::hir::HirReturnValue::Scalar(expression) = returned.value.as_ref().unwrap() else {
        panic!("expected a scalar return value")
    };
    expression
}

#[test]
fn closed_generic_bases_use_ordinary_override_conformance_and_view_semantics() {
    let hir = check_generic_source(
        "interface Ranked { fn rank() -> i64; }\n\
         class Root<T> implements Ranked {\n\
           init() {}\n\
           virtual fn rank() -> i64 { return 1; }\n\
         }\n\
         class Mid<T> extends Root<T> {\n\
           init() { super(); }\n\
           override fn rank() -> i64 { return 2; }\n\
         }\n\
         class Leaf extends Mid<i64> {\n\
           init() { super(); }\n\
           override fn rank() -> i64 { return 3; }\n\
         }\n\
         class Reader<T> where T: Ranked {\n\
           value: T;\n\
           init(ref value: T) { self.value = value; }\n\
           fn read() -> i64 { return self.value.rank(); }\n\
         }\n\
         fn through_root(ref value: Root<i64>) -> i64 { return value.rank(); }\n\
         fn through_interface(ref value: Ranked) -> i64 { return value.rank(); }\n\
         fn type_test(ref value: Obj) -> bool { return value is Mid<i64>; }\n\
         fn checked(ref value: Obj) -> i64 { return ((Mid<i64>) value).rank(); }\n\
         fn sliced(value: Leaf) -> i64 {\n\
           var root: Root<i64> = value;\n\
           return root.rank();\n\
         }\n\
         fn use(ref value: Reader<Leaf>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let root = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Root<"))
        .unwrap();
    let mid = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Mid<"))
        .unwrap();
    let leaf = hir
        .classes
        .iter()
        .find(|class| class.name == "Leaf")
        .unwrap();
    let reader = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("Reader<"))
        .unwrap();

    assert_eq!(mid.direct_base.as_ref().unwrap().class, root.id);
    assert_eq!(leaf.direct_base.as_ref().unwrap().class, mid.id);
    assert_eq!(root.conformances.len(), 1);
    assert_eq!(mid.conformances.len(), 1);
    assert_eq!(leaf.conformances.len(), 1);

    let HirMethodDispatch::VirtualRoot { family, slot } = root.methods[0].kind.dispatch().unwrap()
    else {
        panic!("generic root must allocate an ordinary virtual family")
    };
    for class in [mid, leaf] {
        assert!(matches!(
            class.methods[0].kind.dispatch(),
            Some(HirMethodDispatch::Override {
                family: candidate_family,
                slot: candidate_slot,
                ..
            }) if candidate_family == family && candidate_slot == slot
        ));
    }

    let reader_body = hir
        .member_definition(reader.methods[0].id.into())
        .expect("generated method body must be retained");
    let HirExpressionKind::InterfaceCall {
        receiver, target, ..
    } = &returned_expression(&reader_body.body).kind
    else {
        panic!("bound-selected call must use interface dispatch")
    };
    assert_eq!(target.interface, root.conformances[0].interface);
    assert!(matches!(
        receiver,
        HirInterfaceReceiver::View(view)
            if view.target == HirViewTarget::Interface(target.interface)
    ));

    let dump = dump_hir(&hir);
    for fragment in [
        "InterfaceCall i0 i0:requirement0",
        "Dispatch VirtualRoot",
        "Dispatch Override",
        "TypeTest",
        "SliceSource",
    ] {
        assert!(dump.contains(fragment), "missing `{fragment}`:\n{dump}");
    }
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("generic object-model MIR must verify");
}

#[test]
fn bound_calls_on_array_elements_preserve_checked_storage_and_interface_dispatch() {
    let hir = check_generic_source(
        "interface Ranked { fn rank() -> i64; }\n\
         class Item implements Ranked {\n\
           init() {}\n\
           fn rank() -> i64 { return 1; }\n\
         }\n\
         class RankedArray<T> where T: Ranked {\n\
           values: T[];\n\
           init() { self.values = T[](); }\n\
           fn first() -> i64 { return self.values[0].rank(); }\n\
         }\n\
         fn use(ref value: RankedArray<Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let array = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("RankedArray<"))
        .unwrap();
    let body = hir.member_definition(array.methods[0].id.into()).unwrap();
    let HirExpressionKind::InterfaceCall { receiver, .. } = &returned_expression(&body.body).kind
    else {
        panic!("bound-selected array element call must use interface dispatch")
    };
    assert!(matches!(
        receiver,
        HirInterfaceReceiver::View(view)
            if matches!(view.source, HirViewSource::ArrayElement(_))
    ));
    let dump = dump_hir(&hir);
    assert!(dump.contains("ArrayElementPlace"), "{dump}");
    assert!(dump.contains("InterfaceCall"), "{dump}");
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("array-backed bound call MIR must verify");
}

#[test]
fn a_bound_applies_to_an_explicit_shared_pointee_without_lifting_to_the_owner() {
    let hir = check_generic_source(
        "interface Ranked { fn rank() -> i64; }\n\
         class Item implements Ranked {\n\
           init() {}\n\
           fn rank() -> i64 { return 1; }\n\
         }\n\
         class SharedReader<T> where T: Ranked {\n\
           init() {}\n\
           fn read(value: shared T) -> i64 { return value->rank(); }\n\
         }\n\
         fn use(ref value: SharedReader<Item>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let reader = hir
        .classes
        .iter()
        .find(|class| class.name.starts_with("SharedReader<"))
        .unwrap();
    let body = hir.member_definition(reader.methods[0].id.into()).unwrap();
    let HirExpressionKind::InterfaceCall { receiver, .. } = &returned_expression(&body.body).kind
    else {
        panic!("an explicit shared-pointee call must use interface dispatch")
    };
    assert!(matches!(receiver, HirInterfaceReceiver::View(_)));
    let dump = dump_hir(&hir);
    assert!(dump.contains("Origin Shared"), "{dump}");
    let mir = crate::mir::lower_hir(&hir);
    crate::mir::verify_mir(&mir).expect("shared-pointee bound call MIR must verify");
}

#[test]
fn each_closed_signature_builds_an_independent_virtual_family() {
    let hir = check_generic_source(
        "class Root<T> {\n\
           init() {}\n\
           virtual fn echo(value: T) -> T { return value; }\n\
         }\n\
         class Child<T> extends Root<T> {\n\
           init() { super(); }\n\
           override fn echo(value: T) -> T { return value; }\n\
         }\n\
         fn integers(ref value: Child<i64>) -> unit {}\n\
         fn booleans(ref value: Child<bool>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );

    let roots = hir
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Root<"))
        .collect::<Vec<_>>();
    let children = hir
        .classes
        .iter()
        .filter(|class| class.name.starts_with("Child<"))
        .collect::<Vec<_>>();
    assert_eq!((roots.len(), children.len()), (2, 2));

    let mut families = Vec::new();
    for child in children {
        let root = hir
            .class(child.direct_base.as_ref().unwrap().class)
            .expect("closed child base must be the matching closed root");
        assert_eq!(
            child.methods[0].parameters[0].ty,
            root.methods[0].parameters[0].ty
        );
        assert_eq!(child.methods[0].return_type, root.methods[0].return_type);
        let HirMethodDispatch::VirtualRoot { family, slot } =
            root.methods[0].kind.dispatch().unwrap()
        else {
            panic!("each closed root must own a virtual family")
        };
        assert!(matches!(
            child.methods[0].kind.dispatch(),
            Some(HirMethodDispatch::Override {
                family: child_family,
                slot: child_slot,
                ..
            }) if child_family == family && child_slot == slot
        ));
        families.push(family);
    }
    families.sort();
    families.dedup();
    assert_eq!(families.len(), 2);
}

#[test]
fn generic_interface_claims_are_checked_against_each_substituted_signature() {
    let valid = check_generic_source(
        "interface Sink { fn accept(value: i64) -> unit; }\n\
         class Adapter<T> implements Sink {\n\
           init() {}\n\
           fn accept(value: T) -> unit {}\n\
         }\n\
         fn use(ref value: Adapter<i64>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    );
    let adapter = valid
        .classes
        .iter()
        .find(|class| class.name.starts_with("Adapter<"))
        .unwrap();
    assert_eq!(adapter.conformances.len(), 1);
    assert_eq!(
        adapter.conformances[0].implementations[0].method,
        adapter.methods[0].id
    );

    let invalid = crate::typeck::type_check(&resolve_generic_source(
        "interface Sink { fn accept(value: i64) -> unit; }\n\
         class Adapter<T> implements Sink {\n\
           init() {}\n\
           fn accept(value: T) -> unit {}\n\
         }\n\
         fn use(ref value: Adapter<bool>) -> unit {}\n\
         fn main() -> i64 { return 0; }\n",
    ));
    assert!(invalid.hir.is_none());
    assert!(invalid
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_INTERFACE_CONFORMANCE));
}

#[test]
fn closed_generic_applications_remain_invariant() {
    let program = resolve_generic_source(
        "class Base { init() {} }\n\
         class Derived extends Base { init() { super(); } }\n\
         class Box<T> { init() {} }\n\
         fn take(ref value: Box<Base>) -> unit {}\n\
         fn reject(ref value: Box<Derived>) -> unit { take(value); }\n\
         fn main() -> i64 { return 0; }\n",
    );
    let checked = crate::typeck::type_check(&program);
    assert!(checked.hir.is_none());
    assert_eq!(checked.diagnostics.len(), 1, "{:?}", checked.diagnostics);
    assert_eq!(
        checked.diagnostics.iter().next().unwrap().code,
        TYPE_MISMATCH
    );
}

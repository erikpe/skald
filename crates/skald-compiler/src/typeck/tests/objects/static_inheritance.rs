use super::*;
use crate::hir::{HirObjectReturn, HirObjectSource, HirReturnValue, HirViewSource, HirViewTarget};

const STATIC_HIERARCHY: &str = concat!(
    "class Root {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "  mut fn write(value: i64) -> unit { self.value = value; }\n",
    "}\n",
    "class Middle extends Root { init(value: i64) { super(value); } }\n",
    "class Leaf extends Middle { init(value: i64) { super(value); } }\n",
);

#[test]
fn inherited_fields_and_methods_use_identity_selected_base_receivers() {
    let output = check_text(&format!(
        "{STATIC_HIERARCHY}\
         fn inspect(ref leaf: Leaf) -> i64 {{ return leaf.read() + leaf.value; }}\n\
         fn update(mut ref leaf: Leaf) -> unit {{ leaf.write(1); }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let hir = output.hir.unwrap();
    let inspect = hir.definitions.get(FunctionId::new(0)).unwrap();
    let HirStatement::Return(result) = &inspect.body.statements[0] else {
        panic!("inspect must return the inherited reads");
    };
    let Some(HirReturnValue::Scalar(expression)) = &result.value else {
        panic!("inspect must return a scalar expression");
    };
    let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
        panic!("inspect must retain source expression order");
    };
    let HirExpressionKind::MethodCall {
        receiver, target, ..
    } = &left.kind
    else {
        panic!("left operand must be the inherited method call");
    };
    assert_eq!(
        *target,
        crate::hir::HirMethodCallTarget::Direct(MethodId::new(ClassId::new(0), 0))
    );
    assert_eq!(
        receiver.place.projections(),
        [
            ObjectProjection::Base(ClassId::new(1)),
            ObjectProjection::Base(ClassId::new(0)),
        ]
    );
    assert_eq!(receiver.place.class(), ClassId::new(0));
    assert_eq!(receiver.place.access, HirAccess::ReadOnly);

    let HirExpressionKind::FieldRead(field) = &right.kind else {
        panic!("right operand must be the inherited field read");
    };
    assert_eq!(field.field, FieldId::new(ClassId::new(0), 0));
    assert_eq!(field.receiver.projections(), receiver.place.projections());

    let update = hir.definitions.get(FunctionId::new(1)).unwrap();
    let HirStatement::Call(call) = &update.body.statements[0] else {
        panic!("update must contain the inherited mutable call");
    };
    let HirExpressionKind::MethodCall { receiver, .. } = &call.call.kind else {
        panic!("update must retain its method receiver");
    };
    assert_eq!(receiver.place.access, HirAccess::Mutable);
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("inherited member MIR must verify");
    assert!(crate::mir::dump_mir(&mir).contains(".base(c1).base(c0)"));
}

#[test]
fn alias_upcasts_retain_access_target_and_complete_source_identity() {
    let output = check_text(&format!(
        "{STATIC_HIERARCHY}\
         fn root(ref value: Root) -> unit {{}}\n\
         fn mutable_root(mut ref value: Root) -> unit {{}}\n\
         fn any(ref value: Obj) -> unit {{ any(value); }}\n\
         fn forward(mut ref leaf: Leaf) -> unit {{\n\
           root(leaf);\n\
           mutable_root(leaf);\n\
           any(leaf);\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let hir = output.hir.unwrap();
    let any = hir.definitions.get(FunctionId::new(2)).unwrap();
    let HirStatement::Call(recursive) = &any.body.statements[0] else {
        panic!("Obj forwarding must remain a call");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &recursive.call.kind else {
        panic!("Obj forwarding must retain the direct target");
    };
    let HirCallArgument::View(forwarded) = &arguments[0] else {
        panic!("Obj forwarding must be an explicit view");
    };
    assert_eq!(forwarded.target, HirViewTarget::Obj);
    assert!(matches!(
        forwarded.source,
        HirViewSource::Forwarded {
            target: HirViewTarget::Obj,
            ..
        }
    ));

    let forward = hir.definitions.get(FunctionId::new(3)).unwrap();
    let arguments = forward
        .body
        .statements
        .iter()
        .map(|statement| {
            let HirStatement::Call(call) = statement else {
                panic!("forward body must contain only calls");
            };
            let HirExpressionKind::DirectCall { arguments, .. } = &call.call.kind else {
                panic!("forward body must contain direct calls");
            };
            &arguments[0]
        })
        .collect::<Vec<_>>();
    let HirCallArgument::View(readonly_base) = arguments[0] else {
        panic!("derived-to-base alias conversion must be explicit");
    };
    assert_eq!(readonly_base.target, HirViewTarget::Class(ClassId::new(0)));
    assert_eq!(readonly_base.access, HirAccess::ReadOnly);
    let HirViewSource::Place(base_place) = &readonly_base.source else {
        panic!("class upcast must retain the projected source place");
    };
    assert_eq!(
        base_place.projections(),
        [
            ObjectProjection::Base(ClassId::new(1)),
            ObjectProjection::Base(ClassId::new(0)),
        ]
    );
    let HirCallArgument::View(mutable_base) = arguments[1] else {
        panic!("mutable derived-to-base conversion must be explicit");
    };
    assert_eq!(mutable_base.access, HirAccess::Mutable);
    let HirCallArgument::View(obj) = arguments[2] else {
        panic!("class-to-Obj conversion must be explicit");
    };
    assert_eq!(obj.target, HirViewTarget::Obj);

    let dump = crate::hir::dump_hir(&hir);
    assert!(dump.contains("ViewArgument -> class c0 readonly"));
    assert!(dump.contains("ViewArgument -> Obj readonly"));
    assert!(dump.contains("ForwardedView f2:p0 : Obj readonly"));
    assert!(dump.contains("-> base c1 -> base c0 : class c0"));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("class and Obj view MIR must verify");
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains("-> class c0 readonly"));
    assert!(mir_dump.contains("-> class c0 mutable"));
    assert!(mir_dump.contains("-> Obj readonly"));
}

#[test]
fn slicing_is_explicit_across_owning_destinations_and_never_elided() {
    let output = check_text(&format!(
        "{STATIC_HIERARCHY}\
         class Holder {{\n\
           item: Root;\n\
           init(ref leaf: Leaf) {{ self.item = leaf; }}\n\
           mut fn replace(ref leaf: Leaf) -> unit {{ self.item = leaf; }}\n\
         }}\n\
         fn consume(value: Root) -> unit {{}}\n\
         fn make() -> Leaf {{ return Leaf(1); }}\n\
         fn slice_result(ref leaf: Leaf) -> Root {{ return leaf; }}\n\
         fn exercise(destination: Root, ref leaf: Leaf) -> unit {{\n\
           var from_place: Root = leaf;\n\
           var from_fresh: Root = Leaf(2);\n\
           var from_grouped: Root = (Leaf(3));\n\
           var from_call: Root = make();\n\
           consume(leaf);\n\
           consume(make());\n\
           destination = leaf;\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    assert!(
        output.diagnostics.is_empty(),
        "unexpected diagnostics: {:?}",
        output.diagnostics
    );
    let hir = output.hir.unwrap();
    let root = hir.class(ClassId::new(0)).unwrap();
    let root_copy = root.copy_constructor.selected().unwrap();
    let root_assignment = root.copy_assignment.selected().unwrap();

    let holder = hir.class_definitions.get(ClassId::new(3)).unwrap();
    let HirStatement::FieldCopyConstruction(field) = &holder.initializers[0].body.statements[0]
    else {
        panic!("derived field source must slice through copy construction");
    };
    assert_eq!(field.operation, root_copy);
    assert_slice(&field.source, ClassId::new(2), ClassId::new(0));
    let HirStatement::CopyAssignment(field_assignment) = &holder.methods[0].body.statements[0]
    else {
        panic!("method field replacement must use whole-object assignment");
    };
    assert_eq!(field_assignment.operation, root_assignment);
    assert_slice(&field_assignment.source, ClassId::new(2), ClassId::new(0));

    let result = hir.definitions.get(FunctionId::new(2)).unwrap();
    let HirStatement::Return(result) = &result.body.statements[0] else {
        panic!("slice_result must return");
    };
    let Some(HirReturnValue::Object(HirObjectReturn::Copy {
        source, operation, ..
    })) = &result.value
    else {
        panic!("slicing must not use constructor elision");
    };
    assert_eq!(*operation, root_copy);
    assert_slice(source, ClassId::new(2), ClassId::new(0));

    let exercise = hir.definitions.get(FunctionId::new(3)).unwrap();
    for statement in &exercise.body.statements[..4] {
        let HirStatement::Local(local) = statement else {
            panic!("first exercise statements must be locals");
        };
        let HirLocalInitializer::Copy(copy) = &local.initializer else {
            panic!("every derived-to-base local must be an explicit copy");
        };
        assert_eq!(copy.operation, root_copy);
        assert_slice(&copy.source, ClassId::new(2), ClassId::new(0));
    }
    for statement in &exercise.body.statements[4..6] {
        let HirStatement::Call(call) = statement else {
            panic!("middle exercise statements must be calls");
        };
        let HirExpressionKind::DirectCall { arguments, .. } = &call.call.kind else {
            panic!("consume must be a direct call");
        };
        let HirCallArgument::Copy(copy) = &arguments[0] else {
            panic!("owning value arguments must copy");
        };
        assert_eq!(copy.operation, root_copy);
        assert_slice(&copy.source, ClassId::new(2), ClassId::new(0));
    }
    let HirStatement::CopyAssignment(assignment) = &exercise.body.statements[6] else {
        panic!("last exercise statement must assign");
    };
    assert_eq!(assignment.operation, root_assignment);
    assert_slice(&assignment.source, ClassId::new(2), ClassId::new(0));

    let dump = crate::hir::dump_hir(&hir);
    assert!(dump.contains("SliceSource [c1 -> c0] -> c0"));
    assert_eq!(dump, crate::hir::dump_hir(&hir));
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("all lowered owning slice contexts must verify");
    let mir_dump = crate::mir::dump_mir(&mir);
    assert!(mir_dump.contains(".base(c1).base(c0)"));
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
}

#[test]
fn invalid_static_conversions_report_relationship_and_access_failures() {
    let output = check_text(&format!(
        "{STATIC_HIERARCHY}\
         class Other {{ init() {{}} }}\n\
         fn take_root(ref value: Root) -> unit {{}}\n\
         fn mutate_root(mut ref value: Root) -> unit {{}}\n\
         fn bad(ref leaf: Leaf, ref root: Root, ref other: Other, ref any: Obj) -> unit {{\n\
           leaf.write(1);\n\
           mutate_root(leaf);\n\
           take_root(other);\n\
           take_root(any);\n\
           var impossible: Leaf = root;\n\
         }}\n\
         fn main() -> i64 {{ return 0; }}\n"
    ));
    assert!(output.hir.is_none());
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER));
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INSUFFICIENT_ALIAS_ACCESS));
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == TYPE_MISMATCH)
            .count()
            >= 2
    );
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("ancestry relation")
    }));
}

#[test]
fn obj_views_lower_to_verified_mir() {
    let output = check_text(concat!(
        "class Root { init() {} }\n",
        "fn any(ref value: Obj) -> unit {}\n",
        "fn pass(ref value: Root) -> unit { any(value); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let mir = lower_hir(&hir);
    verify_mir(&mir).expect("lowered static object views must verify");
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("view(indirect("));
    assert!(dump.contains(" -> Obj readonly origin forwarded("));
}

#[test]
fn obj_is_restricted_to_non_owning_internal_alias_positions() {
    let output = check_text(concat!(
        "class Invalid { view: Obj; init() {} }\n",
        "fn invalid(value: Obj) -> Obj { return; }\n",
        "extern fn external(ref value: Obj) -> unit;\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.hir.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_DECLARATION)
            .count()
            >= 3
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == INVALID_EXTERNAL_DECLARATION));
}

fn assert_slice(source: &HirObjectSource, actual: ClassId, target: ClassId) {
    let HirObjectSource::Slice(slice) = source else {
        panic!("expected an explicit owning slice");
    };
    assert_eq!(slice.source.class(), actual);
    assert_eq!(slice.target, target);
    assert_eq!(
        slice.bases,
        [ClassId::new(1), ClassId::new(0)],
        "deep slicing must retain every selected direct-base identity"
    );
}

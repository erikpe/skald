use super::*;
use crate::{
    backend::{plan::*, BackendInput, RuntimeTracePolicy},
    mir::*,
    test_support::lower_source_to_complete_final_mir_with_sources,
};

fn fixture(source: &str) -> crate::test_support::FinalMirWithSources {
    lower_source_to_complete_final_mir_with_sources("planning.ska", source)
}

#[test]
fn scalar_and_higher_order_signatures_have_complete_checked_pools() {
    let fixture = fixture("fn add(value: i64) -> i64 { return value + 1; } fn choose() -> fn(i64) -> i64 { return add; } fn invoke(callback: fn(i64) -> i64) -> i64 { return callback(4); } fn main() -> i64 { var callback: fn(i64) -> i64 = choose(); return invoke(callback); }");
    let plan = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    assert_eq!(plan.program().executable_definitions().count(), 4);
    let view = plan.plan().view();
    assert_eq!(view.runtime_trace(), RuntimeTracePolicy::Omitted);
    assert_eq!(
        view.callables()
            .filter(|c| c.body == BodyDisposition::Required)
            .count(),
        5
    );
    for ty in [
        MirType::I64,
        MirType::U64,
        MirType::U8,
        MirType::Bool,
        MirType::F64,
        MirType::Unit,
    ] {
        let id = plan.layout(ty).unwrap();
        let fact = view.layout(view.layout_id(id.index()).unwrap()).unwrap();
        assert_eq!(
            fact.size,
            if ty == MirType::Unit {
                0
            } else if matches!(ty, MirType::U8 | MirType::Bool) {
                1
            } else {
                8
            }
        );
    }
    assert_ne!(plan.layout(MirType::I64), plan.layout(MirType::U64));
    for id in plan.function_types.keys() {
        assert!(plan.function_type(*id).is_some());
    }
    assert!(plan.trace().requests.is_empty());
    assert!(view.artifacts().all(|a| !matches!(
        a.key,
        ArtifactId::TraceTls
            | ArtifactId::Data(
                DataKey::TraceBytes(_) | DataKey::TraceContext(_) | DataKey::TraceLocation(_)
            )
    )));
}

#[test]
fn arithmetic_checks_and_all_primitive_cells_are_admitted() {
    let fixture = fixture("fn compute(x: i64, y: u64, z: u8, f: f64, b: bool) -> i64 { var n: i64 = (i64) f; var count: u64 = 1u; while (count < 3u) { n = n + (x << count); count = count + 1u; } if (b) { return n / x + n % x; } return ((i64) y) + ((i64) z); } fn main() -> i64 { return compute(2, 3u, 4u8, 5.0, true); }");
    let plan = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    assert!(plan
        .plan()
        .view()
        .artifacts()
        .any(|a| a.key == ArtifactId::Runtime(RuntimeService::Panic)));
}

#[test]
fn artifact_policy_cannot_hide_an_unsupported_retained_body() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "excluded.ska",
        "fn dead(values: i64[]) -> i64 { return 0; } fn main() -> i64 { return 0; }",
    );
    for (input, body_must_identify_owner) in [
        (BackendInput::without_runtime_trace(&fixture.mir), false),
        (
            BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
            true,
        ),
    ] {
        let Err(AdmissionError::Unsupported(reason)) = admit(input) else {
            panic!("unsupported retained owner body must reject")
        };
        assert!(!reason.reason.is_empty());
        assert_eq!(reason.callable.is_some(), body_must_identify_owner);
    }
}

#[test]
fn sparse_domains_preserve_absent_declarations_and_never_resurrect_bodies() {
    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "sparse.ska",
        "fn dead() -> i64 { return 4; } fn main() -> i64 { return 0; }",
    );
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("expected sparse rewrite")
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    for reachable in [false, true] {
        let mut input = BackendInput::without_runtime_trace(&sparse);
        if reachable {
            input = input.with_reachable_artifacts_only();
        }
        let plan = admit(input).unwrap();
        let view = plan.plan().view();
        assert_eq!(
            view.callables()
                .filter(|c| matches!(c.key, LirCallableId::Source(_))
                    && c.body == BodyDisposition::Required)
                .count(),
            1
        );
        assert_eq!(
            view.callables()
                .filter(|c| c.body == BodyDisposition::Absent)
                .count(),
            1
        );
        assert_eq!(
            view.artifact_policy(),
            if reachable {
                ArtifactPolicy::Reachable
            } else {
                ArtifactPolicy::Complete
            }
        );
    }
}

#[test]
fn enabled_trace_facts_are_owned_and_omitted_never_looks_up_sources() {
    let fixture = lower_source_to_complete_final_mir_with_sources(
        "trace.ska",
        "fn main() -> i64 { var x: i64 = 2; return x; }",
    );
    let enabled = admit(BackendInput::with_runtime_trace(
        &fixture.mir,
        &fixture.sources,
    ))
    .unwrap();
    assert!(!enabled.trace().strings.is_empty());
    assert_eq!(enabled.trace().contexts.len(), 1);
    let view = enabled.plan().view();
    assert_eq!(
        view.resources().tls.as_ref().unwrap().initializers,
        [DataInitializerFact::Zero(8)]
    );
    for request in &enabled.trace().requests {
        view.artifact_id(ArtifactId::Data(request.location))
            .unwrap();
        assert!(enabled
            .program()
            .has_executable_definition(request.callable));
        assert_eq!(request.span.source_id(), enabled.program().span.source_id());
    }
    for context in &enabled.trace().contexts {
        for key in [context.name, context.path] {
            view.artifact_id(ArtifactId::Data(key)).unwrap();
        }
    }
    for location in &enabled.trace().locations {
        view.artifact_id(ArtifactId::Data(location.context))
            .unwrap();
        assert!(location.line > 0 && location.column > 0);
    }
    let empty = crate::source::SourceDatabase::new();
    assert!(matches!(
        admit(BackendInput::with_runtime_trace(&fixture.mir, &empty)),
        Err(AdmissionError::Backend(_))
    ));
    let omitted = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    assert!(omitted.trace().strings.is_empty());
    assert!(omitted.plan().view().resources().tls.is_none());
}

#[test]
fn excluded_source_families_reject_before_plan_publication() {
    let sources = [
        "fn dead(values: i64[]) -> i64 { return 0; } fn main() -> i64 { return 0; }",
        "class State { static count: i64; init() {} } fn main() -> i64 { return State.count; }",
    ];
    for source in sources {
        let fixture = fixture(source);
        for input in [
            BackendInput::without_runtime_trace(&fixture.mir),
            BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
        ] {
            let Err(AdmissionError::Unsupported(reason)) = admit(input) else {
                panic!("excluded fixture admitted: {source}")
            };
            assert!(!reason.reason.is_empty());
        }
    }
}

#[test]
fn class_lifecycle_is_admitted_under_both_artifact_policies() {
    for source in [
        "class Item { init() {} } fn dead(value: shared Item) -> i64 { return 0; } fn main() -> i64 { return 0; }",
        "class Item { init() {} fn value() -> i64 { return 1; } } fn main() -> i64 { var item: Item = Item(); return item.value(); }",
    ] {
        let fixture = fixture(source);
        for input in [
            BackendInput::without_runtime_trace(&fixture.mir),
            BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only(),
        ] {
            admit(input).expect("class lifecycle is complete under both artifact policies");
        }
    }
}

#[test]
fn extern_cells_are_projected_without_changing_public_emission() {
    let fixture = fixture("extern fn foreign(a: i64, b: u64, c: u8, d: f64, e: bool) -> i64; fn main() -> i64 { return foreign(1, 2u, 3u8, 4.0, true); }");
    let plan = admit(BackendInput::without_runtime_trace(&fixture.mir)).unwrap();
    assert_eq!(
        plan.plan()
            .view()
            .artifacts()
            .filter(|a| matches!(a.key, ArtifactId::External(_)))
            .count(),
        1
    );
    let assembly = crate::backend::emit_assembly(
        crate::backend::Target::X86_64SysV,
        BackendInput::without_runtime_trace(&fixture.mir),
    )
    .unwrap();
    assert!(assembly.contains("call foreign"));
}

#[test]
fn normalized_intrinsics_and_user_panic_respect_the_admission_boundary() {
    for (source, supported) in [
        ("import std::f64; extern fn input() -> f64; fn main() -> i64 { return (i64) std::f64::from_bits(std::f64::to_bits(input())); }", true),
        ("import std::io; fn main() -> i64 { std::io::println_i64(1); return 0; }", false),
        ("import std::error; fn main() -> i64 { std::error::panic(\"failure\"); return 0; }", false),
    ] {
        let (_directory, graph) = crate::test_support::load_module_sources_with_standard_library("app", &[("app.ska", source)]);
        let resolved = crate::resolve::resolve_module_graph(&graph);
        assert!(resolved.diagnostics.is_empty(), "{:?}", resolved.diagnostics);
        let checked = crate::typeck::type_check(&resolved.program);
        assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
        let program = crate::test_support::lower_hir_to_final_mir(&checked.hir.unwrap());
        let verified = crate::passes::run_mir_pipeline(program).unwrap();
        let result = admit(BackendInput::without_runtime_trace(&verified).with_reachable_artifacts_only());
        if supported {result.unwrap();}
        else {assert!(matches!(result, Err(AdmissionError::Unsupported(_))));}
    }
}

#[test]
fn sparse_receiverless_methods_share_canonical_code_signatures() {
    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let fixture = fixture("class Math { init() {} static fn add(x: i64) -> i64 { return x + 1; } } fn invoke(callback: fn(i64) -> i64) -> i64 { return callback(4); } fn main() -> i64 { return invoke(Math.add); }");
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("unused initializer must be removed")
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    let plan = admit(BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only())
        .unwrap();
    let method = sparse
        .program()
        .classes
        .iter()
        .next()
        .unwrap()
        .methods
        .iter()
        .find(|m| m.name == "add")
        .unwrap();
    let declaration = plan
        .plan()
        .view()
        .callables()
        .find(|c| c.key == LirCallableId::Source(method.id.into()))
        .unwrap();
    assert_eq!(declaration.body, BodyDisposition::Required);
    assert!(plan
        .function_types
        .values()
        .any(|id| *id == declaration.signature));
    assert!(plan
        .plan()
        .view()
        .callables()
        .any(|c| c.body == BodyDisposition::Absent));
    let complete = admit(BackendInput::without_runtime_trace(&sparse)).unwrap();
    assert!(complete
        .plan()
        .view()
        .resources()
        .generated
        .iter()
        .any(|fact| matches!(
            fact.callable,
            LirCallableId::Helper(key) if key.family == HelperFamily::ClassFinalizer
        )));
    assert!(complete
        .plan()
        .view()
        .resources()
        .generated
        .iter()
        .all(|fact| !matches!(
            fact.callable,
            LirCallableId::Helper(key) if key.family == HelperFamily::RawClassCopy
        )));
}

#[test]
fn absent_signatures_preserve_source_identity_despite_equal_physical_cells() {
    let fixture = fixture("fn read(ref x: i64) -> i64 { return x; } fn write(mut ref x: i64) -> i64 { return x; } fn ro(callback: fn(ref i64) -> i64) -> unit {} fn rw(callback: fn(mut ref i64) -> i64) -> unit {} fn main() -> i64 { return 0; }");
    let verified = crate::passes::run_mir_pipeline(fixture.mir.program().clone()).unwrap();
    let plan = admit(BackendInput::without_runtime_trace(&verified)).unwrap();
    let view = plan.plan().view();
    let signature = |name| {
        let id = plan
            .program()
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap()
            .id;
        view.callables()
            .find(|c| c.key == LirCallableId::Source(id.into()))
            .unwrap()
            .signature
    };
    let read = signature("read");
    let write = signature("write");
    assert_ne!(read, write);
    assert_eq!(
        view.signature(view.signature_id(read.index()).unwrap())
            .unwrap(),
        view.signature(view.signature_id(write.index()).unwrap())
            .unwrap()
    );
    for name in ["read", "write"] {
        let declaration = plan
            .program()
            .declarations
            .iter()
            .find(|d| d.name == name)
            .unwrap();
        let function_type = plan
            .program()
            .function_types
            .iter()
            .find(|t| t.parameters == declaration.parameters && t.result == declaration.return_type)
            .unwrap();
        assert_eq!(plan.function_type(function_type.id), Some(signature(name)));
    }
}

#[test]
fn scalar_io_intrinsic_rejects_even_without_lifecycle_dependencies() {
    let (_directory, graph) =
        crate::test_support::load_module_sources_with_standard_library_overrides(
            "app",
            &[(
                "app.ska",
                "import std::io; fn main() -> i64 { return std::io::close(1); }",
            )],
            &[(
                "std/io.ska",
                concat!(
                "intrinsic fn _io_standard_handle(stream: u8) -> i64;",
                "intrinsic fn _io_open(ref path: u8[], mode: u8) -> i64;",
                "intrinsic fn _io_read(handle: i64, mut ref destination: u8[], offset: u64) -> i64;",
                "intrinsic fn _io_write(handle: i64, ref source: u8[], offset: u64) -> i64;",
                "intrinsic fn _io_close(handle: i64) -> i64;",
                "public fn close(handle: i64) -> i64 { return _io_close(handle); }",
            ),
            )],
        );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let program = crate::test_support::lower_hir_to_final_mir(&checked.hir.unwrap());
    let verified = crate::passes::run_mir_pipeline(program).unwrap();
    let Err(AdmissionError::Unsupported(reason)) =
        admit(BackendInput::without_runtime_trace(&verified).with_reachable_artifacts_only())
    else {
        panic!("scalar I/O must reject")
    };
    assert!(reason.callable.is_some());
    assert!(reason.reason.contains("Io"), "{}", reason.reason);
}

#[test]
fn semantic_catalog_freezes_layout_views_dispatch_and_recursive_aggregate_facts() {
    let fixture = fixture(concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Base { value: i64; init(value: i64) { self.value = value; } virtual fn read() -> i64 { return self.value; } }\n",
        "class Leaf extends Base implements Readable { tail: u8; init(value: i64) { super(value); self.tail = 1u8; } override fn read() -> i64 { return self.value + 1; } }\n",
        "fn aggregate(value: Leaf, values: i64[], maybe: i64??, boxed: shared Leaf?) -> Leaf { return value; }\n",
        "fn main() -> i64 { return 0; }",
    ));
    let plan = super::projection::project_semantic_catalog(BackendInput::without_runtime_trace(
        &fixture.mir,
    ))
    .unwrap();
    let semantic = plan.view().semantic();

    assert!(semantic.layout(SemanticType::I64).is_some());
    assert_eq!(semantic.classes.len(), 2);
    let leaf = semantic.class(crate::identity::ClassId::new(1)).unwrap();
    assert_eq!(leaf.base.unwrap().class, crate::identity::ClassId::new(0));
    assert_eq!(leaf.fields[0].offset, 8);
    assert_eq!(semantic.field(leaf.fields[0].field), Some(leaf.fields[0]));
    assert!(leaf.shared_allocation.byte_count >= 24);
    assert_eq!(semantic.interfaces.len(), 1);
    assert_eq!(semantic.conformances.len(), 1);
    assert_eq!(semantic.virtual_families.len(), 1);
    assert!(semantic
        .interface(semantic.interfaces[0].interface)
        .is_some());
    assert!(semantic
        .interface_requirement(semantic.interfaces[0].requirements[0].requirement)
        .is_some());
    assert!(semantic
        .conformance(
            semantic.conformances[0].class,
            semantic.conformances[0].interface,
        )
        .is_some());
    assert!(semantic
        .virtual_family(semantic.virtual_families[0].family)
        .is_some());
    assert!(semantic
        .method_slot(MethodSlot::Virtual(semantic.virtual_families[0].family))
        .is_some());
    assert_eq!(semantic.dispatch_tables.len(), 2);
    assert!(semantic
        .dispatch_table(crate::identity::ClassId::new(1))
        .is_some());
    assert!(semantic.arrays.iter().any(|array| {
        array.element == SemanticType::I64
            && array.stride == 8
            && semantic.array(array.array).is_some()
    }));
    assert!(semantic.optionals.len() >= 2);
    assert!(semantic.optional(semantic.optionals[0].optional).is_some());
    assert!(semantic
        .optional_boxes
        .iter()
        .any(|item| item.exact_optional.is_some()
            && item.allocation.is_some()
            && semantic.optional_box(item.optional_box).is_some()));
    let readable = semantic
        .object_views
        .iter()
        .find(|view| matches!(view.target, ObjectViewTarget::Interface(_)))
        .unwrap();
    assert_eq!(semantic.object_view(readable.target), Some(readable));
    assert_eq!(readable.members, [crate::identity::ClassId::new(1)]);
    assert_eq!(
        readable.components,
        [
            ObjectComponent::StaticAddress,
            ObjectComponent::CompleteAddress,
            ObjectComponent::DynamicMetadata,
        ]
    );

    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("the scalar entry must leave aggregate declarations sparse")
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    let admitted =
        admit(BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only())
            .unwrap();
    let aggregate = sparse
        .program()
        .declarations
        .iter()
        .find(|declaration| declaration.name == "aggregate")
        .unwrap();
    let declaration = admitted
        .plan()
        .view()
        .callables()
        .find(|declaration| declaration.key == LirCallableId::Source(aggregate.id.into()))
        .unwrap();
    assert_eq!(declaration.body, BodyDisposition::Absent);
    assert!(matches!(
        admitted
            .plan()
            .view()
            .signature(
                admitted
                    .plan()
                    .view()
                    .signature_id(declaration.signature.index())
                    .unwrap()
            )
            .unwrap()
            .returns,
        ReturnShape::Aggregate(_)
    ));
}

#[test]
fn resource_catalog_freezes_generated_metadata_services_and_data_deterministically() {
    let source = concat!(
        "interface Readable { fn read() -> i64; }\n",
        "class Item implements Readable { value: i64; init(value: i64) { self.value = value; } fn read() -> i64 { return self.value; } }\n",
        "fn retained(value: Item, values: i64[], boxed: shared Item?) -> i64 { return value.read(); }\n",
        "fn main() -> i64 { return 0; }",
    );
    let fixture = fixture(source);
    let input = BackendInput::without_runtime_trace(&fixture.mir);
    let first = super::projection::project_resource_catalog(input).unwrap();
    let second = super::projection::project_resource_catalog(input).unwrap();
    assert_eq!(first.view().resources(), second.view().resources());

    let resources = first.view().resources();
    assert!(resources.generated.iter().any(|fact| matches!(
        fact.callable,
        LirCallableId::Helper(_) | LirCallableId::Entry
    )));
    assert!(resources
        .data
        .iter()
        .any(|fact| matches!(fact.purpose, DataPurpose::ClassDispatch(_))));
    assert!(resources
        .data
        .iter()
        .any(|fact| matches!(fact.purpose, DataPurpose::ArrayDescriptor(_))));
    assert_eq!(
        resources
            .data
            .iter()
            .filter(|fact| fact.purpose == DataPurpose::FailureMessage)
            .count(),
        crate::backend::failure::FailureMessage::ALL.len()
    );
    assert_eq!(
        first
            .view()
            .artifacts()
            .filter(|artifact| matches!(artifact.key, ArtifactId::Runtime(_)))
            .count(),
        9
    );
}

#[test]
fn resource_catalog_separates_complete_inactive_storage_from_reachable_storage() {
    let fixture = fixture(
        "class State {\n\
           static live: i64 = 1;\n\
           static inactive: i64 = 2;\n\
           init() {}\n\
         }\n\
         fn dead() -> i64 { return State.inactive; }\n\
         fn main() -> i64 { return State.live; }",
    );
    let complete = super::projection::project_resource_catalog(
        BackendInput::without_runtime_trace(&fixture.mir),
    )
    .unwrap();
    let resources = complete.view().resources();
    assert_eq!(
        resources
            .statics
            .iter()
            .filter(|fact| fact.disposition == StaticStorageDisposition::Active)
            .count(),
        1
    );
    assert_eq!(
        resources
            .statics
            .iter()
            .filter(|fact| fact.disposition == StaticStorageDisposition::RetainedInactive)
            .count(),
        1
    );
    assert_eq!(resources.activation.len(), 1);
    assert_eq!(resources.shutdown.len(), 1);

    assert!(matches!(
        super::projection::project_resource_catalog(
            BackendInput::without_runtime_trace(&fixture.mir).with_reachable_artifacts_only()
        ),
        Err(AdmissionError::Plan(PlanError::InvalidDomain))
    ));

    use crate::mir::retain::{prepare_reachable_definition_retention, MirDefinitionRetention};
    let MirDefinitionRetention::Changed(retention) =
        prepare_reachable_definition_retention(fixture.mir.program(), fixture.mir.reachability())
            .unwrap()
    else {
        panic!("dead static access must be removed")
    };
    let sparse =
        crate::passes::verify_final_mir(retention.apply(fixture.mir.program().clone()).program)
            .unwrap();
    let reachable = super::projection::project_resource_catalog(
        BackendInput::without_runtime_trace(&sparse).with_reachable_artifacts_only(),
    )
    .unwrap();
    assert!(reachable
        .view()
        .resources()
        .statics
        .iter()
        .all(|fact| fact.disposition == StaticStorageDisposition::Active));
}

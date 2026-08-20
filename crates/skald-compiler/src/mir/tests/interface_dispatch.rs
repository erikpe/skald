use super::{interface_fixtures::*, object_fixtures::messages, *};
use crate::identity::{InterfaceId, InterfaceRequirementId};

#[test]
fn lowers_interface_metadata_views_forwarding_and_calls_explicitly() {
    let (program, ids) = interface_dispatch_mir();
    verify_mir(&program).unwrap();

    let runner = program.interface(ids.runner).unwrap();
    assert_eq!(runner.requirements[0].id, ids.requirement);
    assert_eq!(
        runner.requirements[0].parameters,
        [MirParameter::value(MirType::U64)]
    );
    assert_eq!(runner.requirements[0].return_type, MirType::U64);

    let base = program.conformance(ids.base, ids.runner).unwrap();
    assert_eq!(base.implementations[0].method, ids.base_method);
    let worker = program.conformance(ids.worker, ids.runner).unwrap();
    assert_eq!(worker.implementations[0].method, ids.worker_method);

    let call = first_interface_call(&program);
    assert_eq!(
        call.target,
        MirCallTarget::Interface(MirInterfaceCallTarget {
            interface: ids.runner,
            requirement: ids.requirement,
        })
    );
    let receiver = call
        .receiver
        .as_ref()
        .and_then(MirCallReceiver::as_interface)
        .unwrap();
    assert_eq!(receiver.target, MirViewTarget::Interface(ids.runner));
    assert!(matches!(
        receiver.origin.as_ref(),
        MirObjectOrigin::Forwarded {
            static_target: MirViewTarget::Interface(interface),
            ..
        } if *interface == ids.runner
    ));

    let main = program.definitions.get(ids.main).unwrap();
    assert!(main.body.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::Call(MirCall {
                target: MirCallTarget::Direct(target),
                arguments,
                ..
            }) if *target == ids.invoke
                && matches!(
                    arguments.first(),
                    Some(MirArgument::View(MirObjectView {
                        target: MirViewTarget::Interface(interface),
                        ..
                    })) if *interface == ids.runner
                )
        )
    }));

    let erase = program.definitions.get(ids.erase).unwrap();
    assert!(erase.body.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::Call(MirCall { arguments, .. })
                if matches!(
                    arguments.first(),
                    Some(MirArgument::View(MirObjectView {
                        target: MirViewTarget::Obj,
                        ..
                    }))
                )
        )
    }));
    assert_eq!(ids.forward, FunctionId::new(1));
}

#[test]
fn interface_mir_dump_is_deterministic_and_target_independent() {
    let (program, _) = interface_dispatch_mir();
    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&program));
    assert!(dump.contains("Interface i0 module m0 \"Runner\""));
    assert!(dump.contains("Requirement i0:requirement0 \"run\" readonly (u64) -> u64"));
    assert!(dump.contains("i0:requirement0 -> c1:method0"));
    assert!(dump.contains(
        "call interface i0 i0:requirement0 on view(indirect(f0:s0) -> interface i0 readonly"
    ));
    assert!(!dump.contains("witness"));
    assert!(!dump.contains("offset"));
    assert!(!dump.contains("register"));
}

#[test]
fn interface_object_results_use_existing_destinations_and_cleanup() {
    let program = lower_text(concat!(
        "interface Factory { fn make() -> Product; }\n",
        "class Product { init() {} }\n",
        "class Maker implements Factory {\n",
        "  init() {}\n",
        "  fn make() -> Product { return Product(); }\n",
        "}\n",
        "fn build(ref maker: Factory) -> Product { return maker.make(); }\n",
        "fn main() -> i64 {\n",
        "  var maker: Maker = Maker();\n",
        "  var product: Product = build(maker);\n",
        "  return 0;\n",
        "}\n",
    ));
    verify_mir(&program).unwrap();
    let build = program.definitions.get(FunctionId::new(0)).unwrap();
    let call = build.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Interface(_)) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    let destination = call.destination.as_ref().unwrap();
    assert_eq!(
        build.storage[destination.base.expect_local_storage().index()].kind,
        MirStorageKind::Temporary
    );
    assert!(call.result.is_none());
    assert!(build.body.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::CopyConstruct(copy)
                if copy.destination == MirPlace::base(build.return_storage.unwrap())
                    && copy.source == *destination
        )
    }));
    assert!(build.body.blocks[0].instructions.iter().any(|instruction| {
        matches!(
            instruction,
            MirInstruction::EndFullExpression(end)
                if end.temporaries.iter().any(|cleanup| cleanup.destination == *destination)
        )
    }));
}

#[test]
fn rejects_corrupt_conformance_requirements_views_and_signatures() {
    let (mut wrong_requirement, _) = interface_dispatch_mir();
    let call = first_interface_call_mut(&mut wrong_requirement);
    let MirCallTarget::Interface(target) = &mut call.target else {
        unreachable!()
    };
    target.requirement = InterfaceRequirementId::new(InterfaceId::new(1), 0);
    assert!(messages(&wrong_requirement).iter().any(|message| {
        message == "interface call requirement belongs to a different interface"
    }));

    let (mut wrong_method, ids) = interface_dispatch_mir();
    wrong_method.classes.entries_mut_for_test()[ids.worker.index()].conformances[0]
        .implementations[0]
        .method = ids.base_method;
    assert!(messages(&wrong_method)
        .iter()
        .any(|message| { message.contains("does not select its effective `run` method") }));

    let (mut wrong_view, _) = interface_dispatch_mir();
    let receiver = first_interface_call_mut(&mut wrong_view)
        .receiver
        .as_mut()
        .and_then(MirCallReceiver::as_interface_mut)
        .unwrap();
    receiver.target = MirViewTarget::Obj;
    assert!(messages(&wrong_view)
        .iter()
        .any(|message| message == "interface receiver target differs from the call target"));

    let (mut wrong_signature, _) = interface_dispatch_mir();
    wrong_signature.interfaces.entries_mut_for_test()[0].requirements[0].return_type =
        MirType::Bool;
    let errors = messages(&wrong_signature);
    assert!(errors
        .iter()
        .any(|message| message.contains("different signature or receiver access")));
    assert!(errors
        .iter()
        .any(|message| message == "call result type mismatch"));

    let (mut wrong_access, _) = interface_dispatch_mir();
    wrong_access.interfaces.entries_mut_for_test()[0].requirements[0].receiver_access =
        MirReceiverAccess::Mutable;
    assert!(messages(&wrong_access).iter().any(|message| {
        message == "mutable interface requirement requires mutable receiver access"
    }));

    let (mut static_implementation, ids) = interface_dispatch_mir();
    static_implementation.classes.entries_mut_for_test()[ids.worker.index()].methods[0].kind =
        MirMethodKind::Static;
    assert!(messages(&static_implementation).iter().any(|message| {
        message.contains("static method c1:method0, which cannot satisfy an interface requirement")
    }));
}

#[test]
fn rejects_missing_inherited_conformance_and_owning_interface_storage() {
    let (mut missing, ids) = interface_dispatch_mir();
    missing.classes.entries_mut_for_test()[ids.worker.index()]
        .conformances
        .clear();
    let errors = messages(&missing);
    assert!(errors
        .iter()
        .any(|message| message.contains("omits inherited conformance")));
    assert!(errors
        .iter()
        .any(|message| message.contains("invalid static conversion")));

    let (mut owning, ids) = interface_dispatch_mir();
    let function = owning.definitions.get_mut_for_test(ids.main).unwrap();
    let storage = StorageId::new(ids.main, function.storage.len());
    function.storage.push(MirStorage {
        id: storage,
        source: Some(BindingId::Local(LocalId::new(ids.main, 2))),
        name: "invalid".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Interface(ids.runner),
        span: function.span,
    });
    assert!(messages(&owning).iter().any(|message| {
        message.contains("non-owning interface or `Obj` type must be an alias parameter")
    }));

    let (mut dead, ids) = interface_dispatch_mir();
    let function = dead.definitions.get_mut_for_test(ids.invoke).unwrap();
    let storage = StorageId::new(ids.invoke, function.storage.len());
    function.storage.push(MirStorage {
        id: storage,
        source: Some(BindingId::Local(LocalId::new(ids.invoke, 0))),
        name: "dead".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(ids.worker),
        span: function.span,
    });
    let receiver = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => call
                .receiver
                .as_mut()
                .and_then(MirCallReceiver::as_interface_mut),
            _ => None,
        })
        .unwrap();
    receiver.source = MirPlace::base(storage);
    *receiver.origin = MirObjectOrigin::Exact {
        complete: MirPlace::base(storage),
        dynamic_class: ids.worker,
    };
    let errors = messages(&dead);
    assert!(errors
        .iter()
        .any(|message| message == "interface receiver is not live"));
    assert!(errors
        .iter()
        .any(|message| message == "interface receiver origin is not live"));
}

#[test]
fn closed_generic_interfaces_lower_as_ordinary_exact_mir() {
    let program = lower_text(generic_interface_lowering_source());
    verify_mir(&program).unwrap();

    let final_program = lower_source_to_final_mir(generic_interface_lowering_source());
    verify_mir(&final_program).unwrap();
    assert!(final_program.static_lifecycle.is_some());
    assert_eq!(
        final_program
            .class(ClassId::new(1))
            .unwrap()
            .conformances
            .len(),
        2
    );

    assert_eq!(program.interfaces.iter().count(), 3);
    for interface in program.interfaces.iter() {
        for (index, requirement) in interface.requirements.iter().enumerate() {
            assert_eq!(requirement.id.interface(), interface.id);
            assert_eq!(requirement.id.index(), index);
        }
    }

    let both = program.class(ClassId::new(1)).unwrap();
    assert_eq!(both.conformances.len(), 2);
    assert_ne!(
        both.conformances[0].interface,
        both.conformances[1].interface
    );
    assert!(both.conformances.iter().all(|conformance| {
        conformance.implementations[0].method == MethodId::new(ClassId::new(1), 0)
    }));

    let dump = dump_mir(&program);
    for expected in [
        "Interface i0",
        "Interface i1",
        "Interface i2",
        "i0:requirement0 -> c1:method0",
        "i1:requirement0 -> c1:method0",
        "call interface i0 i0:requirement0",
        "call interface i1 i1:requirement0",
        "shared interface i0",
        "shared-release",
    ] {
        assert!(dump.contains(expected), "missing `{expected}` in:\n{dump}");
    }

    // HIR and MIR intentionally have no identity or type variant capable of
    // carrying any of these pre-closure concepts. Keep dumps equally strict
    // so new lower-IR provenance cannot silently weaken that boundary.
    for forbidden in [
        "interface-template",
        "template-requirement",
        "type-parameter",
        "structural-interface-application",
    ] {
        assert!(
            !dump.contains(forbidden),
            "forbidden `{forbidden}` in:\n{dump}"
        );
    }
}

#[test]
fn rejects_corrupt_closed_generic_interface_identities() {
    let mut undeclared = lower_text(generic_interface_lowering_source());
    undeclared.classes.entries_mut_for_test()[1].conformances[0].interface = InterfaceId::new(99);
    assert!(messages(&undeclared)
        .iter()
        .any(|message| message == "class c1 conforms to undeclared interface i99"));

    let mut cross_application = lower_text(generic_interface_lowering_source());
    let second_requirement = cross_application
        .interfaces
        .get(InterfaceId::new(1))
        .unwrap()
        .requirements[0]
        .id;
    cross_application.classes.entries_mut_for_test()[1].conformances[0].implementations[0]
        .requirement = second_requirement;
    assert!(messages(&cross_application).iter().any(|message| {
        message.contains("implementation 0 names i1:requirement0 instead of i0:requirement0")
    }));

    let mut undeclared_call = lower_text(generic_interface_lowering_source());
    let call = first_interface_call_mut(&mut undeclared_call);
    let MirCallTarget::Interface(target) = &mut call.target else {
        unreachable!()
    };
    target.interface = InterfaceId::new(99);
    assert!(messages(&undeclared_call)
        .iter()
        .any(|message| message == "interface call requirement belongs to a different interface"));
}

fn generic_interface_lowering_source() -> &'static str {
    concat!(
        "interface Named<T> { fn name() -> i64; }\n",
        "interface Factory<T> { fn make() -> T; }\n",
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Both implements Named<i64>, Named<u64> {\n",
        "  init() {}\n",
        "  fn name() -> i64 { return 7; }\n",
        "}\n",
        "class Maker implements Factory<shared Named<i64>> {\n",
        "  init() {}\n",
        "  fn make() -> shared Named<i64> { return new Both(); }\n",
        "}\n",
        "class Reader<Source> where Source: Named<u64> {\n",
        "  init() {}\n",
        "  fn read(ref source: Source) -> i64 { return source.name(); }\n",
        "}\n",
        "fn read_i64(ref value: Named<i64>) -> i64 { return value.name(); }\n",
        "fn read_u64(ref value: Named<u64>) -> i64 { return value.name(); }\n",
        "fn produced(ref factory: Factory<shared Named<i64>>) -> i64 {\n",
        "  return factory.make()->name();\n",
        "}\n",
        "fn use_reader(ref value: Reader<Both>) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var value: Both = Both();\n",
        "  var maker: Maker = Maker();\n",
        "  return read_i64(value) + read_u64(value) + produced(maker);\n",
        "}\n",
    )
}

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
        build.storage[destination.base.storage().index()].kind,
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

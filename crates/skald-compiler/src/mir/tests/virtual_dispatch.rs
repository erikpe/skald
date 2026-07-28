use super::{object_fixtures::messages, virtual_fixtures::*, *};
use crate::identity::{CallableId, VirtualFamilyId, VirtualSlotId};

#[test]
fn lowers_virtual_families_calls_and_forwarded_receivers_explicitly() {
    let (program, ids) = virtual_dispatch_mir();
    verify_mir(&program).unwrap();

    let family = program.virtual_family(ids.family).unwrap();
    assert_eq!(family.slot, ids.slot);
    assert_eq!(family.root, ids.root_method);
    assert_eq!(
        family.members,
        [ids.root_method, ids.middle_method, ids.leaf_method]
    );

    let call = first_virtual_call(&program);
    assert_eq!(
        call.target,
        MirCallTarget::Method(MirMethodCallTarget::Virtual {
            family: ids.family,
            slot: ids.slot,
            selected: ids.root_method,
        })
    );
    let receiver = method_receiver(call);
    assert_eq!(receiver.place.base.storage(), receiver.origin_carrier());
    assert!(matches!(
        receiver.origin.as_ref(),
        MirObjectOrigin::Forwarded {
            static_target: MirViewTarget::Class(class),
            dispatch_limit: None,
            ..
        } if *class == ids.root
    ));

    let through = program.definitions.get(ids.through_root).unwrap();
    let calls: Vec<_> = through.body.blocks[0]
        .instructions
        .iter()
        .filter_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call.target),
            _ => None,
        })
        .collect();
    assert_eq!(
        calls,
        [
            MirCallTarget::Direct(ids.mark),
            MirCallTarget::Method(MirMethodCallTarget::Virtual {
                family: ids.family,
                slot: ids.slot,
                selected: ids.root_method,
            }),
        ]
    );
}

#[test]
fn direct_method_forwarding_retains_dynamic_origin() {
    let (program, ids) = virtual_dispatch_mir();
    let root = program.class(ids.root).unwrap();
    assert_eq!(root.methods[1].id, ids.relay);

    let relay = program
        .member_definition(CallableId::Method(ids.relay))
        .unwrap();
    let call = relay.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    assert!(matches!(
        call.target,
        MirCallTarget::Method(MirMethodCallTarget::Virtual {
            selected,
            ..
        }) if selected == ids.root_method
    ));
    assert!(matches!(
        method_receiver(call).origin.as_ref(),
        MirObjectOrigin::Forwarded { carrier, .. }
            if Some(*carrier) == relay.receiver
    ));
}

#[test]
fn virtual_mir_dump_is_deterministic_and_target_independent() {
    let (program, ids) = virtual_dispatch_mir();
    let dump = dump_mir(&program);
    assert_eq!(dump, dump_mir(&program));
    assert!(dump
        .contains("Family vf0 slot vs0 root c0:method0 members c0:method0 c1:method0 c2:method0"));
    assert!(dump.contains(
        "call virtual vf0 slot vs0 selected c0:method0 on indirect(f1:s0) origin forwarded(f1:s0 : class c0 readonly)"
    ));
    assert!(!dump.contains("offset"));
    assert!(!dump.contains("register"));
    assert_eq!(ids.forward, FunctionId::new(2));
}

#[test]
fn verified_virtual_mir_reaches_indirect_backend_lowering() {
    let (program, _) = virtual_dispatch_mir();
    verify_mir(&program).unwrap();
    let assembly =
        crate::backend::emit_assembly(crate::backend::Target::X86_64SysV, &program).unwrap();
    assert!(assembly.contains(".Lska_class_0_dispatch:"));
    assert!(assembly.contains("call r11"));
}

#[test]
fn rejects_corrupt_family_slot_membership_and_signature_metadata() {
    let (mut wrong_slot, _) = virtual_dispatch_mir();
    wrong_slot.virtual_families.entries_mut_for_test()[0].slot = VirtualSlotId::new(1);
    assert!(messages(&wrong_slot)
        .iter()
        .any(|message| message.contains("non-canonical slot")));

    let (mut wrong_member, ids) = virtual_dispatch_mir();
    first_virtual_call_mut(&mut wrong_member).target =
        MirCallTarget::Method(MirMethodCallTarget::Virtual {
            family: ids.family,
            slot: ids.slot,
            selected: ids.relay,
        });
    assert!(messages(&wrong_member)
        .iter()
        .any(|message| message == "virtual call selected method is not a member of its family"));

    let (mut missing_family, _) = virtual_dispatch_mir();
    let call = first_virtual_call_mut(&mut missing_family);
    let MirCallTarget::Method(MirMethodCallTarget::Virtual { family, .. }) = &mut call.target
    else {
        unreachable!()
    };
    *family = VirtualFamilyId::new(1);
    assert!(messages(&missing_family)
        .iter()
        .any(|message| message.contains("virtual call family vf1 is not declared")));

    let (mut wrong_signature, ids) = virtual_dispatch_mir();
    wrong_signature.classes.entries_mut_for_test()[ids.middle.index()].methods[0].return_type =
        MirType::Bool;
    assert!(messages(&wrong_signature)
        .iter()
        .any(|message| message.contains("different signature or receiver access")));
}

#[test]
fn rejects_corrupt_receiver_carriers_access_and_dispatch_selection() {
    let (mut wrong_carrier, ids) = virtual_dispatch_mir();
    let call = first_virtual_call_mut(&mut wrong_carrier);
    let MirObjectOrigin::Forwarded { carrier, .. } = method_receiver_mut(call).origin.as_mut()
    else {
        unreachable!()
    };
    *carrier = StorageId::new(CallableId::Method(ids.relay), 2);
    assert!(messages(&wrong_carrier)
        .iter()
        .any(|message| message.contains("static place does not come from its forwarded carrier")));

    let (mut wrong_access, _) = virtual_dispatch_mir();
    let call = first_virtual_call_mut(&mut wrong_access);
    let MirObjectOrigin::Forwarded { access, .. } = method_receiver_mut(call).origin.as_mut()
    else {
        unreachable!()
    };
    *access = MirAliasAccess::Mutable;
    assert!(messages(&wrong_access)
        .iter()
        .any(|message| message.contains("forwarded origin access is inconsistent")));

    let (mut wrong_selection, ids) = virtual_dispatch_mir();
    let call = first_virtual_call_mut(&mut wrong_selection);
    call.target = MirCallTarget::Method(MirMethodCallTarget::Virtual {
        family: ids.family,
        slot: ids.slot,
        selected: ids.middle_method,
    });
    assert!(messages(&wrong_selection).iter().any(|message| message
        == "virtual call selected method does not match the receiver's static class"));
}

#[test]
fn rejects_dead_exact_virtual_receiver_origins() {
    let (mut program, ids) = virtual_dispatch_mir();
    let function = program.definitions.get_mut_for_test(ids.forward).unwrap();
    let storage = StorageId::new(ids.forward, function.storage.len());
    function.storage.push(MirStorage {
        id: storage,
        source: Some(BindingId::Local(LocalId::new(ids.forward, 0))),
        name: "dead".to_owned(),
        kind: MirStorageKind::Local,
        ty: MirType::Class(ids.root),
        span: function.span,
    });
    let result = ValueId::new(ids.forward, function.values.len());
    function.values.push(MirValue {
        id: result,
        ty: MirType::I64,
        span: function.span,
    });
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(ids.relay)),
            receiver: Some(MirMethodReceiver::exact(MirPlace::base(storage), ids.root).into()),
            arguments: vec![
                MirArgument::Value(ValueId::new(ids.forward, 0)),
                MirArgument::View(MirObjectView {
                    source: MirPlace::alias_parameter(function.parameters[1]),
                    origin: Box::new(MirObjectOrigin::Forwarded {
                        carrier: function.parameters[1],
                        static_target: MirViewTarget::Class(ids.root),
                        access: MirAliasAccess::ReadOnly,
                        dispatch_limit: None,
                        span: function.span,
                    }),
                    target: MirViewTarget::Class(ids.root),
                    access: MirAliasAccess::ReadOnly,
                    span: function.span,
                }),
            ],
            result: Some(result),
            shared_result: None,
            destination: None,
            span: function.span,
        }));
    let errors = messages(&program);
    assert!(errors
        .iter()
        .any(|message| message == "method receiver is not live"));
    assert!(errors
        .iter()
        .any(|message| message == "method receiver origin is not live"));
}

#[test]
fn rejects_corrupt_exact_selection_and_base_subobject_origins() {
    let source = concat!(
        "class Root { init() {} virtual fn read() -> i64 { return 1; } }\n",
        "class Leaf extends Root {\n",
        "  init() { super(); }\n",
        "  override fn read() -> i64 { return 2; }\n",
        "}\n",
        "fn main() -> i64 { var value: Leaf = Leaf(); return value.read(); }\n",
    );
    let mut wrong_selection = lower_text(source);
    let call = wrong_selection
        .definitions
        .get_mut_for_test(wrong_selection.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    call.target = MirCallTarget::Method(MirMethodCallTarget::Direct(MethodId::new(
        ClassId::new(0),
        0,
    )));
    let projected = method_receiver(call)
        .place
        .clone()
        .project_base(ClassId::new(0));
    method_receiver_mut(call).place = projected;
    assert!(messages(&wrong_selection).iter().any(|message| {
        message
            == "direct virtual-family call selected method does not match the exact or dispatch-limited class"
    }));

    let mut base_origin = lower_text(source);
    let call = base_origin
        .definitions
        .get_mut_for_test(base_origin.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .unwrap();
    let projected = method_receiver(call)
        .place
        .clone()
        .project_base(ClassId::new(0));
    method_receiver_mut(call).place = projected.clone();
    *method_receiver_mut(call).origin = MirObjectOrigin::Exact {
        complete: projected,
        dynamic_class: ClassId::new(0),
    };
    assert!(messages(&base_origin)
        .iter()
        .any(|message| message == "method receiver exact origin does not name a complete object"));
}

trait ReceiverOriginCarrier {
    fn origin_carrier(&self) -> StorageId;
}

fn method_receiver(call: &MirCall) -> &MirMethodReceiver {
    call.receiver
        .as_ref()
        .and_then(MirCallReceiver::as_method)
        .expect("fixture method call must have a method receiver")
}

fn method_receiver_mut(call: &mut MirCall) -> &mut MirMethodReceiver {
    call.receiver
        .as_mut()
        .and_then(MirCallReceiver::as_method_mut)
        .expect("fixture method call must have a method receiver")
}

impl ReceiverOriginCarrier for MirMethodReceiver {
    fn origin_carrier(&self) -> StorageId {
        match self.origin.as_ref() {
            MirObjectOrigin::Forwarded { carrier, .. } => *carrier,
            MirObjectOrigin::Exact { complete, .. } => complete.base.storage(),
            MirObjectOrigin::Shared { owner, .. } => *owner,
        }
    }
}

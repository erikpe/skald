use super::{alias_fixtures::*, object_fixtures::*, *};
use crate::passes::run_mir_pipeline;

#[test]
fn verifies_direct_member_initializer_forwarding_overlap_and_mixed_alias_arguments() {
    let (program, _) = alias_mir();

    assert!(verify_mir(&program).is_ok());
    let expected = program.clone();
    assert_eq!(run_mir_pipeline(program).unwrap(), expected);
}

#[test]
fn dump_exposes_modes_indirect_bases_and_ordered_argument_kinds() {
    let (program, ids) = alias_mir();
    let dump = dump_mir(&program);

    assert!(dump.contains(&format!(
        "Declaration {} module m0 \"forward\" internal",
        ids.forward
    )));
    assert!(dump.contains(&format!(
        "Signature (ref class {}, mut ref class {}, i64) -> unit",
        ids.class, ids.class
    )));
    assert!(dump.contains("ref-parameter"));
    assert!(dump.contains("mut-ref-parameter"));
    assert!(dump.contains("place(indirect(f3:s0))"));
    assert!(dump.contains("value(f3:v0)"));
    assert_eq!(dump, dump_mir(&program));
    assert!(!dump.contains("offset"));
    assert!(!dump.contains("register"));
}

#[test]
fn rejects_parameter_mode_storage_and_external_signature_corruption() {
    let (mut wrong_storage, ids) = alias_mir();
    wrong_storage
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .storage[0]
        .kind = MirStorageKind::Parameter;
    assert!(messages(&wrong_storage)
        .iter()
        .any(|message| message.contains("storage mode differs from declaration")));

    let (mut primitive_alias, ids) = alias_mir();
    primitive_alias.declarations.entries_mut_for_test()[ids.observe.index()].parameters[0].ty =
        MirType::I64;
    primitive_alias
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .storage[0]
        .ty = MirType::I64;
    assert!(messages(&primitive_alias).iter().any(|message| {
        message.contains("alias parameter 0 must have object-view or inline-optional type")
    }));

    let (mut unlisted, ids) = alias_mir();
    unlisted
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap()
        .parameters
        .remove(0);
    let errors = messages(&unlisted);
    assert!(errors
        .iter()
        .any(|message| message.contains("definition has 1 parameters but declaration requires 2")));
    assert!(errors
        .iter()
        .any(|message| message.contains("is not listed by the definition")));

    let (mut external, ids) = alias_mir();
    let declaration = &mut external.declarations.entries_mut_for_test()[ids.observe.index()];
    external.external_links =
        crate::external::ExternalLinkTable::new(vec![crate::external::ExternalLink {
            id: crate::identity::ExternalLinkId::new(0),
            symbol: declaration.name.clone(),
            declarations: vec![declaration.id],
        }]);
    declaration.linkage = MirFunctionLinkage::External {
        link: crate::identity::ExternalLinkId::new(0),
    };
    external.definitions.remove_for_test(ids.observe);
    assert!(messages(&external).iter().any(|message| message.contains(
        "external function cannot declare alias, object value, or shared-owner parameters"
    )));
}

#[test]
fn rejects_argument_kind_type_ownership_and_access_corruption() {
    let (mut kind, ids) = alias_mir();
    let entry = kind.entry_function;
    let call = direct_call_mut(&mut kind, ids.observe);
    call.arguments[0] = MirArgument::Value(ValueId::new(entry, 0));
    assert!(messages(&kind)
        .iter()
        .any(|message| message.contains("call argument 0 must be a place")));

    let (mut ty, ids) = alias_mir();
    let entry = ty.entry_function;
    let call = direct_call_mut(&mut ty, ids.observe);
    call.arguments[0] = MirArgument::Place(
        MirPlace::base(StorageId::new(entry, 0)).project_field(FieldId::new(ids.class, 0)),
    );
    assert!(messages(&ty)
        .iter()
        .any(|message| message.contains("call argument 0 type mismatch")));

    let (mut foreign, ids) = alias_mir();
    let call = direct_call_mut(&mut foreign, ids.observe);
    call.arguments[0] = MirArgument::Place(MirPlace::base(StorageId::new(ids.mutate, 0)));
    assert!(messages(&foreign)
        .iter()
        .any(|message| message.contains("is not declared in this function")));

    let (mut access, ids) = alias_mir();
    let function = access.definitions.get_mut_for_test(ids.forward).unwrap();
    let call = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(ids.mutate) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap();
    call.arguments[0] = MirArgument::Place(MirPlace::alias_parameter(function.parameters[0]));
    assert!(messages(&access)
        .iter()
        .any(|message| message.contains("call argument 0 requires mutable access")));
}

#[test]
fn rejects_direct_alias_homes_readonly_writes_and_mutable_receiver_calls() {
    let (mut direct, ids) = alias_mir();
    let function = direct.definitions.get_mut_for_test(ids.observe).unwrap();
    let MirInstruction::Assign(load) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected alias load");
    };
    let MirRvalueKind::Load(place) = &mut load.rvalue.kind else {
        panic!("expected place load");
    };
    place.base = MirPlaceBase::Storage(function.parameters[0]);
    assert!(messages(&direct)
        .iter()
        .any(|message| message.contains("requires an indirect base")));

    let (mut readonly_store, ids) = alias_mir();
    let function = readonly_store
        .definitions
        .get_mut_for_test(ids.observe)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Store(MirStore {
            destination: MirPlace::alias_parameter(function.parameters[0])
                .project_field(FieldId::new(ids.class, 0)),
            value: ValueId::new(ids.observe, 0),
            span: function.span,
        }));
    assert!(messages(&readonly_store)
        .iter()
        .any(|message| message.contains("store destination requires mutable access")));

    let (mut replacement, ids) = alias_mir();
    let function = replacement
        .definitions
        .get_mut_for_test(ids.mutate)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Initialize(MirInitialize {
            destination: MirPlace::alias_parameter(function.parameters[0]),
            target: ids.initializer,
            arguments: vec![MirArgument::Place(MirPlace::alias_parameter(
                function.parameters[0],
            ))],
            span: function.span,
        }));
    assert!(messages(&replacement)
        .iter()
        .any(|message| message.contains("initializer destination must be owning storage")));

    let (mut receiver, ids) = alias_mir();
    receiver.classes.entries_mut_for_test()[ids.class.index()].methods[ids.method.index()].kind =
        MirMethodKind::instance(MirReceiverAccess::Mutable);
    let function = receiver.definitions.get_mut_for_test(ids.forward).unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Call(MirCall {
            target: MirCallTarget::Method(MirMethodCallTarget::Direct(ids.method)),
            receiver: Some(
                MirMethodReceiver {
                    place: MirPlace::alias_parameter(function.parameters[0]),
                    origin: Box::new(MirObjectOrigin::Forwarded {
                        carrier: function.parameters[0],
                        static_target: MirViewTarget::Class(ids.class),
                        access: MirAliasAccess::ReadOnly,
                        dispatch_limit: None,
                        span: function.span,
                    }),
                }
                .into(),
            ),
            arguments: vec![
                MirArgument::Place(MirPlace::alias_parameter(function.parameters[0])),
                MirArgument::Place(MirPlace::alias_parameter(function.parameters[1])),
                MirArgument::Value(ValueId::new(ids.forward, 0)),
            ],
            result: None,
            shared_result: None,
            destination: None,
            span: function.span,
        }));
    assert!(messages(&receiver)
        .iter()
        .any(|message| message.contains("mutable method receiver requires mutable access")));
}

fn direct_call_mut(program: &mut MirProgram, target: FunctionId) -> &mut MirCall {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .unwrap()
        .body
        .blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(target) => {
                Some(call)
            }
            _ => None,
        })
        .unwrap()
}

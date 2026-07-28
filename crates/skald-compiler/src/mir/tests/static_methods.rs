use super::*;
use crate::test_support::lower_internal_static_scalar_call_to_mir;

const STATIC_SCALAR_SOURCE: &str = concat!(
    "class Math {\n",
    "  init() {}\n",
    "  private fn sum(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64) -> i64 {\n",
    "    return a + b + c + d + e + f + g;\n",
    "  }\n",
    "}\n",
    "fn proxy(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64) -> i64 {\n",
    "  return 99;\n",
    "}\n",
    "fn main() -> i64 { return proxy(1, 2, 3, 4, 5, 6, 7); }\n",
);

pub(super) fn static_scalar_program() -> MirProgram {
    lower_internal_static_scalar_call_to_mir(STATIC_SCALAR_SOURCE)
}

#[test]
fn lowers_private_ready_receiverless_static_methods_and_calls() {
    let program = static_scalar_program();
    verify_mir(&program).unwrap();

    let method = MethodId::new(ClassId::new(0), 0);
    assert_eq!(program.method(method).unwrap().kind, MirMethodKind::Static);
    let definition = program.member_definition(method.into()).unwrap();
    assert_eq!(definition.class_owner, ClassId::new(0));
    assert_eq!(definition.receiver, None);
    assert!(definition
        .storage
        .iter()
        .all(|storage| storage.kind != MirStorageKind::Receiver));

    let entry = program.definitions.get(program.entry_function).unwrap();
    let call = entry.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .expect("entry function must contain the static call");
    assert_eq!(call.target, MirCallTarget::Static(method));
    assert_eq!(call.receiver, None);
    assert_eq!(call.arguments.len(), 7);

    let dump = dump_mir(&program);
    assert!(dump.contains("Method c0:method0 \"sum\" static"));
    assert!(dump.contains("call static c0:method0"));
    assert!(!dump.contains("Receiver c0:method0"));
}

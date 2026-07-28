use super::*;
const STATIC_SCALAR_SOURCE: &str = concat!(
    "class Math {\n",
    "  init() {}\n",
    "  private static fn sum(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64, g: i64) -> i64 {\n",
    "    return a + b + c + d + e + f + g;\n",
    "  }\n",
    "  static fn calculate() -> i64 { return Math.sum(1, 2, 3, 4, 5, 6, 7); }\n",
    "}\n",
    "fn main() -> i64 { return Math.calculate(); }\n",
);

pub(super) fn static_scalar_program() -> MirProgram {
    lower_text(STATIC_SCALAR_SOURCE)
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

    let calculate = program
        .member_definition(MethodId::new(ClassId::new(0), 1).into())
        .unwrap();
    let call = calculate.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .expect("static method must contain the private static call");
    assert_eq!(call.target, MirCallTarget::Static(method));
    assert_eq!(call.receiver, None);
    assert_eq!(call.arguments.len(), 7);

    let dump = dump_mir(&program);
    assert!(dump.contains("Method c0:method0 \"sum\" static"));
    assert!(dump.contains("call static c0:method0"));
    assert!(!dump.contains("Receiver c0:method0"));
}

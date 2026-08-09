use super::*;

const PRODUCED_ALIAS_SOURCE: &str = concat!(
    "interface Readable { fn read() -> i64; }\n",
    "class Root implements Readable {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  virtual fn read() -> i64 { return self.value; }\n",
    "}\n",
    "class Leaf extends Root {\n",
    "  extra: i64;\n",
    "  init(value: i64, extra: i64) { super(value); self.extra = extra; }\n",
    "  override fn read() -> i64 { return self.value + self.extra; }\n",
    "}\n",
    "fn observe(ref exact: Leaf, ref ancestor: Root, ref readable: Readable, ",
    "ref object: Obj) -> i64 {\n",
    "  if (object is Leaf) {\n",
    "    return exact.read() + ancestor.read() + readable.read() ",
    "+ ((Leaf) object).read();\n",
    "  } else { return 0; }\n",
    "}\n",
    "fn main() -> i64 {\n",
    "  return observe(Leaf(1, 2), Leaf(3, 4), Leaf(5, 6), Leaf(7, 8));\n",
    "}\n",
);

#[test]
fn produced_views_use_the_existing_object_alias_abi_without_runtime_support() {
    let program = lower_text(PRODUCED_ALIAS_SOURCE);
    verify_mir(&program).unwrap();

    let observe = program
        .declarations
        .iter()
        .find(|declaration| declaration.name == "observe")
        .expect("fixture observe function must be declared")
        .id;
    let main = program
        .definitions
        .get(program.entry_function)
        .expect("fixture entry function must be defined");
    let arguments = main
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if call.target == MirCallTarget::Direct(observe) => {
                Some(&call.arguments)
            }
            _ => None,
        })
        .expect("fixture entry function must call observe");
    assert_eq!(arguments.len(), 4);
    assert!(arguments
        .iter()
        .all(|argument| matches!(argument, MirArgument::View(_))));

    let first = emit_assembly(Target::X86_64SysV, &program).unwrap();
    let second = emit_assembly(Target::X86_64SysV, &program).unwrap();

    assert_eq!(first, second);
    assert!(first.contains("call .Lska.fn.main.observe.f0"));
    assert!(first.contains("call r11"));
    assert_eq!(
        first
            .lines()
            .map(str::trim)
            .filter(|line| line.starts_with("call ska_rt_"))
            .collect::<Vec<_>>(),
        ["call ska_rt_panic", "call ska_rt_abi_v9"]
    );
    assert_system_assembler_accepts(&first);
    assert_eq!(run_native_assembly(&first).code(), Some(36));

    let runtime_header = include_str!("../../../../../../runtime/include/skald_runtime.h");
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_VERSION UINT64_C(9)"));
    assert!(runtime_header.contains("#define SKALD_RUNTIME_ABI_MARKER ska_rt_abi_v9"));
}

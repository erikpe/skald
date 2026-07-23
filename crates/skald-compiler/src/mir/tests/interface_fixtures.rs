use super::*;
use crate::identity::{InterfaceId, InterfaceRequirementId};

pub(super) struct InterfaceFixtureIds {
    pub runner: InterfaceId,
    pub requirement: InterfaceRequirementId,
    pub base: ClassId,
    pub worker: ClassId,
    pub base_method: MethodId,
    pub worker_method: MethodId,
    pub invoke: FunctionId,
    pub forward: FunctionId,
    pub erase: FunctionId,
    pub main: FunctionId,
}

pub(super) fn interface_dispatch_mir() -> (MirProgram, InterfaceFixtureIds) {
    let program = lower_text(concat!(
        "interface Runner { fn run(value: u64) -> u64; }\n",
        "interface Other { fn other() -> u64; }\n",
        "class Base implements Runner {\n",
        "  init() {}\n",
        "  virtual fn run(value: u64) -> u64 { return value; }\n",
        "}\n",
        "class Worker extends Base {\n",
        "  init() { super(); }\n",
        "  override fn run(value: u64) -> u64 { return value; }\n",
        "}\n",
        "fn invoke(ref runner: Runner, value: u64) -> u64 {\n",
        "  return runner.run(value);\n",
        "}\n",
        "fn forward(ref runner: Runner, value: u64) -> u64 {\n",
        "  return invoke(runner, value);\n",
        "}\n",
        "fn erase(ref runner: Runner) -> unit { any(runner); }\n",
        "fn any(ref value: Obj) -> unit {}\n",
        "fn main() -> i64 {\n",
        "  var worker: Worker = Worker();\n",
        "  var result: u64 = invoke(worker, 7u);\n",
        "  return 0;\n",
        "}\n",
    ));
    let runner = InterfaceId::new(0);
    (
        program,
        InterfaceFixtureIds {
            runner,
            requirement: InterfaceRequirementId::new(runner, 0),
            base: ClassId::new(0),
            worker: ClassId::new(1),
            base_method: MethodId::new(ClassId::new(0), 0),
            worker_method: MethodId::new(ClassId::new(1), 0),
            invoke: FunctionId::new(0),
            forward: FunctionId::new(1),
            erase: FunctionId::new(2),
            main: FunctionId::new(4),
        },
    )
}

pub(super) fn first_interface_call(program: &MirProgram) -> &MirCall {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Interface(_)) => {
                Some(call)
            }
            _ => None,
        })
        .expect("interface fixture must contain an interface call")
}

pub(super) fn first_interface_call_mut(program: &mut MirProgram) -> &mut MirCall {
    let invoke = FunctionId::new(0);
    program
        .definitions
        .get_mut_for_test(invoke)
        .expect("interface fixture must contain invoke")
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Interface(_)) => {
                Some(call)
            }
            _ => None,
        })
        .expect("interface fixture must contain a mutable interface call")
}

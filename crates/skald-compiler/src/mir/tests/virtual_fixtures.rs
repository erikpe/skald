use super::*;
use crate::identity::{VirtualFamilyId, VirtualSlotId};

pub(super) struct VirtualFixtureIds {
    pub root: ClassId,
    pub middle: ClassId,
    pub family: VirtualFamilyId,
    pub slot: VirtualSlotId,
    pub root_method: MethodId,
    pub middle_method: MethodId,
    pub leaf_method: MethodId,
    pub relay: MethodId,
    pub mark: FunctionId,
    pub through_root: FunctionId,
    pub forward: FunctionId,
}

pub(super) fn virtual_dispatch_mir() -> (MirProgram, VirtualFixtureIds) {
    let program = lower_text(concat!(
        "class Root {\n",
        "  init() {}\n",
        "  virtual fn read(amount: i64, ref other: Root) -> i64 { return amount; }\n",
        "  fn relay(amount: i64, ref other: Root) -> i64 { return self.read(amount, other); }\n",
        "}\n",
        "class Middle extends Root {\n",
        "  init() { super(); }\n",
        "  override fn read(value: i64, ref other: Root) -> i64 { return value; }\n",
        "}\n",
        "class Leaf extends Middle {\n",
        "  init() { super(); }\n",
        "  override fn read(value: i64, ref other: Root) -> i64 {\n",
        "    return self.read(value, other);\n",
        "  }\n",
        "}\n",
        "fn mark(value: i64) -> i64 { return value; }\n",
        "fn through_root(ref value: Root, ref other: Root) -> i64 {\n",
        "  return value.read(mark(1), other);\n",
        "}\n",
        "fn forward(ref value: Root, ref other: Root) -> i64 {\n",
        "  return through_root(value, other);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    (
        program,
        VirtualFixtureIds {
            root: ClassId::new(0),
            middle: ClassId::new(1),
            family: VirtualFamilyId::new(0),
            slot: VirtualSlotId::new(0),
            root_method: MethodId::new(ClassId::new(0), 0),
            relay: MethodId::new(ClassId::new(0), 1),
            middle_method: MethodId::new(ClassId::new(1), 0),
            leaf_method: MethodId::new(ClassId::new(2), 0),
            mark: FunctionId::new(0),
            through_root: FunctionId::new(1),
            forward: FunctionId::new(2),
        },
    )
}

pub(super) fn first_virtual_call(program: &MirProgram) -> &MirCall {
    program
        .executable_definitions()
        .flat_map(|definition| &definition.body().blocks)
        .flat_map(|block| &block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if matches!(
                    call.target,
                    MirCallTarget::Method(MirMethodCallTarget::Virtual { .. })
                ) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("virtual fixture must contain a virtual call")
}

pub(super) fn first_virtual_call_mut(program: &mut MirProgram) -> &mut MirCall {
    let root = program
        .virtual_families
        .iter()
        .next()
        .expect("virtual fixture must contain a family")
        .root;
    let relay = MethodId::new(root.class(), 1);
    program
        .member_definitions
        .get_mut_for_test(relay.into())
        .expect("virtual fixture must contain the relay definition")
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if matches!(
                    call.target,
                    MirCallTarget::Method(MirMethodCallTarget::Virtual { .. })
                ) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("relay fixture must contain a mutable virtual call")
}

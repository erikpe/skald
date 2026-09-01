use super::*;
use crate::{
    identity::{InterfaceId, InterfaceRequirementId},
    passes::run_mir_pipeline,
    test_support::{load_module_sources, CANONICAL_OPS_SOURCE},
};

fn operator_hir() -> crate::hir::HirProgram {
    let source = concat!(
        "from std::ops import OpAdd;\n",
        "class Number implements OpAdd<Number, Number> {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn op_add(ref rhs: Number) -> Number { return Number(self.value + rhs.value); }\n",
        "}\n",
        "class Adder<T> where T: OpAdd<T, T> {\n",
        "  init() {}\n",
        "  fn add(ref left: T, ref right: T) -> T { return left + right; }\n",
        "}\n",
        "fn class_answer() -> i64 {\n",
        "  var adder: Adder<Number> = Adder<Number>();\n",
        "  var left: Number = Number(17);\n",
        "  var right: Number = Number(25);\n",
        "  return adder.add(left, right).value;\n",
        "}\n",
        "fn primitive_answer() -> u64 {\n",
        "  var adder: Adder<u64> = Adder<u64>();\n",
        "  return adder.add(17u, 25u);\n",
        "}\n",
        "fn main() -> i64 { return class_answer() + (i64) primitive_answer() - 42; }\n",
    );
    let (_workspace, graph) = load_module_sources(
        "app",
        &[("app.ska", source), ("std/ops.ska", CANONICAL_OPS_SOURCE)],
    );
    let resolved = crate::resolve::resolve_module_graph(&graph);
    assert!(
        resolved.diagnostics.is_empty(),
        "{:?}",
        resolved.diagnostics
    );
    let checked = crate::typeck::type_check(&resolved.program);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    checked
        .hir
        .expect("valid generic operators must produce HIR")
}

fn verification_messages(program: &MirProgram) -> Vec<String> {
    verify_mir(program)
        .expect_err("mutated operator realization must fail its ordinary MIR owner")
        .iter()
        .map(|error| error.message.clone())
        .collect()
}

fn first_operator_interface_call_mut(program: &mut MirProgram) -> &mut MirCall {
    let owner = program
        .member_definitions
        .iter()
        .find(|definition| {
            definition.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(instruction, MirInstruction::Call(call) if matches!(call.target, MirCallTarget::Interface(_)))
                })
            })
        })
        .map(|definition| definition.callable)
        .expect("class operator realization must contain an interface call");
    program
        .member_definitions
        .get_mut_for_test(owner)
        .unwrap()
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
        .unwrap()
}

#[test]
fn generic_operator_realizations_use_only_existing_verified_mir() {
    let hir = operator_hir();
    let hir_dump = crate::hir::dump_hir(&hir);
    assert!(hir_dump.contains("AddU64"), "{hir_dump}");
    assert!(hir_dump.contains("ObjectCall interface"), "{hir_dump}");
    assert!(!hir_dump.contains("OperatorSelection"), "{hir_dump}");

    let preliminary = lower_preliminary_hir(&hir);
    check_preliminary_mir(&preliminary).unwrap();
    let preliminary_dump = dump_preliminary_mir(&preliminary);
    assert!(preliminary_dump.contains("add.u64"), "{preliminary_dump}");
    assert!(
        preliminary_dump.contains("call interface"),
        "{preliminary_dump}"
    );
    assert!(!preliminary_dump.contains("overloaded-operator"));

    let final_mir = run_mir_pipeline(lower_hir(&hir)).unwrap();
    verify_mir(&final_mir).unwrap();
    let final_dump = dump_mir(&final_mir);
    assert!(final_dump.contains("add.u64"), "{final_dump}");
    assert!(final_dump.contains("call interface"), "{final_dump}");
    assert!(!final_dump.contains("overloaded-operator"));
}

#[test]
fn preliminary_operator_call_corruption_is_rejected_by_interface_verification() {
    let mut preliminary = lower_preliminary_hir(&operator_hir());
    let call = first_operator_interface_call_mut(preliminary.program_mut());
    let MirCallTarget::Interface(target) = &mut call.target else {
        panic!("class operator realization must be an interface call");
    };
    target.requirement = InterfaceRequirementId::new(InterfaceId::new(99), 0);

    let errors = check_preliminary_mir(&preliminary).unwrap_err();
    assert!(
        errors.iter().any(|error| {
            error
                .message
                .contains("interface requirement target i99:requirement0 is not declared")
        }),
        "{errors:?}"
    );
}

#[test]
fn final_operator_intrinsic_and_alias_corruption_use_existing_verifiers() {
    let valid = run_mir_pipeline(lower_hir(&operator_hir()))
        .unwrap()
        .program()
        .clone();

    let mut wrong_intrinsic = valid.clone();
    let intrinsic_owner = wrong_intrinsic
        .member_definitions
        .iter()
        .find(|definition| {
            definition.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Assign(MirAssignment {
                            rvalue: MirRvalue {
                                kind: MirRvalueKind::Binary {
                                    operation: MirBinaryOperation::AddU64,
                                    ..
                                },
                                ..
                            },
                            ..
                        })
                    )
                })
            })
        })
        .map(|definition| definition.callable)
        .unwrap_or_else(|| {
            panic!(
                "primitive specialization has no u64 addition:\n{}",
                dump_mir(&wrong_intrinsic)
            )
        });
    let assignment = wrong_intrinsic
        .member_definitions
        .get_mut_for_test(intrinsic_owner)
        .unwrap()
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(
                    assignment.rvalue.kind,
                    MirRvalueKind::Binary {
                        operation: MirBinaryOperation::AddU64,
                        ..
                    }
                ) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .unwrap();
    let MirRvalueKind::Binary { operation, .. } = &mut assignment.rvalue.kind else {
        unreachable!()
    };
    *operation = MirBinaryOperation::AddI64;
    let messages = verification_messages(&wrong_intrinsic);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("binary operation result type mismatch")),
        "{messages:?}"
    );

    let mut missing_alias_end = valid;
    let (owner, alias) = missing_alias_end
        .definitions
        .iter()
        .find_map(|definition| {
            definition
                .storage
                .iter()
                .find(|storage| storage.kind == MirStorageKind::PrimitiveAlias)
                .map(|storage| (definition.function, storage.id))
        })
        .expect("produced primitive arguments must use ordinary alias storage");
    let definition = missing_alias_end
        .definitions
        .get_mut_for_test(owner)
        .unwrap();
    for block in &mut definition.body.blocks {
        block.instructions.retain(|instruction| {
            !matches!(instruction, MirInstruction::StorageDead(event) if event.storage == alias)
        });
    }
    let messages = verification_messages(&missing_alias_end);
    assert!(
        messages
            .iter()
            .any(|message| message.contains("must have one bounded lifetime")),
        "{messages:?}"
    );
}

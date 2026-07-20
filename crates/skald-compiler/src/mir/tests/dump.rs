use super::*;

#[test]
fn mir_dump_is_deterministic() {
    let mir = lower_text("fn main() -> i64 { return 42; }");

    assert_eq!(
        super::dump_mir(&mir),
        concat!(
            "MirProgram @0..31\n",
            "  Entry f0\n",
            "  Classes\n",
            "  Declarations\n",
            "    Declaration f0 \"main\" internal @0..31\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @0..31\n",
            "      Parameters\n",
            "      Storage\n",
            "      Values\n",
            "        f0:v0 : i64 @26..28\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @17..31\n",
            "          f0:v0 = const.i64 42 : i64 @26..28\n",
            "          return f0:v0 @19..29\n",
        )
    );
}

#[test]
fn control_flow_dump_is_exact_and_deterministic() {
    let mut mir = lower_text("fn main() -> i64 { return 0; }");
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.values[0].ty = MirType::Bool;
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected constant assignment");
    };
    assignment.rvalue = MirRvalue {
        kind: MirRvalueKind::ConstantBool(true),
        ty: MirType::Bool,
    };
    let condition = assignment.result;
    let block = function.body.blocks[0].id;
    function.body.blocks[0].terminator = Some(MirTerminator::Branch {
        condition,
        true_target: block,
        false_target: block,
        span: function.span,
    });

    assert!(verify_mir(&mir).is_ok());
    let expected = concat!(
        "MirProgram @0..30\n",
        "  Entry f0\n",
        "  Classes\n",
        "  Declarations\n",
        "    Declaration f0 \"main\" internal @0..30\n",
        "      Signature () -> i64\n",
        "  Definitions\n",
        "    Definition f0 @0..30\n",
        "      Parameters\n",
        "      Storage\n",
        "      Values\n",
        "        f0:v0 : bool @26..27\n",
        "      EntryBlock f0:b0\n",
        "      Blocks\n",
        "        f0:b0 @17..30\n",
        "          f0:v0 = const.bool true : bool @26..27\n",
        "          branch f0:v0, true f0:b0, false f0:b0 @0..30\n",
    );
    assert_eq!(dump_mir(&mir), expected);
    assert_eq!(dump_mir(&mir), dump_mir(&mir));
}

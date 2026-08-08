use super::*;

fn direct_local_unwrap_program() -> MirProgram {
    lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Other { init() {} }\n",
        "fn main() -> i64 {\n",
        "  var maybe: shared? Item = new Item(21);\n",
        "  var owner: shared Item = maybe!;\n",
        "  var values: (shared? Item)[] = (shared? Item)[]{maybe};\n",
        "  var from_element: shared Item = values[0]!;\n",
        "  var other: shared Other = new Other();\n",
        "  return owner->value + from_element->value;\n",
        "}\n",
    ))
}

#[test]
fn direct_local_shared_unwraps_use_fresh_temporary_owners() {
    let program = direct_local_unwrap_program();
    verify_mir(&program).expect("direct-local optional shared unwraps must verify");
    let function = program.definitions.get(program.entry_function).unwrap();
    let unwraps = function
        .body
        .blocks
        .iter()
        .filter_map(|block| match &block.terminator {
            Some(MirTerminator::OptionalSharedUnwrap {
                unwrap,
                success_target,
                ..
            }) => Some((unwrap, *success_target)),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(unwraps.len(), 2);
    for (unwrap, success_target) in unwraps {
        let temporary = function.storage(unwrap.destination).unwrap();
        assert_eq!(temporary.kind, MirStorageKind::Temporary);
        assert_eq!(temporary.source, None);
        assert_eq!(temporary.ty, MirType::Shared(unwrap.target));

        let transfer = function.body.blocks[success_target.index()]
            .instructions
            .iter()
            .find_map(|instruction| match instruction {
                MirInstruction::SharedMove(transfer) if transfer.source == unwrap.destination => {
                    Some(transfer)
                }
                _ => None,
            })
            .expect("the successful unwrap must move its fresh owner into the local");
        let destination = function.storage(transfer.destination).unwrap();
        assert_eq!(destination.kind, MirStorageKind::Local);
        assert_eq!(destination.ty, temporary.ty);
    }

    let dump = dump_mir(&program);
    assert_eq!(dump.matches("optional-shared-unwrap").count(), 2);
    assert_eq!(dump.matches("shared-move").count(), 2);
}

#[test]
fn verifier_preserves_the_direct_local_unwrap_transfer_protocol() {
    let program = direct_local_unwrap_program();

    let mut direct_destination = program.clone();
    let main = direct_destination
        .definitions
        .get_mut_for_test(direct_destination.entry_function)
        .unwrap();
    let (unwrap_block, success_target) = main
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalSharedUnwrap { success_target, .. }) => {
                Some((block.id, success_target))
            }
            _ => None,
        })
        .unwrap();
    let destination = main.body.blocks[success_target.index()]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedMove(transfer) => Some(transfer.destination),
            _ => None,
        })
        .unwrap();
    match main.body.blocks[unwrap_block.index()]
        .terminator
        .as_mut()
        .unwrap()
    {
        MirTerminator::OptionalSharedUnwrap { unwrap, .. } => {
            unwrap.destination = destination;
        }
        _ => unreachable!(),
    }
    assert!(verify_mir(&direct_destination)
        .unwrap_err()
        .to_string()
        .contains("fresh shared owner"));

    let mut missing_move = program.clone();
    let main = missing_move
        .definitions
        .get_mut_for_test(missing_move.entry_function)
        .unwrap();
    let success_target = main
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalSharedUnwrap { success_target, .. }) => {
                Some(success_target)
            }
            _ => None,
        })
        .unwrap();
    main.body.blocks[success_target.index()]
        .instructions
        .retain(|instruction| !matches!(instruction, MirInstruction::SharedMove(_)));
    assert!(verify_mir(&missing_move)
        .unwrap_err()
        .to_string()
        .contains("shared temporary remains live at full-expression boundary"));

    let mut incompatible_move = program;
    let main = incompatible_move
        .definitions
        .get_mut_for_test(incompatible_move.entry_function)
        .unwrap();
    let other = main
        .storage
        .iter()
        .find(|storage| storage.name == "other")
        .unwrap()
        .id;
    let transfer = main
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::SharedMove(transfer) => Some(transfer),
            _ => None,
        })
        .unwrap();
    transfer.destination = other;
    assert!(verify_mir(&incompatible_move)
        .unwrap_err()
        .to_string()
        .contains("compatible temporary"));
}

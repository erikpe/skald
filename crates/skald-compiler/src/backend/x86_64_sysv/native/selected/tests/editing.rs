use super::*;
#[test]
fn consuming_native_edits_rebuild_every_operand_and_reject_stale_witnesses() {
    for_sources(
        "fn main() -> i64 { var value: i64 = 2; return value + 3; }",
        |context, lower| {
            let body = select(context, lower).unwrap();
            let receipt = body.receipt();
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let mut dump = String::new();
            body.dump(&mut dump).unwrap();
            assert!(dump.contains("x86") && dump.contains("origin=instruction"));
            let mut inventory = selected::SelectedProgramBuilder::new(context);
            inventory.complete(&body, &receipt).unwrap();
            // A consuming rebuild changes every arena identity and remaps opcode
            // uses/definitions, semantic memory regions, edges, and origin maps.
            let mut editor = body.into_editor();
            let mut values = editor.values().collect::<Vec<_>>();
            values.reverse();
            let mut blocks = editor.blocks().collect::<Vec<_>>();
            blocks.reverse();
            let mut objects = editor.objects().collect::<Vec<_>>();
            objects.reverse();
            let (changed, remap) = editor.rebuild(&values, &blocks, &objects).unwrap();
            editor = changed;
            remap.require_source(&receipt).unwrap();
            let fresh = editor.finish(&verifier).unwrap();
            assert!(!receipt.matches(&fresh));
            assert!(inventory.complete(&fresh, &receipt).is_err());
            assert!(fresh.analysis().is_ok());
            let mut after = String::new();
            fresh.dump(&mut after).unwrap();
            assert_ne!(dump, after);
        },
    );
}

#[test]
fn changed_address_cannot_retain_another_objects_memory_effect() {
    for_sources(
        "fn main() -> i64 { var a: i64 = 2; var b: i64 = 3; return a + b; }",
        |context, lower| {
            let body = select(context, lower).unwrap();
            let mut addresses = vec![];
            let mut load = None;
            body.visit(|fact| {
                if let SelectedFact::Instruction {
                    block,
                    ordinal,
                    payload,
                } = fact
                {
                    match payload.opcode {
                        Opcode::ObjectAddress { object, out } => {
                            addresses.push((object, out.value))
                        }
                        Opcode::Load {
                            region: crate::backend::effects::MemoryRegion::Object(object),
                            ..
                        } if load.is_none() => load = Some((block, ordinal, object)),
                        _ => {}
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
            let (block, ordinal, object) = load.unwrap();
            let other = addresses.iter().find(|(id, _)| *id != object).unwrap().1;
            let verifier = Verifier::new(lower.receipt().owner().context().profile()).unwrap();
            let mut editor = body.into_editor();
            editor
                .replace_operand(
                    editor.block(block).unwrap(),
                    ordinal,
                    0,
                    editor.value(other).unwrap(),
                )
                .unwrap();
            match editor.finish(&verifier) {
                Err(selected::SelectedEditFailure::Verify(failures)) => {
                    assert!(failures.iter().any(|failure| matches!(
                        failure.reason,
                        selected::SelectedReason::Target(
                            "native object memory access is out of bounds or misaligned"
                        )
                    )))
                }
                _ => panic!("changed concrete address must fail independent native checking"),
            }
        },
    );
}

use super::*;
use crate::backend::graph::EditError;

// Concrete opcode owner controls rewriting. No shared opcode enum or stored operand list.
impl EditablePayload for Node {
    fn replace_use(&mut self, slot: usize, to: SelectedValueId) -> Result<(), EditError> {
        let mut ordinal = 0;
        let mut found = false;
        self.uses(&mut |v| {
            if ordinal == slot {
                v.id = to;
                found = true;
            }
            ordinal += 1;
            Ok(())
        })?;
        if found {
            Ok(())
        } else {
            Err(EditError::InvalidLocation)
        }
    }
    fn remap(&mut self, r: &SelectedRemap<'_>) -> Result<(), EditError> {
        self.uses(&mut |v| {
            v.id = r.values.get(v.id)?;
            Ok(())
        })?;
        if self.skip_definition_remap {
            return Ok(());
        }
        match &mut self.op {
            Op::Address { out, .. } | Op::Load { out, .. } | Op::Add { out, .. } => {
                out.id = r.values.get(out.id)?
            }
            Op::Divide {
                quotient,
                remainder,
                ..
            } => {
                quotient.id = r.values.get(quotient.id)?;
                remainder.id = r.values.get(remainder.id)?;
            }
            Op::Call { out, .. } => {
                for v in out {
                    v.id = r.values.get(v.id)?;
                }
            }
            Op::Branch { .. } | Op::Jump | Op::Return { .. } | Op::Trap | Op::Trace => {}
        }
        self.effects = self.effects.try_map_objects(|id| r.objects.get(id))?;
        Ok(())
    }
    fn relocate_split(
        &mut self,
        _from: SelectedBlockId,
        _to: SelectedBlockId,
        _at: usize,
    ) -> Result<(), EditError> {
        // These opcodes have no instruction-indexed trace sites or block-bound guard tokens.
        // Division protection is rediscovered from the explicit CFG by WitnessTarget.
        Ok(())
    }
}
impl Node {
    fn uses(
        &mut self,
        f: &mut impl FnMut(&mut ValueRef) -> Result<(), EditError>,
    ) -> Result<(), EditError> {
        match &mut self.op {
            Op::Load { address, .. } => f(address)?,
            Op::Divide {
                numerator, divisor, ..
            } => {
                f(numerator)?;
                f(divisor)?;
            }
            Op::Add { a, b, .. } => {
                f(a)?;
                f(b)?;
            }
            Op::Call { target, args, .. } => {
                for v in args {
                    f(v)?;
                }
                if let Some(v) = target {
                    f(v)?;
                }
            }
            Op::Branch { condition, .. } => f(condition)?,
            Op::Return { values, .. } => {
                for v in values {
                    f(v)?;
                }
            }
            Op::Address { .. } | Op::Jump | Op::Trap | Op::Trace => {}
        }
        Ok(())
    }
}
fn target(ctx: &SelectionContext<'_>, shape: Shape) -> WitnessTarget {
    WitnessTarget {
        profile: ctx.catalog.plan().profile(),
        shape,
        reject: false,
    }
}
#[test]
fn selected_split_rebuild_and_program_replacement_use_fresh_receipts() {
    for shape in [Shape::Two, Shape::Three] {
        let p = CheckedPlan::check(supplied(shape)).unwrap();
        let (program, bodies) = inventory(&p);
        let extension = lir::TargetDeclarations::new(program.parent())
            .freeze()
            .unwrap();
        let (r, views, _) = resources();
        let ctx = context(&extension, r, vec![repr(); 2]);
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let sum = b.value(repr(), Some(origin())).unwrap();
        b.append(
            entry,
            Node::new(
                Op::Add {
                    a: vf(args[0], repr()),
                    b: vf(args[1], repr()),
                    out: vf(sum, repr()),
                    destructive: shape == Shape::Two,
                },
                &views,
            ),
        )
        .unwrap();
        ret(&mut b, entry, vec![], vec![], &views);
        let body = checked(b, shape);
        let old = body.receipt();
        let analysis = body.analysis().unwrap();
        assert!(std::ptr::eq(analysis.owner(), body.draft()));
        assert_eq!(analysis.reachable(0), Some(true));
        drop(analysis);
        let mut complete = SelectedProgramBuilder::new(&ctx);
        complete.complete(&body, &old).unwrap();
        let (b, _, _) = begin(&ctx, &bodies[1]);
        let result_binding = b.finish().abi().unwrap().results().to_vec();
        let (mut b, e, a) = begin(&ctx, &bodies[1]);
        ret(&mut b, e, vec![vf(a[0], repr())], result_binding, &views);
        let callee = checked(b, shape);
        complete.complete(&callee, &callee.receipt()).unwrap();
        let (mut complete, mut edit) = complete.finish(&program).unwrap().edit(body).unwrap();
        let entry = edit.block(entry.id()).unwrap();
        let replacement = edit.value(args[1].id()).unwrap();
        edit.replace_operand(entry, 0, 0, replacement).unwrap();
        let next = edit
            .split_block(entry, 0, Node::new(Op::Jump, &views))
            .unwrap();
        let mut values = edit.values().collect::<Vec<_>>();
        values.reverse();
        let blocks = vec![next, entry];
        let edit_objects = edit.objects().collect::<Vec<_>>();
        assert_eq!(edit.blocks().len(), 2);
        let (edit, remap): (SelectedEditor<'_, Node>, SelectedRemap<'_>) =
            edit.rebuild(&values, &blocks, &edit_objects).unwrap();
        remap.require_source(&old).unwrap();
        assert_eq!(remap.blocks.get(next.id()).unwrap().index(), 0);
        let edited = edit.finish(&target(&ctx, shape)).unwrap();
        assert_eq!(
            edited
                .draft()
                .values
                .get_id(remap.values.get(sum.id()).unwrap())
                .unwrap()
                .origin,
            Some(origin())
        );
        assert!(!old.matches(&edited));
        assert!(remap.require_source(&edited.receipt()).is_err());
        assert!(edited
            .receipt()
            .input()
            .unwrap()
            .same_snapshot(&bodies[0].receipt()));
        assert_eq!(
            complete.complete(&edited, &old),
            Err(lir::ProgramError::StaleReceipt)
        );
        complete.complete(&edited, &edited.receipt()).unwrap();
        let complete = complete.finish(&program).unwrap();
        assert_eq!(complete.receipts().len(), 2);
        complete.require_input(&edited.receipt()).unwrap();
        assert_eq!(
            complete.require_input(&old),
            Err(lir::ProgramError::StaleReceipt)
        );
    }
}

#[test]
fn selected_swaps_and_malformed_target_remaps_are_reverified() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    for bad in 0..4 {
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let sum = b.value(repr(), None).unwrap();
        let mut add = Node::new(
            Op::Add {
                a: vf(args[0], repr()),
                b: vf(args[1], repr()),
                out: vf(sum, repr()),
                destructive: true,
            },
            &views,
        );
        // Private test fault injection: omit definition rewriting in the target callback.
        if bad == 2 {
            add.skip_definition_remap = true;
        }
        b.append(entry, add).unwrap();
        let x = b.value(repr(), None).unwrap();
        let y = b.value(repr(), None).unwrap();
        let exit = b.block(&[x, y], None).unwrap();
        b.terminate(entry, Node::new(Op::Jump, &views), &[(exit, args.clone())])
            .unwrap();
        ret(&mut b, exit, vec![], vec![], &views);
        let body = checked(b, Shape::Two);
        let mut edit = body.into_editor();
        let entry = edit.block(entry.id()).unwrap();
        let exit = edit.block(exit.id()).unwrap();
        let a = edit.value(args[0].id()).unwrap();
        let b = edit.value(args[1].id()).unwrap();
        if bad == 3 {
            let values = edit
                .draft()
                .values
                .handles()
                .filter(|v| v.id() != a.id())
                .collect::<Vec<_>>();
            assert!(matches!(
                edit.rebuild(&values, &[entry, exit], &[]),
                Err(EditError::MissingId)
            ));
            continue;
        }
        if bad == 2 {
            let mut values = edit.values().collect::<Vec<_>>();
            values.rotate_left(1);
            assert!(matches!(
                edit.rebuild(&values, &[entry, exit], &[]),
                Err(EditError::DefinitionConflict)
            ));
            continue;
        }
        let swapped = [b, a];
        edit.redirect_edge(
            entry,
            0,
            exit,
            if bad == 1 {
                std::slice::from_ref(&b)
            } else {
                &swapped
            },
        )
        .unwrap();
        match edit.finish(&target(&ctx, Shape::Two)) {
            Ok(body) => {
                assert_eq!(bad, 0);
                let term = body
                    .draft()
                    .blocks
                    .get(entry)
                    .unwrap()
                    .terminal
                    .as_ref()
                    .unwrap();
                assert_eq!(term.edges[0].1, [b.id(), a.id()]);
            }
            Err(SelectedEditFailure::Verify(errors)) => assert!(
                bad == 1
                    && errors
                        .iter()
                        .any(|e| matches!(e.reason, SelectedReason::Graph(_)))
            ),
            Err(SelectedEditFailure::Edit(error)) => panic!("unexpected edit error {error:?}"),
        }
    }
}

#[test]
fn correction_graph_rebuilds_and_divisor_substitution_rechecks_target_guard() {
    let p = CheckedPlan::check(supplied(Shape::Two)).unwrap();
    let (program, bodies) = inventory(&p);
    let extension = lir::TargetDeclarations::new(program.parent())
        .freeze()
        .unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    for broken in 0..3 {
        let (mut b, entry, args) = begin(&ctx, &bodies[0]);
        let success = b.block(&[], Some(origin())).unwrap();
        let failure = b.block(&[], None).unwrap();
        let correction = b.block(&[], Some(origin())).unwrap();
        let quotient = b.value(repr(), Some(origin())).unwrap();
        let remainder = b.value(repr(), None).unwrap();
        let corrected = b.value(repr(), None).unwrap();
        let joined = b.value(repr(), None).unwrap();
        let join = b.block(&[joined], None).unwrap();
        b.terminate(
            entry,
            Node::new(
                Op::Branch {
                    condition: vf(args[1], repr()),
                    successors: 2,
                },
                &views,
            ),
            &[(success, vec![]), (failure, vec![])],
        )
        .unwrap();
        b.terminate(failure, Node::new(Op::Trap, &views), &[])
            .unwrap();
        b.append(
            success,
            Node::new(
                Op::Divide {
                    numerator: vf(args[0], repr()),
                    divisor: vf(args[1], repr()),
                    quotient: vf(quotient, repr()),
                    remainder: vf(remainder, repr()),
                },
                &views,
            ),
        )
        .unwrap();
        b.terminate(
            success,
            Node::new(
                Op::Branch {
                    condition: vf(remainder, repr()),
                    successors: 2,
                },
                &views,
            ),
            &[(join, vec![quotient]), (correction, vec![])],
        )
        .unwrap();
        b.append(
            correction,
            Node::new(
                Op::Add {
                    a: vf(quotient, repr()),
                    b: vf(args[1], repr()),
                    out: vf(corrected, repr()),
                    destructive: true,
                },
                &views,
            ),
        )
        .unwrap();
        b.terminate(
            correction,
            Node::new(Op::Jump, &views),
            &[(join, vec![corrected])],
        )
        .unwrap();
        ret(&mut b, join, vec![], vec![], &views);
        let body = checked(b, Shape::Two);
        let mut edit = body.into_editor();
        if broken != 0 {
            if broken == 1 {
                edit.replace_operand(success, 0, 1, args[0]).unwrap();
            } else {
                edit.replace_terminal_operand(entry, 0, args[0]).unwrap();
            }
            assert!(
                matches!(edit.finish(&target(&ctx,Shape::Two)),Err(SelectedEditFailure::Verify(errors)) if errors.iter().any(|e|e.reason==SelectedReason::Target("unsecured divisor")))
            );
        } else {
            edit.split_block(correction, 1, Node::new(Op::Jump, &views))
                .unwrap();
            let mut values = edit.values().collect::<Vec<_>>();
            values.reverse();
            let mut blocks = edit.blocks().collect::<Vec<_>>();
            blocks.reverse();
            let objects = edit.objects().collect::<Vec<_>>();
            let (edit, map) = edit.rebuild(&values, &blocks, &objects).unwrap();
            let body = edit.finish(&target(&ctx, Shape::Two)).unwrap();
            assert_eq!(
                body.analysis()
                    .unwrap()
                    .predecessors(map.blocks.get(join.id()).unwrap().index())
                    .unwrap()
                    .len(),
                2
            );
            assert_eq!(
                body.draft()
                    .blocks
                    .get_id(map.blocks.get(correction.id()).unwrap())
                    .unwrap()
                    .origin,
                Some(origin())
            );
        }
    }
}

#[test]
fn selected_authority_cannot_be_cloned() {
    let _: fn(VerifiedSelectedCallable<'static, Node>) -> SelectedEditor<'static, Node> =
        VerifiedSelectedCallable::into_editor;
    type ReopenedProgram = Result<
        (
            SelectedProgramBuilder<'static>,
            SelectedEditor<'static, Node>,
        ),
        lir::ProgramError,
    >;
    let _: fn(
        VerifiedSelectedProgram<'static>,
        VerifiedSelectedCallable<'static, Node>,
    ) -> ReopenedProgram = VerifiedSelectedProgram::edit;
    trait AmbiguousIfClone<A> {
        fn check() {}
    }
    impl<T> AmbiguousIfClone<()> for T {}
    impl<T: Clone> AmbiguousIfClone<u8> for T {}
    let _ = <VerifiedSelectedCallable<'static, Node> as AmbiguousIfClone<_>>::check;
    let _ = <VerifiedSelectedProgram<'static> as AmbiguousIfClone<_>>::check;
    let _ =
        <graph::GraphSession<'static, SelectedDraft<'static, Node>> as AmbiguousIfClone<_>>::check;
}

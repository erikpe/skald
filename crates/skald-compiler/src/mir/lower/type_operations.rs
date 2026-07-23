//! Type-test and checked-narrowing lowering.

use super::*;
use crate::hir::{
    BlockFlow, HirAccess, HirBlock, HirNarrowing, HirNarrowingKind, HirStatement, HirTypeTest,
    HirTypeTestKind, HirViewTarget,
};

pub(super) fn collect_narrowed_aliases(block: &HirBlock) -> Vec<&HirNarrowing> {
    fn visit<'hir>(block: &'hir HirBlock, aliases: &mut Vec<&'hir HirNarrowing>) {
        for statement in &block.statements {
            match statement {
                HirStatement::Narrowing(narrowing) => {
                    aliases.push(narrowing);
                    visit(&narrowing.body, aliases);
                }
                HirStatement::Conditional(conditional) => {
                    for arm in &conditional.arms {
                        visit(&arm.body, aliases);
                    }
                    if let Some(block) = &conditional.else_block {
                        visit(block, aliases);
                    }
                }
                HirStatement::Block(block) => visit(block, aliases),
                _ => {}
            }
        }
    }

    let mut aliases = Vec::new();
    visit(block, &mut aliases);
    aliases.sort_by_key(|narrowing| narrowing.binding.index());
    aliases
}

impl BodyLowerer<'_> {
    pub(super) fn lower_type_test(
        &mut self,
        expression: &HirExpression,
        test: &HirTypeTest,
    ) -> ValueId {
        let kind = match test.kind {
            HirTypeTestKind::StaticSuccess => MirRvalueKind::ConstantBool(true),
            HirTypeTestKind::StaticFailure => MirRvalueKind::ConstantBool(false),
            HirTypeTestKind::Runtime => MirRvalueKind::TypeTest {
                source: self.lower_object_view(&test.source),
                target: lower_view_target(test.target),
            },
        };
        self.assign(kind, MirType::Bool, expression.span)
    }

    pub(super) fn lower_narrowing(&mut self, narrowing: &HirNarrowing) {
        let binding = MirNarrowedAliasBinding {
            destination: self.narrowed_alias_storage[narrowing.binding.index()],
            view: self.lower_object_view(&narrowing.view),
            span: narrowing.span,
        };
        match narrowing.kind {
            HirNarrowingKind::Static => {
                self.emit(MirInstruction::BindNarrowedAlias(binding));
                self.lower_block(&narrowing.body);
                if !self.body.is_current_terminated() {
                    self.emit(MirInstruction::EndNarrowedAlias(MirNarrowedAliasEnd {
                        alias: self.narrowed_alias_storage[narrowing.binding.index()],
                        span: narrowing.body.span,
                    }));
                }
            }
            HirNarrowingKind::Runtime { .. } => {
                let success = self.body.allocate_block(narrowing.body.span);
                let failure = self.body.allocate_block(narrowing.span);
                let join = (narrowing.body.flow == BlockFlow::FallsThrough)
                    .then(|| self.body.allocate_block(narrowing.span));
                self.terminate(MirTerminator::CheckedNarrow {
                    binding,
                    success_target: success,
                    failure_target: failure,
                    span: narrowing.span,
                });

                self.body
                    .select_block(failure)
                    .expect("allocated narrowing failure block must be selectable");
                self.terminate(MirTerminator::Terminate {
                    reason: MirTerminationReason::NarrowingFailure,
                    span: narrowing.span,
                });

                self.body
                    .select_block(success)
                    .expect("allocated narrowing success block must be selectable");
                self.lower_block(&narrowing.body);
                if !self.body.is_current_terminated() {
                    self.emit(MirInstruction::EndNarrowedAlias(MirNarrowedAliasEnd {
                        alias: self.narrowed_alias_storage[narrowing.binding.index()],
                        span: narrowing.body.span,
                    }));
                    self.terminate(MirTerminator::Goto {
                        target: join.expect("falling-through narrowing requires a join"),
                        span: narrowing.body.span,
                    });
                    self.body
                        .select_block(join.expect("falling-through narrowing requires a join"))
                        .expect("allocated narrowing join block must be selectable");
                }
            }
        }
    }
}

pub(super) const fn lower_view_target(target: HirViewTarget) -> MirViewTarget {
    match target {
        HirViewTarget::Class(class) => MirViewTarget::Class(class),
        HirViewTarget::Interface(interface) => MirViewTarget::Interface(interface),
        HirViewTarget::Obj => MirViewTarget::Obj,
    }
}

pub(super) const fn lower_access(access: HirAccess) -> MirAliasAccess {
    match access {
        HirAccess::ReadOnly => MirAliasAccess::ReadOnly,
        HirAccess::Mutable => MirAliasAccess::Mutable,
    }
}

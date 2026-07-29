//! Conditional CFG construction and deterministic block allocation.

use super::*;
use crate::hir::HirConditional;

impl BodyLowerer<'_> {
    pub(super) fn lower_conditional(&mut self, conditional: &HirConditional) {
        debug_assert!(!conditional.arms.is_empty());
        let needs_join = conditional.effects.can_fall_through();

        // Allocate the complete shape before emitting edges. IDs therefore
        // follow source structure rather than a traversal chosen by lowering:
        // condition, body, next condition, body, else, join.
        let mut condition_blocks = vec![self.body.current()];
        let mut body_blocks = Vec::with_capacity(conditional.arms.len());
        for (index, arm) in conditional.arms.iter().enumerate() {
            body_blocks.push(self.body.allocate_block(arm.body.span));
            if index + 1 < conditional.arms.len() {
                condition_blocks.push(
                    self.body
                        .allocate_block(conditional.arms[index + 1].condition.span),
                );
            }
        }
        let else_block = conditional
            .else_block
            .as_ref()
            .map(|block| self.body.allocate_block(block.span));
        let join_block = needs_join.then(|| self.body.allocate_block(conditional.span));

        for (index, arm) in conditional.arms.iter().enumerate() {
            self.body
                .select_block(condition_blocks[index])
                .expect("allocated conditional block must be selectable");
            let condition = self
                .lower_expression(&arm.condition)
                .expect("typed conditional condition must produce a value");
            self.finish_full_expression(arm.condition.span);
            let false_target = condition_blocks
                .get(index + 1)
                .copied()
                .or(else_block)
                .or(join_block)
                .expect("a conditional's false path must have a target");
            self.terminate(MirTerminator::Branch {
                condition,
                true_target: body_blocks[index],
                false_target,
                span: arm.span,
            });

            self.body
                .select_block(body_blocks[index])
                .expect("allocated conditional body must be selectable");
            self.lower_block(&arm.body);
            if !self.body.is_current_terminated() {
                self.terminate(MirTerminator::Goto {
                    target: join_block.expect("a falling-through arm requires a join block"),
                    span: arm.body.span,
                });
            }
        }

        if let (Some(source), Some(block)) = (&conditional.else_block, else_block) {
            self.body
                .select_block(block)
                .expect("allocated else block must be selectable");
            self.lower_block(source);
            if !self.body.is_current_terminated() {
                self.terminate(MirTerminator::Goto {
                    target: join_block.expect("a falling-through else requires a join block"),
                    span: source.span,
                });
            }
        }

        if let Some(join) = join_block {
            self.body
                .select_block(join)
                .expect("allocated conditional join must be selectable");
        }
    }
}

//! Structured loop lowering into target-independent MIR control flow.

use super::*;
use crate::hir::{HirBreak, HirContinue, HirWhile};

impl BodyLowerer<'_> {
    pub(super) fn lower_break(&mut self, statement: &HirBreak) {
        let context = self
            .loop_contexts
            .find(statement.target)
            .expect("resolved break target must have an active lowering context");
        let cleanup = self
            .cleanup
            .for_scopes_exiting_to(context.retained_scope_depth(), statement.span);
        self.emit_scope_exit(cleanup);
        self.terminate(MirTerminator::Goto {
            target: context.exit_target(),
            span: statement.span,
        });
    }

    pub(super) fn lower_continue(&mut self, statement: &HirContinue) {
        let context = self
            .loop_contexts
            .find(statement.target)
            .expect("resolved continue target must have an active lowering context");
        let cleanup = self
            .cleanup
            .for_scopes_exiting_to(context.retained_scope_depth(), statement.span);
        self.emit_scope_exit(cleanup);
        self.terminate(MirTerminator::Goto {
            target: context
                .latch_target()
                .expect("a typed continue effect must make its loop latch reachable"),
            span: statement.span,
        });
    }

    pub(super) fn lower_while(&mut self, statement: &HirWhile) {
        // Allocate the loop shape before emitting any edge.
        // The current block is the preheader, followed by header, body, an
        // optional reachable latch, and exit in source order.
        let header = self.body.allocate_block(statement.condition.span);
        let body = self.body.allocate_block(statement.body.span);
        let reaches_latch = statement.body.effects.can_fall_through()
            || statement.body.effects.can_continue_to(statement.loop_id);
        let latch = reaches_latch.then(|| self.body.allocate_block(statement.span));
        let exit = self.body.allocate_block(statement.span);

        self.terminate(MirTerminator::Goto {
            target: header,
            span: statement.span,
        });

        self.body
            .select_block(header)
            .expect("allocated loop header must be selectable");
        let condition = self
            .lower_expression(&statement.condition)
            .expect("typed while condition must produce a boolean value");
        self.finish_full_expression(statement.condition.span);
        self.terminate(MirTerminator::Branch {
            condition,
            true_target: body,
            false_target: exit,
            span: statement.span,
        });

        let retained_scope_depth = self.cleanup.retained_scope_depth();
        let context =
            loop_context::LoopContext::new(statement.loop_id, exit, latch, retained_scope_depth)
                .expect("typed loop and its MIR targets must share a callable owner");
        self.loop_contexts.push(context);

        self.body
            .select_block(body)
            .expect("allocated loop body must be selectable");
        self.lower_block(&statement.body);
        if !self.body.is_current_terminated() {
            self.terminate(MirTerminator::Goto {
                target: latch.expect("a falling-through loop body requires a latch"),
                span: statement.body.span,
            });
        }

        self.loop_contexts.pop(statement.loop_id);

        if let Some(latch) = latch {
            self.body
                .select_block(latch)
                .expect("allocated loop latch must be selectable");
            self.terminate(MirTerminator::Goto {
                target: header,
                span: statement.span,
            });
        }

        self.body
            .select_block(exit)
            .expect("allocated loop exit must be selectable");
    }
}

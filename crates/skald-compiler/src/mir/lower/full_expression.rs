//! Ordered full-expression resource tracking.

use crate::{
    identity::CallableId,
    mir::{
        BlockId, MirAssignment, MirInstruction, MirPathCondition, MirPathConditionValue, MirRvalue,
        MirRvalueKind, MirTerminator, MirType, MirValue, PathConditionId, StorageId, ValueId,
    },
    source::Span,
};

use super::{FullExpressionTemporary, MirBodyBuilder};

#[derive(Clone)]
pub(super) struct ConditionalRegistration<T> {
    pub(super) condition: Option<PathConditionId>,
    pub(super) value: T,
}

#[derive(Default)]
pub(super) struct FullExpressionTracker {
    current_condition: Option<PathConditionId>,
    conditions: Vec<MirPathCondition>,
    temporaries: Vec<ConditionalRegistration<FullExpressionTemporary>>,
    storage: Vec<ConditionalRegistration<StorageId>>,
    checked_views: Vec<ConditionalRegistration<StorageId>>,
    has_shared_effect: bool,
}

pub(super) struct FullExpressionPlan {
    pub(super) conditions: Vec<MirPathCondition>,
    pub(super) temporaries: Vec<ConditionalRegistration<FullExpressionTemporary>>,
    pub(super) storage: Vec<ConditionalRegistration<StorageId>>,
    pub(super) checked_views: Vec<ConditionalRegistration<StorageId>>,
    pub(super) has_shared_effect: bool,
}

impl FullExpressionTracker {
    pub(super) fn register_temporary(&mut self, temporary: FullExpressionTemporary) {
        self.temporaries.push(ConditionalRegistration {
            condition: self.current_condition,
            value: temporary,
        });
    }

    pub(super) fn remove_temporary(
        &mut self,
        mut matches: impl FnMut(&FullExpressionTemporary) -> bool,
    ) {
        let index = self
            .temporaries
            .iter()
            .rposition(|registration| matches(&registration.value))
            .expect("consumed temporary must belong to the current full expression");
        self.temporaries.remove(index);
    }

    pub(super) fn register_storage(&mut self, storage: StorageId) {
        self.storage.push(ConditionalRegistration {
            condition: self.current_condition,
            value: storage,
        });
    }

    pub(super) fn remove_storage(&mut self, storage: StorageId) {
        let index = self
            .storage
            .iter()
            .rposition(|registration| registration.value == storage)
            .expect("extended storage must belong to the current full expression");
        self.storage.remove(index);
    }

    pub(super) fn register_checked_view(&mut self, storage: StorageId) {
        self.checked_views.push(ConditionalRegistration {
            condition: self.current_condition,
            value: storage,
        });
    }

    pub(super) fn mark_shared_effect(&mut self) {
        self.has_shared_effect = true;
    }

    pub(super) fn has_temporaries(&self) -> bool {
        !self.temporaries.is_empty()
    }

    /// Establish the condition inherited by subsequent registrations.
    ///
    /// Logical-expression lowering selects this boundary for its right
    /// operand. Keeping condition selection here prevents each resource family
    /// from maintaining an independent path stack.
    pub(super) fn select_condition(&mut self, condition: Option<PathConditionId>) {
        if let Some(condition) = condition {
            assert!(
                self.conditions
                    .iter()
                    .any(|candidate| candidate.id == condition),
                "selected full-expression condition must be registered"
            );
        }
        self.current_condition = condition;
    }

    pub(super) const fn current_condition(&self) -> Option<PathConditionId> {
        self.current_condition
    }

    pub(super) fn has_conditions(&self) -> bool {
        !self.conditions.is_empty()
    }

    /// Register one path decision owned by the current full expression.
    pub(super) fn register_condition(&mut self, condition: MirPathCondition) {
        assert!(
            self.conditions
                .iter()
                .all(|candidate| candidate.id != condition.id),
            "full-expression path condition must be registered exactly once"
        );
        if let Some(parent) = condition.parent {
            assert!(
                self.conditions
                    .iter()
                    .any(|candidate| candidate.id == parent),
                "full-expression parent condition must be registered first"
            );
        }
        self.conditions.push(condition);
    }

    pub(super) fn take_plan(&mut self) -> FullExpressionPlan {
        self.current_condition = None;
        FullExpressionPlan {
            conditions: std::mem::take(&mut self.conditions),
            temporaries: std::mem::take(&mut self.temporaries),
            storage: std::mem::take(&mut self.storage),
            checked_views: std::mem::take(&mut self.checked_views),
            has_shared_effect: std::mem::take(&mut self.has_shared_effect),
        }
    }

    pub(super) fn clear(&mut self) {
        self.current_condition = None;
        self.conditions.clear();
        self.temporaries.clear();
        self.storage.clear();
        self.checked_views.clear();
        self.has_shared_effect = false;
    }
}

impl FullExpressionPlan {
    pub(super) fn requires_boundary(&self) -> bool {
        !self.conditions.is_empty()
            || !self.temporaries.is_empty()
            || !self.storage.is_empty()
            || self.has_shared_effect
    }
}

pub(super) fn condition_chain(
    conditions: &[MirPathCondition],
    condition: PathConditionId,
) -> Vec<&MirPathCondition> {
    let mut chain = Vec::new();
    let mut current = Some(condition);
    while let Some(id) = current {
        let condition = conditions
            .iter()
            .find(|candidate| candidate.id == id)
            .expect("conditional registration must name a tracked path condition");
        chain.push(condition);
        current = condition.parent;
    }
    chain.reverse();
    chain
}

pub(super) struct ConditionalRegion {
    levels: Vec<ConditionalRegionLevel>,
}

struct ConditionalRegionLevel {
    inactive: BlockId,
    merge: BlockId,
}

pub(super) fn build_conditional_region(
    body: &mut MirBodyBuilder,
    values: &mut Vec<MirValue>,
    callable: CallableId,
    condition: PathConditionId,
    conditions: &[MirPathCondition],
    span: Span,
) -> ConditionalRegion {
    let chain = condition_chain(conditions, condition);
    let mut levels = Vec::with_capacity(chain.len());
    for condition in chain {
        let active = body.allocate_block(span);
        let inactive = body.allocate_block(span);
        let merge = body.allocate_block(span);
        let selected = ValueId::new(callable, values.len());
        values.push(MirValue {
            id: selected,
            ty: MirType::Bool,
            span,
        });
        body.push_instruction(MirInstruction::Assign(MirAssignment {
            result: selected,
            rvalue: MirRvalue {
                kind: MirRvalueKind::PathCondition(MirPathConditionValue {
                    condition: condition.id,
                    activation: condition.activation,
                }),
                ty: MirType::Bool,
            },
            span,
        }))
        .expect("full-expression cleanup must emit into an open block");
        body.terminate(MirTerminator::Branch {
            condition: selected,
            true_target: active,
            false_target: inactive,
            span,
        })
        .expect("full-expression cleanup must terminate each decision block once");
        body.select_block(active)
            .expect("allocated conditional cleanup block must be selectable");
        levels.push(ConditionalRegionLevel { inactive, merge });
    }
    ConditionalRegion { levels }
}

pub(super) fn finish_conditional_region(
    body: &mut MirBodyBuilder,
    region: ConditionalRegion,
    span: Span,
) {
    for level in region.levels.into_iter().rev() {
        body.terminate(MirTerminator::Goto {
            target: level.merge,
            span,
        })
        .expect("conditional cleanup action must leave its block open");
        body.select_block(level.inactive)
            .expect("allocated conditional bypass block must be selectable");
        body.terminate(MirTerminator::Goto {
            target: level.merge,
            span,
        })
        .expect("conditional cleanup bypass must terminate once");
        body.select_block(level.merge)
            .expect("allocated conditional merge block must be selectable");
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        identity::{CallableId, FunctionId},
        mir::{BlockId, MirStorageDead, PathConditionId},
        source::SourceDatabase,
    };

    use super::*;

    fn condition(index: usize, parent: Option<usize>) -> MirPathCondition {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "");
        MirPathCondition {
            id: PathConditionId::new(callable, index),
            parent: parent.map(|parent| PathConditionId::new(callable, parent)),
            activation: StorageId::new(callable, index),
            active_predecessor: BlockId::new(callable, index * 3 + 1),
            inactive_predecessor: BlockId::new(callable, index * 3 + 2),
            merge: BlockId::new(callable, index * 3 + 3),
            span: sources.get(source).unwrap().span(0, 0).unwrap(),
        }
    }

    #[test]
    fn registrations_retain_completion_order_and_selected_condition() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut tracker = FullExpressionTracker::default();
        let root = condition(0, None);
        let child = condition(1, Some(0));
        tracker.register_condition(root.clone());
        tracker.register_storage(StorageId::new(callable, 2));
        tracker.select_condition(Some(root.id));
        tracker.register_storage(StorageId::new(callable, 3));
        tracker.register_condition(child.clone());
        tracker.select_condition(Some(child.id));
        tracker.register_storage(StorageId::new(callable, 4));
        tracker.select_condition(None);
        tracker.register_storage(StorageId::new(callable, 5));

        let plan = tracker.take_plan();

        assert_eq!(
            plan.storage
                .iter()
                .map(|registration| (registration.value.index(), registration.condition))
                .collect::<Vec<_>>(),
            [
                (2, None),
                (3, Some(root.id)),
                (4, Some(child.id)),
                (5, None),
            ]
        );
        assert_eq!(
            condition_chain(&plan.conditions, child.id)
                .iter()
                .map(|condition| condition.id)
                .collect::<Vec<_>>(),
            [root.id, child.id]
        );
    }

    #[test]
    fn a_condition_without_selected_resources_still_requires_epoch_cleanup() {
        let mut tracker = FullExpressionTracker::default();
        tracker.register_condition(condition(0, None));

        let plan = tracker.take_plan();

        assert!(plan.requires_boundary());
        assert!(plan.storage.is_empty());
        assert!(plan.temporaries.is_empty());
        assert!(plan.checked_views.is_empty());
    }

    #[test]
    fn body_builder_keeps_path_conditions_in_registration_order() {
        let callable = CallableId::Function(FunctionId::new(0));
        let root = condition(0, None);
        let child = condition(1, Some(0));
        let mut body = MirBodyBuilder::new(callable, root.span);

        assert_eq!(body.register_path_condition(root.clone()), root.id);
        assert_eq!(body.register_path_condition(child.clone()), child.id);

        assert_eq!(body.finish().path_conditions, [root, child]);
    }

    #[test]
    fn conditional_region_tests_parents_before_children_and_reconverges_locally() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "");
        let span = sources.get(source).unwrap().span(0, 0).unwrap();
        let root = condition(0, None);
        let child = condition(1, Some(0));
        let mut body = MirBodyBuilder::new(callable, span);
        let mut values = Vec::new();

        let region = build_conditional_region(
            &mut body,
            &mut values,
            callable,
            child.id,
            &[root.clone(), child.clone()],
            span,
        );
        body.push_instruction(MirInstruction::StorageDead(MirStorageDead {
            storage: StorageId::new(callable, 2),
            span,
        }))
        .unwrap();
        finish_conditional_region(&mut body, region, span);
        body.terminate(MirTerminator::Return { value: None, span })
            .unwrap();
        let body = body.finish();

        assert_eq!(values.len(), 2);
        let MirInstruction::Assign(root_read) = &body.blocks[0].instructions[0] else {
            panic!("outer decision must begin with a condition read");
        };
        let MirRvalueKind::PathCondition(root_read) = root_read.rvalue.kind else {
            panic!("outer decision must read a path condition");
        };
        assert_eq!(root_read.condition, root.id);
        let MirInstruction::Assign(child_read) = &body.blocks[1].instructions[0] else {
            panic!("active parent path must test the child condition");
        };
        let MirRvalueKind::PathCondition(child_read) = child_read.rvalue.kind else {
            panic!("nested decision must read a path condition");
        };
        assert_eq!(child_read.condition, child.id);
        assert!(matches!(
            body.blocks[4].instructions.as_slice(),
            [MirInstruction::StorageDead(_)]
        ));
        assert!(matches!(
            body.blocks[3].terminator,
            Some(MirTerminator::Return { .. })
        ));
        assert_eq!(body.blocks.len(), 7);
    }

    #[test]
    fn sibling_conditional_regions_share_only_the_local_continuation() {
        let callable = CallableId::Function(FunctionId::new(0));
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "");
        let span = sources.get(source).unwrap().span(0, 0).unwrap();
        let first = condition(0, None);
        let second = condition(1, None);
        let conditions = [first.clone(), second.clone()];
        let mut body = MirBodyBuilder::new(callable, span);
        let mut values = Vec::new();

        for condition in [first.id, second.id] {
            let region = build_conditional_region(
                &mut body,
                &mut values,
                callable,
                condition,
                &conditions,
                span,
            );
            body.push_instruction(MirInstruction::StorageDead(MirStorageDead {
                storage: StorageId::new(callable, condition.index() + 2),
                span,
            }))
            .unwrap();
            finish_conditional_region(&mut body, region, span);
        }
        body.terminate(MirTerminator::Return { value: None, span })
            .unwrap();
        let body = body.finish();

        assert_eq!(body.blocks.len(), 7);
        assert_eq!(values.len(), 2);
        assert!(matches!(
            body.blocks[3].terminator,
            Some(MirTerminator::Branch { .. })
        ));
        assert!(matches!(
            body.blocks[6].terminator,
            Some(MirTerminator::Return { .. })
        ));
    }
}

#[cfg(test)]
#[path = "full_expression/owner_tests.rs"]
mod owner_tests;

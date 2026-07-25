//! Verification of runtime type tests and checked object casts.

use std::collections::HashSet;

use super::{
    super::model::{
        MirBasicBlock, MirCheckedViewBinding, MirCheckedViewEnd, MirDefinitionRef, MirInstruction,
        MirRvalue, MirStorageKind, MirTerminator, MirType, MirViewTarget,
    },
    context::Verifier,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) enum TypeRelation {
    StaticSuccess,
    StaticFailure,
    Runtime,
}

impl Verifier<'_> {
    pub(super) fn verify_type_test(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        source: &super::super::model::MirObjectView,
        target: MirViewTarget,
    ) {
        let source_place = self.verify_object_view(function, block, source, "type-test source");
        if rvalue.ty != MirType::Bool {
            self.block_error(
                function.callable(),
                block.id,
                "runtime type-test result is not `bool`",
            );
        }
        self.verify_view_target_declared(function, block, target, "type-test");
        if source_place.is_some_and(|place| {
            self.classify_type_relation(place.ty, target) != TypeRelation::Runtime
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "runtime type test does not require a metadata query",
            );
        }
    }

    pub(super) fn verify_checked_view_binding(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        binding: &MirCheckedViewBinding,
        runtime: bool,
    ) {
        let Some(storage) = function.storage(binding.destination) else {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "checked-view destination {} is not declared",
                    binding.destination
                ),
            );
            return;
        };
        let access = match storage.kind {
            MirStorageKind::CheckedView(access) => access,
            _ => {
                self.block_error(
                    function.callable(),
                    block.id,
                    "checked-view binding destination has the wrong storage kind",
                );
                return;
            }
        };
        let source =
            self.verify_checked_object_view(function, block, &binding.view, "checked-cast view");
        self.verify_view_target_declared(function, block, binding.view.target, "checked cast");
        if storage.ty != binding.view.target.ty() || access != binding.view.access {
            self.block_error(
                function.callable(),
                block.id,
                "checked-view storage does not match its selected view",
            );
        }
        if let Some(source) = source {
            let expected = if runtime {
                TypeRelation::Runtime
            } else {
                TypeRelation::StaticSuccess
            };
            let relation_source = match binding.view.origin.as_ref() {
                super::super::model::MirObjectOrigin::Shared {
                    exact_dynamic_class: Some(class),
                    ..
                } => MirType::Class(*class),
                _ => source.ty,
            };
            if self.classify_type_relation(relation_source, binding.view.target) != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    if runtime {
                        "checked cast does not require a runtime check"
                    } else {
                        "static checked cast is not guaranteed to succeed"
                    },
                );
            }
        }
    }

    pub(super) fn verify_checked_view_end(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        end: &MirCheckedViewEnd,
    ) {
        if !function
            .storage(end.carrier)
            .is_some_and(|storage| matches!(storage.kind, MirStorageKind::CheckedView(_)))
        {
            self.block_error(
                function.callable(),
                block.id,
                "checked-view end does not name checked-view storage",
            );
        }
    }

    pub(super) fn verify_checked_view_definitions(&mut self, function: MirDefinitionRef<'_>) {
        let mut definitions = HashSet::new();
        for block in &function.body().blocks {
            if let Some(MirTerminator::CheckedCast { binding, .. }) = &block.terminator {
                if !definitions.insert(binding.destination) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "checked-view carrier is defined more than once",
                    );
                }
            }
            for instruction in &block.instructions {
                if let MirInstruction::BindCheckedView(binding) = instruction {
                    if !definitions.insert(binding.destination) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "checked-view carrier is defined more than once",
                        );
                    }
                }
            }
        }
        for storage in function.storage_entries() {
            if matches!(storage.kind, MirStorageKind::CheckedView(_))
                && !definitions.contains(&storage.id)
            {
                self.function_error(
                    function.callable(),
                    format!("checked-view storage {} has no definition", storage.id),
                );
            }
        }
    }

    pub(super) fn verify_checked_cast_terminator(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        binding: &MirCheckedViewBinding,
        success_target: super::super::model::BlockId,
        failure_target: super::super::model::BlockId,
    ) {
        self.verify_checked_view_binding(function, block, binding, true);
        self.verify_block_target(function, block, success_target);
        self.verify_block_target(function, block, failure_target);
        if success_target == failure_target {
            self.block_error(
                function.callable(),
                block.id,
                "checked cast success and failure edges must differ",
            );
        }
        if !function.block(failure_target).is_some_and(|failure| {
            matches!(
                failure.terminator,
                Some(MirTerminator::Terminate {
                    reason: super::super::model::MirTerminationReason::ObjectCastFailure,
                    ..
                })
            )
        }) {
            self.block_error(
                function.callable(),
                block.id,
                "checked cast failure edge must terminate with object-cast failure",
            );
        }
    }

    fn verify_view_target_declared(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: MirViewTarget,
        kind: &str,
    ) {
        let declared = match target {
            MirViewTarget::Class(class) => self.program.class(class).is_some(),
            MirViewTarget::Interface(interface) => self.program.interface(interface).is_some(),
            MirViewTarget::Obj => true,
        };
        if !declared {
            self.block_error(
                function.callable(),
                block.id,
                format!("{kind} target is not declared"),
            );
        }
    }

    pub(super) fn classify_type_relation(
        &self,
        source: MirType,
        target: MirViewTarget,
    ) -> TypeRelation {
        if target == MirViewTarget::Obj {
            return TypeRelation::StaticSuccess;
        }
        if self.view_guarantees_target(source, target) {
            return TypeRelation::StaticSuccess;
        }
        let mut any_success = false;
        let mut any_failure = false;
        for class in self.program.classes.iter().map(|class| class.id) {
            if !self.class_can_inhabit_type(class, source) {
                continue;
            }
            if self.class_provides_view(class, target) {
                any_success = true;
            } else {
                any_failure = true;
            }
        }
        match (any_success, any_failure) {
            (true, true) => TypeRelation::Runtime,
            (true, false) => TypeRelation::StaticSuccess,
            _ => TypeRelation::StaticFailure,
        }
    }

    fn view_guarantees_target(&self, source: MirType, target: MirViewTarget) -> bool {
        match source {
            MirType::Class(class) => self.class_provides_view(class, target),
            MirType::Interface(source) => target == MirViewTarget::Interface(source),
            MirType::Obj => false,
            _ => false,
        }
    }

    pub(super) fn class_can_inhabit_type(
        &self,
        class: crate::identity::ClassId,
        source: MirType,
    ) -> bool {
        match source {
            MirType::Class(target) => class == target || self.program.is_ancestor(target, class),
            MirType::Interface(interface) => self.program.conformance(class, interface).is_some(),
            MirType::Obj => true,
            _ => false,
        }
    }

    pub(super) fn class_provides_view(
        &self,
        class: crate::identity::ClassId,
        target: MirViewTarget,
    ) -> bool {
        match target {
            MirViewTarget::Class(target) => {
                class == target || self.program.is_ancestor(target, class)
            }
            MirViewTarget::Interface(interface) => {
                self.program.conformance(class, interface).is_some()
            }
            MirViewTarget::Obj => true,
        }
    }
}

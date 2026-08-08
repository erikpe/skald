//! Call and initializer contract verification.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirCall, MirCallReceiver, MirCallTarget, MirDefinitionRef,
        MirInitialize, MirMethodCallTarget, MirMethodDeclaration, MirMethodKind, MirMethodReceiver,
        MirParameter, MirPlaceBase, MirReceiverAccess, MirStorageKind, MirType, MirViewTarget,
        ValueId,
    },
    context::Verifier,
    view::VerifiedOrigin,
};

struct CallSignature<'mir> {
    parameters: &'mir [MirParameter],
    return_type: MirType,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn verify_call(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        // A call result is not available to its arguments. Retain the
        // pre-call set explicitly before registering the result below.
        let arguments_defined = defined_in_block.clone();
        let result_ty = self.verify_call_result(
            function,
            block,
            call.result,
            defined_values,
            defined_in_block,
        );
        let Some(signature) = self.verify_call_target(function, block, call) else {
            return;
        };

        self.verify_arguments(
            function,
            block,
            "call",
            &call.arguments,
            signature.parameters,
            &arguments_defined,
        );
        self.verify_call_return(function, block, call, signature.return_type, result_ty);
    }

    pub(super) fn verify_initialize(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        initialize: &MirInitialize,
        defined_in_block: &HashSet<ValueId>,
    ) {
        let destination = self.verify_place(function, block, &initialize.destination);
        if matches!(
            initialize.destination.base,
            MirPlaceBase::AliasParameter(_) | MirPlaceBase::CheckedView(_)
        ) {
            self.block_error(
                function.callable(),
                block.id,
                "initializer destination must be owning storage",
            );
        }
        let Some(target) = self.program.initializer(initialize.target) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("initializer target {} is not declared", initialize.target),
            );
            return;
        };
        if destination.map(|place| place.ty) != Some(MirType::Class(initialize.target.class())) {
            self.block_error(
                function.callable(),
                block.id,
                "initializer destination has the wrong class type",
            );
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "initializer destination requires mutable access",
            );
        }
        self.verify_arguments(
            function,
            block,
            "initializer",
            &initialize.arguments,
            &target.parameters,
            defined_in_block,
        );
    }

    fn verify_call_result(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        result: Option<ValueId>,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) -> Option<MirType> {
        let result = result?;
        let metadata = function.value(result);
        if metadata.is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("call result {result} is not declared"),
            );
        }
        if !defined_values.insert(result) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {result} is defined more than once"),
            );
        }
        defined_in_block.insert(result);
        metadata.map(|metadata| metadata.ty)
    }

    fn verify_call_target(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
    ) -> Option<CallSignature<'mir>> {
        match call.target {
            MirCallTarget::Direct(target_id) => {
                if call.receiver.is_some() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "ordinary function call must not have a receiver",
                    );
                }
                let Some(target) = self.program.declarations.get(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("call target {target_id} is not declared"),
                    );
                    return None;
                };
                if matches!(
                    target.linkage,
                    super::super::model::MirFunctionLinkage::Intrinsic { .. }
                ) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "intrinsic function must not remain as an ordinary MIR call",
                    );
                    return None;
                }
                Some(CallSignature {
                    parameters: &target.parameters,
                    return_type: target.return_type,
                })
            }
            MirCallTarget::Static(target_id) => {
                if call.receiver.is_some() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "static method call must not have a receiver",
                    );
                }
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("static method target {target_id} is not declared"),
                    );
                    return None;
                };
                if target.kind != MirMethodKind::Static {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "static call target is an instance method",
                    );
                }
                Some(CallSignature {
                    parameters: &target.parameters,
                    return_type: target.return_type,
                })
            }
            MirCallTarget::Method(method_target) => {
                let target_id = method_target.selected();
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("method target {target_id} is not declared"),
                    );
                    return None;
                };
                if !matches!(target.kind, MirMethodKind::Instance { .. }) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "receiver-bearing method call target is static",
                    );
                }
                self.verify_method_receiver(
                    function,
                    block,
                    call.receiver.as_ref().and_then(MirCallReceiver::as_method),
                    method_target,
                    target,
                );
                if call
                    .receiver
                    .as_ref()
                    .is_some_and(|receiver| receiver.as_method().is_none())
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "method call requires a method receiver",
                    );
                }
                Some(CallSignature {
                    parameters: &target.parameters,
                    return_type: target.return_type,
                })
            }
            MirCallTarget::Interface(target) => {
                let Some(requirement) = self.program.interface_requirement(target.requirement)
                else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!(
                            "interface requirement target {} is not declared",
                            target.requirement
                        ),
                    );
                    return None;
                };
                if target.requirement.interface() != target.interface
                    || requirement.id.interface() != target.interface
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "interface call requirement belongs to a different interface",
                    );
                }
                let receiver = call
                    .receiver
                    .as_ref()
                    .and_then(MirCallReceiver::as_interface);
                self.verify_interface_receiver(
                    function,
                    block,
                    receiver,
                    target.interface,
                    requirement,
                );
                if call
                    .receiver
                    .as_ref()
                    .is_some_and(|receiver| receiver.as_interface().is_none())
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "interface call requires an interface-view receiver",
                    );
                }
                Some(CallSignature {
                    parameters: &requirement.parameters,
                    return_type: requirement.return_type,
                })
            }
        }
    }

    fn verify_interface_receiver(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        receiver: Option<&super::super::model::MirObjectView>,
        interface: crate::identity::InterfaceId,
        requirement: &super::super::model::MirInterfaceRequirement,
    ) {
        let Some(receiver) = receiver else {
            self.block_error(
                function.callable(),
                block.id,
                "interface call requires a receiver",
            );
            return;
        };
        self.verify_object_view(function, block, receiver, "interface receiver");
        if receiver.target != MirViewTarget::Interface(interface) {
            self.block_error(
                function.callable(),
                block.id,
                "interface receiver target differs from the call target",
            );
        }
        if requirement.receiver_access == MirReceiverAccess::Mutable
            && receiver.access != MirAliasAccess::Mutable
        {
            self.block_error(
                function.callable(),
                block.id,
                "mutable interface requirement requires mutable receiver access",
            );
        }
    }

    fn verify_method_receiver(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        receiver: Option<&MirMethodReceiver>,
        call_target: MirMethodCallTarget,
        target: &MirMethodDeclaration,
    ) {
        let Some(receiver) = receiver else {
            self.block_error(
                function.callable(),
                block.id,
                "method call requires a receiver",
            );
            return;
        };
        let place = self.verify_place(function, block, &receiver.place);
        let target_id = call_target.selected();
        if place.map(|place| place.ty) != Some(MirType::Class(target_id.class())) {
            self.block_error(
                function.callable(),
                block.id,
                "method receiver has the wrong class type",
            );
        }
        if target.kind.receiver_access() == Some(MirReceiverAccess::Mutable)
            && place.is_some_and(|place| place.access != MirAliasAccess::Mutable)
        {
            self.block_error(
                function.callable(),
                block.id,
                "mutable method receiver requires mutable access",
            );
        }
        let origin = self.verify_object_origin(
            function,
            block,
            &receiver.origin,
            &receiver.place,
            place,
            "method receiver",
        );
        self.verify_dispatch_target(function, block, call_target, target, origin);
    }

    fn verify_dispatch_target(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        target: MirMethodCallTarget,
        declaration: &MirMethodDeclaration,
        origin: Option<VerifiedOrigin>,
    ) {
        let family_for_method = self
            .program
            .virtual_families
            .iter()
            .find(|family| family.members.contains(&declaration.id));
        match target {
            MirMethodCallTarget::Direct(_) => {
                if let Some(family) = family_for_method {
                    if origin.is_some_and(|origin| origin.forwarded && !origin.dispatch_limited) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "direct virtual-family call requires exact or dispatch-limited receiver origin",
                        );
                    }
                    if let Some(static_class) = origin.and_then(|origin| origin.static_class) {
                        if self.selected_family_member(family, static_class) != Some(declaration.id)
                        {
                            self.block_error(
                                function.callable(),
                                block.id,
                                "direct virtual-family call selected method does not match the exact or dispatch-limited class",
                            );
                        }
                    }
                }
            }
            MirMethodCallTarget::Virtual {
                family,
                slot,
                selected,
            } => {
                let Some(metadata) = self.program.virtual_family(family) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("virtual call family {family} is not declared"),
                    );
                    return;
                };
                if metadata.slot != slot {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "virtual call slot does not match its family",
                    );
                }
                if !metadata.members.contains(&selected) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "virtual call selected method is not a member of its family",
                    );
                }
                let Some(origin) = origin else {
                    return;
                };
                if !origin.forwarded || origin.dispatch_limited {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "virtual call requires an unrestricted forwarded receiver origin",
                    );
                }
                let Some(static_class) = origin.static_class else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "virtual call requires a class-typed receiver origin",
                    );
                    return;
                };
                if self.selected_family_member(metadata, static_class) != Some(selected) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "virtual call selected method does not match the receiver's static class",
                    );
                }
            }
        }
    }

    fn selected_family_member(
        &self,
        family: &crate::mir::MirVirtualFamily,
        static_class: crate::identity::ClassId,
    ) -> Option<crate::identity::MethodId> {
        family
            .members
            .iter()
            .copied()
            .fold(None, |selected, method| {
                let owner = method.class();
                if owner != static_class && !self.program.is_ancestor(owner, static_class) {
                    return selected;
                }
                match selected {
                    Some(current) if !self.program.is_ancestor(current.class(), owner) => {
                        Some(current)
                    }
                    _ => Some(method),
                }
            })
    }

    fn verify_call_return(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        return_type: MirType,
        result_ty: Option<MirType>,
    ) {
        if let MirType::Shared(target) = return_type {
            if call.result.is_some() || call.destination.is_some() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "shared-returning call must not have a scalar or object result",
                );
            }
            let valid = call.shared_result.is_some_and(|result| {
                function.storage(result).is_some_and(|storage| {
                    matches!(
                        storage.kind,
                        MirStorageKind::Local
                            | MirStorageKind::Temporary
                            | MirStorageKind::SharedAnchor
                            | MirStorageKind::Argument
                            | MirStorageKind::Return
                    ) && matches!(
                        storage.ty,
                        MirType::Shared(destination)
                            if self.shared_target_accepts(destination, target)
                    )
                })
            });
            if !valid {
                self.block_error(
                    function.callable(),
                    block.id,
                    "shared-returning call requires matching caller-owned result storage",
                );
            }
            return;
        }
        if let MirType::OptionalShared(target) = return_type {
            if call.result.is_some() || call.destination.is_some() {
                self.block_error(
                    function.callable(),
                    block.id,
                    "optional-shared-returning call must not have a scalar or object result",
                );
            }
            let valid = call.shared_result.is_some_and(|result| {
                function.storage(result).is_some_and(|storage| {
                    matches!(
                        storage.kind,
                        MirStorageKind::Local
                            | MirStorageKind::Temporary
                            | MirStorageKind::Argument
                            | MirStorageKind::Return
                    ) && matches!(
                        storage.ty,
                        MirType::OptionalShared(destination)
                            if self.shared_target_accepts(destination, target)
                    )
                })
            });
            if !valid {
                self.block_error(
                    function.callable(),
                    block.id,
                    "optional-shared-returning call requires matching caller-owned result storage",
                );
            }
            return;
        }
        if call.shared_result.is_some() {
            self.block_error(
                function.callable(),
                block.id,
                "non-shared call must not declare a shared result",
            );
        }
        let destination = call
            .destination
            .as_ref()
            .and_then(|place| self.verify_place(function, block, place));

        match (return_type, result_ty, destination) {
            (MirType::Array(_), Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "array-returning call must not have a scalar result",
            ),
            (MirType::Array(array), None, destination) => {
                let complete_destination = call.destination.as_ref().is_some_and(|place| {
                    self.is_static_initializer_destination(function, place, MirType::Array(array))
                        || (place.projections.is_empty()
                            && matches!(place.base, MirPlaceBase::Storage(_))
                            && function
                                .storage(place.base.expect_local_storage())
                                .is_some_and(|storage| {
                                    matches!(
                                        storage.kind,
                                        MirStorageKind::Local
                                            | MirStorageKind::ArrayProduced
                                            | MirStorageKind::Argument
                                            | MirStorageKind::Return
                                    )
                                }))
                });
                if destination.map(|place| place.ty) != Some(MirType::Array(array))
                    || !complete_destination
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array-returning call requires a complete matching caller destination",
                    );
                }
            }
            (MirType::Unit, Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "unit-returning call must not have a result",
            ),
            (MirType::Unit, None, Some(_)) => self.block_error(
                function.callable(),
                block.id,
                "unit-returning call must not have a destination",
            ),
            (MirType::Unit, None, None) => {}
            (MirType::Class(_), Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "object-returning call must not have a scalar result",
            ),
            (MirType::Class(class), None, destination) => {
                let complete_destination = call.destination.as_ref().is_some_and(|place| {
                    self.is_static_initializer_destination(
                        function,
                        place,
                        MirType::Class(class),
                    ) || (matches!(place.base, MirPlaceBase::Storage(_))
                        && function
                            .storage(place.base.expect_local_storage())
                            .is_some_and(|storage| match place.projections.as_slice() {
                                [] => matches!(
                                    storage.kind,
                                    MirStorageKind::Local | MirStorageKind::Temporary
                                ),
                                [crate::mir::MirPlaceProjection::OptionalPayload(_)] => matches!(
                                    storage.kind,
                                    MirStorageKind::Local | MirStorageKind::Temporary
                                ),
                                [crate::mir::MirPlaceProjection::ArrayElement { .. }] => {
                                    storage.kind == MirStorageKind::ArrayBacking
                                }
                                [
                                    crate::mir::MirPlaceProjection::ArrayElement { .. },
                                    crate::mir::MirPlaceProjection::OptionalPayload(_),
                                ] => storage.kind == MirStorageKind::ArrayBacking,
                                _ => false,
                            }))
                });
                if destination.map(|place| place.ty) != Some(MirType::Class(class))
                    || !complete_destination
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "object-returning call requires complete exact-class local or temporary destination storage, an optional payload, or an unpublished element-list slot",
                    );
                }
            }
            (MirType::OptionalPrimitive(_), Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "optional-returning call must not have a scalar result",
            ),
            (MirType::OptionalPrimitive(payload), None, destination) => {
                let complete_destination = call.destination.as_ref().is_some_and(|place| {
                    self.is_static_initializer_destination(
                        function,
                        place,
                        MirType::OptionalPrimitive(payload),
                    ) || (place.projections.is_empty()
                        && matches!(place.base, MirPlaceBase::Storage(_))
                        && function
                            .storage(place.base.expect_local_storage())
                            .is_some_and(|storage| {
                                matches!(
                                    storage.kind,
                                    MirStorageKind::Local
                                        | MirStorageKind::Temporary
                                        | MirStorageKind::Argument
                                )
                            }))
                });
                if destination.map(|place| place.ty) != Some(MirType::OptionalPrimitive(payload))
                    || !complete_destination
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "optional-returning call requires complete matching caller-owned destination storage",
                    );
                }
            }
            (MirType::OptionalClass(_), Some(_), _) => self.block_error(
                function.callable(),
                block.id,
                "class-optional-returning call must not have a scalar result",
            ),
            (MirType::OptionalClass(class), None, destination) => {
                if destination.map(|place| place.ty) != Some(MirType::OptionalClass(class)) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class-optional-returning call requires matching caller-owned destination storage",
                    );
                }
            }
            (_, Some(_), Some(_)) => self.block_error(
                function.callable(),
                block.id,
                "scalar-returning call must not have an object destination",
            ),
            (_, Some(result_ty), None) if result_ty != return_type => {
                self.block_error(function.callable(), block.id, "call result type mismatch");
            }
            (_, None, _) => self.block_error(
                function.callable(),
                block.id,
                "value-returning call has no result",
            ),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests;

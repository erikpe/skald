//! Call and initializer contract verification.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirCall, MirCallTarget, MirDefinitionRef, MirInitialize,
        MirMethodDeclaration, MirParameter, MirPlace, MirPlaceBase, MirReceiverAccess,
        MirStorageKind, MirType, ValueId,
    },
    context::Verifier,
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
        if matches!(initialize.destination.base, MirPlaceBase::AliasParameter(_)) {
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
                Some(CallSignature {
                    parameters: &target.parameters,
                    return_type: target.return_type,
                })
            }
            MirCallTarget::Method(target_id) => {
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("method target {target_id} is not declared"),
                    );
                    return None;
                };
                self.verify_method_receiver(
                    function,
                    block,
                    call.receiver.as_ref(),
                    target_id,
                    target,
                );
                Some(CallSignature {
                    parameters: &target.parameters,
                    return_type: target.return_type,
                })
            }
        }
    }

    fn verify_method_receiver(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        receiver: Option<&MirPlace>,
        target_id: crate::identity::MethodId,
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
        let receiver = self.verify_place(function, block, receiver);
        if receiver.map(|place| place.ty) != Some(MirType::Class(target_id.class())) {
            self.block_error(
                function.callable(),
                block.id,
                "method receiver has the wrong class type",
            );
        }
        if target.receiver_access == MirReceiverAccess::Mutable
            && receiver.is_some_and(|place| place.access != MirAliasAccess::Mutable)
        {
            self.block_error(
                function.callable(),
                block.id,
                "mutable method receiver requires mutable access",
            );
        }
    }

    fn verify_call_return(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        return_type: MirType,
        result_ty: Option<MirType>,
    ) {
        let destination = call
            .destination
            .as_ref()
            .and_then(|place| self.verify_place(function, block, place));

        match (return_type, result_ty, destination) {
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
                    place.projections.is_empty()
                        && matches!(place.base, MirPlaceBase::Storage(_))
                        && function
                            .storage(place.base.storage())
                            .is_some_and(|storage| {
                                matches!(
                                    storage.kind,
                                    MirStorageKind::Local | MirStorageKind::Temporary
                                )
                            })
                });
                if destination.map(|place| place.ty) != Some(MirType::Class(class))
                    || !complete_destination
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "object-returning call requires complete exact-class local or temporary destination storage",
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

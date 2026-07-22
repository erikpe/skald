//! Shared MIR verifier context.

use std::collections::HashSet;

use crate::identity::CallableId;

use super::{super::model::*, sink::ErrorSink, MirVerificationError};

pub(super) struct Verifier<'mir> {
    pub(super) program: &'mir MirProgram,
    pub(super) errors: ErrorSink,
}

#[derive(Clone, Copy)]
pub(super) struct VerifiedPlace {
    pub(super) ty: MirType,
    pub(super) access: MirAliasAccess,
}

impl<'mir> Verifier<'mir> {
    pub(super) fn new(program: &'mir MirProgram) -> Self {
        Self {
            program,
            errors: ErrorSink::new(),
        }
    }

    pub(super) fn into_errors(self) -> Vec<MirVerificationError> {
        self.errors.into_errors()
    }

    pub(super) fn verify_call(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        call: &MirCall,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        let arguments_defined = defined_in_block.clone();
        let result_ty = match call.result {
            Some(result) => {
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
            None => None,
        };

        let (parameters, return_type) = match call.target {
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
                    return;
                };
                (&target.parameters, target.return_type)
            }
            MirCallTarget::Method(target_id) => {
                let Some(target) = self.program.method(target_id) else {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("method target {target_id} is not declared"),
                    );
                    return;
                };
                match &call.receiver {
                    Some(receiver) => {
                        let receiver = self.verify_place(function, block, receiver);
                        if receiver.map(|place| place.ty) != Some(MirType::Class(target_id.class()))
                        {
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
                    None => self.block_error(
                        function.callable(),
                        block.id,
                        "method call requires a receiver",
                    ),
                }
                (&target.parameters, target.return_type)
            }
        };
        self.verify_arguments(
            function,
            block,
            "call",
            &call.arguments,
            parameters,
            &arguments_defined,
        );

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
                self.block_error(function.callable(), block.id, "call result type mismatch")
            }
            (_, None, _) => self.block_error(
                function.callable(),
                block.id,
                "value-returning call has no result",
            ),
            _ => {}
        }
    }

    pub(super) fn verify_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        kind: &str,
        arguments: &[MirArgument],
        parameters: &[MirParameter],
        defined: &HashSet<ValueId>,
    ) {
        if arguments.len() != parameters.len() {
            self.block_error(
                function.callable(),
                block.id,
                format!(
                    "{kind} has {} arguments but requires {}",
                    arguments.len(),
                    parameters.len()
                ),
            );
        }
        let mut owned_arguments = HashSet::new();
        for (index, argument) in arguments.iter().enumerate() {
            let Some(parameter) = parameters.get(index) else {
                match argument {
                    MirArgument::Value(value) => {
                        self.verify_value_use(function, block, *value, defined);
                    }
                    MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                        self.verify_place(function, block, place);
                    }
                }
                continue;
            };
            match (argument, parameter.mode) {
                (MirArgument::Value(value), MirParameterMode::Value) => {
                    let argument_ty = self.verify_value_use(function, block, *value, defined);
                    if argument_ty.is_some() && argument_ty != Some(parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                }
                (MirArgument::OwnedPlace(place), MirParameterMode::Value)
                    if matches!(parameter.ty, MirType::Class(_)) =>
                {
                    let argument = self.verify_place(function, block, place);
                    let complete_argument_storage = matches!(place.base, MirPlaceBase::Storage(_))
                        && place.projections.is_empty()
                        && function
                            .storage(place.base.storage())
                            .is_some_and(|storage| storage.kind == MirStorageKind::Argument);
                    if !complete_argument_storage {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!(
                                "{kind} argument {index} must transfer complete caller argument storage"
                            ),
                        );
                    }
                    if argument.is_some_and(|argument| argument.ty != parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                    if !owned_arguments.insert(place.clone()) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} transfers storage more than once"),
                        );
                    }
                }
                (MirArgument::Place(place), MirParameterMode::ReadOnlyAlias)
                | (MirArgument::Place(place), MirParameterMode::MutableAlias) => {
                    let argument = self.verify_place(function, block, place);
                    if argument.is_some_and(|argument| argument.ty != parameter.ty) {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} type mismatch"),
                        );
                    }
                    if parameter.mode == MirParameterMode::MutableAlias
                        && argument
                            .is_some_and(|argument| argument.access != MirAliasAccess::Mutable)
                    {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} argument {index} requires mutable access"),
                        );
                    }
                }
                (MirArgument::Value(value), _) => {
                    self.verify_value_use(function, block, *value, defined);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a place"),
                    );
                }
                (MirArgument::Place(place), MirParameterMode::Value) => {
                    self.verify_place(function, block, place);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} must be a scalar value or owned place"),
                    );
                }
                (MirArgument::OwnedPlace(place), _) => {
                    self.verify_place(function, block, place);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} cannot transfer ownership"),
                    );
                }
            }
        }
    }

    pub(super) fn verify_place(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        place: &MirPlace,
    ) -> Option<VerifiedPlace> {
        let storage_id = place.base.storage();
        let Some(storage) = function.storage(storage_id) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("place base {storage_id} is not declared in this function"),
            );
            return None;
        };
        let access = match (place.base, storage.kind) {
            (MirPlaceBase::Storage(_), MirStorageKind::AliasParameter(_)) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("alias parameter storage {storage_id} requires an indirect base"),
                );
                return None;
            }
            (MirPlaceBase::AliasParameter(_), MirStorageKind::AliasParameter(access)) => access,
            (MirPlaceBase::AliasParameter(_), _) => {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("indirect alias base {storage_id} is not alias parameter storage"),
                );
                return None;
            }
            (MirPlaceBase::Storage(_), _) => self.storage_access(function, storage),
        };
        let mut ty = storage.ty;
        for projection in &place.projections {
            match *projection {
                MirPlaceProjection::Field(field_id) => {
                    let MirType::Class(owner) = ty else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} has a non-class base"),
                        );
                        return None;
                    };
                    if field_id.class() != owner {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} belongs to the wrong class"),
                        );
                        return None;
                    }
                    let Some(field) = self.program.field(field_id) else {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("field projection {field_id} is not declared"),
                        );
                        return None;
                    };
                    ty = field.ty;
                }
            }
        }
        Some(VerifiedPlace { ty, access })
    }

    fn storage_access(
        &self,
        function: MirDefinitionRef<'_>,
        storage: &MirStorage,
    ) -> MirAliasAccess {
        if storage.kind != MirStorageKind::Receiver {
            return MirAliasAccess::Mutable;
        }
        match function.callable() {
            CallableId::Method(method) => match self
                .program
                .method(method)
                .map(|method| method.receiver_access)
            {
                Some(MirReceiverAccess::ReadOnly) => MirAliasAccess::ReadOnly,
                Some(MirReceiverAccess::Mutable) => MirAliasAccess::Mutable,
                None => MirAliasAccess::ReadOnly,
            },
            CallableId::Initializer(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_) => MirAliasAccess::Mutable,
            CallableId::Function(_) => MirAliasAccess::ReadOnly,
        }
    }

    pub(super) fn verify_value_use(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        defined: &HashSet<ValueId>,
    ) -> Option<MirType> {
        let Some(metadata) = function.value(value) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is not declared in this function"),
            );
            return None;
        };
        if !defined.contains(&value) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {value} is used before it is defined in this block"),
            );
        }
        Some(metadata.ty)
    }

    pub(super) fn program_error(&mut self, message: impl Into<String>) {
        self.errors.program(message);
    }

    pub(super) fn function_error(
        &mut self,
        callable: impl Into<CallableId>,
        message: impl Into<String>,
    ) {
        self.errors.callable(callable, message);
    }

    pub(super) fn block_error(
        &mut self,
        callable: impl Into<CallableId>,
        block: BlockId,
        message: impl Into<String>,
    ) {
        self.errors.block(callable, block, message);
    }
}

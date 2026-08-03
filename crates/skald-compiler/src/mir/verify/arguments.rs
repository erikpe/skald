//! Call and initializer argument verification.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirArgument, MirBasicBlock, MirDefinitionRef, MirObjectView, MirParameter,
        MirParameterMode, MirPlace, MirPlaceBase, MirStorageKind, MirType, ValueId,
    },
    context::Verifier,
};

#[derive(Clone, Copy)]
struct ArgumentSite<'a> {
    function: MirDefinitionRef<'a>,
    block: &'a MirBasicBlock,
    kind: &'a str,
}

impl Verifier<'_> {
    pub(super) fn verify_arguments(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        kind: &str,
        arguments: &[MirArgument],
        parameters: &[MirParameter],
        defined: &HashSet<ValueId>,
    ) {
        let site = ArgumentSite {
            function,
            block,
            kind,
        };
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
        let mut shared_arguments = HashSet::new();
        for (index, argument) in arguments.iter().enumerate() {
            let Some(parameter) = parameters.get(index) else {
                self.verify_extra_argument(site, argument, defined);
                continue;
            };
            match (argument, parameter.mode) {
                (MirArgument::Value(value), MirParameterMode::Value) => {
                    self.verify_value_argument(site, index, *value, parameter.ty, defined)
                }
                (MirArgument::OwnedPlace(place), MirParameterMode::Value)
                    if matches!(
                        parameter.ty,
                        MirType::Class(_)
                            | MirType::Array(_)
                            | MirType::OptionalPrimitive(_)
                            | MirType::OptionalClass(_)
                    ) =>
                {
                    self.verify_owned_place_argument(
                        site,
                        index,
                        place,
                        parameter.ty,
                        &mut owned_arguments,
                    );
                }
                (MirArgument::SharedOwner(owner), MirParameterMode::Value)
                    if matches!(
                        parameter.ty,
                        MirType::Shared(_) | MirType::OptionalShared(_)
                    ) =>
                {
                    self.verify_shared_owner_argument(
                        site,
                        index,
                        *owner,
                        parameter.ty,
                        &mut shared_arguments,
                    );
                }
                (MirArgument::Place(place), MirParameterMode::ReadOnlyAlias)
                | (MirArgument::Place(place), MirParameterMode::MutableAlias) => {
                    self.verify_alias_place_argument(site, index, place, parameter)
                }
                (MirArgument::View(view), MirParameterMode::ReadOnlyAlias)
                | (MirArgument::View(view), MirParameterMode::MutableAlias) => {
                    self.verify_view_argument(site, index, view, parameter)
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
                (MirArgument::View(view), MirParameterMode::Value) => {
                    self.verify_place(function, block, &view.source);
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} cannot pass a view by value"),
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
                (MirArgument::SharedOwner(owner), _) => {
                    if function.storage(*owner).is_none() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("{kind} shared argument storage {owner} is not declared"),
                        );
                    }
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} argument {index} cannot transfer a shared owner"),
                    );
                }
            }
        }
    }

    fn verify_extra_argument(
        &mut self,
        site: ArgumentSite<'_>,
        argument: &MirArgument,
        defined: &HashSet<ValueId>,
    ) {
        match argument {
            MirArgument::Value(value) => {
                self.verify_value_use(site.function, site.block, *value, defined);
            }
            MirArgument::Place(place) | MirArgument::OwnedPlace(place) => {
                self.verify_place(site.function, site.block, place);
            }
            MirArgument::View(view) => {
                self.verify_object_view(site.function, site.block, view, "extra object view");
            }
            MirArgument::SharedOwner(owner) => {
                if site.function.storage(*owner).is_none() {
                    self.block_error(
                        site.function.callable(),
                        site.block.id,
                        format!("extra shared argument storage {owner} is not declared"),
                    );
                }
            }
        }
    }

    fn verify_shared_owner_argument(
        &mut self,
        site: ArgumentSite<'_>,
        index: usize,
        owner: super::super::model::StorageId,
        parameter_ty: MirType,
        shared_arguments: &mut HashSet<super::super::model::StorageId>,
    ) {
        let valid = site.function.storage(owner).is_some_and(|storage| {
            storage.kind == MirStorageKind::Argument && storage.ty == parameter_ty
        });
        if !valid {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} must transfer matching shared caller argument storage",
                    site.kind
                ),
            );
        }
        if !shared_arguments.insert(owner) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} transfers shared storage more than once",
                    site.kind
                ),
            );
        }
    }

    fn verify_value_argument(
        &mut self,
        site: ArgumentSite<'_>,
        index: usize,
        value: ValueId,
        parameter_ty: MirType,
        defined: &HashSet<ValueId>,
    ) {
        let argument_ty = self.verify_value_use(site.function, site.block, value, defined);
        if argument_ty.is_some() && argument_ty != Some(parameter_ty) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} type mismatch", site.kind),
            );
        }
    }

    fn verify_owned_place_argument(
        &mut self,
        site: ArgumentSite<'_>,
        index: usize,
        place: &MirPlace,
        parameter_ty: MirType,
        owned_arguments: &mut HashSet<MirPlace>,
    ) {
        let argument = self.verify_place(site.function, site.block, place);
        let complete_argument_storage = matches!(place.base, MirPlaceBase::Storage(_))
            && place.projections.is_empty()
            && site
                .function
                .storage(place.base.expect_local_storage())
                .is_some_and(|storage| storage.kind == MirStorageKind::Argument);
        if !complete_argument_storage {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} must transfer complete caller argument storage",
                    site.kind
                ),
            );
        }
        if argument.is_some_and(|argument| argument.ty != parameter_ty) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} type mismatch", site.kind),
            );
        }
        // Owned arguments are required to be complete storage roots, so root
        // equality is exactly the overlap rule for ownership transfer.
        if !owned_arguments.insert(place.clone()) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} transfers storage more than once",
                    site.kind
                ),
            );
        }
    }

    fn verify_alias_place_argument(
        &mut self,
        site: ArgumentSite<'_>,
        index: usize,
        place: &MirPlace,
        parameter: &MirParameter,
    ) {
        let argument = self.verify_place(site.function, site.block, place);
        if matches!(parameter.ty, MirType::Interface(_) | MirType::Obj) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} must represent a non-owning conversion as a view",
                    site.kind
                ),
            );
        }
        if argument.is_some_and(|argument| argument.ty != parameter.ty) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} type mismatch", site.kind),
            );
        }
        if parameter.mode == MirParameterMode::MutableAlias
            && argument.is_some_and(|argument| argument.access != MirAliasAccess::Mutable)
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} requires mutable access", site.kind),
            );
        }
    }

    fn verify_view_argument(
        &mut self,
        site: ArgumentSite<'_>,
        index: usize,
        view: &MirObjectView,
        parameter: &MirParameter,
    ) {
        self.verify_object_view(site.function, site.block, view, "object view");
        let target_ty = view.target.ty();
        if target_ty != parameter.ty {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} view target type mismatch", site.kind),
            );
        }
        if parameter.mode == MirParameterMode::MutableAlias
            && view.access != MirAliasAccess::Mutable
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} argument {index} requires a mutable view", site.kind),
            );
        }
        let required_access = match parameter.mode {
            MirParameterMode::ReadOnlyAlias => MirAliasAccess::ReadOnly,
            MirParameterMode::MutableAlias => MirAliasAccess::Mutable,
            MirParameterMode::Value => unreachable!("view arguments require alias parameters"),
        };
        if view.access != required_access {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} argument {index} view access does not match the parameter",
                    site.kind
                ),
            );
        }
    }
}

#[cfg(test)]
mod tests;

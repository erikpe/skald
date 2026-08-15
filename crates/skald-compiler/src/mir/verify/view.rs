//! Non-owning object-view conversions and complete-object provenance.

use super::{
    super::model::{
        MirAliasAccess, MirBasicBlock, MirDefinitionRef, MirObjectOrigin, MirObjectView, MirPlace,
        MirPlaceBase, MirPlaceProjection, MirStorageKind, MirType, MirViewTarget,
    },
    context::Verifier,
    place::{is_ancestor, VerifiedPlace},
};

#[derive(Clone, Copy)]
pub(super) struct VerifiedOrigin {
    pub(super) static_class: Option<crate::identity::ClassId>,
    pub(super) forwarded: bool,
    pub(super) dispatch_limited: bool,
}

#[derive(Clone, Copy)]
struct OriginSite<'mir> {
    function: MirDefinitionRef<'mir>,
    block: &'mir MirBasicBlock,
    subject: &'mir MirPlace,
    subject_metadata: Option<VerifiedPlace>,
    kind: &'mir str,
}

impl Verifier<'_> {
    pub(super) fn verify_produced_view_provenance(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        source: &MirPlace,
        origin: &MirObjectOrigin,
        provenance: super::super::model::MirViewProvenance,
        kind: &str,
    ) {
        if provenance != super::super::model::MirViewProvenance::Produced {
            return;
        }
        let valid = match origin {
            MirObjectOrigin::Exact { complete, .. } => {
                (complete.projections.is_empty()
                    || matches!(
                        complete.projections.last(),
                        Some(MirPlaceProjection::Field(_))
                    ))
                    && is_ancestor(complete, source)
                    && complete
                        .base
                        .local_storage()
                        .and_then(|storage| function.storage(storage))
                        .is_some_and(|storage| storage.kind == MirStorageKind::Temporary)
            }
            MirObjectOrigin::Forwarded { .. } | MirObjectOrigin::Shared { .. } => false,
        };
        if !valid {
            self.block_error(
                function.callable(),
                block.id,
                format!("produced {kind} requires an exact complete temporary origin"),
            );
        }
    }

    pub(super) fn verify_object_view(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        view: &MirObjectView,
        kind: &str,
    ) -> Option<VerifiedPlace> {
        self.verify_object_view_conversion(function, block, view, kind, false)
    }

    pub(super) fn verify_checked_object_view(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        view: &MirObjectView,
        kind: &str,
    ) -> Option<VerifiedPlace> {
        self.verify_object_view_conversion(function, block, view, kind, true)
    }

    fn verify_object_view_conversion(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        view: &MirObjectView,
        kind: &str,
        allow_dynamic_conversion: bool,
    ) -> Option<VerifiedPlace> {
        let source = self.verify_place(function, block, &view.source);
        self.verify_produced_view_provenance(
            function,
            block,
            &view.source,
            &view.origin,
            view.provenance,
            kind,
        );
        let valid_conversion = source.is_some_and(|source| {
            allow_dynamic_conversion
                && matches!(
                    source.ty,
                    MirType::Class(_) | MirType::Interface(_) | MirType::Obj
                )
                || match view.target {
                    MirViewTarget::Class(target) => source.ty == MirType::Class(target),
                    MirViewTarget::Interface(target) => match source.ty {
                        MirType::Class(class) => self.program.conformance(class, target).is_some(),
                        MirType::Interface(source) => source == target,
                        _ => false,
                    },
                    MirViewTarget::Obj => matches!(
                        source.ty,
                        MirType::Class(_) | MirType::Interface(_) | MirType::Obj
                    ),
                }
        });
        if !valid_conversion {
            self.block_error(
                function.callable(),
                block.id,
                format!("{kind} has an invalid static conversion"),
            );
        }
        if view.access == MirAliasAccess::Mutable
            && source.is_some_and(|source| source.access != MirAliasAccess::Mutable)
        {
            self.block_error(
                function.callable(),
                block.id,
                format!("{kind} grants mutable access"),
            );
        }
        if view.provenance == super::super::model::MirViewProvenance::Produced
            && view.access == MirAliasAccess::Mutable
        {
            self.block_error(
                function.callable(),
                block.id,
                format!("produced {kind} must be read-only"),
            );
        }
        let origin =
            self.verify_object_origin(function, block, &view.origin, &view.source, source, kind);
        if let Some(origin) = origin {
            match view.target {
                MirViewTarget::Class(target)
                    if origin.static_class.is_some_and(|static_class| {
                        let static_conversion = target == static_class
                            || self.program.is_ancestor(target, static_class);
                        let checked_downcast = allow_dynamic_conversion
                            && origin.forwarded
                            && self.program.is_ancestor(static_class, target);
                        !static_conversion && !checked_downcast
                    }) =>
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} target is incompatible with its origin"),
                    );
                }
                MirViewTarget::Interface(target)
                    if origin
                        .static_class
                        .is_some_and(|class| self.program.conformance(class, target).is_none()) =>
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!("{kind} target is not implemented by its origin class"),
                    );
                }
                _ => {}
            }
        }
        source
    }

    pub(super) fn verify_object_origin(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        origin: &MirObjectOrigin,
        subject: &MirPlace,
        subject_metadata: Option<VerifiedPlace>,
        kind: &str,
    ) -> Option<VerifiedOrigin> {
        let site = OriginSite {
            function,
            block,
            subject,
            subject_metadata,
            kind,
        };
        match origin {
            MirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => self.verify_exact_origin(site, complete, *dynamic_class),
            MirObjectOrigin::Forwarded {
                carrier,
                static_target,
                access,
                dispatch_limit,
                ..
            } => self.verify_forwarded_origin(
                site,
                *carrier,
                *static_target,
                *access,
                *dispatch_limit,
            ),
            MirObjectOrigin::Shared {
                owner,
                static_target,
                access,
                exact_dynamic_class,
                ..
            } => self.verify_shared_origin(
                site,
                *owner,
                *static_target,
                *access,
                *exact_dynamic_class,
            ),
        }
    }

    fn verify_shared_origin(
        &mut self,
        site: OriginSite<'_>,
        owner: super::super::model::StorageId,
        static_target: MirViewTarget,
        access: MirAliasAccess,
        exact_dynamic_class: Option<crate::identity::ClassId>,
    ) -> Option<VerifiedOrigin> {
        let Some(storage) = site.function.storage(owner) else {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} shared origin owner {owner} is not declared", site.kind),
            );
            return None;
        };
        let stable_owner = matches!(
            storage.kind,
            MirStorageKind::Local | MirStorageKind::Parameter | MirStorageKind::SharedAnchor
        );
        let ordinary_target = match static_target {
            MirViewTarget::Class(class) => super::super::model::MirSharedTarget::Class(class),
            MirViewTarget::Interface(interface) => {
                super::super::model::MirSharedTarget::Interface(interface)
            }
            MirViewTarget::Obj => super::super::model::MirSharedTarget::Obj,
        };
        let valid_owner = stable_owner
            && match storage.ty {
                MirType::Shared(actual) if actual == ordinary_target => true,
                MirType::Shared(super::super::model::MirSharedTarget::OptionalBox(target)) => self
                    .program
                    .optional_box_type(target)
                    .is_some_and(|metadata| metadata.object_view == Some(static_target)),
                _ => false,
            };
        if !valid_owner {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} shared origin requires a stable or call-anchor owner with the declared static target",
                    site.kind
                ),
            );
        }
        let valid_subject = site.subject.base == MirPlaceBase::SharedPointee(owner)
            || matches!(
                site.subject.base,
                MirPlaceBase::OptionalBoxPayload { owner: subject_owner, target }
                    if subject_owner == owner
                        && matches!(
                            storage.ty,
                            MirType::Shared(super::super::model::MirSharedTarget::OptionalBox(owner_target))
                                if owner_target == target
                        )
            );
        if !valid_subject {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} static place does not come from its shared owner payload",
                    site.kind
                ),
            );
        }
        if access != MirAliasAccess::Mutable
            || site
                .subject_metadata
                .is_some_and(|place| place.access != access)
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} shared origin access is inconsistent", site.kind),
            );
        }
        if exact_dynamic_class.is_some_and(|class| {
            self.program.class(class).is_none()
                || !self.shared_target_accepts(
                    match static_target {
                        MirViewTarget::Class(class) => {
                            super::super::model::MirSharedTarget::Class(class)
                        }
                        MirViewTarget::Interface(interface) => {
                            super::super::model::MirSharedTarget::Interface(interface)
                        }
                        MirViewTarget::Obj => super::super::model::MirSharedTarget::Obj,
                    },
                    super::super::model::MirSharedTarget::Class(class),
                )
        }) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} shared origin has incompatible exact dynamic provenance",
                    site.kind
                ),
            );
        }
        let static_class = exact_dynamic_class.or(match static_target {
            MirViewTarget::Class(class) => Some(class),
            MirViewTarget::Interface(_) | MirViewTarget::Obj => None,
        });
        Some(VerifiedOrigin {
            static_class,
            forwarded: true,
            dispatch_limited: false,
        })
    }

    fn verify_exact_origin(
        &mut self,
        site: OriginSite<'_>,
        complete: &MirPlace,
        dynamic_class: crate::identity::ClassId,
    ) -> Option<VerifiedOrigin> {
        let complete_metadata = self.verify_place(site.function, site.block, complete);
        if self.program.class(dynamic_class).is_none() {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} exact origin has an undeclared dynamic class", site.kind),
            );
        }
        if complete_metadata.map(|place| place.ty) != Some(MirType::Class(dynamic_class)) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} exact origin has the wrong dynamic class", site.kind),
            );
        }
        if !is_ancestor(complete, site.subject) {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} exact origin is not an ancestor of its static place",
                    site.kind
                ),
            );
        }
        let complete_storage = complete
            .base
            .local_storage()
            .and_then(|storage| site.function.storage(storage));
        let forwarded_root = complete_storage.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Receiver
                    | MirStorageKind::AliasParameter(_)
                    | MirStorageKind::CheckedView(_)
            )
        });
        let ends_at_complete_object_boundary = matches!(
            complete.projections.last(),
            Some(
                MirPlaceProjection::Field(_)
                    | MirPlaceProjection::OptionalPayload(_)
                    | MirPlaceProjection::ArrayElement { .. }
            )
        );
        if matches!(
            complete.projections.last(),
            Some(MirPlaceProjection::Base(_))
        ) || (forwarded_root && !ends_at_complete_object_boundary)
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} exact origin does not name a complete object", site.kind),
            );
        }
        if complete_metadata
            .zip(site.subject_metadata)
            .is_some_and(|(complete, subject)| complete.access != subject.access)
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} exact origin access differs from its static place",
                    site.kind
                ),
            );
        }
        Some(VerifiedOrigin {
            static_class: Some(dynamic_class),
            forwarded: false,
            dispatch_limited: false,
        })
    }

    fn verify_forwarded_origin(
        &mut self,
        site: OriginSite<'_>,
        carrier: super::super::model::StorageId,
        static_target: MirViewTarget,
        access: MirAliasAccess,
        dispatch_limit: Option<crate::identity::ClassId>,
    ) -> Option<VerifiedOrigin> {
        let Some(storage) = site.function.storage(carrier) else {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} origin carrier {carrier} is not declared", site.kind),
            );
            return None;
        };
        let carrier_access = match storage.kind {
            MirStorageKind::Receiver => self.storage_access(site.function, storage),
            MirStorageKind::AliasParameter(access) | MirStorageKind::CheckedView(access) => access,
            _ => {
                self.block_error(
                    site.function.callable(),
                    site.block.id,
                    format!("{} origin carrier is not a receiver or alias", site.kind),
                );
                return None;
            }
        };
        let expected_base = match storage.kind {
            MirStorageKind::Receiver => MirPlaceBase::Storage(carrier),
            MirStorageKind::AliasParameter(_) => MirPlaceBase::AliasParameter(carrier),
            MirStorageKind::CheckedView(_) => MirPlaceBase::CheckedView(carrier),
            _ => unreachable!("origin carrier kind checked above"),
        };
        if site.subject.base != expected_base
            || site
                .subject
                .projections
                .iter()
                .any(|projection| matches!(projection, MirPlaceProjection::Field(_)))
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} static place does not come from its forwarded carrier",
                    site.kind
                ),
            );
        }
        if access != carrier_access
            || site
                .subject_metadata
                .is_some_and(|place| place.access != access)
        {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!("{} forwarded origin access is inconsistent", site.kind),
            );
        }
        if static_target.ty() != storage.ty {
            self.block_error(
                site.function.callable(),
                site.block.id,
                format!(
                    "{} forwarded origin target differs from its carrier type",
                    site.kind
                ),
            );
        }
        let static_class = match static_target {
            MirViewTarget::Class(class) => Some(class),
            MirViewTarget::Interface(_) | MirViewTarget::Obj => None,
        };
        if let (Some(static_class), Some(MirType::Class(subject_class))) = (
            static_class,
            site.subject_metadata.map(|metadata| metadata.ty),
        ) {
            if subject_class != static_class
                && !self.program.is_ancestor(subject_class, static_class)
            {
                self.block_error(
                    site.function.callable(),
                    site.block.id,
                    format!(
                        "{} static place is incompatible with its origin target",
                        site.kind
                    ),
                );
            }
        }
        if let Some(limit) = dispatch_limit {
            let valid_limit = matches!(
                (site.function.callable(), storage.kind),
                (
                    crate::identity::CallableId::Destructor(_),
                    MirStorageKind::Receiver
                )
            ) && Some(limit) == static_class;
            if !valid_limit {
                self.block_error(
                    site.function.callable(),
                    site.block.id,
                    format!("{} has an invalid destructor dispatch limit", site.kind),
                );
            }
        }
        Some(VerifiedOrigin {
            static_class,
            forwarded: true,
            dispatch_limited: dispatch_limit.is_some(),
        })
    }
}

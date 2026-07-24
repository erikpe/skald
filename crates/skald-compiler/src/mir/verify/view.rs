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
        let origin =
            self.verify_object_origin(function, block, &view.origin, &view.source, source, kind);
        if let Some(origin) = origin {
            match view.target {
                MirViewTarget::Class(target)
                    if origin.static_class.is_some_and(|static_class| {
                        target != static_class && !self.program.is_ancestor(target, static_class)
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
        }
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
        let complete_storage = site.function.storage(complete.base.storage());
        let forwarded_root = complete_storage.is_some_and(|storage| {
            matches!(
                storage.kind,
                MirStorageKind::Receiver
                    | MirStorageKind::AliasParameter(_)
                    | MirStorageKind::CheckedView(_)
            )
        });
        let ends_at_field = matches!(
            complete.projections.last(),
            Some(MirPlaceProjection::Field(_))
        );
        if matches!(
            complete.projections.last(),
            Some(MirPlaceProjection::Base(_))
        ) || (forwarded_root && !ends_at_field)
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

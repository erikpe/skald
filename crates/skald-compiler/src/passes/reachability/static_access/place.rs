//! Static-place records for values, arguments, views, and place bases.

use super::*;

impl MirDependencyExtractor<'_> {
    pub(super) fn extract_static_rvalue(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        rvalue: &MirRvalue,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_)
            | MirRvalueKind::ConstantU64(_)
            | MirRvalueKind::ConstantU8(_)
            | MirRvalueKind::ConstantF64Bits(_)
            | MirRvalueKind::ConstantBool(_)
            | MirRvalueKind::PathCondition(_)
            | MirRvalueKind::Unary { .. }
            | MirRvalueKind::Binary { .. }
            | MirRvalueKind::IntegerDivision { .. }
            | MirRvalueKind::Shift { .. }
            | MirRvalueKind::PrimitiveComparison { .. }
            | MirRvalueKind::PrimitiveCast { .. }
            | MirRvalueKind::CheckedF64ToInteger { .. }
            | MirRvalueKind::CallableAddress(_) => Ok(()),
            MirRvalueKind::Load(place)
            | MirRvalueKind::OptionalPresence { source: place, .. }
            | MirRvalueKind::ArrayLength { source: place, .. } => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            ),
            MirRvalueKind::OptionalBoxPresence { owner, .. } => self.add_static_place(
                source,
                definition,
                region,
                &MirPlace::base(*owner),
                StaticAccessKind::Read,
                span,
            ),
            MirRvalueKind::TypeTest {
                source: tested_view,
                ..
            } => self.add_static_view(source, definition, region, tested_view, span),
        }
    }

    pub(super) fn add_static_argument(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        argument: &MirArgument,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        match argument {
            MirArgument::Value(_) | MirArgument::SharedOwner(_) => Ok(()),
            MirArgument::Place(place) | MirArgument::OwnedPlace(place) => self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Borrow,
                span,
            ),
            MirArgument::View(view) => self.add_static_view(source, definition, region, view, span),
        }
    }

    pub(super) fn add_static_optional_source(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        optional: &MirOptionalSource,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let MirOptionalSource::Copy(place) = optional {
            self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_static_class_optional_source(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        optional: &MirClassOptionalSource,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) =
            optional
        {
            self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_static_optional_shared_source(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        optional: &MirOptionalSharedSource,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let MirOptionalSharedSource::Copy(place) = optional {
            self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_static_view(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        view: &MirObjectView,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        self.add_static_place(
            source,
            definition,
            region,
            &view.source,
            StaticAccessKind::Borrow,
            span,
        )?;
        self.add_static_origin(source, definition, region, &view.origin, span)
    }

    pub(super) fn add_static_origin(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        origin: &MirObjectOrigin,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let MirObjectOrigin::Exact { complete, .. } = origin {
            self.add_static_place(
                source,
                definition,
                region,
                complete,
                StaticAccessKind::Borrow,
                span,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_static_shared_cast_source(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        cast: &MirSharedCastSource,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        if let MirSharedCastSource::Field { place, .. } = cast {
            self.add_static_place(
                source,
                definition,
                region,
                place,
                StaticAccessKind::Read,
                span,
            )?;
        }
        Ok(())
    }

    pub(super) fn add_static_place(
        &mut self,
        source: crate::mir::MirExecutionNode,
        definition: MirDefinitionRef<'_>,
        region: MirDependencyRegion,
        place: &MirPlace,
        mut kind: StaticAccessKind,
        span: Span,
    ) -> Result<(), MirDependencyExtractionError> {
        let (target, origin) = match place.base {
            MirPlaceBase::StaticField(field) => (field, MirStaticAccessOrigin::Ordinary),
            MirPlaceBase::StaticLifecycleDestination(field) => {
                if !matches!(
                    definition.callable(),
                    CallableId::StaticInitializer(id) if id.field() == field
                ) {
                    return Err(
                        MirDependencyExtractionError::InvalidStaticLifecycleDestination {
                            source: definition.callable(),
                            field,
                        },
                    );
                }
                if matches!(kind, StaticAccessKind::Write | StaticAccessKind::Initialize) {
                    kind = StaticAccessKind::Initialize;
                }
                (field, MirStaticAccessOrigin::LifecycleOwnedDestination)
            }
            _ => return Ok(()),
        };
        if self.program().static_field(target).is_none() {
            return Err(MirDependencyExtractionError::UnknownStaticField(target));
        }
        self.static_accesses.push(MirStaticAccess::new(
            source, target, kind, region, origin, span,
        ));
        Ok(())
    }
}

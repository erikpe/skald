use super::*;

impl Extractor<'_> {
    pub(super) fn add_rvalue(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        rvalue: &MirRvalue,
        span: Span,
    ) {
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
            | MirRvalueKind::CheckedF64ToInteger { .. } => {}
            MirRvalueKind::Load(place)
            | MirRvalueKind::OptionalPresence { source: place, .. }
            | MirRvalueKind::ArrayLength { source: place, .. } => self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Read,
                span,
            ),
            MirRvalueKind::TypeTest {
                source: tested_view,
                ..
            } => self.add_view(source, definition, phase, tested_view, span),
        }
    }

    pub(super) fn add_argument(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        argument: &MirArgument,
        span: Span,
    ) {
        match argument {
            MirArgument::Value(_) | MirArgument::SharedOwner(_) => {}
            MirArgument::Place(place) | MirArgument::OwnedPlace(place) => self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Borrow,
                span,
            ),
            MirArgument::View(view) => self.add_view(source, definition, phase, view, span),
        }
    }

    pub(super) fn add_optional_source(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        optional: &MirOptionalSource,
        span: Span,
    ) {
        if let MirOptionalSource::Copy(place) = optional {
            self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Read,
                span,
            );
        }
    }

    pub(super) fn add_class_optional_source(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        optional: &MirClassOptionalSource,
        span: Span,
    ) {
        if let MirClassOptionalSource::Present(place) | MirClassOptionalSource::Copy(place) =
            optional
        {
            self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Read,
                span,
            );
        }
    }

    pub(super) fn add_optional_shared_source(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        optional: &MirOptionalSharedSource,
        span: Span,
    ) {
        if let MirOptionalSharedSource::Copy(place) = optional {
            self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Read,
                span,
            );
        }
    }

    pub(super) fn add_view(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        view: &MirObjectView,
        span: Span,
    ) {
        self.add_place(
            source,
            definition,
            phase,
            &view.source,
            StaticAccessKind::Borrow,
            span,
        );
        self.add_origin(source, definition, phase, &view.origin, span);
    }

    pub(super) fn add_origin(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        origin: &MirObjectOrigin,
        span: Span,
    ) {
        if let MirObjectOrigin::Exact { complete, .. } = origin {
            self.add_place(
                source,
                definition,
                phase,
                complete,
                StaticAccessKind::Borrow,
                span,
            );
        }
    }

    pub(super) fn add_shared_cast_source(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        cast: &MirSharedCastSource,
        span: Span,
    ) {
        if let MirSharedCastSource::Field { place, .. } = cast {
            self.add_place(
                source,
                definition,
                phase,
                place,
                StaticAccessKind::Read,
                span,
            );
        }
    }

    pub(super) fn add_place(
        &mut self,
        source: StaticEffectNode,
        definition: MirDefinitionRef<'_>,
        phase: StaticEffectPhase,
        place: &MirPlace,
        mut access: StaticAccessKind,
        span: Span,
    ) {
        let (field, lifecycle_owned) = match place.base {
            MirPlaceBase::StaticField(field) => (field, false),
            MirPlaceBase::StaticLifecycleDestination(field) => {
                debug_assert!(
                    matches!(definition.callable(), CallableId::StaticInitializer(id) if id.field() == field)
                );
                if matches!(
                    access,
                    StaticAccessKind::Write | StaticAccessKind::Initialize
                ) {
                    access = StaticAccessKind::Initialize;
                }
                (field, true)
            }
            _ => return,
        };
        self.nodes
            .get_mut(&source)
            .expect("seeded source node")
            .direct
            .push(StaticAccessEvidence {
                field,
                access,
                phase,
                lifecycle_owned,
                span,
                witness: Vec::new(),
            });
    }

    pub(super) fn place_type(
        &self,
        definition: MirDefinitionRef<'_>,
        place: &MirPlace,
    ) -> Option<MirType> {
        let mut ty = match place.base {
            MirPlaceBase::StaticField(field) | MirPlaceBase::StaticLifecycleDestination(field) => {
                self.program.static_field(field)?.ty
            }
            base => definition.storage(base.local_storage()?)?.ty,
        };
        for projection in &place.projections {
            ty = match *projection {
                MirPlaceProjection::Base(class) | MirPlaceProjection::OptionalPayload(class) => {
                    MirType::Class(class)
                }
                MirPlaceProjection::Field(field) => self.program.field(field)?.ty,
                MirPlaceProjection::ArrayElement { array, .. } => {
                    self.program.array_type(array)?.element
                }
            };
        }
        Some(ty)
    }

    pub(super) fn add_call_targets(
        &mut self,
        source: StaticEffectNode,
        target: MirCallTarget,
        phase: StaticEffectPhase,
        span: Span,
    ) {
        match target {
            MirCallTarget::Direct(function) => {
                if self.program.definitions.get(function).is_some() {
                    self.add_edge(
                        source,
                        StaticEffectNode::Callable(function.into()),
                        StaticEffectEdgeKind::DirectCall,
                        phase,
                        span,
                    );
                }
            }
            MirCallTarget::Static(method) => self.add_edge(
                source,
                StaticEffectNode::Callable(method.into()),
                StaticEffectEdgeKind::StaticCall,
                phase,
                span,
            ),
            MirCallTarget::Method(MirMethodCallTarget::Direct(method)) => self.add_edge(
                source,
                StaticEffectNode::Callable(method.into()),
                StaticEffectEdgeKind::DirectMethodCall,
                phase,
                span,
            ),
            MirCallTarget::Method(MirMethodCallTarget::Virtual { family, .. }) => {
                let members = self
                    .program
                    .virtual_family(family)
                    .expect("verified virtual family must exist")
                    .members
                    .clone();
                for method in members {
                    self.add_edge(
                        source,
                        StaticEffectNode::Callable(method.into()),
                        StaticEffectEdgeKind::VirtualDispatch,
                        phase,
                        span,
                    );
                }
            }
            MirCallTarget::Interface(target) => {
                let methods = self
                    .program
                    .classes
                    .iter()
                    .filter_map(|class| self.program.conformance(class.id, target.interface))
                    .flat_map(|conformance| &conformance.implementations)
                    .filter(|implementation| implementation.requirement == target.requirement)
                    .map(|implementation| implementation.method)
                    .collect::<Vec<_>>();
                for method in methods {
                    self.add_edge(
                        source,
                        StaticEffectNode::Callable(method.into()),
                        StaticEffectEdgeKind::InterfaceDispatch,
                        phase,
                        span,
                    );
                }
            }
        }
    }
}

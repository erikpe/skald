//! Deterministic copy-capability analysis over inline class fields.

use crate::{
    hir::{
        HirBaseCopy, HirCopyCapability, HirSynthesizedCopy, HirSynthesizedFieldCopy, HirUserCopy,
    },
    identity::{ClassId, CopyAssignmentId, CopyConstructorId},
    resolve::{ResolvedClassDeclaration, ResolvedCopyOperation, ResolvedProgram, ResolvedTypeKind},
    type_capabilities::LifecyclePathElement,
};

#[derive(Clone, Debug)]
pub(super) struct CopyCapabilities {
    constructors: CapabilitySet<CopyConstructorId>,
    assignments: CapabilitySet<CopyAssignmentId>,
    array_types: crate::hir::HirArrayTypeTable,
}

#[cfg(test)]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct CopyCapabilityComputationReport {
    pub(super) constructor_rounds: usize,
    pub(super) assignment_rounds: usize,
    pub(super) cloned_constructor_records: usize,
    pub(super) cloned_assignment_records: usize,
    pub(super) provisional_array_builds: usize,
    pub(super) provisional_array_entries: usize,
    pub(super) final_array_builds: usize,
    pub(super) final_array_entries: usize,
    pub(super) final_publication_clones: usize,
    pub(super) final_publication_entries: usize,
    pub(super) constructor_plan_constructions: usize,
    pub(super) assignment_plan_constructions: usize,
}

impl CopyCapabilities {
    pub(super) fn compute(program: &ResolvedProgram) -> Self {
        #[cfg(test)]
        {
            Self::compute_internal(program, None)
        }
        #[cfg(not(test))]
        {
            Self::compute_internal(program)
        }
    }

    #[cfg(test)]
    pub(super) fn compute_with_report(
        program: &ResolvedProgram,
    ) -> (Self, CopyCapabilityComputationReport) {
        let mut report = CopyCapabilityComputationReport::default();
        let capabilities = Self::compute_internal(program, Some(&mut report));
        (capabilities, report)
    }

    fn compute_internal(
        program: &ResolvedProgram,
        #[cfg(test)] mut report: Option<&mut CopyCapabilityComputationReport>,
    ) -> Self {
        let mut constructors =
            CapabilitySet::compute(program, |class| class.copy_constructor, None);
        #[cfg(test)]
        record_count(
            &mut report,
            |report| &mut report.constructor_plan_constructions,
            program.classes.len(),
        );
        let provisional_assignments =
            CapabilitySet::compute(program, |class| class.copy_assignment, Some(&constructors));
        #[cfg(test)]
        record_count(
            &mut report,
            |report| &mut report.assignment_plan_constructions,
            program.classes.len(),
        );
        loop {
            #[cfg(test)]
            record_provisional_round(
                &mut report,
                program.classes.len(),
                program.array_types.len(),
                ArrayOperation::Copy,
            );
            let provisional = Self {
                constructors: constructors.clone(),
                assignments: provisional_assignments.clone(),
                array_types: crate::hir::HirArrayTypeTable::default(),
            };
            let arrays = crate::typeck::arrays::lower_array_types(program, &provisional);
            if !constructors.invalidate_array_dependencies(program, &arrays, ArrayOperation::Copy) {
                break;
            }
        }

        let mut assignments =
            CapabilitySet::compute(program, |class| class.copy_assignment, Some(&constructors));
        #[cfg(test)]
        record_count(
            &mut report,
            |report| &mut report.assignment_plan_constructions,
            program.classes.len(),
        );
        loop {
            #[cfg(test)]
            record_provisional_round(
                &mut report,
                program.classes.len(),
                program.array_types.len(),
                ArrayOperation::Assignment,
            );
            let provisional = Self {
                constructors: constructors.clone(),
                assignments: assignments.clone(),
                array_types: crate::hir::HirArrayTypeTable::default(),
            };
            let arrays = crate::typeck::arrays::lower_array_types(program, &provisional);
            if !assignments.invalidate_array_dependencies(
                program,
                &arrays,
                ArrayOperation::Assignment,
            ) {
                break;
            }
        }

        let mut capabilities = Self {
            constructors,
            assignments,
            array_types: crate::hir::HirArrayTypeTable::default(),
        };
        #[cfg(test)]
        {
            record_count(&mut report, |report| &mut report.final_array_builds, 1);
            record_count(
                &mut report,
                |report| &mut report.final_array_entries,
                program.array_types.len(),
            );
        }
        capabilities.array_types = crate::typeck::arrays::lower_array_types(program, &capabilities);
        capabilities
    }

    pub(super) fn constructor(&self, class: ClassId) -> &HirCopyCapability<CopyConstructorId> {
        self.constructors.capability(class)
    }

    pub(super) fn assignment(&self, class: ClassId) -> &HirCopyCapability<CopyAssignmentId> {
        self.assignments.capability(class)
    }

    pub(super) fn array(&self, array: crate::identity::ArrayTypeId) -> &crate::hir::HirArrayType {
        self.array_types
            .get(array)
            .expect("resolved array identity must have typed lifecycle metadata")
    }

    pub(super) fn array_types(&self) -> crate::hir::HirArrayTypeTable {
        self.array_types.clone()
    }

    #[cfg(test)]
    pub(super) fn array_types_with_report(
        &self,
        report: &mut CopyCapabilityComputationReport,
    ) -> crate::hir::HirArrayTypeTable {
        report.final_publication_clones = report.final_publication_clones.saturating_add(1);
        report.final_publication_entries = report
            .final_publication_entries
            .saturating_add(self.array_types.len());
        self.array_types()
    }

    pub(crate) fn constructor_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.constructors.failure(class)
    }

    pub(crate) fn assignment_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.assignments.failure(class)
    }
}

#[cfg(test)]
fn record_count(
    report: &mut Option<&mut CopyCapabilityComputationReport>,
    select: impl FnOnce(&mut CopyCapabilityComputationReport) -> &mut usize,
    count: usize,
) {
    if let Some(report) = report.as_deref_mut() {
        let value = select(report);
        *value = value.saturating_add(count);
    }
}

#[cfg(test)]
fn record_provisional_round(
    report: &mut Option<&mut CopyCapabilityComputationReport>,
    class_count: usize,
    array_count: usize,
    operation: ArrayOperation,
) {
    let Some(report) = report.as_deref_mut() else {
        return;
    };
    match operation {
        ArrayOperation::Copy => {
            report.constructor_rounds = report.constructor_rounds.saturating_add(1)
        }
        ArrayOperation::Assignment => {
            report.assignment_rounds = report.assignment_rounds.saturating_add(1)
        }
    }
    report.cloned_constructor_records = report
        .cloned_constructor_records
        .saturating_add(class_count);
    report.cloned_assignment_records = report.cloned_assignment_records.saturating_add(class_count);
    report.provisional_array_builds = report.provisional_array_builds.saturating_add(1);
    report.provisional_array_entries = report.provisional_array_entries.saturating_add(array_count);
}

#[derive(Clone, Debug)]
struct CapabilitySet<I> {
    capabilities: Vec<HirCopyCapability<I>>,
    /// The deterministic outer-to-inner field path responsible for an
    /// unavailable synthesized operation. An empty path means the class's own
    /// resolved operation is unavailable.
    failure_paths: Vec<Option<Vec<LifecyclePathElement>>>,
}

#[derive(Clone, Copy)]
enum ArrayOperation {
    Copy,
    Assignment,
}

impl<I: Copy> CapabilitySet<I> {
    fn compute(
        program: &ResolvedProgram,
        resolved_operation: fn(&ResolvedClassDeclaration) -> ResolvedCopyOperation<I>,
        required_constructors: Option<&CapabilitySet<CopyConstructorId>>,
    ) -> Self {
        let mut capabilities = vec![None; program.classes.len()];
        let mut failure_paths = vec![None; program.classes.len()];
        let mut states = vec![VisitState::Unvisited; program.classes.len()];

        for class in program.classes.iter() {
            compute_class(
                class.id,
                program,
                resolved_operation,
                &mut capabilities,
                &mut failure_paths,
                &mut states,
                required_constructors,
            );
        }

        Self {
            capabilities: capabilities
                .into_iter()
                .map(|capability| capability.expect("every class capability must be computed"))
                .collect(),
            failure_paths,
        }
    }

    fn capability(&self, class: ClassId) -> &HirCopyCapability<I> {
        &self.capabilities[class.index()]
    }

    fn failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.failure_paths[class.index()].as_deref()
    }

    fn invalidate_array_dependencies(
        &mut self,
        program: &ResolvedProgram,
        arrays: &crate::hir::HirArrayTypeTable,
        operation: ArrayOperation,
    ) -> bool {
        let mut changed = false;
        for (index, capability) in self.capabilities.iter_mut().enumerate() {
            let HirCopyCapability::Synthesized(copy) = capability else {
                continue;
            };
            let unavailable = copy.fields.iter().find_map(|field| {
                let (field, array) = match field {
                    HirSynthesizedFieldCopy::Array { field, array } => (*field, *array),
                    HirSynthesizedFieldCopy::Optional { field, optional } => {
                        (*field, optional_array_payload(program, *optional)?)
                    }
                    _ => return None,
                };
                let lifecycle = &arrays
                    .get(array)
                    .expect("array dependency must have lifecycle metadata")
                    .lifecycle;
                let available = match operation {
                    ArrayOperation::Copy => lifecycle.copy.is_some(),
                    ArrayOperation::Assignment => lifecycle.assignment.is_some(),
                };
                (!available).then_some(field)
            });
            if let Some(field) = unavailable {
                *capability = HirCopyCapability::Unavailable;
                self.failure_paths[index] = Some(vec![LifecyclePathElement::Field(field)]);
                changed = true;
            }
        }
        changed
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

fn compute_class<I: Copy>(
    class_id: ClassId,
    program: &ResolvedProgram,
    resolved_operation: fn(&ResolvedClassDeclaration) -> ResolvedCopyOperation<I>,
    capabilities: &mut [Option<HirCopyCapability<I>>],
    failure_paths: &mut [Option<Vec<LifecyclePathElement>>],
    states: &mut [VisitState],
    required_constructors: Option<&CapabilitySet<CopyConstructorId>>,
) -> HirCopyCapability<I> {
    match states[class_id.index()] {
        VisitState::Complete => {
            return capabilities[class_id.index()]
                .as_ref()
                .expect("complete capability must exist")
                .clone();
        }
        VisitState::Visiting => return HirCopyCapability::Unavailable,
        VisitState::Unvisited => states[class_id.index()] = VisitState::Visiting,
    }

    let class = program
        .class(class_id)
        .expect("capability class must be resolved");
    let base = class.direct_base.and_then(|direct_base| {
        let nested = compute_class(
            direct_base.class,
            program,
            resolved_operation,
            capabilities,
            failure_paths,
            states,
            required_constructors,
        );
        nested.selected().map(|operation| HirBaseCopy {
            base: direct_base.class,
            operation,
        })
    });
    if let Some(direct_base) = class.direct_base {
        if base.is_none() {
            let mut path = vec![LifecyclePathElement::Base(direct_base.class)];
            if let Some(nested_path) = &failure_paths[direct_base.class.index()] {
                path.extend(nested_path);
            }
            capabilities[class_id.index()] = Some(HirCopyCapability::Unavailable);
            failure_paths[class_id.index()] = Some(path);
            states[class_id.index()] = VisitState::Complete;
            return HirCopyCapability::Unavailable;
        }
    }

    let (capability, failure) = match resolved_operation(class) {
        ResolvedCopyOperation::User(operation) => (
            HirCopyCapability::User(HirUserCopy { operation, base }),
            None,
        ),
        ResolvedCopyOperation::Unavailable => (HirCopyCapability::Unavailable, Some(Vec::new())),
        ResolvedCopyOperation::Synthesized(_) => {
            let mut fields = Vec::with_capacity(class.fields.len());
            let mut failure = None;
            for field in &class.fields {
                match field.type_syntax.kind {
                    ResolvedTypeKind::Class(target) => {
                        let nested = compute_class(
                            target,
                            program,
                            resolved_operation,
                            capabilities,
                            failure_paths,
                            states,
                            required_constructors,
                        );
                        let Some(operation) = nested.selected() else {
                            let mut path = vec![LifecyclePathElement::Field(field.id)];
                            if let Some(nested_path) = &failure_paths[target.index()] {
                                path.extend(nested_path);
                            }
                            failure = Some(path);
                            break;
                        };
                        fields.push(HirSynthesizedFieldCopy::Class {
                            field: field.id,
                            operation,
                        });
                    }
                    ResolvedTypeKind::Shared(_) => {
                        fields.push(HirSynthesizedFieldCopy::Shared { field: field.id });
                    }
                    ResolvedTypeKind::Optional(optional) => {
                        let payload = program
                            .optional_types
                            .get(optional)
                            .expect("resolved optional identities must name table entries")
                            .payload
                            .kind;
                        let payload = match payload {
                            ResolvedTypeKind::I64 => crate::hir::HirPrimitiveType::I64,
                            ResolvedTypeKind::U64 => crate::hir::HirPrimitiveType::U64,
                            ResolvedTypeKind::U8 => crate::hir::HirPrimitiveType::U8,
                            ResolvedTypeKind::F64 => crate::hir::HirPrimitiveType::F64,
                            ResolvedTypeKind::Bool => crate::hir::HirPrimitiveType::Bool,
                            ResolvedTypeKind::Shared(target) => {
                                fields.push(HirSynthesizedFieldCopy::OptionalShared {
                                    field: field.id,
                                    target: crate::typeck::shared::lower_shared_target(target),
                                });
                                continue;
                            }
                            ResolvedTypeKind::Class(target) => {
                                if let Some(constructors) = required_constructors {
                                    if constructors.capability(target).selected().is_none() {
                                        let mut path = vec![LifecyclePathElement::Field(field.id)];
                                        if let Some(nested_path) = constructors.failure(target) {
                                            path.extend(nested_path);
                                        }
                                        failure = Some(path);
                                        break;
                                    }
                                }
                                let nested = compute_class(
                                    target,
                                    program,
                                    resolved_operation,
                                    capabilities,
                                    failure_paths,
                                    states,
                                    required_constructors,
                                );
                                let Some(operation) = nested.selected() else {
                                    let mut path = vec![LifecyclePathElement::Field(field.id)];
                                    if let Some(nested_path) = &failure_paths[target.index()] {
                                        path.extend(nested_path);
                                    }
                                    failure = Some(path);
                                    break;
                                };
                                fields.push(HirSynthesizedFieldCopy::OptionalClass {
                                    field: field.id,
                                    class: target,
                                    operation,
                                });
                                continue;
                            }
                            ResolvedTypeKind::Optional(mut nested) => {
                                let leaf = loop {
                                    let kind = program
                                        .optional_types
                                        .get(nested)
                                        .expect("nested optional identity must exist")
                                        .payload
                                        .kind;
                                    match kind {
                                        ResolvedTypeKind::Optional(inner) => nested = inner,
                                        leaf => break leaf,
                                    }
                                };
                                if let ResolvedTypeKind::Class(target) = leaf {
                                    if let Some(constructors) = required_constructors {
                                        if constructors.capability(target).selected().is_none() {
                                            let mut path =
                                                vec![LifecyclePathElement::Field(field.id)];
                                            if let Some(nested_path) = constructors.failure(target)
                                            {
                                                path.extend(nested_path);
                                            }
                                            failure = Some(path);
                                            break;
                                        }
                                    }
                                    if compute_class(
                                        target,
                                        program,
                                        resolved_operation,
                                        capabilities,
                                        failure_paths,
                                        states,
                                        required_constructors,
                                    )
                                    .selected()
                                    .is_none()
                                    {
                                        let mut path = vec![LifecyclePathElement::Field(field.id)];
                                        if let Some(nested_path) = &failure_paths[target.index()] {
                                            path.extend(nested_path);
                                        }
                                        failure = Some(path);
                                        break;
                                    }
                                }
                                fields.push(HirSynthesizedFieldCopy::Optional {
                                    field: field.id,
                                    optional,
                                });
                                continue;
                            }
                            ResolvedTypeKind::Array(_) => {
                                fields.push(HirSynthesizedFieldCopy::Optional {
                                    field: field.id,
                                    optional,
                                });
                                continue;
                            }
                            ResolvedTypeKind::Unit
                            | ResolvedTypeKind::Obj
                            | ResolvedTypeKind::Interface(_)
                            | ResolvedTypeKind::Function(_) => {
                                failure = Some(vec![LifecyclePathElement::Field(field.id)]);
                                break;
                            }
                        };
                        fields.push(HirSynthesizedFieldCopy::OptionalPrimitive {
                            field: field.id,
                            payload,
                        });
                    }
                    ResolvedTypeKind::Array(array) => {
                        fields.push(HirSynthesizedFieldCopy::Array {
                            field: field.id,
                            array,
                        });
                    }
                    _ => fields.push(HirSynthesizedFieldCopy::Scalar { field: field.id }),
                }
            }
            match failure {
                Some(path) => (HirCopyCapability::Unavailable, Some(path)),
                None => (
                    HirCopyCapability::Synthesized(HirSynthesizedCopy {
                        class: class_id,
                        base,
                        fields,
                        final_fields: if required_constructors.is_some() {
                            class
                                .fields
                                .iter()
                                .filter(|field| field.final_span.is_some())
                                .map(|field| field.id)
                                .collect()
                        } else {
                            Vec::new()
                        },
                    }),
                    None,
                ),
            }
        }
    };

    capabilities[class_id.index()] = Some(capability.clone());
    failure_paths[class_id.index()] = failure;
    states[class_id.index()] = VisitState::Complete;
    capability
}

fn optional_array_payload(
    program: &ResolvedProgram,
    mut optional: crate::identity::OptionalTypeId,
) -> Option<crate::identity::ArrayTypeId> {
    loop {
        match program.optional_types.get(optional)?.payload.kind {
            ResolvedTypeKind::Optional(nested) => optional = nested,
            ResolvedTypeKind::Array(array) => return Some(array),
            _ => return None,
        }
    }
}

//! Deterministic copy-capability analysis over inline class fields.

use crate::{
    hir::{
        HirBaseCopy, HirCopyCapability, HirSynthesizedCopy, HirSynthesizedFieldCopy, HirUserCopy,
    },
    identity::{ClassId, CopyAssignmentId, CopyConstructorId, FieldId},
    resolve::{ResolvedClassDeclaration, ResolvedCopyOperation, ResolvedProgram, ResolvedTypeKind},
};

#[derive(Clone, Debug)]
pub(super) struct CopyCapabilities {
    constructors: CapabilitySet<CopyConstructorId>,
    assignments: CapabilitySet<CopyAssignmentId>,
    array_types: crate::hir::HirArrayTypeTable,
}

impl CopyCapabilities {
    pub(super) fn compute(program: &ResolvedProgram) -> Self {
        let mut constructors =
            CapabilitySet::compute(program, |class| class.copy_constructor, None);
        let provisional_assignments =
            CapabilitySet::compute(program, |class| class.copy_assignment, Some(&constructors));
        loop {
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
        loop {
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

    pub(crate) fn constructor_failure(&self, class: ClassId) -> Option<&[CopyPathElement]> {
        self.constructors.failure(class)
    }

    pub(crate) fn assignment_failure(&self, class: ClassId) -> Option<&[CopyPathElement]> {
        self.assignments.failure(class)
    }
}

#[derive(Clone, Debug)]
struct CapabilitySet<I> {
    capabilities: Vec<HirCopyCapability<I>>,
    /// The deterministic outer-to-inner field path responsible for an
    /// unavailable synthesized operation. An empty path means the class's own
    /// resolved operation is unavailable.
    failure_paths: Vec<Option<Vec<CopyPathElement>>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CopyPathElement {
    Base(ClassId),
    Field(FieldId),
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

    fn failure(&self, class: ClassId) -> Option<&[CopyPathElement]> {
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
                self.failure_paths[index] = Some(vec![CopyPathElement::Field(field)]);
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
    failure_paths: &mut [Option<Vec<CopyPathElement>>],
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
            let mut path = vec![CopyPathElement::Base(direct_base.class)];
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
                            let mut path = vec![CopyPathElement::Field(field.id)];
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
                                        let mut path = vec![CopyPathElement::Field(field.id)];
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
                                    let mut path = vec![CopyPathElement::Field(field.id)];
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
                                            let mut path = vec![CopyPathElement::Field(field.id)];
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
                                        let mut path = vec![CopyPathElement::Field(field.id)];
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
                                failure = Some(vec![CopyPathElement::Field(field.id)]);
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

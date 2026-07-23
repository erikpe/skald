//! Deterministic copy-capability analysis over inline class fields.

use crate::{
    hir::{
        HirBaseCopy, HirCopyCapability, HirSynthesizedCopy, HirSynthesizedFieldCopy, HirUserCopy,
    },
    identity::{ClassId, CopyAssignmentId, FieldId, InitializerId},
    resolve::{ResolvedClassDeclaration, ResolvedCopyOperation, ResolvedProgram, ResolvedTypeKind},
};

#[derive(Clone, Debug)]
pub(super) struct CopyCapabilities {
    constructors: CapabilitySet<InitializerId>,
    assignments: CapabilitySet<CopyAssignmentId>,
}

impl CopyCapabilities {
    pub(super) fn compute(program: &ResolvedProgram) -> Self {
        Self {
            constructors: CapabilitySet::compute(program, |class| class.copy_constructor),
            assignments: CapabilitySet::compute(program, |class| class.copy_assignment),
        }
    }

    pub(super) fn constructor(&self, class: ClassId) -> &HirCopyCapability<InitializerId> {
        self.constructors.capability(class)
    }

    pub(super) fn assignment(&self, class: ClassId) -> &HirCopyCapability<CopyAssignmentId> {
        self.assignments.capability(class)
    }

    pub(super) fn constructor_failure(&self, class: ClassId) -> Option<&[CopyPathElement]> {
        self.constructors.failure(class)
    }

    pub(super) fn assignment_failure(&self, class: ClassId) -> Option<&[CopyPathElement]> {
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
pub(super) enum CopyPathElement {
    Base(ClassId),
    Field(FieldId),
}

impl<I: Copy> CapabilitySet<I> {
    fn compute(
        program: &ResolvedProgram,
        resolved_operation: fn(&ResolvedClassDeclaration) -> ResolvedCopyOperation<I>,
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
                    _ => fields.push(HirSynthesizedFieldCopy::Primitive { field: field.id }),
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

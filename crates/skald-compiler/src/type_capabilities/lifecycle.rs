//! Phase-neutral lifecycle availability over a closed resolved program.

use crate::{
    identity::{ArrayTypeId, ClassId, FieldId},
    resolve::{ResolvedCopyOperation, ResolvedProgram, ResolvedTypeKind},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LifecyclePathElement {
    Base(ClassId),
    Field(FieldId),
}

#[derive(Clone, Debug)]
pub(crate) struct ResolvedLifecycleCapabilities {
    constructors: CapabilitySet,
    assignments: CapabilitySet,
    array_copy: Vec<bool>,
    array_assignment: Vec<bool>,
}

impl ResolvedLifecycleCapabilities {
    pub(crate) fn compute(program: &ResolvedProgram) -> Self {
        let mut constructors = CapabilitySet::compute(program, Operation::Constructor, None);
        loop {
            let array_copy =
                array_capabilities(program, &constructors, None, Operation::Constructor);
            if !constructors.invalidate_array_dependencies(program, &array_copy) {
                break;
            }
        }

        let mut assignments =
            CapabilitySet::compute(program, Operation::Assignment, Some(&constructors));
        loop {
            let array_assignment = array_capabilities(
                program,
                &constructors,
                Some(&assignments),
                Operation::Assignment,
            );
            if !assignments.invalidate_array_dependencies(program, &array_assignment) {
                break;
            }
        }

        let array_copy = array_capabilities(program, &constructors, None, Operation::Constructor);
        let array_assignment = array_capabilities(
            program,
            &constructors,
            Some(&assignments),
            Operation::Assignment,
        );
        Self {
            constructors,
            assignments,
            array_copy,
            array_assignment,
        }
    }

    pub(crate) fn constructor(&self, class: ClassId) -> bool {
        self.constructors.available(class)
    }

    pub(crate) fn assignment(&self, class: ClassId) -> bool {
        self.assignments.available(class)
    }

    pub(crate) fn constructor_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.constructors.failure(class)
    }

    pub(crate) fn assignment_failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.assignments.failure(class)
    }

    pub(crate) fn array_copy(&self, array: ArrayTypeId) -> bool {
        self.array_copy[array.index()]
    }

    pub(crate) fn array_assignment(&self, array: ArrayTypeId) -> bool {
        self.array_assignment[array.index()]
    }
}

#[derive(Clone, Debug)]
struct CapabilitySet {
    available: Vec<bool>,
    failure_paths: Vec<Option<Vec<LifecyclePathElement>>>,
    synthesized: Vec<bool>,
}

impl CapabilitySet {
    fn compute(
        program: &ResolvedProgram,
        operation: Operation,
        constructors: Option<&Self>,
    ) -> Self {
        let mut capabilities = Self {
            available: vec![false; program.classes.len()],
            failure_paths: vec![None; program.classes.len()],
            synthesized: vec![false; program.classes.len()],
        };
        let mut states = vec![VisitState::Unvisited; program.classes.len()];
        for class in program.classes.iter() {
            compute_class(
                class.id,
                program,
                operation,
                constructors,
                &mut capabilities,
                &mut states,
            );
        }
        capabilities
    }

    fn available(&self, class: ClassId) -> bool {
        self.available[class.index()]
    }

    fn failure(&self, class: ClassId) -> Option<&[LifecyclePathElement]> {
        self.failure_paths[class.index()].as_deref()
    }

    fn invalidate_array_dependencies(
        &mut self,
        program: &ResolvedProgram,
        arrays: &[bool],
    ) -> bool {
        let mut changed = false;
        for class in program.classes.iter() {
            if !self.available(class.id) || !self.synthesized[class.id.index()] {
                continue;
            }
            let unavailable = class.fields.iter().find_map(|field| {
                field_array_dependency(program, field.type_syntax.kind)
                    .filter(|array| !arrays[array.index()])
                    .map(|_| field.id)
            });
            if let Some(field) = unavailable {
                self.available[class.id.index()] = false;
                self.failure_paths[class.id.index()] =
                    Some(vec![LifecyclePathElement::Field(field)]);
                changed = true;
            }
        }
        changed
    }
}

#[derive(Clone, Copy)]
enum Operation {
    Constructor,
    Assignment,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

fn compute_class(
    class: ClassId,
    program: &ResolvedProgram,
    operation: Operation,
    constructors: Option<&CapabilitySet>,
    capabilities: &mut CapabilitySet,
    states: &mut [VisitState],
) -> bool {
    match states[class.index()] {
        VisitState::Complete => return capabilities.available(class),
        VisitState::Visiting => return false,
        VisitState::Unvisited => states[class.index()] = VisitState::Visiting,
    }

    let declaration = program
        .class(class)
        .expect("capability class must be resolved");
    if let Some(base) = declaration.direct_base {
        if !compute_class(
            base.class,
            program,
            operation,
            constructors,
            capabilities,
            states,
        ) {
            let mut path = vec![LifecyclePathElement::Base(base.class)];
            if let Some(nested) = capabilities.failure(base.class) {
                path.extend_from_slice(nested);
            }
            return complete_unavailable(class, path, capabilities, states);
        }
    }

    let resolved = match operation {
        Operation::Constructor => operation_kind(declaration.copy_constructor),
        Operation::Assignment => operation_kind(declaration.copy_assignment),
    };
    match resolved {
        OperationKind::Unavailable => complete_unavailable(class, Vec::new(), capabilities, states),
        OperationKind::User => complete_available(class, false, capabilities, states),
        OperationKind::Synthesized => {
            capabilities.synthesized[class.index()] = true;
            for field in &declaration.fields {
                let dependency = field_class_dependency(program, field.type_syntax.kind);
                let Some((target, inside_optional)) = dependency else {
                    if optional_payload_is_invalid(program, field.type_syntax.kind) {
                        return complete_unavailable(
                            class,
                            vec![LifecyclePathElement::Field(field.id)],
                            capabilities,
                            states,
                        );
                    }
                    continue;
                };
                if inside_optional
                    && constructors.is_some_and(|constructors| !constructors.available(target))
                {
                    let mut path = vec![LifecyclePathElement::Field(field.id)];
                    if let Some(nested) = constructors.and_then(|set| set.failure(target)) {
                        path.extend_from_slice(nested);
                    }
                    return complete_unavailable(class, path, capabilities, states);
                }
                if !compute_class(
                    target,
                    program,
                    operation,
                    constructors,
                    capabilities,
                    states,
                ) {
                    let mut path = vec![LifecyclePathElement::Field(field.id)];
                    if let Some(nested) = capabilities.failure(target) {
                        path.extend_from_slice(nested);
                    }
                    return complete_unavailable(class, path, capabilities, states);
                }
            }
            complete_available(class, true, capabilities, states)
        }
    }
}

#[derive(Clone, Copy)]
enum OperationKind {
    Unavailable,
    User,
    Synthesized,
}

fn operation_kind<I>(operation: ResolvedCopyOperation<I>) -> OperationKind {
    match operation {
        ResolvedCopyOperation::Unavailable => OperationKind::Unavailable,
        ResolvedCopyOperation::User(_) => OperationKind::User,
        ResolvedCopyOperation::Synthesized(_) => OperationKind::Synthesized,
    }
}

fn complete_available(
    class: ClassId,
    synthesized: bool,
    capabilities: &mut CapabilitySet,
    states: &mut [VisitState],
) -> bool {
    capabilities.available[class.index()] = true;
    capabilities.synthesized[class.index()] = synthesized;
    states[class.index()] = VisitState::Complete;
    true
}

fn complete_unavailable(
    class: ClassId,
    path: Vec<LifecyclePathElement>,
    capabilities: &mut CapabilitySet,
    states: &mut [VisitState],
) -> bool {
    capabilities.available[class.index()] = false;
    capabilities.failure_paths[class.index()] = Some(path);
    states[class.index()] = VisitState::Complete;
    false
}

fn field_class_dependency(
    program: &ResolvedProgram,
    kind: ResolvedTypeKind,
) -> Option<(ClassId, bool)> {
    match kind {
        ResolvedTypeKind::Class(class) => Some((class, false)),
        ResolvedTypeKind::Optional(optional) => match optional_leaf(program, optional) {
            ResolvedTypeKind::Class(class) => Some((class, true)),
            _ => None,
        },
        _ => None,
    }
}

fn field_array_dependency(
    program: &ResolvedProgram,
    kind: ResolvedTypeKind,
) -> Option<ArrayTypeId> {
    match kind {
        ResolvedTypeKind::Array(array) => Some(array),
        ResolvedTypeKind::Optional(optional) => match optional_leaf(program, optional) {
            ResolvedTypeKind::Array(array) => Some(array),
            _ => None,
        },
        _ => None,
    }
}

fn optional_payload_is_invalid(program: &ResolvedProgram, kind: ResolvedTypeKind) -> bool {
    let ResolvedTypeKind::Optional(optional) = kind else {
        return false;
    };
    matches!(
        optional_leaf(program, optional),
        ResolvedTypeKind::Unit
            | ResolvedTypeKind::Obj
            | ResolvedTypeKind::Interface(_)
            | ResolvedTypeKind::Function(_)
    )
}

fn optional_leaf(
    program: &ResolvedProgram,
    mut optional: crate::identity::OptionalTypeId,
) -> ResolvedTypeKind {
    loop {
        let kind = program
            .optional_types
            .get(optional)
            .expect("optional identity must name resolved metadata")
            .payload
            .kind;
        match kind {
            ResolvedTypeKind::Optional(nested) => optional = nested,
            leaf => return leaf,
        }
    }
}

fn array_capabilities(
    program: &ResolvedProgram,
    constructors: &CapabilitySet,
    assignments: Option<&CapabilitySet>,
    operation: Operation,
) -> Vec<bool> {
    let mut arrays = Vec::with_capacity(program.array_types.len());
    for array in program.array_types.iter() {
        arrays.push(type_lifecycle_available(
            program,
            constructors,
            assignments,
            &arrays,
            array.element.kind,
            operation,
            false,
        ));
    }
    arrays
}

fn type_lifecycle_available(
    program: &ResolvedProgram,
    constructors: &CapabilitySet,
    assignments: Option<&CapabilitySet>,
    arrays: &[bool],
    kind: ResolvedTypeKind,
    operation: Operation,
    inside_optional: bool,
) -> bool {
    match kind {
        ResolvedTypeKind::I64
        | ResolvedTypeKind::U64
        | ResolvedTypeKind::U8
        | ResolvedTypeKind::F64
        | ResolvedTypeKind::Bool
        | ResolvedTypeKind::Shared(_) => true,
        ResolvedTypeKind::Class(class) => match operation {
            Operation::Constructor => constructors.available(class),
            Operation::Assignment => {
                (!inside_optional || constructors.available(class))
                    && assignments.is_some_and(|assignments| assignments.available(class))
            }
        },
        ResolvedTypeKind::Array(array) => arrays
            .get(array.index())
            .copied()
            .expect("nested array identities must precede containing identities"),
        ResolvedTypeKind::Optional(optional) => {
            let payload = program
                .optional_types
                .get(optional)
                .expect("optional identity must name resolved metadata")
                .payload
                .kind;
            type_lifecycle_available(
                program,
                constructors,
                assignments,
                arrays,
                payload,
                operation,
                true,
            )
        }
        ResolvedTypeKind::Unit
        | ResolvedTypeKind::Obj
        | ResolvedTypeKind::Interface(_)
        | ResolvedTypeKind::Function(_) => false,
    }
}

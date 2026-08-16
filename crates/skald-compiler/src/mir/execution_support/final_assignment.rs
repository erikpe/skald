//! Identification of complete-value assignments that require final-update evidence.

use std::collections::BTreeSet;

use crate::{identity::ClassId, source::Span};

use super::super::{MirArrayInstruction, MirInstruction, MirProgram, MirType};

pub(super) fn unsupported_assignment_spans(program: &MirProgram) -> Vec<Span> {
    let mut classifier = FinalRepresentationClassifier::new(program);
    let mut spans = Vec::new();
    let mut seen = BTreeSet::new();
    for definition in program.executable_definitions() {
        for instruction in definition
            .body()
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
        {
            let span = match instruction {
                MirInstruction::CopyAssign(copy) if classifier.class_contains_final(copy.class) => {
                    Some(copy.span)
                }
                MirInstruction::ClassOptionalAssign(assignment)
                    if assignment.copy_assignment.is_some()
                        && classifier.class_contains_final(assignment.class) =>
                {
                    Some(assignment.span)
                }
                MirInstruction::AggregateOptionalAssign(assignment)
                    if classifier.optional_contains_final(assignment.optional) =>
                {
                    Some(assignment.span)
                }
                MirInstruction::Array(
                    operation @ (MirArrayInstruction::ElementAssign { .. }
                    | MirArrayInstruction::SliceAssignNext { .. }),
                ) if array_assignment_contains_final(&mut classifier, operation) => {
                    Some(operation.span())
                }
                _ => None,
            };
            let Some(span) = span else {
                continue;
            };
            let key = (
                span.source_id().index(),
                span.range().start(),
                span.range().end(),
            );
            if seen.insert(key) {
                spans.push(span);
            }
        }
    }
    spans
}

fn array_assignment_contains_final(
    classifier: &mut FinalRepresentationClassifier<'_>,
    operation: &MirArrayInstruction,
) -> bool {
    let operation = match operation {
        MirArrayInstruction::ElementAssign { operation, .. }
        | MirArrayInstruction::SliceAssignNext { operation, .. } => *operation,
        _ => return false,
    };
    match operation {
        super::super::MirArrayAssignElement::Class { class, .. }
        | super::super::MirArrayAssignElement::OptionalClass { class, .. } => {
            classifier.class_contains_final(class)
        }
        super::super::MirArrayAssignElement::Array(array) => classifier.array_contains_final(array),
        super::super::MirArrayAssignElement::Optional(optional) => {
            classifier.optional_contains_final(optional)
        }
        super::super::MirArrayAssignElement::Primitive
        | super::super::MirArrayAssignElement::OptionalPrimitive
        | super::super::MirArrayAssignElement::Shared(_)
        | super::super::MirArrayAssignElement::OptionalShared(_) => false,
    }
}

#[derive(Clone, Copy)]
enum ClassState {
    Unknown,
    Visiting,
    Complete(bool),
}

struct FinalRepresentationClassifier<'program> {
    program: &'program MirProgram,
    classes: Vec<ClassState>,
}

impl<'program> FinalRepresentationClassifier<'program> {
    fn new(program: &'program MirProgram) -> Self {
        Self {
            program,
            classes: vec![ClassState::Unknown; program.classes.len()],
        }
    }

    fn class_contains_final(&mut self, class: ClassId) -> bool {
        match self.classes.get(class.index()).copied() {
            Some(ClassState::Complete(result)) => return result,
            Some(ClassState::Visiting) | None => return false,
            Some(ClassState::Unknown) => {}
        }
        self.classes[class.index()] = ClassState::Visiting;
        let declaration = self
            .program
            .class(class)
            .expect("verified assignment class must exist");
        let direct_final = declaration
            .fields
            .iter()
            .any(|field| field.final_span.is_some());
        if direct_final {
            self.classes[class.index()] = ClassState::Complete(true);
            return true;
        }
        let base_final = declaration
            .direct_base
            .is_some_and(|base| self.class_contains_final(base.class));
        if base_final {
            self.classes[class.index()] = ClassState::Complete(true);
            return true;
        }
        let field_final = declaration
            .fields
            .iter()
            .any(|field| self.type_contains_final(field.ty));
        self.classes[class.index()] = ClassState::Complete(field_final);
        field_final
    }

    fn type_contains_final(&mut self, ty: MirType) -> bool {
        match ty {
            MirType::Class(class) => self.class_contains_final(class),
            MirType::Optional(optional) => self.optional_contains_final(optional),
            MirType::Array(array) => self.array_contains_final(array),
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::F64
            | MirType::Bool
            | MirType::Unit
            | MirType::Obj
            | MirType::Interface(_)
            | MirType::Shared(_)
            | MirType::Function(_) => false,
        }
    }

    fn optional_contains_final(&mut self, optional: crate::identity::OptionalTypeId) -> bool {
        self.program
            .optional_type(optional)
            .is_some_and(|metadata| self.type_contains_final(metadata.payload))
    }

    fn array_contains_final(&mut self, array: crate::identity::ArrayTypeId) -> bool {
        self.program
            .array_type(array)
            .is_some_and(|metadata| self.type_contains_final(metadata.element))
    }
}

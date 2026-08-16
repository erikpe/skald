//! Queries over typed private-cell write authorization.

use crate::{
    hir::{
        HirBlock, HirFieldPlace, HirFieldWriteAuthorization, HirOptionalStorage, HirProgram,
        HirStatement,
    },
    identity::FieldId,
    source::Span,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct HirCellWrite {
    pub field: FieldId,
    pub span: Span,
}

pub(crate) fn collect_cell_writes(program: &HirProgram) -> Vec<HirCellWrite> {
    let mut writes = Vec::new();
    for definition in program.definitions.iter() {
        collect_block(&definition.body, &mut writes);
    }
    for class in program.class_definitions.iter() {
        for definition in class
            .initializers
            .iter()
            .chain(class.copy_constructor.iter())
            .chain(class.copy_assignment.iter())
            .chain(class.destructor.iter())
            .chain(class.methods.iter())
        {
            collect_block(&definition.body, &mut writes);
        }
    }
    writes
}

fn collect_block(block: &HirBlock, writes: &mut Vec<HirCellWrite>) {
    for statement in &block.statements {
        if let Some(place) = field_write_place(statement) {
            if place.write_authorization == Some(HirFieldWriteAuthorization::DeclaringClassCell) {
                writes.push(HirCellWrite {
                    field: place.field,
                    span: statement.span(),
                });
            }
        }
        match statement {
            HirStatement::Conditional(conditional) => {
                for arm in &conditional.arms {
                    collect_block(&arm.body, writes);
                }
                if let Some(block) = &conditional.else_block {
                    collect_block(block, writes);
                }
            }
            HirStatement::While(statement) => collect_block(&statement.body, writes),
            HirStatement::Block(block) => collect_block(block, writes),
            _ => {}
        }
    }
}

fn field_write_place(statement: &HirStatement) -> Option<&HirFieldPlace> {
    match statement {
        HirStatement::FieldAssignment(statement) => Some(&statement.place),
        HirStatement::FieldCopyAssignment(statement) => Some(&statement.place),
        HirStatement::SharedFieldWrite(statement) => Some(&statement.place),
        HirStatement::OptionalAssignment(statement) => {
            optional_field(&statement.destination.storage)
        }
        HirStatement::ClassOptionalAssignment(statement) => {
            optional_field(&statement.destination.storage)
        }
        HirStatement::OptionalSharedAssignment(statement) => {
            optional_field(&statement.destination.storage)
        }
        HirStatement::AggregateOptionalAssignment(statement) => {
            optional_field(&statement.destination.storage)
        }
        HirStatement::ArrayAssignment(statement) => match &statement.destination {
            crate::hir::HirArrayPlace::Field { place, .. } => Some(place),
            _ => None,
        },
        _ => None,
    }
}

fn optional_field(storage: &HirOptionalStorage) -> Option<&HirFieldPlace> {
    match storage {
        HirOptionalStorage::Field(place) => Some(place),
        _ => None,
    }
}

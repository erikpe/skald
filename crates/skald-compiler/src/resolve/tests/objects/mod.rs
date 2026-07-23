use super::*;
use crate::identity::FieldId;
use crate::object_path::ObjectProjection;

fn class(output: &ResolveOutput, index: usize) -> &ResolvedClassDeclaration {
    output
        .program
        .classes
        .get(ClassId::new(index))
        .expect("expected resolved class")
}

mod declarations_lifecycle;
mod diagnostics;
mod dumps;
mod member_lookup;
mod object_places;
mod virtual_methods;

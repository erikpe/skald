//! Structural verification for canonical planned lifecycle data.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::{StaticFieldId, StaticInitializerId},
    mir::{
        MirStaticFieldInitialization, MirStaticLifecycleDefinition, MirType, MirVerificationError,
        PreliminaryMirStaticField, PreliminaryMirStaticInitializer,
    },
    source::Span,
};

use super::{super::plan::PlannedMirProgram, program_error};

pub(super) fn verify(program: &PlannedMirProgram, errors: &mut Vec<MirVerificationError>) {
    let index = PlannedVerificationIndex::new(program, errors);
    verify_activation_order(program, &index.declared_fields, errors);
    verify_definitions(program, &index, errors);
}

fn verify_activation_order(
    program: &PlannedMirProgram,
    declared: &BTreeSet<StaticFieldId>,
    errors: &mut Vec<MirVerificationError>,
) {
    let activation = program.lifecycle().activation();
    let unique = activation.iter().copied().collect::<BTreeSet<_>>();
    if activation.len() != declared.len() || unique != *declared {
        program_error(
            errors,
            "static activation order does not cover every field exactly once",
        );
    }
}

fn verify_definitions(
    program: &PlannedMirProgram,
    index: &PlannedVerificationIndex<'_>,
    errors: &mut Vec<MirVerificationError>,
) {
    let definitions = program.lifecycle_mir().definitions();
    for pair in definitions.windows(2) {
        match pair[0].field.cmp(&pair[1].field) {
            std::cmp::Ordering::Equal => program_error(
                errors,
                format!("duplicate lifecycle definition for {}", pair[0].field),
            ),
            std::cmp::Ordering::Greater => program_error(
                errors,
                "lifecycle definition table is not in canonical field order",
            ),
            std::cmp::Ordering::Less => {}
        }
    }
    if definitions.len() != index.fields.len() {
        program_error(
            errors,
            "lifecycle definition table does not cover every static field",
        );
    }

    let mut definition_fields = BTreeSet::new();
    for definition in definitions {
        definition_fields.insert(definition.field);
        let Some(field) = index.fields.get(&definition.field).copied() else {
            program_error(
                errors,
                format!(
                    "lifecycle definition names foreign static field {}",
                    definition.field
                ),
            );
            continue;
        };
        verify_definition(definition, field, index, errors);
    }
    if definition_fields != index.declared_fields {
        program_error(
            errors,
            "lifecycle definition table does not cover every static field",
        );
    }
}

fn verify_definition(
    definition: &MirStaticLifecycleDefinition,
    field: &PreliminaryMirStaticField,
    index: &PlannedVerificationIndex<'_>,
    errors: &mut Vec<MirVerificationError>,
) {
    let expected_mode = field.initializer.map_or(
        MirStaticFieldInitialization::ZeroDefault,
        MirStaticFieldInitialization::Explicit,
    );
    if field.final_span.is_some() && field.initializer.is_none() {
        program_error(
            errors,
            format!(
                "final static field {} cannot use zero-default lifecycle activation",
                field.field
            ),
        );
    }
    if definition.ty != field.ty
        || definition.initialization != expected_mode
        || definition.final_span != field.final_span
        || definition.span != field.span
    {
        program_error(
            errors,
            format!(
                "lifecycle definition for {} disagrees with its declaration",
                field.field
            ),
        );
    }
    if !index
        .declarations
        .get(&field.field)
        .is_some_and(|declaration| {
            declaration.ty == field.ty
                && declaration.initialization == expected_mode
                && declaration.final_span == field.final_span
                && declaration.span == field.span
        })
    {
        program_error(
            errors,
            format!(
                "static field {} has an inconsistent declaration",
                field.field
            ),
        );
    }
    if let MirStaticFieldInitialization::Explicit(initializer) = definition.initialization {
        let valid = index
            .initializers
            .get(&initializer)
            .is_some_and(|body| body.field == field.field && body.destination_type == field.ty);
        if !valid {
            program_error(
                errors,
                format!(
                    "lifecycle definition for {} has no matching initializer body",
                    field.field
                ),
            );
        }
    }
}

struct PlannedVerificationIndex<'mir> {
    fields: BTreeMap<StaticFieldId, &'mir PreliminaryMirStaticField>,
    declarations: BTreeMap<StaticFieldId, IndexedDeclaration>,
    initializers: BTreeMap<StaticInitializerId, &'mir PreliminaryMirStaticInitializer>,
    declared_fields: BTreeSet<StaticFieldId>,
}

impl<'mir> PlannedVerificationIndex<'mir> {
    fn new(program: &'mir PlannedMirProgram, errors: &mut Vec<MirVerificationError>) -> Self {
        let mut fields = BTreeMap::new();
        for field in program.static_fields() {
            if fields.insert(field.field, field).is_some() {
                program_error(
                    errors,
                    format!("static inventory contains duplicate field {}", field.field),
                );
            }
        }
        let declarations = program
            .preliminary()
            .program()
            .classes
            .iter()
            .flat_map(|class| {
                class.static_fields.iter().map(|field| {
                    (
                        field.id,
                        IndexedDeclaration {
                            ty: field.ty,
                            initialization: field.initialization,
                            final_span: field.final_span,
                            span: field.span,
                        },
                    )
                })
            })
            .collect::<BTreeMap<_, _>>();
        for field in fields.keys() {
            if !declarations.contains_key(field) {
                program_error(
                    errors,
                    format!("static inventory names undeclared field {field}"),
                );
            }
        }
        let mut initializers = BTreeMap::new();
        for initializer in program.static_initializers() {
            if initializers.insert(initializer.id, initializer).is_some() {
                program_error(
                    errors,
                    format!(
                        "static initializer inventory contains duplicate body {}",
                        initializer.id
                    ),
                );
            }
        }
        let declared_fields = declarations.keys().copied().collect();
        Self {
            fields,
            declarations,
            initializers,
            declared_fields,
        }
    }
}

#[derive(Clone, Copy)]
struct IndexedDeclaration {
    ty: MirType,
    initialization: MirStaticFieldInitialization,
    final_span: Option<Span>,
    span: Span,
}

//! Resolution of generated callable bodies through the ordinary body resolver.

use super::super::class::{ClassWorkItem, InitializerWorkItem, StaticInitializerWorkItem};
use super::*;
use crate::diagnostics::{Label, LabelStyle};

pub(super) fn specialize_bodies(
    input: SpecializationBodyInput<'_, '_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> SpecializedBodies {
    let mut output = SpecializedBodies {
        definitions: Vec::new(),
        static_initializers: Vec::new(),
        valid: true,
    };

    for specialization in input.specializations.iter() {
        let GenericSpecializationState::Complete(class_id) = specialization.state else {
            output.valid = false;
            continue;
        };
        let Some((unit, source, ast_index)) =
            template_source(input.units, specialization.key.template)
        else {
            unreachable!("specialization keys reference collected templates")
        };
        let Some(declaration) = input.classes.get(class_id) else {
            output.valid = false;
            continue;
        };
        let semantics = input
            .semantics
            .get(specialization.key.template)
            .expect("specialization keys reference template semantics");
        let work = generated_work_item(declaration, source, unit.module, ast_index);
        let environment = BodyResolutionEnvironment::new(
            input.lookups.for_unit(unit, input.modules),
            input.functions,
            input.classes,
            input.interfaces,
            input.hierarchy,
            input.has_module_context,
            input.string_literals,
        )
        .with_specialization(BodySpecializationEnvironment::new(
            semantics,
            specialization,
        ));

        let mut body_diagnostics = Diagnostics::new();
        let static_initializers = resolve_static_field_initializers(
            unit.ast,
            std::slice::from_ref(&work),
            input.classes,
            environment,
            type_interner,
            &mut body_diagnostics,
        );
        let mut definitions = resolve_class_bodies(
            unit.ast,
            std::slice::from_ref(&work),
            input.classes,
            environment,
            type_interner,
            &mut body_diagnostics,
        );

        if body_diagnostics.has_errors() {
            output.valid = false;
            append_specialization_diagnostics(
                diagnostics,
                body_diagnostics,
                specialization,
                source,
            );
            continue;
        }
        output.static_initializers.extend(static_initializers);
        output.definitions.append(&mut definitions);
    }

    if !output.valid {
        output.definitions.clear();
        output.static_initializers.clear();
    }
    output
}

fn generated_work_item(
    declaration: &ResolvedClassDeclaration,
    source: &syntax::ClassDecl,
    module: ModuleId,
    ast_index: usize,
) -> ClassWorkItem {
    let mut static_field = 0usize;
    let mut initializer = 0usize;
    let mut work = ClassWorkItem {
        id: declaration.id,
        module,
        ast_index,
        static_initializer_members: Vec::new(),
        initializer_members: Vec::new(),
        copy_constructor_member: None,
        copy_assignment_member: None,
        destructor_member: None,
        method_members: Vec::new(),
    };

    for (member_index, member) in source.members.iter().enumerate() {
        match member {
            syntax::ClassMember::Field(_) => {}
            syntax::ClassMember::StaticField(field) => {
                if field.initializer.is_some() {
                    work.static_initializer_members
                        .push(StaticInitializerWorkItem {
                            id: declaration.static_fields[static_field].id.into(),
                            member_index,
                        });
                }
                static_field += 1;
            }
            syntax::ClassMember::Initializer(_) => {
                work.initializer_members.push(InitializerWorkItem {
                    id: declaration.initializers[initializer].id,
                    member_index,
                });
                initializer += 1;
            }
            syntax::ClassMember::CopyConstructor(_) => {
                work.copy_constructor_member = Some(member_index)
            }
            syntax::ClassMember::CopyAssignment(_) => {
                work.copy_assignment_member = Some(member_index)
            }
            syntax::ClassMember::Destructor(_) => work.destructor_member = Some(member_index),
            syntax::ClassMember::Method(_) => work.method_members.push(member_index),
        }
    }
    work
}

fn append_specialization_diagnostics(
    diagnostics: &mut Diagnostics,
    body_diagnostics: Diagnostics,
    specialization: &GenericSpecialization,
    source: &syntax::ClassDecl,
) {
    let origin = specialization
        .provenance
        .origins
        .first()
        .expect("requested specialization retains an application origin");
    for mut diagnostic in body_diagnostics.into_vec() {
        for label in &mut diagnostic.labels {
            label.style = LabelStyle::Secondary;
        }
        diagnostic.labels.insert(
            0,
            Label {
                style: LabelStyle::Primary,
                span: origin.span,
                message: "this application specializes the invalid body".to_owned(),
            },
        );
        diagnostic.labels.push(Label {
            style: LabelStyle::Secondary,
            span: source.name.span,
            message: "template declared here".to_owned(),
        });
        diagnostics.push(diagnostic);
    }
}

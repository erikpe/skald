//! Materialization of closed interface requirements into ordinary declarations.

use super::super::resolver::ModuleUnit;
use super::names::{InterfaceNameSources, SpecializationNameRenderer};
use super::*;

pub(in crate::resolve::resolver::program) struct InterfaceMaterializationInput<'program, 'ast> {
    pub(in crate::resolve::resolver::program) units: &'program [ModuleUnit<'ast>],
    pub(in crate::resolve::resolver::program) modules: &'program ProgramModuleTable,
    pub(in crate::resolve::resolver::program) templates: &'program ResolvedInterfaceTemplateTable,
    pub(in crate::resolve::resolver::program) semantics:
        &'program ResolvedInterfaceTemplateSemanticTable,
    pub(in crate::resolve::resolver::program) class_specializations:
        &'program GenericSpecializationTable,
    pub(in crate::resolve::resolver::program) ordinary_interfaces:
        &'program ResolvedInterfaceDeclarationTable,
    pub(in crate::resolve::resolver::program) type_interner: &'program ResolvedTypeInterner,
}

pub(in crate::resolve::resolver::program) struct MaterializedInterfaces {
    pub(in crate::resolve::resolver::program) declarations: Vec<ResolvedInterfaceDeclaration>,
    pub(in crate::resolve::resolver::program) valid: bool,
}

pub(in crate::resolve::resolver::program) fn materialize_interface_declarations(
    input: InterfaceMaterializationInput<'_, '_>,
    specializations: &mut GenericInterfaceSpecializationTable,
    diagnostics: &mut Diagnostics,
) -> MaterializedInterfaces {
    if specializations.iter().any(|specialization| {
        !matches!(
            specialization.state,
            GenericInterfaceSpecializationState::Complete(_)
        )
    }) {
        specializations.fail_all();
        return MaterializedInterfaces {
            declarations: Vec::new(),
            valid: false,
        };
    }

    let ordinary_classes = ResolvedClassDeclarationTable::default();
    let names = SpecializationNameRenderer::new(
        input.units,
        input.modules,
        input.class_specializations,
        Some(InterfaceNameSources {
            specializations,
            templates: input.templates,
        }),
        &ordinary_classes,
        input.ordinary_interfaces,
        input.type_interner,
    );
    let mut declarations = Vec::new();
    let mut mappings = Vec::new();
    let mut valid = true;

    for specialization in specializations.iter() {
        let GenericInterfaceSpecializationState::Complete(interface) = specialization.state else {
            unreachable!("non-complete entries are rejected before materialization")
        };
        let template = input
            .templates
            .get(specialization.key.template)
            .expect("specialization key references a template declaration");
        let semantics = input
            .semantics
            .get(specialization.key.template)
            .expect("specialization key references template semantics");
        if !semantics.valid {
            report_invalid_template(specialization, template, diagnostics);
            valid = false;
            continue;
        }

        let mut requirements = Vec::with_capacity(semantics.requirements.len());
        let mut requirement_mappings = Vec::with_capacity(semantics.requirements.len());
        for signature in &semantics.requirements {
            let closed = InterfaceTypeUseLookup::new(semantics, specialization);
            let mut parameters = Vec::with_capacity(signature.parameters.len());
            for (parameter, source) in signature.parameters.iter().enumerate() {
                let context = ResolvedInterfaceTemplateTypeUseContext::RequirementParameter {
                    requirement: signature.id,
                    parameter,
                };
                let Some(kind) = closed.get(context) else {
                    report_unclosed_type_use(
                        specialization,
                        template,
                        &source.type_syntax,
                        diagnostics,
                    );
                    valid = false;
                    continue;
                };
                parameters.push(ResolvedInterfaceParameter {
                    binding_mode: source.binding_mode,
                    name: source.name.clone(),
                    name_span: source.name_span,
                    type_syntax: ResolvedType {
                        kind,
                        span: source.type_syntax.span,
                    },
                    span: source.span,
                });
            }
            let result_context = ResolvedInterfaceTemplateTypeUseContext::RequirementResult {
                requirement: signature.id,
            };
            let Some(result) = closed.get(result_context) else {
                report_unclosed_type_use(
                    specialization,
                    template,
                    &signature.return_type,
                    diagnostics,
                );
                valid = false;
                continue;
            };
            if parameters.len() != signature.parameters.len() {
                continue;
            }

            let closed_id = InterfaceRequirementId::new(interface, signature.id.index());
            requirement_mappings.push(GenericInterfaceRequirementMapping {
                template: signature.id,
                closed: closed_id,
            });
            requirements.push(ResolvedInterfaceRequirement {
                id: closed_id,
                name: signature.name.clone(),
                name_span: signature.name_span,
                mutable: signature.mutable,
                parameters,
                return_type: ResolvedType {
                    kind: result,
                    span: signature.return_type.span,
                },
                span: signature.span,
            });
        }
        if requirements.len() != semantics.requirements.len() {
            continue;
        }

        mappings.push((interface, requirement_mappings));
        declarations.push(ResolvedInterfaceDeclaration {
            id: interface,
            module: template.module,
            visibility: template.visibility,
            name: names.specialized_interface_name(template, &specialization.key.arguments),
            name_span: template.name_span,
            requirements,
            span: template.span,
        });
    }
    if !valid || declarations.len() != specializations.iter().len() {
        specializations.fail_all();
        return MaterializedInterfaces {
            declarations: Vec::new(),
            valid: false,
        };
    }
    for (interface, requirement_mappings) in mappings {
        specializations
            .iter_mut()
            .find(|specialization| specialization.interface() == Some(interface))
            .expect("materialized interfaces retain their specialization")
            .requirement_mappings = requirement_mappings;
    }
    MaterializedInterfaces {
        declarations,
        valid: true,
    }
}

struct InterfaceTypeUseLookup<'semantic, 'specialization> {
    semantics: &'semantic ResolvedInterfaceTemplateSemantics,
    specialization: &'specialization GenericInterfaceSpecialization,
}

impl<'semantic, 'specialization> InterfaceTypeUseLookup<'semantic, 'specialization> {
    const fn new(
        semantics: &'semantic ResolvedInterfaceTemplateSemantics,
        specialization: &'specialization GenericInterfaceSpecialization,
    ) -> Self {
        Self {
            semantics,
            specialization,
        }
    }

    fn get(&self, context: ResolvedInterfaceTemplateTypeUseContext) -> Option<ResolvedTypeKind> {
        self.semantics
            .type_uses
            .iter()
            .zip(&self.specialization.closed_type_uses)
            .find_map(|(type_use, closed)| (type_use.context == context).then_some(*closed))
            .flatten()
    }
}

fn report_invalid_template(
    specialization: &GenericInterfaceSpecialization,
    template: &ResolvedInterfaceTemplate,
    diagnostics: &mut Diagnostics,
) {
    let origin = specialization
        .provenance
        .origins
        .first()
        .expect("requested specialization retains an origin");
    diagnostics.push(
        Diagnostic::error(
            super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
            format!(
                "cannot specialize invalid generic interface `{}`",
                template.name
            ),
        )
        .with_primary_label(origin.span, "closed application requested here")
        .with_secondary_label(template.name_span, "invalid template declared here"),
    );
}

fn report_unclosed_type_use(
    specialization: &GenericInterfaceSpecialization,
    template: &ResolvedInterfaceTemplate,
    type_use: &ResolvedTemplateType,
    diagnostics: &mut Diagnostics,
) {
    let origin = specialization
        .provenance
        .origins
        .first()
        .expect("requested specialization retains an origin");
    diagnostics.push(
        Diagnostic::error(
            super::super::super::INVALID_GENERIC_INTERFACE_REQUIREMENT,
            format!(
                "type arguments for `{}` do not produce a valid closed interface signature",
                template.name
            ),
        )
        .with_primary_label(origin.span, "invalid closed application")
        .with_secondary_label(type_use.span, "this requirement type cannot be closed")
        .with_secondary_label(template.name_span, "generic interface declared here"),
    );
}

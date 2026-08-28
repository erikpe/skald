//! Current-module declaration lookup and direct module-binding selection.

use std::collections::HashMap;

use super::*;
use crate::{
    identity::ModuleId,
    module::{ModulePath, ProgramModuleTable},
};

#[derive(Clone, Copy, Debug)]
pub(super) enum TopLevelLookup {
    Found(TopLevelSymbol),
    Missing,
    Diagnosed,
}

#[derive(Clone, Copy)]
pub(super) struct ModuleLookupProgram<'program> {
    pub(super) ordinary_bindings: &'program ResolvedOrdinaryBindings,
    pub(super) bindings: &'program ResolvedModuleBindings,
    pub(super) declarations: &'program ResolvedModuleDeclarationTable,
    pub(super) modules: &'program ProgramModuleTable,
    pub(super) module_spans: &'program [Span],
    pub(super) class_templates: &'program ResolvedClassTemplateTable,
    pub(super) type_parameters: &'program ResolvedTypeParameterTable,
    pub(super) specializations: Option<&'program GenericSpecializationTable>,
    pub(super) interface_specializations: Option<&'program GenericInterfaceSpecializationTable>,
}

#[derive(Clone, Copy)]
pub(super) struct ModuleLookup<'program> {
    current: ModuleId,
    top_levels: &'program HashMap<String, TopLevelSymbol>,
    ordinary_bindings: &'program ResolvedOrdinaryBindings,
    bindings: &'program ResolvedModuleBindings,
    declarations: &'program ResolvedModuleDeclarationTable,
    modules: &'program ProgramModuleTable,
    module_spans: &'program [Span],
    class_templates: &'program ResolvedClassTemplateTable,
    type_parameters: &'program ResolvedTypeParameterTable,
    specializations: Option<&'program GenericSpecializationTable>,
    interface_specializations: Option<&'program GenericInterfaceSpecializationTable>,
    qualified_enabled: bool,
}

impl<'program> ModuleLookup<'program> {
    pub(super) fn new(
        current: ModuleId,
        top_levels: &'program HashMap<String, TopLevelSymbol>,
        program: ModuleLookupProgram<'program>,
        qualified_enabled: bool,
    ) -> Self {
        Self {
            current,
            top_levels,
            ordinary_bindings: program.ordinary_bindings,
            bindings: program.bindings,
            declarations: program.declarations,
            modules: program.modules,
            module_spans: program.module_spans,
            class_templates: program.class_templates,
            type_parameters: program.type_parameters,
            specializations: program.specializations,
            interface_specializations: program.interface_specializations,
            qualified_enabled,
        }
    }

    pub(super) fn select(
        self,
        name: &syntax::Name,
        diagnostics: &mut Diagnostics,
    ) -> TopLevelLookup {
        if !name.is_qualified() {
            if let Some(symbol) = self.top_levels.get(name.text.as_str()).copied() {
                return TopLevelLookup::Found(symbol);
            }
            return self
                .ordinary_bindings
                .get(name.text.as_str())
                .map(|binding| TopLevelLookup::Found(self.imported_symbol(binding)))
                .unwrap_or(TopLevelLookup::Missing);
        }
        if !self.qualified_enabled {
            diagnostics.push(
                Diagnostic::error(
                    MODULE_CONTEXT_REQUIRED,
                    "qualified names require whole-program module compilation",
                )
                .with_primary_label(
                    name.span,
                    "use a compilation request to supply module roots and an entry",
                ),
            );
            return TopLevelLookup::Diagnosed;
        }

        let components = name.components().collect::<Vec<_>>();
        let (leaf, module_components) = components
            .split_last()
            .expect("qualified names contain at least two components");
        let local_path = ModulePath::from_components(
            module_components
                .iter()
                .map(|component| component.text.to_owned()),
        )
        .expect("parsed qualified-name components are valid module components");
        let Some(binding) = self.bindings.get(&local_path) else {
            self.report_unknown_binding(name, &local_path, diagnostics);
            return TopLevelLookup::Diagnosed;
        };
        let target_path = self
            .modules
            .get(binding.target)
            .expect("module bindings reference loaded modules")
            .module_path();
        let declarations = self
            .declarations
            .get(binding.target)
            .expect("every loaded module has a declaration index");
        let Some(declaration) = declarations.get(leaf.text) else {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_QUALIFIED_DECLARATION,
                    format!(
                        "module `{target_path}` has no declaration named `{}`",
                        leaf.text
                    ),
                )
                .with_primary_label(leaf.span, "unknown declaration")
                .with_secondary_label(binding.name_span, "module bound here"),
            );
            return TopLevelLookup::Diagnosed;
        };
        if !declaration.visibility.is_public() {
            diagnostics.push(
                Diagnostic::error(
                    PRIVATE_DECLARATION,
                    format!(
                        "declaration `{target_path}::{}` is private",
                        declaration.name
                    ),
                )
                .with_primary_label(leaf.span, "private declaration used here")
                .with_secondary_label(declaration.name_span, "declared private here"),
            );
            return TopLevelLookup::Diagnosed;
        }
        TopLevelLookup::Found(TopLevelSymbol {
            kind: match declaration.declaration {
                ResolvedTopLevelId::Function(function) => TopLevelSymbolKind::Function(function),
                ResolvedTopLevelId::Class(class) => TopLevelSymbolKind::Class(class),
                ResolvedTopLevelId::ClassTemplate(template) => {
                    TopLevelSymbolKind::ClassTemplate(template)
                }
                ResolvedTopLevelId::Interface(interface) => {
                    TopLevelSymbolKind::Interface(interface)
                }
                ResolvedTopLevelId::InterfaceTemplate(template) => {
                    TopLevelSymbolKind::InterfaceTemplate(template)
                }
            },
            name_span: declaration.name_span,
        })
    }

    fn imported_symbol(self, binding: &ResolvedOrdinaryBinding) -> TopLevelSymbol {
        let declaration = self
            .declarations
            .declaration(binding.target_module, binding.target)
            .expect("ordinary bindings reference target declarations");
        TopLevelSymbol {
            kind: match binding.target {
                ResolvedTopLevelId::Function(function) => TopLevelSymbolKind::Function(function),
                ResolvedTopLevelId::Class(class) => TopLevelSymbolKind::Class(class),
                ResolvedTopLevelId::ClassTemplate(template) => {
                    TopLevelSymbolKind::ClassTemplate(template)
                }
                ResolvedTopLevelId::Interface(interface) => {
                    TopLevelSymbolKind::Interface(interface)
                }
                ResolvedTopLevelId::InterfaceTemplate(template) => {
                    TopLevelSymbolKind::InterfaceTemplate(template)
                }
            },
            name_span: declaration.name_span,
        }
    }

    pub(super) fn template_arity(self, template: crate::identity::ClassTemplateId) -> usize {
        debug_assert!(self.class_templates.get(template).is_some());
        self.type_parameters
            .for_template(template)
            .expect("every class template has one parameter list")
            .len()
    }

    pub(super) fn interface_template_arity(
        self,
        template: crate::identity::InterfaceTemplateId,
    ) -> usize {
        self.type_parameters
            .for_interface_template(template)
            .expect("every interface template has one parameter list")
            .len()
    }

    pub(super) fn specialized_class(self, span: Span) -> Option<ClassId> {
        self.specializations?
            .class_at_application(self.current, span)
    }

    pub(super) fn specialized_interface(self, span: Span) -> Option<InterfaceId> {
        self.interface_specializations?
            .interface_at_application(self.current, span)
    }

    pub(super) fn class_specialization(
        self,
        class: ClassId,
    ) -> Option<&'program GenericSpecialization> {
        self.specializations?.for_class(class)
    }

    pub(super) fn specialization_at(self, span: Span) -> Option<&'program GenericSpecialization> {
        self.specializations?.at_application(self.current, span)
    }

    fn report_unknown_binding(
        self,
        name: &syntax::Name,
        local_path: &ModulePath,
        diagnostics: &mut Diagnostics,
    ) {
        let mut diagnostic = Diagnostic::error(
            UNKNOWN_MODULE_BINDING,
            format!("unknown module binding `{local_path}`"),
        )
        .with_primary_label(
            name.span,
            "qualified access requires an exact direct module import",
        );

        if let Some(target) = self.modules.find(local_path) {
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.target == target.module_id())
            {
                diagnostic = diagnostic
                    .with_secondary_label(
                        binding.name_span,
                        format!("this module is bound locally as `{}`", binding.local_path),
                    )
                    .with_note(format!(
                        "use `{}::{}` through that binding",
                        binding.local_path,
                        name.components()
                            .last()
                            .expect("qualified names have a declaration leaf")
                            .text
                    ));
            } else {
                diagnostic = diagnostic
                    .with_secondary_label(
                        self.module_spans[target.module_id().index()],
                        "this module is reachable but is not directly imported here",
                    )
                    .with_note(format!(
                        "add `import {};` to this module",
                        target.module_path()
                    ));
            }
        } else if let Some(ancestor) = self.bindings.iter().find(|binding| {
            binding.local_path.len() < local_path.len()
                && binding
                    .local_path
                    .components()
                    .zip(local_path.components())
                    .all(|(left, right)| left == right)
        }) {
            diagnostic = diagnostic
                .with_secondary_label(
                    ancestor.name_span,
                    "this binding names only the imported module, not its descendants",
                )
                .with_note("import the descendant module directly before using it");
        } else {
            diagnostic = diagnostic.with_note(
                "absolute logical paths and transitive imports do not create local bindings",
            );
        }
        diagnostics.push(diagnostic.with_note(format!(
            "lookup occurred in module `{}`",
            self.modules
                .get(self.current)
                .expect("current module is loaded")
                .module_path()
        )));
    }
}

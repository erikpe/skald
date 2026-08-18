//! Source-facing names for ordinary closed specialization identities.

use super::super::resolver::ModuleUnit;
use super::*;
use crate::identity::{ArrayTypeId, FunctionTypeId, OptionalBoxTypeId, OptionalTypeId};

pub(super) struct SpecializationNameRenderer<'program, 'ast> {
    units: &'program [ModuleUnit<'ast>],
    modules: &'program ProgramModuleTable,
    specializations: &'program GenericSpecializationTable,
    interface_sources: Option<InterfaceNameSources<'program>>,
    ordinary_classes: &'program ResolvedClassDeclarationTable,
    interfaces: &'program ResolvedInterfaceDeclarationTable,
    type_interner: &'program ResolvedTypeInterner,
}

#[derive(Clone, Copy)]
pub(super) struct InterfaceNameSources<'program> {
    pub(super) specializations: &'program GenericInterfaceSpecializationTable,
    pub(super) templates: &'program ResolvedInterfaceTemplateTable,
}

impl<'program, 'ast> SpecializationNameRenderer<'program, 'ast> {
    pub(super) fn new(
        units: &'program [ModuleUnit<'ast>],
        modules: &'program ProgramModuleTable,
        specializations: &'program GenericSpecializationTable,
        interface_sources: Option<InterfaceNameSources<'program>>,
        ordinary_classes: &'program ResolvedClassDeclarationTable,
        interfaces: &'program ResolvedInterfaceDeclarationTable,
        type_interner: &'program ResolvedTypeInterner,
    ) -> Self {
        Self {
            units,
            modules,
            specializations,
            interface_sources,
            ordinary_classes,
            interfaces,
            type_interner,
        }
    }

    pub(super) fn specialized_name(
        &self,
        source_module: ModuleId,
        source: &syntax::ClassDecl,
        arguments: &[ResolvedTypeKind],
    ) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(arguments);
        format!(
            "{}<{arguments}>",
            self.declaration_name(source_module, &source.name.text)
        )
    }

    pub(super) fn specialized_interface_name(
        &self,
        template: &ResolvedInterfaceTemplate,
        arguments: &[ResolvedTypeKind],
    ) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(arguments);
        format!(
            "{}<{arguments}>",
            self.declaration_name(template.module, &template.name)
        )
    }

    /// Singleton compilation has no possible source-name ambiguity, so retain
    /// the compact spelling used before module graphs existed. Whole-program
    /// compilation uses canonical paths and is therefore independent of local
    /// import aliases and graph traversal order.
    fn declaration_name(&self, module: ModuleId, name: &str) -> String {
        if self.modules.len() == 1 {
            return name.to_owned();
        }
        self.modules.get(module).map_or_else(
            || name.to_owned(),
            |module| format!("{}::{name}", module.module_path()),
        )
    }
}

impl ResolvedTypeNameContext for SpecializationNameRenderer<'_, '_> {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.type_interner.array(id)
    }

    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.type_interner.function(id)
    }

    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.type_interner.optional(id)
    }

    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.type_interner.optional_box(id)
    }

    fn direct_class_name(&self, id: ClassId) -> Option<String> {
        self.ordinary_classes
            .get(id)
            .map(|declaration| self.declaration_name(declaration.module, &declaration.name))
            .or_else(|| {
                class_source(self.units, id)
                    .map(|(unit, source)| self.declaration_name(unit.module, &source.name.text))
            })
    }

    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey> {
        self.specializations
            .iter()
            .find(|specialization| specialization.class() == Some(id))
            .map(|specialization| &specialization.key)
    }

    fn template_name(&self, id: ClassTemplateId) -> Option<String> {
        template_source(self.units, id)
            .map(|(unit, source, _)| self.declaration_name(unit.module, &source.name.text))
    }

    fn interface_name(&self, id: InterfaceId) -> Option<String> {
        self.interfaces
            .get(id)
            .map(|declaration| self.declaration_name(declaration.module, &declaration.name))
            .or_else(|| {
                let sources = self.interface_sources?;
                let specialization = sources.specializations.for_interface(id)?;
                let template = sources.templates.get(specialization.key.template)?;
                Some(self.specialized_interface_name(template, &specialization.key.arguments))
            })
    }
}

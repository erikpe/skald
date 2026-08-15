//! Source-facing names for ordinary closed specialization identities.

use super::super::resolver::ModuleUnit;
use super::*;
use crate::identity::{ArrayTypeId, FunctionTypeId, OptionalBoxTypeId, OptionalTypeId};

pub(super) struct SpecializationNameRenderer<'program, 'ast> {
    units: &'program [ModuleUnit<'ast>],
    modules: &'program ProgramModuleTable,
    specializations: &'program GenericSpecializationTable,
    ordinary_classes: &'program ResolvedClassDeclarationTable,
    interfaces: &'program ResolvedInterfaceDeclarationTable,
    type_interner: &'program ResolvedTypeInterner,
}

impl<'program, 'ast> SpecializationNameRenderer<'program, 'ast> {
    pub(super) fn new(
        units: &'program [ModuleUnit<'ast>],
        modules: &'program ProgramModuleTable,
        specializations: &'program GenericSpecializationTable,
        ordinary_classes: &'program ResolvedClassDeclarationTable,
        interfaces: &'program ResolvedInterfaceDeclarationTable,
        type_interner: &'program ResolvedTypeInterner,
    ) -> Self {
        Self {
            units,
            modules,
            specializations,
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
    }
}

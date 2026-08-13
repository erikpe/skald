//! Source-facing names for ordinary closed specialization identities.

use super::super::resolver::ModuleUnit;
use super::*;

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
        self.specialization_name(source_module, source, arguments, &mut Vec::new())
    }

    fn specialization_name(
        &self,
        source_module: ModuleId,
        source: &syntax::ClassDecl,
        arguments: &[ResolvedTypeKind],
        visiting: &mut Vec<ClassId>,
    ) -> String {
        let arguments = arguments
            .iter()
            .map(|argument| self.type_name(*argument, visiting))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}<{arguments}>",
            self.declaration_name(source_module, &source.name.text)
        )
    }

    fn type_name(&self, ty: ResolvedTypeKind, visiting: &mut Vec<ClassId>) -> String {
        match ty {
            ResolvedTypeKind::I64 => "i64".to_owned(),
            ResolvedTypeKind::U64 => "u64".to_owned(),
            ResolvedTypeKind::U8 => "u8".to_owned(),
            ResolvedTypeKind::F64 => "f64".to_owned(),
            ResolvedTypeKind::Bool => "bool".to_owned(),
            ResolvedTypeKind::Unit => "unit".to_owned(),
            ResolvedTypeKind::Obj => "Obj".to_owned(),
            ResolvedTypeKind::Class(class) => self.class_name(class, visiting),
            ResolvedTypeKind::Interface(interface) => self.interface_name(interface),
            ResolvedTypeKind::Array(array) => {
                let element = self
                    .type_interner
                    .array(array)
                    .map(|array| self.type_name(array.element.kind, visiting))
                    .unwrap_or_else(|| array.to_string());
                format!("{element}[]")
            }
            ResolvedTypeKind::Shared(target) => {
                format!("shared {}", self.shared_target_name(target, visiting))
            }
            ResolvedTypeKind::Optional(optional) => {
                let Some(payload) = self.type_interner.optional(optional) else {
                    return optional.to_string();
                };
                let payload_name = self.type_name(payload.payload.kind, visiting);
                if matches!(payload.payload.kind, ResolvedTypeKind::Shared(_)) {
                    format!("({payload_name})?")
                } else {
                    format!("{payload_name}?")
                }
            }
        }
    }

    fn class_name(&self, class: ClassId, visiting: &mut Vec<ClassId>) -> String {
        if let Some(declaration) = self.ordinary_classes.get(class) {
            return self.declaration_name(declaration.module, &declaration.name);
        }
        if visiting.contains(&class) {
            return class.to_string();
        }
        let Some(specialization) = self
            .specializations
            .iter()
            .find(|specialization| specialization.class() == Some(class))
        else {
            return class.to_string();
        };
        let Some((unit, source, _)) = template_source(self.units, specialization.key.template)
        else {
            return class.to_string();
        };
        visiting.push(class);
        let name =
            self.specialization_name(unit.module, source, &specialization.key.arguments, visiting);
        visiting.pop();
        name
    }

    fn interface_name(&self, interface: InterfaceId) -> String {
        self.interfaces
            .get(interface)
            .map(|declaration| self.declaration_name(declaration.module, &declaration.name))
            .unwrap_or_else(|| interface.to_string())
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

    fn shared_target_name(
        &self,
        target: ResolvedSharedTarget,
        visiting: &mut Vec<ClassId>,
    ) -> String {
        match target {
            ResolvedSharedTarget::Obj => "Obj".to_owned(),
            ResolvedSharedTarget::Class(class) => self.class_name(class, visiting),
            ResolvedSharedTarget::Interface(interface) => self.interface_name(interface),
            ResolvedSharedTarget::Array(array) => {
                self.type_name(ResolvedTypeKind::Array(array), visiting)
            }
            ResolvedSharedTarget::OptionalBox(optional_box) => {
                let Some(target) = self.type_interner.optional_box(optional_box) else {
                    return optional_box.to_string();
                };
                if let Some(optional) = target.optional {
                    return self.type_name(ResolvedTypeKind::Optional(optional), visiting);
                }
                let mut name = match target.object_leaf {
                    Some(ResolvedObjectTarget::Obj) => "Obj".to_owned(),
                    Some(ResolvedObjectTarget::Class(class)) => self.class_name(class, visiting),
                    Some(ResolvedObjectTarget::Interface(interface)) => {
                        self.interface_name(interface)
                    }
                    None => optional_box.to_string(),
                };
                name.extend(std::iter::repeat_n('?', target.optional_depth));
                name
            }
        }
    }
}

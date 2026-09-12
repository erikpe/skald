//! Program-aware semantic type names used throughout the resolved dump.

use crate::identity::{
    ArrayTypeId, ClassId, ClassTemplateId, FunctionTypeId, InterfaceId, ModuleId,
    OptionalBoxTypeId, OptionalTypeId,
};

use super::super::ir::*;
use super::ResolvedDumper;

const fn function_parameter_mode_prefix(mode: ResolvedFunctionTypeParameterMode) -> &'static str {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => "",
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
        ResolvedFunctionTypeParameterMode::MutableAlias => "mut ref ",
    }
}

impl ResolvedDumper<'_> {
    pub(super) fn type_syntax(&mut self, type_syntax: &ResolvedType) {
        let name = match type_syntax.kind {
            ResolvedTypeKind::I64 => "I64",
            ResolvedTypeKind::U64 => "U64",
            ResolvedTypeKind::U8 => "U8",
            ResolvedTypeKind::F64 => "F64",
            ResolvedTypeKind::Bool => "Bool",
            ResolvedTypeKind::Unit => "Unit",
            ResolvedTypeKind::Obj => "Obj",
            ResolvedTypeKind::Class(class) => {
                self.line(&format!("Type Class {class}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Interface(interface) => {
                self.line(&format!("Type Interface {interface}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Function(function) => {
                self.line(
                    &format!(
                        "Type Function {function} {}",
                        self.render_semantic_type_kind(ResolvedTypeKind::Function(function))
                    ),
                    type_syntax.span,
                );
                return;
            }
            ResolvedTypeKind::Array(array) => {
                self.line(&format!("Type Array {array}"), type_syntax.span);
                return;
            }
            ResolvedTypeKind::Shared(target) => {
                self.line(
                    &format!("Type Shared {}", self.render_shared_target(target)),
                    type_syntax.span,
                );
                return;
            }
            ResolvedTypeKind::Optional(optional) => {
                self.line(
                    &format!(
                        "Type Optional {optional} {}",
                        self.render_type_kind(ResolvedTypeKind::Optional(optional))
                    ),
                    type_syntax.span,
                );
                return;
            }
        };
        self.line(&format!("Type {name}"), type_syntax.span);
    }

    pub(super) fn render_type_kind(&self, kind: ResolvedTypeKind) -> String {
        match kind {
            ResolvedTypeKind::I64 => "i64".to_owned(),
            ResolvedTypeKind::U64 => "u64".to_owned(),
            ResolvedTypeKind::U8 => "u8".to_owned(),
            ResolvedTypeKind::F64 => "f64".to_owned(),
            ResolvedTypeKind::Bool => "bool".to_owned(),
            ResolvedTypeKind::Unit => "unit".to_owned(),
            ResolvedTypeKind::Obj => "Obj".to_owned(),
            ResolvedTypeKind::Class(class) => format!("class {class}"),
            ResolvedTypeKind::Interface(interface) => format!("interface {interface}"),
            ResolvedTypeKind::Function(function) => {
                let function = self
                    .program
                    .function_types
                    .get(function)
                    .expect("resolved function-type identities must name table entries");
                let parameters = function
                    .parameters
                    .iter()
                    .map(|parameter| {
                        format!(
                            "{}{}",
                            function_parameter_mode_prefix(parameter.mode),
                            self.render_type_kind(parameter.type_syntax.kind)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "fn({parameters}) -> {}",
                    self.render_type_kind(function.result.kind)
                )
            }
            ResolvedTypeKind::Array(array) => format!("array {array}"),
            ResolvedTypeKind::Shared(target) => {
                format!("shared {}", self.render_shared_target(target))
            }
            ResolvedTypeKind::Optional(optional) => {
                let payload = self
                    .program
                    .optional_types
                    .get(optional)
                    .expect("resolved optional identities must name table entries");
                let name = self.render_type_kind(payload.payload.kind);
                if matches!(
                    payload.payload.kind,
                    ResolvedTypeKind::Shared(_) | ResolvedTypeKind::Function(_)
                ) {
                    format!("({name})?")
                } else {
                    format!("{name}?")
                }
            }
        }
    }

    pub(super) fn render_semantic_type_kind(&self, kind: ResolvedTypeKind) -> String {
        ResolvedTypeNameRenderer::new(self).render(kind)
    }

    pub(super) fn render_specialization_key(&self, key: &GenericClassInstanceKey) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(&key.arguments);
        format!("{}<{arguments}>", self.template_name(key.template))
    }

    pub(super) fn render_interface_specialization_key(
        &self,
        key: &GenericInterfaceInstanceKey,
    ) -> String {
        let arguments = ResolvedTypeNameRenderer::new(self).render_list(&key.arguments);
        let name = self
            .program
            .interface_templates
            .get(key.template)
            .map_or_else(
                || key.template.to_string(),
                |template| self.qualified_declaration_name(template.module, &template.name),
            );
        format!("{name}<{arguments}>")
    }

    pub(super) fn render_cross_kind_specialization_key(
        &self,
        key: &GenericSpecializationKey,
    ) -> String {
        match key {
            GenericSpecializationKey::Class(key) => self.render_specialization_key(key),
            GenericSpecializationKey::Interface(key) => {
                self.render_interface_specialization_key(key)
            }
        }
    }

    pub(super) fn render_shared_target(&self, target: ResolvedSharedTarget) -> String {
        match target.category() {
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Obj) => "Obj".to_owned(),
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Class(class)) => {
                format!("class {class}")
            }
            ResolvedSharedTargetCategory::Object(ResolvedObjectTarget::Interface(interface)) => {
                format!("interface {interface}")
            }
            ResolvedSharedTargetCategory::Array(array) => format!("array {array}"),
            ResolvedSharedTargetCategory::OptionalBox(target) => {
                let metadata = self
                    .program
                    .optional_box_types
                    .get(target)
                    .expect("resolved optional-box identities must name table entries");
                format!(
                    "optional-box {target} exact {}",
                    metadata
                        .optional
                        .map(|optional| optional.to_string())
                        .unwrap_or_else(|| "view-only".to_owned())
                )
            }
        }
    }

    pub(super) fn template_name(&self, template: ClassTemplateId) -> String {
        self.program.class_templates.get(template).map_or_else(
            || template.to_string(),
            |template| self.qualified_declaration_name(template.module, &template.name),
        )
    }

    pub(super) fn qualified_declaration_name(&self, module: ModuleId, name: &str) -> String {
        if self.program.modules.len() == 1 {
            return name.to_owned();
        }
        self.program.modules.get(module).map_or_else(
            || name.to_owned(),
            |module| format!("{}::{name}", module.module_path()),
        )
    }
}

impl ResolvedTypeNameContext for ResolvedDumper<'_> {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.program.array_types.get(id)
    }

    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.program.function_types.get(id)
    }

    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.program.optional_types.get(id)
    }

    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.program.optional_box_types.get(id)
    }

    fn direct_class_name(&self, id: ClassId) -> Option<String> {
        let declaration = self.program.class(id)?;
        if self.program.generic_specializations.for_class(id).is_some() {
            return Some(declaration.name.clone());
        }
        Some(self.qualified_declaration_name(declaration.module, &declaration.name))
    }

    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey> {
        self.program
            .generic_specializations
            .for_class(id)
            .map(|specialization| &specialization.key)
    }

    fn template_name(&self, id: ClassTemplateId) -> Option<String> {
        self.program
            .class_templates
            .get(id)
            .map(|template| self.qualified_declaration_name(template.module, &template.name))
    }

    fn interface_name(&self, id: InterfaceId) -> Option<String> {
        self.program.interface(id).map(|declaration| {
            self.qualified_declaration_name(declaration.module, &declaration.name)
        })
    }
}

//! Shared source-facing rendering for recursive resolved types.

use crate::identity::{
    ArrayTypeId, ClassId, ClassTemplateId, FunctionTypeId, InterfaceId, OptionalBoxTypeId,
    OptionalTypeId,
};

use super::{
    GenericClassInstanceKey, ResolvedArrayType, ResolvedFunctionType,
    ResolvedFunctionTypeParameterMode, ResolvedObjectTarget, ResolvedOptionalBoxType,
    ResolvedOptionalType, ResolvedSharedTarget, ResolvedTypeKind,
};

/// Supplies metadata and consumer-specific nominal names to the structural
/// resolved-type renderer.
pub(crate) trait ResolvedTypeNameContext {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType>;
    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType>;
    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType>;
    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType>;

    fn direct_class_name(&self, id: ClassId) -> Option<String>;
    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey>;
    fn template_name(&self, id: ClassTemplateId) -> Option<String>;
    fn interface_name(&self, id: InterfaceId) -> Option<String>;

    fn missing_optional_box_leaf_name(&self, id: OptionalBoxTypeId) -> String {
        id.to_string()
    }
}

/// Renders exact source-shaped names while retaining one class-cycle guard
/// across the complete recursive type graph.
pub(crate) struct ResolvedTypeNameRenderer<'context, Context> {
    context: &'context Context,
    visiting_classes: Vec<ClassId>,
}

impl<'context, Context: ResolvedTypeNameContext> ResolvedTypeNameRenderer<'context, Context> {
    pub(crate) fn new(context: &'context Context) -> Self {
        Self {
            context,
            visiting_classes: Vec::new(),
        }
    }

    pub(crate) fn render(&mut self, kind: ResolvedTypeKind) -> String {
        match kind {
            ResolvedTypeKind::I64 => "i64".to_owned(),
            ResolvedTypeKind::U64 => "u64".to_owned(),
            ResolvedTypeKind::U8 => "u8".to_owned(),
            ResolvedTypeKind::F64 => "f64".to_owned(),
            ResolvedTypeKind::Bool => "bool".to_owned(),
            ResolvedTypeKind::Unit => "unit".to_owned(),
            ResolvedTypeKind::Obj => "Obj".to_owned(),
            ResolvedTypeKind::Class(class) => self.render_class(class),
            ResolvedTypeKind::Interface(interface) => self
                .context
                .interface_name(interface)
                .unwrap_or_else(|| interface.to_string()),
            ResolvedTypeKind::Function(function) => self.render_function(function),
            ResolvedTypeKind::Array(array) => self.render_array(array),
            ResolvedTypeKind::Shared(target) => {
                format!("shared {}", self.render_shared_target(target))
            }
            ResolvedTypeKind::Optional(optional) => self.render_optional(optional),
        }
    }

    pub(crate) fn render_list(&mut self, kinds: &[ResolvedTypeKind]) -> String {
        kinds
            .iter()
            .map(|kind| self.render(*kind))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn render_class(&mut self, class: ClassId) -> String {
        if self.visiting_classes.contains(&class) {
            return class.to_string();
        }
        if let Some(name) = self.context.direct_class_name(class) {
            return name;
        }
        let Some(key) = self.context.class_specialization(class) else {
            return class.to_string();
        };

        self.visiting_classes.push(class);
        let arguments = self.render_list(&key.arguments);
        self.visiting_classes.pop();
        let template = self
            .context
            .template_name(key.template)
            .unwrap_or_else(|| key.template.to_string());
        format!("{template}<{arguments}>")
    }

    fn render_function(&mut self, id: FunctionTypeId) -> String {
        let Some(function) = self.context.function(id) else {
            return id.to_string();
        };
        let parameters = function
            .parameters
            .iter()
            .map(|parameter| {
                format!(
                    "{}{}",
                    function_parameter_mode_prefix(parameter.mode),
                    self.render(parameter.type_syntax.kind)
                )
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("fn({parameters}) -> {}", self.render(function.result.kind))
    }

    fn render_array(&mut self, id: ArrayTypeId) -> String {
        let Some(array) = self.context.array(id) else {
            return id.to_string();
        };
        let element = self.render(array.element.kind);
        if requires_postfix_grouping(array.element.kind) {
            format!("({element})[]")
        } else {
            format!("{element}[]")
        }
    }

    fn render_optional(&mut self, id: OptionalTypeId) -> String {
        let Some(optional) = self.context.optional(id) else {
            return id.to_string();
        };
        let payload = self.render(optional.payload.kind);
        if requires_postfix_grouping(optional.payload.kind) {
            format!("({payload})?")
        } else {
            format!("{payload}?")
        }
    }

    fn render_shared_target(&mut self, target: ResolvedSharedTarget) -> String {
        match target {
            ResolvedSharedTarget::Obj => "Obj".to_owned(),
            ResolvedSharedTarget::Class(class) => self.render(ResolvedTypeKind::Class(class)),
            ResolvedSharedTarget::Interface(interface) => {
                self.render(ResolvedTypeKind::Interface(interface))
            }
            ResolvedSharedTarget::Array(array) => self.render(ResolvedTypeKind::Array(array)),
            ResolvedSharedTarget::OptionalBox(id) => {
                let Some(optional_box) = self.context.optional_box(id) else {
                    return id.to_string();
                };
                if let Some(optional) = optional_box.optional {
                    return self.render(ResolvedTypeKind::Optional(optional));
                }
                let mut name = optional_box.object_leaf.map_or_else(
                    || self.context.missing_optional_box_leaf_name(id),
                    |leaf| match leaf {
                        ResolvedObjectTarget::Obj => "Obj".to_owned(),
                        ResolvedObjectTarget::Class(class) => {
                            self.render(ResolvedTypeKind::Class(class))
                        }
                        ResolvedObjectTarget::Interface(interface) => {
                            self.render(ResolvedTypeKind::Interface(interface))
                        }
                    },
                );
                name.extend(std::iter::repeat_n('?', optional_box.optional_depth));
                name
            }
        }
    }
}

const fn function_parameter_mode_prefix(mode: ResolvedFunctionTypeParameterMode) -> &'static str {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => "",
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => "ref ",
        ResolvedFunctionTypeParameterMode::MutableAlias => "mut ref ",
    }
}

const fn requires_postfix_grouping(kind: ResolvedTypeKind) -> bool {
    matches!(
        kind,
        ResolvedTypeKind::Shared(_) | ResolvedTypeKind::Function(_)
    )
}

#[cfg(test)]
mod tests;

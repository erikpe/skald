//! Structural closing and bottom-up interning for template type terms.

use super::*;

impl SpecializationOwner<'_, '_, '_> {
    pub(super) fn close_template_type(
        &mut self,
        term: &ResolvedTemplateType,
        template: ClassTemplateId,
        arguments: &[ResolvedTypeKind],
    ) -> Option<ResolvedTypeKind> {
        Some(match &term.kind {
            ResolvedTemplateTypeKind::I64 => ResolvedTypeKind::I64,
            ResolvedTemplateTypeKind::U64 => ResolvedTypeKind::U64,
            ResolvedTemplateTypeKind::U8 => ResolvedTypeKind::U8,
            ResolvedTemplateTypeKind::F64 => ResolvedTypeKind::F64,
            ResolvedTemplateTypeKind::Bool => ResolvedTypeKind::Bool,
            ResolvedTemplateTypeKind::Unit => ResolvedTypeKind::Unit,
            ResolvedTemplateTypeKind::Obj => ResolvedTypeKind::Obj,
            ResolvedTemplateTypeKind::Parameter(parameter) => {
                debug_assert_eq!(parameter.template(), template);
                *arguments.get(parameter.index())?
            }
            ResolvedTemplateTypeKind::Class(class) => ResolvedTypeKind::Class(*class),
            ResolvedTemplateTypeKind::Interface(interface) => {
                ResolvedTypeKind::Interface(*interface)
            }
            ResolvedTemplateTypeKind::ClassTemplate {
                template: nested,
                arguments: nested_arguments,
            } => {
                let nested_arguments = nested_arguments
                    .iter()
                    .map(|argument| self.close_template_type(argument, template, arguments))
                    .collect::<Option<Vec<_>>>()?;
                let declaration = self
                    .templates
                    .get(template)
                    .expect("closing environment belongs to a collected template");
                let class = self.request(
                    *nested,
                    nested_arguments,
                    GenericApplicationOrigin {
                        module: declaration.module,
                        span: term.span,
                    },
                )?;
                ResolvedTypeKind::Class(class)
            }
            ResolvedTemplateTypeKind::Shared(target) => ResolvedTypeKind::Shared(
                self.close_template_shared_target(target, template, arguments)?,
            ),
            ResolvedTemplateTypeKind::Optional(payload) => {
                let payload = ResolvedType {
                    kind: self.close_template_type(payload, template, arguments)?,
                    span: payload.span,
                };
                ResolvedTypeKind::Optional(self.interner.intern_optional(payload))
            }
            ResolvedTemplateTypeKind::Array(element) => {
                let element = ResolvedType {
                    kind: self.close_template_type(element, template, arguments)?,
                    span: element.span,
                };
                ResolvedTypeKind::Array(self.interner.intern_array(element))
            }
        })
    }

    fn close_template_shared_target(
        &mut self,
        target: &ResolvedTemplateType,
        template: ClassTemplateId,
        arguments: &[ResolvedTypeKind],
    ) -> Option<ResolvedSharedTarget> {
        let (optional_depth, leaf) = template_optional_leaf(target);
        if optional_depth > 0 {
            let leaf = self.close_template_type(leaf, template, arguments)?;
            if let Some(object) = object_target(leaf) {
                if matches!(
                    object,
                    ResolvedObjectTarget::Obj | ResolvedObjectTarget::Interface(_)
                ) {
                    return Some(ResolvedSharedTarget::OptionalBox(
                        self.interner.intern_optional_object_box_view(
                            optional_depth,
                            object,
                            target.span,
                        ),
                    ));
                }
            }
        }

        let kind = self.close_template_type(target, template, arguments)?;
        match kind {
            ResolvedTypeKind::Optional(optional) => Some(ResolvedSharedTarget::OptionalBox(
                self.interner.intern_optional_box(optional, target.span),
            )),
            kind => ResolvedSharedTarget::from_direct_type(kind),
        }
    }
}

fn template_optional_leaf(mut term: &ResolvedTemplateType) -> (usize, &ResolvedTemplateType) {
    let mut depth = 0;
    while let ResolvedTemplateTypeKind::Optional(payload) = &term.kind {
        depth += 1;
        term = payload;
    }
    (depth, term)
}

pub(super) const fn object_target(kind: ResolvedTypeKind) -> Option<ResolvedObjectTarget> {
    match kind {
        ResolvedTypeKind::Obj => Some(ResolvedObjectTarget::Obj),
        ResolvedTypeKind::Class(class) => Some(ResolvedObjectTarget::Class(class)),
        ResolvedTypeKind::Interface(interface) => Some(ResolvedObjectTarget::Interface(interface)),
        _ => None,
    }
}

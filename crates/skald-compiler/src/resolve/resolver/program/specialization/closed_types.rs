//! Structural closing and bottom-up interning for template type terms.

use super::*;
use crate::identity::GenericTemplateId;

#[derive(Clone, Copy)]
pub(super) struct TypeClosingEnvironment<'arguments> {
    owner: Option<GenericTemplateId>,
    arguments: &'arguments [ResolvedTypeKind],
    module: ModuleId,
}

impl<'arguments> TypeClosingEnvironment<'arguments> {
    pub(super) fn class(
        template: ClassTemplateId,
        arguments: &'arguments [ResolvedTypeKind],
        module: ModuleId,
    ) -> Self {
        Self {
            owner: Some(template.into()),
            arguments,
            module,
        }
    }

    pub(super) fn interface(
        template: InterfaceTemplateId,
        arguments: &'arguments [ResolvedTypeKind],
        module: ModuleId,
    ) -> Self {
        Self {
            owner: Some(template.into()),
            arguments,
            module,
        }
    }
}

impl SpecializationCoordinator<'_, '_, '_> {
    pub(super) fn close_template_interface(
        &mut self,
        interface: &ResolvedInterfaceType,
        span: Span,
        environment: TypeClosingEnvironment<'_>,
    ) -> Option<InterfaceId> {
        match interface {
            ResolvedInterfaceType::Ordinary(interface) => Some(*interface),
            ResolvedInterfaceType::TemplateApplication {
                template,
                arguments,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.close_template_type(argument, environment))
                    .collect::<Option<Vec<_>>>()?;
                self.request_interface(
                    *template,
                    arguments,
                    GenericInterfaceApplicationOrigin {
                        module: environment.module,
                        span,
                    },
                )
            }
        }
    }

    pub(super) fn close_template_type(
        &mut self,
        term: &ResolvedTemplateType,
        environment: TypeClosingEnvironment<'_>,
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
                debug_assert_eq!(Some(parameter.owner()), environment.owner);
                *environment.arguments.get(parameter.index())?
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
                    .map(|argument| self.close_template_type(argument, environment))
                    .collect::<Option<Vec<_>>>()?;
                let class = self.request_class(
                    *nested,
                    nested_arguments,
                    GenericApplicationOrigin {
                        module: environment.module,
                        span: term.span,
                    },
                )?;
                ResolvedTypeKind::Class(class)
            }
            ResolvedTemplateTypeKind::InterfaceTemplate {
                template,
                arguments,
            } => {
                let arguments = arguments
                    .iter()
                    .map(|argument| self.close_template_type(argument, environment))
                    .collect::<Option<Vec<_>>>()?;
                let interface = self.request_interface(
                    *template,
                    arguments,
                    GenericInterfaceApplicationOrigin {
                        module: environment.module,
                        span: term.span,
                    },
                )?;
                if matches!(environment.owner, Some(GenericTemplateId::Class(_))) {
                    // I5 materializes the interface dependency, but an
                    // ordinary class declaration cannot claim it until I6
                    // performs exact generic-interface conformance.
                    return None;
                }
                ResolvedTypeKind::Interface(interface)
            }
            ResolvedTemplateTypeKind::Function { parameters, result } => {
                let parameters = parameters
                    .iter()
                    .map(|parameter| {
                        Some(ResolvedFunctionTypeParameter {
                            mode: parameter.mode,
                            type_syntax: ResolvedType {
                                kind: self
                                    .close_template_type(&parameter.type_syntax, environment)?,
                                span: parameter.type_syntax.span,
                            },
                            span: parameter.span,
                        })
                    })
                    .collect::<Option<Vec<_>>>()?;
                let result = ResolvedType {
                    kind: self.close_template_type(result, environment)?,
                    span: result.span,
                };
                ResolvedTypeKind::Function(
                    self.interner.intern_function(parameters, result, term.span),
                )
            }
            ResolvedTemplateTypeKind::Shared(target) => {
                ResolvedTypeKind::Shared(self.close_template_shared_target(target, environment)?)
            }
            ResolvedTemplateTypeKind::Optional(payload) => {
                let payload = ResolvedType {
                    kind: self.close_template_type(payload, environment)?,
                    span: payload.span,
                };
                ResolvedTypeKind::Optional(self.interner.intern_optional(payload))
            }
            ResolvedTemplateTypeKind::Array(element) => {
                let element = ResolvedType {
                    kind: self.close_template_type(element, environment)?,
                    span: element.span,
                };
                ResolvedTypeKind::Array(self.interner.intern_array(element))
            }
        })
    }

    pub(super) fn close_template_shared_target(
        &mut self,
        target: &ResolvedTemplateType,
        environment: TypeClosingEnvironment<'_>,
    ) -> Option<ResolvedSharedTarget> {
        let (optional_depth, leaf) = template_optional_leaf(target);
        if optional_depth > 0 {
            let leaf = self.close_template_type(leaf, environment)?;
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

        let kind = self.close_template_type(target, environment)?;
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

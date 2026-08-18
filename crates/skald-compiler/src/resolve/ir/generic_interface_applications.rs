//! Closed generic-interface requests retained before specialization assigns IDs.

use crate::{identity::ModuleId, source::Span};

use super::{ResolvedInterfaceType, ResolvedTemplateType, ResolvedTemplateTypeKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericInterfaceApplicationOrigin {
    pub module: ModuleId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedGenericInterfaceApplication {
    pub interface: ResolvedInterfaceType,
    pub origins: Vec<GenericInterfaceApplicationOrigin>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedGenericInterfaceApplicationTable {
    entries: Vec<ResolvedGenericInterfaceApplication>,
}

impl ResolvedGenericInterfaceApplicationTable {
    pub(crate) fn record_interface(
        &mut self,
        interface: &ResolvedInterfaceType,
        origin: GenericInterfaceApplicationOrigin,
    ) {
        let ResolvedInterfaceType::TemplateApplication {
            template,
            arguments,
        } = interface
        else {
            return;
        };
        if !interface.depends_on_parameter() {
            if let Some(entry) = self
                .entries
                .iter_mut()
                .find(|entry| entry.interface.semantically_eq(interface))
            {
                if !entry.origins.contains(&origin) {
                    entry.origins.push(origin);
                }
            } else {
                self.entries.push(ResolvedGenericInterfaceApplication {
                    interface: ResolvedInterfaceType::TemplateApplication {
                        template: *template,
                        arguments: arguments.clone(),
                    },
                    origins: vec![origin],
                });
            }
        }

        for argument in arguments {
            self.record_type(origin.module, argument);
        }
    }

    pub(crate) fn record_type(&mut self, module: ModuleId, type_term: &ResolvedTemplateType) {
        match &type_term.kind {
            ResolvedTemplateTypeKind::InterfaceTemplate {
                template,
                arguments,
            } => self.record_interface(
                &ResolvedInterfaceType::TemplateApplication {
                    template: *template,
                    arguments: arguments.clone(),
                },
                GenericInterfaceApplicationOrigin {
                    module,
                    span: type_term.span,
                },
            ),
            ResolvedTemplateTypeKind::ClassTemplate { arguments, .. } => {
                for argument in arguments {
                    self.record_type(module, argument);
                }
            }
            ResolvedTemplateTypeKind::Function { parameters, result } => {
                for parameter in parameters {
                    self.record_type(module, &parameter.type_syntax);
                }
                self.record_type(module, result);
            }
            ResolvedTemplateTypeKind::Shared(target)
            | ResolvedTemplateTypeKind::Optional(target)
            | ResolvedTemplateTypeKind::Array(target) => self.record_type(module, target),
            ResolvedTemplateTypeKind::I64
            | ResolvedTemplateTypeKind::U64
            | ResolvedTemplateTypeKind::U8
            | ResolvedTemplateTypeKind::F64
            | ResolvedTemplateTypeKind::Bool
            | ResolvedTemplateTypeKind::Unit
            | ResolvedTemplateTypeKind::Obj
            | ResolvedTemplateTypeKind::Parameter(_)
            | ResolvedTemplateTypeKind::Class(_)
            | ResolvedTemplateTypeKind::Interface(_) => {}
        }
    }

    pub(crate) fn sort_by_source_origin(&mut self) {
        for entry in &mut self.entries {
            entry
                .origins
                .sort_by_key(|origin| (origin.module.index(), origin.span.range().start()));
        }
        self.entries.sort_by_key(|entry| {
            let origin = entry
                .origins
                .first()
                .expect("every retained application has an origin");
            (origin.module.index(), origin.span.range().start())
        });
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedGenericInterfaceApplication> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

//! Definition-site resolution for non-executable generic class templates.

mod body;
mod bounds;
mod collection;
mod interface_resolution;
mod interface_validation;
mod requirements;
mod resolution;
mod type_resolution;

pub(super) use collection::{
    collect_generic_templates, ClassTemplateWorkItem, CollectedGenericTemplates,
    InterfaceTemplateWorkItem,
};
pub(super) use interface_resolution::resolve_interface_template_semantics;
pub(super) use resolution::resolve_class_template_semantics;

use super::*;
pub(in crate::resolve::resolver) use type_resolution::TemplateTypeResolver;

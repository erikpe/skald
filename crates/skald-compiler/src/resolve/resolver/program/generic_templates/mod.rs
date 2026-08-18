//! Definition-site resolution for non-executable generic class templates.

mod body;
mod bounds;
mod collection;
mod requirements;
mod resolution;
mod type_resolution;

pub(super) use collection::{
    collect_generic_templates, ClassTemplateWorkItem, CollectedGenericTemplates,
    InterfaceTemplateWorkItem,
};
pub(super) use resolution::resolve_class_template_semantics;

use super::*;
use type_resolution::TemplateTypeResolver;

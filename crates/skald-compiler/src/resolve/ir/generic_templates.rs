//! Stable, non-executable class-template declarations and their parameters.

use crate::{
    id_table::DenseIdTable,
    identity::{ClassTemplateId, ModuleId, TypeParameterId},
    source::Span,
};

use super::ResolvedVisibility;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClassTemplate {
    pub id: ClassTemplateId,
    pub module: ModuleId,
    pub visibility: ResolvedVisibility,
    pub name: String,
    pub name_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassTemplateTable {
    entries: DenseIdTable<ClassTemplateId, ResolvedClassTemplate>,
}

impl ResolvedClassTemplateTable {
    pub(crate) fn new(entries: Vec<ResolvedClassTemplate>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: ClassTemplateId) -> Option<&ResolvedClassTemplate> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedClassTemplate> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameter {
    pub id: TypeParameterId,
    pub name: String,
    pub name_span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameters {
    pub template: ClassTemplateId,
    parameters: Vec<ResolvedTypeParameter>,
}

impl ResolvedTypeParameters {
    pub(crate) fn new(template: ClassTemplateId, parameters: Vec<ResolvedTypeParameter>) -> Self {
        Self {
            template,
            parameters,
        }
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedTypeParameter> {
        self.parameters.iter()
    }

    pub fn get(&self, name: &str) -> Option<&ResolvedTypeParameter> {
        self.parameters
            .iter()
            .find(|parameter| parameter.name == name)
    }

    pub fn len(&self) -> usize {
        self.parameters.len()
    }

    pub fn is_empty(&self) -> bool {
        self.parameters.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTypeParameterTable {
    entries: DenseIdTable<ClassTemplateId, ResolvedTypeParameters>,
}

impl ResolvedTypeParameterTable {
    pub(crate) fn new(entries: Vec<ResolvedTypeParameters>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.template),
        }
    }

    pub fn for_template(&self, template: ClassTemplateId) -> Option<&ResolvedTypeParameters> {
        self.entries.get(template, |entry| entry.template)
    }

    pub fn get(&self, id: TypeParameterId) -> Option<&ResolvedTypeParameter> {
        self.for_template(id.template())?
            .iter()
            .find(|parameter| parameter.id == id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedTypeParameters> {
        self.entries.iter()
    }
}

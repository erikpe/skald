//! Stable, non-executable generic-template declarations and their parameters.

use crate::{
    id_table::DenseIdTable,
    identity::{
        ClassId, ClassTemplateId, GenericTemplateId, InterfaceId, InterfaceRequirementId,
        InterfaceTemplateId, InterfaceTemplateRequirementId, ModuleId, TypeParameterId,
    },
    source::Span,
};

use super::{
    GenericRequirement, ResolvedFunctionTypeParameterMode, ResolvedInterfaceClaim,
    ResolvedTopLevelId, ResolvedVisibility,
};

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceTemplateRequirement {
    pub id: InterfaceTemplateRequirementId,
    pub name: String,
    pub name_span: Span,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceTemplate {
    pub id: InterfaceTemplateId,
    pub module: ModuleId,
    pub visibility: ResolvedVisibility,
    pub name: String,
    pub name_span: Span,
    pub span: Span,
    requirements: Vec<ResolvedInterfaceTemplateRequirement>,
}

impl ResolvedInterfaceTemplate {
    pub(crate) fn new(
        id: InterfaceTemplateId,
        module: ModuleId,
        visibility: ResolvedVisibility,
        name: String,
        name_span: Span,
        span: Span,
        requirements: Vec<ResolvedInterfaceTemplateRequirement>,
    ) -> Self {
        Self {
            id,
            module,
            visibility,
            name,
            name_span,
            span,
            requirements,
        }
    }

    pub fn requirements(
        &self,
    ) -> impl ExactSizeIterator<Item = &ResolvedInterfaceTemplateRequirement> {
        self.requirements.iter()
    }

    pub fn requirement(
        &self,
        id: InterfaceTemplateRequirementId,
    ) -> Option<&ResolvedInterfaceTemplateRequirement> {
        (id.template() == self.id)
            .then(|| self.requirements.get(id.index()))
            .flatten()
            .filter(|requirement| requirement.id == id)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedInterfaceTemplateTable {
    entries: DenseIdTable<InterfaceTemplateId, ResolvedInterfaceTemplate>,
}

impl ResolvedInterfaceTemplateTable {
    pub(crate) fn new(entries: Vec<ResolvedInterfaceTemplate>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }

    pub fn get(&self, id: InterfaceTemplateId) -> Option<&ResolvedInterfaceTemplate> {
        self.entries.get(id, |entry| entry.id)
    }

    pub fn requirement(
        &self,
        id: InterfaceTemplateRequirementId,
    ) -> Option<&ResolvedInterfaceTemplateRequirement> {
        self.get(id.template())?.requirement(id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedInterfaceTemplate> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
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
    pub owner: GenericTemplateId,
    parameters: Vec<ResolvedTypeParameter>,
}

impl ResolvedTypeParameters {
    pub(crate) fn new(
        owner: impl Into<GenericTemplateId>,
        parameters: Vec<ResolvedTypeParameter>,
    ) -> Self {
        Self {
            owner: owner.into(),
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
    class_entries: DenseIdTable<ClassTemplateId, ResolvedTypeParameters>,
    interface_entries: DenseIdTable<InterfaceTemplateId, ResolvedTypeParameters>,
}

/// Structural type used only while a class template still contains parameters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTemplateType {
    pub(crate) kind: ResolvedTemplateTypeKind,
    pub(crate) span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTemplateTypeKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Obj,
    Parameter(TypeParameterId),
    Class(ClassId),
    Interface(InterfaceId),
    ClassTemplate {
        template: ClassTemplateId,
        arguments: Vec<ResolvedTemplateType>,
    },
    Function {
        parameters: Vec<ResolvedTemplateFunctionTypeParameter>,
        result: Box<ResolvedTemplateType>,
    },
    Shared(Box<ResolvedTemplateType>),
    Optional(Box<ResolvedTemplateType>),
    Array(Box<ResolvedTemplateType>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTemplateFunctionTypeParameter {
    pub(crate) mode: ResolvedFunctionTypeParameterMode,
    pub(crate) type_syntax: ResolvedTemplateType,
    pub(crate) span: Span,
}

impl ResolvedTemplateType {
    pub(crate) fn parameter(&self) -> Option<TypeParameterId> {
        match self.kind {
            ResolvedTemplateTypeKind::Parameter(parameter) => Some(parameter),
            _ => None,
        }
    }

    pub(crate) fn depends_on_parameter(&self) -> bool {
        match &self.kind {
            ResolvedTemplateTypeKind::Parameter(_) => true,
            ResolvedTemplateTypeKind::ClassTemplate { arguments, .. } => {
                arguments.iter().any(Self::depends_on_parameter)
            }
            ResolvedTemplateTypeKind::Function { parameters, result } => {
                parameters
                    .iter()
                    .any(|parameter| parameter.type_syntax.depends_on_parameter())
                    || result.depends_on_parameter()
            }
            ResolvedTemplateTypeKind::Shared(target)
            | ResolvedTemplateTypeKind::Optional(target)
            | ResolvedTemplateTypeKind::Array(target) => target.depends_on_parameter(),
            ResolvedTemplateTypeKind::I64
            | ResolvedTemplateTypeKind::U64
            | ResolvedTemplateTypeKind::U8
            | ResolvedTemplateTypeKind::F64
            | ResolvedTemplateTypeKind::Bool
            | ResolvedTemplateTypeKind::Unit
            | ResolvedTemplateTypeKind::Obj
            | ResolvedTemplateTypeKind::Class(_)
            | ResolvedTemplateTypeKind::Interface(_) => false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTemplateBound {
    pub(crate) parameter: TypeParameterId,
    pub(crate) interface: InterfaceId,
    pub(crate) parameter_span: Span,
    pub(crate) interface_span: Span,
    pub(crate) span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTemplateTypeUseContext {
    DirectBase,
    Field { member: usize },
    StaticField { member: usize },
    InitializerParameter { member: usize, parameter: usize },
    CopyConstructorParameter { member: usize, parameter: usize },
    CopyAssignmentParameter { member: usize, parameter: usize },
    MethodParameter { member: usize, parameter: usize },
    MethodResult { member: usize },
    Local { member: usize },
    CastTarget { member: usize },
    TypeTestTarget { member: usize },
    ConstructionTarget { member: usize },
    StaticSelectionTarget { member: usize },
    ArrayConstructionTarget { member: usize },
    OptionalBoxTarget { member: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedTemplateTypeUse {
    pub(crate) context: ResolvedTemplateTypeUseContext,
    pub(crate) type_term: ResolvedTemplateType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTemplateConstructionMode {
    Inline,
    Shared,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTemplateDependentSelectionKind {
    Construction(ResolvedTemplateConstructionMode),
    Cast,
    TypeTest,
    StaticMember,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedTemplateSelection {
    TopLevel {
        declaration: ResolvedTopLevelId,
        span: Span,
    },
    TemplateMember {
        member: usize,
        member_name: String,
        span: Span,
    },
    DefinitionSite {
        kind: ResolvedTemplateDependentSelectionKind,
        target: ResolvedTemplateType,
        member_name: Option<String>,
        span: Span,
    },
    ArgumentDependent {
        kind: ResolvedTemplateDependentSelectionKind,
        target: ResolvedTemplateType,
        member_name: Option<String>,
        span: Span,
    },
    BoundMember {
        parameter: TypeParameterId,
        interface: InterfaceId,
        requirement: InterfaceRequirementId,
        member_name: String,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedClassTemplateSemantics {
    pub(crate) template: ClassTemplateId,
    pub(crate) direct_base: Option<ResolvedTemplateType>,
    pub(crate) implemented_interfaces: Vec<ResolvedInterfaceClaim>,
    pub(crate) bounds: Vec<ResolvedTemplateBound>,
    pub(crate) type_uses: Vec<ResolvedTemplateTypeUse>,
    pub(crate) requirements: Vec<GenericRequirement>,
    pub(crate) selections: Vec<ResolvedTemplateSelection>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ResolvedClassTemplateSemanticTable {
    entries: DenseIdTable<ClassTemplateId, ResolvedClassTemplateSemantics>,
}

impl ResolvedClassTemplateSemanticTable {
    pub(crate) fn new(entries: Vec<ResolvedClassTemplateSemantics>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.template),
        }
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedClassTemplateSemantics> {
        self.entries.iter()
    }

    #[allow(dead_code)] // Closed specialization will query templates by identity.
    pub(crate) fn get(&self, template: ClassTemplateId) -> Option<&ResolvedClassTemplateSemantics> {
        self.entries.get(template, |entry| entry.template)
    }
}

impl ResolvedTypeParameterTable {
    pub(crate) fn new(
        class_entries: Vec<ResolvedTypeParameters>,
        interface_entries: Vec<ResolvedTypeParameters>,
    ) -> Self {
        Self {
            class_entries: DenseIdTable::new(class_entries, |entry| match entry.owner {
                GenericTemplateId::Class(template) => template,
                GenericTemplateId::Interface(_) => {
                    unreachable!("class parameter tables require class owners")
                }
            }),
            interface_entries: DenseIdTable::new(interface_entries, |entry| match entry.owner {
                GenericTemplateId::Interface(template) => template,
                GenericTemplateId::Class(_) => {
                    unreachable!("interface parameter tables require interface owners")
                }
            }),
        }
    }

    pub fn for_template(&self, template: ClassTemplateId) -> Option<&ResolvedTypeParameters> {
        self.class_entries.get(template, |entry| match entry.owner {
            GenericTemplateId::Class(template) => template,
            GenericTemplateId::Interface(_) => {
                unreachable!("class parameter tables require class owners")
            }
        })
    }

    pub fn for_interface_template(
        &self,
        template: InterfaceTemplateId,
    ) -> Option<&ResolvedTypeParameters> {
        self.interface_entries
            .get(template, |entry| match entry.owner {
                GenericTemplateId::Interface(template) => template,
                GenericTemplateId::Class(_) => {
                    unreachable!("interface parameter tables require interface owners")
                }
            })
    }

    pub fn get(&self, id: TypeParameterId) -> Option<&ResolvedTypeParameter> {
        let parameters = match id.owner() {
            GenericTemplateId::Class(template) => self.for_template(template),
            GenericTemplateId::Interface(template) => self.for_interface_template(template),
        }?;
        parameters.iter().find(|parameter| parameter.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &ResolvedTypeParameters> {
        self.class_entries
            .iter()
            .chain(self.interface_entries.iter())
    }
}

//! Typed declarations and their stable ID-indexed tables.

use crate::{
    external::ExternalLinkTable,
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, ExternalLinkId,
        FieldId, FunctionId, InitializerId, InterfaceId, InterfaceRequirementId, LocalId, MethodId,
        ModuleId, ParameterId, StaticFieldId, StaticInitializerId, VirtualFamilyId, VirtualSlotId,
    },
    intrinsic::Intrinsic,
    module::ProgramModuleTable,
    source::Span,
};

use super::{
    body::{HirClassDefinitionTable, HirFunctionDefinitionTable, HirMemberDefinition},
    object::HirCopyCapability,
    HirAccess, Type,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirProgram {
    pub modules: ProgramModuleTable,
    pub external_links: ExternalLinkTable,
    pub array_types: super::HirArrayTypeTable,
    pub optional_types: super::HirOptionalTypeTable,
    pub optional_box_types: super::HirOptionalBoxTypeTable,
    pub string_language_item: Option<super::HirStringLanguageItem>,
    pub literal_data: super::HirLiteralDataTable,
    pub classes: HirClassDeclarationTable,
    pub interfaces: HirInterfaceDeclarationTable,
    pub virtual_families: HirVirtualFamilyTable,
    pub class_definitions: HirClassDefinitionTable,
    pub declarations: HirFunctionDeclarationTable,
    pub definitions: HirFunctionDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

impl HirProgram {
    pub fn optional_type(
        &self,
        id: crate::identity::OptionalTypeId,
    ) -> Option<&super::HirOptionalType> {
        self.optional_types.get(id)
    }

    pub fn optional_box_type(
        &self,
        id: crate::identity::OptionalBoxTypeId,
    ) -> Option<&super::HirOptionalBoxType> {
        self.optional_box_types.get(id)
    }

    pub fn class(&self, id: ClassId) -> Option<&HirClassDeclaration> {
        self.classes.get(id)
    }
    pub fn interface(&self, id: InterfaceId) -> Option<&HirInterfaceDeclaration> {
        self.interfaces.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&HirFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn static_field(&self, id: StaticFieldId) -> Option<&super::HirStaticFieldDeclaration> {
        self.class(id.class())?.static_field(id)
    }

    pub fn static_initializer(
        &self,
        id: StaticInitializerId,
    ) -> Option<&super::HirStaticFieldInitializer> {
        self.static_field(id.field())?
            .initializer
            .as_ref()
            .filter(|initializer| initializer.id == id)
    }

    pub fn static_initializers(&self) -> impl Iterator<Item = &super::HirStaticFieldInitializer> {
        self.classes.iter().flat_map(|class| {
            class
                .static_fields
                .iter()
                .filter_map(|field| field.initializer.as_ref())
        })
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&HirInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn copy_constructor(
        &self,
        id: CopyConstructorId,
    ) -> Option<&HirCopyConstructorDeclaration> {
        self.class(id.class())?.copy_constructor_declaration(id)
    }

    pub fn copy_assignment(&self, id: CopyAssignmentId) -> Option<&HirCopyAssignmentDeclaration> {
        self.class(id.class())?.copy_assignment_declaration(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&HirMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&HirDestructorDeclaration> {
        self.class(id.class())?.destructor(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&HirMemberDefinition> {
        self.class_definitions
            .get(callable.class()?)?
            .member(callable)
    }

    pub fn callable_signature(&self, callable: CallableId) -> Option<HirCallableSignature<'_>> {
        match callable {
            CallableId::Function(function) => {
                self.declarations
                    .get(function)
                    .map(|declaration| HirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: declaration.return_type,
                    })
            }
            CallableId::StaticInitializer(initializer) => {
                self.static_initializer(initializer)
                    .map(|_| HirCallableSignature {
                        parameters: &[],
                        return_type: Type::Unit,
                    })
            }
            CallableId::Initializer(initializer) => {
                self.initializer(initializer)
                    .map(|declaration| HirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: Type::Unit,
                    })
            }
            CallableId::CopyConstructor(copy) => {
                self.copy_constructor(copy)
                    .map(|declaration| HirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: Type::Unit,
                    })
            }
            CallableId::CopyAssignment(assignment) => {
                self.copy_assignment(assignment)
                    .map(|declaration| HirCallableSignature {
                        parameters: std::slice::from_ref(&declaration.parameter),
                        return_type: Type::Unit,
                    })
            }
            CallableId::Destructor(destructor) => {
                self.destructor(destructor).map(|_| HirCallableSignature {
                    parameters: &[],
                    return_type: Type::Unit,
                })
            }
            CallableId::Method(method) => {
                self.method(method).map(|declaration| HirCallableSignature {
                    parameters: &declaration.parameters,
                    return_type: declaration.return_type,
                })
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirInterfaceDeclarationTable {
    entries: DenseIdTable<InterfaceId, HirInterfaceDeclaration>,
}

impl HirInterfaceDeclarationTable {
    pub(crate) fn new(entries: Vec<HirInterfaceDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }
    pub fn get(&self, id: InterfaceId) -> Option<&HirInterfaceDeclaration> {
        self.entries.get(id, |entry| entry.id)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirInterfaceDeclaration> {
        self.entries.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInterfaceDeclaration {
    pub id: InterfaceId,
    pub module: ModuleId,
    pub name: String,
    pub name_span: Span,
    pub requirements: Vec<HirInterfaceRequirement>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInterfaceRequirement {
    pub id: InterfaceRequirementId,
    pub name: String,
    pub name_span: Span,
    pub receiver_access: HirAccess,
    pub parameters: Vec<HirInterfaceParameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInterfaceParameter {
    pub mode: HirParameterMode,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirRequirementImplementation {
    pub requirement: InterfaceRequirementId,
    pub method: MethodId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInterfaceConformance {
    pub interface: InterfaceId,
    pub implementations: Vec<HirRequirementImplementation>,
}

#[derive(Clone, Copy, Debug)]
pub struct HirCallableSignature<'hir> {
    pub parameters: &'hir [HirParameter],
    pub return_type: Type,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirClassDeclarationTable {
    entries: DenseIdTable<ClassId, HirClassDeclaration>,
}

impl HirClassDeclarationTable {
    pub(crate) fn new(entries: Vec<HirClassDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |class| class.id),
        }
    }

    pub fn get(&self, id: ClassId) -> Option<&HirClassDeclaration> {
        self.entries.get(id, |class| class.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirClassDeclaration> {
        self.entries.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [HirClassDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirClassDeclaration {
    pub id: ClassId,
    pub module: ModuleId,
    pub name: String,
    pub name_span: Span,
    pub direct_base: Option<HirDirectBase>,
    pub conformances: Vec<HirInterfaceConformance>,
    pub fields: Vec<HirFieldDeclaration>,
    pub static_fields: Vec<super::HirStaticFieldDeclaration>,
    pub initializers: Vec<HirInitializerDeclaration>,
    pub copy_constructor_declaration: Option<HirCopyConstructorDeclaration>,
    pub copy_constructor: HirCopyCapability<CopyConstructorId>,
    pub copy_assignment_declaration: Option<HirCopyAssignmentDeclaration>,
    pub copy_assignment: HirCopyCapability<CopyAssignmentId>,
    pub destructor: Option<HirDestructorDeclaration>,
    pub destruction: HirDestructionPlan,
    pub methods: Vec<HirMethodDeclaration>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDirectBase {
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDestructionPlan {
    pub steps: Vec<HirDestructionStep>,
}

impl HirDestructionPlan {
    pub(crate) fn new(
        destructor: Option<DestructorId>,
        owning_fields: &[HirDestructionStep],
        direct_base: Option<ClassId>,
    ) -> Self {
        let mut steps = Vec::with_capacity(
            usize::from(destructor.is_some())
                + owning_fields.len()
                + usize::from(direct_base.is_some()),
        );
        if let Some(destructor) = destructor {
            steps.push(HirDestructionStep::UserBody(destructor));
        }
        steps.extend(owning_fields.iter().rev().copied());
        if let Some(base) = direct_base {
            steps.push(HirDestructionStep::Base(base));
        }
        Self { steps }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirDestructionStep {
    UserBody(DestructorId),
    Field(FieldId),
    SharedField(FieldId),
    OptionalSharedField(FieldId),
    OptionalClassField(FieldId),
    OptionalField {
        field: FieldId,
        optional: crate::identity::OptionalTypeId,
    },
    ArrayField(FieldId),
    Base(ClassId),
}

impl HirClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&HirFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.fields.get(id.index()).filter(|field| field.id == id)
    }

    pub fn static_field(&self, id: StaticFieldId) -> Option<&super::HirStaticFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.static_fields
            .get(id.index())
            .filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&HirInitializerDeclaration> {
        (id.class() == self.id)
            .then(|| self.initializers.get(id.index()))
            .flatten()
            .filter(|initializer| initializer.id == id)
    }

    pub fn copy_constructor_declaration(
        &self,
        id: CopyConstructorId,
    ) -> Option<&HirCopyConstructorDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.copy_constructor_declaration
            .as_ref()
            .filter(|declaration| declaration.id == id)
    }

    pub fn copy_assignment_declaration(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&HirCopyAssignmentDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.copy_assignment_declaration
            .as_ref()
            .filter(|declaration| declaration.id == id)
    }

    pub fn method(&self, id: MethodId) -> Option<&HirMethodDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.methods
            .get(id.index())
            .filter(|method| method.id == id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&HirDestructorDeclaration> {
        self.destructor
            .as_ref()
            .filter(|destructor| destructor.id == id && id.class() == self.id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirFieldDeclaration {
    pub id: FieldId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<HirParameter>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyConstructorDeclaration {
    pub id: CopyConstructorId,
    pub parameters: Vec<HirParameter>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirCopyAssignmentDeclaration {
    pub id: CopyAssignmentId,
    pub parameter: HirParameter,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirDestructorDeclaration {
    pub id: DestructorId,
    pub receiver_access: HirAccess,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub name_span: Span,
    pub kind: HirMethodKind,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirMethodKind {
    Instance {
        receiver_access: HirAccess,
        dispatch: HirMethodDispatch,
    },
    Static,
}

impl HirMethodKind {
    pub const fn receiver_access(self) -> Option<HirAccess> {
        match self {
            Self::Instance {
                receiver_access, ..
            } => Some(receiver_access),
            Self::Static => None,
        }
    }

    pub const fn dispatch(self) -> Option<HirMethodDispatch> {
        match self {
            Self::Instance { dispatch, .. } => Some(dispatch),
            Self::Static => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirMethodDispatch {
    Direct,
    VirtualRoot {
        family: VirtualFamilyId,
        slot: VirtualSlotId,
    },
    Override {
        family: VirtualFamilyId,
        slot: VirtualSlotId,
        root: MethodId,
        overridden: MethodId,
    },
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirVirtualFamilyTable {
    entries: DenseIdTable<VirtualFamilyId, HirVirtualFamily>,
}

impl HirVirtualFamilyTable {
    pub(crate) fn new(entries: Vec<HirVirtualFamily>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |family| family.id),
        }
    }

    pub fn get(&self, id: VirtualFamilyId) -> Option<&HirVirtualFamily> {
        self.entries.get(id, |family| family.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirVirtualFamily> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HirVirtualFamily {
    pub id: VirtualFamilyId,
    pub slot: VirtualSlotId,
    pub root: MethodId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct HirFunctionDeclarationTable {
    entries: DenseIdTable<FunctionId, HirFunctionDeclaration>,
}

impl HirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<HirFunctionDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&HirFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &HirFunctionDeclaration> {
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
pub struct HirFunctionDeclaration {
    pub id: FunctionId,
    pub module: ModuleId,
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<HirParameter>,
    pub return_type: Type,
    pub linkage: HirFunctionLinkage,
    pub span: Span,
}

impl HirFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&HirParameter> {
        (id.callable() == self.id.into())
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirFunctionLinkage {
    Internal,
    External { link: ExternalLinkId },
    Intrinsic { intrinsic: Intrinsic },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirParameter {
    pub id: ParameterId,
    pub mode: HirParameterMode,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HirParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

impl HirParameterMode {
    pub const fn required_access(self) -> Option<HirAccess> {
        match self {
            Self::Value => None,
            Self::ReadOnlyAlias => Some(HirAccess::ReadOnly),
            Self::MutableAlias => Some(HirAccess::Mutable),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HirLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub ty: Type,
    pub span: Span,
}

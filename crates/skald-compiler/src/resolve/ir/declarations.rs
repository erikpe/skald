//! Source-name-bearing declarations and their typed-ID tables.

use crate::{
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, FieldId,
        FunctionId, InitializerId, InterfaceId, InterfaceRequirementId, LocalId, MethodId,
        ParameterId, VirtualFamilyId, VirtualSlotId,
    },
    source::Span,
};

use super::array_types::ResolvedArrayTypeTable;
use super::body::{
    ResolvedClassDefinitionTable, ResolvedFunctionDefinitionTable, ResolvedMemberDefinition,
};
use super::hierarchy::ResolvedClassHierarchy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub array_types: ResolvedArrayTypeTable,
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    pub classes: ResolvedClassDeclarationTable,
    pub interfaces: ResolvedInterfaceDeclarationTable,
    pub hierarchy: ResolvedClassHierarchy,
    pub virtual_families: ResolvedVirtualFamilyTable,
    pub class_definitions: ResolvedClassDefinitionTable,
    /// Function named `main`, selected during resolution. Type checking
    /// validates its signature and diagnoses its absence.
    pub entry_function: Option<FunctionId>,
    pub span: Span,
}

impl ResolvedProgram {
    pub fn class(&self, id: ClassId) -> Option<&ResolvedClassDeclaration> {
        self.classes.get(id)
    }
    pub fn interface(&self, id: InterfaceId) -> Option<&ResolvedInterfaceDeclaration> {
        self.interfaces.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&ResolvedInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn copy_constructor(
        &self,
        id: CopyConstructorId,
    ) -> Option<&ResolvedCopyConstructorDeclaration> {
        self.class(id.class())?.copy_constructor_declaration(id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&ResolvedDestructorDeclaration> {
        self.class(id.class())?.destructor(id)
    }

    pub fn copy_assignment(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&ResolvedCopyAssignmentDeclaration> {
        self.class(id.class())?.copy_assignment_declaration(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&ResolvedMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&ResolvedMemberDefinition> {
        let class = callable.class()?;
        self.class_definitions.get(class)?.member(callable)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedInterfaceDeclarationTable {
    entries: DenseIdTable<InterfaceId, ResolvedInterfaceDeclaration>,
}
impl ResolvedInterfaceDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedInterfaceDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |entry| entry.id),
        }
    }
    pub fn get(&self, id: InterfaceId) -> Option<&ResolvedInterfaceDeclaration> {
        self.entries.get(id, |entry| entry.id)
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedInterfaceDeclaration> {
        self.entries.iter()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceDeclaration {
    pub id: InterfaceId,
    pub name: String,
    pub name_span: Span,
    pub requirements: Vec<ResolvedInterfaceRequirement>,
    pub span: Span,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceRequirement {
    pub id: InterfaceRequirementId,
    pub name: String,
    pub name_span: Span,
    pub mutable: bool,
    pub parameters: Vec<ResolvedInterfaceParameter>,
    pub return_type: ResolvedType,
    pub span: Span,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceParameter {
    pub binding_mode: ResolvedParameterBindingMode,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedClassDeclarationTable {
    entries: DenseIdTable<ClassId, ResolvedClassDeclaration>,
}

impl ResolvedClassDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedClassDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |class| class.id),
        }
    }

    pub fn get(&self, id: ClassId) -> Option<&ResolvedClassDeclaration> {
        self.entries.get(id, |class| class.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedClassDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn iter_mut(
        &mut self,
    ) -> impl ExactSizeIterator<Item = &mut ResolvedClassDeclaration> {
        self.entries.iter_mut()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedClassDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClassDeclaration {
    pub id: ClassId,
    pub name: String,
    pub name_span: Span,
    pub direct_base: Option<ResolvedDirectBase>,
    pub implemented_interfaces: Vec<ResolvedInterfaceClaim>,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub initializers: Vec<ResolvedInitializerDeclaration>,
    pub copy_constructor_declaration: Option<ResolvedCopyConstructorDeclaration>,
    pub copy_constructor: ResolvedCopyOperation<CopyConstructorId>,
    pub copy_assignment_declaration: Option<ResolvedCopyAssignmentDeclaration>,
    pub copy_assignment: ResolvedCopyOperation<CopyAssignmentId>,
    pub destructor: Option<ResolvedDestructorDeclaration>,
    pub methods: Vec<ResolvedMethodDeclaration>,
    pub span: Span,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceClaim {
    pub interface: InterfaceId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedDirectBase {
    pub class: ClassId,
    pub span: Span,
}

impl ResolvedClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.fields.get(id.index()).filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&ResolvedInitializerDeclaration> {
        (id.class() == self.id)
            .then(|| self.initializers.get(id.index()))
            .flatten()
            .filter(|initializer| initializer.id == id)
    }

    pub fn copy_constructor_declaration(
        &self,
        id: CopyConstructorId,
    ) -> Option<&ResolvedCopyConstructorDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.copy_constructor_declaration
            .as_ref()
            .filter(|constructor| constructor.id == id)
    }

    pub fn copy_assignment_declaration(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&ResolvedCopyAssignmentDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.copy_assignment_declaration
            .as_ref()
            .filter(|assignment| assignment.id == id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&ResolvedDestructorDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.destructor
            .as_ref()
            .filter(|destructor| destructor.id == id)
    }

    pub fn method(&self, id: MethodId) -> Option<&ResolvedMethodDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.methods
            .get(id.index())
            .filter(|method| method.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFieldDeclaration {
    pub id: FieldId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<ResolvedParameter>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCopyConstructorDeclaration {
    pub id: CopyConstructorId,
    pub parameters: Vec<ResolvedParameter>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedCopyOperation<I> {
    User(I),
    Synthesized(ClassId),
    Unavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedCopyAssignmentDeclaration {
    pub id: CopyAssignmentId,
    pub parameter: ResolvedParameter,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedDestructorDeclaration {
    pub id: DestructorId,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedReceiverAccess {
    ReadOnly,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub name_span: Span,
    pub receiver_access: ResolvedReceiverAccess,
    pub modifier: ResolvedMethodModifier,
    pub dispatch: ResolvedMethodDispatch,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMethodModifier {
    Direct,
    Virtual { span: Span },
    Override { span: Span },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMethodDispatch {
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

impl ResolvedMethodDispatch {
    pub const fn family(self) -> Option<VirtualFamilyId> {
        match self {
            Self::Direct => None,
            Self::VirtualRoot { family, .. } | Self::Override { family, .. } => Some(family),
        }
    }

    pub const fn slot(self) -> Option<VirtualSlotId> {
        match self {
            Self::Direct => None,
            Self::VirtualRoot { slot, .. } | Self::Override { slot, .. } => Some(slot),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedVirtualFamilyTable {
    entries: DenseIdTable<VirtualFamilyId, ResolvedVirtualFamily>,
}

impl ResolvedVirtualFamilyTable {
    pub(crate) fn new(entries: Vec<ResolvedVirtualFamily>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |family| family.id),
        }
    }

    pub fn get(&self, id: VirtualFamilyId) -> Option<&ResolvedVirtualFamily> {
        self.entries.get(id, |family| family.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedVirtualFamily> {
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
pub struct ResolvedVirtualFamily {
    pub id: VirtualFamilyId,
    pub slot: VirtualSlotId,
    pub root: MethodId,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResolvedFunctionDeclarationTable {
    entries: DenseIdTable<FunctionId, ResolvedFunctionDeclaration>,
}

impl ResolvedFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<ResolvedFunctionDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&ResolvedFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &ResolvedFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedFunctionDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedFunctionDeclaration {
    pub id: FunctionId,
    /// Retained for diagnostics, dumps, and external-linkage selection.
    pub name: String,
    pub name_span: Span,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub linkage: ResolvedFunctionLinkage,
    pub span: Span,
}

impl ResolvedFunctionDeclaration {
    pub fn parameter(&self, id: ParameterId) -> Option<&ResolvedParameter> {
        (id.callable() == self.id.into())
            .then(|| self.parameters.get(id.index()))
            .flatten()
            .filter(|parameter| parameter.id == id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedFunctionLinkage {
    Internal,
    External { symbol: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedParameter {
    pub id: ParameterId,
    pub binding_mode: ResolvedParameterBindingMode,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedParameterBindingMode {
    Value,
    ReadOnlyAlias { ref_span: Span },
    MutableAlias { mut_span: Span, ref_span: Span },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedLocal {
    pub id: LocalId,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedTypeKind {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(crate::identity::ArrayTypeId),
    Shared(ResolvedSharedTarget),
    Optional {
        payload: ResolvedOptionalPayload,
        payload_span: Span,
        question_span: Span,
    },
    OptionalShared {
        target: ResolvedSharedTarget,
        shared_span: Span,
        question_span: Span,
        target_span: Span,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ResolvedOptionalPayload {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Class(ClassId),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedSharedTarget {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(crate::identity::ArrayTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub kind: ResolvedTypeKind,
    pub span: Span,
}

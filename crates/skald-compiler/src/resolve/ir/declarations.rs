//! Source-name-bearing declarations and their typed-ID tables.

use crate::{
    external::ExternalLinkTable,
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, ExternalLinkId,
        FieldId, FunctionId, FunctionTypeId, InitializerId, InterfaceId, InterfaceRequirementId,
        LocalId, MethodId, ModuleId, OptionalTypeId, ParameterId, StaticFieldId,
        StaticInitializerId, VirtualFamilyId, VirtualSlotId,
    },
    intrinsic::Intrinsic,
    module::ProgramModuleTable,
    source::Span,
};

use super::array_types::ResolvedArrayTypeTable;
use super::body::{
    ResolvedClassDefinitionTable, ResolvedFunctionDefinitionTable, ResolvedMemberDefinition,
};
use super::function_types::ResolvedFunctionTypeTable;
use super::generic_templates::{
    ResolvedClassTemplateSemanticTable, ResolvedClassTemplateTable,
    ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateTable, ResolvedInterfaceType,
    ResolvedTypeParameterTable,
};
use super::hierarchy::ResolvedClassHierarchy;
use super::modules::{
    ResolvedModuleBindingTable, ResolvedModuleDeclarationTable, ResolvedOrdinaryBindingTable,
    ResolvedVisibility,
};
use super::optional_types::ResolvedOptionalTypeTable;
use super::GenericInterfaceSpecializationTable;
use super::GenericSpecializationTable;
use super::ResolvedAddressTakenCallableTable;
use super::{ResolvedOptionalBoxTypeTable, ResolvedSharedTarget};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub modules: ProgramModuleTable,
    pub external_links: ExternalLinkTable,
    pub module_bindings: ResolvedModuleBindingTable,
    pub ordinary_bindings: ResolvedOrdinaryBindingTable,
    pub module_declarations: ResolvedModuleDeclarationTable,
    pub class_templates: ResolvedClassTemplateTable,
    pub interface_templates: ResolvedInterfaceTemplateTable,
    pub interface_template_semantics: ResolvedInterfaceTemplateSemanticTable,
    pub type_parameters: ResolvedTypeParameterTable,
    pub(crate) template_semantics: ResolvedClassTemplateSemanticTable,
    pub(crate) generic_specializations: GenericSpecializationTable,
    pub generic_interface_specializations: GenericInterfaceSpecializationTable,
    pub function_types: ResolvedFunctionTypeTable,
    pub address_taken_callables: ResolvedAddressTakenCallableTable,
    pub array_types: ResolvedArrayTypeTable,
    pub optional_types: ResolvedOptionalTypeTable,
    pub optional_box_types: ResolvedOptionalBoxTypeTable,
    pub iterable_language_item: Option<super::ResolvedIterableLanguageItem>,
    pub operator_language_item: Option<super::ResolvedOperatorLanguageItem>,
    pub range_language_item: Option<super::ResolvedRangeLanguageItem>,
    /// Successful concise-range operator spans retained as compiler-owned
    /// dependency provenance and for construction-origin validation.
    pub range_expression_spans: Vec<Span>,
    pub string_language_item: Option<super::ResolvedStringLanguageItem>,
    pub literal_data: super::ResolvedLiteralDataTable,
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    pub classes: ResolvedClassDeclarationTable,
    pub interfaces: ResolvedInterfaceDeclarationTable,
    pub hierarchy: ResolvedClassHierarchy,
    pub virtual_families: ResolvedVirtualFamilyTable,
    pub class_definitions: ResolvedClassDefinitionTable,
    /// Function named `main` in the selected entry module, selected during
    /// resolution. Type checking validates its signature and diagnoses its
    /// absence.
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

    pub fn static_field(&self, id: StaticFieldId) -> Option<&ResolvedStaticFieldDeclaration> {
        self.class(id.class())?.static_field(id)
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
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub(crate) fn extend(&mut self, entries: Vec<ResolvedInterfaceDeclaration>) {
        self.entries.extend(entries, |entry| entry.id);
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceDeclaration {
    pub id: InterfaceId,
    pub module: ModuleId,
    pub visibility: ResolvedVisibility,
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

    pub(crate) fn get_mut(&mut self, id: ClassId) -> Option<&mut ResolvedClassDeclaration> {
        self.entries.get_mut(id, |class| class.id)
    }

    pub(crate) fn extend(&mut self, entries: Vec<ResolvedClassDeclaration>) {
        self.entries.extend(entries, |class| class.id);
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [ResolvedClassDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedClassDeclaration {
    pub id: ClassId,
    pub module: ModuleId,
    pub visibility: ResolvedVisibility,
    pub name: String,
    pub name_span: Span,
    pub direct_base: Option<ResolvedDirectBase>,
    pub implemented_interfaces: Vec<ResolvedInterfaceClaim>,
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub static_fields: Vec<ResolvedStaticFieldDeclaration>,
    pub initializers: Vec<ResolvedInitializerDeclaration>,
    pub copy_constructor_declaration: Option<ResolvedCopyConstructorDeclaration>,
    pub copy_constructor: ResolvedCopyOperation<CopyConstructorId>,
    pub copy_assignment_declaration: Option<ResolvedCopyAssignmentDeclaration>,
    pub copy_assignment: ResolvedCopyOperation<CopyAssignmentId>,
    pub destructor: Option<ResolvedDestructorDeclaration>,
    pub methods: Vec<ResolvedMethodDeclaration>,
    pub span: Span,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInterfaceClaim {
    pub interface: ResolvedInterfaceType,
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

    pub fn static_field(&self, id: StaticFieldId) -> Option<&ResolvedStaticFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.static_fields
            .get(id.index())
            .filter(|field| field.id == id)
    }

    pub(crate) fn static_field_mut(
        &mut self,
        id: StaticFieldId,
    ) -> Option<&mut ResolvedStaticFieldDeclaration> {
        if id.class() != self.id {
            return None;
        }
        self.static_fields
            .get_mut(id.index())
            .filter(|field| field.id == id)
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
    pub visibility: ResolvedMemberVisibility,
    pub cell_span: Option<Span>,
    pub final_span: Option<Span>,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStaticFieldDeclaration {
    pub id: StaticFieldId,
    pub visibility: ResolvedMemberVisibility,
    pub static_span: Span,
    pub final_span: Option<Span>,
    pub name: String,
    pub name_span: Span,
    pub type_syntax: ResolvedType,
    pub initializer: Option<ResolvedStaticFieldInitializer>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedStaticFieldInitializer {
    pub id: StaticInitializerId,
    pub equal_span: Span,
    pub expression: super::ResolvedExpression,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMemberVisibility {
    Public,
    Private { span: Span },
}

impl ResolvedMemberVisibility {
    pub const fn private_span(self) -> Option<Span> {
        match self {
            Self::Public => None,
            Self::Private { span } => Some(span),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInitializerDeclaration {
    pub id: InitializerId,
    pub visibility: ResolvedMemberVisibility,
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
    pub visibility: ResolvedMemberVisibility,
    pub name: String,
    pub name_span: Span,
    pub kind: ResolvedMethodKind,
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedMethodKind {
    Instance {
        receiver_access: ResolvedReceiverAccess,
        modifier: ResolvedMethodModifier,
        dispatch: ResolvedMethodDispatch,
    },
    Static,
}

impl ResolvedMethodKind {
    pub const fn receiver_access(self) -> Option<ResolvedReceiverAccess> {
        match self {
            Self::Instance {
                receiver_access, ..
            } => Some(receiver_access),
            Self::Static => None,
        }
    }

    pub const fn modifier(self) -> Option<ResolvedMethodModifier> {
        match self {
            Self::Instance { modifier, .. } => Some(modifier),
            Self::Static => None,
        }
    }

    pub const fn dispatch(self) -> Option<ResolvedMethodDispatch> {
        match self {
            Self::Instance { dispatch, .. } => Some(dispatch),
            Self::Static => None,
        }
    }

    pub(crate) fn set_instance_dispatch(&mut self, value: ResolvedMethodDispatch) {
        let Self::Instance { dispatch, .. } = self else {
            debug_assert!(false, "static methods cannot receive instance dispatch");
            return;
        };
        *dispatch = value;
    }
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
    pub module: ModuleId,
    pub visibility: ResolvedVisibility,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResolvedFunctionLinkage {
    Internal,
    External {
        link: ExternalLinkId,
    },
    Intrinsic {
        intrinsic: Intrinsic,
    },
    /// An `intrinsic fn` outside the compiler's closed registry.
    ///
    /// Resolution diagnoses this state before HIR construction. Keeping it
    /// explicit avoids assigning a valid semantic identity to malformed
    /// source while diagnostics are collected.
    UnrecognizedIntrinsic,
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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
    Function(FunctionTypeId),
    Array(crate::identity::ArrayTypeId),
    Shared(ResolvedSharedTarget),
    Optional(OptionalTypeId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub kind: ResolvedTypeKind,
    pub span: Span,
}

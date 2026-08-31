//! MIR program metadata, declarations, and typed-ID tables.

use std::fmt;

use crate::{
    external::ExternalLinkTable,
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, CopyConstructorId, DestructorId, ExternalLinkId,
        FieldId, FunctionId, InitializerId, InterfaceId, InterfaceRequirementId, MethodId,
        ModuleId, StaticFieldId, VirtualFamilyId, VirtualSlotId,
    },
    intrinsic::Intrinsic,
    module::ProgramModuleTable,
    source::Span,
};

use super::{
    array::{MirArrayType, MirArrayTypeTable},
    definition::{
        MirDefinitionRef, MirFunctionDefinitionTable, MirMemberDefinition, MirMemberDefinitionTable,
    },
    interface::{MirInterfaceConformance, MirInterfaceDeclarationTable, MirInterfaceRequirement},
    shared::MirSharedTarget,
    value::MirType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgram {
    pub modules: ProgramModuleTable,
    pub external_links: ExternalLinkTable,
    pub function_types: super::MirFunctionTypeTable,
    pub array_types: MirArrayTypeTable,
    pub optional_types: super::MirOptionalTypeTable,
    pub optional_box_types: super::MirOptionalBoxTypeTable,
    pub string_language_item: Option<super::MirStringLanguageItem>,
    pub literal_data: super::MirLiteralDataTable,
    pub classes: MirClassDeclarationTable,
    pub interfaces: MirInterfaceDeclarationTable,
    pub virtual_families: MirVirtualFamilyTable,
    pub declarations: MirFunctionDeclarationTable,
    pub definitions: MirFunctionDefinitionTable,
    pub member_definitions: MirMemberDefinitionTable,
    /// Present after static-lifecycle synthesis. Backends never receive the
    /// preliminary or merely planned products.
    pub static_lifecycle: Option<super::MirStaticLifecycleCoordinator>,
    pub entry_function: FunctionId,
    pub span: Span,
}

impl MirProgram {
    pub fn function_type(
        &self,
        id: crate::identity::FunctionTypeId,
    ) -> Option<&super::MirFunctionType> {
        self.function_types.get(id)
    }

    pub fn array_type(&self, id: crate::identity::ArrayTypeId) -> Option<&MirArrayType> {
        self.array_types.get(id)
    }

    pub fn optional_type(
        &self,
        id: crate::identity::OptionalTypeId,
    ) -> Option<&super::MirOptionalType> {
        self.optional_types.get(id)
    }

    pub fn optional_box_type(
        &self,
        id: crate::identity::OptionalBoxTypeId,
    ) -> Option<&super::MirOptionalBoxType> {
        self.optional_box_types.get(id)
    }

    /// Returns the exact optional-box identity that owns `optional` storage.
    ///
    /// View-only polymorphic box identities deliberately do not participate:
    /// allocation metadata always names the one exact physical wrapper.
    pub fn exact_optional_box_type(
        &self,
        optional: crate::identity::OptionalTypeId,
    ) -> Option<&super::MirOptionalBoxType> {
        self.optional_box_types
            .iter()
            .find(|box_type| box_type.exact_optional == Some(optional))
    }

    pub fn shared_target_type(&self, target: MirSharedTarget) -> Option<MirType> {
        match target {
            MirSharedTarget::OptionalBox(box_type) => self
                .optional_box_type(box_type)?
                .exact_optional
                .map(MirType::Optional),
            target => Some(target.ty()),
        }
    }

    pub fn optional_for_payload(
        &self,
        payload: MirType,
    ) -> Option<crate::identity::OptionalTypeId> {
        self.optional_types
            .iter()
            .find(|optional| optional.payload == payload)
            .map(|optional| optional.id)
    }

    pub fn class(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.classes.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn static_field(&self, id: StaticFieldId) -> Option<&MirStaticFieldDeclaration> {
        self.class(id.class())?.static_field(id)
    }

    /// Returns the source-facing owner and field name for one identity-selected
    /// static slot.
    ///
    /// Closed generic classes carry their canonical type arguments in the
    /// class name, so this remains readable without weakening identity-based
    /// lookup in analysis, planning, or code generation.
    pub(crate) fn static_field_qualified_name(&self, id: StaticFieldId) -> Option<String> {
        let class = self.class(id.class())?;
        let field = class.static_field(id)?;
        Some(format!("{}.{}", class.name, field.name))
    }

    pub fn direct_base(&self, class: ClassId) -> Option<ClassId> {
        self.class(class)?.direct_base.map(|base| base.class)
    }

    pub fn is_ancestor(&self, ancestor: ClassId, mut class: ClassId) -> bool {
        for _ in 0..self.classes.len() {
            let Some(base) = self.direct_base(class) else {
                return false;
            };
            if base == ancestor {
                return true;
            }
            class = base;
        }
        false
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn copy_constructor(
        &self,
        id: CopyConstructorId,
    ) -> Option<&MirCopyConstructorDeclaration> {
        self.class(id.class())?.copy_constructor_declaration(id)
    }

    pub fn copy_assignment(&self, id: CopyAssignmentId) -> Option<&MirCopyAssignmentDeclaration> {
        self.class(id.class())?.copy_assignment_declaration(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&MirMethodDeclaration> {
        self.class(id.class())?.method(id)
    }

    pub fn interface(&self, id: InterfaceId) -> Option<&super::interface::MirInterfaceDeclaration> {
        self.interfaces.get(id)
    }

    pub fn interface_requirement(
        &self,
        id: InterfaceRequirementId,
    ) -> Option<&MirInterfaceRequirement> {
        self.interface(id.interface())?.requirement(id)
    }

    pub fn conformance(
        &self,
        class: ClassId,
        interface: InterfaceId,
    ) -> Option<&MirInterfaceConformance> {
        self.class(class)?
            .conformances
            .iter()
            .find(|conformance| conformance.interface == interface)
    }

    pub fn virtual_family(&self, id: VirtualFamilyId) -> Option<&MirVirtualFamily> {
        self.virtual_families.get(id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&MirDestructorDeclaration> {
        self.class(id.class())?.destructor(id)
    }

    pub fn member_definition(&self, callable: CallableId) -> Option<&MirMemberDefinition> {
        self.member_definitions.get(callable)
    }

    pub fn executable_definitions(&self) -> impl Iterator<Item = MirDefinitionRef<'_>> {
        self.definitions
            .iter()
            .map(MirDefinitionRef::Function)
            .chain(self.member_definitions.iter().map(MirDefinitionRef::Member))
            .chain(
                self.static_lifecycle
                    .iter()
                    .flat_map(|coordinator| coordinator.initializers())
                    .map(MirDefinitionRef::from),
            )
    }

    /// Whether one declared callable has a body physically retained in this
    /// final-MIR product.
    pub(crate) fn has_executable_definition(&self, callable: CallableId) -> bool {
        match callable {
            CallableId::Function(function) => self.definitions.get(function).is_some(),
            CallableId::StaticInitializer(initializer) => {
                self.static_lifecycle.as_ref().is_some_and(|coordinator| {
                    coordinator
                        .initializers()
                        .iter()
                        .any(|body| body.id == initializer)
                })
            }
            CallableId::Initializer(_)
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => self.member_definition(callable).is_some(),
        }
    }

    /// Removes exactly one executable body for sparse final-MIR verifier tests.
    /// Production definition retention is introduced through its own atomic
    /// ownership boundary rather than through mutable table access.
    #[cfg(test)]
    pub(crate) fn remove_executable_definition_for_test(&mut self, callable: CallableId) {
        assert!(
            self.has_executable_definition(callable),
            "test fixture must contain executable definition {callable}"
        );
        match callable {
            CallableId::Function(function) => self.definitions.remove_for_test(function),
            CallableId::StaticInitializer(initializer) => self
                .static_lifecycle
                .as_mut()
                .expect("static initializer definition requires a coordinator")
                .initializers_mut_for_test()
                .retain(|body| body.id != initializer),
            CallableId::Initializer(_)
            | CallableId::CopyConstructor(_)
            | CallableId::CopyAssignment(_)
            | CallableId::Destructor(_)
            | CallableId::Method(_) => self.member_definitions.remove_for_test(callable),
        }
    }

    /// Expands a shared static view to the finite set of linked dynamic
    /// lifecycle implementations that may own its allocation.
    pub(crate) fn shared_lifecycle_targets(
        &self,
        target: MirSharedTarget,
    ) -> Vec<super::PreliminaryMirSharedLifecycleTarget> {
        match target {
            MirSharedTarget::Array(array) => {
                vec![super::PreliminaryMirSharedLifecycleTarget::Array(array)]
            }
            MirSharedTarget::Obj => self
                .classes
                .iter()
                .map(|class| super::PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
            MirSharedTarget::Class(base) => self
                .classes
                .iter()
                .filter(|class| class.id == base || self.is_ancestor(base, class.id))
                .map(|class| super::PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
            MirSharedTarget::Interface(interface) => self
                .classes
                .iter()
                .filter(|class| self.conformance(class.id, interface).is_some())
                .map(|class| super::PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
            MirSharedTarget::OptionalBox(target) => {
                vec![super::PreliminaryMirSharedLifecycleTarget::OptionalBox(
                    target,
                )]
            }
        }
    }

    pub fn callable_signature(&self, callable: CallableId) -> Option<MirCallableSignature<'_>> {
        match callable {
            CallableId::Function(function) => {
                self.declarations
                    .get(function)
                    .map(|declaration| MirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: declaration.return_type,
                    })
            }
            CallableId::StaticInitializer(initializer) => self
                .static_lifecycle
                .as_ref()
                .and_then(|coordinator| {
                    coordinator
                        .initializers()
                        .iter()
                        .find(|body| body.id == initializer)
                })
                .map(|_| MirCallableSignature {
                    parameters: &[],
                    return_type: MirType::Unit,
                }),
            CallableId::Initializer(initializer) => {
                self.initializer(initializer)
                    .map(|declaration| MirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: MirType::Unit,
                    })
            }
            CallableId::CopyConstructor(copy) => {
                self.copy_constructor(copy)
                    .map(|declaration| MirCallableSignature {
                        parameters: &declaration.parameters,
                        return_type: MirType::Unit,
                    })
            }
            CallableId::CopyAssignment(assignment) => {
                self.copy_assignment(assignment)
                    .map(|declaration| MirCallableSignature {
                        parameters: std::slice::from_ref(&declaration.parameter),
                        return_type: MirType::Unit,
                    })
            }
            CallableId::Destructor(destructor) => {
                self.destructor(destructor).map(|_| MirCallableSignature {
                    parameters: &[],
                    return_type: MirType::Unit,
                })
            }
            CallableId::Method(method) => {
                self.method(method).map(|declaration| MirCallableSignature {
                    parameters: &declaration.parameters,
                    return_type: declaration.return_type,
                })
            }
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirVirtualFamilyTable {
    entries: DenseIdTable<VirtualFamilyId, MirVirtualFamily>,
}

impl MirVirtualFamilyTable {
    pub(crate) fn new(entries: Vec<MirVirtualFamily>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |family| family.id),
        }
    }

    pub fn get(&self, id: VirtualFamilyId) -> Option<&MirVirtualFamily> {
        self.entries.get(id, |family| family.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirVirtualFamily> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirVirtualFamily] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVirtualFamily {
    pub id: VirtualFamilyId,
    pub slot: VirtualSlotId,
    pub root: MethodId,
    /// Root followed by overrides in deterministic declaration order.
    pub members: Vec<MethodId>,
}

#[derive(Clone, Copy, Debug)]
pub struct MirCallableSignature<'mir> {
    pub parameters: &'mir [MirParameter],
    pub return_type: MirType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirParameter {
    pub mode: MirParameterMode,
    pub ty: MirType,
}

impl MirParameter {
    pub const fn value(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::Value,
            ty,
        }
    }

    pub const fn read_only_alias(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::ReadOnlyAlias,
            ty,
        }
    }

    pub const fn mutable_alias(ty: MirType) -> Self {
        Self {
            mode: MirParameterMode::MutableAlias,
            ty,
        }
    }

    pub fn values(types: impl IntoIterator<Item = MirType>) -> Vec<Self> {
        types.into_iter().map(Self::value).collect()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirParameterMode {
    Value,
    ReadOnlyAlias,
    MutableAlias,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirClassDeclarationTable {
    entries: DenseIdTable<ClassId, MirClassDeclaration>,
}

impl MirClassDeclarationTable {
    pub(crate) fn new(entries: Vec<MirClassDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |class| class.id),
        }
    }

    pub fn get(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirClassDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirClassDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirClassDeclaration {
    pub id: ClassId,
    pub module: ModuleId,
    pub name: String,
    pub direct_base: Option<MirDirectBase>,
    pub conformances: Vec<MirInterfaceConformance>,
    pub fields: Vec<MirFieldDeclaration>,
    pub static_fields: Vec<MirStaticFieldDeclaration>,
    pub initializers: Vec<MirInitializerDeclaration>,
    pub copy_constructor_declaration: Option<MirCopyConstructorDeclaration>,
    pub copy_constructor: MirCopyCapability<CopyConstructorId>,
    pub copy_assignment_declaration: Option<MirCopyAssignmentDeclaration>,
    pub copy_assignment: MirCopyCapability<CopyAssignmentId>,
    pub destruction: MirDestructionPlan,
    pub methods: Vec<MirMethodDeclaration>,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirDirectBase {
    pub class: ClassId,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirStaticFieldDeclaration {
    pub id: StaticFieldId,
    pub final_span: Option<Span>,
    pub name: String,
    pub ty: MirType,
    pub initialization: super::MirStaticFieldInitialization,
    pub span: Span,
}

impl MirClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        (id.class() == self.id)
            .then(|| self.fields.get(id.index()))
            .flatten()
            .filter(|field| field.id == id)
    }

    pub fn static_field(&self, id: StaticFieldId) -> Option<&MirStaticFieldDeclaration> {
        (id.class() == self.id)
            .then(|| self.static_fields.get(id.index()))
            .flatten()
            .filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        (id.class() == self.id)
            .then(|| self.initializers.get(id.index()))
            .flatten()
            .filter(|initializer| initializer.id == id)
    }

    pub fn copy_constructor_declaration(
        &self,
        id: CopyConstructorId,
    ) -> Option<&MirCopyConstructorDeclaration> {
        self.copy_constructor_declaration
            .as_ref()
            .filter(|declaration| declaration.id == id && id.class() == self.id)
    }

    pub fn copy_assignment_declaration(
        &self,
        id: CopyAssignmentId,
    ) -> Option<&MirCopyAssignmentDeclaration> {
        self.copy_assignment_declaration
            .as_ref()
            .filter(|declaration| declaration.id == id && id.class() == self.id)
    }

    pub fn method(&self, id: MethodId) -> Option<&MirMethodDeclaration> {
        (id.class() == self.id)
            .then(|| self.methods.get(id.index()))
            .flatten()
            .filter(|method| method.id == id)
    }

    pub fn destructor(&self, id: DestructorId) -> Option<&MirDestructorDeclaration> {
        self.destruction
            .destructor
            .as_ref()
            .filter(|destructor| destructor.id == id && id.class() == self.id)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFieldDeclaration {
    pub id: FieldId,
    pub cell_span: Option<Span>,
    pub final_span: Option<Span>,
    pub name: String,
    pub ty: MirType,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirInitializerDeclaration {
    pub id: InitializerId,
    pub parameters: Vec<MirParameter>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCopyConstructorDeclaration {
    pub id: CopyConstructorId,
    pub parameters: Vec<MirParameter>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirCopyAssignmentDeclaration {
    pub id: CopyAssignmentId,
    pub parameter: MirParameter,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCopyCapability<I> {
    User(MirUserCopy<I>),
    Synthesized(MirSynthesizedCopy<I>),
    Unavailable,
}

impl<I: Copy> MirCopyCapability<I> {
    pub const fn selected(&self) -> Option<MirSelectedCopyOperation<I>> {
        match self {
            Self::User(copy) => Some(MirSelectedCopyOperation::User(copy.operation)),
            Self::Synthesized(copy) => Some(MirSelectedCopyOperation::Synthesized(copy.class)),
            Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirUserCopy<I> {
    pub operation: I,
    pub base: Option<MirBaseCopy<I>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSynthesizedCopy<I> {
    pub class: ClassId,
    pub base: Option<MirBaseCopy<I>>,
    pub fields: Vec<MirSynthesizedFieldCopy<I>>,
    pub final_fields: Vec<FieldId>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirBaseCopy<I> {
    pub base: ClassId,
    pub operation: MirSelectedCopyOperation<I>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSynthesizedFieldCopy<I> {
    Primitive {
        field: FieldId,
    },
    OptionalPrimitive {
        field: FieldId,
        payload: super::MirPrimitiveType,
    },
    OptionalClass {
        field: FieldId,
        class: ClassId,
        operation: MirSelectedCopyOperation<I>,
    },
    Shared {
        field: FieldId,
    },
    OptionalShared {
        field: FieldId,
        target: MirSharedTarget,
    },
    Optional {
        field: FieldId,
        optional: crate::identity::OptionalTypeId,
    },
    Class {
        field: FieldId,
        operation: MirSelectedCopyOperation<I>,
    },
    Array {
        field: FieldId,
        array: crate::identity::ArrayTypeId,
    },
}

impl<I> MirSynthesizedFieldCopy<I> {
    pub const fn field(&self) -> FieldId {
        match self {
            Self::Primitive { field }
            | Self::OptionalPrimitive { field, .. }
            | Self::OptionalClass { field, .. }
            | Self::Shared { field }
            | Self::OptionalShared { field, .. }
            | Self::Optional { field, .. }
            | Self::Class { field, .. }
            | Self::Array { field, .. } => *field,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSelectedCopyOperation<I> {
    User(I),
    Synthesized(ClassId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirDestructorDeclaration {
    pub id: DestructorId,
    pub receiver_access: MirReceiverAccess,
    pub span: Span,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirDestructionPlan {
    pub destructor: Option<MirDestructorDeclaration>,
    pub steps: Vec<MirDestructionStep>,
}

impl MirDestructionPlan {
    /// Builds the canonical complete-object order from class-typed fields in
    /// declaration order: the optional user body first, then fields in reverse.
    pub fn new(destructor: Option<MirDestructorDeclaration>, class_fields: &[FieldId]) -> Self {
        Self::with_base(destructor, class_fields, None)
    }

    pub fn with_base(
        destructor: Option<MirDestructorDeclaration>,
        class_fields: &[FieldId],
        direct_base: Option<ClassId>,
    ) -> Self {
        let mut steps = Vec::with_capacity(
            class_fields.len()
                + usize::from(destructor.is_some())
                + usize::from(direct_base.is_some()),
        );
        if let Some(declaration) = &destructor {
            steps.push(MirDestructionStep::UserBody(declaration.id));
        }
        steps.extend(
            class_fields
                .iter()
                .rev()
                .copied()
                .map(MirDestructionStep::Field),
        );
        if let Some(base) = direct_base {
            steps.push(MirDestructionStep::Base(base));
        }
        Self { destructor, steps }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDestructionStep {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirReceiverAccess {
    ReadOnly,
    Mutable,
}

impl fmt::Display for MirReceiverAccess {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadOnly => formatter.write_str("readonly"),
            Self::Mutable => formatter.write_str("mutable"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirMethodDeclaration {
    pub id: MethodId,
    pub name: String,
    pub kind: MirMethodKind,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirMethodKind {
    Instance { receiver_access: MirReceiverAccess },
    Static,
}

impl MirMethodKind {
    pub const fn instance(receiver_access: MirReceiverAccess) -> Self {
        Self::Instance { receiver_access }
    }

    pub const fn receiver_access(self) -> Option<MirReceiverAccess> {
        match self {
            Self::Instance { receiver_access } => Some(receiver_access),
            Self::Static => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirFunctionDeclarationTable {
    entries: DenseIdTable<FunctionId, MirFunctionDeclaration>,
}

impl MirFunctionDeclarationTable {
    pub(crate) fn new(entries: Vec<MirFunctionDeclaration>) -> Self {
        Self {
            entries: DenseIdTable::new(entries, |declaration| declaration.id),
        }
    }

    pub fn get(&self, id: FunctionId) -> Option<&MirFunctionDeclaration> {
        self.entries.get(id, |declaration| declaration.id)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirFunctionDeclaration> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn entries_mut_for_test(&mut self) -> &mut [MirFunctionDeclaration] {
        self.entries.entries_mut_for_test()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirFunctionDeclaration {
    pub id: FunctionId,
    pub module: ModuleId,
    pub name: String,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub linkage: MirFunctionLinkage,
    pub span: Span,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirFunctionLinkage {
    Internal,
    External { link: ExternalLinkId },
    Intrinsic { intrinsic: Intrinsic },
}

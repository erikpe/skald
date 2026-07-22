//! MIR program metadata, declarations, and typed-ID tables.

use std::fmt;

use crate::{
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, DestructorId, FieldId, FunctionId, InitializerId,
        MethodId,
    },
    source::Span,
};

use super::{
    definition::{
        MirDefinitionRef, MirFunctionDefinitionTable, MirMemberDefinition, MirMemberDefinitionTable,
    },
    value::MirType,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirProgram {
    pub classes: MirClassDeclarationTable,
    pub declarations: MirFunctionDeclarationTable,
    pub definitions: MirFunctionDefinitionTable,
    pub member_definitions: MirMemberDefinitionTable,
    pub entry_function: FunctionId,
    pub span: Span,
}

impl MirProgram {
    pub fn class(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.classes.get(id)
    }

    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
    }

    pub fn copy_assignment(&self, id: CopyAssignmentId) -> Option<&MirCopyAssignmentDeclaration> {
        self.class(id.class())?.copy_assignment_declaration(id)
    }

    pub fn method(&self, id: MethodId) -> Option<&MirMethodDeclaration> {
        self.class(id.class())?.method(id)
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
            CallableId::Initializer(initializer) => {
                self.initializer(initializer)
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
    pub name: String,
    pub fields: Vec<MirFieldDeclaration>,
    pub initializers: Vec<MirInitializerDeclaration>,
    pub copy_constructor_declaration: Option<MirInitializerDeclaration>,
    pub copy_constructor: MirCopyCapability<InitializerId>,
    pub copy_assignment_declaration: Option<MirCopyAssignmentDeclaration>,
    pub copy_assignment: MirCopyCapability<CopyAssignmentId>,
    pub destruction: MirDestructionPlan,
    pub methods: Vec<MirMethodDeclaration>,
    pub span: Span,
}

impl MirClassDeclaration {
    pub fn field(&self, id: FieldId) -> Option<&MirFieldDeclaration> {
        (id.class() == self.id)
            .then(|| self.fields.get(id.index()))
            .flatten()
            .filter(|field| field.id == id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&MirInitializerDeclaration> {
        let ordinary = (id.class() == self.id)
            .then(|| self.initializers.get(id.index()))
            .flatten()
            .filter(|initializer| initializer.id == id);
        ordinary.or_else(|| {
            self.copy_constructor_declaration
                .as_ref()
                .filter(|declaration| declaration.id == id && id.class() == self.id)
        })
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
pub struct MirCopyAssignmentDeclaration {
    pub id: CopyAssignmentId,
    pub parameter: MirParameter,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirCopyCapability<I> {
    User(I),
    Synthesized(MirSynthesizedCopy<I>),
    Unavailable,
}

impl<I: Copy> MirCopyCapability<I> {
    pub const fn selected(&self) -> Option<MirSelectedCopyOperation<I>> {
        match self {
            Self::User(id) => Some(MirSelectedCopyOperation::User(*id)),
            Self::Synthesized(copy) => Some(MirSelectedCopyOperation::Synthesized(copy.class)),
            Self::Unavailable => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirSynthesizedCopy<I> {
    pub class: ClassId,
    pub fields: Vec<MirSynthesizedFieldCopy<I>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirSynthesizedFieldCopy<I> {
    Primitive {
        field: FieldId,
    },
    Class {
        field: FieldId,
        operation: MirSelectedCopyOperation<I>,
    },
}

impl<I> MirSynthesizedFieldCopy<I> {
    pub const fn field(&self) -> FieldId {
        match self {
            Self::Primitive { field } | Self::Class { field, .. } => *field,
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
        let mut steps = Vec::with_capacity(class_fields.len() + usize::from(destructor.is_some()));
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
        Self { destructor, steps }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MirDestructionStep {
    UserBody(DestructorId),
    Field(FieldId),
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
    pub receiver_access: MirReceiverAccess,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub span: Span,
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
    pub name: String,
    pub parameters: Vec<MirParameter>,
    pub return_type: MirType,
    pub linkage: MirFunctionLinkage,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirFunctionLinkage {
    Internal,
    External { symbol: String },
}

//! Source-name-bearing declarations and their typed-ID tables.

use crate::{
    id_table::DenseIdTable,
    identity::{
        CallableId, ClassId, CopyAssignmentId, DestructorId, FieldId, FunctionId, InitializerId,
        LocalId, MethodId, ParameterId,
    },
    source::Span,
};

use super::body::{
    ResolvedClassDefinitionTable, ResolvedFunctionDefinitionTable, ResolvedMemberDefinition,
};
use super::hierarchy::ResolvedClassHierarchy;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedProgram {
    pub declarations: ResolvedFunctionDeclarationTable,
    pub definitions: ResolvedFunctionDefinitionTable,
    pub classes: ResolvedClassDeclarationTable,
    pub hierarchy: ResolvedClassHierarchy,
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

    pub fn field(&self, id: FieldId) -> Option<&ResolvedFieldDeclaration> {
        self.class(id.class())?.field(id)
    }

    pub fn initializer(&self, id: InitializerId) -> Option<&ResolvedInitializerDeclaration> {
        self.class(id.class())?.initializer(id)
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
    pub fields: Vec<ResolvedFieldDeclaration>,
    pub initializer: Option<ResolvedInitializerDeclaration>,
    pub copy_constructor_declaration: Option<ResolvedInitializerDeclaration>,
    pub copy_constructor: ResolvedCopyOperation<InitializerId>,
    pub copy_assignment_declaration: Option<ResolvedCopyAssignmentDeclaration>,
    pub copy_assignment: ResolvedCopyOperation<CopyAssignmentId>,
    pub destructor: Option<ResolvedDestructorDeclaration>,
    pub methods: Vec<ResolvedMethodDeclaration>,
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
        if id.class() != self.id {
            return None;
        }
        self.initializer
            .as_ref()
            .filter(|initializer| initializer.id == id)
            .or_else(|| {
                self.copy_constructor_declaration
                    .as_ref()
                    .filter(|initializer| initializer.id == id)
            })
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
    pub parameters: Vec<ResolvedParameter>,
    pub return_type: ResolvedType,
    pub span: Span,
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
    Class(ClassId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedType {
    pub kind: ResolvedTypeKind,
    pub span: Span,
}

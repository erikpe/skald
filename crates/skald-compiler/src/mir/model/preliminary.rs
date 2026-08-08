//! Unplanned MIR produced before whole-program static lifecycle analysis.

use crate::{
    identity::{ArrayTypeId, ClassId, StaticFieldId, StaticInitializerId},
    source::Span,
};

use super::{
    BlockId, MirArrayType, MirBasicBlock, MirBody, MirClassDeclaration, MirDefinitionRef,
    MirProgram, MirSharedTarget, MirStorage, MirType, MirValue, StorageId, ValueId,
};

/// The one control-flow edge after destination completion and before
/// full-expression cleanup.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MirStaticPublication {
    pub initialization_exit: BlockId,
    pub cleanup_entry: BlockId,
    pub span: Span,
}

/// One independently analyzable explicit static declaration initializer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreliminaryMirStaticInitializer {
    pub id: StaticInitializerId,
    pub field: StaticFieldId,
    pub destination_type: MirType,
    pub publication: MirStaticPublication,
    pub storage: Vec<MirStorage>,
    pub values: Vec<MirValue>,
    pub body: MirBody,
    pub span: Span,
}

/// Preliminary initialization mode retained for every declared static slot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreliminaryMirStaticField {
    pub field: StaticFieldId,
    pub ty: MirType,
    pub initializer: Option<StaticInitializerId>,
    pub span: Span,
}

/// One closed-world dynamic lifecycle implementation reachable through a
/// shared-owner target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreliminaryMirSharedLifecycleTarget {
    Class(ClassId),
    Array(ArrayTypeId),
}

impl PreliminaryMirStaticInitializer {
    pub const fn callable(&self) -> crate::identity::CallableId {
        crate::identity::CallableId::StaticInitializer(self.id)
    }

    pub fn storage(&self, id: StorageId) -> Option<&MirStorage> {
        (id.callable() == self.callable())
            .then(|| self.storage.get(id.index()))
            .flatten()
            .filter(|storage| storage.id == id)
    }

    pub fn value(&self, id: ValueId) -> Option<&MirValue> {
        (id.callable() == self.callable())
            .then(|| self.values.get(id.index()))
            .flatten()
            .filter(|value| value.id == id)
    }

    pub fn block(&self, id: BlockId) -> Option<&MirBasicBlock> {
        (id.callable() == self.callable())
            .then(|| self.body.blocks.get(id.index()))
            .flatten()
            .filter(|block| block.id == id)
    }
}

/// Closed-world MIR before static dependency analysis and lifecycle planning.
///
/// The ordinary program is intentionally private. A product that still owns
/// static initializer bodies cannot be passed to a backend as `MirProgram`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreliminaryMirProgram {
    program: MirProgram,
    static_fields: Vec<PreliminaryMirStaticField>,
    static_initializers: Vec<PreliminaryMirStaticInitializer>,
}

impl PreliminaryMirProgram {
    pub(crate) fn new(
        program: MirProgram,
        static_fields: Vec<PreliminaryMirStaticField>,
        static_initializers: Vec<PreliminaryMirStaticInitializer>,
    ) -> Self {
        Self {
            program,
            static_fields,
            static_initializers,
        }
    }

    pub fn static_fields(&self) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticField> {
        self.static_fields.iter()
    }

    pub fn static_initializer(
        &self,
        id: StaticInitializerId,
    ) -> Option<&PreliminaryMirStaticInitializer> {
        self.static_initializers
            .iter()
            .find(|initializer| initializer.id == id)
    }

    pub fn static_initializers(
        &self,
    ) -> impl ExactSizeIterator<Item = &PreliminaryMirStaticInitializer> {
        self.static_initializers.iter()
    }

    pub fn has_static_initializers(&self) -> bool {
        self.static_fields
            .iter()
            .any(|field| field.initializer.is_some())
    }

    /// Iterates every ordinary and static-initializer body in stable identity
    /// order. Static lifecycle analysis must scan this complete body set.
    pub fn executable_definitions(&self) -> impl Iterator<Item = MirDefinitionRef<'_>> {
        self.program
            .executable_definitions()
            .chain(self.static_initializers.iter().map(MirDefinitionRef::from))
    }

    pub fn class(&self, id: ClassId) -> Option<&MirClassDeclaration> {
        self.program.class(id)
    }

    pub fn array_type(&self, id: ArrayTypeId) -> Option<&MirArrayType> {
        self.program.array_type(id)
    }

    /// Expands the static view carried by a shared owner to the conservative,
    /// finite set of dynamic lifecycle implementations in this linked program.
    pub fn shared_lifecycle_targets(
        &self,
        target: MirSharedTarget,
    ) -> Vec<PreliminaryMirSharedLifecycleTarget> {
        match target {
            MirSharedTarget::Array(array) => {
                vec![PreliminaryMirSharedLifecycleTarget::Array(array)]
            }
            MirSharedTarget::Obj => self
                .program
                .classes
                .iter()
                .map(|class| PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
            MirSharedTarget::Class(base) => self
                .program
                .classes
                .iter()
                .filter(|class| class.id == base || self.program.is_ancestor(base, class.id))
                .map(|class| PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
            MirSharedTarget::Interface(interface) => self
                .program
                .classes
                .iter()
                .filter(|class| self.program.conformance(class.id, interface).is_some())
                .map(|class| PreliminaryMirSharedLifecycleTarget::Class(class.id))
                .collect(),
        }
    }

    /// Converts only a lifecycle-free product into backend-consumable MIR.
    pub fn try_into_final(self) -> Result<MirProgram, Box<Self>> {
        if !self.has_static_initializers() && self.static_initializers.is_empty() {
            Ok(self.program)
        } else {
            Err(Box::new(self))
        }
    }

    pub(crate) const fn program(&self) -> &MirProgram {
        &self.program
    }

    #[cfg(test)]
    pub(crate) fn static_initializers_mut_for_test(
        &mut self,
    ) -> &mut Vec<PreliminaryMirStaticInitializer> {
        &mut self.static_initializers
    }

    #[cfg(test)]
    pub(crate) fn static_fields_mut_for_test(&mut self) -> &mut Vec<PreliminaryMirStaticField> {
        &mut self.static_fields
    }
}

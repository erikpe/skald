//! Parent-bound declaration extension; selected publication is a separate check.
use super::{data, DataDefinition, ProgramError, VerifiedProgram};
use crate::backend::plan::{
    ArtifactCategory, ArtifactDeclaration, ArtifactId, DataKey, LirCallableId, PlanError,
};
use std::collections::BTreeMap;

#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TargetDeclarations<'a, 'p> {
    parent: &'a VerifiedProgram<'p>,
    declarations: BTreeMap<ArtifactId, ArtifactDeclaration>,
    data: BTreeMap<DataKey, DataDefinition>,
}
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct TargetExtension<'a, 'p> {
    parent: &'a VerifiedProgram<'p>,
    declarations: BTreeMap<ArtifactId, ArtifactDeclaration>,
    data: BTreeMap<DataKey, DataDefinition>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a, 'p> TargetDeclarations<'a, 'p> {
    pub(in crate::backend) fn new(parent: &'a VerifiedProgram<'p>) -> Self {
        Self {
            parent,
            declarations: BTreeMap::new(),
            data: BTreeMap::new(),
        }
    }
    /// IDs refer exclusively to the immutable parent pools. New facts require replanning.
    pub(in crate::backend) fn declare(
        &mut self,
        declaration: ArtifactDeclaration,
    ) -> Result<(), ProgramError> {
        let view = self.parent.parent();
        if view.artifact_id(declaration.key).is_ok()
            || self.declarations.contains_key(&declaration.key)
        {
            return Err(ProgramError::DuplicateDefinition);
        }
        match declaration.key {
            ArtifactId::Callable(LirCallableId::TargetThunk(_)) if declaration.layout.is_none() => {
                view.signature(
                    view.signature_id(
                        declaration
                            .signature
                            .ok_or(PlanError::InvalidSignature)?
                            .index(),
                    )?,
                )?;
            }
            ArtifactId::Data(
                DataKey::Table(_) | DataKey::Literal(_) | DataKey::FailureMessage(_),
            ) if declaration.signature.is_none() => {
                let layout = view.layout(
                    view.layout_id(declaration.layout.ok_or(PlanError::InvalidLayout)?.index())?,
                )?;
                if layout.disposition != crate::backend::plan::LayoutDisposition::Addressable {
                    return Err(PlanError::InvalidLayout.into());
                }
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        }
        self.declarations.insert(declaration.key, declaration);
        Ok(())
    }
    pub(in crate::backend) fn define_data(
        &mut self,
        definition: DataDefinition,
    ) -> Result<(), ProgramError> {
        if !self
            .declarations
            .contains_key(&ArtifactId::Data(definition.key))
        {
            return Err(PlanError::UnknownDeclaration.into());
        }
        if self.data.contains_key(&definition.key) {
            return Err(ProgramError::DuplicateDefinition);
        }
        data::check(&definition, self.parent.parent(), |key, category| {
            lookup(self.parent, &self.declarations, key, category)
        })?;
        self.data.insert(definition.key, definition);
        Ok(())
    }
    pub(in crate::backend) fn freeze(self) -> Result<TargetExtension<'a, 'p>, ProgramError> {
        for key in self.declarations.keys() {
            if let ArtifactId::Data(key) = key {
                if !self.data.contains_key(key) {
                    return Err(ProgramError::MissingDefinition(ArtifactId::Data(*key)));
                }
            }
        }
        Ok(TargetExtension {
            parent: self.parent,
            declarations: self.declarations,
            data: self.data,
        })
    }
}
#[cfg_attr(not(test), allow(dead_code))]
fn lookup(
    parent: &VerifiedProgram<'_>,
    declarations: &BTreeMap<ArtifactId, ArtifactDeclaration>,
    key: ArtifactId,
    category: ArtifactCategory,
) -> Result<Option<usize>, ProgramError> {
    if key.category() != category {
        return Err(PlanError::ArtifactCategoryMismatch.into());
    }
    if let Some(declaration) = declarations.get(&key) {
        return declaration
            .layout
            .map(|layout| {
                parent
                    .parent()
                    .layout(parent.parent().layout_id(layout.index())?)
                    .map(|l| l.size)
            })
            .transpose()
            .map_err(ProgramError::from);
    }
    data::parent_artifact(parent.parent(), key, category)
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'a, 'p> TargetExtension<'a, 'p> {
    pub(in crate::backend) fn parent(&self) -> &'a VerifiedProgram<'p> {
        self.parent
    }
    pub(in crate::backend) fn selection_binding(
        &self,
        key: LirCallableId,
    ) -> Result<crate::backend::plan::CallableBinding<'p>, ProgramError> {
        if matches!(key, LirCallableId::TargetThunk(_)) {
            let declaration = self
                .declarations
                .get(&ArtifactId::Callable(key))
                .ok_or(PlanError::UnknownDeclaration)?;
            Ok(self.parent.parent().thunk_binding(
                key,
                declaration.signature.ok_or(PlanError::InvalidSignature)?,
            )?)
        } else {
            Ok(self.parent.parent().callable(key)?)
        }
    }

    pub(in crate::backend) fn require_parent(
        &self,
        parent: &VerifiedProgram<'p>,
    ) -> Result<(), ProgramError> {
        self.parent.require_same_snapshot(parent)
    }
    pub(in crate::backend) fn artifact(
        &self,
        key: ArtifactId,
        category: ArtifactCategory,
    ) -> Result<Option<usize>, ProgramError> {
        lookup(self.parent, &self.declarations, key, category)
    }
    pub(in crate::backend) fn declarations(
        &self,
    ) -> impl ExactSizeIterator<Item = &ArtifactDeclaration> {
        self.declarations.values()
    }
    pub(in crate::backend) fn data(&self) -> impl ExactSizeIterator<Item = &DataDefinition> {
        self.data.values()
    }
}

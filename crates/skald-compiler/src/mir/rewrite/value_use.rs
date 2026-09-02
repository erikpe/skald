//! Exhaustive semantic use sites for one callable-local transient value.

use std::convert::Infallible;

use crate::{
    identity::CallableId,
    mir::{MirDefinitionRef, ValueId},
};

use super::{
    census::{value_use_census_for_definition, MirValueCensusEntry},
    edit::MirCallableEdit,
    map::observe_body_local_identities,
    MirLocalIdentity, MirLocalIdentityObserver, MirLocalIdentitySite, MirRewriteError,
};

/// Operand position within an ordinary scalar rvalue.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirScalarValueUse {
    UnaryOperand,
    BinaryLeft,
    BinaryRight,
    ComparisonLeft,
    ComparisonRight,
}

/// Value-bearing position within an ordinary call.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirCallValueUse {
    Target,
    Receiver,
    Argument(usize),
}

/// Closed semantic classification of a transient value use.
///
/// New value-bearing MIR fields must select one role in the exhaustive local-
/// identity traversal. Unknown and protocol-bearing roles are deliberately
/// retained as forwarding barriers rather than inheriting permissive behavior.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum MirValueUseRole {
    OrdinaryScalarRvalue(MirScalarValueUse),
    OrdinaryPrimitiveCast,
    OrdinaryStore,
    OrdinaryCall(MirCallValueUse),
    OrdinaryReturn,
    OrdinaryBranch,
    CheckedProtocol,
    ProofMetadata,
    OwnershipOrLifecycle,
    InputOutput,
    Unknown,
}

impl MirValueUseRole {
    /// Whether substituting another same-typed, dominating scalar value is
    /// permitted at this semantic role.
    pub(crate) const fn is_forwarding_safe(self) -> bool {
        match self {
            Self::OrdinaryScalarRvalue(_)
            | Self::OrdinaryPrimitiveCast
            | Self::OrdinaryStore
            | Self::OrdinaryCall(_)
            | Self::OrdinaryReturn
            | Self::OrdinaryBranch => true,
            Self::CheckedProtocol
            | Self::ProofMetadata
            | Self::OwnershipOrLifecycle
            | Self::InputOutput
            | Self::Unknown => false,
        }
    }
}

/// One deterministic semantic occurrence of a selected value.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct MirValueUseSite {
    site: MirLocalIdentitySite,
    role: MirValueUseRole,
}

impl MirValueUseSite {
    pub(crate) const fn site(self) -> MirLocalIdentitySite {
        self.site
    }

    pub(crate) const fn role(self) -> MirValueUseRole {
        self.role
    }
}

/// Definition and semantic uses of one live value in one callable snapshot.
///
/// Like the compact value census, this owned observation is invalid after any
/// rewrite. Recompute it before making another position-based decision.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MirValueUseSites {
    callable: CallableId,
    value: ValueId,
    definition: MirLocalIdentitySite,
    uses: Vec<MirValueUseSite>,
}

impl MirValueUseSites {
    pub(crate) const fn callable(&self) -> CallableId {
        self.callable
    }

    pub(crate) const fn value(&self) -> ValueId {
        self.value
    }

    pub(crate) const fn definition(&self) -> MirLocalIdentitySite {
        self.definition
    }

    pub(crate) fn uses(&self) -> &[MirValueUseSite] {
        &self.uses
    }

    pub(crate) fn all_uses_follow_definition_in_same_block(&self) -> bool {
        let MirLocalIdentitySite::Instruction {
            block: definition_block,
            instruction: definition_instruction,
        } = self.definition
        else {
            return false;
        };

        self.uses.iter().all(|use_site| match use_site.site {
            MirLocalIdentitySite::Instruction { block, instruction } => {
                block == definition_block && instruction > definition_instruction
            }
            MirLocalIdentitySite::Terminator(block) => block == definition_block,
            MirLocalIdentitySite::ReturnStorage
            | MirLocalIdentitySite::Receiver
            | MirLocalIdentitySite::Parameter(_)
            | MirLocalIdentitySite::StaticPublicationInitializationExit
            | MirLocalIdentitySite::StaticPublicationCleanupEntry
            | MirLocalIdentitySite::StorageDeclaration(_)
            | MirLocalIdentitySite::ValueDeclaration(_)
            | MirLocalIdentitySite::BodyEntry
            | MirLocalIdentitySite::BlockDeclaration(_)
            | MirLocalIdentitySite::PathCondition(_)
            | MirLocalIdentitySite::LogicalExpression(_) => false,
        })
    }

    pub(crate) fn is_forwarding_safe(&self) -> bool {
        self.all_uses_follow_definition_in_same_block()
            && self.uses.iter().all(|site| site.role.is_forwarding_safe())
    }
}

impl MirCallableEdit {
    /// Enumerates semantic use sites from the current sparse edit snapshot.
    pub(crate) fn value_use_sites(
        &self,
        value: ValueId,
    ) -> Result<MirValueUseSites, MirRewriteError> {
        self.value(value)?;
        let census = self.value_use_census()?;
        let definition = required_definition(census.get(value), value)?;
        let mut collector = SelectedValueUseCollector::new(value);
        infallible(self.observe_live_references(&mut collector));
        build_result(value, definition, collector.uses)
    }
}

/// Enumerates semantic use sites from one dense, read-only definition.
pub(crate) fn value_use_sites_for_definition(
    definition: MirDefinitionRef<'_>,
    value: ValueId,
) -> Result<MirValueUseSites, MirRewriteError> {
    if value.callable() != definition.callable() {
        return Err(MirRewriteError::ForeignIdentity {
            expected: definition.callable(),
            identity: MirLocalIdentity::Value(value),
        });
    }
    if definition.values().get(value.index()).is_none() {
        return Err(MirRewriteError::UnknownIdentity {
            identity: MirLocalIdentity::Value(value),
        });
    }
    let census = value_use_census_for_definition(definition)?;
    let definition_site = required_definition(census.get(value), value)?;
    let mut collector = SelectedValueUseCollector::new(value);
    infallible(observe_body_local_identities(
        definition.body(),
        &mut collector,
    ));
    build_result(value, definition_site, collector.uses)
}

fn required_definition(
    entry: Option<&MirValueCensusEntry>,
    value: ValueId,
) -> Result<MirLocalIdentitySite, MirRewriteError> {
    let Some(entry) = entry else {
        return Err(MirRewriteError::UnknownIdentity {
            identity: MirLocalIdentity::Value(value),
        });
    };
    entry
        .definition()
        .ok_or(MirRewriteError::MissingValueDefinition { value })
}

fn build_result(
    value: ValueId,
    definition: MirLocalIdentitySite,
    uses: Vec<MirValueUseSite>,
) -> Result<MirValueUseSites, MirRewriteError> {
    if !matches!(definition, MirLocalIdentitySite::Instruction { .. }) {
        return Err(MirRewriteError::InvalidValueDefinitionSite {
            value,
            site: definition,
        });
    }
    Ok(MirValueUseSites {
        callable: value.callable(),
        value,
        definition,
        uses,
    })
}

struct SelectedValueUseCollector {
    selected: ValueId,
    uses: Vec<MirValueUseSite>,
}

impl SelectedValueUseCollector {
    const fn new(selected: ValueId) -> Self {
        Self {
            selected,
            uses: Vec::new(),
        }
    }
}

impl MirLocalIdentityObserver for SelectedValueUseCollector {
    type Error = Infallible;

    fn observe_value_use(
        &mut self,
        site: MirLocalIdentitySite,
        role: MirValueUseRole,
        value: ValueId,
    ) -> Result<(), Self::Error> {
        if value == self.selected {
            self.uses.push(MirValueUseSite { site, role });
        }
        Ok(())
    }
}

fn infallible(result: Result<(), Infallible>) {
    match result {
        Ok(()) => {}
        Err(never) => match never {},
    }
}

#[cfg(test)]
#[path = "value_use/tests.rs"]
mod tests;

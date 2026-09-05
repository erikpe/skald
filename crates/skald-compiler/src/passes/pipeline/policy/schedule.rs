use super::{
    descriptor::MirPassRegistration,
    error::MirPassScheduleError,
    identity::MirPassIdentity,
    profile::MirOptimizationProfile,
    registry::{MirPassRegistry, NORMALIZATION_NAME},
    stage::MirPassStage,
};
use crate::passes::pipeline::execution::{
    MirFinalPassTransform, MirProofPassTransform, MirProofTransitionTransform,
};

/// One position in an explicitly resolved pass schedule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct MirPassOccurrence {
    position: usize,
    registration: MirPassRegistration,
    occurrence: usize,
}

impl MirPassOccurrence {
    pub(crate) const fn position(self) -> usize {
        self.position
    }

    pub(crate) const fn identity(self) -> MirPassIdentity {
        self.registration.descriptor().identity()
    }

    pub(crate) const fn occurrence(self) -> usize {
        self.occurrence
    }

    pub(crate) const fn stage(self) -> MirPassStage {
        self.registration.descriptor().stage()
    }

    pub(in crate::passes::pipeline) const fn name(self) -> &'static str {
        self.registration.descriptor().name()
    }

    pub(in crate::passes::pipeline) const fn proof_transform(
        self,
    ) -> Option<MirProofPassTransform> {
        self.registration.implementation().proof_transform()
    }

    pub(in crate::passes::pipeline) const fn final_transform(
        self,
    ) -> Option<MirFinalPassTransform> {
        self.registration.implementation().final_transform()
    }

    pub(in crate::passes::pipeline) const fn transition_transform(
        self,
    ) -> Option<MirProofTransitionTransform> {
        self.registration.implementation().transition_transform()
    }
}

/// Immutable ordered target-independent pass schedule for one request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPassSchedule {
    occurrences: Vec<MirPassOccurrence>,
    proof_rich_end: usize,
    transition_position: Option<usize>,
    final_start: usize,
}

impl MirPassSchedule {
    #[cfg(test)]
    pub(crate) fn as_slice(&self) -> &[MirPassOccurrence] {
        &self.occurrences
    }

    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = MirPassOccurrence> + '_ {
        self.occurrences.iter().copied()
    }

    pub(crate) fn proof_rich(&self) -> impl Iterator<Item = MirPassOccurrence> + '_ {
        self.occurrences[..self.proof_rich_end].iter().copied()
    }

    pub(crate) fn proof_transition(&self) -> Option<MirPassOccurrence> {
        self.transition_position
            .map(|position| self.occurrences[position])
    }

    pub(crate) fn final_stage(&self) -> impl Iterator<Item = MirPassOccurrence> + '_ {
        self.occurrences[self.final_start..].iter().copied()
    }

    #[cfg(test)]
    pub(crate) const fn normalization_position(&self) -> usize {
        match self.transition_position {
            Some(position) => position,
            None => self.final_start,
        }
    }

    #[cfg(test)]
    pub(crate) fn is_empty(&self) -> bool {
        self.occurrences.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.occurrences.len()
    }
}

pub(super) fn resolve_profile<'a>(
    registry: MirPassRegistry,
    profile: MirOptimizationProfile,
    disabled_names: impl IntoIterator<Item = &'a str>,
) -> Result<MirPassSchedule, MirPassScheduleError> {
    resolve_identities(registry, profile.identities(), disabled_names)
}

#[allow(dead_code)]
pub(super) fn resolve_exact(
    registry: MirPassRegistry,
    identities: &[MirPassIdentity],
) -> Result<MirPassSchedule, MirPassScheduleError> {
    resolve_identities(registry, identities, std::iter::empty())
}

pub(super) fn resolve_identities<'a>(
    registry: MirPassRegistry,
    identities: &[MirPassIdentity],
    disabled_names: impl IntoIterator<Item = &'a str>,
) -> Result<MirPassSchedule, MirPassScheduleError> {
    registry.validate()?;

    let mut disabled_identities = Vec::new();
    let mut unknown_names = Vec::new();
    for name in disabled_names {
        if name == NORMALIZATION_NAME {
            return Err(MirPassScheduleError::MandatoryNormalizationSelection);
        }
        match registry.identity_for_name(name) {
            Some(identity) if !disabled_identities.contains(&identity) => {
                disabled_identities.push(identity);
            }
            Some(_) => {}
            None => unknown_names.push(name.to_owned()),
        }
    }
    if !unknown_names.is_empty() {
        unknown_names.sort_unstable();
        unknown_names.dedup();
        return Err(MirPassScheduleError::UnknownNames {
            names: unknown_names,
            known_names: registry.known_names(),
        });
    }

    for identity in identities {
        if !registry.contains_identity(*identity) {
            return Err(MirPassScheduleError::UnknownIdentity {
                identity: *identity,
            });
        }
    }

    let retained = identities
        .iter()
        .copied()
        .filter(|identity| !disabled_identities.contains(identity))
        .collect::<Vec<_>>();
    validate_stage_order(registry, &retained)?;
    Ok(number_occurrences(registry, retained))
}

fn validate_stage_order(
    registry: MirPassRegistry,
    identities: &[MirPassIdentity],
) -> Result<(), MirPassScheduleError> {
    let mut transition_position = None;
    let mut reached_final = false;
    for (position, identity) in identities.iter().copied().enumerate() {
        let registration = registry
            .registration(identity)
            .expect("validated schedule identity must have one registration");
        match registration.descriptor().stage() {
            MirPassStage::ProofRich if transition_position.is_some() || reached_final => {
                return Err(MirPassScheduleError::WrongStageOrder {
                    proof_rich: identity,
                    position,
                });
            }
            MirPassStage::ProofRich => {}
            MirPassStage::ProofTransition if reached_final => {
                return Err(MirPassScheduleError::ProofTransitionAfterFinal {
                    transition: identity,
                    position,
                });
            }
            MirPassStage::ProofTransition => {
                if let Some(first_position) = transition_position {
                    return Err(MirPassScheduleError::RepeatedProofTransition {
                        transition: identity,
                        first_position,
                        position,
                    });
                }
                transition_position = Some(position);
            }
            MirPassStage::Final => reached_final = true,
        }
    }
    Ok(())
}

fn number_occurrences(
    registry: MirPassRegistry,
    identities: impl IntoIterator<Item = MirPassIdentity>,
) -> MirPassSchedule {
    let mut counts = Vec::<(MirPassIdentity, usize)>::new();
    let mut occurrences = Vec::new();

    for (position, identity) in identities.into_iter().enumerate() {
        let registration = registry
            .registration(identity)
            .expect("validated schedule identity must have one registration");
        let occurrence = match counts
            .iter_mut()
            .find(|(known_identity, _)| *known_identity == identity)
        {
            Some((_, count)) => {
                let occurrence = *count;
                *count = count.saturating_add(1);
                occurrence
            }
            None => {
                counts.push((identity, 1));
                0
            }
        };
        occurrences.push(MirPassOccurrence {
            position,
            registration,
            occurrence,
        });
    }

    let proof_rich_end = occurrences
        .iter()
        .position(|occurrence| occurrence.stage() != MirPassStage::ProofRich)
        .unwrap_or(occurrences.len());
    let transition_position = occurrences
        .iter()
        .position(|occurrence| occurrence.stage() == MirPassStage::ProofTransition);
    let final_start = occurrences
        .iter()
        .position(|occurrence| occurrence.stage() == MirPassStage::Final)
        .unwrap_or(occurrences.len());
    MirPassSchedule {
        occurrences,
        proof_rich_end,
        transition_position,
        final_start,
    }
}

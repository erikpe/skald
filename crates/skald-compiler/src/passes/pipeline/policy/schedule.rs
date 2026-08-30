use super::{
    descriptor::MirPassRegistration, error::MirPassScheduleError, identity::MirPassIdentity,
    profile::MirOptimizationProfile, registry::MirPassRegistry,
};
use crate::passes::pipeline::execution::MirPassTransform;

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

    pub(in crate::passes::pipeline) const fn name(self) -> &'static str {
        self.registration.descriptor().name()
    }

    pub(in crate::passes::pipeline) const fn transform(self) -> MirPassTransform {
        self.registration.implementation().transform()
    }
}

/// Immutable ordered target-independent pass schedule for one request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct MirPassSchedule {
    occurrences: Vec<MirPassOccurrence>,
}

impl MirPassSchedule {
    #[cfg(test)]
    pub(crate) fn as_slice(&self) -> &[MirPassOccurrence] {
        &self.occurrences
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = MirPassOccurrence> + '_ {
        self.occurrences.iter().copied()
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
        .filter(|identity| !disabled_identities.contains(identity));
    Ok(number_occurrences(registry, retained))
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

    MirPassSchedule { occurrences }
}

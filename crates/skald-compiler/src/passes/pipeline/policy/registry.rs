use super::{
    descriptor::MirPassRegistration,
    error::{MirPassRegistryError, MirPassRegistryErrors},
    identity::MirPassIdentity,
};
use crate::passes::pipeline::optimizations::dead_pure_definition_elimination;

/// Immutable view of the compiler-owned final-MIR pass registry.
#[derive(Clone, Copy)]
pub(in crate::passes::pipeline) struct MirPassRegistry {
    registrations: &'static [MirPassRegistration],
}

impl MirPassRegistry {
    pub(in crate::passes::pipeline) const fn new(
        registrations: &'static [MirPassRegistration],
    ) -> Self {
        Self { registrations }
    }

    pub(super) fn validate(self) -> Result<(), MirPassRegistryErrors> {
        let mut errors = Vec::new();

        for (index, registration) in self.registrations.iter().copied().enumerate() {
            let descriptor = registration.descriptor();
            if !is_stable_pass_name(descriptor.name()) {
                errors.push(MirPassRegistryError::InvalidName {
                    name: descriptor.name(),
                });
            }
            if descriptor.description().trim().is_empty() {
                errors.push(MirPassRegistryError::EmptyDescription {
                    identity: descriptor.identity(),
                });
            }
            if descriptor.identity() != registration.implementation().identity() {
                errors.push(MirPassRegistryError::ImplementationIdentityMismatch {
                    descriptor: descriptor.identity(),
                    implementation: registration.implementation().identity(),
                });
            }

            for earlier in self.registrations[..index].iter().copied() {
                if earlier.descriptor().identity() == descriptor.identity() {
                    errors.push(MirPassRegistryError::DuplicateIdentity {
                        identity: descriptor.identity(),
                    });
                    break;
                }
            }
            for earlier in self.registrations[..index].iter().copied() {
                if earlier.descriptor().name() == descriptor.name() {
                    errors.push(MirPassRegistryError::DuplicateName {
                        name: descriptor.name(),
                    });
                    break;
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(MirPassRegistryErrors::new(errors))
        }
    }

    pub(super) fn contains_identity(self, identity: MirPassIdentity) -> bool {
        self.registrations
            .iter()
            .any(|registration| registration.descriptor().identity() == identity)
    }

    pub(super) fn registration(self, identity: MirPassIdentity) -> Option<MirPassRegistration> {
        self.registrations
            .iter()
            .copied()
            .find(|registration| registration.descriptor().identity() == identity)
    }

    pub(super) fn identity_for_name(self, name: &str) -> Option<MirPassIdentity> {
        self.registrations
            .iter()
            .find(|registration| registration.descriptor().name() == name)
            .map(|registration| registration.descriptor().identity())
    }

    pub(super) fn known_names(self) -> Vec<&'static str> {
        let mut names = self
            .registrations
            .iter()
            .map(|registration| registration.descriptor().name())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names
    }
}

fn is_stable_pass_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    if !matches!(bytes.next(), Some(b'a'..=b'z')) {
        return false;
    }

    let mut previous_was_separator = false;
    for byte in bytes {
        match byte {
            b'a'..=b'z' | b'0'..=b'9' => previous_was_separator = false,
            b'-' if !previous_was_separator => previous_was_separator = true,
            _ => return false,
        }
    }
    !previous_was_separator
}

static PRODUCTION_REGISTRATIONS: [MirPassRegistration; 1] =
    [dead_pure_definition_elimination::REGISTRATION];

pub(super) fn production_registry() -> MirPassRegistry {
    MirPassRegistry::new(&PRODUCTION_REGISTRATIONS)
}

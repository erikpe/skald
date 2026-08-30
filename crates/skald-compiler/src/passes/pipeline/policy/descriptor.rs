use super::identity::MirPassIdentity;

/// Stable selection and inspection metadata for one pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MirPassDescriptor {
    identity: MirPassIdentity,
    name: &'static str,
    description: &'static str,
}

impl MirPassDescriptor {
    pub(super) const fn new(
        identity: MirPassIdentity,
        name: &'static str,
        description: &'static str,
    ) -> Self {
        Self {
            identity,
            name,
            description,
        }
    }

    pub(super) const fn identity(self) -> MirPassIdentity {
        self.identity
    }

    pub(super) const fn name(self) -> &'static str {
        self.name
    }

    pub(super) const fn description(self) -> &'static str {
        self.description
    }
}

/// Identity declared by a pass implementation.
///
/// The verified runner will add the transformation entry point to this owner.
/// Keeping its identity separate now lets registry validation reject metadata
/// wired to the wrong implementation before execution exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MirPassImplementation {
    identity: MirPassIdentity,
}

impl MirPassImplementation {
    pub(super) const fn new(identity: MirPassIdentity) -> Self {
        Self { identity }
    }

    pub(super) const fn identity(self) -> MirPassIdentity {
        self.identity
    }
}

/// One immutable compiler-owned registry entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct MirPassRegistration {
    descriptor: MirPassDescriptor,
    implementation: MirPassImplementation,
}

impl MirPassRegistration {
    pub(super) const fn new(
        descriptor: MirPassDescriptor,
        implementation: MirPassImplementation,
    ) -> Self {
        Self {
            descriptor,
            implementation,
        }
    }

    pub(super) const fn descriptor(self) -> MirPassDescriptor {
        self.descriptor
    }

    pub(super) const fn implementation(self) -> MirPassImplementation {
        self.implementation
    }
}

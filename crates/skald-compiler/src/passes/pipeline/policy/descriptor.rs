use super::identity::MirPassIdentity;
use crate::passes::pipeline::execution::MirPassTransform;

/// Stable selection and inspection metadata for one pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirPassDescriptor {
    identity: MirPassIdentity,
    name: &'static str,
    description: &'static str,
}

impl MirPassDescriptor {
    pub(in crate::passes::pipeline) const fn new(
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

    pub(in crate::passes::pipeline) const fn identity(self) -> MirPassIdentity {
        self.identity
    }

    pub(in crate::passes::pipeline) const fn name(self) -> &'static str {
        self.name
    }

    pub(in crate::passes::pipeline) const fn description(self) -> &'static str {
        self.description
    }
}

/// Identity and transformation entry point declared by a pass implementation.
#[derive(Clone, Copy, Debug)]
pub(in crate::passes::pipeline) struct MirPassImplementation {
    identity: MirPassIdentity,
    transform: MirPassTransform,
}

impl MirPassImplementation {
    pub(in crate::passes::pipeline) const fn new(
        identity: MirPassIdentity,
        transform: MirPassTransform,
    ) -> Self {
        Self {
            identity,
            transform,
        }
    }

    pub(in crate::passes::pipeline) const fn identity(self) -> MirPassIdentity {
        self.identity
    }

    pub(in crate::passes::pipeline) const fn transform(self) -> MirPassTransform {
        self.transform
    }
}

impl PartialEq for MirPassImplementation {
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl Eq for MirPassImplementation {}

/// One immutable compiler-owned registry entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline) struct MirPassRegistration {
    descriptor: MirPassDescriptor,
    implementation: MirPassImplementation,
}

impl MirPassRegistration {
    pub(in crate::passes::pipeline) const fn new(
        descriptor: MirPassDescriptor,
        implementation: MirPassImplementation,
    ) -> Self {
        Self {
            descriptor,
            implementation,
        }
    }

    pub(in crate::passes::pipeline) const fn descriptor(self) -> MirPassDescriptor {
        self.descriptor
    }

    pub(in crate::passes::pipeline) const fn implementation(self) -> MirPassImplementation {
        self.implementation
    }
}

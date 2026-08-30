//! Uniform borrowed access to executable MIR definitions.

use crate::mir::{MirDefinitionRef, MirProgram, MirStaticInitializerBody, PreliminaryMirProgram};

/// A read-only view over ordinary, member, and static-initializer definitions.
///
/// The view borrows the original MIR containers and never snapshots bodies.
/// Iteration is always functions, members, then static initializers in their
/// existing stable identity order.
#[derive(Clone, Copy)]
pub(crate) struct MirExecutableDefinitionView<'mir> {
    program: &'mir MirProgram,
    initializers: &'mir [MirStaticInitializerBody],
}

impl<'mir> MirExecutableDefinitionView<'mir> {
    pub(crate) fn preliminary(program: &'mir PreliminaryMirProgram) -> Self {
        Self::from_parts(program.program(), program.static_initializer_bodies())
    }

    pub(crate) fn final_program(program: &'mir MirProgram) -> Self {
        Self::from_parts(
            program,
            program
                .static_lifecycle
                .as_ref()
                .map_or(&[], |coordinator| coordinator.initializers()),
        )
    }

    pub(crate) const fn from_parts(
        program: &'mir MirProgram,
        initializers: &'mir [MirStaticInitializerBody],
    ) -> Self {
        Self {
            program,
            initializers,
        }
    }

    pub(crate) const fn program(self) -> &'mir MirProgram {
        self.program
    }

    pub(crate) fn iter(self) -> impl Iterator<Item = MirDefinitionRef<'mir>> {
        self.program
            .definitions
            .iter()
            .map(MirDefinitionRef::Function)
            .chain(
                self.program
                    .member_definitions
                    .iter()
                    .map(MirDefinitionRef::Member),
            )
            .chain(self.initializers.iter().map(MirDefinitionRef::from))
    }
}

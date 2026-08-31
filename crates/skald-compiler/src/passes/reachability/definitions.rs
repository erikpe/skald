//! Uniform borrowed access to executable MIR definitions.

use crate::{
    identity::CallableId,
    mir::{
        MirDefinitionRef, MirFunctionLinkage, MirProgram, MirStaticFieldInitialization,
        MirStaticInitializerBody, PreliminaryMirProgram,
    },
};

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

/// Iterates semantic callable declarations that may independently retain a
/// body in final MIR. The declaration domain remains complete even when the
/// definition containers are sparse.
pub(super) fn declared_executable_callables(
    program: &MirProgram,
) -> impl Iterator<Item = CallableId> + '_ {
    let functions = program
        .declarations
        .iter()
        .filter(|declaration| declaration.linkage == MirFunctionLinkage::Internal)
        .map(|declaration| CallableId::Function(declaration.id));
    let members = program.classes.iter().flat_map(|class| {
        class
            .initializers
            .iter()
            .map(|declaration| CallableId::Initializer(declaration.id))
            .chain(
                class
                    .copy_constructor_declaration
                    .iter()
                    .map(|declaration| CallableId::CopyConstructor(declaration.id)),
            )
            .chain(
                class
                    .copy_assignment_declaration
                    .iter()
                    .map(|declaration| CallableId::CopyAssignment(declaration.id)),
            )
            .chain(
                class
                    .destruction
                    .destructor
                    .iter()
                    .map(|declaration| CallableId::Destructor(declaration.id)),
            )
            .chain(
                class
                    .methods
                    .iter()
                    .map(|declaration| CallableId::Method(declaration.id)),
            )
    });
    let static_initializers = program.static_lifecycle.iter().flat_map(|coordinator| {
        coordinator
            .lifecycle()
            .definitions()
            .iter()
            .filter_map(|definition| match definition.initialization {
                MirStaticFieldInitialization::Explicit(initializer) => {
                    Some(CallableId::StaticInitializer(initializer))
                }
                MirStaticFieldInitialization::ZeroDefault => None,
            })
    });

    functions.chain(members).chain(static_initializers)
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

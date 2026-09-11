//! Ordered selection between validated specialization candidates and ordinary products.

use super::*;

pub(in crate::resolve::resolver::program) struct OrdinaryProgramProducts {
    classes: ResolvedClassDeclarationTable,
    interfaces: ResolvedInterfaceDeclarationTable,
    hierarchy: ResolvedClassHierarchy,
}

impl OrdinaryProgramProducts {
    pub(in crate::resolve::resolver::program) fn new(
        classes: ResolvedClassDeclarationTable,
        interfaces: ResolvedInterfaceDeclarationTable,
        hierarchy: ResolvedClassHierarchy,
    ) -> Self {
        Self {
            classes,
            interfaces,
            hierarchy,
        }
    }
}

pub(in crate::resolve::resolver::program) struct CandidateProgram {
    program: ResolvedProgram,
}

impl CandidateProgram {
    pub(in crate::resolve::resolver::program) fn new(program: ResolvedProgram) -> Self {
        Self { program }
    }

    pub(in crate::resolve::resolver::program) fn validate_and_publish(
        mut self,
        diagnostics: &mut Diagnostics,
        ordinary: OrdinaryProgramProducts,
    ) -> ResolvedProgram {
        if !super::validation::validate_specialization_requirements(&self.program, diagnostics) {
            self.reject_class_candidates(&ordinary);
        }
        if !super::interface_validation::validate_interface_specializations(
            &self.program,
            diagnostics,
        ) {
            self.reject_interface_candidates(&ordinary);
        }
        self.program
    }

    fn reject_class_candidates(&mut self, ordinary: &OrdinaryProgramProducts) {
        let generated = self
            .program
            .generic_specializations
            .iter()
            .filter_map(GenericSpecialization::class)
            .collect::<Vec<_>>();
        for class in generated {
            self.program.generic_specializations.fail_class(class);
        }
        self.program.classes = ordinary.classes.clone();
        self.program.hierarchy = ordinary.hierarchy.clone();
        self.program.class_definitions = ResolvedClassDefinitionTable::default();
        self.program.definitions = ResolvedFunctionDefinitionTable::default();
        self.program.virtual_families = ResolvedVirtualFamilyTable::default();
    }

    fn reject_interface_candidates(&mut self, ordinary: &OrdinaryProgramProducts) {
        self.program.generic_interface_specializations.fail_all();
        self.program.interfaces = ordinary.interfaces.clone();
    }
}

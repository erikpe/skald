//! Canonical source-order discovery of explicit closed applications.
//!
//! This facade owns discovery inputs and coordinates syntax closing with the
//! source-order AST scanner. Each traversal remains private to this module.

mod source_request_scanner;
mod syntax_type_closer;

use source_request_scanner::SourceRequestScanner;
use syntax_type_closer::SyntaxTypeCloser;

use super::super::resolver::{ModuleUnit, ProgramLookupTables};
use super::*;

pub(crate) struct SpecializationDiscoveryInput<'program, 'ast> {
    units: &'program [ModuleUnit<'ast>],
    modules: &'program crate::module::ProgramModuleTable,
    lookups: ProgramLookupTables<'program>,
    templates: GenericTemplateDiscoveryInput<'program>,
    ordinary_class_count: usize,
    ordinary_interface_count: usize,
}

pub(crate) struct GenericTemplateDiscoveryInput<'program> {
    class_semantics: &'program ResolvedClassTemplateSemanticTable,
    interface_semantics: &'program ResolvedInterfaceTemplateSemanticTable,
    classes: &'program ResolvedClassTemplateTable,
    interfaces: &'program ResolvedInterfaceTemplateTable,
}

impl<'program> GenericTemplateDiscoveryInput<'program> {
    pub(crate) const fn new(
        class_semantics: &'program ResolvedClassTemplateSemanticTable,
        interface_semantics: &'program ResolvedInterfaceTemplateSemanticTable,
        classes: &'program ResolvedClassTemplateTable,
        interfaces: &'program ResolvedInterfaceTemplateTable,
    ) -> Self {
        Self {
            class_semantics,
            interface_semantics,
            classes,
            interfaces,
        }
    }
}

impl<'program, 'ast> SpecializationDiscoveryInput<'program, 'ast> {
    pub(crate) fn new(
        units: &'program [ModuleUnit<'ast>],
        modules: &'program crate::module::ProgramModuleTable,
        lookups: ProgramLookupTables<'program>,
        templates: GenericTemplateDiscoveryInput<'program>,
        ordinary_class_count: usize,
        ordinary_interface_count: usize,
    ) -> Self {
        Self {
            units,
            modules,
            lookups,
            templates,
            ordinary_class_count,
            ordinary_interface_count,
        }
    }
}

pub(crate) struct GenericApplicationDiscovery {
    pub(crate) class_specializations: GenericSpecializationTable,
    pub(crate) interface_specializations: GenericInterfaceSpecializationTable,
}

pub(crate) fn discover_specializations(
    input: SpecializationDiscoveryInput<'_, '_>,
    range_language_item: Option<&ResolvedRangeLanguageItem>,
    interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> GenericApplicationDiscovery {
    let mut owner = SpecializationCoordinator::new(
        input.templates.class_semantics,
        input.templates.interface_semantics,
        input.templates.classes,
        input.templates.interfaces,
        interner,
        diagnostics,
        input.ordinary_class_count,
        input.ordinary_interface_count,
        range_language_item.map(|item| item.range_template),
    );
    for unit in input.units {
        let lookup = input.lookups.for_unit(unit, input.modules);
        SourceRequestScanner::new(
            SyntaxTypeCloser::new(&mut owner, lookup, unit.module),
            range_language_item.map(|item| item.range_template),
        )
        .visit_unit(unit.ast);
    }
    owner.finish()
}

//! Program-level validation and typed-HIR orchestration.

use crate::{
    diagnostics::Diagnostics,
    hir::{
        HirClassDeclarationTable, HirClassDefinitionTable, HirFunctionDeclarationTable,
        HirFunctionDefinitionTable, HirInterfaceDeclarationTable, HirLiteralData,
        HirLiteralDataTable, HirProgram, HirStringLanguageItem, HirVirtualFamily,
        HirVirtualFamilyTable,
    },
    resolve::ResolvedProgram,
};

use super::{
    capabilities::CopyCapabilities, containment::validate_containment, function::CallableChecker,
};

mod class;
mod declarations;
mod function_types;
mod interfaces;
mod overrides;
mod static_fields;

use class::{check_class_definitions, lower_class_declarations};
use declarations::{
    check_entry_point, check_external_declarations, check_internal_function_parameters,
    lower_declaration,
};
use interfaces::analyze_interfaces;
use overrides::validate_override_signatures;

#[derive(Debug)]
pub struct TypeCheckOutput {
    /// Present only when the entire resolved program type-checks successfully.
    pub hir: Option<HirProgram>,
    pub diagnostics: Diagnostics,
}

impl TypeCheckOutput {
    /// Returns whether semantic checking failed and no complete HIR exists.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }

    /// Returns whether the output is valid input to executable MIR lowering.
    pub fn is_executable(&self) -> bool {
        self.hir.is_some()
    }
}

pub fn type_check(program: &ResolvedProgram) -> TypeCheckOutput {
    let mut diagnostics = Diagnostics::new();
    let optional_types_valid =
        super::optional_validation::validate_optional_types(program, &mut diagnostics);
    validate_containment(program, &mut diagnostics);
    if !optional_types_valid {
        return TypeCheckOutput {
            hir: None,
            diagnostics,
        };
    }
    super::arrays::validate_array_types(program, &mut diagnostics);
    let function_types = function_types::lower_function_types(program, &mut diagnostics);
    check_internal_function_parameters(program, &mut diagnostics);
    check_external_declarations(program, &mut diagnostics);
    let entry_function = check_entry_point(program, &mut diagnostics);
    validate_override_signatures(program, &mut diagnostics);
    let interface_analysis = analyze_interfaces(program, &mut diagnostics);
    let copy_capabilities = CopyCapabilities::compute(program);
    let optional_types = super::optional_types::lower_optional_types(program, &copy_capabilities);
    let optional_box_types = super::optional_box_types::lower_optional_box_types(program);
    let classes = lower_class_declarations(
        program,
        &copy_capabilities,
        &interface_analysis.conformances,
        &mut diagnostics,
    );
    let declarations = program.declarations.iter().map(lower_declaration).collect();
    let definitions = program
        .declarations
        .iter()
        .map(|declaration| {
            program.definitions.get(declaration.id).map(|definition| {
                CallableChecker::new(
                    program,
                    &copy_capabilities,
                    declaration,
                    definition,
                    &mut diagnostics,
                )
                .check()
            })
        })
        .collect();
    let class_definitions = check_class_definitions(program, &copy_capabilities, &mut diagnostics);

    let hir = if diagnostics.has_errors() {
        None
    } else {
        assert_closed_interface_boundary(program);
        Some(HirProgram {
            modules: program.modules.clone(),
            external_links: program.external_links.clone(),
            function_types,
            array_types: copy_capabilities.array_types(),
            optional_types,
            optional_box_types,
            string_language_item: program.string_language_item.as_ref().map(|item| {
                HirStringLanguageItem {
                    class: item.class,
                    storage_field: item.storage_field,
                    start_field: item.start_field,
                    length_field: item.length_field,
                    hash_code_field: item.hash_code_field,
                }
            }),
            literal_data: HirLiteralDataTable::new(
                program
                    .literal_data
                    .iter()
                    .map(|literal| HirLiteralData {
                        id: literal.id,
                        bytes: literal.bytes.clone(),
                        span: literal.span,
                    })
                    .collect(),
            ),
            classes: HirClassDeclarationTable::new(classes),
            interfaces: HirInterfaceDeclarationTable::new(interface_analysis.declarations),
            virtual_families: HirVirtualFamilyTable::new(
                program
                    .virtual_families
                    .iter()
                    .map(|family| HirVirtualFamily {
                        id: family.id,
                        slot: family.slot,
                        root: family.root,
                    })
                    .collect(),
            ),
            class_definitions: HirClassDefinitionTable::new(class_definitions),
            declarations: HirFunctionDeclarationTable::new(declarations),
            definitions: HirFunctionDefinitionTable::new(definitions),
            entry_function: entry_function.expect("valid program must have an entry function"),
            span: program.span,
        })
    };

    TypeCheckOutput { hir, diagnostics }
}

/// Enforces the specialization trust boundary immediately before executable
/// HIR is constructed. Ordinary resolved types already cannot represent type
/// parameters or structural interface applications; class claims are the one
/// structural declaration form that exists in the resolved program model.
fn assert_closed_interface_boundary(program: &ResolvedProgram) {
    for class in program.classes.iter() {
        for claim in &class.implemented_interfaces {
            assert!(
                claim.interface.ordinary().is_some(),
                "successful type checking cannot lower a structural interface claim for class {}",
                class.id
            );
        }
    }
}

//! Program-level validation and typed-HIR orchestration.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    hir::{
        HirClassDeclarationTable, HirClassDefinitionTable, HirFunctionDeclaration,
        HirFunctionDeclarationTable, HirFunctionDefinitionTable, HirFunctionLinkage,
        HirInterfaceDeclarationTable, HirLiteralData, HirLiteralDataTable, HirParameter,
        HirProgram, HirStringLanguageItem, HirVirtualFamily, HirVirtualFamilyTable, Type,
    },
    identity::FunctionId,
    resolve::{
        ResolvedFunctionDeclaration, ResolvedFunctionLinkage, ResolvedParameter,
        ResolvedParameterBindingMode, ResolvedProgram,
    },
    source::Span,
};

use super::{
    capabilities::CopyCapabilities,
    categories::supports_alias_type,
    containment::validate_containment,
    conversion::{lower_parameter_mode, lower_type},
    diagnostic_codes::{
        INVALID_ALIAS_PARAMETER, INVALID_ENTRY_POINT, INVALID_EXTERNAL_DECLARATION,
        INVALID_OBJECT_DECLARATION, MISSING_ENTRY_POINT,
    },
    function::CallableChecker,
};

mod class;
mod function_types;
mod interfaces;
mod overrides;
mod static_fields;

use class::{check_class_definitions, lower_class_declarations};
use interfaces::analyze_interfaces;
use overrides::validate_override_signatures;

const EXTERNAL_PARAMETER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const EXTERNAL_RESULT_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool", "unit"];

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

fn check_internal_function_parameters(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    for declaration in program.declarations.iter() {
        if matches!(declaration.linkage, ResolvedFunctionLinkage::Internal) {
            validate_parameters(program, &declaration.parameters, diagnostics, "function");
            if matches!(
                lower_type(&declaration.return_type),
                Type::Obj | Type::Interface(_)
            ) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!(
                            "function `{}` cannot return a non-owning view",
                            declaration.name
                        ),
                    )
                    .with_primary_label(
                        declaration.return_type.span,
                        "non-owning views cannot escape a call",
                    ),
                );
            }
        }
    }
}

fn validate_parameters(
    program: &ResolvedProgram,
    parameters: &[ResolvedParameter],
    diagnostics: &mut Diagnostics,
    owner: &'static str,
) -> bool {
    let mut valid = true;
    for parameter in parameters {
        let ty = lower_type(&parameter.type_syntax);
        match parameter.binding_mode {
            ResolvedParameterBindingMode::Value
                if matches!(ty, Type::Unit | Type::Obj | Type::Interface(_)) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!(
                            "{owner} parameter `{}` requires a stored value type",
                            parameter.name
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "`unit`, `Obj`, and interface value parameters are unavailable",
                    ),
                );
                valid = false;
            }
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
            | ResolvedParameterBindingMode::MutableAlias { .. }
                if !supports_alias_type(program, ty) =>
            {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_ALIAS_PARAMETER,
                        format!(
                            "{owner} alias parameter `{}` must name a primitive, class, array, shared owner, interface, `Obj`, or supported optional",
                            parameter.name
                        ),
                    )
                    .with_primary_label(
                        parameter.type_syntax.span,
                        "`unit` and function aliases are unavailable",
                    ),
                );
                valid = false;
            }
            _ => {}
        }
    }
    valid
}

fn lower_parameter(parameter: &ResolvedParameter) -> HirParameter {
    HirParameter {
        id: parameter.id,
        mode: lower_parameter_mode(parameter.binding_mode),
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: lower_type(&parameter.type_syntax),
        span: parameter.span,
    }
}

fn check_entry_point(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Option<FunctionId> {
    let Some(entry_id) = program.entry_function else {
        let start = program.span.range().start();
        diagnostics.push(
            Diagnostic::error(MISSING_ENTRY_POINT, "missing entry function `main`")
                .with_primary_label(
                    Span::empty(program.span.source_id(), start),
                    "define `fn main() -> i64` in this file",
                ),
        );
        return None;
    };
    let entry = program
        .declarations
        .get(entry_id)
        .expect("resolved entry ID must exist in the declaration table");
    let return_type = lower_type(&entry.return_type);

    if !matches!(entry.linkage, ResolvedFunctionLinkage::Internal)
        || program.definitions.get(entry_id).is_none()
    {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(
                entry.name_span,
                "an external declaration cannot be the entry point",
            )
            .with_note("define `fn main() -> i64` with a Skald function body"),
        );
        return None;
    }

    if !entry.parameters.is_empty() || return_type != Type::I64 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_ENTRY_POINT,
                "entry function must have signature `fn main() -> i64`",
            )
            .with_primary_label(entry.name_span, "invalid entry signature")
            .with_note(format!(
                "found {} parameter{} and return type `{}`",
                entry.parameters.len(),
                if entry.parameters.len() == 1 { "" } else { "s" },
                return_type.name()
            )),
        );
        return None;
    }

    Some(entry_id)
}

fn check_external_declarations(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    for declaration in program.declarations.iter() {
        let ResolvedFunctionLinkage::External { link } = declaration.linkage else {
            continue;
        };
        let symbol = &program
            .external_links
            .get(link)
            .expect("resolved external declarations reference link entries")
            .symbol;
        if let Some(parameter) = declaration
            .parameters
            .iter()
            .find(|parameter| parameter.binding_mode != ResolvedParameterBindingMode::Value)
        {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_EXTERNAL_DECLARATION,
                    format!(
                        "external function `{}` cannot declare alias parameters",
                        declaration.name
                    ),
                )
                .with_primary_label(parameter.span, "aliases have no supported C ABI yet")
                .with_note("external parameters must be passed by value"),
            );
            continue;
        }
        if declaration.parameters.iter().any(|parameter| {
            crate::typeck::arrays::resolved_type_contains_array(program, parameter.type_syntax.kind)
        }) || crate::typeck::arrays::resolved_type_contains_array(
            program,
            declaration.return_type.kind,
        ) {
            // Array validation emits the more precise external-ABI diagnostic
            // at each offending type rather than duplicating this generic one.
            continue;
        }
        let has_valid_parameters = declaration.parameters.iter().all(|parameter| {
            matches!(
                lower_type(&parameter.type_syntax),
                Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
            )
        });
        let has_valid_return = matches!(
            lower_type(&declaration.return_type),
            Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool | Type::Unit
        );
        if !has_valid_parameters || !has_valid_return || symbol != &declaration.name {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_EXTERNAL_DECLARATION,
                    format!(
                        "external function `{}` has an unsupported signature",
                        declaration.name
                    ),
                )
                .with_primary_label(
                    declaration.span,
                    format!(
                        "expected by-value {} parameters and a result of type {}",
                        format_type_list(EXTERNAL_PARAMETER_TYPE_NAMES),
                        format_type_list(EXTERNAL_RESULT_TYPE_NAMES)
                    ),
                )
                .with_note("the source function name must also be its exact linker symbol"),
            );
        }
    }
}

fn lower_declaration(function: &ResolvedFunctionDeclaration) -> HirFunctionDeclaration {
    let parameters = function.parameters.iter().map(lower_parameter).collect();

    HirFunctionDeclaration {
        id: function.id,
        module: function.module,
        name: function.name.clone(),
        name_span: function.name_span,
        parameters,
        return_type: lower_type(&function.return_type),
        linkage: match &function.linkage {
            ResolvedFunctionLinkage::Internal => HirFunctionLinkage::Internal,
            ResolvedFunctionLinkage::External { link } => {
                HirFunctionLinkage::External { link: *link }
            }
            ResolvedFunctionLinkage::Intrinsic { intrinsic } => HirFunctionLinkage::Intrinsic {
                intrinsic: *intrinsic,
            },
            ResolvedFunctionLinkage::UnrecognizedIntrinsic => {
                unreachable!("unrecognized intrinsic declarations fail during resolution")
            }
        },
        span: function.span,
    }
}

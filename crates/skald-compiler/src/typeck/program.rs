//! Program-level validation and typed-HIR orchestration.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    hir::{
        HirClassDeclaration, HirClassDeclarationTable, HirClassDefinition, HirClassDefinitionTable,
        HirFieldDeclaration, HirFunctionDeclaration, HirFunctionDeclarationTable,
        HirFunctionDefinitionTable, HirFunctionLinkage, HirInitializerDeclaration,
        HirMethodDeclaration, HirParameter, HirProgram, HirReceiverAccess, Type,
    },
    identity::FunctionId,
    resolve::{
        ResolvedClassDeclaration, ResolvedFunctionDeclaration, ResolvedFunctionLinkage,
        ResolvedParameter, ResolvedParameterBindingMode, ResolvedProgram, ResolvedReceiverAccess,
        ResolvedType, ResolvedTypeKind,
    },
    source::Span,
};

use super::function::{CallableChecker, MemberCheckContext, ReceiverContext};

const EXTERNAL_PARAMETER_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool"];
const EXTERNAL_RESULT_TYPE_NAMES: &[&str] = &["i64", "u64", "u8", "f64", "bool", "unit"];

pub const MISSING_ENTRY_POINT: &str = "TYP001";
pub const INVALID_ENTRY_POINT: &str = "TYP002";
pub const INTEGER_LITERAL_OUT_OF_RANGE: &str = "TYP003";
pub const WRONG_ARGUMENT_COUNT: &str = "TYP004";
pub const TYPE_MISMATCH: &str = "TYP005";
pub const MISSING_RETURN: &str = "TYP006";
pub const INVALID_RETURN: &str = "TYP007";
pub const INVALID_CALL_STATEMENT: &str = "TYP008";
pub const INVALID_EXTERNAL_DECLARATION: &str = "TYP009";
pub const U64_LITERAL_OUT_OF_RANGE: &str = "TYP010";
pub const U8_LITERAL_OUT_OF_RANGE: &str = "TYP011";
pub const F64_LITERAL_OUT_OF_RANGE: &str = "TYP012";
pub const INVALID_OBJECT_DECLARATION: &str = "TYP013";
pub const INVALID_OBJECT_CONTEXT: &str = "TYP014";
pub const INVALID_CONSTRUCTION: &str = "TYP015";
pub const INVALID_INITIALIZER_BODY: &str = "TYP016";
pub const FIELD_INITIALIZATION: &str = "TYP017";
pub const READ_ONLY_RECEIVER: &str = "TYP018";
/// Temporary capability boundary removed when AL3 gives HIR explicit alias
/// parameter and argument-place semantics.
pub const ALIAS_PARAMETER_NOT_TYPE_CHECKED: &str = "TYP019";

#[derive(Debug)]
pub struct TypeCheckOutput {
    /// Present only when the entire resolved program type-checks successfully.
    pub hir: Option<HirProgram>,
    pub diagnostics: Diagnostics,
}

impl TypeCheckOutput {
    pub fn has_errors(&self) -> bool {
        self.diagnostics.has_errors()
    }
}

pub fn type_check(program: &ResolvedProgram) -> TypeCheckOutput {
    let mut diagnostics = Diagnostics::new();
    report_unimplemented_alias_parameters(program, &mut diagnostics);
    check_external_declarations(program, &mut diagnostics);
    let entry_function = check_entry_point(program, &mut diagnostics);
    let classes = lower_class_declarations(program, &mut diagnostics);
    let declarations = program.declarations.iter().map(lower_declaration).collect();
    let definitions = program
        .declarations
        .iter()
        .map(|declaration| {
            program.definitions.get(declaration.id).map(|definition| {
                CallableChecker::new(program, declaration, definition, &mut diagnostics).check()
            })
        })
        .collect();
    let class_definitions = check_class_definitions(program, &mut diagnostics);

    let hir = if diagnostics.has_errors() {
        None
    } else {
        Some(HirProgram {
            classes: HirClassDeclarationTable::new(classes),
            class_definitions: HirClassDefinitionTable::new(class_definitions),
            declarations: HirFunctionDeclarationTable::new(declarations),
            definitions: HirFunctionDefinitionTable::new(definitions),
            entry_function: entry_function.expect("valid program must have an entry function"),
            span: program.span,
        })
    };

    TypeCheckOutput { hir, diagnostics }
}

fn report_unimplemented_alias_parameters(program: &ResolvedProgram, diagnostics: &mut Diagnostics) {
    let function_parameters = program
        .declarations
        .iter()
        .flat_map(|declaration| &declaration.parameters);
    let member_parameters = program.classes.iter().flat_map(|class| {
        class
            .initializer
            .iter()
            .flat_map(|initializer| &initializer.parameters)
            .chain(class.methods.iter().flat_map(|method| &method.parameters))
    });

    for parameter in function_parameters.chain(member_parameters) {
        if parameter.binding_mode == ResolvedParameterBindingMode::Value {
            continue;
        }
        diagnostics.push(
            Diagnostic::error(
                ALIAS_PARAMETER_NOT_TYPE_CHECKED,
                "alias parameters are not available in typed HIR yet",
            )
            .with_primary_label(parameter.span, "the alias signature was resolved here")
            .with_note("AL3 adds alias access checking and typed place arguments"),
        );
    }
}

fn lower_class_declarations(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDeclaration> {
    program
        .classes
        .iter()
        .filter_map(|class| lower_class_declaration(class, diagnostics))
        .collect()
}

fn lower_class_declaration(
    class: &ResolvedClassDeclaration,
    diagnostics: &mut Diagnostics,
) -> Option<HirClassDeclaration> {
    let mut valid = true;
    let fields = class
        .fields
        .iter()
        .map(|field| {
            let ty = lower_type(&field.type_syntax);
            if !is_payload_primitive(ty) {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("field `{}` must have a primitive type", field.name),
                    )
                    .with_primary_label(
                        field.type_syntax.span,
                        "object and `unit` fields are unavailable",
                    ),
                );
                valid = false;
            }
            HirFieldDeclaration {
                id: field.id,
                name: field.name.clone(),
                name_span: field.name_span,
                ty,
                span: field.span,
            }
        })
        .collect();
    let Some(initializer) = &class.initializer else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_OBJECT_DECLARATION,
                format!("class `{}` requires an explicit initializer", class.name),
            )
            .with_primary_label(
                class.name_span,
                "add `init() {}` even when the class is empty",
            ),
        );
        return None;
    };
    valid &= validate_primitive_parameters(&initializer.parameters, diagnostics, "initializer");
    let initializer = HirInitializerDeclaration {
        id: initializer.id,
        parameters: initializer.parameters.iter().map(lower_parameter).collect(),
        span: initializer.span,
    };
    let methods = class
        .methods
        .iter()
        .map(|method| {
            valid &= validate_primitive_parameters(&method.parameters, diagnostics, "method");
            let return_type = lower_type(&method.return_type);
            if !is_payload_primitive(return_type) && return_type != Type::Unit {
                diagnostics.push(
                    Diagnostic::error(
                        INVALID_OBJECT_DECLARATION,
                        format!("method `{}` has an unavailable result type", method.name),
                    )
                    .with_primary_label(
                        method.return_type.span,
                        "expected a primitive type or `unit`",
                    ),
                );
                valid = false;
            }
            HirMethodDeclaration {
                id: method.id,
                name: method.name.clone(),
                name_span: method.name_span,
                receiver_access: lower_receiver_access(method.receiver_access),
                parameters: method.parameters.iter().map(lower_parameter).collect(),
                return_type,
                span: method.span,
            }
        })
        .collect();
    valid.then_some(HirClassDeclaration {
        id: class.id,
        name: class.name.clone(),
        name_span: class.name_span,
        fields,
        initializer,
        methods,
        span: class.span,
    })
}

fn check_class_definitions(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> Vec<HirClassDefinition> {
    program
        .classes
        .iter()
        .filter_map(|class| {
            let initializer = class.initializer.as_ref()?;
            let definition = program.class_definitions.get(class.id)?;
            let initializer_definition = definition.initializer.as_ref()?;
            let initializer_body = CallableChecker::new_member(
                program,
                MemberCheckContext {
                    callable: initializer.id.into(),
                    parameters: &initializer.parameters,
                    definition: initializer_definition,
                    return_type: Type::Unit,
                    receiver: ReceiverContext {
                        class: class.id,
                        access: HirReceiverAccess::Mutable,
                        initializer: true,
                    },
                    callable_name: format!("initializer for class `{}`", class.name),
                },
                diagnostics,
            )
            .check_member();
            let methods = class
                .methods
                .iter()
                .zip(&definition.methods)
                .map(|(method, body)| {
                    CallableChecker::new_member(
                        program,
                        MemberCheckContext {
                            callable: method.id.into(),
                            parameters: &method.parameters,
                            definition: body,
                            return_type: lower_type(&method.return_type),
                            receiver: ReceiverContext {
                                class: class.id,
                                access: lower_receiver_access(method.receiver_access),
                                initializer: false,
                            },
                            callable_name: format!("method `{}`", method.name),
                        },
                        diagnostics,
                    )
                    .check_member()
                })
                .collect();
            Some(HirClassDefinition {
                class: class.id,
                initializer: initializer_body,
                methods,
                span: definition.span,
            })
        })
        .collect()
}

fn validate_primitive_parameters(
    parameters: &[ResolvedParameter],
    diagnostics: &mut Diagnostics,
    owner: &'static str,
) -> bool {
    let mut valid = true;
    for parameter in parameters {
        if parameter.binding_mode != ResolvedParameterBindingMode::Value {
            continue;
        }
        if !is_payload_primitive(lower_type(&parameter.type_syntax)) {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_DECLARATION,
                    format!(
                        "{owner} parameter `{}` must have a primitive type",
                        parameter.name
                    ),
                )
                .with_primary_label(
                    parameter.type_syntax.span,
                    "object and `unit` parameters are unavailable",
                ),
            );
            valid = false;
        }
    }
    valid
}

fn lower_parameter(parameter: &ResolvedParameter) -> HirParameter {
    HirParameter {
        id: parameter.id,
        name: parameter.name.clone(),
        name_span: parameter.name_span,
        ty: lower_type(&parameter.type_syntax),
        span: parameter.span,
    }
}

const fn is_payload_primitive(ty: Type) -> bool {
    matches!(
        ty,
        Type::I64 | Type::U64 | Type::U8 | Type::F64 | Type::Bool
    )
}

const fn lower_receiver_access(access: ResolvedReceiverAccess) -> HirReceiverAccess {
    match access {
        ResolvedReceiverAccess::ReadOnly => HirReceiverAccess::ReadOnly,
        ResolvedReceiverAccess::Mutable => HirReceiverAccess::Mutable,
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
        let ResolvedFunctionLinkage::External { symbol } = &declaration.linkage else {
            continue;
        };
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
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| HirParameter {
            id: parameter.id,
            name: parameter.name.clone(),
            name_span: parameter.name_span,
            ty: lower_type(&parameter.type_syntax),
            span: parameter.span,
        })
        .collect();

    HirFunctionDeclaration {
        id: function.id,
        name: function.name.clone(),
        name_span: function.name_span,
        parameters,
        return_type: lower_type(&function.return_type),
        linkage: match &function.linkage {
            ResolvedFunctionLinkage::Internal => HirFunctionLinkage::Internal,
            ResolvedFunctionLinkage::External { symbol } => HirFunctionLinkage::External {
                symbol: symbol.clone(),
            },
        },
        span: function.span,
    }
}

pub(super) fn lower_type(type_syntax: &ResolvedType) -> Type {
    match type_syntax.kind {
        ResolvedTypeKind::I64 => Type::I64,
        ResolvedTypeKind::U64 => Type::U64,
        ResolvedTypeKind::U8 => Type::U8,
        ResolvedTypeKind::F64 => Type::F64,
        ResolvedTypeKind::Bool => Type::Bool,
        ResolvedTypeKind::Unit => Type::Unit,
        ResolvedTypeKind::Class(class) => Type::Class(class),
    }
}

//! Program-level validation and typed-HIR orchestration.

use crate::{
    diagnostics::{format_type_list, Diagnostic, Diagnostics},
    hir::{
        HirFunctionDeclaration, HirFunctionDeclarationTable, HirFunctionDefinitionTable,
        HirFunctionLinkage, HirParameter, HirProgram, Type,
    },
    identity::FunctionId,
    resolve::{
        ResolvedFunctionDeclaration, ResolvedFunctionLinkage, ResolvedProgram, ResolvedType,
        ResolvedTypeKind,
    },
    source::Span,
};

use super::function::FunctionChecker;

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
    check_external_declarations(program, &mut diagnostics);
    let entry_function = check_entry_point(program, &mut diagnostics);
    let declarations = program.declarations.iter().map(lower_declaration).collect();
    let definitions = program
        .declarations
        .iter()
        .map(|declaration| {
            program.definitions.get(declaration.id).map(|definition| {
                FunctionChecker::new(program, declaration, definition, &mut diagnostics).check()
            })
        })
        .collect();

    let hir = if diagnostics.has_errors() {
        None
    } else {
        Some(HirProgram {
            declarations: HirFunctionDeclarationTable::new(declarations),
            definitions: HirFunctionDefinitionTable::new(definitions),
            entry_function: entry_function.expect("valid program must have an entry function"),
            span: program.span,
        })
    };

    TypeCheckOutput { hir, diagnostics }
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

pub(super) const fn lower_type(type_syntax: &ResolvedType) -> Type {
    match type_syntax.kind {
        ResolvedTypeKind::I64 => Type::I64,
        ResolvedTypeKind::U64 => Type::U64,
        ResolvedTypeKind::U8 => Type::U8,
        ResolvedTypeKind::F64 => Type::F64,
        ResolvedTypeKind::Bool => Type::Bool,
        ResolvedTypeKind::Unit => Type::Unit,
    }
}

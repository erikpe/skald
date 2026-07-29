//! Canonical `std::error::panic` intrinsic selection and validation.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    intrinsic::Intrinsic,
    module::{ModulePath, ProgramModuleTable},
    resolve::{
        ResolvedFunctionDeclarationTable, ResolvedFunctionLinkage, ResolvedModuleDeclarationTable,
        ResolvedParameterBindingMode, ResolvedTopLevelId, ResolvedTypeKind, ResolvedVisibility,
    },
    source::Span,
};

use super::super::INVALID_INTRINSIC_DECLARATION;

const MODULE_PATH: &str = "std::error";
const STRING_MODULE_PATH: &str = "std::str";
const DECLARATION_NAME: &str = "panic";

pub(super) fn validate_panic_intrinsic(
    modules: &ProgramModuleTable,
    module_declarations: &ResolvedModuleDeclarationTable,
    functions: &ResolvedFunctionDeclarationTable,
    diagnostics: &mut Diagnostics,
) {
    let canonical_module = modules
        .find(&ModulePath::try_from(MODULE_PATH).expect("canonical error module path is valid"))
        .map(|module| module.module_id());

    for declaration in functions.iter().filter(|declaration| {
        matches!(
            declaration.linkage,
            ResolvedFunctionLinkage::Intrinsic { .. }
        )
    }) {
        if Some(declaration.module) != canonical_module || declaration.name != DECLARATION_NAME {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "intrinsic functions are reserved for compiler-defined declarations",
                )
                .with_primary_label(
                    declaration.span,
                    "this is not the canonical `std::error::panic` declaration",
                )
                .with_note(
                    "only `public intrinsic fn panic(message: std::str::Str) -> unit;` \
                     in `std::error` is recognized",
                ),
            );
        }
    }

    let Some(error_module) = canonical_module else {
        return;
    };
    let declarations = module_declarations
        .get(error_module)
        .expect("every loaded module has a declaration table");
    let Some(indexed) = declarations.get(DECLARATION_NAME) else {
        let source_id = modules
            .get(error_module)
            .expect("canonical error module must be loaded")
            .source_id();
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error` must declare the canonical `panic` intrinsic",
            )
            .with_primary_label(
                Span::empty(source_id, 0),
                "add `public intrinsic fn panic(message: std::str::Str) -> unit;`",
            ),
        );
        return;
    };
    let ResolvedTopLevelId::Function(function_id) = indexed.declaration else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must be an intrinsic function",
            )
            .with_primary_label(indexed.name_span, "declared with the wrong kind"),
        );
        return;
    };
    let declaration = functions
        .get(function_id)
        .expect("resolved function declaration identity must exist");
    if !matches!(
        declaration.linkage,
        ResolvedFunctionLinkage::Intrinsic {
            intrinsic: Intrinsic::Panic
        }
    ) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must use `intrinsic fn`",
            )
            .with_primary_label(
                declaration.span,
                "ordinary and external functions are not panic",
            ),
        );
        return;
    }

    if declaration.visibility != ResolvedVisibility::Public {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must be public",
            )
            .with_primary_label(declaration.name_span, "private intrinsic declaration"),
        );
    }

    let string_class = modules
        .find(
            &ModulePath::try_from(STRING_MODULE_PATH)
                .expect("canonical string module path is valid"),
        )
        .and_then(|module| module_declarations.get(module.module_id()))
        .and_then(|declarations| declarations.get("Str"))
        .and_then(|declaration| match declaration.declaration {
            ResolvedTopLevelId::Class(class) => Some(class),
            _ => None,
        });

    if declaration.parameters.len() != 1 {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must declare one parameter",
            )
            .with_primary_label(
                declaration.name_span,
                format!("found {} parameters", declaration.parameters.len()),
            ),
        );
    } else {
        let parameter = &declaration.parameters[0];
        if parameter.name != "message" {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must be named `message`",
                )
                .with_primary_label(parameter.name_span, "rename this parameter to `message`"),
            );
        }
        if parameter.binding_mode != ResolvedParameterBindingMode::Value {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must be passed by value",
                )
                .with_primary_label(parameter.span, "alias parameters are not allowed"),
            );
        }
        if string_class.is_none()
            || !matches!(
                parameter.type_syntax.kind,
                ResolvedTypeKind::Class(class) if Some(class) == string_class
            )
        {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_INTRINSIC_DECLARATION,
                    "the `std::error::panic` parameter must have exact type `std::str::Str`",
                )
                .with_primary_label(parameter.type_syntax.span, "wrong panic message type"),
            );
        }
    }

    if declaration.return_type.kind != ResolvedTypeKind::Unit {
        diagnostics.push(
            Diagnostic::error(
                INVALID_INTRINSIC_DECLARATION,
                "`std::error::panic` must return `unit`",
            )
            .with_primary_label(declaration.return_type.span, "wrong panic result type"),
        );
    }
}

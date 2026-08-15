//! Function-value signature validation and canonical HIR metadata lowering.

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirFunctionType, HirFunctionTypeParameter, HirFunctionTypeParameterMode,
        HirFunctionTypeTable,
    },
    resolve::{ResolvedFunctionTypeParameterMode, ResolvedProgram, ResolvedTypeKind},
};

use super::{lower_type, INVALID_ALIAS_PARAMETER};

pub(super) fn lower_function_types(
    program: &ResolvedProgram,
    diagnostics: &mut Diagnostics,
) -> HirFunctionTypeTable {
    let entries = program
        .function_types
        .iter()
        .map(|function| {
            let parameters = function
                .parameters
                .iter()
                .map(|parameter| {
                    if matches!(
                        parameter.mode,
                        ResolvedFunctionTypeParameterMode::ReadOnlyAlias
                            | ResolvedFunctionTypeParameterMode::MutableAlias
                    ) && matches!(parameter.type_syntax.kind, ResolvedTypeKind::Function(_))
                    {
                        diagnostics.push(
                            Diagnostic::error(
                                INVALID_ALIAS_PARAMETER,
                                "function values cannot be passed through callback-slot aliases",
                            )
                            .with_primary_label(
                                parameter.span,
                                "use a by-value function parameter in this function type",
                            ),
                        );
                    }
                    HirFunctionTypeParameter {
                        mode: lower_mode(parameter.mode),
                        ty: lower_type(program, &parameter.type_syntax),
                        span: parameter.span,
                    }
                })
                .collect();
            HirFunctionType {
                id: function.id,
                parameters,
                result: lower_type(program, &function.result),
                span: function.span,
            }
        })
        .collect();
    HirFunctionTypeTable::new(entries)
}

const fn lower_mode(mode: ResolvedFunctionTypeParameterMode) -> HirFunctionTypeParameterMode {
    match mode {
        ResolvedFunctionTypeParameterMode::Value => HirFunctionTypeParameterMode::Value,
        ResolvedFunctionTypeParameterMode::ReadOnlyAlias => {
            HirFunctionTypeParameterMode::ReadOnlyAlias
        }
        ResolvedFunctionTypeParameterMode::MutableAlias => {
            HirFunctionTypeParameterMode::MutableAlias
        }
    }
}

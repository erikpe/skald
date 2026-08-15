//! Deterministic external-link allocation and ABI compatibility checking.

use std::{collections::BTreeSet, fmt};

use super::*;
use crate::{
    external::{ExternalLink, ExternalLinkTable},
    identity::ExternalLinkId,
    module::ProgramModuleTable,
};

pub(super) struct ExternalLinkPlan {
    symbols: Vec<String>,
}

impl ExternalLinkPlan {
    pub(super) fn new<'symbol>(symbols: impl Iterator<Item = &'symbol str>) -> Self {
        let symbols = symbols
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        Self { symbols }
    }

    pub(super) fn link_for(&self, symbol: &str) -> ExternalLinkId {
        ExternalLinkId::new(
            self.symbols
                .binary_search_by(|candidate| candidate.as_str().cmp(symbol))
                .expect("every external declaration symbol is planned"),
        )
    }

    pub(super) fn finish(
        self,
        declarations: &[ResolvedFunctionDeclaration],
        modules: &ProgramModuleTable,
        diagnostics: &mut Diagnostics,
    ) -> ExternalLinkTable {
        let entries = self
            .symbols
            .into_iter()
            .enumerate()
            .map(|(index, symbol)| {
                let id = ExternalLinkId::new(index);
                let linked = declarations
                    .iter()
                    .filter(|declaration| {
                        matches!(
                            declaration.linkage,
                            ResolvedFunctionLinkage::External { link } if link == id
                        )
                    })
                    .collect::<Vec<_>>();
                report_incompatible_signatures(&symbol, &linked, modules, diagnostics);
                ExternalLink {
                    id,
                    symbol,
                    declarations: linked
                        .into_iter()
                        .map(|declaration| declaration.id)
                        .collect(),
                }
            })
            .collect();
        ExternalLinkTable::new(entries)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExternalAbiSignature {
    parameters: Vec<ExternalAbiType>,
    result: ExternalAbiType,
}

impl fmt::Display for ExternalAbiSignature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("fn(")?;
        for (index, parameter) in self.parameters.iter().enumerate() {
            if index > 0 {
                formatter.write_str(", ")?;
            }
            parameter.fmt(formatter)?;
        }
        write!(formatter, ") -> {}", self.result)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExternalAbiType {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
}

impl fmt::Display for ExternalAbiType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::I64 => "i64",
            Self::U64 => "u64",
            Self::U8 => "u8",
            Self::F64 => "f64",
            Self::Bool => "bool",
            Self::Unit => "unit",
        })
    }
}

fn report_incompatible_signatures(
    symbol: &str,
    declarations: &[&ResolvedFunctionDeclaration],
    modules: &ProgramModuleTable,
    diagnostics: &mut Diagnostics,
) {
    let valid = declarations
        .iter()
        .filter_map(|declaration| {
            external_abi_signature(declaration).map(|signature| (*declaration, signature))
        })
        .collect::<Vec<_>>();
    let Some((_, first)) = valid.first() else {
        return;
    };
    if valid.iter().all(|(_, signature)| signature == first) {
        return;
    }

    let mut diagnostic = Diagnostic::error(
        INCOMPATIBLE_EXTERNAL_ABI,
        format!("incompatible declarations for external symbol `{symbol}`"),
    );
    for (index, (declaration, signature)) in valid.iter().enumerate() {
        let module = modules
            .get(declaration.module)
            .expect("resolved declarations have loaded module owners");
        let label = format!("module `{}` declares `{signature}`", module.module_path());
        diagnostic = if index == 0 {
            diagnostic.with_primary_label(declaration.span, label)
        } else {
            diagnostic.with_secondary_label(declaration.span, label)
        };
    }
    diagnostics.push(diagnostic.with_note(
        "parameter names, visibility, aliases, and module ownership do not affect ABI compatibility",
    ));
}

fn external_abi_signature(
    declaration: &ResolvedFunctionDeclaration,
) -> Option<ExternalAbiSignature> {
    let parameters = declaration
        .parameters
        .iter()
        .map(|parameter| {
            matches!(parameter.binding_mode, ResolvedParameterBindingMode::Value)
                .then(|| external_abi_type(parameter.type_syntax.kind))
                .flatten()
                .filter(|ty| *ty != ExternalAbiType::Unit)
        })
        .collect::<Option<Vec<_>>>()?;
    let result = external_abi_type(declaration.return_type.kind)?;
    Some(ExternalAbiSignature { parameters, result })
}

fn external_abi_type(kind: ResolvedTypeKind) -> Option<ExternalAbiType> {
    match kind {
        ResolvedTypeKind::I64 => Some(ExternalAbiType::I64),
        ResolvedTypeKind::U64 => Some(ExternalAbiType::U64),
        ResolvedTypeKind::U8 => Some(ExternalAbiType::U8),
        ResolvedTypeKind::F64 => Some(ExternalAbiType::F64),
        ResolvedTypeKind::Bool => Some(ExternalAbiType::Bool),
        ResolvedTypeKind::Unit => Some(ExternalAbiType::Unit),
        ResolvedTypeKind::Obj
        | ResolvedTypeKind::Class(_)
        | ResolvedTypeKind::Interface(_)
        | ResolvedTypeKind::Function(_)
        | ResolvedTypeKind::Array(_)
        | ResolvedTypeKind::Shared(_)
        | ResolvedTypeKind::Optional(_) => None,
    }
}

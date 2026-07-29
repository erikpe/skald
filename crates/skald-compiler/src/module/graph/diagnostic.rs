use std::{collections::BTreeMap, io};

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    source::{SourceDatabase, SourceId, Span, TextRange},
};

use super::{
    entry::{EntryError, ModuleResolutionError},
    model::ModuleGraphLoadFailure,
};
use crate::module::{ModuleCandidate, ModulePath};

pub(super) const INVALID_ENTRY: &str = "MOD001";
pub(super) const AMBIGUOUS_ENTRY_IDENTITY: &str = "MOD002";
pub(super) const MISSING_MODULE: &str = "MOD003";
pub(super) const AMBIGUOUS_MODULE: &str = "MOD004";
pub(super) const MODULE_LOOKUP_FAILURE: &str = "MOD005";
pub(super) const MODULE_SOURCE_FAILURE: &str = "MOD006";
pub(super) const SELF_IMPORT: &str = "MOD007";

pub(super) enum PendingLoadError {
    Resolution {
        importing_module: ModulePath,
        import_range: TextRange,
        target: ModulePath,
        error: ModuleResolutionError,
    },
    Source {
        module_path: ModulePath,
        candidate: ModuleCandidate,
        imported_from: Option<(ModulePath, TextRange)>,
        kind: io::ErrorKind,
    },
}

pub(super) fn entry_failure(error: EntryError) -> ModuleGraphLoadFailure {
    let diagnostic = match error {
        EntryError::Invalid { path, reason } => {
            Diagnostic::error(INVALID_ENTRY, format!("invalid entry `{}`", path.display()))
                .with_note(reason)
        }
        EntryError::AmbiguousIdentity { path, identities } => {
            let mut diagnostic = Diagnostic::error(
                AMBIGUOUS_ENTRY_IDENTITY,
                format!(
                    "positional entry `{}` has multiple logical identities",
                    path.display()
                ),
            );
            for (provider, module_path) in identities {
                diagnostic = diagnostic.with_note(format!("{provider} provides `{module_path}`"));
            }
            diagnostic
        }
        EntryError::Resolution(error) => resolution_diagnostic(error, None),
    };
    let mut diagnostics = Diagnostics::new();
    diagnostics.push(diagnostic);
    ModuleGraphLoadFailure::new(SourceDatabase::new(), diagnostics)
}

pub(super) fn append_pending_diagnostics(
    diagnostics: &mut Diagnostics,
    mut errors: Vec<PendingLoadError>,
    source_ids: &BTreeMap<ModulePath, SourceId>,
) {
    errors.sort_by_key(pending_error_key);
    for error in errors {
        diagnostics.push(pending_error_diagnostic(error, source_ids));
    }
}

fn pending_error_key(error: &PendingLoadError) -> (String, String, usize) {
    match error {
        PendingLoadError::Resolution {
            importing_module,
            import_range,
            target,
            ..
        } => (
            importing_module.to_string(),
            target.to_string(),
            import_range.start(),
        ),
        PendingLoadError::Source {
            module_path,
            imported_from,
            ..
        } => (
            imported_from
                .as_ref()
                .map_or_else(|| module_path.to_string(), |(path, _)| path.to_string()),
            module_path.to_string(),
            imported_from
                .as_ref()
                .map_or(usize::MAX, |(_, range)| range.start()),
        ),
    }
}

fn pending_error_diagnostic(
    error: PendingLoadError,
    source_ids: &BTreeMap<ModulePath, SourceId>,
) -> Diagnostic {
    match error {
        PendingLoadError::Resolution {
            importing_module,
            import_range,
            error,
            ..
        } => resolution_diagnostic(
            error,
            Some(Span::new(source_ids[&importing_module], import_range)),
        ),
        PendingLoadError::Source {
            module_path,
            candidate,
            imported_from,
            kind,
        } => {
            let mut diagnostic = Diagnostic::error(
                MODULE_SOURCE_FAILURE,
                format!("cannot read module `{module_path}`"),
            )
            .with_note(format!(
                "source `{}` failed with {kind:?}",
                candidate.display_source_path().display()
            ));
            if let Some((importer, range)) = imported_from {
                diagnostic = diagnostic.with_primary_label(
                    Span::new(source_ids[&importer], range),
                    "this import reaches the unreadable source",
                );
            }
            diagnostic
        }
    }
}

fn resolution_diagnostic(error: ModuleResolutionError, span: Option<Span>) -> Diagnostic {
    let (mut diagnostic, label) = match error {
        ModuleResolutionError::Missing(path) => (
            Diagnostic::error(MISSING_MODULE, format!("module `{path}` was not found")),
            "this import has no provider candidate",
        ),
        ModuleResolutionError::Ambiguous(candidates) => {
            let path = candidates[0].module_path().clone();
            let mut diagnostic =
                Diagnostic::error(AMBIGUOUS_MODULE, format!("module `{path}` is ambiguous"));
            for candidate in candidates {
                diagnostic = diagnostic.with_note(format!(
                    "{} provides `{}`",
                    candidate.provider_id(),
                    candidate.display_source_path().display()
                ));
            }
            (diagnostic, "this import has multiple provider candidates")
        }
        ModuleResolutionError::Lookup(errors) => {
            let path = errors[0].module_path().clone();
            let mut diagnostic = Diagnostic::error(
                MODULE_LOOKUP_FAILURE,
                format!("module `{path}` could not be resolved"),
            );
            for error in errors {
                diagnostic = diagnostic.with_note(error.to_string());
            }
            (
                diagnostic,
                "this import reaches an invalid filesystem candidate",
            )
        }
    };
    if let Some(span) = span {
        diagnostic = diagnostic.with_primary_label(span, label);
    }
    diagnostic
}

pub(super) fn self_import_diagnostic(module: &ModulePath, span: Span) -> Diagnostic {
    Diagnostic::error(
        SELF_IMPORT,
        format!("module `{module}` cannot import itself"),
    )
    .with_primary_label(span, "remove this redundant import")
    .with_note("a module's own declarations are available without importing it")
}

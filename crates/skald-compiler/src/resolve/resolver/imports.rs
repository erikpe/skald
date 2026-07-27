//! Collection of direct module and selective ordinary import bindings.

use std::collections::HashMap;

use super::*;
use crate::{
    identity::ModuleId,
    module::{ModulePath, ProgramModuleTable},
};

pub(super) fn collect_module_bindings(
    module: ModuleId,
    ast: &syntax::CompilationUnit,
    modules: &ProgramModuleTable,
    diagnostics: &mut Diagnostics,
) -> ResolvedModuleBindings {
    let mut bindings = Vec::<ResolvedModuleBinding>::new();
    let mut indexes = HashMap::<ModulePath, usize>::new();

    for import in &ast.imports {
        let syntax::ImportDeclaration::Module(import) = import else {
            continue;
        };
        let canonical_path = module_path(&import.module);
        let target = modules
            .find(&canonical_path)
            .expect("a loaded graph contains every imported module")
            .module_id();
        let (local_path, name_span) = import.alias.as_ref().map_or_else(
            || (canonical_path.clone(), import.module.span),
            |alias| {
                (
                    ModulePath::from_components([alias.text.to_string()])
                        .expect("parsed aliases are valid module components"),
                    alias.span,
                )
            },
        );

        if let Some(previous) = indexes.get(&local_path).copied() {
            let previous = &bindings[previous];
            let previous_target = modules
                .get(previous.target)
                .expect("module bindings reference loaded modules")
                .module_path();
            let message = if previous.target == target {
                format!("repeated module binding `{local_path}`")
            } else {
                format!("conflicting module binding `{local_path}`")
            };
            diagnostics.push(
                Diagnostic::error(DUPLICATE_MODULE_BINDING, message)
                    .with_primary_label(name_span, "rebound here")
                    .with_secondary_label(previous.name_span, "first bound here")
                    .with_note(format!(
                        "the first binding selects `{previous_target}`; this import selects `{canonical_path}`"
                    )),
            );
            continue;
        }

        indexes.insert(local_path.clone(), bindings.len());
        bindings.push(ResolvedModuleBinding {
            local_path,
            target,
            name_span,
        });
    }
    bindings.sort_by(|left, right| left.local_path.cmp(&right.local_path));

    ResolvedModuleBindings::new(module, bindings)
}

pub(super) fn collect_ordinary_bindings(
    module: ModuleId,
    ast: &syntax::CompilationUnit,
    top_levels: &HashMap<String, TopLevelSymbol>,
    modules: &ProgramModuleTable,
    declarations: &ResolvedModuleDeclarationTable,
    module_spans: &[Span],
    diagnostics: &mut Diagnostics,
) -> ResolvedOrdinaryBindings {
    let mut bindings = Vec::<ResolvedOrdinaryBinding>::new();
    let mut indexes = HashMap::<String, usize>::new();

    for import in &ast.imports {
        let syntax::ImportDeclaration::Selective(import) = import else {
            continue;
        };
        let canonical_path = module_path(&import.module);
        let target_module = modules
            .find(&canonical_path)
            .expect("a loaded graph contains every imported module")
            .module_id();
        let target_declarations = declarations
            .get(target_module)
            .expect("every loaded module has a declaration index");

        for item in &import.items {
            let (local_name, name_span) = item.alias.as_ref().map_or_else(
                || (item.name.text.to_string(), item.name.span),
                |alias| (alias.text.to_string(), alias.span),
            );
            let Some(target) = target_declarations.get(item.name.text.as_str()) else {
                diagnostics.push(
                    Diagnostic::error(
                        UNKNOWN_IMPORTED_DECLARATION,
                        format!(
                            "module `{canonical_path}` has no directly owned declaration named `{}`",
                            item.name.text
                        ),
                    )
                    .with_primary_label(item.name.span, "unknown selective-import target")
                    .with_secondary_label(
                        module_spans[target_module.index()],
                        "import source module defined here",
                    )
                    .with_note(
                        "members, module bindings, and declarations imported by the target module cannot be selected",
                    ),
                );
                continue;
            };
            if !target.visibility.is_public() {
                diagnostics.push(
                    Diagnostic::error(
                        PRIVATE_DECLARATION,
                        format!("declaration `{canonical_path}::{}` is private", target.name),
                    )
                    .with_primary_label(item.name.span, "private declaration imported here")
                    .with_secondary_label(target.name_span, "declared private here"),
                );
                continue;
            }
            if let Some(owned) = top_levels.get(local_name.as_str()) {
                diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_ORDINARY_BINDING,
                        format!("imported name `{local_name}` conflicts with a local declaration"),
                    )
                    .with_primary_label(name_span, "imported under this name")
                    .with_secondary_label(owned.name_span, "declared locally here"),
                );
                continue;
            }
            if let Some(previous) = indexes.get(local_name.as_str()).copied() {
                diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_ORDINARY_BINDING,
                        format!("repeated imported name `{local_name}`"),
                    )
                    .with_primary_label(name_span, "rebound here")
                    .with_secondary_label(bindings[previous].name_span, "first bound here"),
                );
                continue;
            }

            indexes.insert(local_name.clone(), bindings.len());
            bindings.push(ResolvedOrdinaryBinding {
                local_name,
                target_module,
                target: target.declaration,
                name_span,
            });
        }
    }
    bindings.sort_by(|left, right| left.local_name.cmp(&right.local_name));
    ResolvedOrdinaryBindings::new(module, bindings)
}

fn module_path(name: &syntax::Name) -> ModulePath {
    ModulePath::from_components(name.components().map(|component| component.text.to_owned()))
        .expect("parsed import paths are valid module paths")
}

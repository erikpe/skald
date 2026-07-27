//! Current-module declaration lookup and direct module-binding selection.

use std::collections::HashMap;

use super::*;
use crate::{
    identity::ModuleId,
    module::{ModulePath, ProgramModuleTable},
};

#[derive(Clone, Copy, Debug)]
pub(super) enum TopLevelLookup {
    Found(TopLevelSymbol),
    Missing,
    Diagnosed,
}

#[derive(Clone, Copy)]
pub(super) struct ModuleLookup<'program> {
    current: ModuleId,
    top_levels: &'program HashMap<String, TopLevelSymbol>,
    bindings: &'program ResolvedModuleBindings,
    declarations: &'program ResolvedModuleDeclarationTable,
    modules: &'program ProgramModuleTable,
    module_spans: &'program [Span],
    qualified_enabled: bool,
}

impl<'program> ModuleLookup<'program> {
    pub(super) fn new(
        current: ModuleId,
        top_levels: &'program HashMap<String, TopLevelSymbol>,
        bindings: &'program ResolvedModuleBindings,
        declarations: &'program ResolvedModuleDeclarationTable,
        modules: &'program ProgramModuleTable,
        module_spans: &'program [Span],
        qualified_enabled: bool,
    ) -> Self {
        Self {
            current,
            top_levels,
            bindings,
            declarations,
            modules,
            module_spans,
            qualified_enabled,
        }
    }

    pub(super) fn select(
        self,
        name: &syntax::Name,
        diagnostics: &mut Diagnostics,
    ) -> TopLevelLookup {
        if !name.is_qualified() {
            return self
                .top_levels
                .get(name.text.as_str())
                .copied()
                .map_or(TopLevelLookup::Missing, TopLevelLookup::Found);
        }
        if !self.qualified_enabled {
            diagnostics.push(
                Diagnostic::error(
                    UNSUPPORTED_MODULE_SYNTAX,
                    "qualified names require whole-program module compilation",
                )
                .with_primary_label(
                    name.span,
                    "the single-file semantic adapter cannot resolve this name",
                ),
            );
            return TopLevelLookup::Diagnosed;
        }

        let components = name.components().collect::<Vec<_>>();
        let (leaf, module_components) = components
            .split_last()
            .expect("qualified names contain at least two components");
        let local_path = ModulePath::from_components(
            module_components
                .iter()
                .map(|component| component.text.to_owned()),
        )
        .expect("parsed qualified-name components are valid module components");
        let Some(binding) = self.bindings.get(&local_path) else {
            self.report_unknown_binding(name, &local_path, diagnostics);
            return TopLevelLookup::Diagnosed;
        };
        let target_path = self
            .modules
            .get(binding.target)
            .expect("module bindings reference loaded modules")
            .module_path();
        let declarations = self
            .declarations
            .get(binding.target)
            .expect("every loaded module has a declaration index");
        let Some(declaration) = declarations.get(leaf.text) else {
            diagnostics.push(
                Diagnostic::error(
                    UNKNOWN_QUALIFIED_DECLARATION,
                    format!(
                        "module `{target_path}` has no declaration named `{}`",
                        leaf.text
                    ),
                )
                .with_primary_label(leaf.span, "unknown declaration")
                .with_secondary_label(binding.name_span, "module bound here"),
            );
            return TopLevelLookup::Diagnosed;
        };
        if !declaration.visibility.is_public() {
            diagnostics.push(
                Diagnostic::error(
                    PRIVATE_DECLARATION,
                    format!(
                        "declaration `{target_path}::{}` is private",
                        declaration.name
                    ),
                )
                .with_primary_label(leaf.span, "private declaration used here")
                .with_secondary_label(declaration.name_span, "declared private here"),
            );
            return TopLevelLookup::Diagnosed;
        }
        TopLevelLookup::Found(TopLevelSymbol {
            kind: match declaration.declaration {
                ResolvedTopLevelId::Function(function) => TopLevelSymbolKind::Function(function),
                ResolvedTopLevelId::Class(class) => TopLevelSymbolKind::Class(class),
                ResolvedTopLevelId::Interface(interface) => {
                    TopLevelSymbolKind::Interface(interface)
                }
            },
            name_span: declaration.name_span,
        })
    }

    fn report_unknown_binding(
        self,
        name: &syntax::Name,
        local_path: &ModulePath,
        diagnostics: &mut Diagnostics,
    ) {
        let mut diagnostic = Diagnostic::error(
            UNKNOWN_MODULE_BINDING,
            format!("unknown module binding `{local_path}`"),
        )
        .with_primary_label(
            name.span,
            "qualified access requires an exact direct module import",
        );

        if let Some(target) = self.modules.find(local_path) {
            if let Some(binding) = self
                .bindings
                .iter()
                .find(|binding| binding.target == target.module_id())
            {
                diagnostic = diagnostic
                    .with_secondary_label(
                        binding.name_span,
                        format!("this module is bound locally as `{}`", binding.local_path),
                    )
                    .with_note(format!(
                        "use `{}::{}` through that binding",
                        binding.local_path,
                        name.components()
                            .last()
                            .expect("qualified names have a declaration leaf")
                            .text
                    ));
            } else {
                diagnostic = diagnostic
                    .with_secondary_label(
                        self.module_spans[target.module_id().index()],
                        "this module is reachable but is not directly imported here",
                    )
                    .with_note(format!(
                        "add `import {};` to this module",
                        target.module_path()
                    ));
            }
        } else if let Some(ancestor) = self.bindings.iter().find(|binding| {
            binding.local_path.len() < local_path.len()
                && binding
                    .local_path
                    .components()
                    .zip(local_path.components())
                    .all(|(left, right)| left == right)
        }) {
            diagnostic = diagnostic
                .with_secondary_label(
                    ancestor.name_span,
                    "this binding names only the imported module, not its descendants",
                )
                .with_note("import the descendant module directly before using it");
        } else {
            diagnostic = diagnostic.with_note(
                "absolute logical paths and transitive imports do not create local bindings",
            );
        }
        diagnostics.push(diagnostic.with_note(format!(
            "lookup occurred in module `{}`",
            self.modules
                .get(self.current)
                .expect("current module is loaded")
                .module_path()
        )));
    }
}

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

fn module_path(name: &syntax::Name) -> ModulePath {
    ModulePath::from_components(name.components().map(|component| component.text.to_owned()))
        .expect("parsed import paths are valid module paths")
}

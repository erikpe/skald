//! Deterministic multi-module declaration collection and body resolution.

use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{CallableId, InterfaceId, ModuleId, ParameterId},
    module::{ModuleGraph, ProgramModuleTable},
};

pub(super) const fn resolved_visibility(visibility: syntax::Visibility) -> ResolvedVisibility {
    match visibility {
        syntax::Visibility::Private => ResolvedVisibility::Private,
        syntax::Visibility::Public { .. } => ResolvedVisibility::Public,
    }
}

#[derive(Clone, Copy)]
struct FunctionWorkItem {
    id: FunctionId,
    ast_index: usize,
}

struct ModuleUnit<'ast> {
    ast: &'ast syntax::CompilationUnit,
    module: ModuleId,
    qualified_enabled: bool,
    top_levels: HashMap<String, TopLevelSymbol>,
    function_work: Vec<FunctionWorkItem>,
    class_work: Vec<(ClassId, usize)>,
    interface_work: Vec<(InterfaceId, usize)>,
    declarations: Vec<ResolvedModuleDeclaration>,
}

impl<'ast> ModuleUnit<'ast> {
    fn new(ast: &'ast syntax::CompilationUnit, module: ModuleId, qualified_enabled: bool) -> Self {
        Self {
            ast,
            module,
            qualified_enabled,
            top_levels: HashMap::new(),
            function_work: Vec::new(),
            class_work: Vec::new(),
            interface_work: Vec::new(),
            declarations: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
struct ProgramLookupTables<'program> {
    bindings: &'program ResolvedModuleBindingTable,
    ordinary_bindings: &'program ResolvedOrdinaryBindingTable,
    declarations: &'program ResolvedModuleDeclarationTable,
    module_spans: &'program [Span],
}

impl<'program> ProgramLookupTables<'program> {
    fn for_unit(
        self,
        unit: &'program ModuleUnit<'_>,
        modules: &'program ProgramModuleTable,
    ) -> ModuleLookup<'program> {
        ModuleLookup::new(
            unit.module,
            &unit.top_levels,
            ModuleLookupProgram {
                ordinary_bindings: self
                    .ordinary_bindings
                    .get(unit.module)
                    .expect("every module has an ordinary binding namespace"),
                bindings: self
                    .bindings
                    .get(unit.module)
                    .expect("every module has a binding namespace"),
                declarations: self.declarations,
                modules,
                module_spans: self.module_spans,
            },
            unit.qualified_enabled,
        )
    }
}

pub(super) struct ProgramResolver<'ast> {
    units: Vec<ModuleUnit<'ast>>,
    modules: ProgramModuleTable,
    reject_imports: bool,
    array_types: ArrayTypeInterner,
    diagnostics: Diagnostics,
}

impl<'ast> ProgramResolver<'ast> {
    pub(super) fn singleton(ast: &'ast syntax::CompilationUnit) -> Self {
        Self {
            units: vec![ModuleUnit::new(ast, ModuleId::new(0), false)],
            modules: ProgramModuleTable::singleton(ast.span.source_id()),
            reject_imports: true,
            array_types: ArrayTypeInterner::default(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub(super) fn from_graph(graph: &'ast ModuleGraph) -> Self {
        Self {
            units: graph
                .modules()
                .iter()
                .map(|module| ModuleUnit::new(module.ast(), module.provenance().module_id(), true))
                .collect(),
            modules: ProgramModuleTable::from_graph(graph),
            reject_imports: false,
            array_types: ArrayTypeInterner::default(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub(super) fn resolve(mut self) -> ResolveOutput {
        if self.reject_imports {
            for unit in &self.units {
                for import in &unit.ast.imports {
                    self.diagnostics.push(
                        Diagnostic::error(
                            UNSUPPORTED_MODULE_SYNTAX,
                            "module imports require whole-program module compilation",
                        )
                        .with_primary_label(
                            import.span(),
                            "the single-file semantic adapter cannot resolve this import",
                        )
                        .with_note("use the parsed module-graph resolver for module programs"),
                    );
                }
            }
        }
        self.collect_top_levels();

        let module_declarations = ResolvedModuleDeclarationTable::new(
            self.units
                .iter()
                .map(|unit| ResolvedModuleDeclarations::new(unit.module, unit.declarations.clone()))
                .collect(),
        );
        let module_bindings = self.collect_module_bindings();
        let module_spans = self
            .units
            .iter()
            .map(|unit| unit.ast.span)
            .collect::<Vec<_>>();
        let ordinary_bindings = self.collect_ordinary_bindings(&module_declarations, &module_spans);
        let lookups = ProgramLookupTables {
            bindings: &module_bindings,
            ordinary_bindings: &ordinary_bindings,
            declarations: &module_declarations,
            module_spans: &module_spans,
        };

        let function_declarations = self.collect_function_declarations(lookups);
        let interfaces = self.collect_interface_declarations(lookups);
        let (class_declarations, class_symbols, class_work) =
            self.collect_class_declarations(lookups);
        let function_declarations = ResolvedFunctionDeclarationTable::new(function_declarations);
        let mut class_declarations = ResolvedClassDeclarationTable::new(class_declarations);
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            resolve_interface_claims(
                unit.ast,
                &unit.class_work,
                lookup,
                &mut class_declarations,
                &mut self.diagnostics,
            );
        }
        let hierarchy =
            build_class_hierarchy(&class_declarations, &class_symbols, &mut self.diagnostics);
        let asts = self.units.iter().map(|unit| unit.ast).collect::<Vec<_>>();
        let virtual_families = resolve_virtual_families(
            &asts,
            &class_work,
            &mut class_declarations,
            &class_symbols,
            &hierarchy,
            &mut self.diagnostics,
        );

        let function_definitions = self.resolve_function_bodies(
            lookups,
            &function_declarations,
            &class_declarations,
            &hierarchy,
            &interfaces,
        );
        let mut class_definitions = Vec::with_capacity(class_declarations.len());
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            let unit_class_work = class_work
                .iter()
                .filter(|item| item.module == unit.module)
                .cloned()
                .collect::<Vec<_>>();
            class_definitions.extend(resolve_class_bodies(
                unit.ast,
                &unit_class_work,
                &class_declarations,
                BodyResolutionEnvironment::new(
                    lookup,
                    &function_declarations,
                    &class_declarations,
                    &interfaces,
                    &hierarchy,
                ),
                &mut self.array_types,
                &mut self.diagnostics,
            ));
        }
        let entry_unit = &self.units[self.modules.selected().index()];
        let entry_function =
            entry_unit
                .top_levels
                .get("main")
                .and_then(|symbol| match symbol.kind {
                    TopLevelSymbolKind::Function(function) => Some(function),
                    TopLevelSymbolKind::Class(_) => None,
                    TopLevelSymbolKind::Interface(_) => None,
                });

        let span = entry_unit.ast.span;
        ResolveOutput {
            program: ResolvedProgram {
                modules: self.modules,
                module_bindings,
                ordinary_bindings,
                module_declarations,
                array_types: self.array_types.finish(),
                declarations: function_declarations,
                definitions: ResolvedFunctionDefinitionTable::new(function_definitions),
                classes: class_declarations,
                interfaces,
                hierarchy,
                virtual_families,
                class_definitions: ResolvedClassDefinitionTable::new(class_definitions),
                entry_function,
                span,
            },
            diagnostics: self.diagnostics,
        }
    }

    fn collect_top_levels(&mut self) {
        let mut function_count = 0;
        let mut class_count = 0;
        let mut interface_count = 0;
        for unit in &mut self.units {
            for (ast_index, declaration) in unit.ast.declarations.iter().enumerate() {
                let name = declaration.name();
                if name.text == "Obj" {
                    self.diagnostics.push(
                        Diagnostic::error(
                            DUPLICATE_TOP_LEVEL,
                            "`Obj` is reserved as the universal object-view type",
                        )
                        .with_primary_label(name.span, "choose another top-level declaration name"),
                    );
                    continue;
                }
                let kind = match declaration {
                    syntax::TopLevelDeclaration::Function(_)
                    | syntax::TopLevelDeclaration::ExternalFunction(_) => {
                        TopLevelSymbolKind::Function(FunctionId::new(function_count))
                    }
                    syntax::TopLevelDeclaration::Class(_) => {
                        TopLevelSymbolKind::Class(ClassId::new(class_count))
                    }
                    syntax::TopLevelDeclaration::Interface(_) => {
                        TopLevelSymbolKind::Interface(InterfaceId::new(interface_count))
                    }
                };

                if let Some(previous) = unit.top_levels.get(name.text.as_str()) {
                    let both_functions = matches!(
                        (previous.kind, kind),
                        (
                            TopLevelSymbolKind::Function(_),
                            TopLevelSymbolKind::Function(_)
                        )
                    );
                    self.diagnostics.push(
                        Diagnostic::error(
                            DUPLICATE_TOP_LEVEL,
                            if both_functions {
                                format!("duplicate function `{}`", name.text)
                            } else {
                                format!("duplicate top-level declaration `{}`", name.text)
                            },
                        )
                        .with_primary_label(name.span, "redeclared here")
                        .with_secondary_label(previous.name_span, "first declared here"),
                    );
                    continue;
                }

                unit.top_levels.insert(
                    name.text.to_string(),
                    TopLevelSymbol {
                        kind,
                        name_span: name.span,
                    },
                );
                let visibility = match declaration.visibility() {
                    syntax::Visibility::Private => ResolvedVisibility::Private,
                    syntax::Visibility::Public { .. } => ResolvedVisibility::Public,
                };
                let declaration_id = match kind {
                    TopLevelSymbolKind::Function(id) => {
                        function_count += 1;
                        unit.function_work.push(FunctionWorkItem { id, ast_index });
                        ResolvedTopLevelId::Function(id)
                    }
                    TopLevelSymbolKind::Class(id) => {
                        class_count += 1;
                        unit.class_work.push((id, ast_index));
                        ResolvedTopLevelId::Class(id)
                    }
                    TopLevelSymbolKind::Interface(id) => {
                        interface_count += 1;
                        unit.interface_work.push((id, ast_index));
                        ResolvedTopLevelId::Interface(id)
                    }
                };
                unit.declarations.push(ResolvedModuleDeclaration {
                    name: name.text.to_string(),
                    name_span: name.span,
                    visibility,
                    declaration: declaration_id,
                });
            }
        }
    }

    fn collect_module_bindings(&mut self) -> ResolvedModuleBindingTable {
        ResolvedModuleBindingTable::new(
            self.units
                .iter()
                .map(|unit| {
                    if self.reject_imports {
                        ResolvedModuleBindings::new(unit.module, Vec::new())
                    } else {
                        collect_module_bindings(
                            unit.module,
                            unit.ast,
                            &self.modules,
                            &mut self.diagnostics,
                        )
                    }
                })
                .collect(),
        )
    }

    fn collect_ordinary_bindings(
        &mut self,
        declarations: &ResolvedModuleDeclarationTable,
        module_spans: &[Span],
    ) -> ResolvedOrdinaryBindingTable {
        ResolvedOrdinaryBindingTable::new(
            self.units
                .iter()
                .map(|unit| {
                    if self.reject_imports {
                        ResolvedOrdinaryBindings::new(unit.module, Vec::new())
                    } else {
                        collect_ordinary_bindings(
                            unit.module,
                            unit.ast,
                            &unit.top_levels,
                            &self.modules,
                            declarations,
                            module_spans,
                            &mut self.diagnostics,
                        )
                    }
                })
                .collect(),
        )
    }

    fn collect_function_declarations(
        &mut self,
        lookups: ProgramLookupTables<'_>,
    ) -> Vec<ResolvedFunctionDeclaration> {
        let mut declarations = Vec::new();
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            for item in &unit.function_work {
                let declaration = match &unit.ast.declarations[item.ast_index] {
                    syntax::TopLevelDeclaration::Function(function) => {
                        ResolvedFunctionDeclaration {
                            id: item.id,
                            module: unit.module,
                            visibility: resolved_visibility(function.visibility),
                            name: function.name.text.to_string(),
                            name_span: function.name.span,
                            parameters: resolve_parameters(
                                item.id.into(),
                                &function.parameters,
                                lookup,
                                &mut self.array_types,
                                &mut self.diagnostics,
                            ),
                            return_type: resolve_result_type(
                                &function.return_type,
                                lookup,
                                &mut self.array_types,
                                &mut self.diagnostics,
                            ),
                            linkage: ResolvedFunctionLinkage::Internal,
                            span: function.span,
                        }
                    }
                    syntax::TopLevelDeclaration::ExternalFunction(function) => {
                        ResolvedFunctionDeclaration {
                            id: item.id,
                            module: unit.module,
                            visibility: resolved_visibility(function.visibility),
                            name: function.name.text.to_string(),
                            name_span: function.name.span,
                            parameters: resolve_parameters(
                                item.id.into(),
                                &function.parameters,
                                lookup,
                                &mut self.array_types,
                                &mut self.diagnostics,
                            ),
                            return_type: resolve_result_type(
                                &function.return_type,
                                lookup,
                                &mut self.array_types,
                                &mut self.diagnostics,
                            ),
                            linkage: ResolvedFunctionLinkage::External {
                                symbol: function.name.text.to_string(),
                            },
                            span: function.span,
                        }
                    }
                    syntax::TopLevelDeclaration::Class(_)
                    | syntax::TopLevelDeclaration::Interface(_) => {
                        unreachable!("function work item must reference a function")
                    }
                };
                declarations.push(declaration);
            }
        }
        declarations
    }

    fn collect_interface_declarations(
        &mut self,
        lookups: ProgramLookupTables<'_>,
    ) -> ResolvedInterfaceDeclarationTable {
        let mut declarations = Vec::new();
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            declarations.extend(collect_interface_declarations(
                unit.ast,
                unit.module,
                &unit.interface_work,
                lookup,
                &mut self.array_types,
                &mut self.diagnostics,
            ));
        }
        ResolvedInterfaceDeclarationTable::new(declarations)
    }

    fn collect_class_declarations(
        &mut self,
        lookups: ProgramLookupTables<'_>,
    ) -> (
        Vec<ResolvedClassDeclaration>,
        Vec<ClassSymbols>,
        Vec<ClassWorkItem>,
    ) {
        let mut declarations = Vec::new();
        let mut symbols = Vec::new();
        let mut body_work = Vec::new();

        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            for &(id, ast_index) in &unit.class_work {
                let syntax::TopLevelDeclaration::Class(class) = &unit.ast.declarations[ast_index]
                else {
                    unreachable!("class work item must reference a class")
                };
                let (declaration, class_symbols, item) = collect_class(
                    id,
                    unit.module,
                    ast_index,
                    class,
                    lookup,
                    &mut self.array_types,
                    &mut self.diagnostics,
                );
                declarations.push(declaration);
                symbols.push(class_symbols);
                body_work.push(item);
            }
        }

        (declarations, symbols, body_work)
    }

    fn resolve_function_bodies(
        &mut self,
        lookups: ProgramLookupTables<'_>,
        functions: &ResolvedFunctionDeclarationTable,
        classes: &ResolvedClassDeclarationTable,
        hierarchy: &ResolvedClassHierarchy,
        interfaces: &ResolvedInterfaceDeclarationTable,
    ) -> Vec<Option<ResolvedFunctionDefinition>> {
        let mut definitions = Vec::with_capacity(functions.len());
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            for item in &unit.function_work {
                let declaration = functions
                    .get(item.id)
                    .expect("function work and declaration table must agree");
                let syntax::TopLevelDeclaration::Function(function) =
                    &unit.ast.declarations[item.ast_index]
                else {
                    definitions.push(None);
                    continue;
                };
                let body = resolve_callable_body(
                    CallableResolutionContext::function(item.id.into()),
                    &declaration.parameters,
                    &function.body,
                    BodyResolutionEnvironment::new(
                        lookup, functions, classes, interfaces, hierarchy,
                    ),
                    &mut self.array_types,
                    &mut self.diagnostics,
                );
                definitions.push(Some(ResolvedFunctionDefinition {
                    function: item.id,
                    locals: body.locals,
                    body: body.body,
                    span: function.span,
                }));
            }
        }
        definitions
    }
}

pub(super) fn resolve_parameters(
    callable: CallableId,
    parameters: &[syntax::Parameter],
    lookup: ModuleLookup<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedParameter> {
    let mut names = HashMap::<String, Span>::new();
    let mut resolved = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        if let Some(previous_span) = names.get(parameter.name.text.as_str()) {
            diagnostics.push(
                Diagnostic::error(
                    DUPLICATE_BINDING,
                    format!("duplicate parameter `{}`", parameter.name.text),
                )
                .with_primary_label(parameter.name.span, "redeclared here")
                .with_secondary_label(*previous_span, "first declared here"),
            );
            continue;
        }
        names.insert(parameter.name.text.to_string(), parameter.name.span);
        let Some(type_syntax) =
            resolve_type(&parameter.type_syntax, lookup, array_types, diagnostics)
        else {
            continue;
        };
        resolved.push(ResolvedParameter {
            id: ParameterId::new(callable, resolved.len()),
            binding_mode: resolve_parameter_binding_mode(parameter.binding_mode),
            name: parameter.name.text.to_string(),
            name_span: parameter.name.span,
            type_syntax,
            span: parameter.span,
        });
    }
    resolved
}

pub(super) const fn resolve_parameter_binding_mode(
    mode: syntax::ParameterBindingMode,
) -> ResolvedParameterBindingMode {
    match mode {
        syntax::ParameterBindingMode::Value => ResolvedParameterBindingMode::Value,
        syntax::ParameterBindingMode::ReadOnlyAlias { ref_span } => {
            ResolvedParameterBindingMode::ReadOnlyAlias { ref_span }
        }
        syntax::ParameterBindingMode::MutableAlias { mut_span, ref_span } => {
            ResolvedParameterBindingMode::MutableAlias { mut_span, ref_span }
        }
    }
}

pub(super) fn resolve_result_type(
    type_syntax: &syntax::TypeSyntax,
    lookup: ModuleLookup<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> ResolvedType {
    resolve_type(type_syntax, lookup, array_types, diagnostics).unwrap_or(ResolvedType {
        // Resolution diagnostics stop later phases. Retaining a payload-free
        // placeholder keeps declaration collection total and panic-free.
        kind: ResolvedTypeKind::Unit,
        span: type_syntax.span,
    })
}

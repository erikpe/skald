//! Deterministic multi-module declaration collection and body resolution.

use super::super::body::StringLiteralResolutionEnvironment;
use super::*;
use crate::{
    diagnostics::Diagnostic,
    identity::{
        CallableId, ClassTemplateId, InterfaceId, InterfaceTemplateId, LiteralDataId, ModuleId,
        ParameterId,
    },
    lexer::decode_string_literal,
    module::{ModuleGraph, ModulePath, ProgramModuleTable},
};
use std::path::Path;

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

pub(super) struct ModuleUnit<'ast> {
    pub(super) ast: &'ast syntax::CompilationUnit,
    pub(super) module: ModuleId,
    qualified_enabled: bool,
    top_levels: HashMap<String, TopLevelSymbol>,
    function_work: Vec<FunctionWorkItem>,
    class_work: Vec<(ClassId, usize)>,
    pub(super) template_work: Vec<ClassTemplateWorkItem>,
    interface_work: Vec<(InterfaceId, usize)>,
    pub(super) interface_template_work: Vec<InterfaceTemplateWorkItem>,
    declarations: Vec<ResolvedModuleDeclaration>,
}

fn collect_literal_data(
    graph: &ModuleGraph,
) -> (Vec<ResolvedLiteralData>, HashMap<Span, LiteralDataId>) {
    let path = ModulePath::try_from("std::str").expect("canonical string module path is valid");
    let Some(target) = graph
        .find(&path)
        .map(|module| module.provenance().module_id())
    else {
        return (Vec::new(), HashMap::new());
    };
    let spans = graph.modules().iter().flat_map(|module| {
        module
            .imports()
            .iter()
            .filter(move |edge| edge.target() == target)
            .flat_map(|edge| edge.string_literal_spans().iter().copied())
    });
    let mut data = Vec::new();
    let mut ids = HashMap::new();
    for span in spans {
        let source = graph
            .sources()
            .get(span.source_id())
            .expect("literal evidence must reference a loaded source");
        let lexeme = source
            .slice(span.range())
            .expect("literal evidence must lie within its source");
        let id = LiteralDataId::new(data.len());
        ids.insert(span, id);
        data.push(ResolvedLiteralData {
            id,
            bytes: decode_string_literal(lexeme),
            span,
        });
    }
    (data, ids)
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
            template_work: Vec::new(),
            interface_work: Vec::new(),
            interface_template_work: Vec::new(),
            declarations: Vec::new(),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct ProgramLookupTables<'program> {
    bindings: &'program ResolvedModuleBindingTable,
    ordinary_bindings: &'program ResolvedOrdinaryBindingTable,
    declarations: &'program ResolvedModuleDeclarationTable,
    module_spans: &'program [Span],
    class_templates: &'program ResolvedClassTemplateTable,
    type_parameters: &'program ResolvedTypeParameterTable,
    specializations: Option<&'program GenericSpecializationTable>,
}

impl<'program> ProgramLookupTables<'program> {
    pub(super) const fn with_specializations(
        mut self,
        specializations: &'program GenericSpecializationTable,
    ) -> Self {
        self.specializations = Some(specializations);
        self
    }

    pub(super) fn for_unit(
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
                class_templates: self.class_templates,
                type_parameters: self.type_parameters,
                specializations: self.specializations,
            },
            unit.qualified_enabled,
        )
    }
}

pub(super) struct ProgramResolver<'ast> {
    units: Vec<ModuleUnit<'ast>>,
    modules: ProgramModuleTable,
    has_module_context: bool,
    type_interner: ResolvedTypeInterner,
    address_taken_callables: ResolvedAddressTakenCallableTable,
    literal_data: Vec<ResolvedLiteralData>,
    literal_ids: HashMap<Span, LiteralDataId>,
    diagnostics: Diagnostics,
}

impl<'ast> ProgramResolver<'ast> {
    pub(super) fn singleton(ast: &'ast syntax::CompilationUnit, source_path: &Path) -> Self {
        Self {
            units: vec![ModuleUnit::new(ast, ModuleId::new(0), false)],
            modules: ProgramModuleTable::singleton(ast.span.source_id(), source_path),
            has_module_context: false,
            type_interner: ResolvedTypeInterner::default(),
            address_taken_callables: ResolvedAddressTakenCallableTable::default(),
            literal_data: Vec::new(),
            literal_ids: HashMap::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub(super) fn from_graph(graph: &'ast ModuleGraph) -> Self {
        let (literal_data, literal_ids) = collect_literal_data(graph);
        Self {
            units: graph
                .modules()
                .iter()
                .map(|module| ModuleUnit::new(module.ast(), module.provenance().module_id(), true))
                .collect(),
            modules: ProgramModuleTable::from_graph(graph),
            has_module_context: true,
            type_interner: ResolvedTypeInterner::default(),
            address_taken_callables: ResolvedAddressTakenCallableTable::default(),
            literal_data,
            literal_ids,
            diagnostics: Diagnostics::new(),
        }
    }

    pub(super) fn resolve(mut self) -> ResolveOutput {
        if !self.has_module_context {
            for unit in &self.units {
                for import in &unit.ast.imports {
                    self.diagnostics.push(
                        Diagnostic::error(
                            MODULE_CONTEXT_REQUIRED,
                            "module imports require whole-program module compilation",
                        )
                        .with_primary_label(
                            import.span(),
                            "use a compilation request to supply module roots and an entry",
                        )
                        .with_note("the source-text convenience API has no filesystem context"),
                    );
                }
            }
        }
        self.collect_top_levels();

        let CollectedGenericTemplates {
            classes: class_templates,
            interfaces: interface_templates,
            parameters: type_parameters,
        } = collect_generic_templates(&self.units, &mut self.diagnostics);

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
            class_templates: &class_templates,
            type_parameters: &type_parameters,
            specializations: None,
        };

        let external_link_plan = ExternalLinkPlan::new(self.units.iter().flat_map(|unit| {
            unit.function_work.iter().filter_map(|item| {
                match &unit.ast.declarations[item.ast_index] {
                    syntax::TopLevelDeclaration::ExternalFunction(function) => {
                        Some(function.name.text.as_str())
                    }
                    syntax::TopLevelDeclaration::Function(_)
                    | syntax::TopLevelDeclaration::IntrinsicFunction(_)
                    | syntax::TopLevelDeclaration::Class(_)
                    | syntax::TopLevelDeclaration::Interface(_) => None,
                }
            })
        }));
        let interfaces = self.collect_interface_declarations(lookups);
        let mut interface_template_semantics = Vec::new();
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            for item in &unit.interface_template_work {
                let syntax::TopLevelDeclaration::Interface(interface) =
                    &unit.ast.declarations[item.ast_index]
                else {
                    unreachable!("interface-template work must reference an interface declaration")
                };
                let parameters = type_parameters
                    .for_interface_template(item.id)
                    .expect("every interface template has one parameter list");
                interface_template_semantics.push(resolve_interface_template_semantics(
                    item.id,
                    interface,
                    parameters,
                    lookup,
                    &mut self.diagnostics,
                ));
            }
        }
        let interface_template_semantics =
            ResolvedInterfaceTemplateSemanticTable::new(interface_template_semantics);
        let mut template_semantics = Vec::new();
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            for item in &unit.template_work {
                let syntax::TopLevelDeclaration::Class(class) =
                    &unit.ast.declarations[item.ast_index]
                else {
                    unreachable!("class-template work must reference a class declaration")
                };
                let parameters = type_parameters
                    .for_template(item.id)
                    .expect("every class template has one parameter list");
                template_semantics.push(resolve_class_template_semantics(
                    item.id,
                    class,
                    parameters,
                    lookup,
                    &interfaces,
                    &mut self.diagnostics,
                ));
            }
        }
        let template_semantics = ResolvedClassTemplateSemanticTable::new(template_semantics);
        let ordinary_class_count = self.units.iter().map(|unit| unit.class_work.len()).sum();
        let discovery = discover_specializations(
            SpecializationDiscoveryInput::new(
                &self.units,
                &self.modules,
                lookups,
                GenericTemplateDiscoveryInput::new(
                    &template_semantics,
                    &interface_template_semantics,
                    &class_templates,
                    &interface_templates,
                ),
                ordinary_class_count,
                interfaces.len(),
            ),
            &mut self.type_interner,
            &mut self.diagnostics,
        );
        let generic_specializations = discovery.class_specializations;
        let generic_interface_specializations = discovery.interface_specializations;
        let lookups = lookups.with_specializations(&generic_specializations);
        let function_declarations =
            self.collect_function_declarations(lookups, &external_link_plan);
        let external_links =
            external_link_plan.finish(&function_declarations, &self.modules, &mut self.diagnostics);
        let (class_declarations, class_symbols, class_work) =
            self.collect_class_declarations(lookups);
        debug_assert_eq!(class_declarations.len(), ordinary_class_count);
        let function_declarations = ResolvedFunctionDeclarationTable::new(function_declarations);
        validate_intrinsic_declarations(
            &self.modules,
            &module_declarations,
            &function_declarations,
            &self.type_interner,
            &mut self.diagnostics,
        );
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
        let ordinary_hierarchy = {
            let mut diagnostics = Diagnostics::new();
            build_class_hierarchy(&class_declarations, &class_symbols, &mut diagnostics)
        };
        let ordinary_classes = class_declarations.clone();
        let specialized = specialize_declarations(
            SpecializationDeclarationInput::new(
                &self.units,
                &self.modules,
                &template_semantics,
                &generic_specializations,
                &class_declarations,
                &interfaces,
                &self.type_interner,
            ),
            &mut self.diagnostics,
        );
        let mut class_symbols = class_symbols;
        if specialized.valid {
            class_declarations.extend(specialized.declarations);
            class_symbols.extend(specialized.symbols);
        }
        let hierarchy =
            build_class_hierarchy(&class_declarations, &class_symbols, &mut self.diagnostics);
        let asts = self.units.iter().map(|unit| unit.ast).collect::<Vec<_>>();
        let mut virtual_work = class_work.clone();
        if specialized.valid {
            virtual_work.extend(generated_class_work(
                &self.units,
                &generic_specializations,
                &class_declarations,
            ));
        }
        let virtual_families = resolve_virtual_families(
            &asts,
            &virtual_work,
            &mut class_declarations,
            &class_symbols,
            &hierarchy,
            &mut self.diagnostics,
        );
        let string_language_item = validate_string_language_item(
            &self.modules,
            &module_declarations,
            &class_declarations,
            &function_declarations,
            &self.type_interner,
            &self.literal_data,
            &mut self.diagnostics,
        );

        let mut static_initializer_updates = Vec::new();
        for unit in &self.units {
            let lookup = lookups.for_unit(unit, &self.modules);
            let unit_class_work = class_work
                .iter()
                .filter(|item| item.module == unit.module)
                .cloned()
                .collect::<Vec<_>>();
            static_initializer_updates.extend(resolve_static_field_initializers(
                unit.ast,
                &unit_class_work,
                &class_declarations,
                BodyResolutionEnvironment::new(
                    lookup,
                    &function_declarations,
                    &class_declarations,
                    &interfaces,
                    &hierarchy,
                    self.has_module_context,
                    StringLiteralResolutionEnvironment::new(
                        string_language_item.as_ref(),
                        &self.literal_ids,
                    ),
                ),
                &mut self.type_interner,
                &mut self.address_taken_callables,
                &mut self.diagnostics,
            ));
        }
        attach_static_field_initializers(&mut class_declarations, static_initializer_updates);

        let specialized_bodies = specialize_bodies(
            SpecializationBodyInput {
                units: &self.units,
                modules: &self.modules,
                lookups,
                semantics: &template_semantics,
                specializations: &generic_specializations,
                functions: &function_declarations,
                classes: &class_declarations,
                interfaces: &interfaces,
                hierarchy: &hierarchy,
                has_module_context: self.has_module_context,
                string_literals: StringLiteralResolutionEnvironment::new(
                    string_language_item.as_ref(),
                    &self.literal_ids,
                ),
            },
            &mut self.type_interner,
            &mut self.address_taken_callables,
            &mut self.diagnostics,
        );
        if specialized_bodies.valid {
            attach_static_field_initializers(
                &mut class_declarations,
                specialized_bodies.static_initializers,
            );
        }

        let function_definitions = self.resolve_function_bodies(
            lookups,
            &function_declarations,
            &class_declarations,
            &hierarchy,
            &interfaces,
            string_language_item.as_ref(),
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
                    self.has_module_context,
                    StringLiteralResolutionEnvironment::new(
                        string_language_item.as_ref(),
                        &self.literal_ids,
                    ),
                ),
                &mut self.type_interner,
                &mut self.address_taken_callables,
                &mut self.diagnostics,
            ));
        }
        if specialized_bodies.valid {
            class_definitions.extend(specialized_bodies.definitions);
        }
        let entry_unit = &self.units[self.modules.selected().index()];
        let entry_function =
            entry_unit
                .top_levels
                .get("main")
                .and_then(|symbol| match symbol.kind {
                    TopLevelSymbolKind::Function(function) => Some(function),
                    TopLevelSymbolKind::Class(_) => None,
                    TopLevelSymbolKind::ClassTemplate(_) => None,
                    TopLevelSymbolKind::Interface(_) => None,
                    TopLevelSymbolKind::InterfaceTemplate(_) => None,
                });

        let span = entry_unit.ast.span;
        let (array_types, function_types, optional_types, optional_box_types) =
            self.type_interner.finish();
        let mut output = ResolveOutput {
            program: ResolvedProgram {
                modules: self.modules,
                external_links,
                module_bindings,
                ordinary_bindings,
                module_declarations,
                class_templates,
                interface_templates,
                interface_template_semantics,
                type_parameters,
                template_semantics,
                generic_specializations,
                generic_interface_specializations,
                function_types,
                address_taken_callables: self.address_taken_callables,
                array_types,
                optional_types,
                optional_box_types,
                string_language_item,
                literal_data: ResolvedLiteralDataTable::new(self.literal_data),
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
        };
        validate_specialization_requirements(
            &mut output.program,
            &mut output.diagnostics,
            ordinary_class_count,
            ordinary_hierarchy,
            ordinary_classes,
        );
        output
    }

    fn collect_top_levels(&mut self) {
        let mut function_count = 0;
        let mut class_count = 0;
        let mut interface_count = 0;
        let mut template_count = 0;
        let mut interface_template_count = 0;
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
                    | syntax::TopLevelDeclaration::ExternalFunction(_)
                    | syntax::TopLevelDeclaration::IntrinsicFunction(_) => {
                        TopLevelSymbolKind::Function(FunctionId::new(function_count))
                    }
                    syntax::TopLevelDeclaration::Class(class)
                        if class.type_parameters.is_some() =>
                    {
                        TopLevelSymbolKind::ClassTemplate(ClassTemplateId::new(template_count))
                    }
                    syntax::TopLevelDeclaration::Class(_) => {
                        TopLevelSymbolKind::Class(ClassId::new(class_count))
                    }
                    syntax::TopLevelDeclaration::Interface(interface)
                        if interface.type_parameters.is_some()
                            || interface.where_clause.is_some() =>
                    {
                        TopLevelSymbolKind::InterfaceTemplate(InterfaceTemplateId::new(
                            interface_template_count,
                        ))
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
                    TopLevelSymbolKind::ClassTemplate(id) => {
                        template_count += 1;
                        unit.template_work
                            .push(ClassTemplateWorkItem { id, ast_index });
                        ResolvedTopLevelId::ClassTemplate(id)
                    }
                    TopLevelSymbolKind::Interface(id) => {
                        interface_count += 1;
                        unit.interface_work.push((id, ast_index));
                        ResolvedTopLevelId::Interface(id)
                    }
                    TopLevelSymbolKind::InterfaceTemplate(id) => {
                        interface_template_count += 1;
                        unit.interface_template_work
                            .push(InterfaceTemplateWorkItem { id, ast_index });
                        ResolvedTopLevelId::InterfaceTemplate(id)
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
                    if !self.has_module_context {
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
                    if !self.has_module_context {
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
        external_links: &ExternalLinkPlan,
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
                                &mut self.type_interner,
                                &mut self.diagnostics,
                            ),
                            return_type: resolve_result_type(
                                &function.return_type,
                                lookup,
                                &mut self.type_interner,
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
                                &mut self.type_interner,
                                &mut self.diagnostics,
                            ),
                            return_type: resolve_result_type(
                                &function.return_type,
                                lookup,
                                &mut self.type_interner,
                                &mut self.diagnostics,
                            ),
                            linkage: ResolvedFunctionLinkage::External {
                                link: external_links.link_for(function.name.text.as_str()),
                            },
                            span: function.span,
                        }
                    }
                    syntax::TopLevelDeclaration::IntrinsicFunction(function) => {
                        let linkage = intrinsic_for_declaration(
                            &self.modules,
                            unit.module,
                            function.name.text.as_str(),
                        )
                        .map(|intrinsic| ResolvedFunctionLinkage::Intrinsic { intrinsic })
                        .unwrap_or(ResolvedFunctionLinkage::UnrecognizedIntrinsic);
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
                                &mut self.type_interner,
                                &mut self.diagnostics,
                            ),
                            return_type: resolve_result_type(
                                &function.return_type,
                                lookup,
                                &mut self.type_interner,
                                &mut self.diagnostics,
                            ),
                            linkage,
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
                &mut self.type_interner,
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
                    &mut self.type_interner,
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
        string_language_item: Option<&ResolvedStringLanguageItem>,
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
                        lookup,
                        functions,
                        classes,
                        interfaces,
                        hierarchy,
                        self.has_module_context,
                        StringLiteralResolutionEnvironment::new(
                            string_language_item,
                            &self.literal_ids,
                        ),
                    ),
                    &mut self.type_interner,
                    &mut self.address_taken_callables,
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
    type_interner: &mut ResolvedTypeInterner,
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
            resolve_type(&parameter.type_syntax, lookup, type_interner, diagnostics)
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
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> ResolvedType {
    resolve_type(type_syntax, lookup, type_interner, diagnostics).unwrap_or(ResolvedType {
        // Resolution diagnostics stop later phases. Retaining a payload-free
        // placeholder keeps declaration collection total and panic-free.
        kind: ResolvedTypeKind::Unit,
        span: type_syntax.span,
    })
}

//! Program-wide declaration collection and deterministic identity assignment.

use super::{
    body::{resolve_callable_body, BaseInitializationPolicy, BodyResolutionEnvironment},
    *,
};
use crate::{
    diagnostics::Diagnostic,
    identity::{CallableId, ParameterId},
};

mod class;
mod class_body;
mod hierarchy;

use class::{collect_class, ClassWorkItem};
use class_body::resolve_class_bodies;
use hierarchy::build_class_hierarchy;

#[derive(Clone, Copy)]
struct FunctionWorkItem {
    id: FunctionId,
    ast_index: usize,
}

pub(super) struct ProgramResolver<'ast> {
    ast: &'ast syntax::CompilationUnit,
    top_levels: HashMap<String, TopLevelSymbol>,
    function_work: Vec<FunctionWorkItem>,
    class_work: Vec<(ClassId, usize)>,
    diagnostics: Diagnostics,
}

impl<'ast> ProgramResolver<'ast> {
    pub(super) fn new(ast: &'ast syntax::CompilationUnit) -> Self {
        Self {
            ast,
            top_levels: HashMap::new(),
            function_work: Vec::new(),
            class_work: Vec::new(),
            diagnostics: Diagnostics::new(),
        }
    }

    pub(super) fn resolve(mut self) -> ResolveOutput {
        self.collect_top_levels();

        let function_declarations = self.collect_function_declarations();
        let (class_declarations, class_symbols, class_work) = self.collect_class_declarations();
        let function_declarations = ResolvedFunctionDeclarationTable::new(function_declarations);
        let class_declarations = ResolvedClassDeclarationTable::new(class_declarations);
        let hierarchy = build_class_hierarchy(
            self.ast,
            &class_work,
            &class_declarations,
            &class_symbols,
            &mut self.diagnostics,
        );

        let function_definitions = self.resolve_function_bodies(
            &function_declarations,
            &class_declarations,
            &class_symbols,
        );
        let class_definitions = resolve_class_bodies(
            self.ast,
            &self.top_levels,
            &class_work,
            &class_declarations,
            &class_symbols,
            &mut self.diagnostics,
        );
        let entry_function = self
            .top_levels
            .get("main")
            .and_then(|symbol| match symbol.kind {
                TopLevelSymbolKind::Function(function) => Some(function),
                TopLevelSymbolKind::Class(_) => None,
            });

        ResolveOutput {
            program: ResolvedProgram {
                declarations: function_declarations,
                definitions: ResolvedFunctionDefinitionTable::new(function_definitions),
                classes: class_declarations,
                hierarchy,
                class_definitions: ResolvedClassDefinitionTable::new(class_definitions),
                entry_function,
                span: self.ast.span,
            },
            diagnostics: self.diagnostics,
        }
    }

    fn collect_top_levels(&mut self) {
        for (ast_index, declaration) in self.ast.declarations.iter().enumerate() {
            let name = declaration.name();
            let kind = match declaration {
                syntax::TopLevelDeclaration::Function(_)
                | syntax::TopLevelDeclaration::ExternalFunction(_) => {
                    TopLevelSymbolKind::Function(FunctionId::new(self.function_work.len()))
                }
                syntax::TopLevelDeclaration::Class(_) => {
                    TopLevelSymbolKind::Class(ClassId::new(self.class_work.len()))
                }
            };

            if let Some(previous) = self.top_levels.get(&name.text) {
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

            self.top_levels.insert(
                name.text.clone(),
                TopLevelSymbol {
                    kind,
                    name_span: name.span,
                },
            );
            match kind {
                TopLevelSymbolKind::Function(id) => {
                    self.function_work.push(FunctionWorkItem { id, ast_index });
                }
                TopLevelSymbolKind::Class(id) => self.class_work.push((id, ast_index)),
            }
        }
    }

    fn collect_function_declarations(&mut self) -> Vec<ResolvedFunctionDeclaration> {
        let work = self.function_work.clone();
        work.into_iter()
            .map(|item| match &self.ast.declarations[item.ast_index] {
                syntax::TopLevelDeclaration::Function(function) => ResolvedFunctionDeclaration {
                    id: item.id,
                    name: function.name.text.clone(),
                    name_span: function.name.span,
                    parameters: resolve_parameters(
                        item.id.into(),
                        &function.parameters,
                        &self.top_levels,
                        &mut self.diagnostics,
                    ),
                    return_type: resolve_result_type(
                        &function.return_type,
                        &self.top_levels,
                        &mut self.diagnostics,
                    ),
                    linkage: ResolvedFunctionLinkage::Internal,
                    span: function.span,
                },
                syntax::TopLevelDeclaration::ExternalFunction(function) => {
                    ResolvedFunctionDeclaration {
                        id: item.id,
                        name: function.name.text.clone(),
                        name_span: function.name.span,
                        parameters: resolve_parameters(
                            item.id.into(),
                            &function.parameters,
                            &self.top_levels,
                            &mut self.diagnostics,
                        ),
                        return_type: resolve_result_type(
                            &function.return_type,
                            &self.top_levels,
                            &mut self.diagnostics,
                        ),
                        linkage: ResolvedFunctionLinkage::External {
                            symbol: function.name.text.clone(),
                        },
                        span: function.span,
                    }
                }
                syntax::TopLevelDeclaration::Class(_) => {
                    unreachable!("function work item must reference a function")
                }
            })
            .collect()
    }

    fn collect_class_declarations(
        &mut self,
    ) -> (
        Vec<ResolvedClassDeclaration>,
        Vec<ClassSymbols>,
        Vec<ClassWorkItem>,
    ) {
        let work = self.class_work.clone();
        let mut declarations = Vec::with_capacity(work.len());
        let mut symbols = Vec::with_capacity(work.len());
        let mut body_work = Vec::with_capacity(work.len());

        for (id, ast_index) in work {
            let syntax::TopLevelDeclaration::Class(class) = &self.ast.declarations[ast_index]
            else {
                unreachable!("class work item must reference a class")
            };
            let (declaration, class_symbols, item) = collect_class(
                id,
                ast_index,
                class,
                &self.top_levels,
                &mut self.diagnostics,
            );
            declarations.push(declaration);
            symbols.push(class_symbols);
            body_work.push(item);
        }

        (declarations, symbols, body_work)
    }

    fn resolve_function_bodies(
        &mut self,
        functions: &ResolvedFunctionDeclarationTable,
        classes: &ResolvedClassDeclarationTable,
        class_symbols: &[ClassSymbols],
    ) -> Vec<Option<ResolvedFunctionDefinition>> {
        let work = self.function_work.clone();
        work.into_iter()
            .map(|item| {
                let declaration = functions
                    .get(item.id)
                    .expect("function work and declaration table must agree");
                let syntax::TopLevelDeclaration::Function(function) =
                    &self.ast.declarations[item.ast_index]
                else {
                    return None;
                };
                let body = resolve_callable_body(
                    item.id.into(),
                    None,
                    &declaration.parameters,
                    &function.body,
                    BaseInitializationPolicy::Forbidden,
                    BodyResolutionEnvironment::new(&self.top_levels, classes, class_symbols),
                    &mut self.diagnostics,
                );
                Some(ResolvedFunctionDefinition {
                    function: item.id,
                    locals: body.locals,
                    body: body.body,
                    span: function.span,
                })
            })
            .collect()
    }
}

fn resolve_parameters(
    callable: CallableId,
    parameters: &[syntax::Parameter],
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedParameter> {
    let mut names = HashMap::<String, Span>::new();
    let mut resolved = Vec::with_capacity(parameters.len());
    for parameter in parameters {
        if let Some(previous_span) = names.get(&parameter.name.text) {
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
        names.insert(parameter.name.text.clone(), parameter.name.span);
        let Some(type_syntax) = resolve_type(&parameter.type_syntax, top_levels, diagnostics)
        else {
            continue;
        };
        resolved.push(ResolvedParameter {
            id: ParameterId::new(callable, resolved.len()),
            binding_mode: resolve_parameter_binding_mode(parameter.binding_mode),
            name: parameter.name.text.clone(),
            name_span: parameter.name.span,
            type_syntax,
            span: parameter.span,
        });
    }
    resolved
}

const fn resolve_parameter_binding_mode(
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

fn resolve_result_type(
    type_syntax: &syntax::TypeSyntax,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> ResolvedType {
    resolve_type(type_syntax, top_levels, diagnostics).unwrap_or(ResolvedType {
        // Resolution diagnostics stop later phases. Retaining a payload-free
        // placeholder keeps declaration collection total and panic-free.
        kind: ResolvedTypeKind::Unit,
        span: type_syntax.span,
    })
}

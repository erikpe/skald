//! Program-wide declaration collection and deterministic identity assignment.

use super::{
    body::{resolve_callable_body, BodyResolutionEnvironment},
    *,
};
use crate::{
    diagnostics::Diagnostic,
    identity::{CallableId, ParameterId},
};

#[derive(Clone, Copy)]
struct FunctionWorkItem {
    id: FunctionId,
    ast_index: usize,
}

#[derive(Clone)]
struct ClassWorkItem {
    id: ClassId,
    ast_index: usize,
    initializer_member: Option<usize>,
    method_members: Vec<usize>,
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

        let function_definitions = self.resolve_function_bodies(
            &function_declarations,
            &class_declarations,
            &class_symbols,
        );
        let class_definitions =
            self.resolve_class_bodies(&class_work, &class_declarations, &class_symbols);
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
                    return_type: resolve_scalar_type(&function.return_type),
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
                        return_type: resolve_scalar_type(&function.return_type),
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
            let (declaration, class_symbols, item) = self.collect_class(id, ast_index, class);
            declarations.push(declaration);
            symbols.push(class_symbols);
            body_work.push(item);
        }

        (declarations, symbols, body_work)
    }

    fn collect_class(
        &mut self,
        id: ClassId,
        ast_index: usize,
        class: &syntax::ClassDecl,
    ) -> (ResolvedClassDeclaration, ClassSymbols, ClassWorkItem) {
        let mut fields = Vec::new();
        let mut initializer = None;
        let mut methods = Vec::new();
        let mut symbols = ClassSymbols::default();
        let mut initializer_member = None;
        let mut method_members = Vec::new();

        for (member_index, member) in class.members.iter().enumerate() {
            match member {
                syntax::ClassMember::Field(field) => {
                    if !declare_ordinary_member(
                        &mut symbols,
                        &field.name,
                        OrdinaryMemberSymbolKind::Field(FieldId::new(id, fields.len())),
                        &mut self.diagnostics,
                    ) {
                        continue;
                    }
                    let field_id = FieldId::new(id, fields.len());
                    fields.push(ResolvedFieldDeclaration {
                        id: field_id,
                        name: field.name.text.clone(),
                        name_span: field.name.span,
                        type_syntax: resolve_scalar_type(&field.type_syntax),
                        span: field.span,
                    });
                }
                syntax::ClassMember::Initializer(source) => {
                    if let Some(previous_span) = symbols.initializer_span {
                        self.diagnostics.push(
                            Diagnostic::error(
                                DUPLICATE_MEMBER,
                                format!("duplicate initializer in class `{}`", class.name.text),
                            )
                            .with_primary_label(source.introducer_span, "redeclared here")
                            .with_secondary_label(previous_span, "first declared here"),
                        );
                        continue;
                    }
                    let initializer_id = InitializerId::new(id, 0);
                    symbols.initializer = Some(initializer_id);
                    symbols.initializer_span = Some(source.introducer_span);
                    initializer = Some(ResolvedInitializerDeclaration {
                        id: initializer_id,
                        parameters: resolve_parameters(
                            initializer_id.into(),
                            &source.parameters,
                            &self.top_levels,
                            &mut self.diagnostics,
                        ),
                        span: source.span,
                    });
                    initializer_member = Some(member_index);
                }
                syntax::ClassMember::Method(method) => {
                    let method_id = MethodId::new(id, methods.len());
                    if !declare_ordinary_member(
                        &mut symbols,
                        &method.name,
                        OrdinaryMemberSymbolKind::Method(method_id),
                        &mut self.diagnostics,
                    ) {
                        continue;
                    }
                    methods.push(ResolvedMethodDeclaration {
                        id: method_id,
                        name: method.name.text.clone(),
                        name_span: method.name.span,
                        receiver_access: if method.mut_span.is_some() {
                            ResolvedReceiverAccess::Mutable
                        } else {
                            ResolvedReceiverAccess::ReadOnly
                        },
                        parameters: resolve_parameters(
                            method_id.into(),
                            &method.parameters,
                            &self.top_levels,
                            &mut self.diagnostics,
                        ),
                        return_type: resolve_scalar_type(&method.return_type),
                        span: method.span,
                    });
                    method_members.push(member_index);
                }
            }
        }

        (
            ResolvedClassDeclaration {
                id,
                name: class.name.text.clone(),
                name_span: class.name.span,
                fields,
                initializer,
                methods,
                span: class.span,
            },
            symbols,
            ClassWorkItem {
                id,
                ast_index,
                initializer_member,
                method_members,
            },
        )
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

    fn resolve_class_bodies(
        &mut self,
        work: &[ClassWorkItem],
        classes: &ResolvedClassDeclarationTable,
        class_symbols: &[ClassSymbols],
    ) -> Vec<ResolvedClassDefinition> {
        work.iter()
            .map(|item| {
                let declaration = classes
                    .get(item.id)
                    .expect("class work and declaration table must agree");
                let syntax::TopLevelDeclaration::Class(class) =
                    &self.ast.declarations[item.ast_index]
                else {
                    unreachable!("class work item must reference a class")
                };

                let initializer = item.initializer_member.map(|member_index| {
                    let syntax::ClassMember::Initializer(source) = &class.members[member_index]
                    else {
                        unreachable!("initializer work must reference an initializer")
                    };
                    let metadata = declaration
                        .initializer
                        .as_ref()
                        .expect("accepted initializer must have declaration metadata");
                    let body = resolve_callable_body(
                        metadata.id.into(),
                        Some(item.id),
                        &metadata.parameters,
                        &source.body,
                        BodyResolutionEnvironment::new(&self.top_levels, classes, class_symbols),
                        &mut self.diagnostics,
                    );
                    ResolvedMemberDefinition {
                        callable: metadata.id.into(),
                        locals: body.locals,
                        body: body.body,
                        span: source.span,
                    }
                });

                let methods = item
                    .method_members
                    .iter()
                    .enumerate()
                    .map(|(method_index, member_index)| {
                        let syntax::ClassMember::Method(source) = &class.members[*member_index]
                        else {
                            unreachable!("method work must reference a method")
                        };
                        let metadata = &declaration.methods[method_index];
                        let body = resolve_callable_body(
                            metadata.id.into(),
                            Some(item.id),
                            &metadata.parameters,
                            &source.body,
                            BodyResolutionEnvironment::new(
                                &self.top_levels,
                                classes,
                                class_symbols,
                            ),
                            &mut self.diagnostics,
                        );
                        ResolvedMemberDefinition {
                            callable: metadata.id.into(),
                            locals: body.locals,
                            body: body.body,
                            span: source.span,
                        }
                    })
                    .collect();

                ResolvedClassDefinition {
                    class: item.id,
                    initializer,
                    methods,
                    span: class.span,
                }
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
        let type_syntax = match parameter.binding_mode {
            syntax::ParameterBindingMode::Value => resolve_scalar_type(&parameter.type_syntax),
            syntax::ParameterBindingMode::ReadOnlyAlias { .. }
            | syntax::ParameterBindingMode::MutableAlias { .. } => {
                let Some(ty) = resolve_type(&parameter.type_syntax, top_levels, diagnostics) else {
                    continue;
                };
                ty
            }
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

fn resolve_scalar_type(type_syntax: &syntax::TypeSyntax) -> ResolvedType {
    let kind = match &type_syntax.kind {
        syntax::TypeKind::I64 => ResolvedTypeKind::I64,
        syntax::TypeKind::U64 => ResolvedTypeKind::U64,
        syntax::TypeKind::U8 => ResolvedTypeKind::U8,
        syntax::TypeKind::F64 => ResolvedTypeKind::F64,
        syntax::TypeKind::Bool => ResolvedTypeKind::Bool,
        syntax::TypeKind::Unit => ResolvedTypeKind::Unit,
        syntax::TypeKind::Named(_) => {
            unreachable!("named types are admitted only for locals in the OBJ profile")
        }
    };
    ResolvedType {
        kind,
        span: type_syntax.span,
    }
}

fn declare_ordinary_member(
    symbols: &mut ClassSymbols,
    name: &syntax::Name,
    kind: OrdinaryMemberSymbolKind,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some(previous) = symbols.ordinary.get(&name.text) {
        diagnostics.push(
            Diagnostic::error(
                DUPLICATE_MEMBER,
                format!("duplicate class member `{}`", name.text),
            )
            .with_primary_label(name.span, "redeclared here")
            .with_secondary_label(previous.name_span, "first declared here"),
        );
        return false;
    }
    symbols.ordinary.insert(
        name.text.clone(),
        OrdinaryMemberSymbol {
            kind,
            name_span: name.span,
        },
    );
    true
}

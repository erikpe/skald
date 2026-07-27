//! Source-ordered class declaration collection.

use super::*;

struct LifecycleDeclarations {
    initializers: Vec<ResolvedInitializerDeclaration>,
    copy_constructor: Option<ResolvedCopyConstructorDeclaration>,
    copy_assignment: Option<ResolvedCopyAssignmentDeclaration>,
    destructor: Option<ResolvedDestructorDeclaration>,
    copy_constructor_invalid: bool,
    copy_assignment_invalid: bool,
}

impl LifecycleDeclarations {
    fn new() -> Self {
        Self {
            initializers: Vec::new(),
            copy_constructor: None,
            copy_assignment: None,
            destructor: None,
            copy_constructor_invalid: false,
            copy_assignment_invalid: false,
        }
    }
}

struct ClassCollectionState {
    id: ClassId,
    direct_base: Option<ResolvedDirectBase>,
    fields: Vec<ResolvedFieldDeclaration>,
    methods: Vec<ResolvedMethodDeclaration>,
    lifecycle: LifecycleDeclarations,
    symbols: ClassSymbols,
    work: ClassWorkItem,
}

impl ClassCollectionState {
    fn new(id: ClassId, ast_index: usize, direct_base: Option<ResolvedDirectBase>) -> Self {
        Self {
            id,
            direct_base,
            fields: Vec::new(),
            methods: Vec::new(),
            lifecycle: LifecycleDeclarations::new(),
            symbols: ClassSymbols::default(),
            work: ClassWorkItem {
                id,
                ast_index,
                initializer_members: Vec::new(),
                copy_constructor_member: None,
                copy_assignment_member: None,
                destructor_member: None,
                method_members: Vec::new(),
            },
        }
    }

    fn collect_field(
        &mut self,
        field: &syntax::FieldDecl,
        top_levels: &HashMap<String, TopLevelSymbol>,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) {
        let Some(type_syntax) =
            resolve_type(&field.type_syntax, top_levels, array_types, diagnostics)
        else {
            return;
        };
        let field_id = FieldId::new(self.id, self.fields.len());
        if !declare_ordinary_member(
            &mut self.symbols,
            &field.name,
            OrdinaryMemberSymbolKind::Field(field_id),
            diagnostics,
        ) {
            return;
        }
        self.fields.push(ResolvedFieldDeclaration {
            id: field_id,
            name: field.name.text.to_string(),
            name_span: field.name.span,
            type_syntax,
            span: field.span,
        });
    }

    fn collect_initializer(
        &mut self,
        member_index: usize,
        source: &syntax::InitializerDecl,
        top_levels: &HashMap<String, TopLevelSymbol>,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) {
        let id = InitializerId::new(self.id, self.lifecycle.initializers.len());
        let declaration = ResolvedInitializerDeclaration {
            id,
            parameters: resolve_parameters(
                id.into(),
                &source.parameters,
                top_levels,
                array_types,
                diagnostics,
            ),
            span: source.span,
        };
        if declaration.parameters.len() == source.parameters.len() {
            if let Some(previous) = self
                .lifecycle
                .initializers
                .iter()
                .find(|previous| same_parameter_types(previous, &declaration))
            {
                diagnostics.push(
                    Diagnostic::error(
                        DUPLICATE_MEMBER,
                        "duplicate ordinary initializer signature",
                    )
                    .with_primary_label(source.introducer_span, "redeclared here")
                    .with_secondary_label(previous.span, "first declared here")
                    .with_note(
                        "parameter names and binding modes do not distinguish initializer overloads",
                    ),
                );
                return;
            }
        }
        self.lifecycle.initializers.push(declaration);
        self.work
            .initializer_members
            .push(InitializerWorkItem { id, member_index });
    }

    fn collect_copy_constructor(
        &mut self,
        member_index: usize,
        source: &syntax::CopyConstructorDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) {
        if report_duplicate_lifecycle(
            self.symbols.copy_constructor_span,
            source.introducer_span,
            "copy constructor",
            class_name,
            diagnostics,
        ) {
            return;
        }
        self.symbols.copy_constructor_span = Some(source.introducer_span);
        let id = CopyConstructorId::new(self.id, 0);
        let Some(parameter) = resolve_copy_source_parameter(
            CopySourceContext {
                callable: id.into(),
                owner: self.id,
                declaration_span: source.span,
                operation: CopyLifecycleKind::Constructor,
            },
            &source.parameters,
            top_levels,
            array_types,
            diagnostics,
        ) else {
            self.lifecycle.copy_constructor_invalid = true;
            return;
        };
        let declaration = ResolvedCopyConstructorDeclaration {
            id,
            parameters: vec![parameter],
            span: source.span,
        };
        self.lifecycle.copy_constructor = Some(declaration);
        self.work.copy_constructor_member = Some(member_index);
    }

    fn collect_copy_assignment(
        &mut self,
        member_index: usize,
        source: &syntax::CopyAssignmentDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) {
        if report_duplicate_lifecycle(
            self.symbols.copy_assignment_span,
            source.introducer_span,
            "copy assignment",
            class_name,
            diagnostics,
        ) {
            return;
        }
        self.symbols.copy_assignment_span = Some(source.introducer_span);
        let id = CopyAssignmentId::new(self.id, 0);
        let Some(parameter) = resolve_copy_source_parameter(
            CopySourceContext {
                callable: id.into(),
                owner: self.id,
                declaration_span: source.span,
                operation: CopyLifecycleKind::Assignment,
            },
            &source.parameters,
            top_levels,
            array_types,
            diagnostics,
        ) else {
            self.lifecycle.copy_assignment_invalid = true;
            return;
        };
        self.lifecycle.copy_assignment = Some(ResolvedCopyAssignmentDeclaration {
            id,
            parameter,
            span: source.span,
        });
        self.work.copy_assignment_member = Some(member_index);
    }

    fn collect_destructor(
        &mut self,
        member_index: usize,
        source: &syntax::DestructorDecl,
        class_name: &str,
        diagnostics: &mut Diagnostics,
    ) {
        if report_duplicate_lifecycle(
            self.symbols.destructor_span,
            source.introducer_span,
            "destructor",
            class_name,
            diagnostics,
        ) {
            return;
        }
        let id = DestructorId::new(self.id, 0);
        self.symbols.destructor_span = Some(source.introducer_span);
        self.lifecycle.destructor = Some(ResolvedDestructorDeclaration {
            id,
            span: source.span,
        });
        self.work.destructor_member = Some(member_index);
    }

    fn collect_method(
        &mut self,
        member_index: usize,
        method: &syntax::MethodDecl,
        top_levels: &HashMap<String, TopLevelSymbol>,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) {
        let id = MethodId::new(self.id, self.methods.len());
        if !declare_ordinary_member(
            &mut self.symbols,
            &method.name,
            OrdinaryMemberSymbolKind::Method(id),
            diagnostics,
        ) {
            return;
        }
        self.methods.push(ResolvedMethodDeclaration {
            id,
            name: method.name.text.to_string(),
            name_span: method.name.span,
            receiver_access: if method.mut_span.is_some() {
                ResolvedReceiverAccess::Mutable
            } else {
                ResolvedReceiverAccess::ReadOnly
            },
            modifier: match method.modifier {
                None => ResolvedMethodModifier::Direct,
                Some(syntax::MethodModifier::Virtual { span }) => {
                    ResolvedMethodModifier::Virtual { span }
                }
                Some(syntax::MethodModifier::Override { span }) => {
                    ResolvedMethodModifier::Override { span }
                }
            },
            dispatch: ResolvedMethodDispatch::Direct,
            parameters: resolve_parameters(
                id.into(),
                &method.parameters,
                top_levels,
                array_types,
                diagnostics,
            ),
            return_type: resolve_result_type(
                &method.return_type,
                top_levels,
                array_types,
                diagnostics,
            ),
            span: method.span,
        });
        self.work.method_members.push(member_index);
    }

    fn finish(
        self,
        module: ModuleId,
        class: &syntax::ClassDecl,
    ) -> (ResolvedClassDeclaration, ClassSymbols, ClassWorkItem) {
        let copy_constructor = if self.lifecycle.copy_constructor_invalid {
            ResolvedCopyOperation::Unavailable
        } else {
            self.lifecycle
                .copy_constructor
                .as_ref()
                .map_or(ResolvedCopyOperation::Synthesized(self.id), |declaration| {
                    ResolvedCopyOperation::User(declaration.id)
                })
        };
        let copy_assignment = if self.lifecycle.copy_assignment_invalid {
            ResolvedCopyOperation::Unavailable
        } else {
            self.lifecycle
                .copy_assignment
                .as_ref()
                .map_or(ResolvedCopyOperation::Synthesized(self.id), |declaration| {
                    ResolvedCopyOperation::User(declaration.id)
                })
        };
        (
            ResolvedClassDeclaration {
                id: self.id,
                module,
                name: class.name.text.to_string(),
                name_span: class.name.span,
                direct_base: self.direct_base,
                implemented_interfaces: Vec::new(),
                fields: self.fields,
                initializers: self.lifecycle.initializers,
                copy_constructor,
                copy_constructor_declaration: self.lifecycle.copy_constructor,
                copy_assignment,
                copy_assignment_declaration: self.lifecycle.copy_assignment,
                destructor: self.lifecycle.destructor,
                methods: self.methods,
                span: class.span,
            },
            self.symbols,
            self.work,
        )
    }
}

pub(super) fn collect_class(
    id: ClassId,
    module: ModuleId,
    ast_index: usize,
    class: &syntax::ClassDecl,
    top_levels: &HashMap<String, TopLevelSymbol>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> (ResolvedClassDeclaration, ClassSymbols, ClassWorkItem) {
    let direct_base = resolve_direct_base(id, class, top_levels, diagnostics);
    let mut state = ClassCollectionState::new(id, ast_index, direct_base);
    for (member_index, member) in class.members.iter().enumerate() {
        match member {
            syntax::ClassMember::Field(field) => {
                state.collect_field(field, top_levels, array_types, diagnostics)
            }
            syntax::ClassMember::Initializer(initializer) => state.collect_initializer(
                member_index,
                initializer,
                top_levels,
                array_types,
                diagnostics,
            ),
            syntax::ClassMember::CopyConstructor(constructor) => state.collect_copy_constructor(
                member_index,
                constructor,
                &class.name.text,
                top_levels,
                array_types,
                diagnostics,
            ),
            syntax::ClassMember::CopyAssignment(assignment) => state.collect_copy_assignment(
                member_index,
                assignment,
                &class.name.text,
                top_levels,
                array_types,
                diagnostics,
            ),
            syntax::ClassMember::Destructor(destructor) => {
                state.collect_destructor(member_index, destructor, &class.name.text, diagnostics)
            }
            syntax::ClassMember::Method(method) => {
                state.collect_method(member_index, method, top_levels, array_types, diagnostics)
            }
        }
    }
    state.finish(module, class)
}

fn same_parameter_types(
    left: &ResolvedInitializerDeclaration,
    right: &ResolvedInitializerDeclaration,
) -> bool {
    left.parameters.len() == right.parameters.len()
        && left
            .parameters
            .iter()
            .zip(&right.parameters)
            .all(|(left, right)| left.type_syntax.kind == right.type_syntax.kind)
}

fn resolve_direct_base(
    owner: ClassId,
    class: &syntax::ClassDecl,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedDirectBase> {
    let base = class.direct_base.as_ref()?;
    if reject_qualified_name(base, diagnostics) {
        return None;
    }
    match top_levels.get(base.text.as_str()) {
        Some(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(base_id),
            ..
        }) if *base_id == owner => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_BASE_CLASS,
                    format!("class `{}` cannot extend itself", class.name.text),
                )
                .with_primary_label(base.span, "this resolves to the enclosing class")
                .with_secondary_label(class.name.span, "class declared here"),
            );
            None
        }
        Some(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(base_id),
            ..
        }) => Some(ResolvedDirectBase {
            class: *base_id,
            span: base.span,
        }),
        Some(symbol) => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_BASE_CLASS,
                    format!("`{}` does not name a base class", base.text),
                )
                .with_primary_label(base.span, "expected a class name")
                .with_secondary_label(symbol.name_span, "function declared here"),
            );
            None
        }
        None => {
            diagnostics.push(
                Diagnostic::error(
                    INVALID_BASE_CLASS,
                    format!("unknown base class `{}`", base.text),
                )
                .with_primary_label(base.span, "no class with this name is declared"),
            );
            None
        }
    }
}

#[derive(Clone)]
pub(super) struct ClassWorkItem {
    pub(super) id: ClassId,
    pub(super) ast_index: usize,
    pub(super) initializer_members: Vec<InitializerWorkItem>,
    pub(super) copy_constructor_member: Option<usize>,
    pub(super) copy_assignment_member: Option<usize>,
    pub(super) destructor_member: Option<usize>,
    pub(super) method_members: Vec<usize>,
}

#[derive(Clone, Copy)]
pub(super) struct InitializerWorkItem {
    pub(super) id: InitializerId,
    pub(super) member_index: usize,
}

#[derive(Clone, Copy)]
enum CopyLifecycleKind {
    Constructor,
    Assignment,
}

#[derive(Clone, Copy)]
struct CopySourceContext {
    callable: CallableId,
    owner: ClassId,
    declaration_span: Span,
    operation: CopyLifecycleKind,
}

impl CopyLifecycleKind {
    const fn description(self) -> &'static str {
        match self {
            Self::Constructor => "copy constructor",
            Self::Assignment => "copy assignment",
        }
    }

    const fn introducer(self) -> &'static str {
        match self {
            Self::Constructor => "copy",
            Self::Assignment => "assign",
        }
    }
}

fn resolve_copy_source_parameter(
    context: CopySourceContext,
    parameters: &[syntax::Parameter],
    top_levels: &HashMap<String, TopLevelSymbol>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedParameter> {
    let description = context.operation.description();
    let [parameter] = parameters else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_LIFECYCLE_SIGNATURE,
                format!("{description} requires exactly one source parameter"),
            )
            .with_primary_label(
                context.declaration_span,
                format!(
                    "use `{}(ref name: EnclosingClass) {{ ... }}`",
                    context.operation.introducer()
                ),
            ),
        );
        return None;
    };

    if !matches!(
        parameter.binding_mode,
        syntax::ParameterBindingMode::ReadOnlyAlias { .. }
    ) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_LIFECYCLE_SIGNATURE,
                format!("{description} source must be a read-only alias"),
            )
            .with_primary_label(parameter.span, "use `ref name: EnclosingClass`"),
        );
        return None;
    }

    let ty = resolve_type(&parameter.type_syntax, top_levels, array_types, diagnostics)?;
    if ty.kind != ResolvedTypeKind::Class(context.owner) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_LIFECYCLE_SIGNATURE,
                format!("{description} source must have the exact enclosing class type"),
            )
            .with_primary_label(parameter.type_syntax.span, "expected the enclosing class"),
        );
        return None;
    }

    Some(ResolvedParameter {
        id: ParameterId::new(context.callable, 0),
        binding_mode: resolve_parameter_binding_mode(parameter.binding_mode),
        name: parameter.name.text.to_string(),
        name_span: parameter.name.span,
        type_syntax: ty,
        span: parameter.span,
    })
}

fn report_duplicate_lifecycle(
    previous_span: Option<Span>,
    current_span: Span,
    kind: &str,
    class_name: &str,
    diagnostics: &mut Diagnostics,
) -> bool {
    let Some(previous_span) = previous_span else {
        return false;
    };
    diagnostics.push(
        Diagnostic::error(
            DUPLICATE_MEMBER,
            format!("duplicate {kind} in class `{class_name}`"),
        )
        .with_primary_label(current_span, "redeclared here")
        .with_secondary_label(previous_span, "first declared here"),
    );
    true
}

fn declare_ordinary_member(
    symbols: &mut ClassSymbols,
    name: &syntax::Name,
    kind: OrdinaryMemberSymbolKind,
    diagnostics: &mut Diagnostics,
) -> bool {
    if let Some(previous) = symbols.ordinary.get(name.text.as_str()) {
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
        name.text.to_string(),
        OrdinaryMemberSymbol {
            kind,
            name_span: name.span,
        },
    );
    true
}

#[cfg(test)]
mod tests;

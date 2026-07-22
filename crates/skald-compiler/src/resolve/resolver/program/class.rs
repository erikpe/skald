//! Source-ordered class declaration collection.

use super::*;

struct LifecycleDeclarations {
    initializer: Option<ResolvedInitializerDeclaration>,
    copy_constructor: Option<ResolvedInitializerDeclaration>,
    copy_assignment: Option<ResolvedCopyAssignmentDeclaration>,
    destructor: Option<ResolvedDestructorDeclaration>,
    next_initializer_index: usize,
    copy_assignment_invalid: bool,
}

impl LifecycleDeclarations {
    fn new() -> Self {
        Self {
            initializer: None,
            copy_constructor: None,
            copy_assignment: None,
            destructor: None,
            next_initializer_index: 0,
            copy_assignment_invalid: false,
        }
    }
}

struct ClassCollectionState {
    id: ClassId,
    fields: Vec<ResolvedFieldDeclaration>,
    methods: Vec<ResolvedMethodDeclaration>,
    lifecycle: LifecycleDeclarations,
    symbols: ClassSymbols,
    work: ClassWorkItem,
}

impl ClassCollectionState {
    fn new(id: ClassId, ast_index: usize) -> Self {
        Self {
            id,
            fields: Vec::new(),
            methods: Vec::new(),
            lifecycle: LifecycleDeclarations::new(),
            symbols: ClassSymbols::default(),
            work: ClassWorkItem {
                id,
                ast_index,
                initializer_member: None,
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
        diagnostics: &mut Diagnostics,
    ) {
        let Some(type_syntax) = resolve_type(&field.type_syntax, top_levels, diagnostics) else {
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
            name: field.name.text.clone(),
            name_span: field.name.span,
            type_syntax,
            span: field.span,
        });
    }

    fn collect_initializer(
        &mut self,
        member_index: usize,
        source: &syntax::InitializerDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
        diagnostics: &mut Diagnostics,
    ) {
        if is_copy_constructor(source, self.id, top_levels) {
            self.collect_copy_constructor(
                member_index,
                source,
                class_name,
                top_levels,
                diagnostics,
            );
        } else {
            self.collect_ordinary_initializer(
                member_index,
                source,
                class_name,
                top_levels,
                diagnostics,
            );
        }
    }

    fn collect_ordinary_initializer(
        &mut self,
        member_index: usize,
        source: &syntax::InitializerDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
        diagnostics: &mut Diagnostics,
    ) {
        if report_duplicate_lifecycle(
            self.symbols.initializer_span,
            source.introducer_span,
            "ordinary initializer",
            class_name,
            diagnostics,
        ) {
            return;
        }
        let declaration = self.resolve_initializer(source, top_levels, diagnostics);
        self.symbols.initializer = Some(declaration.id);
        self.symbols.initializer_span = Some(source.introducer_span);
        self.lifecycle.initializer = Some(declaration);
        self.work.initializer_member = Some(member_index);
    }

    fn collect_copy_constructor(
        &mut self,
        member_index: usize,
        source: &syntax::InitializerDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
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
        let declaration = self.resolve_initializer(source, top_levels, diagnostics);
        self.symbols.copy_constructor_span = Some(source.introducer_span);
        self.lifecycle.copy_constructor = Some(declaration);
        self.work.copy_constructor_member = Some(member_index);
    }

    fn resolve_initializer(
        &mut self,
        source: &syntax::InitializerDecl,
        top_levels: &HashMap<String, TopLevelSymbol>,
        diagnostics: &mut Diagnostics,
    ) -> ResolvedInitializerDeclaration {
        let id = InitializerId::new(self.id, self.lifecycle.next_initializer_index);
        self.lifecycle.next_initializer_index += 1;
        ResolvedInitializerDeclaration {
            id,
            parameters: resolve_parameters(id.into(), &source.parameters, top_levels, diagnostics),
            span: source.span,
        }
    }

    fn collect_copy_assignment(
        &mut self,
        member_index: usize,
        source: &syntax::CopyAssignmentDecl,
        class_name: &str,
        top_levels: &HashMap<String, TopLevelSymbol>,
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
        let Some(parameter) =
            resolve_copy_assignment_parameter(id, self.id, source, top_levels, diagnostics)
        else {
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
            name: method.name.text.clone(),
            name_span: method.name.span,
            receiver_access: if method.mut_span.is_some() {
                ResolvedReceiverAccess::Mutable
            } else {
                ResolvedReceiverAccess::ReadOnly
            },
            parameters: resolve_parameters(id.into(), &method.parameters, top_levels, diagnostics),
            return_type: resolve_result_type(&method.return_type, top_levels, diagnostics),
            span: method.span,
        });
        self.work.method_members.push(member_index);
    }

    fn finish(
        self,
        class: &syntax::ClassDecl,
    ) -> (ResolvedClassDeclaration, ClassSymbols, ClassWorkItem) {
        let copy_constructor = self
            .lifecycle
            .copy_constructor
            .as_ref()
            .map_or(ResolvedCopyOperation::Synthesized(self.id), |declaration| {
                ResolvedCopyOperation::User(declaration.id)
            });
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
                name: class.name.text.clone(),
                name_span: class.name.span,
                fields: self.fields,
                initializer: self.lifecycle.initializer,
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
    ast_index: usize,
    class: &syntax::ClassDecl,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> (ResolvedClassDeclaration, ClassSymbols, ClassWorkItem) {
    let mut state = ClassCollectionState::new(id, ast_index);
    for (member_index, member) in class.members.iter().enumerate() {
        match member {
            syntax::ClassMember::Field(field) => {
                state.collect_field(field, top_levels, diagnostics)
            }
            syntax::ClassMember::Initializer(initializer) => state.collect_initializer(
                member_index,
                initializer,
                &class.name.text,
                top_levels,
                diagnostics,
            ),
            syntax::ClassMember::CopyAssignment(assignment) => state.collect_copy_assignment(
                member_index,
                assignment,
                &class.name.text,
                top_levels,
                diagnostics,
            ),
            syntax::ClassMember::Destructor(destructor) => {
                state.collect_destructor(member_index, destructor, &class.name.text, diagnostics)
            }
            syntax::ClassMember::Method(method) => {
                state.collect_method(member_index, method, top_levels, diagnostics)
            }
        }
    }
    state.finish(class)
}

#[derive(Clone)]
pub(super) struct ClassWorkItem {
    pub(super) id: ClassId,
    pub(super) ast_index: usize,
    pub(super) initializer_member: Option<usize>,
    pub(super) copy_constructor_member: Option<usize>,
    pub(super) copy_assignment_member: Option<usize>,
    pub(super) destructor_member: Option<usize>,
    pub(super) method_members: Vec<usize>,
}

fn is_copy_constructor(
    initializer: &syntax::InitializerDecl,
    owner: ClassId,
    top_levels: &HashMap<String, TopLevelSymbol>,
) -> bool {
    let [parameter] = initializer.parameters.as_slice() else {
        return false;
    };
    if !matches!(
        parameter.binding_mode,
        syntax::ParameterBindingMode::ReadOnlyAlias { .. }
    ) {
        return false;
    }
    let syntax::TypeKind::Named(name) = &parameter.type_syntax.kind else {
        return false;
    };
    matches!(
        top_levels.get(&name.text),
        Some(TopLevelSymbol {
            kind: TopLevelSymbolKind::Class(class),
            ..
        }) if *class == owner
    )
}

fn resolve_copy_assignment_parameter(
    callable: CopyAssignmentId,
    owner: ClassId,
    assignment: &syntax::CopyAssignmentDecl,
    top_levels: &HashMap<String, TopLevelSymbol>,
    diagnostics: &mut Diagnostics,
) -> Option<ResolvedParameter> {
    let [parameter] = assignment.parameters.as_slice() else {
        diagnostics.push(
            Diagnostic::error(
                INVALID_LIFECYCLE_SIGNATURE,
                "copy assignment requires exactly one source parameter",
            )
            .with_primary_label(
                assignment.span,
                "use `assign(ref name: EnclosingClass) { ... }`",
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
                "copy-assignment source must be a read-only alias",
            )
            .with_primary_label(parameter.span, "use `ref name: EnclosingClass`"),
        );
        return None;
    }

    let ty = resolve_type(&parameter.type_syntax, top_levels, diagnostics)?;
    if ty.kind != ResolvedTypeKind::Class(owner) {
        diagnostics.push(
            Diagnostic::error(
                INVALID_LIFECYCLE_SIGNATURE,
                "copy-assignment source must have the exact enclosing class type",
            )
            .with_primary_label(parameter.type_syntax.span, "expected the enclosing class"),
        );
        return None;
    }

    Some(ResolvedParameter {
        id: ParameterId::new(callable, 0),
        binding_mode: resolve_parameter_binding_mode(parameter.binding_mode),
        name: parameter.name.text.clone(),
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

#[cfg(test)]
mod tests;

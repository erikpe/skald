//! Substitution of complete template headers into ordinary class declarations.

use super::super::resolver::ModuleUnit;
use super::*;

pub(crate) struct SpecializedDeclarations {
    pub(crate) declarations: Vec<ResolvedClassDeclaration>,
    pub(crate) symbols: Vec<ClassSymbols>,
    pub(crate) valid: bool,
}

pub(crate) fn specialize_declarations(
    units: &[ModuleUnit<'_>],
    semantics: &ResolvedClassTemplateSemanticTable,
    specializations: &GenericSpecializationTable,
    diagnostics: &mut Diagnostics,
) -> SpecializedDeclarations {
    let mut output = SpecializedDeclarations {
        declarations: Vec::new(),
        symbols: Vec::new(),
        valid: true,
    };

    for specialization in specializations.iter() {
        let GenericSpecializationState::Complete(class_id) = specialization.state else {
            output.valid = false;
            continue;
        };
        let Some((unit, source, _)) = template_source(units, specialization.key.template) else {
            unreachable!("specialization keys reference collected templates")
        };
        let semantic = semantics
            .get(specialization.key.template)
            .expect("specialization keys reference resolved template semantics");
        match DeclarationSpecializer::new(
            class_id,
            unit.module,
            source,
            semantic,
            specialization,
            diagnostics,
        )
        .specialize()
        {
            Some((declaration, symbols)) => {
                output.declarations.push(declaration);
                output.symbols.push(symbols);
            }
            None => output.valid = false,
        }
    }

    if !output.valid {
        output.declarations.clear();
        output.symbols.clear();
    }
    output
}

struct DeclarationSpecializer<'source, 'semantic, 'specialization, 'diagnostics> {
    class_id: ClassId,
    module: ModuleId,
    source: &'source syntax::ClassDecl,
    semantic: &'semantic ResolvedClassTemplateSemantics,
    specialization: &'specialization GenericSpecialization,
    diagnostics: &'diagnostics mut Diagnostics,
    symbols: ClassSymbols,
    fields: Vec<ResolvedFieldDeclaration>,
    static_fields: Vec<ResolvedStaticFieldDeclaration>,
    initializers: Vec<ResolvedInitializerDeclaration>,
    copy_constructor: Option<ResolvedCopyConstructorDeclaration>,
    copy_assignment: Option<ResolvedCopyAssignmentDeclaration>,
    destructor: Option<ResolvedDestructorDeclaration>,
    methods: Vec<ResolvedMethodDeclaration>,
    valid: bool,
}

impl<'source, 'semantic, 'specialization, 'diagnostics>
    DeclarationSpecializer<'source, 'semantic, 'specialization, 'diagnostics>
{
    fn new(
        class_id: ClassId,
        module: ModuleId,
        source: &'source syntax::ClassDecl,
        semantic: &'semantic ResolvedClassTemplateSemantics,
        specialization: &'specialization GenericSpecialization,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            class_id,
            module,
            source,
            semantic,
            specialization,
            diagnostics,
            symbols: ClassSymbols::default(),
            fields: Vec::new(),
            static_fields: Vec::new(),
            initializers: Vec::new(),
            copy_constructor: None,
            copy_assignment: None,
            destructor: None,
            methods: Vec::new(),
            valid: true,
        }
    }

    fn specialize(mut self) -> Option<(ResolvedClassDeclaration, ClassSymbols)> {
        let direct_base = self.direct_base();
        for (member, source) in self.source.members.iter().enumerate() {
            match source {
                syntax::ClassMember::Field(field) => self.field(member, field),
                syntax::ClassMember::StaticField(field) => self.static_field(member, field),
                syntax::ClassMember::Initializer(initializer) => {
                    self.initializer(member, initializer)
                }
                syntax::ClassMember::CopyConstructor(copy) => self.copy_constructor(member, copy),
                syntax::ClassMember::CopyAssignment(copy) => self.copy_assignment(member, copy),
                syntax::ClassMember::Destructor(destructor) => self.destructor(destructor),
                syntax::ClassMember::Method(method) => self.method(member, method),
            }
        }

        self.valid.then(|| {
            let copy_constructor = self
                .copy_constructor
                .as_ref()
                .map_or(ResolvedCopyOperation::Synthesized(self.class_id), |copy| {
                    ResolvedCopyOperation::User(copy.id)
                });
            let copy_assignment = self
                .copy_assignment
                .as_ref()
                .map_or(ResolvedCopyOperation::Synthesized(self.class_id), |copy| {
                    ResolvedCopyOperation::User(copy.id)
                });
            (
                ResolvedClassDeclaration {
                    id: self.class_id,
                    module: self.module,
                    visibility: resolved_visibility(self.source.visibility),
                    name: specialized_name(self.source, &self.specialization.key.arguments),
                    name_span: self.source.name.span,
                    direct_base,
                    implemented_interfaces: self.semantic.implemented_interfaces.clone(),
                    fields: self.fields,
                    static_fields: self.static_fields,
                    initializers: self.initializers,
                    copy_constructor_declaration: self.copy_constructor,
                    copy_constructor,
                    copy_assignment_declaration: self.copy_assignment,
                    copy_assignment,
                    destructor: self.destructor,
                    methods: self.methods,
                    span: self.source.span,
                },
                self.symbols,
            )
        })
    }

    fn direct_base(&mut self) -> Option<ResolvedDirectBase> {
        self.semantic.direct_base.as_ref().and_then(|base| {
            let kind = self.closed(ResolvedTemplateTypeUseContext::DirectBase)?;
            let ResolvedTypeKind::Class(class) = kind else {
                self.fail(
                    base.span,
                    "the substituted direct base is not an exact class",
                );
                return None;
            };
            Some(ResolvedDirectBase {
                class,
                span: base.span,
            })
        })
    }

    fn field(&mut self, member: usize, source: &syntax::FieldDecl) {
        let Some(kind) = self.closed(ResolvedTemplateTypeUseContext::Field { member }) else {
            return;
        };
        let id = FieldId::new(self.class_id, self.fields.len());
        if !self.declare_member(&source.name, OrdinaryMemberSymbolKind::Field(id)) {
            return;
        }
        self.fields.push(ResolvedFieldDeclaration {
            id,
            visibility: member_visibility(source.visibility),
            name: source.name.text.to_string(),
            name_span: source.name.span,
            type_syntax: ResolvedType {
                kind,
                span: source.type_syntax.span,
            },
            span: source.span,
        });
    }

    fn static_field(&mut self, member: usize, source: &syntax::StaticFieldDecl) {
        let Some(kind) = self.closed(ResolvedTemplateTypeUseContext::StaticField { member }) else {
            return;
        };
        let id = StaticFieldId::new(self.class_id, self.static_fields.len());
        if !self.declare_member(&source.name, OrdinaryMemberSymbolKind::StaticField(id)) {
            return;
        }
        self.static_fields.push(ResolvedStaticFieldDeclaration {
            id,
            visibility: member_visibility(source.visibility),
            static_span: source.static_span,
            name: source.name.text.to_string(),
            name_span: source.name.span,
            type_syntax: ResolvedType {
                kind,
                span: source.type_syntax.span,
            },
            initializer: None,
            span: source.span,
        });
    }

    fn initializer(&mut self, member: usize, source: &syntax::InitializerDecl) {
        let id = InitializerId::new(self.class_id, self.initializers.len());
        let Some(parameters) = self.parameters(id.into(), &source.parameters, |parameter| {
            ResolvedTemplateTypeUseContext::InitializerParameter { member, parameter }
        }) else {
            return;
        };
        if let Some(previous) = self.initializers.iter().find(|previous| {
            previous.parameters.len() == parameters.len()
                && previous
                    .parameters
                    .iter()
                    .zip(&parameters)
                    .all(|(left, right)| left.type_syntax.kind == right.type_syntax.kind)
        }) {
            self.fail_with_secondary(
                source.introducer_span,
                "substitution produces a duplicate initializer signature",
                previous.span,
                "the other initializer specializes to the same parameter types",
            );
            return;
        }
        self.initializers.push(ResolvedInitializerDeclaration {
            id,
            visibility: member_visibility(source.visibility),
            parameters,
            span: source.span,
        });
    }

    fn copy_constructor(&mut self, member: usize, source: &syntax::CopyConstructorDecl) {
        if self.copy_constructor.is_some() {
            self.fail(
                source.introducer_span,
                "duplicate copy constructor after specialization",
            );
            return;
        }
        let id = CopyConstructorId::new(self.class_id, 0);
        let Some(parameters) = self.parameters(id.into(), &source.parameters, |parameter| {
            ResolvedTemplateTypeUseContext::CopyConstructorParameter { member, parameter }
        }) else {
            return;
        };
        let [parameter] = parameters.as_slice() else {
            self.fail(
                source.span,
                "copy constructor requires exactly one source parameter",
            );
            return;
        };
        if !matches!(
            parameter.binding_mode,
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
        ) || parameter.type_syntax.kind != ResolvedTypeKind::Class(self.class_id)
        {
            self.fail(
                parameter.span,
                "copy constructor source must be `ref` to this exact specialization",
            );
            return;
        }
        self.copy_constructor = Some(ResolvedCopyConstructorDeclaration {
            id,
            parameters,
            span: source.span,
        });
    }

    fn copy_assignment(&mut self, member: usize, source: &syntax::CopyAssignmentDecl) {
        if self.copy_assignment.is_some() {
            self.fail(
                source.introducer_span,
                "duplicate copy assignment after specialization",
            );
            return;
        }
        let id = CopyAssignmentId::new(self.class_id, 0);
        let Some(parameters) = self.parameters(id.into(), &source.parameters, |parameter| {
            ResolvedTemplateTypeUseContext::CopyAssignmentParameter { member, parameter }
        }) else {
            return;
        };
        let [parameter] = parameters.as_slice() else {
            self.fail(
                source.span,
                "copy assignment requires exactly one source parameter",
            );
            return;
        };
        if !matches!(
            parameter.binding_mode,
            ResolvedParameterBindingMode::ReadOnlyAlias { .. }
        ) || parameter.type_syntax.kind != ResolvedTypeKind::Class(self.class_id)
        {
            self.fail(
                parameter.span,
                "copy assignment source must be `ref` to this exact specialization",
            );
            return;
        }
        self.copy_assignment = Some(ResolvedCopyAssignmentDeclaration {
            id,
            parameter: parameter.clone(),
            span: source.span,
        });
    }

    fn destructor(&mut self, source: &syntax::DestructorDecl) {
        if self.destructor.is_some() {
            self.fail(
                source.introducer_span,
                "duplicate destructor after specialization",
            );
            return;
        }
        self.destructor = Some(ResolvedDestructorDeclaration {
            id: DestructorId::new(self.class_id, 0),
            span: source.span,
        });
    }

    fn method(&mut self, member: usize, source: &syntax::MethodDecl) {
        let id = MethodId::new(self.class_id, self.methods.len());
        if !self.declare_member(&source.name, OrdinaryMemberSymbolKind::Method(id)) {
            return;
        }
        let Some(parameters) = self.parameters(id.into(), &source.parameters, |parameter| {
            ResolvedTemplateTypeUseContext::MethodParameter { member, parameter }
        }) else {
            return;
        };
        let Some(return_kind) =
            self.closed(ResolvedTemplateTypeUseContext::MethodResult { member })
        else {
            return;
        };
        self.methods.push(ResolvedMethodDeclaration {
            id,
            visibility: member_visibility(source.visibility),
            name: source.name.text.to_string(),
            name_span: source.name.span,
            kind: method_kind(source),
            parameters,
            return_type: ResolvedType {
                kind: return_kind,
                span: source.return_type.span,
            },
            span: source.span,
        });
    }

    fn parameters(
        &mut self,
        callable: CallableId,
        source: &[syntax::Parameter],
        context: impl Fn(usize) -> ResolvedTemplateTypeUseContext,
    ) -> Option<Vec<ResolvedParameter>> {
        let mut parameters = Vec::with_capacity(source.len());
        for (parameter, declaration) in source.iter().enumerate() {
            let kind = self.closed(context(parameter))?;
            parameters.push(ResolvedParameter {
                id: ParameterId::new(callable, parameter),
                binding_mode: resolve_parameter_binding_mode(declaration.binding_mode),
                name: declaration.name.text.to_string(),
                name_span: declaration.name.span,
                type_syntax: ResolvedType {
                    kind,
                    span: declaration.type_syntax.span,
                },
                span: declaration.span,
            });
        }
        Some(parameters)
    }

    fn closed(&mut self, context: ResolvedTemplateTypeUseContext) -> Option<ResolvedTypeKind> {
        let result = self
            .semantic
            .type_uses
            .iter()
            .zip(&self.specialization.closed_type_uses)
            .find_map(|(type_use, closed)| (type_use.context == context).then_some(*closed))
            .flatten();
        if result.is_none() {
            self.valid = false;
        }
        result
    }

    fn declare_member(&mut self, name: &syntax::Name, kind: OrdinaryMemberSymbolKind) -> bool {
        if let Some(previous) = self.symbols.ordinary.get(name.text.as_str()) {
            self.fail_with_secondary(
                name.span,
                "substitution retains a duplicate ordinary member name",
                previous.name_span,
                "first declared here",
            );
            return false;
        }
        self.symbols.ordinary.insert(
            name.text.to_string(),
            OrdinaryMemberSymbol {
                kind,
                name_span: name.span,
            },
        );
        true
    }

    fn fail(&mut self, source: Span, message: &'static str) {
        self.valid = false;
        let origin = self
            .specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization has an application origin");
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
                format!("cannot specialize class `{}`", self.source.name.text),
            )
            .with_primary_label(origin.span, message)
            .with_secondary_label(source, "requirement originates here")
            .with_secondary_label(self.source.name.span, "template declared here"),
        );
    }

    fn fail_with_secondary(
        &mut self,
        source: Span,
        message: &'static str,
        secondary: Span,
        secondary_message: &'static str,
    ) {
        self.valid = false;
        let origin = self
            .specialization
            .provenance
            .origins
            .first()
            .expect("requested specialization has an application origin");
        self.diagnostics.push(
            Diagnostic::error(
                super::super::super::UNSATISFIED_GENERIC_REQUIREMENT,
                format!("cannot specialize class `{}`", self.source.name.text),
            )
            .with_primary_label(origin.span, message)
            .with_secondary_label(source, "conflicting declaration here")
            .with_secondary_label(secondary, secondary_message),
        );
    }
}

const fn member_visibility(visibility: syntax::MemberVisibility) -> ResolvedMemberVisibility {
    match visibility {
        syntax::MemberVisibility::Public => ResolvedMemberVisibility::Public,
        syntax::MemberVisibility::Private { span } => ResolvedMemberVisibility::Private { span },
    }
}

fn method_kind(method: &syntax::MethodDecl) -> ResolvedMethodKind {
    if method.static_span.is_some() {
        return ResolvedMethodKind::Static;
    }
    ResolvedMethodKind::Instance {
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
    }
}

fn specialized_name(source: &syntax::ClassDecl, arguments: &[ResolvedTypeKind]) -> String {
    let arguments = arguments
        .iter()
        .map(|argument| specialized_argument_name(*argument))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{}<{arguments}>", source.name.text)
}

fn specialized_argument_name(argument: ResolvedTypeKind) -> String {
    match argument {
        ResolvedTypeKind::I64 => "i64".to_owned(),
        ResolvedTypeKind::U64 => "u64".to_owned(),
        ResolvedTypeKind::U8 => "u8".to_owned(),
        ResolvedTypeKind::F64 => "f64".to_owned(),
        ResolvedTypeKind::Bool => "bool".to_owned(),
        ResolvedTypeKind::Unit => "unit".to_owned(),
        ResolvedTypeKind::Obj => "Obj".to_owned(),
        ResolvedTypeKind::Class(class) => class.to_string(),
        ResolvedTypeKind::Interface(interface) => interface.to_string(),
        ResolvedTypeKind::Array(array) => array.to_string(),
        ResolvedTypeKind::Shared(target) => format!("shared {}", shared_target_name(target)),
        ResolvedTypeKind::Optional(optional) => optional.to_string(),
    }
}

fn shared_target_name(target: ResolvedSharedTarget) -> String {
    match target {
        ResolvedSharedTarget::Obj => "Obj".to_owned(),
        ResolvedSharedTarget::Class(class) => class.to_string(),
        ResolvedSharedTarget::Interface(interface) => interface.to_string(),
        ResolvedSharedTarget::Array(array) => array.to_string(),
        ResolvedSharedTarget::OptionalBox(optional_box) => optional_box.to_string(),
    }
}

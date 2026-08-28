//! Per-callable context, statement checking, and structured control flow.

use std::collections::BTreeSet;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        HirAccess, HirConstruction, HirFieldAssignment, HirFieldConstruction,
        HirFunctionDefinition, HirLocal, HirMemberDefinition, HirStatement, Type,
    },
    identity::{BindingId, CallableId, ClassId, FieldId},
    resolve::{
        ResolvedBlock, ResolvedExpression, ResolvedFunctionDeclaration, ResolvedFunctionDefinition,
        ResolvedLocal, ResolvedMemberDefinition, ResolvedParameter, ResolvedProgram,
    },
};

use super::{
    capabilities::CopyCapabilities,
    expression::{
        direct_call_through_groups, is_call_through_groups, require_type, ObjectPlaceUse,
    },
    program::{
        lower_type, COPY_OPERATION_UNAVAILABLE, FIELD_INITIALIZATION, INVALID_CALL_STATEMENT,
        INVALID_CONSTRUCTION, INVALID_INITIALIZER_BODY, INVALID_OBJECT_CONTEXT, INVALID_RETURN,
        MISSING_RETURN, READ_ONLY_RECEIVER,
    },
};

mod construction;
mod copy;
mod field_write;
mod initializer;
mod iteration;
mod overload;
mod range_construction;
mod statement;

pub(super) use copy::is_ungrouped_object_call;
use statement::CheckedStatement;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MemberBodyKind {
    OrdinaryInitializer,
    CopyConstructor,
    CopyAssignment,
    MethodOrDestructor,
}

impl MemberBodyKind {
    pub(super) const fn initializes_receiver(self) -> bool {
        matches!(self, Self::OrdinaryInitializer | Self::CopyConstructor)
    }
}

#[derive(Clone, Copy)]
pub(super) struct ReceiverContext {
    pub(super) class: ClassId,
    pub(super) access: HirAccess,
}

pub(super) struct MemberCheckContext<'program> {
    pub(super) callable: CallableId,
    pub(super) owner: ClassId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) definition: &'program ResolvedMemberDefinition,
    pub(super) return_type: Type,
    pub(super) receiver: Option<ReceiverContext>,
    pub(super) body_kind: MemberBodyKind,
    pub(super) callable_name: String,
}

pub(super) struct CallableChecker<'program, 'diagnostics> {
    pub(super) program: &'program ResolvedProgram,
    pub(super) copy_capabilities: &'program CopyCapabilities,
    pub(super) callable: CallableId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) locals: &'program [ResolvedLocal],
    body: Option<&'program ResolvedBlock>,
    definition_span: crate::source::Span,
    callable_name: String,
    pub(super) return_type: Type,
    pub(super) class_owner: Option<ClassId>,
    pub(super) receiver: Option<ReceiverContext>,
    pub(super) member_body_kind: Option<MemberBodyKind>,
    pub(super) initialized_fields: BTreeSet<FieldId>,
    /// Loop-produced owning locals are immutable while their body is checked.
    pub(super) read_only_locals: BTreeSet<crate::identity::LocalId>,
    pub(super) base_initialized: bool,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
}

impl<'program, 'diagnostics> CallableChecker<'program, 'diagnostics> {
    pub(super) fn optional_kind(
        &self,
        ty: Type,
    ) -> Option<super::optional_types::OptionalPayloadKind> {
        super::optional_types::optional_id(ty)
            .and_then(|optional| super::optional_types::classify_payload(self.program, optional))
    }

    pub(super) fn optional_operand_class(
        &self,
        operand: &crate::hir::HirOptionalOperand,
    ) -> ClassId {
        match operand {
            crate::hir::HirOptionalOperand::ClassPlace(place) => place.class,
            crate::hir::HirOptionalOperand::ClassProduced(expression) => {
                let Some(super::optional_types::OptionalPayloadKind::Class(class)) =
                    self.optional_kind(expression.ty)
                else {
                    unreachable!("class optional operand must retain class metadata")
                };
                class
            }
            _ => unreachable!("expected a class optional operand"),
        }
    }

    pub(super) fn diagnostic_type_name(&self, ty: Type) -> String {
        match ty {
            Type::Optional(optional) => format!(
                "{}?",
                self.diagnostic_type_name(super::optional_types::payload_type(
                    self.program,
                    optional
                ))
            ),
            _ => ty.name().into_owned(),
        }
    }

    pub(super) fn require_exact_type(
        &mut self,
        actual: Type,
        expected: Type,
        span: crate::source::Span,
        context: &'static str,
    ) -> bool {
        if actual == expected {
            return true;
        }
        self.diagnostics.push(
            Diagnostic::error(
                super::program::TYPE_MISMATCH,
                format!(
                    "{context} has type `{}` but `{}` is required",
                    self.diagnostic_type_name(actual),
                    self.diagnostic_type_name(expected),
                ),
            )
            .with_primary_label(span, "type mismatch"),
        );
        false
    }

    pub(super) fn new(
        program: &'program ResolvedProgram,
        copy_capabilities: &'program CopyCapabilities,
        declaration: &'program ResolvedFunctionDeclaration,
        definition: &'program ResolvedFunctionDefinition,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            copy_capabilities,
            callable: declaration.id.into(),
            parameters: &declaration.parameters,
            locals: &definition.locals,
            body: Some(&definition.body),
            definition_span: definition.span,
            callable_name: format!("function `{}`", declaration.name),
            return_type: lower_type(program, &declaration.return_type),
            class_owner: None,
            receiver: None,
            member_body_kind: None,
            initialized_fields: BTreeSet::new(),
            read_only_locals: BTreeSet::new(),
            base_initialized: true,
            diagnostics,
        }
    }

    pub(super) fn check(mut self) -> HirFunctionDefinition {
        let body = self.body.expect("function checker needs a body");
        let locals = self.lower_locals();
        let checked_body = self.check_block(body);

        if self.return_type != Type::Unit && checked_body.effects.can_fall_through() {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(body.span, "a return value is required on every path")
                .with_note(format!(
                    "{} declares return type `{}`",
                    self.callable_name,
                    self.return_type.name()
                )),
            );
        }

        HirFunctionDefinition {
            function: self
                .callable
                .as_function()
                .expect("function checker needs function ID"),
            locals,
            body: checked_body,
            span: self.definition_span,
        }
    }

    pub(super) fn new_member(
        program: &'program ResolvedProgram,
        copy_capabilities: &'program CopyCapabilities,
        context: MemberCheckContext<'program>,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        let base_initialized = context.body_kind != MemberBodyKind::OrdinaryInitializer
            || program
                .class(context.owner)
                .expect("member owner class must exist")
                .direct_base
                .is_none();
        Self {
            program,
            copy_capabilities,
            callable: context.callable,
            parameters: context.parameters,
            locals: &context.definition.locals,
            body: Some(&context.definition.body),
            definition_span: context.definition.span,
            callable_name: context.callable_name,
            return_type: context.return_type,
            class_owner: Some(context.owner),
            receiver: context.receiver,
            member_body_kind: Some(context.body_kind),
            initialized_fields: BTreeSet::new(),
            read_only_locals: BTreeSet::new(),
            base_initialized,
            diagnostics,
        }
    }

    pub(super) fn check_member(mut self) -> HirMemberDefinition {
        let source_body = self.body.expect("member checker needs a body");
        let locals = self.lower_locals();
        let body = self.check_block(source_body);
        let owner = self
            .class_owner
            .expect("member checker needs a class owner");
        let body_kind = self
            .member_body_kind
            .expect("member checker needs a member body kind");
        if body_kind.initializes_receiver() {
            let class = self
                .program
                .class(owner)
                .expect("member owner must reference a class");
            if class.direct_base.is_some() && !self.base_initialized {
                self.diagnostics.push(
                    Diagnostic::error(
                        INVALID_INITIALIZER_BODY,
                        format!("base subobject of `{}` is not initialized", class.name),
                    )
                    .with_primary_label(
                        source_body.span,
                        "a derived ordinary initializer must begin with a valid `super(...)`",
                    ),
                );
            }
            for field in &class.fields {
                if !self.initialized_fields.contains(&field.id) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            FIELD_INITIALIZATION,
                            format!("field `{}` is not initialized", field.name),
                        )
                        .with_primary_label(
                            field.name_span,
                            "this field needs one assignment in `init`",
                        ),
                    );
                }
            }
        } else if self.return_type != Type::Unit && body.effects.can_fall_through() {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(source_body.span, "a return value is required on every path")
                .with_note(format!(
                    "{} declares return type `{}`",
                    self.callable_name,
                    self.return_type.name()
                )),
            );
        }
        HirMemberDefinition {
            callable: self.callable,
            class_owner: owner,
            receiver_class: self.receiver.map(|receiver| receiver.class),
            locals,
            body,
            span: self.definition_span,
        }
    }

    pub(super) fn new_static_initializer(
        program: &'program ResolvedProgram,
        copy_capabilities: &'program CopyCapabilities,
        initializer: &'program crate::resolve::ResolvedStaticFieldInitializer,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            copy_capabilities,
            callable: initializer.id.into(),
            parameters: &[],
            locals: &[],
            body: None,
            definition_span: initializer.span,
            callable_name: format!("static initializer `{}`", initializer.id),
            return_type: Type::Unit,
            class_owner: Some(initializer.id.class()),
            receiver: None,
            member_body_kind: None,
            initialized_fields: BTreeSet::new(),
            read_only_locals: BTreeSet::new(),
            base_initialized: true,
            diagnostics,
        }
    }

    pub(super) fn check_static_initializer(
        mut self,
        expected: Type,
        expression: &'program ResolvedExpression,
    ) -> Option<crate::hir::HirStoredValueInitialization> {
        debug_assert!(self.parameters.is_empty());
        debug_assert!(self.locals.is_empty());
        debug_assert!(self.body.is_none());
        debug_assert!(self.receiver.is_none());
        debug_assert_eq!(self.class_owner, self.callable.class());
        self.check_stored_value_initialization(expected, expression, "static field initializer")
    }

    fn lower_locals(&mut self) -> Vec<HirLocal> {
        self.locals
            .iter()
            .map(|local| {
                let ty = lower_type(self.program, &local.type_syntax);
                if matches!(ty, Type::Obj | Type::Interface(_)) {
                    self.diagnostics.push(
                        Diagnostic::error(
                            INVALID_OBJECT_CONTEXT,
                            format!("local `{}` cannot store a non-owning view", local.name),
                        )
                        .with_primary_label(
                            local.type_syntax.span,
                            "`Obj` and interfaces are available only as alias parameters",
                        ),
                    );
                }
                HirLocal {
                    id: local.id,
                    name: local.name.clone(),
                    name_span: local.name_span,
                    ty,
                    span: local.span,
                }
            })
            .collect()
    }
}
pub(in crate::typeck) use copy::lower_object_call;

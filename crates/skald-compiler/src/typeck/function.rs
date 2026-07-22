//! Per-callable context, statement checking, and structured control flow.

use std::collections::BTreeSet;

use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    hir::{
        BlockFlow, HirAccess, HirConstruction, HirFieldAssignment, HirFieldConstruction,
        HirFunctionDefinition, HirLocal, HirMemberDefinition, HirStatement, Type,
    },
    identity::{BindingId, CallableId, ClassId, FieldId},
    resolve::{
        ResolvedBlock, ResolvedFunctionDeclaration, ResolvedFunctionDefinition, ResolvedLocal,
        ResolvedMemberDefinition, ResolvedParameter, ResolvedProgram,
    },
};

use super::{
    capabilities::CopyCapabilities,
    expression::{is_call_through_groups, require_type, ObjectPlaceUse},
    program::{
        lower_type, COPY_OPERATION_UNAVAILABLE, FIELD_INITIALIZATION, INVALID_CALL_STATEMENT,
        INVALID_CONSTRUCTION, INVALID_INITIALIZER_BODY, INVALID_OBJECT_CONTEXT, INVALID_RETURN,
        MISSING_RETURN, READ_ONLY_RECEIVER,
    },
};

mod copy;
mod initializer;
mod statement;

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
    pub(super) body_kind: MemberBodyKind,
}

pub(super) struct MemberCheckContext<'program> {
    pub(super) callable: CallableId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) definition: &'program ResolvedMemberDefinition,
    pub(super) return_type: Type,
    pub(super) receiver: ReceiverContext,
    pub(super) callable_name: String,
}

pub(super) struct CallableChecker<'program, 'diagnostics> {
    pub(super) program: &'program ResolvedProgram,
    pub(super) copy_capabilities: &'program CopyCapabilities,
    pub(super) callable: CallableId,
    pub(super) parameters: &'program [ResolvedParameter],
    pub(super) locals: &'program [ResolvedLocal],
    body: &'program ResolvedBlock,
    definition_span: crate::source::Span,
    callable_name: String,
    pub(super) return_type: Type,
    pub(super) receiver: Option<ReceiverContext>,
    pub(super) initialized_fields: BTreeSet<FieldId>,
    pub(super) diagnostics: &'diagnostics mut Diagnostics,
}

impl<'program, 'diagnostics> CallableChecker<'program, 'diagnostics> {
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
            body: &definition.body,
            definition_span: definition.span,
            callable_name: format!("function `{}`", declaration.name),
            return_type: lower_type(&declaration.return_type),
            receiver: None,
            initialized_fields: BTreeSet::new(),
            diagnostics,
        }
    }

    pub(super) fn check(mut self) -> HirFunctionDefinition {
        let locals = self.lower_locals();
        let body = self.check_block(self.body);

        if self.return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(self.body.span, "a return value is required on every path")
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
            body,
            span: self.definition_span,
        }
    }

    pub(super) fn new_member(
        program: &'program ResolvedProgram,
        copy_capabilities: &'program CopyCapabilities,
        context: MemberCheckContext<'program>,
        diagnostics: &'diagnostics mut Diagnostics,
    ) -> Self {
        Self {
            program,
            copy_capabilities,
            callable: context.callable,
            parameters: context.parameters,
            locals: &context.definition.locals,
            body: &context.definition.body,
            definition_span: context.definition.span,
            callable_name: context.callable_name,
            return_type: context.return_type,
            receiver: Some(context.receiver),
            initialized_fields: BTreeSet::new(),
            diagnostics,
        }
    }

    pub(super) fn check_member(mut self) -> HirMemberDefinition {
        let locals = self.lower_locals();
        let body = self.check_block(self.body);
        let receiver = self.receiver.expect("member checker needs receiver");
        if receiver.body_kind.initializes_receiver() {
            let class = self
                .program
                .class(receiver.class)
                .expect("member receiver must reference a class");
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
        } else if self.return_type != Type::Unit && body.flow == BlockFlow::FallsThrough {
            self.diagnostics.push(
                Diagnostic::error(
                    MISSING_RETURN,
                    format!("{} does not return a value", self.callable_name),
                )
                .with_primary_label(self.body.span, "a return value is required on every path")
                .with_note(format!(
                    "{} declares return type `{}`",
                    self.callable_name,
                    self.return_type.name()
                )),
            );
        }
        HirMemberDefinition {
            callable: self.callable,
            locals,
            body,
            span: self.definition_span,
        }
    }

    fn lower_locals(&self) -> Vec<HirLocal> {
        self.locals
            .iter()
            .map(|local| HirLocal {
                id: local.id,
                name: local.name.clone(),
                name_span: local.name_span,
                ty: lower_type(&local.type_syntax),
                span: local.span,
            })
            .collect()
    }

    fn check_construction_initializer(
        &mut self,
        expected_class: ClassId,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<HirConstruction> {
        let crate::resolve::ResolvedExpression::Construct(construction) = expression else {
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_OBJECT_CONTEXT,
                    "an object local must be initialized by direct construction",
                )
                .with_primary_label(
                    expression.span(),
                    "expected an ungrouped `Class(...)` expression",
                ),
            );
            return None;
        };
        self.check_object_construction(expected_class, construction, "object local")
    }

    pub(super) fn check_object_construction(
        &mut self,
        expected_class: ClassId,
        construction: &crate::resolve::ResolvedConstructExpr,
        destination: &str,
    ) -> Option<HirConstruction> {
        if construction.class != expected_class {
            let actual_name = &self
                .program
                .class(construction.class)
                .expect("resolved constructor class must exist")
                .name;
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved local class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("constructor type does not match the {destination}"),
                )
                .with_primary_label(
                    construction.callee_span,
                    format!("constructs `{actual_name}`"),
                )
                .with_note(format!("the {destination} requires `{expected_name}`")),
            );
            return None;
        }
        self.check_construction_arguments(construction)
    }

    fn check_field_construction(
        &mut self,
        expected_class: ClassId,
        field_name: &str,
        expression: &crate::resolve::ResolvedExpression,
    ) -> Option<HirConstruction> {
        let crate::resolve::ResolvedExpression::Construct(construction) = expression else {
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved field class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("class field `{field_name}` requires direct construction"),
                )
                .with_primary_label(
                    expression.span(),
                    format!("expected an ungrouped `{expected_name}(...)` construction"),
                ),
            );
            return None;
        };
        if construction.class != expected_class {
            let actual_name = &self
                .program
                .class(construction.class)
                .expect("resolved constructor class must exist")
                .name;
            let expected_name = &self
                .program
                .class(expected_class)
                .expect("resolved field class must exist")
                .name;
            self.diagnostics.push(
                Diagnostic::error(
                    INVALID_CONSTRUCTION,
                    format!("constructor type does not match class field `{field_name}`"),
                )
                .with_primary_label(
                    construction.callee_span,
                    format!("constructs `{actual_name}`"),
                )
                .with_note(format!("the field requires `{expected_name}`")),
            );
            return None;
        }
        self.check_construction_arguments(construction)
    }

    fn check_construction_arguments(
        &mut self,
        construction: &crate::resolve::ResolvedConstructExpr,
    ) -> Option<HirConstruction> {
        let initializer = self
            .program
            .initializer(construction.initializer)
            .expect("resolved construction must reference an initializer");
        let arguments = self.check_arguments(
            &construction.arguments,
            &initializer.parameters,
            construction.callee_span,
            "initializer",
            None,
            None,
        )?;
        Some(HirConstruction {
            class: construction.class,
            initializer: construction.initializer,
            arguments,
            span: construction.span,
        })
    }
}

//! Rendering for HIR expressions, iteration, and I/O operations.

use std::fmt::Display;

use super::super::ir::*;
use super::call::method_target;
use super::ownership::{access_name, view_target_name};
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn for_in(&mut self, statement: &HirForIn) {
        self.line(
            &format!("ForIn {} binding {}", statement.loop_id, statement.binding),
            statement.spans.span,
        );
        self.indented(|dumper| {
            match &statement.plan {
                HirForInPlan::Protocol(plan) => dumper.protocol_iteration_plan(plan),
                HirForInPlan::PrimitiveRange(plan) => dumper.primitive_range_plan(plan),
            }
            dumper.raw_line(&format!("Effects {:?}", statement.effects));
            dumper.block(&statement.body);
        });
    }

    fn protocol_iteration_plan(&mut self, plan: &HirProtocolIterationPlan) {
        self.raw_line(&format!(
            "Protocol interface={} item={} state={} result={}",
            plan.protocol.interface,
            self.type_name(plan.protocol.item),
            self.type_name(plan.protocol.state),
            plan.protocol.result,
        ));
        self.raw_line(&format!(
            "Requirements iter_state={} iter_next={}",
            plan.protocol.iter_state, plan.protocol.iter_next,
        ));
        self.raw_line(&format!(
            "Receiver iterable={} lifetime={:?}",
            self.type_name(plan.receiver.iterable),
            plan.receiver.lifetime,
        ));
        self.indented(|dumper| match &plan.receiver.carrier {
            HirIterationReceiverCarrier::View(view) => dumper.object_view("LoopReceiver", view),
            HirIterationReceiverCarrier::Checked(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                dumper.object_view(&format!("LoopReceiver Checked {kind}"), &view.view);
            }
        });
        self.raw_line(&format!(
            "State copy={:?} destruction={:?}",
            plan.state.value.copy, plan.state.value.destruction,
        ));
        self.raw_line(&format!(
            "IterState target={}:{} receiver={} result={}",
            plan.state.initialize.target.interface,
            plan.state.initialize.target.requirement,
            access_name(plan.state.initialize.receiver_access),
            self.type_name(plan.state.initialize.result),
        ));
        self.raw_line(&format!(
            "IterNext target={}:{} receiver={} state-alias={} {} result={}",
            plan.state.advance.target.interface,
            plan.state.advance.target.requirement,
            access_name(plan.state.advance.receiver_access),
            access_name(plan.state.advance.state_alias.access),
            self.type_name(plan.state.advance.state_alias.ty),
            self.type_name(plan.state.advance.result),
        ));
        self.raw_line(&format!(
            "Result optional={} payload={} presence={:?} unwrap={:?} destruction={:?}",
            plan.result.optional,
            self.type_name(plan.result.payload),
            plan.result.presence,
            plan.result.unwrap,
            plan.result.destruction,
        ));
        self.iteration_item(&plan.item);
    }

    fn primitive_range_plan(&mut self, plan: &HirPrimitiveRangeIterationPlan) {
        self.raw_line(&format!(
            "PrimitiveRange endpoint={} compare={:?} increment={:?}",
            plan.integer.name(),
            plan.comparison,
            plan.increment,
        ));
        self.raw_line(&format!(
            "RangeLoopEvidence template={} class={} initializer={} iterable={}",
            plan.evidence.range_template,
            plan.evidence.range_class,
            plan.evidence.initializer,
            plan.evidence.iterable,
        ));
        self.indented(|dumper| {
            dumper.range_protocol_evidence(
                "Ordering",
                plan.evidence.ordering,
                plan.evidence.operator_span,
            );
            dumper.range_protocol_evidence(
                "Successor",
                plan.evidence.successor,
                plan.evidence.operator_span,
            );
            dumper.line("Lower", plan.lower.span);
            dumper.indented(|dumper| dumper.expression(&plan.lower));
            dumper.line("Upper", plan.upper.span);
            dumper.indented(|dumper| dumper.expression(&plan.upper));
        });
        self.iteration_item(&plan.item);
    }

    fn iteration_item(&mut self, item: &HirIterationItemPlan) {
        self.raw_line(&format!(
            "Item binding={} access={} copy={:?} destruction={:?}",
            item.binding,
            access_name(item.access),
            item.value.copy,
            item.value.destruction,
        ));
    }

    pub(super) fn copy_construction(&mut self, copy: &crate::hir::HirCopyConstruction) {
        self.line("CopyConstruction", copy.span);
        self.indented(|dumper| {
            dumper.object_place(&copy.destination);
            dumper.object_source(&copy.source);
            dumper.selected_copy_operation(copy.operation);
        });
    }

    pub(super) fn selected_copy_operation<I: Display>(
        &mut self,
        operation: HirSelectedCopyOperation<I>,
    ) {
        match operation {
            HirSelectedCopyOperation::User(id) => self.raw_line(&format!("Operation User {id}")),
            HirSelectedCopyOperation::Synthesized(class) => {
                self.raw_line(&format!("Operation Synthesized {class}"));
            }
        }
    }

    pub(super) fn conditional(&mut self, statement: &HirConditional) {
        self.line("Conditional", statement.span);
        self.indented(|dumper| {
            for (index, arm) in statement.arms.iter().enumerate() {
                dumper.line(if index == 0 { "IfArm" } else { "ElifArm" }, arm.span);
                dumper.indented(|dumper| {
                    dumper.heading("Condition");
                    dumper.indented(|dumper| dumper.expression(&arm.condition));
                    dumper.block(&arm.body);
                });
            }
            if let Some(block) = &statement.else_block {
                dumper.heading("ElseArm");
                dumper.indented(|dumper| dumper.block(block));
            }
        });
    }

    pub(super) fn expression(&mut self, expression: &HirExpression) {
        match &expression.kind {
            HirExpressionKind::Binding(binding) => {
                self.typed_line(&format!("Binding {binding}"), expression);
            }
            HirExpressionKind::FunctionReference(reference) => {
                self.typed_line(
                    &format!(
                        "FunctionReference {} signature {}",
                        reference.target, reference.function_type
                    ),
                    expression,
                );
            }
            HirExpressionKind::I64(value) => {
                self.typed_line(&format!("Integer {value}"), expression);
            }
            HirExpressionKind::U64(value) => {
                self.typed_line(&format!("U64 {value}"), expression);
            }
            HirExpressionKind::U8(value) => {
                self.typed_line(&format!("U8 {value}"), expression);
            }
            HirExpressionKind::F64Bits(bits) => {
                self.typed_line(&format!("F64 0x{bits:016x}"), expression);
            }
            HirExpressionKind::Boolean(value) => {
                self.typed_line(&format!("Boolean {value}"), expression);
            }
            HirExpressionKind::Unary { operation, operand } => {
                let operation = match operation {
                    HirUnaryOperation::NegateI64 => "NegateI64".to_owned(),
                    HirUnaryOperation::NegateF64 => "NegateF64".to_owned(),
                    HirUnaryOperation::LogicalNotBool => "LogicalNotBool".to_owned(),
                    HirUnaryOperation::BitwiseComplement(integer) => {
                        format!("BitwiseComplement.{}", integer.name())
                    }
                };
                self.typed_line(&format!("Unary {operation}"), expression);
                self.indented(|dumper| dumper.expression(operand));
            }
            HirExpressionKind::Binary {
                operation,
                left,
                right,
            } => {
                let operation = match operation {
                    HirBinaryOperation::AddI64 => "AddI64".to_owned(),
                    HirBinaryOperation::SubtractI64 => "SubtractI64".to_owned(),
                    HirBinaryOperation::MultiplyI64 => "MultiplyI64".to_owned(),
                    HirBinaryOperation::AddU64 => "AddU64".to_owned(),
                    HirBinaryOperation::SubtractU64 => "SubtractU64".to_owned(),
                    HirBinaryOperation::MultiplyU64 => "MultiplyU64".to_owned(),
                    HirBinaryOperation::AddU8 => "AddU8".to_owned(),
                    HirBinaryOperation::SubtractU8 => "SubtractU8".to_owned(),
                    HirBinaryOperation::MultiplyU8 => "MultiplyU8".to_owned(),
                    HirBinaryOperation::AddF64 => "AddF64".to_owned(),
                    HirBinaryOperation::SubtractF64 => "SubtractF64".to_owned(),
                    HirBinaryOperation::MultiplyF64 => "MultiplyF64".to_owned(),
                    HirBinaryOperation::DivideF64 => "DivideF64".to_owned(),
                    HirBinaryOperation::IntegerBitwise { operation, operand } => {
                        format!("Bitwise{}.{}", bitwise_title(*operation), operand.name())
                    }
                };
                self.typed_line(&format!("Binary {operation}"), expression);
                self.indented(|dumper| {
                    dumper.expression(left);
                    dumper.expression(right);
                });
            }
            HirExpressionKind::CheckedShift(shift) => {
                self.typed_line(
                    &format!(
                        "CheckedShift {}.{} count=u64 width={} failure=shift-count-out-of-range",
                        shift.operation.mnemonic(),
                        shift.operation.left.name(),
                        shift.operation.width()
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Left");
                    dumper.indented(|dumper| dumper.expression(&shift.left));
                    dumper.heading("Count");
                    dumper.indented(|dumper| dumper.expression(&shift.count));
                });
            }
            HirExpressionKind::CheckedIntegerDivision(division) => {
                division.validate(expression.ty);
                let signed_semantics = division
                    .operation
                    .signed_semantics()
                    .map(|semantics| {
                        let minimum_pair_result = match semantics.minimum_pair_result {
                            crate::hir::HirSignedMinimumPairResult::Minimum => "minimum",
                            crate::hir::HirSignedMinimumPairResult::Zero => "zero",
                        };
                        format!(
                            " signed-quotient=floor signed-remainder-sign=divisor minimum-pair={minimum_pair_result}"
                        )
                    })
                    .unwrap_or_default();
                self.typed_line(
                    &format!(
                        "CheckedIntegerDivision {}.{}{} failure={}",
                        division.operation.mnemonic(),
                        division.operation.operand.name(),
                        signed_semantics,
                        division.operation.failure().mnemonic(),
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Dividend");
                    dumper.indented(|dumper| dumper.expression(&division.dividend));
                    dumper.heading("Divisor");
                    dumper.indented(|dumper| dumper.expression(&division.divisor));
                });
            }
            HirExpressionKind::Logical(logical) => {
                let operation = match logical.operation {
                    crate::hir::HirLogicalOperation::And => "And",
                    crate::hir::HirLogicalOperation::Or => "Or",
                };
                self.typed_line(&format!("Logical {operation}"), expression);
                self.indented(|dumper| {
                    dumper.heading("Left");
                    dumper.indented(|dumper| dumper.expression(&logical.left));
                    dumper.heading("Right");
                    dumper.indented(|dumper| dumper.expression(&logical.right));
                });
            }
            HirExpressionKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => {
                let family = match operation.operand {
                    HirComparisonOperand::Integer(_) => "IntegerComparison",
                    HirComparisonOperand::F64 => "FloatingComparison",
                    HirComparisonOperand::Bool => "BooleanComparison",
                };
                self.typed_line(
                    &format!(
                        "{family} {}.{}",
                        operation.predicate.mnemonic(),
                        operation.operand.name()
                    ),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.expression(left);
                    dumper.expression(right);
                });
            }
            HirExpressionKind::PrimitiveCast { operation, operand } => {
                let failure = if operation.may_terminate() {
                    " failure=primitive-cast-out-of-range"
                } else {
                    ""
                };
                self.typed_line(
                    &format!(
                        "PrimitiveCast {} {}.{}{}",
                        operation.kind().mnemonic(),
                        operation.source.name(),
                        operation.target.name(),
                        failure,
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.expression(operand));
            }
            HirExpressionKind::Io(operation) => self.io_operation(expression, operation),
            HirExpressionKind::DirectCall {
                function,
                arguments,
            } => {
                self.typed_line(&format!("DirectCall {function}"), expression);
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::IndirectCall(call) => {
                self.typed_line(
                    &format!("IndirectCall type {}", call.function_type),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.call_argument(argument);
                        }
                    });
                });
            }
            HirExpressionKind::StaticCall { method, arguments } => {
                self.typed_line(&format!("StaticCall {method}"), expression);
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::Grouped(inner) => {
                self.typed_line("Grouped", expression);
                self.indented(|dumper| dumper.expression(inner));
            }
            HirExpressionKind::FieldRead(place) => {
                self.typed_line(&format!("FieldRead {}", place.field), expression);
                self.indented(|dumper| match place.receiver.inspection_place() {
                    Some(receiver) => dumper.object_place(receiver),
                    None => dumper.field_place(place),
                });
            }
            HirExpressionKind::StaticRead(place) => {
                self.typed_line(&format!("StaticRead {}", place.field), expression);
            }
            HirExpressionKind::MethodCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(&format!("MethodCall {}", method_target(target)), expression);
                self.indented(|dumper| {
                    dumper.method_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::InterfaceCall {
                receiver,
                target,
                arguments,
            } => {
                self.typed_line(
                    &format!("InterfaceCall {} {}", target.interface, target.requirement),
                    expression,
                );
                self.indented(|dumper| {
                    dumper.interface_receiver(receiver);
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirExpressionKind::TypeTest(test) => {
                let kind = match test.kind {
                    HirTypeTestKind::StaticSuccess => "static-success",
                    HirTypeTestKind::StaticFailure => "static-failure",
                    HirTypeTestKind::Runtime => "runtime",
                };
                self.typed_line(
                    &format!("TypeTest -> {} {kind}", view_target_name(test.target)),
                    expression,
                );
                self.indented(|dumper| dumper.object_view("ObjectView", &test.source));
            }
            HirExpressionKind::PresenceTest { source, kind } => {
                let kind = match kind {
                    crate::hir::HirPresenceTestKind::Some => "Some",
                    crate::hir::HirPresenceTestKind::None => "None",
                };
                self.typed_line(&format!("PresenceTest {kind}"), expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
            HirExpressionKind::Unwrap(source) => {
                self.typed_line("OptionalUnwrap", expression);
                self.indented(|dumper| dumper.optional_operand(source));
            }
            HirExpressionKind::NestedOptionalUnwrap(unwrap) => {
                self.typed_line(
                    &format!(
                        "NestedOptionalUnwrap optional={} payload={}",
                        unwrap.optional, unwrap.payload
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.optional_operand(&unwrap.source));
            }
            HirExpressionKind::OptionalArrayUnwrap(unwrap) => {
                self.typed_line(
                    &format!(
                        "OptionalArrayUnwrap optional={} array={}",
                        unwrap.optional, unwrap.array
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.optional_operand(&unwrap.source));
            }
            HirExpressionKind::ArrayConstruction(construction) => {
                self.typed_line("ArrayConstruction", expression);
                self.indented(|dumper| dumper.array_construction(construction));
            }
            HirExpressionKind::ArrayLength(length) => {
                self.typed_line("ArrayLength", expression);
                self.indented(|dumper| dumper.array_receiver(&length.receiver));
            }
            HirExpressionKind::ArrayElement(place) => {
                self.typed_line("ArrayElement", expression);
                self.indented(|dumper| dumper.array_element(place));
            }
            HirExpressionKind::ArraySlice(slice) => {
                self.typed_line("CopiedArraySlice", expression);
                self.indented(|dumper| dumper.array_slice(slice));
            }
            HirExpressionKind::OptionalBoxPresence(presence) => {
                self.typed_line(
                    &format!(
                        "OptionalBoxPresence {:?} target={}",
                        presence.kind, presence.box_target
                    ),
                    expression,
                );
                self.indented(|dumper| dumper.shared_source(&presence.source));
            }
        }
    }

    fn io_operation(&mut self, expression: &HirExpression, operation: &HirIoOperation) {
        let name = match operation {
            HirIoOperation::StandardHandle { .. } => "StandardHandle",
            HirIoOperation::Open { .. } => "Open",
            HirIoOperation::Read { .. } => "Read",
            HirIoOperation::Write { .. } => "Write",
            HirIoOperation::Close { .. } => "Close",
        };
        self.typed_line(&format!("Io {name}"), expression);
        self.indented(|dumper| match operation {
            HirIoOperation::StandardHandle { stream } => dumper.expression(stream),
            HirIoOperation::Open { path, mode } => {
                dumper.call_argument(&HirCallArgument::ArrayAlias(path.clone()));
                dumper.expression(mode);
            }
            HirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                dumper.expression(handle);
                dumper.call_argument(&HirCallArgument::ArrayAlias(destination.clone()));
                dumper.expression(offset);
            }
            HirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                dumper.expression(handle);
                dumper.call_argument(&HirCallArgument::ArrayAlias(source.clone()));
                dumper.expression(offset);
            }
            HirIoOperation::Close { handle } => dumper.expression(handle),
        });
    }
}

const fn bitwise_title(operation: crate::hir::HirIntegerBitwiseOperation) -> &'static str {
    match operation {
        crate::hir::HirIntegerBitwiseOperation::And => "And",
        crate::hir::HirIntegerBitwiseOperation::Or => "Or",
        crate::hir::HirIntegerBitwiseOperation::Xor => "Xor",
    }
}

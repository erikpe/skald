//! Discovery of block-changing control-flow effects nested in HIR expressions.

use crate::hir::{
    HirCallArgument, HirCheckedObjectView, HirCheckedObjectViewKind, HirExpression,
    HirExpressionKind, HirInterfaceReceiver, HirObjectCallTarget, HirObjectProducer,
    HirObjectSource, HirSharedAllocationMode, HirSharedProducer, HirSharedSource, HirViewSource,
};

pub(super) fn expression_contains_control_effect(expression: &HirExpression) -> bool {
    match &expression.kind {
        HirExpressionKind::Unary { operand, .. } | HirExpressionKind::Grouped(operand) => {
            expression_contains_control_effect(operand)
        }
        HirExpressionKind::PrimitiveCast { operation, operand } => {
            operation.may_terminate() || expression_contains_control_effect(operand)
        }
        HirExpressionKind::Binary { left, right, .. }
        | HirExpressionKind::PrimitiveComparison { left, right, .. } => {
            expression_contains_control_effect(left) || expression_contains_control_effect(right)
        }
        // Checked arithmetic always introduces a semantic-check diamond,
        // regardless of whether its operands are otherwise pure.
        HirExpressionKind::CheckedIntegerDivision(_) | HirExpressionKind::CheckedShift(_) => true,
        // IO3 lowers these operations into explicit control-flow and runtime
        // calls. Treat them conservatively until that lowering owns them.
        HirExpressionKind::Io(_) => true,
        // Logical expressions always select blocks even when both operands
        // are otherwise pure.
        HirExpressionKind::Logical(_) => true,
        HirExpressionKind::DirectCall { arguments, .. }
        | HirExpressionKind::StaticCall { arguments, .. } => {
            arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::IndirectCall(call) => {
            expression_contains_control_effect(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(call_argument_contains_control_effect)
        }
        HirExpressionKind::MethodCall {
            receiver,
            arguments,
            ..
        } => {
            method_receiver_contains_control_effect(receiver)
                || arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::InterfaceCall {
            receiver,
            arguments,
            ..
        } => {
            interface_receiver_contains_control_effect(receiver)
                || arguments.iter().any(call_argument_contains_control_effect)
        }
        HirExpressionKind::FieldRead(place) => {
            object_receiver_contains_control_effect(&place.receiver)
        }
        HirExpressionKind::TypeTest(test) => {
            view_source_contains_control_effect(&test.source.source)
        }
        HirExpressionKind::PresenceTest { source, .. } => {
            optional_operand_contains_control_effect(source)
        }
        HirExpressionKind::OptionalBoxPresence(presence) => {
            matches!(presence.source, crate::hir::HirSharedSource::Produced(_))
        }
        HirExpressionKind::Binding(_)
        | HirExpressionKind::FunctionReference(_)
        | HirExpressionKind::StaticRead(_)
        | HirExpressionKind::I64(_)
        | HirExpressionKind::U64(_)
        | HirExpressionKind::U8(_)
        | HirExpressionKind::F64Bits(_)
        | HirExpressionKind::Boolean(_) => false,
        HirExpressionKind::Unwrap(_)
        | HirExpressionKind::NestedOptionalUnwrap(_)
        | HirExpressionKind::OptionalArrayUnwrap(_) => true,
        HirExpressionKind::ArrayConstruction(construction) => {
            array_construction_contains_control_effect(construction)
        }
        // Checked array access lowers through explicit MIR control-flow
        // blocks, so earlier scalar values must be spilled before a later
        // operand changes the current block.
        HirExpressionKind::ArrayLength(_)
        | HirExpressionKind::ArrayElement(_)
        | HirExpressionKind::ArraySlice(_) => true,
    }
}

pub(super) fn call_argument_contains_control_effect(argument: &HirCallArgument) -> bool {
    match argument {
        HirCallArgument::Value(expression) => expression_contains_control_effect(expression),
        HirCallArgument::Optional { .. } => true,
        HirCallArgument::ClassOptional(_) => true,
        HirCallArgument::OptionalShared(_) => true,
        HirCallArgument::AggregateOptional(_) => true,
        HirCallArgument::CheckedView(view) => checked_view_contains_control_effect(view),
        HirCallArgument::View(view) => view_source_contains_control_effect(&view.source),
        HirCallArgument::Copy(copy) => object_source_contains_control_effect(&copy.source),
        HirCallArgument::OptionalPlace(place) => optional_alias_contains_control_effect(place),
        HirCallArgument::Place(_) | HirCallArgument::PrimitivePlace(_) => false,
        HirCallArgument::Shared(transfer) => match &transfer.source {
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                shared_allocation_contains_control_effect(allocation)
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                expression_contains_control_effect(call)
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                cast.kind == crate::hir::HirSharedCastKind::RuntimeTerminate
                    || shared_source_contains_control_effect(&cast.source)
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap { .. }) => true,
            HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
                array_construction_contains_control_effect(construction)
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalBoxAllocation(_)) => true,
            HirSharedSource::Place(_) => false,
        },
        HirCallArgument::Array(initialization) => {
            array_initialization_contains_control_effect(initialization)
        }
        HirCallArgument::ArrayAlias(alias) => match &alias.source {
            crate::hir::HirArrayAliasSource::Whole(receiver) => {
                array_receiver_contains_control_effect(receiver)
            }
            crate::hir::HirArrayAliasSource::Element(_)
            | crate::hir::HirArrayAliasSource::OptionalPayload { .. } => true,
        },
    }
}

fn optional_alias_contains_control_effect(place: &crate::hir::HirOptionalAliasPlace) -> bool {
    match place {
        crate::hir::HirOptionalAliasPlace::Primitive(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalAliasPlace::Class(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalAliasPlace::Nested(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
    }
}

fn optional_operand_contains_control_effect(operand: &crate::hir::HirOptionalOperand) -> bool {
    match operand {
        crate::hir::HirOptionalOperand::Place(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalOperand::ClassPlace(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalOperand::SharedPlace(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalOperand::AggregatePlace(place) => {
            optional_storage_contains_control_effect(&place.storage)
        }
        crate::hir::HirOptionalOperand::Produced(expression)
        | crate::hir::HirOptionalOperand::ClassProduced(expression)
        | crate::hir::HirOptionalOperand::SharedProduced(expression)
        | crate::hir::HirOptionalOperand::AggregateProduced(expression) => {
            expression_contains_control_effect(expression)
        }
    }
}

fn optional_storage_contains_control_effect(storage: &crate::hir::HirOptionalStorage) -> bool {
    match storage {
        crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
            shared_source_contains_control_effect(&pointee.source)
        }
        crate::hir::HirOptionalStorage::Binding(_)
        | crate::hir::HirOptionalStorage::Static(_)
        | crate::hir::HirOptionalStorage::Field(_)
        | crate::hir::HirOptionalStorage::ArrayElement(_) => false,
    }
}

fn shared_source_contains_control_effect(source: &HirSharedSource) -> bool {
    match source {
        HirSharedSource::Place(place) => shared_place_contains_control_effect(place),
        HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
            shared_allocation_contains_control_effect(allocation)
        }
        HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
            expression_contains_control_effect(call)
        }
        HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
            cast.kind == crate::hir::HirSharedCastKind::RuntimeTerminate
                || shared_source_contains_control_effect(&cast.source)
        }
        HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap { .. }) => true,
        HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
            array_construction_contains_control_effect(construction)
        }
        HirSharedSource::Produced(HirSharedProducer::OptionalBoxAllocation(_)) => true,
    }
}

fn shared_place_contains_control_effect(place: &crate::hir::HirSharedPlace) -> bool {
    match place {
        crate::hir::HirSharedPlace::Binding { .. } | crate::hir::HirSharedPlace::Static { .. } => {
            false
        }
        crate::hir::HirSharedPlace::Field { place, .. } => {
            object_receiver_contains_control_effect(&place.receiver)
        }
        // Array position and backing-storage checks select successor blocks
        // before the shared owner can be copied or borrowed.
        crate::hir::HirSharedPlace::ArrayElement { .. } => true,
    }
}

fn array_construction_contains_control_effect(
    _construction: &crate::hir::HirArrayConstruction,
) -> bool {
    // Every array production performs checked backing allocation and can move
    // lowering into a successor block before any source-specific effects run.
    true
}

fn array_initialization_contains_control_effect(
    initialization: &crate::hir::HirArrayInitialize,
) -> bool {
    match initialization.operation {
        // A named by-value array transfer lowers through a checked allocation
        // and copy loop even when evaluating the source place itself is pure.
        crate::hir::HirArrayTransfer::DeepCopy(_) => true,
        crate::hir::HirArrayTransfer::Adopt => {
            array_source_contains_control_effect(&initialization.source)
        }
    }
}

fn array_source_contains_control_effect(source: &crate::hir::HirArraySource) -> bool {
    array_receiver_contains_control_effect(&source.receiver)
}

fn array_receiver_contains_control_effect(receiver: &crate::hir::HirArrayReceiver) -> bool {
    match &receiver.source {
        crate::hir::HirArrayReceiverSource::Inline(expression) => {
            expression_contains_control_effect(expression)
        }
        crate::hir::HirArrayReceiverSource::Shared(source) => {
            shared_source_contains_control_effect(source)
        }
    }
}

fn shared_allocation_contains_control_effect(allocation: &crate::hir::HirSharedAllocation) -> bool {
    match &allocation.mode {
        HirSharedAllocationMode::Initialize { arguments, .. } => {
            arguments.iter().any(call_argument_contains_control_effect)
        }
        HirSharedAllocationMode::Copy { source, .. } => {
            object_source_contains_control_effect(source)
        }
    }
}

fn object_source_contains_control_effect(source: &HirObjectSource) -> bool {
    match source {
        HirObjectSource::Place(_) => false,
        HirObjectSource::Static { .. } => false,
        // Forming an object source from an array element performs checked
        // position lowering before the owning copy can begin.
        HirObjectSource::ArrayElement(_) => true,
        HirObjectSource::Produced(producer) => producer_contains_control_effect(producer),
        HirObjectSource::Checked(view) => checked_view_contains_control_effect(view),
        HirObjectSource::Slice(slice) => object_source_contains_control_effect(&slice.source),
    }
}

fn checked_view_contains_control_effect(view: &HirCheckedObjectView) -> bool {
    view.kind == HirCheckedObjectViewKind::RuntimeTerminate
        || view_source_contains_control_effect(&view.view.source)
}

fn view_source_contains_control_effect(source: &HirViewSource) -> bool {
    match source {
        HirViewSource::Produced { producer, .. } => producer_contains_control_effect(producer),
        HirViewSource::Place(_)
        | HirViewSource::Static { .. }
        | HirViewSource::Forwarded { .. }
        | HirViewSource::Shared { .. } => false,
        HirViewSource::ArrayElement(_) => true,
        HirViewSource::AnchoredShared { source, .. } => {
            shared_source_contains_control_effect(source)
        }
        HirViewSource::OptionalPayload { .. } | HirViewSource::OptionalBoxPayload { .. } => true,
    }
}

fn producer_contains_control_effect(producer: &HirObjectProducer) -> bool {
    match producer {
        HirObjectProducer::StringLiteral(_) => false,
        HirObjectProducer::Construct(construction) => match &construction.mode {
            crate::hir::HirConstructionMode::Initialize { arguments, .. } => {
                arguments.iter().any(call_argument_contains_control_effect)
            }
            crate::hir::HirConstructionMode::Copy { source, .. } => {
                object_source_contains_control_effect(source)
            }
        },
        HirObjectProducer::Call(call) => {
            let receiver_has_cast = match &call.target {
                HirObjectCallTarget::Direct(_) | HirObjectCallTarget::Static(_) => false,
                HirObjectCallTarget::Method { receiver, .. } => {
                    method_receiver_contains_control_effect(receiver)
                }
                HirObjectCallTarget::Interface { receiver, .. } => {
                    interface_receiver_contains_control_effect(receiver)
                }
            };
            receiver_has_cast
                || call
                    .arguments
                    .iter()
                    .any(call_argument_contains_control_effect)
        }
        HirObjectProducer::IndirectCall(call) => {
            expression_contains_control_effect(&call.callee)
                || call
                    .arguments
                    .iter()
                    .any(call_argument_contains_control_effect)
        }
    }
}

fn method_receiver_contains_control_effect(receiver: &crate::hir::HirMethodReceiver) -> bool {
    object_receiver_contains_control_effect(receiver)
}

fn object_receiver_contains_control_effect(receiver: &crate::hir::HirObjectReceiver) -> bool {
    match receiver {
        crate::hir::HirObjectReceiver::Place { .. } => false,
        crate::hir::HirObjectReceiver::Checked { view, .. } => {
            checked_view_contains_control_effect(view)
        }
        crate::hir::HirObjectReceiver::View { view, .. } => {
            view_source_contains_control_effect(&view.source)
        }
        crate::hir::HirObjectReceiver::ArrayElement { .. } => true,
    }
}

fn interface_receiver_contains_control_effect(receiver: &HirInterfaceReceiver) -> bool {
    match receiver {
        HirInterfaceReceiver::View(view) => view_source_contains_control_effect(&view.source),
        HirInterfaceReceiver::Checked(view) => checked_view_contains_control_effect(view),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        hir::{
            HirBinaryOperation, HirCheckedIntegerDivision, HirIntegerDivisionKind,
            HirIntegerDivisionOperation, HirIntegerType, HirLocalInitializer, HirPrimitiveCast,
            HirPrimitiveType, HirReturnValue, HirStatement, Type,
        },
        identity::FunctionId,
        test_support::type_check_source,
    };

    #[test]
    fn checked_integer_division_is_control_affecting_with_pure_operands() {
        let mut hir = type_check_source("fn main() -> i64 { return 8 + 3; }\n")
            .hir
            .unwrap();
        let definition = hir
            .definitions
            .get_mut_for_test(hir.entry_function)
            .unwrap();
        let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
            panic!("expected scalar return value");
        };
        let HirExpressionKind::Binary { left, right, .. } = &expression.kind else {
            panic!("expected binary expression");
        };
        expression.kind =
            HirExpressionKind::CheckedIntegerDivision(Box::new(HirCheckedIntegerDivision::new(
                HirIntegerDivisionOperation {
                    kind: HirIntegerDivisionKind::Quotient,
                    operand: HirIntegerType::I64,
                },
                (**left).clone(),
                (**right).clone(),
            )));

        assert!(expression_contains_control_effect(expression));
    }

    #[test]
    fn indirect_calls_include_callee_then_argument_control_effects() {
        let hir = type_check_source(concat!(
            "fn identity(value: i64) -> i64 { return value; }\n",
            "fn choose(value: i64) -> fn(i64) -> i64 { return identity; }\n",
            "fn callee_effect(divisor: i64) -> i64 { return choose(8 / divisor)(1); }\n",
            "fn argument_effect(callback: fn(i64) -> i64, divisor: i64) -> i64 {\n",
            "  return callback(8 / divisor);\n",
            "}\n",
            "fn main() -> i64 { return callee_effect(1); }\n",
        ))
        .hir
        .expect("indirect control-effect source must type check");

        for function in [FunctionId::new(2), FunctionId::new(3)] {
            let definition = hir.definitions.get(function).unwrap();
            let HirStatement::Return(returned) = definition.body.statements.last().unwrap() else {
                panic!("expected return statement");
            };
            let HirReturnValue::Scalar(expression) = returned.value.as_ref().unwrap() else {
                panic!("expected scalar return");
            };
            assert!(matches!(
                expression.kind,
                HirExpressionKind::IndirectCall(_)
            ));
            assert!(expression_contains_control_effect(expression));
        }
    }

    #[test]
    fn checked_primitive_cast_is_control_affecting_with_a_pure_operand() {
        let mut hir = type_check_source(concat!(
            "fn exercise() -> unit { var value: bool = (bool) 1.5; }\n",
            "fn main() -> i64 { return 0; }\n",
        ))
        .hir
        .unwrap();
        let definition = hir
            .definitions
            .get_mut_for_test(crate::identity::FunctionId::new(0))
            .unwrap();
        definition.locals[0].ty = Type::I64;
        let HirStatement::Local(local) = &mut definition.body.statements[0] else {
            unreachable!()
        };
        let HirLocalInitializer::Value(expression) = &mut local.initializer else {
            unreachable!()
        };
        let HirExpressionKind::PrimitiveCast { operation, .. } = &mut expression.kind else {
            unreachable!()
        };
        *operation = HirPrimitiveCast::new(HirPrimitiveType::F64, HirPrimitiveType::I64);
        expression.ty = Type::I64;

        assert!(expression_contains_control_effect(expression));
    }

    #[test]
    fn floating_division_is_not_control_affecting_with_pure_operands() {
        let mut hir = type_check_source(concat!(
            "fn divide() -> f64 { return 8.0 * 2.0; }\n",
            "fn main() -> i64 { return 0; }\n",
        ))
        .hir
        .unwrap();
        let definition = hir
            .definitions
            .get_mut_for_test(crate::identity::FunctionId::new(0))
            .unwrap();
        let HirStatement::Return(statement) = definition.body.statements.last_mut().unwrap() else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = statement.value.as_mut().unwrap() else {
            panic!("expected scalar return value");
        };
        let HirExpressionKind::Binary { operation, .. } = &mut expression.kind else {
            panic!("expected binary expression");
        };
        *operation = HirBinaryOperation::DivideF64;

        assert!(!expression_contains_control_effect(expression));
    }

    #[test]
    fn pure_array_element_lists_are_control_affecting_because_allocation_is_checked() {
        let hir = type_check_source(concat!(
            "fn exercise() -> unit {\n",
            "  var values: i64[] = i64[]{1, 2};\n",
            "  return;\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ))
        .hir
        .unwrap();
        let definition = hir
            .definitions
            .get(crate::identity::FunctionId::new(0))
            .unwrap();
        let HirStatement::Local(local) = &definition.body.statements[0] else {
            panic!("expected array local");
        };
        let HirLocalInitializer::Array(initialization) = &local.initializer else {
            panic!("expected array initializer");
        };
        let crate::hir::HirArrayReceiverSource::Inline(expression) =
            &initialization.source.receiver.source
        else {
            panic!("expected inline element-list source");
        };

        assert!(expression_contains_control_effect(expression));
    }

    #[test]
    fn shared_array_element_interface_receiver_is_control_affecting() {
        let hir = type_check_source(concat!(
            "interface Value<T> { fn value() -> T; }\n",
            "fn read(values: (shared Value<i64>)[]) -> i64 {\n",
            "  return 1 + values[0]->value();\n",
            "}\n",
            "fn main() -> i64 { return 0; }\n",
        ))
        .hir
        .unwrap();
        let definition = hir.definitions.get(FunctionId::new(0)).unwrap();
        let HirStatement::Return(returned) = definition.body.statements.last().unwrap() else {
            panic!("expected return statement");
        };
        let HirReturnValue::Scalar(expression) = returned.value.as_ref().unwrap() else {
            panic!("expected scalar return");
        };

        assert!(expression_contains_control_effect(expression));
    }
}

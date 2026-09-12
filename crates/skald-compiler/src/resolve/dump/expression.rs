//! Expressions, calls, iteration, and resolved operator selections.

use std::fmt::Write;

use crate::{
    dump_format::{write_quoted, write_span},
    source::Span,
};

use super::super::ir::*;
use super::ResolvedDumper;

impl ResolvedDumper<'_> {
    pub(super) fn expression(&mut self, expression: &ResolvedExpression) {
        match expression {
            ResolvedExpression::Absent(absent) => self.line("Absent", absent.span),
            ResolvedExpression::Present(present) => {
                self.line("Present", present.span);
                self.indented(|dumper| dumper.expression(&present.value));
            }
            ResolvedExpression::Binding(binding) => {
                self.line(&format!("Binding {}", binding.binding), binding.span);
            }
            ResolvedExpression::FunctionReference(reference) => {
                self.line(
                    &format!(
                        "FunctionReference {} type {}",
                        reference.target, reference.function_type
                    ),
                    reference.span,
                );
            }
            ResolvedExpression::IndirectCall(call) => {
                self.line(
                    &format!("IndirectCall type {}", call.function_type),
                    call.span,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedExpression::StaticFieldAccess(access) => {
                self.line(&format!("StaticFieldAccess {}", access.field), access.span);
            }
            ResolvedExpression::NumericLiteral(literal) => {
                self.write_indentation();
                self.output.push_str(match literal.kind {
                    crate::literal::NumericLiteralKind::I64(_) => "Integer ",
                    crate::literal::NumericLiteralKind::U64(_) => "U64 ",
                    crate::literal::NumericLiteralKind::U8(_) => "U8 ",
                    crate::literal::NumericLiteralKind::F64 => "F64 ",
                });
                write_quoted(&mut self.output, &literal.spelling);
                write_span(&mut self.output, literal.span);
                self.output.push('\n');
            }
            ResolvedExpression::ByteLiteral(literal) => {
                self.line(&format!("Byte {:02x}", literal.value), literal.span);
            }
            ResolvedExpression::StringLiteral(literal) => {
                self.line(
                    &format!("StringLiteral {} class {}", literal.data, literal.class),
                    literal.span,
                );
            }
            ResolvedExpression::Boolean(boolean) => {
                self.line(
                    if boolean.value {
                        "Boolean true"
                    } else {
                        "Boolean false"
                    },
                    boolean.span,
                );
            }
            ResolvedExpression::Unary(unary) => {
                let operator = match unary.operator {
                    ResolvedUnaryOperator::Negate => "Negate",
                    ResolvedUnaryOperator::LogicalNot => "LogicalNot",
                    ResolvedUnaryOperator::BitwiseComplement => "BitwiseComplement",
                };
                self.line(&format!("Unary {operator}"), unary.span);
                self.indented(|dumper| {
                    if let Some(selection) = &unary.selection {
                        dumper.operator_resolution(selection, unary.operator_span);
                    }
                    dumper.expression(&unary.operand);
                });
            }
            ResolvedExpression::Dereference(dereference) => {
                self.dereference(dereference);
            }
            ResolvedExpression::Binary(binary) => {
                let operator = match binary.operator {
                    ResolvedBinaryOperator::Add => "Add",
                    ResolvedBinaryOperator::Subtract => "Subtract",
                    ResolvedBinaryOperator::Multiply => "Multiply",
                    ResolvedBinaryOperator::Divide => "Divide",
                    ResolvedBinaryOperator::Remainder => "Remainder",
                    ResolvedBinaryOperator::ShiftLeft => "ShiftLeft",
                    ResolvedBinaryOperator::ShiftRight => "ShiftRight",
                    ResolvedBinaryOperator::BitwiseAnd => "BitwiseAnd",
                    ResolvedBinaryOperator::BitwiseOr => "BitwiseOr",
                    ResolvedBinaryOperator::BitwiseXor => "BitwiseXor",
                    ResolvedBinaryOperator::Equal => "Equal",
                    ResolvedBinaryOperator::NotEqual => "NotEqual",
                    ResolvedBinaryOperator::LessThan => "LessThan",
                    ResolvedBinaryOperator::LessEqual => "LessEqual",
                    ResolvedBinaryOperator::GreaterThan => "GreaterThan",
                    ResolvedBinaryOperator::GreaterEqual => "GreaterEqual",
                };
                self.line(&format!("Binary {operator}"), binary.span);
                self.indented(|dumper| {
                    if let Some(selection) = &binary.selection {
                        dumper.operator_resolution(selection, binary.operator_span);
                    }
                    dumper.expression(&binary.left);
                    dumper.expression(&binary.right);
                });
            }
            ResolvedExpression::Logical(logical) => {
                let operator = match logical.operator {
                    ResolvedLogicalOperator::And => "And",
                    ResolvedLogicalOperator::Or => "Or",
                };
                self.line(&format!("Logical {operator}"), logical.span);
                self.indented(|dumper| {
                    dumper.expression(&logical.left);
                    dumper.expression(&logical.right);
                });
            }
            ResolvedExpression::TypeTest(test) => {
                self.line(
                    &format!(
                        "TypeTest target {}",
                        self.render_type_kind(test.target.kind)
                    ),
                    test.span,
                );
                self.indented(|dumper| dumper.expression(&test.source));
            }
            ResolvedExpression::PresenceTest(test) => {
                let kind = match test.kind {
                    ResolvedPresenceTestKind::Some => "Some",
                    ResolvedPresenceTestKind::None => "None",
                };
                self.line(&format!("PresenceTest {kind}"), test.span);
                self.indented(|dumper| {
                    dumper.expression(&test.source);
                    dumper.line("Is", test.is_span);
                    dumper.line(kind, test.target_span);
                });
            }
            ResolvedExpression::Unwrap(unwrap) => {
                self.line("Unwrap", unwrap.span);
                self.indented(|dumper| {
                    dumper.expression(&unwrap.source);
                    dumper.line("Bang", unwrap.bang_span);
                });
            }
            ResolvedExpression::PrimitiveCast(cast) => {
                self.line(
                    &format!("PrimitiveCast target {}", cast.target.name()),
                    cast.span,
                );
                self.indented(|dumper| dumper.expression(&cast.source));
            }
            ResolvedExpression::ObjectCast(cast) => {
                let mode = match cast.target_mode {
                    ResolvedObjectCastTargetMode::Plain => "ObjectCast",
                    ResolvedObjectCastTargetMode::Shared { .. } => "SharedObjectCast",
                };
                self.line(
                    &format!("{mode} target {}", self.render_type_kind(cast.target.kind)),
                    cast.span,
                );
                self.indented(|dumper| dumper.expression(&cast.source));
            }
            ResolvedExpression::Allocation(allocation) => {
                let mode = match &allocation.mode {
                    ResolvedConstructionMode::Initialize { .. } => "Allocate",
                    ResolvedConstructionMode::Copy { .. } => "CopyAllocate",
                };
                self.line(&format!("{mode} {}", allocation.class), allocation.span);
                self.indented(|dumper| match &allocation.mode {
                    ResolvedConstructionMode::Initialize { arguments } => {
                        for argument in arguments {
                            dumper.expression(argument);
                        }
                    }
                    ResolvedConstructionMode::Copy { copy_span, source } => {
                        dumper.line("Copy", *copy_span);
                        dumper.heading("Source");
                        dumper.indented(|dumper| dumper.expression(source));
                    }
                });
            }
            ResolvedExpression::OptionalBoxAllocation(allocation) => {
                self.line(
                    &format!(
                        "OptionalBoxAllocate exact {} target {}",
                        allocation.exact_optional, allocation.target
                    ),
                    allocation.span,
                );
                self.indented(|dumper| {
                    dumper.line("New", allocation.new_span);
                    dumper.line("Target", allocation.target_span);
                    match &allocation.initializer {
                        ResolvedOptionalBoxInitializer::Absent {
                            left_paren_span,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.line("RightParen", *right_paren_span);
                        }
                        ResolvedOptionalBoxInitializer::Value {
                            left_paren_span,
                            value,
                            right_paren_span,
                        } => {
                            dumper.line("LeftParen", *left_paren_span);
                            dumper.heading("Initializer");
                            dumper.indented(|dumper| dumper.expression(value));
                            dumper.line("RightParen", *right_paren_span);
                        }
                    }
                });
            }
            ResolvedExpression::ArrayConstruction(construction) => {
                self.line(
                    &format!(
                        "ArrayConstruction {} {}",
                        if construction.new_span.is_some() {
                            "shared"
                        } else {
                            "inline"
                        },
                        self.render_type_kind(construction.array_type.kind)
                    ),
                    construction.span,
                );
                self.indented(|dumper| match &construction.arguments {
                    ResolvedArrayConstructionArguments::Empty { .. } => {
                        dumper.heading("Empty");
                    }
                    ResolvedArrayConstructionArguments::Length { length, .. } => {
                        dumper.heading("Length");
                        dumper.indented(|dumper| dumper.expression(length));
                    }
                    ResolvedArrayConstructionArguments::Copy {
                        copy_span, source, ..
                    } => {
                        dumper.line("Copy", *copy_span);
                        dumper.indented(|dumper| dumper.expression(source));
                    }
                    ResolvedArrayConstructionArguments::Indexed(initializer) => {
                        dumper.line("Indexed", initializer.left_paren_span);
                        dumper.indented(|dumper| {
                            dumper.heading("Length");
                            dumper.indented(|dumper| dumper.expression(&initializer.length));
                            dumper.line("Semicolon", initializer.semicolon_span);
                            dumper.write_indentation();
                            let _ = write!(dumper.output, "Binding {} ", initializer.binding.id);
                            write_quoted(&mut dumper.output, &initializer.binding.name);
                            write_span(&mut dumper.output, initializer.binding.name_span);
                            dumper.output.push('\n');
                            dumper.line("FatArrow", initializer.arrow_span);
                            dumper.heading("Element");
                            dumper.indented(|dumper| dumper.expression(&initializer.element));
                            dumper.line("RightParen", initializer.right_paren_span);
                        });
                    }
                    ResolvedArrayConstructionArguments::Elements(list) => {
                        dumper.line("Elements", list.left_brace_span);
                        dumper.indented(|dumper| {
                            for (index, element) in list.elements.iter().enumerate() {
                                dumper.expression(element);
                                if let Some(comma_span) = list.comma_spans.get(index) {
                                    dumper.line("Comma", *comma_span);
                                }
                            }
                        });
                        dumper.line("RightBrace", list.right_brace_span);
                    }
                });
            }
            ResolvedExpression::ArrayLength(length) => {
                self.line(
                    match length.operator {
                        crate::resolve::ResolvedArrayLengthOperator::Ordinary { .. } => {
                            "ArrayLength"
                        }
                        crate::resolve::ResolvedArrayLengthOperator::Shared { .. } => {
                            "SharedArrayLength"
                        }
                    },
                    length.span,
                );
                self.indented(|dumper| {
                    dumper.expression(&length.receiver);
                    for argument in &length.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::DirectCall(call) => {
                self.line(&format!("DirectCall {}", call.function), call.span);
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::StaticCall(call) => {
                self.line(&format!("StaticCall {}", call.method), call.span);
                self.indented(|dumper| {
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Grouped(grouped) => {
                self.line("Grouped", grouped.span);
                self.indented(|dumper| dumper.expression(&grouped.expression));
            }
            ResolvedExpression::FieldAccess(access) => {
                self.line(&format!("FieldAccess {}", access.field), access.span);
                self.indented(|dumper| dumper.object_receiver(&access.receiver));
            }
            ResolvedExpression::ArrayProjection(projection) => {
                self.line(
                    match projection.operator {
                        ResolvedArrayProjectionOperator::Ordinary { .. } => "ArrayProjection",
                        ResolvedArrayProjectionOperator::Shared { .. } => "SharedArrayProjection",
                    },
                    projection.span,
                );
                self.indented(|dumper| {
                    dumper.expression(&projection.receiver);
                    match &projection.bounds {
                        ResolvedArrayProjectionBounds::Index(index) => {
                            dumper.heading("Index");
                            dumper.indented(|dumper| dumper.expression(index));
                        }
                        ResolvedArrayProjectionBounds::Slice { start, end, .. } => {
                            dumper.heading("Slice");
                            dumper.indented(|dumper| {
                                if let Some(start) = start {
                                    dumper.heading("Start");
                                    dumper.indented(|dumper| dumper.expression(start));
                                }
                                if let Some(end) = end {
                                    dumper.heading("End");
                                    dumper.indented(|dumper| dumper.expression(end));
                                }
                            });
                        }
                    }
                });
            }
            ResolvedExpression::MethodCall(call) => {
                self.line(&format!("MethodCall {}", call.method), call.span);
                self.indented(|dumper| {
                    dumper.object_receiver(&call.receiver);
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.expression(argument);
                        }
                    });
                });
            }
            ResolvedExpression::InterfaceCall(call) => {
                let receiver = match &call.receiver {
                    ResolvedInterfaceReceiver::Binding { binding, .. } => {
                        format!("{binding}")
                    }
                    ResolvedInterfaceReceiver::Object(_) => "exact object".to_owned(),
                    ResolvedInterfaceReceiver::Cast(_) => "checked-cast".to_owned(),
                    ResolvedInterfaceReceiver::Dereference(_) => "dereference".to_owned(),
                    ResolvedInterfaceReceiver::OptionalBoxPayload(_) => {
                        "optional-box payload".to_owned()
                    }
                };
                self.line(
                    &format!(
                        "InterfaceCall {} {} receiver {}",
                        call.interface, call.requirement, receiver
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    match &call.receiver {
                        ResolvedInterfaceReceiver::Object(receiver) => {
                            dumper.object_receiver(receiver)
                        }
                        ResolvedInterfaceReceiver::Dereference(dereference) => {
                            dumper.dereference(dereference)
                        }
                        ResolvedInterfaceReceiver::Binding { .. }
                        | ResolvedInterfaceReceiver::Cast(_)
                        | ResolvedInterfaceReceiver::OptionalBoxPayload(_) => {}
                    }
                    for argument in &call.arguments {
                        dumper.expression(argument);
                    }
                });
            }
            ResolvedExpression::Construct(construct) => match &construct.mode {
                ResolvedConstructionMode::Initialize { arguments } => {
                    self.line(&format!("Construct {}", construct.class), construct.span);
                    self.indented(|dumper| {
                        for argument in arguments {
                            dumper.expression(argument);
                        }
                    });
                }
                ResolvedConstructionMode::Copy { copy_span, source } => {
                    self.line(
                        &format!("CopyConstruct {}", construct.class),
                        construct.span,
                    );
                    self.indented(|dumper| {
                        dumper.line("Copy", *copy_span);
                        dumper.heading("Source");
                        dumper.indented(|dumper| dumper.expression(source));
                    });
                }
            },
        }
    }

    fn operator_resolution(&mut self, resolution: &ResolvedOperatorResolution, span: Span) {
        self.line(
            &format!(
                "OperatorResolution {:?} candidates {} incompatible-rhs {}",
                resolution.protocol,
                resolution.candidates.len(),
                resolution.incompatible_rhs.len(),
            ),
            span,
        );
        self.indented(|dumper| {
            for selection in &resolution.candidates {
                dumper.operator_selection(*selection);
            }
            for selection in &resolution.incompatible_rhs {
                dumper.line("IncompatibleRhs", selection.origin_span);
                dumper.indented(|dumper| dumper.operator_selection(*selection));
            }
        });
    }

    fn operator_selection(&mut self, selection: ResolvedOperatorSelection) {
        self.line(
            &format!(
                "OperatorSelection {:?} interface {} requirement {} rhs {:?} output {:?}",
                selection.protocol,
                selection.interface,
                selection.requirement,
                selection.rhs,
                selection.output,
            ),
            selection.origin_span,
        );
    }
}

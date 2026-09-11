//! Provisional expression shape used only for resolution-time name selection.
//!
//! This query never validates expression types. It exposes facts already
//! selected by resolution so member, protocol, and callable lookup can proceed
//! before authoritative type checking.

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProvisionalExpressionType {
    Known(ResolvedTypeKind),
    Unknown,
    Invalid,
}

impl ProvisionalExpressionType {
    pub(super) const fn known(self) -> Option<ResolvedTypeKind> {
        match self {
            Self::Known(kind) => Some(kind),
            Self::Unknown | Self::Invalid => None,
        }
    }
}

impl CallableResolver<'_, '_> {
    pub(super) fn known_provisional_expression_type(
        &self,
        expression: &ResolvedExpression,
    ) -> Option<ResolvedTypeKind> {
        self.provisional_expression_type(expression).known()
    }

    pub(super) fn provisional_expression_type(
        &self,
        expression: &ResolvedExpression,
    ) -> ProvisionalExpressionType {
        use ProvisionalExpressionType::{Invalid, Known, Unknown};

        match expression {
            ResolvedExpression::Absent(_) | ResolvedExpression::Present(_) => Unknown,
            ResolvedExpression::PresenceTest(_) | ResolvedExpression::TypeTest(_) => {
                Known(ResolvedTypeKind::Bool)
            }
            ResolvedExpression::StringLiteral(literal) => {
                Known(ResolvedTypeKind::Class(literal.class))
            }
            ResolvedExpression::NumericLiteral(literal) => Known(match literal.kind {
                crate::literal::NumericLiteralKind::I64(_) => ResolvedTypeKind::I64,
                crate::literal::NumericLiteralKind::U64(_) => ResolvedTypeKind::U64,
                crate::literal::NumericLiteralKind::U8(_) => ResolvedTypeKind::U8,
                crate::literal::NumericLiteralKind::F64 => ResolvedTypeKind::F64,
            }),
            ResolvedExpression::ByteLiteral(_) => Known(ResolvedTypeKind::U8),
            ResolvedExpression::Boolean(_) => Known(ResolvedTypeKind::Bool),
            ResolvedExpression::Binding(binding) => self
                .receiver_class
                .filter(|_| binding.binding == BindingId::Receiver(self.callable))
                .map(ResolvedTypeKind::Class)
                .or_else(|| {
                    self.scopes
                        .iter()
                        .rev()
                        .flat_map(|scope| scope.values())
                        .find(|symbol| symbol.id == binding.binding)
                        .map(|symbol| symbol.ty)
                })
                .map_or(Invalid, Known),
            ResolvedExpression::Dereference(dereference) => match dereference.target {
                ResolvedSharedTarget::Obj => Known(ResolvedTypeKind::Obj),
                ResolvedSharedTarget::Class(class) => Known(ResolvedTypeKind::Class(class)),
                ResolvedSharedTarget::Interface(interface) => {
                    Known(ResolvedTypeKind::Interface(interface))
                }
                ResolvedSharedTarget::Array(array) => Known(ResolvedTypeKind::Array(array)),
                ResolvedSharedTarget::OptionalBox(target) => self
                    .type_interner
                    .optional_box(target)
                    .and_then(|box_| box_.optional)
                    .map(ResolvedTypeKind::Optional)
                    .map_or(Unknown, Known),
            },
            ResolvedExpression::Unwrap(unwrap) => {
                if let Some(target) = self.resolved_optional_box_object_leaf(unwrap) {
                    return Known(match target {
                        ResolvedObjectTarget::Class(class) => ResolvedTypeKind::Class(class),
                        ResolvedObjectTarget::Interface(interface) => {
                            ResolvedTypeKind::Interface(interface)
                        }
                        ResolvedObjectTarget::Obj => ResolvedTypeKind::Obj,
                    });
                }
                match self.provisional_expression_type(&unwrap.source) {
                    Known(ResolvedTypeKind::Optional(optional)) => self
                        .type_interner
                        .optional(optional)
                        .map(|entry| entry.payload.kind)
                        .map_or(Invalid, Known),
                    Known(_) | Invalid => Invalid,
                    Unknown => Unknown,
                }
            }
            ResolvedExpression::Grouped(grouped) => {
                self.provisional_expression_type(&grouped.expression)
            }
            ResolvedExpression::Unary(unary) => match &unary.selection {
                Some(resolution) => provisional_operator_output(resolution),
                None => self.provisional_expression_type(&unary.operand),
            },
            ResolvedExpression::Binary(binary) => match &binary.selection {
                Some(resolution) => provisional_operator_output(resolution),
                None => match binary.operator {
                    ResolvedBinaryOperator::Equal
                    | ResolvedBinaryOperator::NotEqual
                    | ResolvedBinaryOperator::LessThan
                    | ResolvedBinaryOperator::LessEqual
                    | ResolvedBinaryOperator::GreaterThan
                    | ResolvedBinaryOperator::GreaterEqual => Known(ResolvedTypeKind::Bool),
                    _ => self.provisional_expression_type(&binary.left),
                },
            },
            ResolvedExpression::Logical(_) => Known(ResolvedTypeKind::Bool),
            ResolvedExpression::PrimitiveCast(cast) => Known(match cast.target {
                ResolvedPrimitiveType::I64 => ResolvedTypeKind::I64,
                ResolvedPrimitiveType::U64 => ResolvedTypeKind::U64,
                ResolvedPrimitiveType::U8 => ResolvedTypeKind::U8,
                ResolvedPrimitiveType::F64 => ResolvedTypeKind::F64,
                ResolvedPrimitiveType::Bool => ResolvedTypeKind::Bool,
            }),
            ResolvedExpression::ObjectCast(cast) => match cast.target_mode {
                ResolvedObjectCastTargetMode::Plain => Known(cast.target.kind),
                ResolvedObjectCastTargetMode::Shared { .. } => match cast.target.kind {
                    ResolvedTypeKind::Class(class) => {
                        Known(ResolvedTypeKind::Shared(ResolvedSharedTarget::Class(class)))
                    }
                    ResolvedTypeKind::Interface(interface) => Known(ResolvedTypeKind::Shared(
                        ResolvedSharedTarget::Interface(interface),
                    )),
                    ResolvedTypeKind::Obj => {
                        Known(ResolvedTypeKind::Shared(ResolvedSharedTarget::Obj))
                    }
                    _ => Invalid,
                },
            },
            ResolvedExpression::ArrayConstruction(construction) => {
                let ResolvedTypeKind::Array(array) = construction.array_type.kind else {
                    return Invalid;
                };
                Known(if construction.new_span.is_some() {
                    ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array))
                } else {
                    ResolvedTypeKind::Array(array)
                })
            }
            ResolvedExpression::ArrayProjection(projection) => {
                let receiver = match self.provisional_expression_type(&projection.receiver) {
                    Known(receiver) => receiver,
                    Unknown => return Unknown,
                    Invalid => return Invalid,
                };
                let array = match (projection.operator, receiver) {
                    (
                        ResolvedArrayProjectionOperator::Ordinary { .. },
                        ResolvedTypeKind::Array(array),
                    )
                    | (
                        ResolvedArrayProjectionOperator::Shared { .. },
                        ResolvedTypeKind::Shared(ResolvedSharedTarget::Array(array)),
                    ) => array,
                    _ => return Invalid,
                };
                match projection.bounds {
                    ResolvedArrayProjectionBounds::Index(_) => self
                        .type_interner
                        .array(array)
                        .map(|entry| entry.element.kind)
                        .map_or(Invalid, Known),
                    ResolvedArrayProjectionBounds::Slice { .. } => {
                        Known(ResolvedTypeKind::Array(array))
                    }
                }
            }
            ResolvedExpression::ArrayLength(_) => Known(ResolvedTypeKind::U64),
            ResolvedExpression::FieldAccess(access) => self
                .environment
                .classes
                .get(access.field.class())
                .and_then(|class| class.field(access.field))
                .map(|field| field.type_syntax.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::StaticFieldAccess(access) => self
                .environment
                .classes
                .get(access.field.class())
                .and_then(|class| class.static_field(access.field))
                .map(|field| field.type_syntax.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::FunctionReference(reference) => {
                Known(ResolvedTypeKind::Function(reference.function_type))
            }
            ResolvedExpression::IndirectCall(call) => self
                .type_interner
                .function(call.function_type)
                .map(|signature| signature.result.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::DirectCall(call) => self
                .environment
                .functions
                .get(call.function)
                .map(|declaration| declaration.return_type.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::StaticCall(call) => self
                .environment
                .classes
                .get(call.method.class())
                .and_then(|class| class.method(call.method))
                .map(|method| method.return_type.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::MethodCall(call) => self
                .environment
                .classes
                .get(call.method.class())
                .and_then(|class| class.method(call.method))
                .map(|method| method.return_type.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::InterfaceCall(call) => self
                .environment
                .interfaces
                .get(call.interface)
                .and_then(|interface| interface.requirements.get(call.requirement.index()))
                .map(|requirement| requirement.return_type.kind)
                .map_or(Invalid, Known),
            ResolvedExpression::Allocation(allocation) => Known(ResolvedTypeKind::Shared(
                ResolvedSharedTarget::Class(allocation.class),
            )),
            ResolvedExpression::OptionalBoxAllocation(allocation) => Known(
                ResolvedTypeKind::Shared(ResolvedSharedTarget::OptionalBox(allocation.target)),
            ),
            ResolvedExpression::Construct(construction) => {
                Known(ResolvedTypeKind::Class(construction.class))
            }
        }
    }
}

fn provisional_operator_output(
    resolution: &ResolvedOperatorResolution,
) -> ProvisionalExpressionType {
    resolution
        .selected()
        .map(|selection| ProvisionalExpressionType::Known(selection.output))
        .unwrap_or(ProvisionalExpressionType::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{InterfaceId, InterfaceRequirementId},
        source::SourceDatabase,
    };

    fn operator_selection(output: ResolvedTypeKind) -> ResolvedOperatorSelection {
        let interface = InterfaceId::new(0);
        let mut sources = SourceDatabase::new();
        let source = sources.add("test.ska", "");
        ResolvedOperatorSelection {
            protocol: CanonicalOperatorProtocol::Add,
            interface,
            requirement: InterfaceRequirementId::new(interface, 0),
            rhs: Some(ResolvedTypeKind::I64),
            output,
            origin_span: Span::empty(source, 0),
        }
    }

    #[test]
    fn operator_output_is_known_only_for_one_selected_candidate() {
        let selected = operator_selection(ResolvedTypeKind::U64);
        let resolution = ResolvedOperatorResolution {
            protocol: CanonicalOperatorProtocol::Add,
            candidates: vec![selected],
            incompatible_rhs: Vec::new(),
        };
        assert_eq!(
            provisional_operator_output(&resolution),
            ProvisionalExpressionType::Known(ResolvedTypeKind::U64)
        );

        for candidates in [Vec::new(), vec![selected, selected]] {
            assert_eq!(
                provisional_operator_output(&ResolvedOperatorResolution {
                    protocol: CanonicalOperatorProtocol::Add,
                    candidates,
                    incompatible_rhs: Vec::new(),
                }),
                ProvisionalExpressionType::Invalid
            );
        }
    }

    #[test]
    fn only_known_provisional_types_convert_to_an_option() {
        assert_eq!(
            ProvisionalExpressionType::Known(ResolvedTypeKind::Bool).known(),
            Some(ResolvedTypeKind::Bool)
        );
        assert_eq!(ProvisionalExpressionType::Unknown.known(), None);
        assert_eq!(ProvisionalExpressionType::Invalid.known(), None);
    }
}

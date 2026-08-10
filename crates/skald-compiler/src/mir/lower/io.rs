//! Lowering of typed standard-I/O operations to semantic MIR.

use super::*;
use crate::hir::{HirArrayAliasArgument, HirArrayAliasSource, HirIoOperation};

impl BodyLowerer<'_> {
    pub(super) fn lower_io(
        &mut self,
        operation: &HirIoOperation,
        span: crate::source::Span,
    ) -> ValueId {
        let operation = match operation {
            HirIoOperation::StandardHandle { stream } => MirIoOperation::StandardHandle {
                stream: self.lower_io_scalar(stream, MirType::U8),
            },
            HirIoOperation::Open { path, mode } => {
                let path = self.lower_io_buffer(path);
                let mode = self.lower_io_scalar(mode, MirType::U8);
                MirIoOperation::Open { path, mode }
            }
            HirIoOperation::Read {
                handle,
                destination,
                offset,
            } => {
                let handle_span = handle.span;
                let handle = self.lower_io_scalar_to_spill(handle, MirType::I64);
                let destination = self.lower_io_buffer(destination);
                let offset = self.lower_array_range_offset(
                    destination.place.clone(),
                    destination.array,
                    offset,
                );
                let handle = self.reload_io_scalar(handle, MirType::I64, handle_span);
                MirIoOperation::Read {
                    handle,
                    destination,
                    offset,
                }
            }
            HirIoOperation::Write {
                handle,
                source,
                offset,
            } => {
                let handle_span = handle.span;
                let handle = self.lower_io_scalar_to_spill(handle, MirType::I64);
                let source = self.lower_io_buffer(source);
                let offset =
                    self.lower_array_range_offset(source.place.clone(), source.array, offset);
                let handle = self.reload_io_scalar(handle, MirType::I64, handle_span);
                MirIoOperation::Write {
                    handle,
                    source,
                    offset,
                }
            }
            HirIoOperation::Close { handle } => MirIoOperation::Close {
                handle: self.lower_io_scalar(handle, MirType::I64),
            },
        };
        let result = self.new_value(MirType::I64, span);
        self.emit(MirInstruction::Io(MirIoInstruction {
            result,
            operation,
            span,
        }));
        result
    }

    fn lower_io_scalar(&mut self, expression: &HirExpression, ty: MirType) -> ValueId {
        debug_assert_eq!(self.lower_type(expression.ty), ty);
        self.lower_expression(expression)
            .expect("typed standard-I/O scalar input must produce a MIR value")
    }

    fn lower_io_scalar_to_spill(&mut self, expression: &HirExpression, ty: MirType) -> StorageId {
        let value = self.lower_io_scalar(expression, ty);
        self.spill_scalar(value, ty, expression.span).0
    }

    fn reload_io_scalar(
        &mut self,
        storage: StorageId,
        ty: MirType,
        span: crate::source::Span,
    ) -> ValueId {
        self.assign(MirRvalueKind::Load(storage.into()), ty, span)
    }

    fn lower_io_buffer(&mut self, argument: &HirArrayAliasArgument) -> MirIoBuffer {
        let array = match argument.target {
            Type::Array(array) => array,
            _ => unreachable!("typed standard-I/O buffer must be an array alias"),
        };
        let (place, anchor) = match &argument.source {
            HirArrayAliasSource::Whole(receiver) => self.lower_array_receiver_with_anchor(receiver),
            HirArrayAliasSource::Element(element) => {
                self.lower_array_alias_element_place_with_anchor(element, argument.access)
            }
            HirArrayAliasSource::OptionalPayload {
                source,
                optional,
                array,
            } => self.lower_optional_array_alias_place_with_anchor(
                source,
                *optional,
                *array,
                argument.span,
            ),
        };
        MirIoBuffer {
            place,
            anchor,
            array,
            access: type_operations::lower_access(argument.access),
        }
    }
}

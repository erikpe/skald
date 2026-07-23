//! Destination-oriented lowering for verified object copy operations.

use crate::{
    backend::BackendError,
    identity::CallableId,
    mir::{
        MirArgument, MirCopyAssignment, MirCopyConstruction, MirEndFullExpression, MirPlace,
        MirSelectedCopyOperation, MirSynthesizedFieldCopy, MirType,
    },
};

use super::{
    super::machine::{ByteRegister, Instruction, Register, XmmRegister},
    value, InstructionSelector,
};

impl InstructionSelector<'_, '_> {
    pub(super) fn select_copy_construction(
        &mut self,
        copy: &MirCopyConstruction,
    ) -> Result<(), BackendError> {
        self.select_construction_operation(
            copy.operation,
            copy.destination.clone(),
            copy.source.clone(),
        )
    }

    pub(super) fn select_copy_assignment(
        &mut self,
        copy: &MirCopyAssignment,
    ) -> Result<(), BackendError> {
        self.select_assignment_operation(
            copy.operation,
            copy.destination.clone(),
            copy.source.clone(),
        )
    }

    pub(super) fn select_end_full_expression(
        &mut self,
        end: &MirEndFullExpression,
    ) -> Result<(), BackendError> {
        for cleanup in &end.temporaries {
            self.select_cleanup(cleanup)?;
        }
        Ok(())
    }

    fn select_construction_operation(
        &mut self,
        operation: MirSelectedCopyOperation<crate::identity::InitializerId>,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        match operation {
            MirSelectedCopyOperation::User(initializer) => {
                let class = initializer.class();
                let base = match &self
                    .program
                    .class(class)
                    .expect("verified user copy class must exist")
                    .copy_constructor
                {
                    crate::mir::MirCopyCapability::User(copy) => copy.base,
                    _ => unreachable!("verified copy operation must match its class capability"),
                };
                if let Some(base) = base {
                    self.select_construction_operation(
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                    )?;
                }
                self.select_copy_call(CallableId::Initializer(initializer), &destination, &source)
            }
            MirSelectedCopyOperation::Synthesized(class) => {
                let (base, fields) = match &self
                    .program
                    .class(class)
                    .expect("verified synthesized copy class must exist")
                    .copy_constructor
                {
                    crate::mir::MirCopyCapability::Synthesized(copy) => {
                        (copy.base, copy.fields.clone())
                    }
                    _ => unreachable!("verified copy operation must match its class capability"),
                };
                if let Some(base) = base {
                    self.select_construction_operation(
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                    )?;
                }
                for field in fields {
                    match field {
                        MirSynthesizedFieldCopy::Primitive { field } => {
                            self.select_primitive_copy(
                                destination.clone().project_field(field),
                                source.clone().project_field(field),
                                self.program
                                    .field(field)
                                    .expect("verified copy field must exist")
                                    .ty,
                            )?;
                        }
                        MirSynthesizedFieldCopy::Class { field, operation } => {
                            self.select_construction_operation(
                                operation,
                                destination.clone().project_field(field),
                                source.clone().project_field(field),
                            )?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn select_assignment_operation(
        &mut self,
        operation: MirSelectedCopyOperation<crate::identity::CopyAssignmentId>,
        destination: MirPlace,
        source: MirPlace,
    ) -> Result<(), BackendError> {
        match operation {
            MirSelectedCopyOperation::User(assignment) => {
                let class = assignment.class();
                let base = match &self
                    .program
                    .class(class)
                    .expect("verified user assignment class must exist")
                    .copy_assignment
                {
                    crate::mir::MirCopyCapability::User(copy) => copy.base,
                    _ => unreachable!("verified copy operation must match its class capability"),
                };
                if let Some(base) = base {
                    self.select_assignment_operation(
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                    )?;
                }
                self.select_copy_call(
                    CallableId::CopyAssignment(assignment),
                    &destination,
                    &source,
                )
            }
            MirSelectedCopyOperation::Synthesized(class) => {
                let (base, fields) = match &self
                    .program
                    .class(class)
                    .expect("verified synthesized copy class must exist")
                    .copy_assignment
                {
                    crate::mir::MirCopyCapability::Synthesized(copy) => {
                        (copy.base, copy.fields.clone())
                    }
                    _ => unreachable!("verified copy operation must match its class capability"),
                };
                if let Some(base) = base {
                    self.select_assignment_operation(
                        base.operation,
                        destination.clone().project_base(base.base),
                        source.clone().project_base(base.base),
                    )?;
                }
                for field in fields {
                    match field {
                        MirSynthesizedFieldCopy::Primitive { field } => {
                            self.select_primitive_copy(
                                destination.clone().project_field(field),
                                source.clone().project_field(field),
                                self.program
                                    .field(field)
                                    .expect("verified copy field must exist")
                                    .ty,
                            )?;
                        }
                        MirSynthesizedFieldCopy::Class { field, operation } => {
                            self.select_assignment_operation(
                                operation,
                                destination.clone().project_field(field),
                                source.clone().project_field(field),
                            )?;
                        }
                    }
                }
                Ok(())
            }
        }
    }

    fn select_copy_call(
        &mut self,
        target: CallableId,
        destination: &MirPlace,
        source: &MirPlace,
    ) -> Result<(), BackendError> {
        self.select_callable(
            target,
            None,
            Some(destination),
            &[MirArgument::Place(source.clone())],
            None,
        )
    }

    fn select_primitive_copy(
        &mut self,
        destination: MirPlace,
        source: MirPlace,
        ty: MirType,
    ) -> Result<(), BackendError> {
        // Preserve the destination address in a caller-saved register while
        // source addressing uses the established `%rax`/`%r11` scratch path.
        self.materialize_place_address(&destination, Register::Rdx)?;
        let (_, source) = self.frame_place(&source)?;
        let destination = value::memory(Register::Rdx, 0);
        match ty {
            MirType::F64 => {
                value::load_float(
                    value::float_operand(source),
                    XmmRegister::Xmm14,
                    self.output,
                );
                value::store_float(
                    XmmRegister::Xmm14,
                    value::float_operand(destination),
                    self.output,
                );
            }
            MirType::U8 | MirType::Bool => {
                value::load_byte_rax(source, self.output);
                self.output.push(Instruction::MoveByte {
                    source: ByteRegister::Al,
                    destination,
                });
            }
            MirType::I64 | MirType::U64 => {
                value::load_rax(source, self.output);
                value::store_rax(destination, self.output);
            }
            MirType::Class(_) | MirType::Obj | MirType::Unit => {
                unreachable!("verified primitive copy step must have a payload primitive type")
            }
        }
        Ok(())
    }
}

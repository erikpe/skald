//! Address-based array element lifecycle bodies.
//!
//! Generated array loops have raw backing addresses rather than MIR places.  This
//! module keeps that representation boundary local and expresses every lifecycle
//! action as ordinary checked LIR. Recursive class and array graphs are calls to
//! separately reserved helpers, so body construction never recurses in the host.

use super::{begin, byte_offset, constant_u64, edge, finish, reserve_block};
use crate::{
    backend::{
        lir::{
            Call, CallArgument, CallAttribution, CallTarget, Constant, DraftBuilder,
            MemoryRepresentation, Operation, Terminator, ValueHandle, VerifiedCallable,
        },
        plan::{
            ArrayCopyElementFact, ArrayDefaultElementFact, ArrayDestroyElementFact, ArtifactId,
            CallableBinding, ComponentRole, DataKey, HelperFamily, LayoutId, LirCallableId,
            OptionalStorageFact, PlanError, PlanView, RuntimeService, ScalarType, SemanticType,
        },
        planning::AdmittedProgram,
    },
    identity::{ClassId, OptionalTypeId},
    mir::{MirCopyCapability, MirSynthesizedFieldCopy},
    primitive_comparison::PrimitiveComparisonPredicate,
};

use super::super::LowerError;

pub(super) fn initializer<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let mut emitter = Emitter::new(admitted, owner)?;
    let [backing, index] = emitter.inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let address = emitter.element_address(*backing, *index, array)?;
    let current = emitter.entry;
    let Some(default) = array.default else {
        return emitter.finish(current);
    };
    match default {
        ArrayDefaultElementFact::Primitive => {
            emitter.store_zero(current, address, array.element, array.element_layout)?;
        }
        ArrayDefaultElementFact::OptionalAbsent => {
            let SemanticType::Optional(optional) = array.element else {
                return Err(PlanError::InvalidDomain.into());
            };
            emitter.store_optional_absent(current, address, optional)?;
        }
        ArrayDefaultElementFact::Class { class, initializer } => {
            emitter.call_object_source(current, initializer.into(), class, address, None)?;
        }
        ArrayDefaultElementFact::ArrayEmpty(_) => {
            emitter.store_null(current, address)?;
        }
        ArrayDefaultElementFact::SharedClass { class, initializer } => {
            let fact = emitter
                .plan
                .semantic()
                .class(class)
                .ok_or(PlanError::UnknownDeclaration)?;
            let handle = emitter.allocate(current, fact.shared_allocation.byte_count)?;
            let payload = byte_offset(
                &mut emitter.builder,
                current,
                handle,
                fact.shared_allocation.payload_offset,
            )?;
            emitter.call_object_source(current, initializer.into(), class, payload, None)?;
            emitter.publish_shared(current, handle, DataKey::ClassDispatch(class))?;
            emitter.store_pointer(current, address, handle)?;
        }
        ArrayDefaultElementFact::SharedArrayEmpty(inner) => {
            let inner = emitter
                .plan
                .semantic()
                .array(inner)
                .ok_or(PlanError::UnknownDeclaration)?;
            let handle = emitter.allocate(current, inner.shared_element_offset as u64)?;
            emitter.store_u64_at(current, handle, inner.shared_length_offset, 0)?;
            emitter.publish_shared(current, handle, DataKey::ArrayDescriptor(inner.array))?;
            emitter.store_pointer(current, address, handle)?;
        }
        ArrayDefaultElementFact::SharedOptionalBoxAbsent(optional_box) => {
            let fact = emitter
                .plan
                .semantic()
                .optional_box(optional_box)
                .ok_or(PlanError::UnknownDeclaration)?;
            let allocation = fact.allocation.ok_or(PlanError::InvalidLayout)?;
            let optional = fact.exact_optional.ok_or(PlanError::InvalidDomain)?;
            let handle = emitter.allocate(current, allocation.byte_count)?;
            let payload = byte_offset(
                &mut emitter.builder,
                current,
                handle,
                allocation.payload_offset,
            )?;
            emitter.store_optional_absent(current, payload, optional)?;
            emitter.publish_shared(
                current,
                handle,
                DataKey::OptionalBoxDescriptor(optional_box),
            )?;
            emitter.store_pointer(current, address, handle)?;
        }
    }
    emitter.finish(current)
}

pub(super) fn copier<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let mut emitter = Emitter::new(admitted, owner)?;
    let [destination, source, destination_index, source_index] = emitter.inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let (destination, source, destination_index, source_index) =
        (*destination, *source, *destination_index, *source_index);
    let destination = emitter.element_address(destination, destination_index, array)?;
    let source = emitter.element_address(source, source_index, array)?;
    let current = emitter.entry;
    let Some(copy) = array.copy else {
        return emitter.finish(current);
    };
    let complete = match copy {
        ArrayCopyElementFact::Primitive => {
            emitter.copy_scalar(
                current,
                destination,
                source,
                array.element,
                array.element_layout,
            )?;
            current
        }
        ArrayCopyElementFact::OptionalPrimitive => {
            let SemanticType::Optional(optional) = array.element else {
                return Err(PlanError::InvalidDomain.into());
            };
            emitter.copy_optional(current, destination, source, optional)?
        }
        ArrayCopyElementFact::Class { class, .. } => {
            emitter.call_class_copy(current, class, destination, source)?;
            current
        }
        ArrayCopyElementFact::OptionalClass { .. } | ArrayCopyElementFact::Optional(_) => {
            let SemanticType::Optional(optional) = array.element else {
                return Err(PlanError::InvalidDomain.into());
            };
            emitter.copy_optional(current, destination, source, optional)?
        }
        ArrayCopyElementFact::Array(inner) => {
            emitter.copy_array(current, destination, source, inner)?;
            current
        }
        ArrayCopyElementFact::Shared(_) | ArrayCopyElementFact::OptionalShared(_) => {
            emitter.copy_shared_nullable(current, destination, source)?
        }
    };
    emitter.finish(complete)
}

pub(super) fn destroyer<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    array: &crate::backend::plan::ArrayLayoutFact,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let mut emitter = Emitter::new(admitted, owner)?;
    let [backing, index] = emitter.inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let address = emitter.element_address(*backing, *index, array)?;
    let current = emitter.entry;
    let complete = match array.destruction {
        ArrayDestroyElementFact::Trivial => current,
        ArrayDestroyElementFact::Class(class) => {
            emitter.call_class_finalizer(current, class, address)?;
            current
        }
        ArrayDestroyElementFact::OptionalClass(_)
        | ArrayDestroyElementFact::OptionalShared(_)
        | ArrayDestroyElementFact::Optional(_) => {
            let SemanticType::Optional(optional) = array.element else {
                return Err(PlanError::InvalidDomain.into());
            };
            emitter.destroy_optional(current, address, optional)?
        }
        ArrayDestroyElementFact::Array(inner) => {
            emitter.release_array_at(current, address, inner)?;
            current
        }
        ArrayDestroyElementFact::Shared(_) => {
            emitter.release_shared_at(current, address)?;
            current
        }
    };
    emitter.finish(complete)
}

pub(super) fn raw_class_copy<'plan>(
    admitted: &'plan AdmittedProgram<'_>,
    owner: CallableBinding<'plan>,
    layout: LayoutId,
) -> Result<VerifiedCallable<'plan>, LowerError> {
    let class = admitted
        .plan()
        .view()
        .semantic()
        .classes
        .iter()
        .find(|class| class.complete_layout == layout)
        .map(|class| class.class)
        .ok_or(PlanError::UnknownDeclaration)?;
    let mut emitter = Emitter::new(admitted, owner)?;
    let [destination, source] = emitter.inputs.as_slice() else {
        return Err(PlanError::InvalidSignature.into());
    };
    let destination = *destination;
    let source = *source;
    let current = emitter.entry;
    let capability = admitted
        .program()
        .class(class)
        .ok_or(PlanError::UnknownDeclaration)?
        .copy_constructor
        .clone();
    match capability {
        MirCopyCapability::User(copy) => {
            if let Some(base) = copy.base {
                let offset = emitter.class_base_offset(class, base.base)?;
                let destination = emitter.offset(current, destination, offset)?;
                let source = emitter.offset(current, source, offset)?;
                emitter.call_class_copy(current, base.base, destination, source)?;
            }
            emitter.call_object_source(
                current,
                copy.operation.into(),
                class,
                destination,
                Some(source),
            )?;
        }
        MirCopyCapability::Synthesized(copy) => {
            if let Some(base) = copy.base {
                let offset = emitter.class_base_offset(class, base.base)?;
                let destination = emitter.offset(current, destination, offset)?;
                let source = emitter.offset(current, source, offset)?;
                emitter.call_class_copy(current, base.base, destination, source)?;
            }
            let mut current = current;
            for field in copy.fields {
                current = emitter.copy_class_field(current, destination, source, field)?;
            }
            return emitter.finish(current);
        }
        MirCopyCapability::Unavailable => return Err(PlanError::InvalidDomain.into()),
    }
    emitter.finish(current)
}

struct Emitter<'plan> {
    plan: PlanView<'plan>,
    boundary: LirCallableId,
    builder: DraftBuilder<'plan>,
    entry: crate::backend::lir::BlockHandle<'plan>,
    inputs: Vec<ValueHandle<'plan>>,
}

impl<'plan> Emitter<'plan> {
    fn new(
        admitted: &'plan AdmittedProgram<'_>,
        owner: CallableBinding<'plan>,
    ) -> Result<Self, LowerError> {
        let plan = admitted.plan().view();
        let boundary = owner.key();
        let (builder, entry, inputs) = begin(owner)?;
        Ok(Self {
            plan,
            boundary,
            builder,
            entry,
            inputs,
        })
    }

    fn finish(
        self,
        block: crate::backend::lir::BlockHandle<'plan>,
    ) -> Result<VerifiedCallable<'plan>, LowerError> {
        finish(self.builder, block, vec![])
    }

    fn element_address(
        &mut self,
        backing: ValueHandle<'plan>,
        index: ValueHandle<'plan>,
        array: &crate::backend::plan::ArrayLayoutFact,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let base = backing;
        let stride = constant_u64(&mut self.builder, self.entry, array.stride as u64)?;
        let displacement = self.builder.append(
            self.entry,
            Operation::Binary {
                operation: crate::backend::lir::BinaryOperation::Multiply,
                left: index,
                right: stride,
            },
        )?[0];
        Ok(self.builder.append(
            self.entry,
            Operation::ByteOffset {
                base,
                offset: displacement,
            },
        )?[0])
    }

    fn offset(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        offset: usize,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        byte_offset(&mut self.builder, block, base, offset)
    }

    fn representation(
        &self,
        ty: SemanticType,
        layout: LayoutId,
    ) -> Result<MemoryRepresentation, LowerError> {
        let layout = self.plan.layout(self.plan.layout_id(layout.index())?)?;
        let scalar = match ty {
            SemanticType::I64 => ScalarType::I64,
            SemanticType::U64 => ScalarType::U64,
            SemanticType::U8 => ScalarType::U8,
            SemanticType::F64 => ScalarType::F64,
            SemanticType::Bool => ScalarType::Bool,
            SemanticType::Array(_) | SemanticType::Shared(_) | SemanticType::Function(_) => {
                ScalarType::DataAddress
            }
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        Ok(MemoryRepresentation {
            scalar,
            bytes: layout.size,
            alignment: layout.alignment,
        })
    }

    fn pointer_representation(&self) -> MemoryRepresentation {
        let data = self.plan.profile().data_layout;
        MemoryRepresentation {
            scalar: ScalarType::DataAddress,
            bytes: data.pointer_bytes,
            alignment: data.pointer_alignment,
        }
    }

    fn state_representation(&self) -> MemoryRepresentation {
        let data = self.plan.profile().data_layout;
        MemoryRepresentation {
            scalar: ScalarType::U64,
            bytes: data.pointer_bytes,
            alignment: data.pointer_alignment,
        }
    }

    fn copy_scalar(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
        ty: SemanticType,
        layout: LayoutId,
    ) -> Result<(), LowerError> {
        let representation = self.representation(ty, layout)?;
        let value = self.builder.append(
            block,
            Operation::Load {
                address: source,
                representation,
            },
        )?[0];
        self.builder.append(
            block,
            Operation::Store {
                address: destination,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    fn store_zero(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        ty: SemanticType,
        layout: LayoutId,
    ) -> Result<(), LowerError> {
        let representation = self.representation(ty, layout)?;
        let constant = match representation.scalar {
            ScalarType::I64 => Constant::I64(0),
            ScalarType::U64 => Constant::U64(0),
            ScalarType::U8 => Constant::U8(0),
            ScalarType::Bool => Constant::Bool(false),
            ScalarType::F64 => Constant::F64(0),
            ScalarType::DataAddress => Constant::Null(ScalarType::DataAddress),
            _ => return Err(PlanError::InvalidDomain.into()),
        };
        let value = self.builder.append(block, Operation::Constant(constant))?[0];
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation,
            },
        )?;
        Ok(())
    }

    fn store_pointer(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        value: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation: self.pointer_representation(),
            },
        )?;
        Ok(())
    }

    fn store_null(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let null = self.builder.append(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        self.store_pointer(block, address, null)
    }

    fn store_u64_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        base: ValueHandle<'plan>,
        offset: usize,
        bits: u64,
    ) -> Result<(), LowerError> {
        let address = self.offset(block, base, offset)?;
        let value = constant_u64(&mut self.builder, block, bits)?;
        self.builder.append(
            block,
            Operation::Store {
                address,
                value,
                representation: self.state_representation(),
            },
        )?;
        Ok(())
    }

    fn load_pointer(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        Ok(self.builder.append(
            block,
            Operation::Load {
                address,
                representation: self.pointer_representation(),
            },
        )?[0])
    }

    fn store_optional_absent(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        optional: OptionalTypeId,
    ) -> Result<(), LowerError> {
        let fact = self
            .plan
            .semantic()
            .optional(optional)
            .ok_or(PlanError::UnknownDeclaration)?;
        if fact.nullable_niche {
            self.store_null(block, address)
        } else {
            self.store_u64_at(
                block,
                address,
                fact.state_offset.ok_or(PlanError::InvalidLayout)?,
                0,
            )
        }
    }

    fn copy_optional(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
        optional: OptionalTypeId,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let fact = self
            .plan
            .semantic()
            .optional(optional)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        if fact.nullable_niche {
            return self.copy_shared_nullable(block, destination, source);
        }
        let state_offset = fact.state_offset.ok_or(PlanError::InvalidLayout)?;
        let source_state = self.offset(block, source, state_offset)?;
        let source_state = self.builder.append(
            block,
            Operation::Load {
                address: source_state,
                representation: self.state_representation(),
            },
        )?[0];
        let zero = constant_u64(&mut self.builder, block, 0)?;
        let present = self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: source_state,
                right: zero,
            },
        )?[0];
        let present_block = reserve_block(&mut self.builder)?;
        let absent_block = reserve_block(&mut self.builder)?;
        let complete = reserve_block(&mut self.builder)?;
        self.builder.terminate(
            block,
            Terminator::Branch {
                condition: present,
                true_edge: edge(present_block),
                false_edge: edge(absent_block),
            },
        )?;
        self.store_u64_at(absent_block, destination, state_offset, 0)?;
        self.builder
            .terminate(absent_block, Terminator::Jump(edge(complete)))?;
        let destination_payload = self.offset(present_block, destination, fact.payload_offset)?;
        let source_payload = self.offset(present_block, source, fact.payload_offset)?;
        let present_block =
            self.copy_optional_payload(present_block, destination_payload, source_payload, &fact)?;
        self.store_u64_at(present_block, destination, state_offset, 1)?;
        self.builder
            .terminate(present_block, Terminator::Jump(edge(complete)))?;
        Ok(complete)
    }

    fn copy_optional_payload(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
        fact: &crate::backend::plan::OptionalLayoutFact,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        match fact.storage {
            OptionalStorageFact::Scalar => {
                self.copy_scalar(
                    block,
                    destination,
                    source,
                    fact.payload,
                    fact.payload_layout,
                )?;
                Ok(block)
            }
            OptionalStorageFact::InlineClass(class) => {
                self.call_class_copy(block, class, destination, source)?;
                Ok(block)
            }
            OptionalStorageFact::InlineArray(array) => {
                self.copy_array(block, destination, source, array)?;
                Ok(block)
            }
            OptionalStorageFact::SharedOwner(_) => {
                self.copy_shared_nullable(block, destination, source)
            }
            OptionalStorageFact::Nested(inner) => {
                self.copy_optional(block, destination, source, inner)
            }
        }
    }

    fn destroy_optional(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        optional: OptionalTypeId,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let fact = self
            .plan
            .semantic()
            .optional(optional)
            .cloned()
            .ok_or(PlanError::UnknownDeclaration)?;
        if fact.nullable_niche {
            return self.release_shared_nullable_at(block, address);
        }
        let state = self.offset(
            block,
            address,
            fact.state_offset.ok_or(PlanError::InvalidLayout)?,
        )?;
        let state = self.builder.append(
            block,
            Operation::Load {
                address: state,
                representation: self.state_representation(),
            },
        )?[0];
        let zero = constant_u64(&mut self.builder, block, 0)?;
        let present = self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: state,
                right: zero,
            },
        )?[0];
        let body = reserve_block(&mut self.builder)?;
        let complete = reserve_block(&mut self.builder)?;
        self.builder.terminate(
            block,
            Terminator::Branch {
                condition: present,
                true_edge: edge(body),
                false_edge: edge(complete),
            },
        )?;
        let payload = self.offset(body, address, fact.payload_offset)?;
        let body = match fact.storage {
            OptionalStorageFact::Scalar => body,
            OptionalStorageFact::InlineClass(class) => {
                self.call_class_finalizer(body, class, payload)?;
                body
            }
            OptionalStorageFact::InlineArray(array) => {
                self.release_array_at(body, payload, array)?;
                body
            }
            OptionalStorageFact::SharedOwner(_) => {
                self.release_shared_at(body, payload)?;
                body
            }
            OptionalStorageFact::Nested(inner) => self.destroy_optional(body, payload, inner)?,
        };
        self.builder
            .terminate(body, Terminator::Jump(edge(complete)))?;
        Ok(complete)
    }

    fn copy_array(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), LowerError> {
        let handle = self.load_pointer(block, source)?;
        let result = self.call_helper(
            block,
            self.array_helper(array, HelperFamily::ArrayClone)?,
            vec![handle],
        )?;
        let [handle] = result.as_slice() else {
            return Err(PlanError::InvalidSignature.into());
        };
        self.store_pointer(block, destination, *handle)
    }

    fn release_array_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
        array: crate::identity::ArrayTypeId,
    ) -> Result<(), LowerError> {
        let handle = self.load_pointer(block, address)?;
        self.call_helper(
            block,
            self.array_helper(array, HelperFamily::ArrayRelease)?,
            vec![handle],
        )?;
        Ok(())
    }

    fn copy_shared(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let handle = self.load_pointer(block, source)?;
        self.call_helper(
            block,
            self.owner_helper(HelperFamily::Retain)?,
            vec![handle],
        )?;
        self.store_pointer(block, destination, handle)
    }

    fn copy_shared_nullable(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let handle = self.load_pointer(block, source)?;
        let null = self.builder.append(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let present = self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: handle,
                right: null,
            },
        )?[0];
        let retain = reserve_block(&mut self.builder)?;
        let absent = reserve_block(&mut self.builder)?;
        let complete = reserve_block(&mut self.builder)?;
        self.builder.terminate(
            block,
            Terminator::Branch {
                condition: present,
                true_edge: edge(retain),
                false_edge: edge(absent),
            },
        )?;
        self.call_helper(
            retain,
            self.owner_helper(HelperFamily::Retain)?,
            vec![handle],
        )?;
        self.store_pointer(retain, destination, handle)?;
        self.builder
            .terminate(retain, Terminator::Jump(edge(complete)))?;
        self.store_pointer(absent, destination, null)?;
        self.builder
            .terminate(absent, Terminator::Jump(edge(complete)))?;
        Ok(complete)
    }

    fn release_shared_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let handle = self.load_pointer(block, address)?;
        self.call_helper(
            block,
            self.owner_helper(HelperFamily::Release)?,
            vec![handle],
        )?;
        Ok(())
    }

    fn release_shared_nullable_at(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        address: ValueHandle<'plan>,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let handle = self.load_pointer(block, address)?;
        let null = self.builder.append(
            block,
            Operation::Constant(Constant::Null(ScalarType::DataAddress)),
        )?[0];
        let present = self.builder.append(
            block,
            Operation::Compare {
                predicate: PrimitiveComparisonPredicate::NotEqual,
                left: handle,
                right: null,
            },
        )?[0];
        let release = reserve_block(&mut self.builder)?;
        let complete = reserve_block(&mut self.builder)?;
        self.builder.terminate(
            block,
            Terminator::Branch {
                condition: present,
                true_edge: edge(release),
                false_edge: edge(complete),
            },
        )?;
        self.call_helper(
            release,
            self.owner_helper(HelperFamily::Release)?,
            vec![handle],
        )?;
        self.builder
            .terminate(release, Terminator::Jump(edge(complete)))?;
        Ok(complete)
    }

    fn call_class_copy(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        class: ClassId,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let target = self.class_helper(class, HelperFamily::RawClassCopy)?;
        self.call_helper(block, target, vec![destination, source])?;
        Ok(())
    }

    fn call_class_finalizer(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        class: ClassId,
        address: ValueHandle<'plan>,
    ) -> Result<(), LowerError> {
        let target = self.class_helper(class, HelperFamily::ClassFinalizer)?;
        self.call_helper(block, target, vec![address])?;
        Ok(())
    }

    fn class_helper(
        &self,
        class: ClassId,
        family: HelperFamily,
    ) -> Result<LirCallableId, LowerError> {
        let layout = self
            .plan
            .semantic()
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?
            .complete_layout;
        self.helper(family, layout)
    }

    fn array_helper(
        &self,
        array: crate::identity::ArrayTypeId,
        family: HelperFamily,
    ) -> Result<LirCallableId, LowerError> {
        let layout = self
            .plan
            .semantic()
            .array(array)
            .ok_or(PlanError::UnknownDeclaration)?
            .descriptor_layout;
        self.helper(family, layout)
    }

    fn owner_helper(&self, family: HelperFamily) -> Result<LirCallableId, LowerError> {
        let layout = self
            .plan
            .semantic()
            .shared_header
            .ok_or(PlanError::InvalidLayout)?
            .handle_layout;
        self.helper(family, layout)
    }

    fn helper(&self, family: HelperFamily, layout: LayoutId) -> Result<LirCallableId, LowerError> {
        self.plan
            .resources()
            .generated
            .iter()
            .find_map(|fact| match fact.callable {
                LirCallableId::Helper(key) if key.family == family && key.layout == layout => {
                    Some(fact.callable)
                }
                _ => None,
            })
            .ok_or(PlanError::UnknownDeclaration.into())
    }

    fn call_helper(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        target: LirCallableId,
        values: Vec<ValueHandle<'plan>>,
    ) -> Result<Vec<ValueHandle<'plan>>, LowerError> {
        let binding = self.plan.callable(target)?;
        let signature = binding.signature_id();
        let roles = binding
            .signature()?
            .inputs
            .iter()
            .map(|component| component.role)
            .collect::<Vec<_>>();
        if roles.len() != values.len() {
            return Err(PlanError::InvalidSignature.into());
        }
        let arguments = roles
            .into_iter()
            .zip(values)
            .map(|(role, value)| CallArgument { role, value })
            .collect();
        Ok(self.builder.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature,
                arguments,
                attribution: CallAttribution::InheritedOperation {
                    boundary: self.boundary,
                },
            }),
        )?)
    }

    fn call_object_source(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        target: crate::identity::CallableId,
        class: ClassId,
        destination: ValueHandle<'plan>,
        source: Option<ValueHandle<'plan>>,
    ) -> Result<(), LowerError> {
        let target = LirCallableId::Source(target);
        let binding = self.plan.callable(target)?;
        let metadata = self.builder.append(
            block,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(DataKey::ClassDispatch(class)),
                ty: ScalarType::DataAddress,
            },
        )?[0];
        let arguments = binding
            .signature()?
            .inputs
            .iter()
            .map(|component| {
                let value = match component.role {
                    ComponentRole::ReceiverStatic | ComponentRole::ReceiverComplete => destination,
                    ComponentRole::ReceiverMetadata => metadata,
                    ComponentRole::AliasAddress(0) | ComponentRole::AliasComplete(0) => {
                        source.ok_or(PlanError::InvalidSignature)?
                    }
                    ComponentRole::AliasMetadata(0) => metadata,
                    _ => return Err(PlanError::InvalidSignature),
                };
                Ok(CallArgument {
                    role: component.role,
                    value,
                })
            })
            .collect::<Result<Vec<_>, PlanError>>()?;
        self.builder.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Callable(target)),
                signature: binding.signature_id(),
                arguments,
                attribution: CallAttribution::SourceBodyFromOmittedHelper {
                    boundary: self.boundary,
                },
            }),
        )?;
        Ok(())
    }

    fn allocate(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        bytes: u64,
    ) -> Result<ValueHandle<'plan>, LowerError> {
        let bytes = constant_u64(&mut self.builder, block, bytes)?;
        let target = ArtifactId::Runtime(RuntimeService::Allocate);
        let artifact = self
            .plan
            .artifact(self.plan.artifact_id(target)?, target.category())?;
        let values = self.builder.append(
            block,
            Operation::Call(Call {
                target: CallTarget::Direct(target),
                signature: artifact.signature.ok_or(PlanError::InvalidSignature)?,
                arguments: vec![CallArgument {
                    role: ComponentRole::RuntimeParameter(0),
                    value: bytes,
                }],
                attribution: CallAttribution::InheritedOperation {
                    boundary: self.boundary,
                },
            }),
        )?;
        values
            .into_iter()
            .next()
            .ok_or_else(|| PlanError::InvalidSignature.into())
    }

    fn publish_shared(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        handle: ValueHandle<'plan>,
        descriptor: DataKey,
    ) -> Result<(), LowerError> {
        let header = self
            .plan
            .semantic()
            .shared_header
            .ok_or(PlanError::InvalidLayout)?;
        let metadata = self.builder.append(
            block,
            Operation::SymbolAddress {
                symbol: ArtifactId::Data(descriptor),
                ty: ScalarType::DataAddress,
            },
        )?[0];
        let metadata_address = self.offset(block, handle, header.dynamic_metadata_offset)?;
        self.store_pointer(block, metadata_address, metadata)?;
        self.store_u64_at(block, handle, header.owner_count_offset, 1)
    }

    fn class_base_offset(&self, class: ClassId, base: ClassId) -> Result<usize, LowerError> {
        let fact = self
            .plan
            .semantic()
            .class(class)
            .ok_or(PlanError::UnknownDeclaration)?;
        fact.base
            .filter(|fact| fact.class == base)
            .map(|fact| fact.offset)
            .ok_or(PlanError::InvalidDomain.into())
    }

    fn copy_class_field<I: Copy>(
        &mut self,
        block: crate::backend::lir::BlockHandle<'plan>,
        destination: ValueHandle<'plan>,
        source: ValueHandle<'plan>,
        field: MirSynthesizedFieldCopy<I>,
    ) -> Result<crate::backend::lir::BlockHandle<'plan>, LowerError> {
        let field_fact = self
            .plan
            .semantic()
            .field(field.field())
            .ok_or(PlanError::UnknownDeclaration)?;
        let destination = self.offset(block, destination, field_fact.offset)?;
        let source = self.offset(block, source, field_fact.offset)?;
        match field {
            MirSynthesizedFieldCopy::Scalar { .. } => {
                self.copy_scalar(block, destination, source, field_fact.ty, field_fact.layout)?;
                Ok(block)
            }
            MirSynthesizedFieldCopy::OptionalPrimitive { .. }
            | MirSynthesizedFieldCopy::OptionalClass { .. }
            | MirSynthesizedFieldCopy::Optional { .. } => {
                let SemanticType::Optional(optional) = field_fact.ty else {
                    return Err(PlanError::InvalidDomain.into());
                };
                self.copy_optional(block, destination, source, optional)
            }
            MirSynthesizedFieldCopy::Class { .. } => {
                let SemanticType::Class(class) = field_fact.ty else {
                    return Err(PlanError::InvalidDomain.into());
                };
                self.call_class_copy(block, class, destination, source)?;
                Ok(block)
            }
            MirSynthesizedFieldCopy::Array { array, .. } => {
                self.copy_array(block, destination, source, array)?;
                Ok(block)
            }
            MirSynthesizedFieldCopy::Shared { .. } => {
                self.copy_shared(block, destination, source)?;
                Ok(block)
            }
            MirSynthesizedFieldCopy::OptionalShared { .. } => {
                self.copy_shared_nullable(block, destination, source)
            }
        }
    }
}

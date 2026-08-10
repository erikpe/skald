//! Ordered ordinary-instruction verification.

use std::collections::HashSet;

use super::{
    super::model::{
        MirAliasAccess, MirAssignment, MirBasicBlock, MirComparisonOperand, MirCopyAssignment,
        MirCopyConstruction, MirDefinitionRef, MirEndFullExpression, MirInstruction, MirPlace,
        MirPlaceBase, MirRvalue, MirRvalueKind, MirStorageKind, MirStore, MirType, ValueId,
    },
    context::Verifier,
    place::places_overlap,
};

#[derive(Clone, Copy)]
enum CopyOperationKind {
    Construction,
    Assignment,
}

impl Verifier<'_> {
    pub(super) fn verify_block(
        &mut self,
        return_type: MirType,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        defined_values: &mut HashSet<ValueId>,
    ) {
        // MIR transient values are deliberately block-local before SSA. A
        // separate set per block prevents vector order from accidentally
        // permitting values to cross control-flow edges.
        let mut defined_in_block = HashSet::new();
        for instruction in &block.instructions {
            self.verify_instruction(
                function,
                block,
                instruction,
                defined_values,
                &mut defined_in_block,
            );
        }
        self.verify_terminator(return_type, function, block, &defined_in_block);
    }

    fn verify_instruction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        instruction: &MirInstruction,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        match instruction {
            MirInstruction::StorageLive(_) | MirInstruction::StorageDead(_) => {}
            MirInstruction::Assign(assignment) => self.verify_assignment(
                function,
                block,
                assignment,
                defined_values,
                defined_in_block,
            ),
            MirInstruction::Call(call) => {
                self.verify_call(function, block, call, defined_values, defined_in_block)
            }
            MirInstruction::Cleanup(cleanup) => {
                self.verify_cleanup_instruction(function, block, cleanup)
            }
            MirInstruction::Initialize(initialize) => {
                self.verify_initialize(function, block, initialize, defined_in_block)
            }
            MirInstruction::CopyConstruct(copy) => {
                self.verify_copy_construction(function, block, copy)
            }
            MirInstruction::CopyAssign(copy) => self.verify_copy_assignment(function, block, copy),
            MirInstruction::EndFullExpression(end) => {
                self.verify_full_expression_end(function, block, end)
            }
            MirInstruction::Store(store) => {
                self.verify_store(function, block, store, defined_in_block)
            }
            MirInstruction::BindCheckedView(binding) => {
                self.verify_checked_view_binding(function, block, binding, false)
            }
            MirInstruction::EndCheckedView(end) => {
                self.verify_checked_view_end(function, block, end)
            }
            MirInstruction::SharedAllocate(allocation) => {
                self.verify_shared_allocate(function, block, allocation)
            }
            MirInstruction::SharedInitialize(initialize) => {
                self.verify_shared_initialize(function, block, initialize, defined_in_block)
            }
            MirInstruction::SharedPublish(publish) => {
                self.verify_shared_publish(function, block, publish)
            }
            MirInstruction::SharedStatic(static_owner) => {
                self.verify_shared_static(function, block, static_owner)
            }
            MirInstruction::SharedAdopt(adopt) => self.verify_shared_adopt(function, block, adopt),
            MirInstruction::SharedCopy(copy) => self.verify_shared_copy(function, block, copy),
            MirInstruction::SharedFieldCopy(copy) => {
                self.verify_shared_field_copy(function, block, copy)
            }
            MirInstruction::SharedCast(cast) => {
                self.verify_shared_cast(function, block, cast, false)
            }
            MirInstruction::SharedMove(transfer) => {
                self.verify_shared_move(function, block, transfer)
            }
            MirInstruction::SharedRelease(release) => {
                self.verify_shared_release(function, block, release)
            }
            MirInstruction::SharedFieldInitialize(initialize) => {
                self.verify_shared_field_initialize(function, block, initialize)
            }
            MirInstruction::SharedFieldReplace(replace) => {
                self.verify_shared_field_replace(function, block, replace)
            }
            MirInstruction::StringInitialize(initialize) => {
                self.verify_string_initialize(function, block, initialize)
            }
            MirInstruction::OptionalInitialize(initialize) => self.verify_optional_initialize(
                function,
                block,
                &initialize.destination,
                &initialize.source,
                defined_in_block,
            ),
            MirInstruction::OptionalAssign(assignment) => self.verify_optional_assign(
                function,
                block,
                &assignment.destination,
                &assignment.source,
                defined_in_block,
            ),
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                self.verify_aggregate_optional_operation(
                    function,
                    block,
                    initialize.optional,
                    &initialize.destination,
                    Some(&initialize.source),
                    false,
                );
            }
            MirInstruction::AggregateOptionalAssign(assignment) => {
                if matches!(
                    assignment.source,
                    crate::mir::MirAggregateOptionalSource::Unpublished
                ) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "nested optional assignment cannot use an unpublished source",
                    );
                }
                self.verify_aggregate_optional_operation(
                    function,
                    block,
                    assignment.optional,
                    &assignment.destination,
                    Some(&assignment.source),
                    true,
                );
            }
            MirInstruction::AggregateOptionalPublish(publish) => {
                self.verify_aggregate_optional_operation(
                    function,
                    block,
                    publish.optional,
                    &publish.destination,
                    None,
                    false,
                );
            }
            MirInstruction::AggregateOptionalCleanup(cleanup) => {
                self.verify_aggregate_optional_operation(
                    function,
                    block,
                    cleanup.optional,
                    &cleanup.destination,
                    None,
                    true,
                );
            }
            MirInstruction::OptionalSharedInitialize(initialize) => self
                .verify_optional_shared_operation(
                    function,
                    block,
                    &initialize.destination,
                    &initialize.source,
                    initialize.optional,
                    initialize.target,
                ),
            MirInstruction::OptionalSharedAssign(assignment) => self
                .verify_optional_shared_operation(
                    function,
                    block,
                    &assignment.destination,
                    &assignment.source,
                    assignment.optional,
                    assignment.target,
                ),
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                self.verify_optional_shared_cleanup(function, block, cleanup)
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                self.verify_class_optional_places(
                    function,
                    block,
                    &initialize.destination,
                    &initialize.source,
                    initialize.optional,
                    initialize.class,
                );
                let expected = self
                    .program
                    .class(initialize.class)
                    .and_then(|class| class.copy_constructor.selected());
                let valid = match initialize.source {
                    crate::mir::MirClassOptionalSource::Absent => {
                        initialize.copy_constructor.is_none()
                    }
                    _ => initialize.copy_constructor == expected && expected.is_some(),
                };
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class optional initialization has an invalid copy operation",
                    );
                }
            }
            MirInstruction::ClassOptionalAssign(assignment) => {
                self.verify_class_optional_places(
                    function,
                    block,
                    &assignment.destination,
                    &assignment.source,
                    assignment.optional,
                    assignment.class,
                );
                if self
                    .verify_place(function, block, &assignment.destination)
                    .is_some_and(|place| place.access != crate::mir::MirAliasAccess::Mutable)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class optional assignment destination requires mutable access",
                    );
                }
                let declaration = self.program.class(assignment.class);
                let expected_constructor =
                    declaration.and_then(|class| class.copy_constructor.selected());
                let expected_assignment =
                    declaration.and_then(|class| class.copy_assignment.selected());
                let valid = match assignment.source {
                    crate::mir::MirClassOptionalSource::Absent => {
                        assignment.copy_constructor.is_none()
                            && assignment.copy_assignment.is_none()
                    }
                    _ => {
                        assignment.copy_constructor == expected_constructor
                            && assignment.copy_assignment == expected_assignment
                            && expected_constructor.is_some()
                            && expected_assignment.is_some()
                    }
                };
                if !valid {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class optional assignment has invalid payload copy operations",
                    );
                }
            }
            MirInstruction::ClassOptionalPublish(publish) => {
                if self
                    .verify_place(function, block, &publish.destination)
                    .map(|p| p.ty)
                    != Some(MirType::Optional(publish.optional))
                    || self.optional_class(MirType::Optional(publish.optional))
                        != Some(publish.class)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class optional publish destination has the wrong type",
                    );
                }
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                if self
                    .verify_place(function, block, &cleanup.destination)
                    .map(|p| p.ty)
                    != Some(MirType::Optional(cleanup.optional))
                    || self.optional_class(MirType::Optional(cleanup.optional))
                        != Some(cleanup.class)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "class optional cleanup destination has the wrong type",
                    );
                }
            }
            MirInstruction::EndOptionalView(end) => {
                self.verify_optional_view_end(function, block, end)
            }
            MirInstruction::Array(instruction) => {
                self.verify_array_instruction(function, block, instruction, defined_in_block)
            }
            MirInstruction::Io(instruction) => self.verify_io_instruction(
                function,
                block,
                instruction,
                defined_values,
                defined_in_block,
            ),
        }
    }

    fn verify_class_optional_places(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination: &MirPlace,
        source: &crate::mir::MirClassOptionalSource,
        optional: crate::identity::OptionalTypeId,
        class: crate::identity::ClassId,
    ) {
        if self
            .verify_place(function, block, destination)
            .map(|p| p.ty)
            != Some(MirType::Optional(optional))
            || self.optional_class(MirType::Optional(optional)) != Some(class)
        {
            self.block_error(
                function.callable(),
                block.id,
                "class optional destination has the wrong type",
            );
        }
        let (source, expected) = match source {
            crate::mir::MirClassOptionalSource::Absent => return,
            crate::mir::MirClassOptionalSource::Present(place) => (place, MirType::Class(class)),
            crate::mir::MirClassOptionalSource::Copy(place) => (place, MirType::Optional(optional)),
        };
        if self.verify_place(function, block, source).map(|p| p.ty) != Some(expected) {
            self.block_error(
                function.callable(),
                block.id,
                "class optional source has the wrong type",
            );
        }
    }

    fn verify_assignment(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        assignment: &MirAssignment,
        defined_values: &mut HashSet<ValueId>,
        defined_in_block: &mut HashSet<ValueId>,
    ) {
        let Some(result) = function.value(assignment.result) else {
            self.block_error(
                function.callable(),
                block.id,
                format!("assignment result {} is not declared", assignment.result),
            );
            return;
        };
        if defined_values.contains(&assignment.result) {
            self.block_error(
                function.callable(),
                block.id,
                format!("value {} is defined more than once", assignment.result),
            );
        }
        if result.ty != assignment.rvalue.ty {
            self.block_error(
                function.callable(),
                block.id,
                format!("assignment type does not match value {}", assignment.result),
            );
        }
        self.verify_rvalue(function, block, &assignment.rvalue, defined_in_block);
        defined_values.insert(assignment.result);
        defined_in_block.insert(assignment.result);
    }

    fn verify_copy_construction(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirCopyConstruction,
    ) {
        self.verify_copy_places(
            function,
            block,
            &copy.destination,
            &copy.source,
            copy.class,
            CopyOperationKind::Construction,
        );
        let selected = self
            .program
            .class(copy.class)
            .and_then(|class| class.copy_constructor.selected());
        if selected != Some(copy.operation) {
            self.block_error(
                function.callable(),
                block.id,
                "copy-construction operation does not match the class capability",
            );
        }
    }

    fn verify_copy_assignment(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        copy: &MirCopyAssignment,
    ) {
        self.verify_copy_places(
            function,
            block,
            &copy.destination,
            &copy.source,
            copy.class,
            CopyOperationKind::Assignment,
        );
        let selected = self
            .program
            .class(copy.class)
            .and_then(|class| class.copy_assignment.selected());
        if selected != Some(copy.operation) {
            self.block_error(
                function.callable(),
                block.id,
                "copy-assignment operation does not match the class capability",
            );
        }
    }

    fn verify_full_expression_end(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        end: &MirEndFullExpression,
    ) {
        for cleanup in &end.temporaries {
            let destination = self.verify_place(function, block, &cleanup.destination);
            let is_temporary = function
                .storage(cleanup.destination.base.expect_local_storage())
                .is_some_and(|storage| storage.kind == MirStorageKind::Temporary);
            if !is_temporary
                || !cleanup.destination.projections.is_empty()
                || !matches!(cleanup.destination.base, MirPlaceBase::Storage(_))
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "full-expression cleanup must name complete temporary storage",
                );
            }
            if destination.map(|place| place.ty) != Some(MirType::Class(cleanup.target)) {
                self.block_error(
                    function.callable(),
                    block.id,
                    "full-expression cleanup has the wrong class type",
                );
            }
        }
    }

    fn verify_store(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        store: &MirStore,
        defined_in_block: &HashSet<ValueId>,
    ) {
        let destination = self.verify_place(function, block, &store.destination);
        let storage_ty = destination.map(|place| place.ty);
        let value_ty = self.verify_value_use(function, block, store.value, defined_in_block);
        if storage_ty.is_some_and(|ty| !ty.is_scalar_value()) {
            self.block_error(
                function.callable(),
                block.id,
                "store destination must have scalar value type",
            );
        }
        if storage_ty.is_some() && value_ty.is_some() && storage_ty != value_ty {
            self.block_error(function.callable(), block.id, "store operand type mismatch");
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "store destination requires mutable access",
            );
        }
    }

    fn verify_copy_places(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        destination_place: &MirPlace,
        source_place: &MirPlace,
        class: crate::identity::ClassId,
        operation: CopyOperationKind,
    ) {
        let construction = matches!(operation, CopyOperationKind::Construction);
        let allocation_payload = matches!(
            destination_place.base,
            MirPlaceBase::SharedAllocationPayload(_)
        );
        let destination = if construction && allocation_payload {
            self.verify_copy_allocation_destination(function, block, destination_place)
        } else {
            self.verify_place(function, block, destination_place)
        };
        let source = self.verify_place(function, block, source_place);
        if self.program.class(class).is_none() {
            self.block_error(
                function.callable(),
                block.id,
                format!("copy operation class {class} is not declared"),
            );
        }
        if destination.map(|place| place.ty) != Some(MirType::Class(class))
            || source.map(|place| place.ty) != Some(MirType::Class(class))
        {
            self.block_error(
                function.callable(),
                block.id,
                "copy source and destination must have the exact operation class",
            );
        }
        if destination.is_some_and(|place| place.access != MirAliasAccess::Mutable) {
            self.block_error(
                function.callable(),
                block.id,
                "copy destination requires mutable access",
            );
        }
        if allocation_payload && (!construction || !destination_place.projections.is_empty()) {
            self.block_error(
                function.callable(),
                block.id,
                "copy-allocation destination must name one complete unpublished payload",
            );
        }
        let destination_storage = destination_place
            .base
            .local_storage()
            .and_then(|storage| function.storage(storage));
        if matches!(destination_place.base, MirPlaceBase::AliasParameter(_))
            || destination_storage
                .is_some_and(|storage| matches!(storage.kind, MirStorageKind::AliasParameter(_)))
        {
            self.block_error(
                function.callable(),
                block.id,
                if construction {
                    "copy-construction destination must be owning storage"
                } else {
                    "copy-assignment destination must be owning storage"
                },
            );
        }
        if !construction
            && destination_place.projections.is_empty()
            && destination_place
                .base
                .local_storage()
                .is_some_and(|storage| function.receiver() == Some(storage))
        {
            self.block_error(
                function.callable(),
                block.id,
                "copy assignment cannot replace the complete receiver",
            );
        }
        if construction && places_overlap(destination_place, source_place) {
            self.block_error(
                function.callable(),
                block.id,
                "copy-construction source and destination must not overlap",
            );
        }
    }

    fn verify_rvalue(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        rvalue: &MirRvalue,
        defined: &HashSet<ValueId>,
    ) {
        match &rvalue.kind {
            MirRvalueKind::ConstantI64(_) => {
                if rvalue.ty != MirType::I64 {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer constant is not `i64`",
                    );
                }
            }
            MirRvalueKind::ConstantU64(_) => {
                if rvalue.ty != MirType::U64 {
                    self.block_error(function.callable(), block.id, "u64 constant is not `u64`");
                }
            }
            MirRvalueKind::ConstantU8(_) => {
                if rvalue.ty != MirType::U8 {
                    self.block_error(function.callable(), block.id, "u8 constant is not `u8`");
                }
            }
            MirRvalueKind::ConstantF64Bits(_) => {
                if rvalue.ty != MirType::F64 {
                    self.block_error(function.callable(), block.id, "f64 constant is not `f64`");
                }
            }
            MirRvalueKind::ConstantBool(_) => {
                if rvalue.ty != MirType::Bool {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "boolean constant is not `bool`",
                    );
                }
            }
            MirRvalueKind::PathCondition(condition) => {
                if rvalue.ty != MirType::Bool {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "path-condition value is not `bool`",
                    );
                }
                match function.path_condition(condition.condition) {
                    Some(declaration) if declaration.activation == condition.activation => {}
                    Some(_) => self.block_error(
                        function.callable(),
                        block.id,
                        "path-condition value uses the wrong activation storage",
                    ),
                    None => self.block_error(
                        function.callable(),
                        block.id,
                        format!(
                            "path-condition value references undeclared condition {}",
                            condition.condition
                        ),
                    ),
                }
            }
            MirRvalueKind::Load(place) => {
                let place_ty = self
                    .verify_place(function, block, place)
                    .map(|place| place.ty);
                if place
                    .base
                    .local_storage()
                    .and_then(|storage| function.storage(storage))
                    .is_some_and(|storage| storage.kind == MirStorageKind::PathCondition)
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "path-condition storage must be read through a path-condition value",
                    );
                }
                if place_ty.is_some_and(|ty| !ty.is_scalar_value()) {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "load source must have scalar value type",
                    );
                }
                if place_ty.is_some() && place_ty != Some(rvalue.ty) {
                    self.block_error(function.callable(), block.id, "load result type mismatch");
                }
            }
            MirRvalueKind::Unary { operation, operand } => {
                if rvalue.ty != operation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "unary operation result type mismatch",
                    );
                }
                self.verify_unary_operand(
                    function,
                    block,
                    *operand,
                    operation.operand_type(),
                    defined,
                );
            }
            MirRvalueKind::Binary {
                operation,
                left,
                right,
            } => {
                let expected = operation.operand_type();
                if rvalue.ty != operation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "binary operation result type mismatch",
                    );
                }
                self.verify_binary_operand(function, block, *left, expected, defined);
                self.verify_binary_operand(function, block, *right, expected, defined);
            }
            MirRvalueKind::IntegerDivision {
                operation,
                dividend,
                divisor,
            } => {
                let expected = operation.operand_type();
                if rvalue.ty != operation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "integer division or remainder result type mismatch",
                    );
                }
                self.verify_binary_operand(function, block, *dividend, expected, defined);
                self.verify_binary_operand(function, block, *divisor, expected, defined);
            }
            MirRvalueKind::Shift {
                operation,
                left,
                count,
            } => {
                if rvalue.ty != operation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "shift operation result type mismatch",
                    );
                }
                self.verify_shift_operand(
                    function,
                    block,
                    *left,
                    operation.left_type(),
                    "left",
                    defined,
                );
                self.verify_shift_operand(
                    function,
                    block,
                    *count,
                    operation.count_type(),
                    "count",
                    defined,
                );
            }
            MirRvalueKind::PrimitiveComparison {
                operation,
                left,
                right,
            } => {
                if rvalue.ty != operation.result_type() {
                    let message = match operation.operand {
                        MirComparisonOperand::Integer(_) => {
                            "integer comparison result must be `bool`"
                        }
                        MirComparisonOperand::F64 => "floating comparison result must be `bool`",
                        MirComparisonOperand::Bool => "boolean comparison result must be `bool`",
                    };
                    self.block_error(function.callable(), block.id, message);
                }
                if !operation.is_valid() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!(
                            "comparison predicate `{}` is not valid for `{}`",
                            operation.predicate.mnemonic(),
                            operation.operand.name()
                        ),
                    );
                }
                let expected = operation.operand_type();
                self.verify_comparison_operand(function, block, *left, expected, defined);
                self.verify_comparison_operand(function, block, *right, expected, defined);
            }
            MirRvalueKind::PrimitiveCast { operation, operand } => {
                if rvalue.ty != operation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "primitive cast result type mismatch",
                    );
                }
                if !operation.is_semantically_consistent() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        format!(
                            "primitive cast semantic class `{}` does not match `{} -> {}`",
                            operation.kind().mnemonic(),
                            operation.source,
                            operation.target
                        ),
                    );
                }
                if operation.may_terminate() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "checked primitive cast requires explicit control flow",
                    );
                }
                if let Some(ty) = self.verify_value_use(function, block, *operand, defined) {
                    if ty != operation.source_type() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            format!("primitive cast source is not `{}`", operation.source_type()),
                        );
                    }
                }
            }
            MirRvalueKind::CheckedF64ToInteger { relation, operand } => {
                if rvalue.ty != relation.result_type() {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "checked primitive cast result type mismatch",
                    );
                }
                if let Some(ty) = self.verify_value_use(function, block, *operand, defined) {
                    if ty != relation.source_type() {
                        self.block_error(
                            function.callable(),
                            block.id,
                            "checked primitive cast source is not `f64`",
                        );
                    }
                }
            }
            MirRvalueKind::TypeTest { source, target } => {
                self.verify_type_test(function, block, rvalue, source, *target)
            }
            MirRvalueKind::OptionalPresence { source, .. } => {
                self.verify_optional_presence(function, block, source, rvalue.ty)
            }
            MirRvalueKind::ArrayLength { source, array } => {
                if rvalue.ty != MirType::U64 {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array length result must be `u64`",
                    );
                }
                if self
                    .verify_place(function, block, source)
                    .map(|place| place.ty)
                    != Some(MirType::Array(*array))
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        "array length source has the wrong exact array type",
                    );
                }
            }
        }
    }

    fn verify_binary_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("binary operand is not `{expected}`"),
                );
            }
        }
    }

    fn verify_shift_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        name: &str,
        defined: &HashSet<ValueId>,
    ) {
        if self.verify_value_use(function, block, value, defined) != Some(expected) {
            self.block_error(
                function.callable(),
                block.id,
                format!("shift {name} operand is not `{expected}`"),
            );
        }
    }

    fn verify_unary_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("unary operand is not `{expected}`"),
                );
            }
        }
    }

    fn verify_aggregate_optional_operation(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        optional: crate::identity::OptionalTypeId,
        destination: &MirPlace,
        source: Option<&crate::mir::MirAggregateOptionalSource>,
        mutable: bool,
    ) {
        let valid_metadata = self
            .program
            .optional_type(optional)
            .is_some_and(|metadata| {
                matches!(
                    metadata.storage,
                    crate::mir::MirOptionalStorage::Nested(_)
                        | crate::mir::MirOptionalStorage::InlineArray(_)
                )
            });
        let destination = self.verify_place(function, block, destination);
        if !valid_metadata
            || destination.as_ref().map(|place| place.ty) != Some(MirType::Optional(optional))
        {
            self.block_error(
                function.callable(),
                block.id,
                "nested optional operation has incompatible destination metadata",
            );
        }
        if mutable
            && destination
                .as_ref()
                .is_some_and(|place| place.access != MirAliasAccess::Mutable)
        {
            self.block_error(
                function.callable(),
                block.id,
                "nested optional mutation requires mutable access",
            );
        }
        if let Some(crate::mir::MirAggregateOptionalSource::Copy(source)) = source {
            if self
                .verify_place(function, block, source)
                .map(|place| place.ty)
                != Some(MirType::Optional(optional))
            {
                self.block_error(
                    function.callable(),
                    block.id,
                    "nested optional copy source has the wrong type",
                );
            }
        }
    }

    fn verify_comparison_operand(
        &mut self,
        function: MirDefinitionRef<'_>,
        block: &MirBasicBlock,
        value: ValueId,
        expected: MirType,
        defined: &HashSet<ValueId>,
    ) {
        if let Some(ty) = self.verify_value_use(function, block, value, defined) {
            if ty != expected {
                self.block_error(
                    function.callable(),
                    block.id,
                    format!("comparison operand is not `{expected}`"),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests;

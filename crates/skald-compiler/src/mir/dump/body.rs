//! Rendering for executable MIR bodies, blocks, instructions, and terminators.

use std::fmt::Write;

use crate::dump_format::{write_quoted, write_span};

use super::super::model::*;
use super::value::{
    dump_aggregate_optional_source, dump_argument, dump_array_instruction,
    dump_cell_write_authorization, dump_copy_operation, dump_final_write_authorization,
    dump_io_instruction, dump_object_origin, dump_object_view, dump_optional_shared_source,
    dump_optional_source, dump_place, dump_rvalue, dump_shared_cast,
};

pub(super) fn dump_executable_body(output: &mut String, function: MirDefinitionRef<'_>) {
    write_span(output, function.span());
    output.push('\n');
    if let Some(receiver) = function.receiver() {
        let _ = writeln!(output, "      Receiver {receiver}");
    }
    if let Some(storage) = function.return_storage() {
        let _ = writeln!(output, "      ReturnStorage {storage}");
    }
    output.push_str("      Parameters");
    for parameter in function.parameters() {
        let _ = write!(output, " {parameter}");
    }
    output.push('\n');
    output.push_str("      Storage\n");
    for storage in function.storage_entries() {
        let kind = match storage.kind {
            MirStorageKind::Return => "return",
            MirStorageKind::Receiver => "receiver",
            MirStorageKind::Parameter => "parameter",
            MirStorageKind::AliasParameter(MirAliasAccess::ReadOnly) => "ref-parameter",
            MirStorageKind::AliasParameter(MirAliasAccess::Mutable) => "mut-ref-parameter",
            MirStorageKind::CheckedView(MirAliasAccess::ReadOnly) => "checked-view",
            MirStorageKind::CheckedView(MirAliasAccess::Mutable) => "checked-mut-view",
            MirStorageKind::Local => "local",
            MirStorageKind::Argument => "argument",
            MirStorageKind::Temporary => "temporary",
            MirStorageKind::SharedAnchor => "shared-anchor",
            MirStorageKind::ScalarSpill => "scalar-spill",
            MirStorageKind::PrimitiveAlias => "primitive-alias",
            MirStorageKind::PathCondition => "path-condition",
            MirStorageKind::NormalizedPathActivation => "normalized-path-activation",
            MirStorageKind::OptionalUnwrap => "optional-unwrap",
            MirStorageKind::SharedAllocation => "shared-allocation",
            MirStorageKind::ArrayBacking => "array-backing",
            MirStorageKind::ArrayProduced => "array-produced",
            MirStorageKind::ArraySlice => "array-slice",
            MirStorageKind::ArrayPosition => "array-position",
            MirStorageKind::ArrayAnchor(_) => "array-anchor",
            MirStorageKind::ArrayAlias(MirAliasAccess::ReadOnly) => "array-alias",
            MirStorageKind::ArrayAlias(MirAliasAccess::Mutable) => "array-mut-alias",
        };
        let _ = write!(output, "        {} {kind} ", storage.id);
        match storage.source {
            Some(source) => {
                let _ = write!(output, "{source} ");
            }
            None => match storage.kind {
                MirStorageKind::Return => output.push_str("<return> "),
                MirStorageKind::Local => output.push_str("<compiler-local> "),
                MirStorageKind::Argument => output.push_str("<argument> "),
                MirStorageKind::Temporary => output.push_str("<temporary> "),
                MirStorageKind::SharedAnchor => output.push_str("<shared-anchor> "),
                MirStorageKind::CheckedView(_) => output.push_str("<checked-view> "),
                MirStorageKind::ScalarSpill => output.push_str("<scalar-spill> "),
                MirStorageKind::PrimitiveAlias => output.push_str("<primitive-alias> "),
                MirStorageKind::PathCondition => output.push_str("<path-condition> "),
                MirStorageKind::NormalizedPathActivation => {
                    output.push_str("<normalized-path-activation> ")
                }
                MirStorageKind::OptionalUnwrap => output.push_str("<optional-unwrap> "),
                MirStorageKind::SharedAllocation => output.push_str("<shared-allocation> "),
                MirStorageKind::ArrayBacking => output.push_str("<array-backing> "),
                MirStorageKind::ArrayProduced => output.push_str("<array-produced> "),
                MirStorageKind::ArraySlice => output.push_str("<array-slice> "),
                MirStorageKind::ArrayPosition => output.push_str("<array-position> "),
                MirStorageKind::ArrayAnchor(_) => output.push_str("<array-anchor> "),
                MirStorageKind::ArrayAlias(_) => output.push_str("<array-alias> "),
                _ => unreachable!("verified language storage has a source binding"),
            },
        }
        write_quoted(output, &storage.name);
        let _ = write!(output, " : {}", storage.ty);
        write_span(output, storage.span);
        output.push('\n');
    }
    output.push_str("      Values\n");
    for value in function.values() {
        let _ = write!(output, "        {} : {}", value.id, value.ty);
        write_span(output, value.span);
        output.push('\n');
    }
    if !function.path_conditions().is_empty() {
        output.push_str("      PathConditions\n");
        for condition in function.path_conditions() {
            let _ = write!(output, "        {} parent ", condition.id,);
            match condition.parent {
                Some(parent) => {
                    let _ = write!(output, "{parent}");
                }
                None => output.push_str("<root>"),
            }
            let _ = write!(
                output,
                " activation {} active {} inactive {} merge {}",
                condition.activation,
                condition.active_predecessor,
                condition.inactive_predecessor,
                condition.merge,
            );
            write_span(output, condition.span);
            output.push('\n');
        }
    }
    if !function.logical_expressions().is_empty() {
        output.push_str("      LogicalExpressions\n");
        for logical in function.logical_expressions() {
            let operation = match logical.operation {
                MirLogicalOperation::And => "and",
                MirLogicalOperation::Or => "or",
            };
            let _ = write!(
                output,
                "        {operation} condition {} result {} left {} split {} selection {} right {}..{} value {} short {} join {} selected {}",
                logical.condition,
                logical.result,
                logical.left_result,
                logical.split,
                logical.selection,
                logical.right_entry,
                logical.right_exit,
                logical.right_result,
                logical.short,
                logical.join,
                logical.selected_result,
            );
            write_span(output, logical.span);
            output.push('\n');
        }
    }
    let _ = writeln!(output, "      EntryBlock {}", function.body().entry);
    output.push_str("      Blocks\n");
    for block in &function.body().blocks {
        dump_block(output, block);
    }
}

fn dump_block(output: &mut String, block: &MirBasicBlock) {
    let _ = write!(output, "        {}", block.id);
    write_span(output, block.span);
    output.push('\n');
    for instruction in &block.instructions {
        output.push_str("          ");
        match instruction {
            MirInstruction::StorageLive(lifetime) => {
                let _ = write!(output, "storage-live {}", lifetime.storage);
                write_span(output, lifetime.span);
            }
            MirInstruction::StorageDead(lifetime) => {
                let _ = write!(output, "storage-dead {}", lifetime.storage);
                write_span(output, lifetime.span);
            }
            MirInstruction::Assign(assignment) => {
                let _ = write!(output, "{} = ", assignment.result);
                dump_rvalue(output, &assignment.rvalue);
                write_span(output, assignment.span);
            }
            MirInstruction::Call(call) => {
                if let Some(destination) = &call.destination {
                    dump_place(output, destination);
                    output.push_str(" <- ");
                } else if let Some(result) = call.shared_result {
                    let _ = write!(output, "{result} = shared-result ");
                } else if let Some(result) = call.result {
                    let _ = write!(output, "{result} = ");
                }
                match call.target {
                    MirCallTarget::Direct(target) => {
                        let _ = write!(output, "call {target}");
                    }
                    MirCallTarget::Static(target) => {
                        let _ = write!(output, "call static {target}");
                    }
                    MirCallTarget::Indirect(target) => {
                        let _ = write!(
                            output,
                            "call indirect {} : {}",
                            target.callee, target.function_type
                        );
                    }
                    MirCallTarget::Method(MirMethodCallTarget::Direct(target)) => {
                        let _ = write!(output, "call direct {target}");
                    }
                    MirCallTarget::Method(MirMethodCallTarget::Virtual {
                        family,
                        slot,
                        selected,
                    }) => {
                        let _ = write!(
                            output,
                            "call virtual {family} slot {slot} selected {selected}"
                        );
                    }
                    MirCallTarget::Interface(target) => {
                        let _ = write!(
                            output,
                            "call interface {} {}",
                            target.interface, target.requirement
                        );
                    }
                }
                if let Some(receiver) = &call.receiver {
                    output.push_str(" on ");
                    match receiver {
                        MirCallReceiver::Method(receiver) => {
                            dump_place(output, &receiver.place);
                            let _ = write!(output, " {}", receiver.access);
                            if receiver.provenance == MirViewProvenance::Produced {
                                output.push_str(" produced");
                            }
                            output.push_str(" origin ");
                            dump_object_origin(output, &receiver.origin);
                        }
                        MirCallReceiver::Interface(receiver) => {
                            dump_object_view(output, receiver);
                        }
                    }
                }
                output.push('(');
                for (index, argument) in call.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, call.span);
            }
            MirInstruction::Cleanup(cleanup) => {
                output.push_str("cleanup ");
                dump_place(output, &cleanup.destination);
                let _ = write!(output, " as {}", cleanup.target);
                write_span(output, cleanup.span);
            }
            MirInstruction::Initialize(initialize) => {
                output.push_str("initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(output, " with {}(", initialize.target);
                for (index, argument) in initialize.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, initialize.span);
            }
            MirInstruction::Store(store) => {
                output.push_str("store ");
                dump_place(output, &store.destination);
                let _ = write!(output, ", {}", store.value);
                dump_cell_write_authorization(output, store.authorization);
                dump_final_write_authorization(output, store.final_authorization);
                write_span(output, store.span);
            }
            MirInstruction::CopyConstruct(copy) => {
                output.push_str("copy-construct ");
                dump_place(output, &copy.destination);
                output.push_str(" from ");
                dump_place(output, &copy.source);
                let _ = write!(output, " as {} via ", copy.class);
                dump_copy_operation(output, copy.operation);
                write_span(output, copy.span);
            }
            MirInstruction::CopyAssign(copy) => {
                output.push_str("copy-assign ");
                dump_place(output, &copy.destination);
                output.push_str(" from ");
                dump_place(output, &copy.source);
                let _ = write!(output, " as {} via ", copy.class);
                dump_copy_operation(output, copy.operation);
                dump_cell_write_authorization(output, copy.authorization);
                dump_final_write_authorization(output, copy.final_authorization);
                write_span(output, copy.span);
            }
            MirInstruction::EndFullExpression(end) => {
                output.push_str("end-full-expression");
                for cleanup in &end.temporaries {
                    output.push_str(" cleanup ");
                    dump_place(output, &cleanup.destination);
                    let _ = write!(output, " as {}", cleanup.target);
                }
                write_span(output, end.span);
            }
            MirInstruction::BindCheckedView(binding) => {
                let _ = write!(output, "bind-checked-view {} = ", binding.destination);
                dump_object_view(output, &binding.view);
                write_span(output, binding.span);
            }
            MirInstruction::EndCheckedView(end) => {
                let _ = write!(output, "end-checked-view {}", end.carrier);
                write_span(output, end.span);
            }
            MirInstruction::SharedAllocate(allocation) => {
                let origin = match allocation.origin {
                    MirSharedAllocationOrigin::New => "new",
                    MirSharedAllocationOrigin::OptionalBox => "optional-box",
                    MirSharedAllocationOrigin::Unspecified => "unspecified",
                };
                let _ = write!(
                    output,
                    "shared-allocate {} exact {} from {origin}",
                    allocation.allocation, allocation.target,
                );
                match &allocation.mode {
                    MirSharedAllocationMode::Initialize => output.push_str(" initialize"),
                    MirSharedAllocationMode::Copy { source } => {
                        output.push_str(" copy ");
                        dump_place(output, source);
                    }
                    MirSharedAllocationMode::OptionalBox { completion } => {
                        let _ = write!(output, " complete-with {completion:?}");
                    }
                }
                write_span(output, allocation.span);
            }
            MirInstruction::SharedInitialize(initialize) => {
                let _ = write!(
                    output,
                    "shared-initialize {} with {}(",
                    initialize.allocation, initialize.target
                );
                for (index, argument) in initialize.arguments.iter().enumerate() {
                    if index != 0 {
                        output.push_str(", ");
                    }
                    dump_argument(output, argument);
                }
                output.push(')');
                write_span(output, initialize.span);
            }
            MirInstruction::SharedPublish(publish) => {
                let _ = write!(output, "shared-publish {}", publish.allocation);
                write_span(output, publish.span);
            }
            MirInstruction::SharedStatic(static_owner) => {
                let _ = write!(
                    output,
                    "shared-static {} from {} : {} {:?}",
                    static_owner.destination,
                    static_owner.data,
                    static_owner.target,
                    static_owner.origin
                );
                write_span(output, static_owner.span);
            }
            MirInstruction::SharedAdopt(adopt) => {
                let _ = write!(
                    output,
                    "shared-adopt {} from {}",
                    adopt.destination, adopt.allocation
                );
                write_span(output, adopt.span);
            }
            MirInstruction::SharedCopy(copy) => {
                let _ = write!(
                    output,
                    "shared-copy {} from {}",
                    copy.destination, copy.source
                );
                write_span(output, copy.span);
            }
            MirInstruction::SharedFieldCopy(copy) => {
                let _ = write!(output, "shared-field-copy {} from ", copy.destination);
                dump_place(output, &copy.source);
                write_span(output, copy.span);
            }
            MirInstruction::SharedCast(cast) => {
                output.push_str("shared-cast-static ");
                dump_shared_cast(output, cast);
                write_span(output, cast.span);
            }
            MirInstruction::SharedMove(transfer) => {
                let _ = write!(
                    output,
                    "shared-move {} from {}",
                    transfer.destination, transfer.source
                );
                write_span(output, transfer.span);
            }
            MirInstruction::SharedRelease(release) => {
                let _ = write!(output, "shared-release {}", release.owner);
                write_span(output, release.span);
            }
            MirInstruction::SharedFieldInitialize(initialize) => {
                output.push_str("shared-field-initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(output, " from {}", initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::SharedFieldReplace(replace) => {
                output.push_str("shared-field-replace ");
                dump_place(output, &replace.destination);
                let _ = write!(output, " from {}", replace.source);
                dump_cell_write_authorization(output, replace.authorization);
                dump_final_write_authorization(output, replace.final_authorization);
                write_span(output, replace.span);
            }
            MirInstruction::StringInitialize(initialize) => {
                output.push_str("string-initialize ");
                dump_place(output, &initialize.destination);
                let _ = write!(
                    output,
                    " from {} backing {} : class {} fields [{}, {}, {}, {}] start {} length {} hash-code absent",
                    initialize.data,
                    initialize.backing,
                    initialize.class,
                    initialize.storage_field,
                    initialize.start_field,
                    initialize.length_field,
                    initialize.hash_code_field,
                    initialize.start,
                    initialize.length
                );
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalInitialize(initialize) => {
                output.push_str("optional-initialize ");
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_optional_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalAssign(assignment) => {
                output.push_str("optional-assign ");
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_optional_source(output, &assignment.source);
                dump_cell_write_authorization(output, assignment.authorization);
                dump_final_write_authorization(output, assignment.final_authorization);
                write_span(output, assignment.span);
            }
            MirInstruction::AggregateOptionalInitialize(initialize) => {
                let _ = write!(
                    output,
                    "aggregate-optional-initialize {} ",
                    initialize.optional
                );
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_aggregate_optional_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::AggregateOptionalAssign(assignment) => {
                let _ = write!(output, "aggregate-optional-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_aggregate_optional_source(output, &assignment.source);
                dump_cell_write_authorization(output, assignment.authorization);
                dump_final_write_authorization(output, assignment.final_authorization);
                write_span(output, assignment.span);
            }
            MirInstruction::AggregateOptionalPublish(publish) => {
                let _ = write!(output, "aggregate-optional-publish {} ", publish.optional);
                dump_place(output, &publish.destination);
                write_span(output, publish.span);
            }
            MirInstruction::AggregateOptionalCleanup(cleanup) => {
                let _ = write!(output, "aggregate-optional-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::OptionalSharedInitialize(initialize) => {
                let _ = write!(
                    output,
                    "optional-shared-initialize {} ",
                    initialize.optional
                );
                dump_place(output, &initialize.destination);
                output.push_str(" from ");
                dump_optional_shared_source(output, &initialize.source);
                write_span(output, initialize.span);
            }
            MirInstruction::OptionalSharedAssign(assignment) => {
                let _ = write!(output, "optional-shared-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                output.push_str(" from ");
                dump_optional_shared_source(output, &assignment.source);
                dump_cell_write_authorization(output, assignment.authorization);
                dump_final_write_authorization(output, assignment.final_authorization);
                write_span(output, assignment.span);
            }
            MirInstruction::OptionalSharedCleanup(cleanup) => {
                let _ = write!(output, "optional-shared-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::ClassOptionalInitialize(initialize) => {
                let _ = write!(output, "class-optional-initialize {} ", initialize.optional);
                dump_place(output, &initialize.destination);
                let _ = write!(output, " : class {}?", initialize.class);
                write_span(output, initialize.span);
            }
            MirInstruction::ClassOptionalAssign(assignment) => {
                let _ = write!(output, "class-optional-assign {} ", assignment.optional);
                dump_place(output, &assignment.destination);
                let _ = write!(output, " : class {}?", assignment.class);
                dump_cell_write_authorization(output, assignment.authorization);
                dump_final_write_authorization(output, assignment.final_authorization);
                write_span(output, assignment.span);
            }
            MirInstruction::ClassOptionalPublish(publish) => {
                let _ = write!(output, "class-optional-publish {} ", publish.optional);
                dump_place(output, &publish.destination);
                write_span(output, publish.span);
            }
            MirInstruction::ClassOptionalCleanup(cleanup) => {
                let _ = write!(output, "class-optional-cleanup {} ", cleanup.optional);
                dump_place(output, &cleanup.destination);
                write_span(output, cleanup.span);
            }
            MirInstruction::EndOptionalView(end) => {
                let _ = write!(
                    output,
                    "end-optional-view {} optional {} ",
                    end.guard, end.optional
                );
                dump_place(output, &end.source);
                let _ = write!(output, " : payload {:?}", end.payload);
                write_span(output, end.span);
            }
            MirInstruction::EndOptionalBoxView(end) => {
                let _ = write!(
                    output,
                    "end-optional-box-view {} {} layer {} owner {}",
                    end.guard, end.box_target, end.layer, end.owner
                );
                write_span(output, end.span);
            }
            MirInstruction::Array(instruction) => dump_array_instruction(output, instruction),
            MirInstruction::Io(instruction) => dump_io_instruction(output, instruction),
        }
        output.push('\n');
    }
    output.push_str("          ");
    match &block.terminator {
        Some(MirTerminator::Return { value, span }) => {
            output.push_str("return");
            if let Some(value) = value {
                let _ = write!(output, " {value}");
            }
            write_span(output, *span);
        }
        Some(MirTerminator::ReturnShared { owner, span }) => {
            let _ = write!(output, "return-shared {owner}");
            write_span(output, *span);
        }
        Some(MirTerminator::ReturnOptionalShared { owner, span }) => {
            let _ = write!(output, "return-optional-shared {owner}");
            write_span(output, *span);
        }
        Some(MirTerminator::Goto { target, span }) => {
            let _ = write!(output, "goto {target}");
            write_span(output, *span);
        }
        Some(MirTerminator::Branch {
            condition,
            true_target,
            false_target,
            span,
        }) => {
            let _ = write!(
                output,
                "branch {condition}, true {true_target}, false {false_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ShiftCountCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "shift-count-check {}.{} left {} count {} result {} width {} -> {success_target} else {failure_target}",
                check.operation.mnemonic(),
                check.operation.left.name(),
                check.left,
                check.count,
                check.result,
                check.operation.width(),
            );
            write_span(output, *span);
        }
        Some(MirTerminator::IntegerDivisorCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "integer-divisor-check {}.{} dividend={} divisor={} result={} -> {success_target} else {failure_target}",
                check.operation.mnemonic(),
                check.operation.operand.name(),
                check.dividend,
                check.divisor,
                check.result,
            );
            write_span(output, *span);
        }
        Some(MirTerminator::PrimitiveCastRangeCheck {
            check,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "primitive-cast-range-check f64.{} source={} result={} finite trunc=toward-zero -> {success_target} else {failure_target}",
                check.relation.target.name(),
                check.source,
                check.result,
            );
            write_span(output, *span);
        }
        Some(MirTerminator::CheckedCast {
            binding,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(output, "checked-cast {} = ", binding.destination);
            dump_object_view(output, &binding.view);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::SharedCast {
            cast,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("shared-cast-runtime ");
            dump_shared_cast(output, cast);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::OptionalUnwrap {
            source,
            destination,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("optional-unwrap ");
            dump_place(output, source);
            let _ = write!(
                output,
                " into {destination}, success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::OptionalSharedUnwrap {
            unwrap,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(output, "optional-shared-unwrap {} ", unwrap.optional);
            dump_place(output, &unwrap.source);
            let _ = write!(
                output,
                " into {}, success {success_target}, failure {failure_target}",
                unwrap.destination
            );
            write_span(output, *span);
        }
        Some(MirTerminator::BeginOptionalView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span,
        }) => {
            let _ = write!(
                output,
                "begin-optional-view {} optional {} ",
                begin.guard, begin.optional
            );
            dump_place(output, &begin.source);
            let _ = write!(
                output,
                " : payload {}, success {success_target}, absent {absent_target}, overflow {overflow_target}",
                format_args!("{:?}", begin.payload)
            );
            write_span(output, *span);
        }
        Some(MirTerminator::BeginOptionalBoxView {
            begin,
            success_target,
            absent_target,
            overflow_target,
            span,
        }) => {
            let _ = write!(
                output,
                "begin-optional-box-view {} {} layer {} owner {}, success {success_target}, absent {absent_target}, overflow {overflow_target}",
                begin.guard, begin.box_target, begin.layer, begin.owner
            );
            write_span(output, *span);
        }
        Some(MirTerminator::CheckOptionalMutation {
            source,
            success_target,
            failure_target,
            span,
        }) => {
            output.push_str("check-optional-mutation ");
            dump_place(output, source);
            let _ = write!(
                output,
                ", success {success_target}, failure {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayPositionCheck {
            position,
            kind,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-position-check {position} {kind:?} -> {success_target} else {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayOperationCheck {
            failure,
            success_target,
            failure_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-operation-check {failure:?} -> {success_target} else {failure_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::ArrayLoop {
            backing,
            index,
            length,
            kind,
            body_target,
            complete_target,
            span,
        }) => {
            let _ = write!(
                output,
                "array-loop {kind:?} {backing}[{index}] < {length} -> {body_target} else {complete_target}"
            );
            write_span(output, *span);
        }
        Some(MirTerminator::Terminate { reason, span }) => {
            let _ = write!(output, "terminate {}", reason.mnemonic());
            write_span(output, *span);
        }
        Some(MirTerminator::Panic { message, span }) => {
            let _ = write!(output, "panic ");
            dump_place(output, message);
            write_span(output, *span);
        }
        None => output.push_str("<unterminated>"),
    }
    output.push('\n');
}

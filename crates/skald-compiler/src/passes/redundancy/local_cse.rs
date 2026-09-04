//! Read-only census of exact same-block primitive common subexpressions.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{MirLocalIdentitySite, MirRewriteError, MirValueUseRole},
        BlockId, MirBinaryOperation, MirComparisonOperand, MirDefinitionRef, MirInstruction,
        MirPrimitiveComparison, MirRvalueKind, MirType, MirUnaryOperation, ValueId,
    },
    passes::{pipeline::PrimitiveConstant, VerifiedFinalMirProgram, VerifiedProofMirProgram},
};

use super::{
    cse_model::{
        LocalCseBlocker, LocalCseCallableObservation, LocalCseConsumer, LocalCseCount,
        LocalCseExcludedFamily, LocalCseObservation, LocalCseObservationCounts,
        LocalCseOperationFamily, LocalCseOutcome,
    },
    scalar_spill::ScalarSpillFacts,
};

mod accumulator;
use accumulator::Accumulator;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExpressionKey {
    Unary {
        operation: MirUnaryOperation,
        ty: MirType,
        operand: ValueId,
    },
    Binary {
        operation: MirBinaryOperation,
        ty: MirType,
        left: ValueId,
        right: ValueId,
    },
    Comparison {
        operation: MirPrimitiveComparison,
        ty: MirType,
        left: ValueId,
        right: ValueId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OperandFact {
    Value(ValueId),
    Constant(PrimitiveConstant),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VirtualExpressionKey {
    Unary {
        operation: MirUnaryOperation,
        ty: MirType,
        operand: OperandFact,
    },
    Binary {
        operation: MirBinaryOperation,
        ty: MirType,
        left: OperandFact,
        right: OperandFact,
    },
    Comparison {
        operation: MirPrimitiveComparison,
        ty: MirType,
        left: OperandFact,
        right: OperandFact,
    },
}

#[derive(Clone, Copy)]
struct Site {
    callable: CallableId,
    block: usize,
    block_id: BlockId,
    instruction: usize,
    result: ValueId,
    key: ExpressionKey,
}

#[derive(Clone, Copy)]
struct Seen {
    key: ExpressionKey,
    first: Site,
    repetitions: u64,
}

#[derive(Clone, Copy)]
struct VirtualSeen {
    key: VirtualExpressionKey,
    uses_spill_fact: bool,
}

/// Measures exact same-block integer and boolean common subexpressions
/// without cloning, mutating, or invalidating verified final MIR.
pub fn analyze_local_primitive_common_subexpressions(
    verified: &VerifiedFinalMirProgram,
) -> LocalCseObservation {
    analyze_program(verified.program())
}

/// Measures the same opportunities at a proof-rich inspection checkpoint.
pub fn analyze_proof_local_primitive_common_subexpressions(
    verified: &VerifiedProofMirProgram,
) -> LocalCseObservation {
    analyze_program(verified.program())
}

fn analyze_program(program: &crate::mir::MirProgram) -> LocalCseObservation {
    let mut total = Accumulator::default();
    let mut callables = Vec::new();
    for definition in program.executable_definitions() {
        let callable = definition.callable();
        let observed = analyze_definition(definition)
            .expect("verified MIR must have coherent callable-local identities");
        if observed.has_observations() {
            total.merge(&observed);
            let affected = u64::from(observed.counts.interesting != 0);
            let examples = observed.examples.clone();
            callables.push(LocalCseCallableObservation::new(
                callable,
                observed.finish(affected),
                examples,
            ));
        }
    }
    let affected_callables = callables
        .iter()
        .filter(|observation| observation.counts().interesting() != 0)
        .count() as u64;
    let examples = total.examples.clone();
    LocalCseObservation::new(total.finish(affected_callables), callables, examples)
}

fn analyze_definition(definition: MirDefinitionRef<'_>) -> Result<Accumulator, MirRewriteError> {
    let mut observed = Accumulator::default();
    let spill_facts = ScalarSpillFacts::new(definition);
    let literals = literal_facts(definition);
    let malformed_values = definition
        .values()
        .iter()
        .enumerate()
        .any(|(index, value)| value.id != ValueId::new(definition.callable(), index));

    for (block_index, block) in definition.body().blocks.iter().enumerate() {
        let mut seen = Vec::<Seen>::new();
        let mut virtual_seen = Vec::<VirtualSeen>::new();
        for (instruction_index, instruction) in block.instructions.iter().enumerate() {
            let MirInstruction::Assign(assignment) = instruction else {
                observed.increment_excluded(excluded_instruction(instruction));
                continue;
            };
            let Some((key, family)) = expression_key(&assignment.rvalue.kind, assignment.rvalue.ty)
            else {
                observed.increment_excluded(excluded_rvalue(&assignment.rvalue.kind));
                continue;
            };

            observed.increment_inspected();
            observed.increment_operation_family(family);
            let site = Site {
                callable: definition.callable(),
                block: block_index,
                block_id: block.id,
                instruction: instruction_index,
                result: assignment.result,
                key,
            };
            let (virtual_key, uses_spill_fact) =
                virtual_key(&literals, &spill_facts, key, block.id, instruction_index);

            if let Some(previous) = seen.iter_mut().find(|previous| previous.key == key) {
                previous.repetitions = previous.repetitions.saturating_add(1);
                observed.maximum_repetitions(previous.repetitions);
                observed.record_candidate(definition, previous.first, site, malformed_values)?;
            } else {
                observed.increment_non_candidate();
                seen.push(Seen {
                    key,
                    first: site,
                    repetitions: 0,
                });
                if virtual_seen.iter().any(|previous| {
                    previous.key == virtual_key && (previous.uses_spill_fact || uses_spill_fact)
                }) {
                    observed.increment_scalar_spill_unlock();
                }
            }

            virtual_seen.push(VirtualSeen {
                key: virtual_key,
                uses_spill_fact,
            });
        }
    }
    Ok(observed)
}

#[cfg(test)]
pub(super) fn analyze_unverified_definition(
    definition: MirDefinitionRef<'_>,
) -> Result<LocalCseObservationCounts, MirRewriteError> {
    analyze_definition(definition).map(|observed| observed.finish(1))
}

fn expression_key(
    kind: &MirRvalueKind,
    ty: MirType,
) -> Option<(ExpressionKey, LocalCseOperationFamily)> {
    match *kind {
        MirRvalueKind::Unary { operation, operand } => {
            let family = match operation {
                MirUnaryOperation::NegateF64 => return None,
                MirUnaryOperation::LogicalNotBool => LocalCseOperationFamily::BooleanUnary,
                MirUnaryOperation::NegateI64 | MirUnaryOperation::BitwiseComplement(_) => {
                    LocalCseOperationFamily::IntegerUnary
                }
            };
            Some((
                ExpressionKey::Unary {
                    operation,
                    ty,
                    operand,
                },
                family,
            ))
        }
        MirRvalueKind::Binary {
            operation,
            left,
            right,
        } => {
            if matches!(
                operation,
                MirBinaryOperation::AddF64
                    | MirBinaryOperation::SubtractF64
                    | MirBinaryOperation::MultiplyF64
                    | MirBinaryOperation::DivideF64
            ) {
                return None;
            }
            Some((
                ExpressionKey::Binary {
                    operation,
                    ty,
                    left,
                    right,
                },
                LocalCseOperationFamily::IntegerBinary,
            ))
        }
        MirRvalueKind::PrimitiveComparison {
            operation,
            left,
            right,
        } => {
            let family = match operation.operand {
                MirComparisonOperand::Integer(_) => LocalCseOperationFamily::IntegerComparison,
                MirComparisonOperand::Bool => LocalCseOperationFamily::BooleanComparison,
                MirComparisonOperand::F64 => return None,
            };
            Some((
                ExpressionKey::Comparison {
                    operation,
                    ty,
                    left,
                    right,
                },
                family,
            ))
        }
        _ => None,
    }
}

fn virtual_key(
    literals: &BTreeMap<ValueId, PrimitiveConstant>,
    spill_facts: &ScalarSpillFacts<'_>,
    key: ExpressionKey,
    block: crate::mir::BlockId,
    instruction: usize,
) -> (VirtualExpressionKey, bool) {
    let mut used_spill = false;
    let mut operand = |value| {
        if let Some(constant) = spill_facts.constant_at_instruction(value, block, instruction) {
            used_spill = true;
            OperandFact::Constant(constant)
        } else if let Some(constant) = literals.get(&value).copied() {
            OperandFact::Constant(constant)
        } else {
            OperandFact::Value(value)
        }
    };
    let key = match key {
        ExpressionKey::Unary {
            operation,
            ty,
            operand: value,
        } => VirtualExpressionKey::Unary {
            operation,
            ty,
            operand: operand(value),
        },
        ExpressionKey::Binary {
            operation,
            ty,
            left,
            right,
        } => VirtualExpressionKey::Binary {
            operation,
            ty,
            left: operand(left),
            right: operand(right),
        },
        ExpressionKey::Comparison {
            operation,
            ty,
            left,
            right,
        } => VirtualExpressionKey::Comparison {
            operation,
            ty,
            left: operand(left),
            right: operand(right),
        },
    };
    (key, used_spill)
}

fn literal_facts(definition: MirDefinitionRef<'_>) -> BTreeMap<ValueId, PrimitiveConstant> {
    let mut literals = BTreeMap::new();
    for block in &definition.body().blocks {
        for instruction in &block.instructions {
            let MirInstruction::Assign(assignment) = instruction else {
                continue;
            };
            let constant = match assignment.rvalue.kind {
                MirRvalueKind::ConstantI64(value) => Some(PrimitiveConstant::I64(value)),
                MirRvalueKind::ConstantU64(value) => Some(PrimitiveConstant::U64(value)),
                MirRvalueKind::ConstantU8(value) => Some(PrimitiveConstant::U8(value)),
                MirRvalueKind::ConstantBool(value) => Some(PrimitiveConstant::Bool(value)),
                _ => None,
            };
            if let Some(constant) = constant {
                literals.insert(assignment.result, constant);
            }
        }
    }
    literals
}

fn validation_barriers(
    definition: MirDefinitionRef<'_>,
    site: Site,
    malformed_values: bool,
) -> BTreeSet<LocalCseBlocker> {
    let mut barriers = BTreeSet::new();
    if malformed_values {
        barriers.insert(LocalCseBlocker::MalformedIdentity);
    }
    let mut validate_value = |value: ValueId, expected: MirType| {
        let declaration = definition.value(value);
        if value.callable() != definition.callable() || declaration.is_none() {
            barriers.insert(LocalCseBlocker::MalformedIdentity);
        } else if declaration.map(|value| value.ty) != Some(expected) {
            barriers.insert(LocalCseBlocker::UnsupportedTypeOrOperation);
        }
    };
    match site.key {
        ExpressionKey::Unary {
            operation,
            ty,
            operand,
        } => {
            validate_value(operand, operation.operand_type());
            validate_value(site.result, ty);
            if ty != operation.result_type() {
                barriers.insert(LocalCseBlocker::UnsupportedTypeOrOperation);
            }
        }
        ExpressionKey::Binary {
            operation,
            ty,
            left,
            right,
        } => {
            validate_value(left, operation.operand_type());
            validate_value(right, operation.operand_type());
            validate_value(site.result, ty);
            if ty != operation.result_type() {
                barriers.insert(LocalCseBlocker::UnsupportedTypeOrOperation);
            }
        }
        ExpressionKey::Comparison {
            operation,
            ty,
            left,
            right,
        } => {
            validate_value(left, operation.operand_type());
            validate_value(right, operation.operand_type());
            validate_value(site.result, ty);
            if !operation.is_valid() || ty != operation.result_type() {
                barriers.insert(LocalCseBlocker::UnsupportedTypeOrOperation);
            }
        }
    }
    barriers
}

fn add_use_barriers(
    barriers: &mut BTreeSet<LocalCseBlocker>,
    uses: &crate::mir::rewrite::MirValueUseSites,
    block: usize,
) {
    for use_site in uses.uses() {
        if let Some(blocker) = use_blocker(use_site.role()) {
            barriers.insert(blocker);
        }
        let same_block = match use_site.site() {
            MirLocalIdentitySite::Instruction {
                block: use_block, ..
            }
            | MirLocalIdentitySite::Terminator(use_block) => use_block == block,
            _ => false,
        };
        if !same_block {
            barriers.insert(LocalCseBlocker::ControlFlowBoundary);
        }
    }
}

const fn use_blocker(role: MirValueUseRole) -> Option<LocalCseBlocker> {
    match role {
        MirValueUseRole::InputOutput => Some(LocalCseBlocker::SourceObservation),
        MirValueUseRole::CheckedProtocol
        | MirValueUseRole::ProofMetadata
        | MirValueUseRole::OwnershipOrLifecycle
        | MirValueUseRole::Unknown => Some(LocalCseBlocker::ProtectedMetadataOrUse),
        MirValueUseRole::OrdinaryScalarRvalue(_)
        | MirValueUseRole::OrdinaryPrimitiveCast
        | MirValueUseRole::OrdinaryStore
        | MirValueUseRole::OrdinaryCall(_)
        | MirValueUseRole::OrdinaryReturn
        | MirValueUseRole::OrdinaryBranch => None,
    }
}

fn consumer(role: MirValueUseRole) -> LocalCseConsumer {
    match role {
        MirValueUseRole::OrdinaryScalarRvalue(_) => LocalCseConsumer::TotalPrimitive,
        MirValueUseRole::OrdinaryPrimitiveCast => LocalCseConsumer::PrimitiveCast,
        MirValueUseRole::OrdinaryBranch => LocalCseConsumer::ConditionalBranch,
        MirValueUseRole::OrdinaryStore => LocalCseConsumer::Store,
        MirValueUseRole::OrdinaryReturn => LocalCseConsumer::Return,
        MirValueUseRole::OrdinaryCall(_) => LocalCseConsumer::Call,
        MirValueUseRole::CheckedProtocol => LocalCseConsumer::CheckedProtocol,
        MirValueUseRole::ProofMetadata => LocalCseConsumer::ProtectedMetadata,
        MirValueUseRole::OwnershipOrLifecycle => LocalCseConsumer::OwnershipOrLifecycle,
        MirValueUseRole::InputOutput => LocalCseConsumer::InputOutput,
        MirValueUseRole::Unknown => LocalCseConsumer::Other,
    }
}

fn excluded_rvalue(kind: &MirRvalueKind) -> LocalCseExcludedFamily {
    match kind {
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_) => LocalCseExcludedFamily::Constant,
        MirRvalueKind::PrimitiveCast { .. } => LocalCseExcludedFamily::Cast,
        MirRvalueKind::Load(_) => LocalCseExcludedFamily::Load,
        MirRvalueKind::Unary {
            operation: MirUnaryOperation::NegateF64,
            ..
        }
        | MirRvalueKind::Binary {
            operation:
                MirBinaryOperation::AddF64
                | MirBinaryOperation::SubtractF64
                | MirBinaryOperation::MultiplyF64
                | MirBinaryOperation::DivideF64,
            ..
        }
        | MirRvalueKind::PrimitiveComparison {
            operation:
                MirPrimitiveComparison {
                    operand: MirComparisonOperand::F64,
                    ..
                },
            ..
        } => LocalCseExcludedFamily::FloatingOperation,
        MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. } => LocalCseExcludedFamily::CheckedProtocol,
        MirRvalueKind::PathCondition(_)
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => LocalCseExcludedFamily::SemanticQuery,
        MirRvalueKind::CallableAddress(_) => LocalCseExcludedFamily::Other,
        MirRvalueKind::Unary { .. }
        | MirRvalueKind::Binary { .. }
        | MirRvalueKind::PrimitiveComparison { .. } => LocalCseExcludedFamily::Other,
    }
}

fn excluded_instruction(instruction: &MirInstruction) -> LocalCseExcludedFamily {
    match instruction {
        MirInstruction::Call(_) => LocalCseExcludedFamily::Call,
        MirInstruction::Io(_) => LocalCseExcludedFamily::SourceObservation,
        MirInstruction::Store(_) | MirInstruction::Array(_) => {
            LocalCseExcludedFamily::SemanticQuery
        }
        MirInstruction::Assign(_) => LocalCseExcludedFamily::Other,
        _ => LocalCseExcludedFamily::OwnershipOrLifecycle,
    }
}

#[cfg(test)]
#[path = "local_cse/tests.rs"]
mod tests;

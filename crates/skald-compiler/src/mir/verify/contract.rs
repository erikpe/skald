//! Verification-stage ownership and proof-provenance classification.
//!
//! Proof-rich verification establishes every path-sensitive language
//! invariant. Normalized verification is deliberately narrower: it checks
//! executable structure after the future one-way normalizer has consumed the
//! proof-only path and logical records. Keeping the distinction here prevents
//! individual checks from silently choosing their own idea of the boundary.

use std::fmt;

use super::{
    super::model::{
        MirDefinitionRef, MirInstruction, MirRvalueKind, MirStorageKind, MirTerminator,
    },
    context::Verifier,
};
use crate::mir::{BlockId, StorageId, ValueId};

/// The verifier contract applied to one MIR product.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MirVerificationContract {
    /// Producer and optimization MIR retaining path-sensitive proof records.
    ProofRich,
    /// Executable MIR after all consumable proof provenance has been removed.
    #[cfg(test)]
    Normalized,
}

impl MirVerificationContract {
    pub(super) const fn requires_proof_provenance(self) -> bool {
        matches!(self, Self::ProofRich)
    }
}

/// How one MIR form participates in the post-proof boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::mir) enum MirProofDisposition {
    /// The form remains part of executable or continuing semantic MIR.
    PermanentSemantic,
    /// The form exists only to prove lowering and is removed after proof use.
    ConsumableProof,
    /// The form contains executable behavior plus a consumable proof identity.
    ExecutableCarrierWithProof,
}

/// Proof-only record families consumed by provenance normalization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MirProofRecordKind {
    PathCondition,
    LogicalExpression,
}

/// Classifies every proof record family at the normalization boundary.
pub(super) const fn classify_proof_record(record: MirProofRecordKind) -> MirProofDisposition {
    match record {
        MirProofRecordKind::PathCondition | MirProofRecordKind::LogicalExpression => {
            MirProofDisposition::ConsumableProof
        }
    }
}

impl MirProofDisposition {
    const fn description(self) -> &'static str {
        match self {
            Self::PermanentSemantic => "permanent semantic",
            Self::ConsumableProof => "consumable proof",
            Self::ExecutableCarrierWithProof => "executable carrier with proof",
        }
    }
}

/// Closed normalized-MIR failures owned by the proof boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MirNormalizedInvariantViolation {
    PathConditionRecords {
        count: usize,
    },
    LogicalExpressionRecords {
        count: usize,
    },
    PathConditionStorage {
        storage: StorageId,
    },
    PathConditionRvalue {
        result: ValueId,
    },
    UnexpectedProofInstruction {
        index: usize,
        disposition: MirProofDisposition,
    },
    UnexpectedProofTerminator {
        disposition: MirProofDisposition,
    },
}

impl fmt::Display for MirNormalizedInvariantViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathConditionRecords { count } => write!(
                formatter,
                "normalized MIR retains {count} path-condition record(s)"
            ),
            Self::LogicalExpressionRecords { count } => write!(
                formatter,
                "normalized MIR retains {count} logical-expression record(s)"
            ),
            Self::PathConditionStorage { storage } => write!(
                formatter,
                "normalized MIR retains path-condition storage {storage}"
            ),
            Self::PathConditionRvalue { result } => write!(
                formatter,
                "normalized MIR value {result} retains a path-condition rvalue"
            ),
            Self::UnexpectedProofInstruction { index, disposition } => write!(
                formatter,
                "normalized MIR instruction {index} has unsupported {} provenance",
                disposition.description()
            ),
            Self::UnexpectedProofTerminator { disposition } => write!(
                formatter,
                "normalized MIR terminator has unsupported {} provenance",
                disposition.description()
            ),
        }
    }
}

/// The role of a callable-local identity traversal site in CFG retention.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::mir) enum MirIdentitySiteRole {
    BodyEntry,
    PermanentAttachment,
    ConsumableProof,
    Ordinary,
}

/// Classifies every callable-local traversal site.
///
/// The exhaustive match is a compile-time maintenance point shared by current
/// proof-aware CFG retention and the future normalizer.
pub(in crate::mir) const fn classify_local_identity_site(
    site: crate::mir::rewrite::MirLocalIdentitySite,
) -> MirIdentitySiteRole {
    use crate::mir::rewrite::MirLocalIdentitySite;

    match site {
        MirLocalIdentitySite::BodyEntry => MirIdentitySiteRole::BodyEntry,
        MirLocalIdentitySite::StaticPublicationInitializationExit
        | MirLocalIdentitySite::StaticPublicationCleanupEntry => {
            MirIdentitySiteRole::PermanentAttachment
        }
        MirLocalIdentitySite::PathCondition(_) | MirLocalIdentitySite::LogicalExpression(_) => {
            MirIdentitySiteRole::ConsumableProof
        }
        MirLocalIdentitySite::ReturnStorage
        | MirLocalIdentitySite::Receiver
        | MirLocalIdentitySite::Parameter(_)
        | MirLocalIdentitySite::StorageDeclaration(_)
        | MirLocalIdentitySite::ValueDeclaration(_)
        | MirLocalIdentitySite::BlockDeclaration(_)
        | MirLocalIdentitySite::Instruction { .. }
        | MirLocalIdentitySite::Terminator(_) => MirIdentitySiteRole::Ordinary,
    }
}

pub(in crate::mir) const fn classify_storage_kind(kind: MirStorageKind) -> MirProofDisposition {
    match kind {
        MirStorageKind::PathCondition => MirProofDisposition::ExecutableCarrierWithProof,
        MirStorageKind::Return
        | MirStorageKind::Receiver
        | MirStorageKind::Parameter
        | MirStorageKind::AliasParameter(_)
        | MirStorageKind::CheckedView(_)
        | MirStorageKind::Local
        | MirStorageKind::Argument
        | MirStorageKind::Temporary
        | MirStorageKind::SharedAnchor
        | MirStorageKind::ScalarSpill
        | MirStorageKind::PrimitiveAlias
        | MirStorageKind::OptionalUnwrap
        | MirStorageKind::SharedAllocation
        | MirStorageKind::ArrayBacking
        | MirStorageKind::ArrayProduced
        | MirStorageKind::ArraySlice
        | MirStorageKind::ArrayPosition
        | MirStorageKind::ArrayAnchor(_)
        | MirStorageKind::ArrayAlias(_) => MirProofDisposition::PermanentSemantic,
    }
}

pub(in crate::mir) const fn classify_rvalue_kind(kind: &MirRvalueKind) -> MirProofDisposition {
    match kind {
        MirRvalueKind::PathCondition(_) => MirProofDisposition::ExecutableCarrierWithProof,
        MirRvalueKind::ConstantI64(_)
        | MirRvalueKind::ConstantU64(_)
        | MirRvalueKind::ConstantU8(_)
        | MirRvalueKind::ConstantF64Bits(_)
        | MirRvalueKind::ConstantBool(_)
        | MirRvalueKind::CallableAddress(_)
        | MirRvalueKind::Load(_)
        | MirRvalueKind::Unary { .. }
        | MirRvalueKind::Binary { .. }
        | MirRvalueKind::IntegerDivision { .. }
        | MirRvalueKind::Shift { .. }
        | MirRvalueKind::PrimitiveComparison { .. }
        | MirRvalueKind::PrimitiveCast { .. }
        | MirRvalueKind::CheckedF64ToInteger { .. }
        | MirRvalueKind::TypeTest { .. }
        | MirRvalueKind::OptionalPresence { .. }
        | MirRvalueKind::OptionalBoxPresence { .. }
        | MirRvalueKind::ArrayLength { .. } => MirProofDisposition::PermanentSemantic,
    }
}

pub(in crate::mir) const fn classify_instruction(
    instruction: &MirInstruction,
) -> MirProofDisposition {
    match instruction {
        MirInstruction::StorageLive(_)
        | MirInstruction::StorageDead(_)
        | MirInstruction::Assign(_)
        | MirInstruction::Call(_)
        | MirInstruction::Cleanup(_)
        | MirInstruction::Initialize(_)
        | MirInstruction::Store(_)
        | MirInstruction::CopyConstruct(_)
        | MirInstruction::CopyAssign(_)
        | MirInstruction::EndFullExpression(_)
        | MirInstruction::BindCheckedView(_)
        | MirInstruction::EndCheckedView(_)
        | MirInstruction::SharedAllocate(_)
        | MirInstruction::SharedInitialize(_)
        | MirInstruction::SharedPublish(_)
        | MirInstruction::SharedStatic(_)
        | MirInstruction::SharedAdopt(_)
        | MirInstruction::SharedCopy(_)
        | MirInstruction::SharedFieldCopy(_)
        | MirInstruction::SharedCast(_)
        | MirInstruction::SharedMove(_)
        | MirInstruction::SharedRelease(_)
        | MirInstruction::SharedFieldInitialize(_)
        | MirInstruction::SharedFieldReplace(_)
        | MirInstruction::StringInitialize(_)
        | MirInstruction::OptionalInitialize(_)
        | MirInstruction::OptionalAssign(_)
        | MirInstruction::AggregateOptionalInitialize(_)
        | MirInstruction::AggregateOptionalAssign(_)
        | MirInstruction::AggregateOptionalPublish(_)
        | MirInstruction::AggregateOptionalCleanup(_)
        | MirInstruction::ClassOptionalInitialize(_)
        | MirInstruction::ClassOptionalAssign(_)
        | MirInstruction::ClassOptionalPublish(_)
        | MirInstruction::ClassOptionalCleanup(_)
        | MirInstruction::EndOptionalView(_)
        | MirInstruction::EndOptionalBoxView(_)
        | MirInstruction::OptionalSharedInitialize(_)
        | MirInstruction::OptionalSharedAssign(_)
        | MirInstruction::OptionalSharedCleanup(_)
        | MirInstruction::Array(_)
        | MirInstruction::Io(_) => MirProofDisposition::PermanentSemantic,
    }
}

pub(in crate::mir) const fn classify_terminator(terminator: &MirTerminator) -> MirProofDisposition {
    match terminator {
        MirTerminator::Return { .. }
        | MirTerminator::ReturnShared { .. }
        | MirTerminator::ReturnOptionalShared { .. }
        | MirTerminator::Panic { .. }
        | MirTerminator::Goto { .. }
        | MirTerminator::Branch { .. }
        | MirTerminator::ShiftCountCheck { .. }
        | MirTerminator::IntegerDivisorCheck { .. }
        | MirTerminator::PrimitiveCastRangeCheck { .. }
        | MirTerminator::CheckedCast { .. }
        | MirTerminator::SharedCast { .. }
        | MirTerminator::OptionalUnwrap { .. }
        | MirTerminator::OptionalSharedUnwrap { .. }
        | MirTerminator::BeginOptionalView { .. }
        | MirTerminator::BeginOptionalBoxView { .. }
        | MirTerminator::CheckOptionalMutation { .. }
        | MirTerminator::ArrayPositionCheck { .. }
        | MirTerminator::ArrayOperationCheck { .. }
        | MirTerminator::ArrayLoop { .. }
        | MirTerminator::Terminate { .. } => MirProofDisposition::PermanentSemantic,
    }
}

impl Verifier<'_> {
    /// Checks the closed post-proof invariant without reconstructing consumed
    /// path dataflow. Shared structural verification still runs afterward.
    pub(super) fn verify_normalized_definition_contract(&mut self, function: MirDefinitionRef<'_>) {
        if self.verification_contract().requires_proof_provenance() {
            return;
        }

        let body = function.body();
        if !body.path_conditions.is_empty() {
            debug_assert_eq!(
                classify_proof_record(MirProofRecordKind::PathCondition),
                MirProofDisposition::ConsumableProof
            );
            self.normalized_function_error(
                function,
                MirNormalizedInvariantViolation::PathConditionRecords {
                    count: body.path_conditions.len(),
                },
            );
        }
        if !body.logical_expressions.is_empty() {
            debug_assert_eq!(
                classify_proof_record(MirProofRecordKind::LogicalExpression),
                MirProofDisposition::ConsumableProof
            );
            self.normalized_function_error(
                function,
                MirNormalizedInvariantViolation::LogicalExpressionRecords {
                    count: body.logical_expressions.len(),
                },
            );
        }

        for storage in function.storage_entries() {
            if classify_storage_kind(storage.kind)
                == MirProofDisposition::ExecutableCarrierWithProof
            {
                self.normalized_function_error(
                    function,
                    MirNormalizedInvariantViolation::PathConditionStorage {
                        storage: storage.id,
                    },
                );
            }
        }

        for block in &body.blocks {
            self.verify_normalized_block_contract(function, block.id);
        }
    }

    fn verify_normalized_block_contract(
        &mut self,
        function: MirDefinitionRef<'_>,
        block_id: BlockId,
    ) {
        let Some(block) = function.block(block_id) else {
            return;
        };
        for (index, instruction) in block.instructions.iter().enumerate() {
            let disposition = classify_instruction(instruction);
            if disposition != MirProofDisposition::PermanentSemantic {
                self.block_error(
                    function.callable(),
                    block.id,
                    MirNormalizedInvariantViolation::UnexpectedProofInstruction {
                        index,
                        disposition,
                    }
                    .to_string(),
                );
            }
            if let MirInstruction::Assign(assignment) = instruction {
                if classify_rvalue_kind(&assignment.rvalue.kind)
                    == MirProofDisposition::ExecutableCarrierWithProof
                {
                    self.block_error(
                        function.callable(),
                        block.id,
                        MirNormalizedInvariantViolation::PathConditionRvalue {
                            result: assignment.result,
                        }
                        .to_string(),
                    );
                }
            }
        }
        if let Some(terminator) = &block.terminator {
            let disposition = classify_terminator(terminator);
            if disposition != MirProofDisposition::PermanentSemantic {
                self.block_error(
                    function.callable(),
                    block.id,
                    MirNormalizedInvariantViolation::UnexpectedProofTerminator { disposition }
                        .to_string(),
                );
            }
        }
    }

    fn normalized_function_error(
        &mut self,
        function: MirDefinitionRef<'_>,
        violation: MirNormalizedInvariantViolation,
    ) {
        self.function_error(function.callable(), violation.to_string());
    }
}

#[cfg(test)]
mod tests;

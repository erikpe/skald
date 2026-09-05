//! Narrow checked-rewrite projection of certified carrier evidence.

use crate::mir::{
    rewrite::MirRewriteError, BlockId, MirDefinitionRef, MirType, StorageId, ValueId,
};

use super::carrier::{
    certify_checked_integer_carriers, CheckedCarrierCertificationObservation,
    CheckedCarrierProtocolRole,
};

/// Carrier position owned by one checked protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(in crate::passes::pipeline::optimizations) enum CheckedCarrierPlanRole {
    FirstOperand,
    SecondOperand,
    Result,
}

/// Immutable certificate projection required by the checked rewrite planner.
///
/// The full callable snapshot revalidates the underlying declaration, access
/// census, dominance, and lifetime evidence before any planned edit occurs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::passes::pipeline::optimizations) struct CheckedCarrierPlanEvidence {
    storage: StorageId,
    source: ValueId,
    loads: Vec<ValueId>,
    ty: MirType,
    check_block: BlockId,
    role: CheckedCarrierPlanRole,
}

impl CheckedCarrierPlanEvidence {
    pub(in crate::passes::pipeline::optimizations) const fn storage(&self) -> StorageId {
        self.storage
    }

    pub(in crate::passes::pipeline::optimizations) const fn source(&self) -> ValueId {
        self.source
    }

    pub(in crate::passes::pipeline::optimizations) fn loads(&self) -> &[ValueId] {
        &self.loads
    }

    pub(in crate::passes::pipeline::optimizations) const fn ty(&self) -> MirType {
        self.ty
    }

    pub(in crate::passes::pipeline::optimizations) const fn check_block(&self) -> BlockId {
        self.check_block
    }

    pub(in crate::passes::pipeline::optimizations) const fn role(&self) -> CheckedCarrierPlanRole {
        self.role
    }
}

/// Returns only fully certified carriers in deterministic protocol order.
pub(in crate::passes::pipeline::optimizations) fn checked_carrier_plan_evidence(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedCarrierPlanEvidence>, MirRewriteError> {
    certify_checked_integer_carriers(definition)?
        .into_iter()
        .filter_map(|observation| match observation {
            CheckedCarrierCertificationObservation::Certified(certificate) => {
                let owner = certificate.protocol_owner();
                Some(Ok(CheckedCarrierPlanEvidence {
                    storage: certificate.storage(),
                    source: certificate.store().source(),
                    loads: certificate
                        .loads()
                        .iter()
                        .map(|load| load.result())
                        .collect(),
                    ty: certificate.ty(),
                    check_block: owner.check_block(),
                    role: match owner.role() {
                        CheckedCarrierProtocolRole::FirstOperand => {
                            CheckedCarrierPlanRole::FirstOperand
                        }
                        CheckedCarrierProtocolRole::SecondOperand => {
                            CheckedCarrierPlanRole::SecondOperand
                        }
                        CheckedCarrierProtocolRole::Result => CheckedCarrierPlanRole::Result,
                    },
                }))
            }
            CheckedCarrierCertificationObservation::Rejected { .. } => None,
        })
        .collect()
}

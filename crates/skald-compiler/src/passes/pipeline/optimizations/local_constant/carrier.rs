//! Auditable certification of compiler-owned checked-protocol scalar storage.

use std::collections::HashSet;

use crate::mir::{
    checked_scalar_dominates,
    rewrite::{
        storage_use_census_for_definition, MirLocalIdentitySite, MirRewriteError,
        MirStoragePlaceUse, MirStorageUseCensusEntry, MirStorageUseRole,
        MirStorageWriteAuthorization,
    },
    BlockId, MirDefinitionRef, MirInstruction, MirPlace, MirRvalueKind, MirStorage, MirStorageKind,
    MirType, StorageId, ValueId,
};
use crate::source::Span;

use super::super::{
    checked_f64_to_integer_topology::{
        observe_checked_f64_to_integer_topologies, CheckedF64ToIntegerTopologyObservation,
    },
    checked_integer_topology::{
        observe_checked_integer_topologies, CheckedIntegerProtocolCheck,
        CheckedIntegerTopologyObservation,
    },
    checked_scalar_topology::{CheckedScalarInstructionSite, CheckedScalarValueSite},
};

/// Checked-scalar protocol family which owns a certified carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum CheckedScalarProtocolFamily {
    IntegerDivision,
    IntegerShift,
    F64ToInteger,
}

/// Carrier position owned by one checked-scalar protocol.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) enum CheckedCarrierProtocolRole {
    FirstOperand,
    SecondOperand,
    Source,
    Result,
}

/// Exact protocol ownership of one candidate scalar carrier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(super) struct CheckedCarrierProtocolOwner {
    check_block: BlockId,
    family: CheckedScalarProtocolFamily,
    role: CheckedCarrierProtocolRole,
}

impl CheckedCarrierProtocolOwner {
    pub(super) const fn check_block(self) -> BlockId {
        self.check_block
    }

    #[cfg(test)]
    pub(super) const fn family(self) -> CheckedScalarProtocolFamily {
        self.family
    }

    pub(super) const fn role(self) -> CheckedCarrierProtocolRole {
        self.role
    }
}

/// Exact ordinary store which seeds a certified carrier.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedCarrierStore {
    site: CheckedScalarInstructionSite,
    source: ValueId,
    span: Span,
}

impl CheckedCarrierStore {
    #[cfg(test)]
    pub(super) const fn site(self) -> CheckedScalarInstructionSite {
        self.site
    }

    pub(super) const fn source(self) -> ValueId {
        self.source
    }

    #[cfg(test)]
    pub(super) const fn span(self) -> Span {
        self.span
    }
}

/// Exact load eligible to receive a propagated carrier fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedCarrierLoad {
    site: CheckedScalarInstructionSite,
    result: ValueId,
    span: Span,
}

impl CheckedCarrierLoad {
    #[cfg(test)]
    pub(super) const fn site(self) -> CheckedScalarInstructionSite {
        self.site
    }

    pub(super) const fn result(self) -> ValueId {
        self.result
    }

    #[cfg(test)]
    pub(super) const fn span(self) -> Span {
        self.span
    }
}

/// Concrete lifetime sites whose relative dominance was checked.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct CheckedCarrierLifetimeEvidence {
    live: CheckedScalarInstructionSite,
    dead: CheckedScalarInstructionSite,
}

impl CheckedCarrierLifetimeEvidence {
    #[cfg(test)]
    pub(super) const fn live(self) -> CheckedScalarInstructionSite {
        self.live
    }

    #[cfg(test)]
    pub(super) const fn dead(self) -> CheckedScalarInstructionSite {
        self.dead
    }
}

/// Immutable proof that one storage edge is safe for local constant transfer.
///
/// The certificate contains owned MIR data and dense identities from one
/// verified callable snapshot. It must be recomputed after any rewrite.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CheckedCarrierCertificate {
    declaration: MirStorage,
    store: CheckedCarrierStore,
    loads: Vec<CheckedCarrierLoad>,
    ty: MirType,
    protocol_owner: CheckedCarrierProtocolOwner,
    lifetime: CheckedCarrierLifetimeEvidence,
}

impl CheckedCarrierCertificate {
    pub(super) const fn storage(&self) -> StorageId {
        self.declaration.id
    }

    #[cfg(test)]
    pub(super) const fn declaration(&self) -> &MirStorage {
        &self.declaration
    }

    pub(super) const fn store(&self) -> CheckedCarrierStore {
        self.store
    }

    pub(super) fn loads(&self) -> &[CheckedCarrierLoad] {
        &self.loads
    }

    pub(super) const fn ty(&self) -> MirType {
        self.ty
    }

    pub(super) const fn protocol_owner(&self) -> CheckedCarrierProtocolOwner {
        self.protocol_owner
    }

    #[cfg(test)]
    pub(super) const fn lifetime(&self) -> CheckedCarrierLifetimeEvidence {
        self.lifetime
    }
}

/// Conservative reason a checked-protocol carrier was left opaque.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedCarrierRejectionReason {
    DuplicateProtocolOwner,
    MissingDeclaration,
    WrongStorageKind,
    WrongStorageType,
    InvalidAccess,
    MissingOrMultipleStores,
    MissingLoads,
    MissingOrMultipleLifetimeMarkers,
    WrongStore,
    WrongLoad,
    WrongProtocolUse,
    StoreSourceTypeMismatch,
    LoadResultTypeMismatch,
    StoreDoesNotDominateLoad,
    IncompatibleLifetime,
}

/// One deterministic certification result in protocol and carrier order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum CheckedCarrierCertificationObservation {
    Certified(Box<CheckedCarrierCertificate>),
    Rejected {
        storage: StorageId,
        protocol_owner: CheckedCarrierProtocolOwner,
        reason: CheckedCarrierRejectionReason,
    },
}

/// The only storage-use dispositions understood by carrier certification.
///
/// Keeping this conversion exhaustive makes a new storage role or place/
/// authorization variant a compile-time maintenance event instead of silently
/// granting it constant-propagation authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckedCarrierUseDisposition {
    Store,
    Load,
    LifetimeLive,
    LifetimeDead,
    Protocol,
    Declaration,
    Reject,
}

pub(super) const fn carrier_use_disposition(
    role: MirStorageUseRole,
) -> CheckedCarrierUseDisposition {
    match role {
        MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::ExactBase,
            authorization: MirStorageWriteAuthorization::None,
        } => CheckedCarrierUseDisposition::Store,
        MirStorageUseRole::OrdinaryRead(MirStoragePlaceUse::ExactBase) => {
            CheckedCarrierUseDisposition::Load
        }
        MirStorageUseRole::LifetimeLive => CheckedCarrierUseDisposition::LifetimeLive,
        MirStorageUseRole::LifetimeDead => CheckedCarrierUseDisposition::LifetimeDead,
        MirStorageUseRole::CheckedProtocol => CheckedCarrierUseDisposition::Protocol,
        MirStorageUseRole::Declaration => CheckedCarrierUseDisposition::Declaration,
        MirStorageUseRole::OrdinaryRead(
            MirStoragePlaceUse::Projected | MirStoragePlaceUse::Alias,
        )
        | MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::ExactBase,
            authorization:
                MirStorageWriteAuthorization::Cell
                | MirStorageWriteAuthorization::Final
                | MirStorageWriteAuthorization::CellAndFinal,
        }
        | MirStorageUseRole::OrdinaryWrite {
            place: MirStoragePlaceUse::Projected | MirStoragePlaceUse::Alias,
            authorization:
                MirStorageWriteAuthorization::None
                | MirStorageWriteAuthorization::Cell
                | MirStorageWriteAuthorization::Final
                | MirStorageWriteAuthorization::CellAndFinal,
        }
        | MirStorageUseRole::Attachment
        | MirStorageUseRole::ProofMetadata
        | MirStorageUseRole::Alias
        | MirStorageUseRole::Call
        | MirStorageUseRole::OwnershipOrLifecycle
        | MirStorageUseRole::InputOutput
        | MirStorageUseRole::OtherExecutable => CheckedCarrierUseDisposition::Reject,
    }
}

/// Certifies only scalar carriers owned by canonical checked protocols.
/// Unrelated storage is intentionally never considered.
pub(super) fn certify_checked_scalar_carriers(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedCarrierCertificationObservation>, MirRewriteError> {
    let mut candidates = checked_integer_carrier_candidates(definition)?;
    candidates.extend(checked_f64_to_integer_carrier_candidates(definition)?);
    certify_carrier_candidates(definition, candidates)
}

/// Existing integer rewrite consumers retain their original protocol scope.
/// In particular, malformed floating-cast topology cannot affect an integer-
/// only plan before the floating rewrite owner exists.
pub(super) fn certify_checked_integer_carriers(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedCarrierCertificationObservation>, MirRewriteError> {
    certify_carrier_candidates(definition, checked_integer_carrier_candidates(definition)?)
}

fn certify_carrier_candidates(
    definition: MirDefinitionRef<'_>,
    mut candidates: Vec<CheckedCarrierCandidate>,
) -> Result<Vec<CheckedCarrierCertificationObservation>, MirRewriteError> {
    let census = storage_use_census_for_definition(definition)?;
    let mut claimed = HashSet::new();
    let mut observations = Vec::new();

    candidates.sort_by_key(|candidate| (candidate.owner.check_block, candidate.owner.role));
    for candidate in candidates {
        let result = if !claimed.insert(candidate.storage) {
            Err(CheckedCarrierRejectionReason::DuplicateProtocolOwner)
        } else {
            certify_one(
                definition,
                census.get(candidate.storage),
                candidate.storage,
                candidate.ty,
                candidate.owner,
                candidate.expected_load,
            )
        };
        observations.push(match result {
            Ok(certificate) => {
                CheckedCarrierCertificationObservation::Certified(Box::new(certificate))
            }
            Err(reason) => CheckedCarrierCertificationObservation::Rejected {
                storage: candidate.storage,
                protocol_owner: candidate.owner,
                reason,
            },
        });
    }
    Ok(observations)
}

#[derive(Clone, Copy)]
struct CheckedCarrierCandidate {
    storage: StorageId,
    ty: MirType,
    owner: CheckedCarrierProtocolOwner,
    expected_load: CheckedScalarValueSite,
}

fn checked_integer_carrier_candidates(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedCarrierCandidate>, MirRewriteError> {
    let mut candidates = Vec::new();
    for observation in observe_checked_integer_topologies(definition)? {
        let CheckedIntegerTopologyObservation::Protocol(topology) = observation else {
            continue;
        };
        let family = match topology.check {
            CheckedIntegerProtocolCheck::Division(_) => {
                CheckedScalarProtocolFamily::IntegerDivision
            }
            CheckedIntegerProtocolCheck::Shift(_) => CheckedScalarProtocolFamily::IntegerShift,
        };
        let [(first, first_ty), (second, second_ty)] = topology.check.operands();
        let (result, result_ty) = topology.check.result();
        candidates.extend([
            CheckedCarrierCandidate {
                storage: first,
                ty: first_ty,
                owner: CheckedCarrierProtocolOwner {
                    check_block: topology.check_block,
                    family,
                    role: CheckedCarrierProtocolRole::FirstOperand,
                },
                expected_load: topology.operand_loads[0],
            },
            CheckedCarrierCandidate {
                storage: second,
                ty: second_ty,
                owner: CheckedCarrierProtocolOwner {
                    check_block: topology.check_block,
                    family,
                    role: CheckedCarrierProtocolRole::SecondOperand,
                },
                expected_load: topology.operand_loads[1],
            },
            CheckedCarrierCandidate {
                storage: result,
                ty: result_ty,
                owner: CheckedCarrierProtocolOwner {
                    check_block: topology.check_block,
                    family,
                    role: CheckedCarrierProtocolRole::Result,
                },
                expected_load: topology.result_reload,
            },
        ]);
    }
    Ok(candidates)
}

fn checked_f64_to_integer_carrier_candidates(
    definition: MirDefinitionRef<'_>,
) -> Result<Vec<CheckedCarrierCandidate>, MirRewriteError> {
    let mut candidates = Vec::new();
    for observation in observe_checked_f64_to_integer_topologies(definition)? {
        let CheckedF64ToIntegerTopologyObservation::Protocol(topology) = observation else {
            continue;
        };
        let (source, source_ty) = topology.source();
        let (result, result_ty) = topology.result();
        candidates.extend([
            CheckedCarrierCandidate {
                storage: source,
                ty: source_ty,
                owner: CheckedCarrierProtocolOwner {
                    check_block: topology.check_block,
                    family: CheckedScalarProtocolFamily::F64ToInteger,
                    role: CheckedCarrierProtocolRole::Source,
                },
                expected_load: topology.source_load,
            },
            CheckedCarrierCandidate {
                storage: result,
                ty: result_ty,
                owner: CheckedCarrierProtocolOwner {
                    check_block: topology.check_block,
                    family: CheckedScalarProtocolFamily::F64ToInteger,
                    role: CheckedCarrierProtocolRole::Result,
                },
                expected_load: topology.result_reload,
            },
        ]);
    }
    Ok(candidates)
}

fn certify_one(
    definition: MirDefinitionRef<'_>,
    census: Option<&MirStorageUseCensusEntry>,
    storage: StorageId,
    ty: MirType,
    protocol_owner: CheckedCarrierProtocolOwner,
    expected_load: CheckedScalarValueSite,
) -> Result<CheckedCarrierCertificate, CheckedCarrierRejectionReason> {
    let census = census.ok_or(CheckedCarrierRejectionReason::MissingDeclaration)?;
    let declaration = definition
        .storage(storage)
        .ok_or(CheckedCarrierRejectionReason::MissingDeclaration)?;
    if census.kind() != MirStorageKind::ScalarSpill || declaration.kind != census.kind() {
        return Err(CheckedCarrierRejectionReason::WrongStorageKind);
    }
    if declaration.ty != ty {
        return Err(CheckedCarrierRejectionReason::WrongStorageType);
    }

    let mut stores = Vec::new();
    let mut loads = Vec::new();
    let mut lives = Vec::new();
    let mut deads = Vec::new();
    let mut protocol_uses = Vec::new();
    for use_site in census.uses() {
        match carrier_use_disposition(use_site.role()) {
            CheckedCarrierUseDisposition::Store => stores.push(use_site.site()),
            CheckedCarrierUseDisposition::Load => loads.push(use_site.site()),
            CheckedCarrierUseDisposition::LifetimeLive => lives.push(use_site.site()),
            CheckedCarrierUseDisposition::LifetimeDead => deads.push(use_site.site()),
            CheckedCarrierUseDisposition::Protocol => protocol_uses.push(use_site.site()),
            CheckedCarrierUseDisposition::Declaration => {
                unreachable!("declarations are not census uses")
            }
            CheckedCarrierUseDisposition::Reject => {
                return Err(CheckedCarrierRejectionReason::InvalidAccess)
            }
        }
    }

    let [store_site] = stores.as_slice() else {
        return Err(CheckedCarrierRejectionReason::MissingOrMultipleStores);
    };
    if loads.is_empty() {
        return Err(CheckedCarrierRejectionReason::MissingLoads);
    }
    let ([live_site], [dead_site]) = (lives.as_slice(), deads.as_slice()) else {
        return Err(CheckedCarrierRejectionReason::MissingOrMultipleLifetimeMarkers);
    };
    let [protocol_site] = protocol_uses.as_slice() else {
        return Err(CheckedCarrierRejectionReason::WrongProtocolUse);
    };
    if *protocol_site != MirLocalIdentitySite::Terminator(protocol_owner.check_block.index()) {
        return Err(CheckedCarrierRejectionReason::WrongProtocolUse);
    }

    let store = exact_store(definition, *store_site, storage)
        .ok_or(CheckedCarrierRejectionReason::WrongStore)?;
    let loads = loads
        .into_iter()
        .map(|site| {
            exact_load(definition, site, storage).ok_or(CheckedCarrierRejectionReason::WrongLoad)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if !loads
        .iter()
        .any(|load| load.site == expected_load.site && load.result == expected_load.value)
    {
        return Err(CheckedCarrierRejectionReason::WrongLoad);
    }
    if definition.value(store.source).map(|value| value.ty) != Some(ty) {
        return Err(CheckedCarrierRejectionReason::StoreSourceTypeMismatch);
    }
    if loads
        .iter()
        .any(|load| definition.value(load.result).map(|value| value.ty) != Some(ty))
    {
        return Err(CheckedCarrierRejectionReason::LoadResultTypeMismatch);
    }
    if loads
        .iter()
        .any(|load| !instruction_dominates(definition, store.site, load.site))
    {
        return Err(CheckedCarrierRejectionReason::StoreDoesNotDominateLoad);
    }
    let live = instruction_site(definition, *live_site)
        .ok_or(CheckedCarrierRejectionReason::IncompatibleLifetime)?;
    let dead = instruction_site(definition, *dead_site)
        .ok_or(CheckedCarrierRejectionReason::IncompatibleLifetime)?;
    if !instruction_dominates(definition, live, store.site)
        || loads.iter().any(|load| {
            !instruction_dominates(definition, live, load.site)
                || lifetime_ends_before_load(definition, dead, load.site)
        })
    {
        return Err(CheckedCarrierRejectionReason::IncompatibleLifetime);
    }

    Ok(CheckedCarrierCertificate {
        declaration: declaration.clone(),
        store,
        loads,
        ty,
        protocol_owner,
        lifetime: CheckedCarrierLifetimeEvidence { live, dead },
    })
}

/// Verified MIR already proves path-sensitive storage lifetime validity. This
/// additional certificate check rejects a locally or unconditionally earlier
/// end, while permitting a common conditional shape where the load executes
/// only on one branch and `StorageDead` sits in the shared join.
fn lifetime_ends_before_load(
    definition: MirDefinitionRef<'_>,
    dead: CheckedScalarInstructionSite,
    load: CheckedScalarInstructionSite,
) -> bool {
    if dead.block == load.block {
        dead.instruction <= load.instruction
    } else {
        checked_scalar_dominates(definition, dead.block, load.block)
    }
}

fn exact_store(
    definition: MirDefinitionRef<'_>,
    site: MirLocalIdentitySite,
    storage: StorageId,
) -> Option<CheckedCarrierStore> {
    let site = instruction_site(definition, site)?;
    let MirInstruction::Store(store) = definition
        .block(site.block)?
        .instructions
        .get(site.instruction)?
    else {
        return None;
    };
    (store.destination == MirPlace::base(storage)
        && store.authorization.is_none()
        && store.final_authorization.is_none())
    .then_some(CheckedCarrierStore {
        site,
        source: store.value,
        span: store.span,
    })
}

fn exact_load(
    definition: MirDefinitionRef<'_>,
    site: MirLocalIdentitySite,
    storage: StorageId,
) -> Option<CheckedCarrierLoad> {
    let site = instruction_site(definition, site)?;
    let MirInstruction::Assign(assignment) = definition
        .block(site.block)?
        .instructions
        .get(site.instruction)?
    else {
        return None;
    };
    matches!(assignment.rvalue.kind, MirRvalueKind::Load(ref place) if *place == MirPlace::base(storage))
        .then_some(CheckedCarrierLoad {
            site,
            result: assignment.result,
            span: assignment.span,
        })
}

fn instruction_site(
    definition: MirDefinitionRef<'_>,
    site: MirLocalIdentitySite,
) -> Option<CheckedScalarInstructionSite> {
    let MirLocalIdentitySite::Instruction { block, instruction } = site else {
        return None;
    };
    Some(CheckedScalarInstructionSite {
        block: BlockId::new(definition.callable(), block),
        instruction,
    })
}

fn instruction_dominates(
    definition: MirDefinitionRef<'_>,
    dominator: CheckedScalarInstructionSite,
    target: CheckedScalarInstructionSite,
) -> bool {
    dominator.block == target.block && dominator.instruction <= target.instruction
        || dominator.block != target.block
            && checked_scalar_dominates(definition, dominator.block, target.block)
}

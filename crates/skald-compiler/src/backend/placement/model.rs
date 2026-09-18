use crate::backend::{
    graph::{SelectedBlockId, SelectedValueId},
    plan::SignatureId,
    selected::{AbiArea, Representation, VerifiedSelectedCallable, ViewId},
};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::backend) enum Site {
    Instruction {
        block: SelectedBlockId,
        ordinal: usize,
    },
    Terminal(SelectedBlockId),
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::backend) enum Assignment {
    Input(usize),
    Parameter {
        block: SelectedBlockId,
        slot: usize,
    },
    Operand {
        site: Site,
        slot: usize,
    },
    EdgeArgument {
        block: SelectedBlockId,
        edge: usize,
        slot: usize,
    },
    Scratch {
        site: Site,
        group: usize,
        slot: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(in crate::backend) enum TransferPoint {
    Entry,
    Before(Site),
    After(Site),
    Edge { block: SelectedBlockId, slot: usize },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) struct StorageId(pub(super) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum Location {
    Resource(ViewId),
    Storage(StorageId),
    Abi {
        signature: SignatureId,
        area: AbiArea,
        index: usize,
    },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum StoragePurpose {
    Home(SelectedValueId),
    Spill(SelectedValueId),
    TransferScratch,
    CalleeSave(ViewId),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum StorageLifetime {
    WholeCallable,
    Transfer(TransferPoint),
}
#[derive(Clone, Debug)]
pub(in crate::backend) struct Storage {
    pub representation: Representation,
    pub bytes: usize,
    pub alignment: usize,
    pub purpose: StoragePurpose,
    pub lifetime: StorageLifetime,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum TransferKind {
    Copy,
    /// Same-width bits, including integer/SIMD movement; never a numeric conversion.
    Bitwise,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::backend) enum TransferValue {
    Selected(SelectedValueId),
    /// Original incoming bits at the target's promised preservation width.
    Preserved(ViewId),
}
#[derive(Clone, Debug)]
pub(in crate::backend) struct Transfer {
    pub value: TransferValue,
    pub source: Location,
    pub destination: Location,
    pub source_representation: Representation,
    pub destination_representation: Representation,
    pub kind: TransferKind,
    /// Required target working views for this move; no implicit scratch is allowed.
    pub scratch: Vec<ViewId>,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::backend) enum PlacementError {
    WrongSnapshot,
    DuplicateAssignment(Assignment),
    MissingAssignment(Assignment),
    UnknownAssignment(Assignment),
    InvalidLocation(Assignment),
    InvalidStorage(usize),
    InvalidTransfer(TransferPoint, usize),
    UnknownTransferPoint(TransferPoint),
}

/// The borrow prevents consuming edits while this draft is in use. No seal,
/// success flag, frame offset or semantic-object ID belongs in this product.
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) struct PlacementDraft<'s, 'p, P> {
    pub(super) selected: &'s VerifiedSelectedCallable<'p, P>,
    pub(super) assignments: BTreeMap<Assignment, Location>,
    pub(super) storage: Vec<Storage>,
    pub(super) transfers: BTreeMap<TransferPoint, Vec<Transfer>>,
}
#[cfg_attr(not(test), allow(dead_code))]
impl<'s, 'p, P> PlacementDraft<'s, 'p, P> {
    pub(in crate::backend) fn new(selected: &'s VerifiedSelectedCallable<'p, P>) -> Self {
        Self {
            selected,
            assignments: BTreeMap::new(),
            storage: vec![],
            transfers: BTreeMap::new(),
        }
    }
    pub(in crate::backend) fn require_selected(
        &self,
        selected: &VerifiedSelectedCallable<'p, P>,
    ) -> Result<(), PlacementError> {
        if self.selected.receipt().same_snapshot(&selected.receipt()) {
            Ok(())
        } else {
            Err(PlacementError::WrongSnapshot)
        }
    }
    pub(in crate::backend) fn assign(
        &mut self,
        assignment: Assignment,
        location: Location,
    ) -> Result<(), PlacementError> {
        if self.assignments.contains_key(&assignment) {
            return Err(PlacementError::DuplicateAssignment(assignment));
        }
        self.assignments.insert(assignment, location);
        Ok(())
    }
    pub(in crate::backend) fn storage(&mut self, requirement: Storage) -> StorageId {
        let id = StorageId(self.storage.len());
        self.storage.push(requirement);
        id
    }
    pub(in crate::backend) fn transfer(&mut self, point: TransferPoint, transfer: Transfer) {
        self.transfers.entry(point).or_default().push(transfer);
    }
}

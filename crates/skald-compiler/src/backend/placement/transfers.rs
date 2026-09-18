//! Deterministic parallel copies. A typed point-local slot breaks each dependency
//! cycle; target working views must preserve all pending sources and final outputs.
use super::{model::*, requirements::*, target::PlacementTarget};
use crate::backend::selected::{Payload, Representation};

pub(super) fn storage(
    representation: Representation,
    purpose: StoragePurpose,
    lifetime: StorageLifetime,
) -> Storage {
    let bytes = usize::from(representation.bits()).div_ceil(8);
    Storage {
        representation,
        bytes,
        alignment: bytes.next_power_of_two(),
        purpose,
        lifetime,
    }
}
pub(super) fn copy(
    value: TransferValue,
    source: Location,
    destination: Location,
    representation: Representation,
) -> Transfer {
    Transfer {
        value,
        source,
        destination,
        source_representation: representation,
        destination_representation: representation,
        kind: TransferKind::Copy,
        scratch: vec![],
    }
}
pub(super) fn aliases<P>(
    draft: &PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    a: Location,
    ar: Representation,
    b: Location,
    br: Representation,
) -> bool {
    match (a, b) {
        (Location::Resource(a), Location::Resource(b)) => draft
            .selected()
            .draft()
            .context()
            .resources
            .overlaps(a, b)
            .expect("verified views"),
        (Location::Storage(a), Location::Storage(b)) => a == b,
        (
            Location::Abi {
                signature: a,
                area: aa,
                index: ai,
            },
            Location::Abi {
                signature: b,
                area: ba,
                index: bi,
            },
        ) if aa == ba => {
            let a = target
                .slot_footprint(a, aa, ai, ar)
                .expect("verified target slot");
            let b = target
                .slot_footprint(b, ba, bi, br)
                .expect("verified target slot");
            // Difference comparisons avoid overflowing interval endpoints.
            if a.offset <= b.offset {
                b.offset - a.offset < a.bytes
            } else {
                a.offset - b.offset < b.bytes
            }
        }
        _ => false,
    }
}
fn recipe<P>(
    draft: &PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    mut transfer: Transfer,
    protected: &[(Location, Representation)],
    point: TransferPoint,
) -> Result<Transfer, CheckFailure> {
    let resources = &draft.selected().draft().context().resources;
    let legal = |transfer: &Transfer| {
        target.check_transfer(transfer).is_ok_and(|kills| {
            protected.iter().all(|&(location, _)| match location {
                Location::Resource(view) => resources
                    .view_units(view)
                    .expect("verified view")
                    .iter()
                    .all(|unit| !kills.contains(unit)),
                _ => true,
            })
        })
    };
    if legal(&transfer) {
        return Ok(transfer);
    }
    let preserved = target.preserved_views();
    for (view, resource) in resources.views() {
        let bank = resources
            .banks()
            .find(|(id, _)| *id == resource.bank)
            .expect("verified bank")
            .1;
        if resources
            .require_view(view, resource.bits, bank, true)
            .is_err()
            || preserved
                .iter()
                .any(|&original| resources.overlaps(view, original).expect("target views"))
            || protected.iter().any(|&(location, rep)| {
                aliases(
                    draft,
                    target,
                    Location::Resource(view),
                    transfer.source_representation,
                    location,
                    rep,
                )
            })
        {
            continue;
        }
        transfer.scratch = vec![view];
        if legal(&transfer) {
            return Ok(transfer);
        }
    }
    Err(CheckFailure::new(
        CheckLocation::Transfer { point, index: 0 },
        CheckReason::Scratch,
    ))
}
pub(super) fn resolve<P: Payload>(
    draft: &mut PlacementDraft<'_, '_, P>,
    target: &impl PlacementTarget,
    point: TransferPoint,
    mut pending: Vec<Transfer>,
) -> Result<(), CheckFailure> {
    // IDs are selected coordinates, never construction or hash iteration order.
    pending.sort_by_key(|t| (t.destination, t.source, t.value));
    for (i, t) in pending.iter().enumerate() {
        if pending[..i].iter().any(|other| {
            aliases(
                draft,
                target,
                t.destination,
                t.destination_representation,
                other.destination,
                other.destination_representation,
            )
        }) {
            return Err(CheckFailure::new(
                CheckLocation::Transfer { point, index: i },
                CheckReason::Overlap,
            ));
        }
    }
    let outputs: Vec<_> = pending
        .iter()
        .map(|t| (t.destination, t.destination_representation))
        .collect();
    pending.retain(|t| {
        t.source != t.destination || t.source_representation != t.destination_representation
    });
    let mut broken = vec![false; pending.len()];
    while !pending.is_empty() {
        let ready = pending.iter().enumerate().position(|(i, t)| {
            pending.iter().enumerate().all(|(j, other)| {
                i == j
                    || !aliases(
                        draft,
                        target,
                        t.destination,
                        t.destination_representation,
                        other.source,
                        other.source_representation,
                    )
            })
        });
        let mut protected = outputs.clone();
        protected.extend(pending.iter().map(|t| (t.source, t.source_representation)));
        if let Some(i) = ready {
            let transfer = recipe(draft, target, pending.remove(i), &protected, point)?;
            broken.remove(i);
            draft.transfer(point, transfer);
        } else {
            // Each request is captured at most once. Even with partial resource
            // aliases, capturing every original source removes all dependencies.
            let i = broken
                .iter()
                .position(|b| !b)
                .expect("fresh scratch cannot be a cycle destination");
            let t = &pending[i];
            let temporary = Location::Storage(draft.storage(storage(
                t.source_representation,
                StoragePurpose::TransferScratch,
                StorageLifetime::Transfer(point),
            )));
            let capture = recipe(
                draft,
                target,
                copy(t.value, t.source, temporary, t.source_representation),
                &protected,
                point,
            )?;
            draft.transfer(point, capture);
            pending[i].source = temporary;
            broken[i] = true;
        }
    }
    Ok(())
}

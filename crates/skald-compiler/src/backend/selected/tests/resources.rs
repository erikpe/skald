use super::*;

#[test]
fn resource_overlap_reservation_and_partial_preservation_are_unit_based() {
    let (mut r, full, low) = catalog();
    let integer = r.bank(BankKind::Integer);
    let float = r.bank(BankKind::Float);
    let high = r.unit().unwrap();
    let narrow = r.view(integer, 8, &[low], false).unwrap();
    let cross = r.view(float, 128, &[low, high], false).unwrap();
    let independent = r.view(integer, 64, &[high], false).unwrap();
    assert_eq!(r.view_units(full).unwrap(), &[low]);
    assert!(r.overlaps(narrow, full).unwrap());
    assert!(r.overlaps(full, cross).unwrap());
    assert!(!r.overlaps(full, independent).unwrap());
    assert!(r.preserved(full, &[low]).unwrap());
    assert!(!r.preserved(cross, &[low]).unwrap());
    assert_eq!(r.view(integer, 0, &[low], false), Err(ResourceError::Width));
    assert_eq!(r.view(integer, 8, &[], false), Err(ResourceError::Empty));
    assert_eq!(
        r.view(integer, 8, &[low, low], false),
        Err(ResourceError::Duplicate)
    );
    r.require_unit(low).unwrap();
    r.require_view(full, 64, BankKind::Integer, true).unwrap();
    assert_eq!(
        r.require_view(full, 32, BankKind::Integer, false),
        Err(ResourceError::Width)
    );
    r.view(integer, 8, &[low], true).unwrap();
    assert_eq!(
        r.require_view(full, 64, BankKind::Integer, true),
        Err(ResourceError::Reserved)
    );
    r.require_view(full, 64, BankKind::Integer, false).unwrap();
    assert!(Representation::new(RepresentationKind::Bits, 0).is_none());
    assert_eq!(repr().bank(), BankKind::Integer);
    assert_eq!(
        Representation::new(RepresentationKind::Float, 64)
            .unwrap()
            .bank(),
        BankKind::Float
    );
}

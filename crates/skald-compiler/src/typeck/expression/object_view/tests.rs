use super::ObjectViewSourceAdmission;

#[test]
fn source_admission_distinguishes_existing_places_from_produced_objects() {
    assert!(
        !ObjectViewSourceAdmission::ExistingObjectPlace.accepts_produced_inline(),
        "place-only consumers must reject produced inline objects"
    );
    assert!(
        ObjectViewSourceAdmission::ExistingOrProducedObject.accepts_produced_inline(),
        "produced-source consumers must opt in explicitly"
    );
}

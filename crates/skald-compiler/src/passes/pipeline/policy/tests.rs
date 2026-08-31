use super::{
    available_mir_passes,
    descriptor::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    error::{MirPassRegistryError, MirPassScheduleError},
    identity::MirPassIdentity,
    profile::MirOptimizationProfile,
    registry::MirPassRegistry,
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
    schedule::{resolve_exact, resolve_identities},
};
use crate::passes::pipeline::execution::{MirPassCapability, MirPassFailure, MirPassOutcome};
use crate::passes::pipeline::optimizations::{
    dead_pure_definition_elimination, whole_world_reachability,
};

const ALPHA: MirPassIdentity = MirPassIdentity::new(1);
const BETA: MirPassIdentity = MirPassIdentity::new(2);
const GAMMA: MirPassIdentity = MirPassIdentity::new(3);

const fn registration(
    descriptor_identity: MirPassIdentity,
    implementation_identity: MirPassIdentity,
    name: &'static str,
    description: &'static str,
) -> MirPassRegistration {
    MirPassRegistration::new(
        MirPassDescriptor::new(descriptor_identity, name, description),
        MirPassImplementation::new(implementation_identity, metadata_only_pass),
    )
}

fn metadata_only_pass(capability: MirPassCapability) -> Result<MirPassOutcome, MirPassFailure> {
    Ok(capability.unchanged())
}

static VALID_REGISTRATIONS: [MirPassRegistration; 3] = [
    registration(BETA, BETA, "beta-pass", "Runs beta."),
    registration(ALPHA, ALPHA, "alpha-pass", "Runs alpha."),
    registration(GAMMA, GAMMA, "gamma-2-pass", "Runs gamma."),
];

fn valid_registry() -> MirPassRegistry {
    MirPassRegistry::new(&VALID_REGISTRATIONS)
}

#[test]
fn production_profiles_select_the_supported_default_order() {
    assert_eq!(
        MirOptimizationProfile::default(),
        MirOptimizationProfile::Default
    );

    let none = resolve_mir_pass_schedule(MirOptimizationProfile::None, std::iter::empty()).unwrap();
    assert!(none.is_empty());

    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    assert_eq!(default.len(), 2);
    assert_eq!(default.as_slice()[0].position(), 0);
    assert_eq!(default.as_slice()[0].occurrence(), 0);
    assert_eq!(
        default.as_slice()[0].identity(),
        dead_pure_definition_elimination::IDENTITY
    );
    assert_eq!(
        default.as_slice()[0].name(),
        "dead-pure-definition-elimination"
    );
    assert_eq!(default.as_slice()[1].position(), 1);
    assert_eq!(default.as_slice()[1].occurrence(), 0);
    assert_eq!(
        default.as_slice()[1].identity(),
        whole_world_reachability::IDENTITY
    );
    assert_eq!(default.as_slice()[1].name(), "whole-world-reachability");

    let reachability_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["whole-world-reachability"],
    )
    .unwrap();
    assert_eq!(reachability_disabled.len(), 1);
    assert_eq!(
        reachability_disabled.as_slice()[0].identity(),
        dead_pure_definition_elimination::IDENTITY
    );

    let all_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        [
            "dead-pure-definition-elimination",
            "whole-world-reachability",
        ],
    )
    .unwrap();
    assert_eq!(all_disabled, none);

    assert!(resolve_exact_mir_pass_schedule(&[]).unwrap().is_empty());
    let exact =
        resolve_exact_mir_pass_schedule(&[dead_pure_definition_elimination::IDENTITY]).unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(
        exact.as_slice()[0].name(),
        "dead-pure-definition-elimination"
    );
}

#[test]
fn available_passes_come_from_the_validated_registry_in_stable_name_order() {
    let passes = available_mir_passes();
    assert_eq!(passes.len(), 2);
    assert_eq!(
        passes[0].identity(),
        dead_pure_definition_elimination::IDENTITY
    );
    assert_eq!(passes[0].name(), "dead-pure-definition-elimination");
    assert_eq!(
        passes[0].description(),
        "Removes unused non-failing scalar MIR definitions."
    );
    assert_eq!(passes[1].identity(), whole_world_reachability::IDENTITY);
    assert_eq!(passes[1].name(), "whole-world-reachability");
    assert_eq!(
        passes[1].description(),
        "Removes unreachable executable MIR definitions."
    );

    assert_eq!(
        valid_registry()
            .descriptors()
            .into_iter()
            .map(MirPassDescriptor::name)
            .collect::<Vec<_>>(),
        vec!["alpha-pass", "beta-pass", "gamma-2-pass"]
    );
}

#[test]
fn production_exact_schedules_can_order_and_repeat_reachability_without_changing_default() {
    let dead = dead_pure_definition_elimination::IDENTITY;
    let reachability = whole_world_reachability::IDENTITY;
    let schedule =
        resolve_exact_mir_pass_schedule(&[reachability, dead, reachability, dead, reachability])
            .unwrap();

    assert_eq!(
        schedule
            .iter()
            .map(|occurrence| (
                occurrence.position(),
                occurrence.name(),
                occurrence.occurrence()
            ))
            .collect::<Vec<_>>(),
        vec![
            (0, "whole-world-reachability", 0),
            (1, "dead-pure-definition-elimination", 0),
            (2, "whole-world-reachability", 1),
            (3, "dead-pure-definition-elimination", 1),
            (4, "whole-world-reachability", 2),
        ]
    );
}

#[test]
fn registry_rejects_duplicate_identity_and_name() {
    static REGISTRATIONS: [MirPassRegistration; 2] = [
        registration(ALPHA, ALPHA, "same-pass", "First."),
        registration(ALPHA, ALPHA, "same-pass", "Second."),
    ];

    let errors = MirPassRegistry::new(&REGISTRATIONS).validate().unwrap_err();
    assert_eq!(
        errors.as_slice(),
        &[
            MirPassRegistryError::DuplicateIdentity { identity: ALPHA },
            MirPassRegistryError::DuplicateName { name: "same-pass" },
        ]
    );
}

#[test]
fn registry_rejects_invalid_names_empty_descriptions_and_mismatched_implementations() {
    static REGISTRATIONS: [MirPassRegistration; 9] = [
        registration(ALPHA, ALPHA, "", "Valid."),
        registration(BETA, BETA, "Upper-pass", "Valid."),
        registration(GAMMA, GAMMA, "under_score", "Valid."),
        registration(
            MirPassIdentity::new(4),
            MirPassIdentity::new(4),
            "-leading",
            "Valid.",
        ),
        registration(
            MirPassIdentity::new(5),
            MirPassIdentity::new(5),
            "trailing-",
            "Valid.",
        ),
        registration(
            MirPassIdentity::new(6),
            MirPassIdentity::new(6),
            "double--dash",
            "Valid.",
        ),
        registration(
            MirPassIdentity::new(7),
            MirPassIdentity::new(7),
            "1starts",
            "Valid.",
        ),
        registration(
            MirPassIdentity::new(8),
            MirPassIdentity::new(8),
            "empty-description",
            "  ",
        ),
        registration(
            MirPassIdentity::new(9),
            MirPassIdentity::new(10),
            "mismatch",
            "Valid.",
        ),
    ];

    let errors = MirPassRegistry::new(&REGISTRATIONS).validate().unwrap_err();
    assert_eq!(errors.as_slice().len(), 9);
    assert!(matches!(
        errors.as_slice()[7],
        MirPassRegistryError::EmptyDescription { .. }
    ));
    assert_eq!(
        errors.as_slice()[8],
        MirPassRegistryError::ImplementationIdentityMismatch {
            descriptor: MirPassIdentity::new(9),
            implementation: MirPassIdentity::new(10),
        }
    );
}

#[test]
fn exact_schedule_preserves_order_and_numbers_repeated_occurrences() {
    let schedule = resolve_exact(valid_registry(), &[ALPHA, BETA, ALPHA, ALPHA]).unwrap();
    let occurrences = schedule.as_slice();

    assert_eq!(
        occurrences
            .iter()
            .map(|occurrence| (
                occurrence.position(),
                occurrence.identity(),
                occurrence.occurrence()
            ))
            .collect::<Vec<_>>(),
        vec![(0, ALPHA, 0), (1, BETA, 0), (2, ALPHA, 1), (3, ALPHA, 2)]
    );
}

#[test]
fn exclusions_remove_every_occurrence_and_duplicates_are_idempotent() {
    let schedule = resolve_identities(
        valid_registry(),
        &[ALPHA, BETA, ALPHA, GAMMA],
        ["alpha-pass", "alpha-pass"],
    )
    .unwrap();

    assert_eq!(
        schedule
            .iter()
            .map(|occurrence| (
                occurrence.position(),
                occurrence.identity(),
                occurrence.occurrence()
            ))
            .collect::<Vec<_>>(),
        vec![(0, BETA, 0), (1, GAMMA, 0)]
    );
}

#[test]
fn unknown_exclusions_and_known_names_are_lexically_sorted_and_deduplicated() {
    let error = resolve_identities(
        valid_registry(),
        &[ALPHA],
        ["zeta-pass", "missing-pass", "zeta-pass"],
    )
    .unwrap_err();

    assert_eq!(
        error,
        MirPassScheduleError::UnknownNames {
            names: vec!["missing-pass".to_owned(), "zeta-pass".to_owned()],
            known_names: vec!["alpha-pass", "beta-pass", "gamma-2-pass"],
        }
    );
}

#[test]
fn schedule_order_is_independent_of_registry_order() {
    static REVERSED_REGISTRATIONS: [MirPassRegistration; 3] = [
        registration(GAMMA, GAMMA, "gamma-2-pass", "Runs gamma."),
        registration(ALPHA, ALPHA, "alpha-pass", "Runs alpha."),
        registration(BETA, BETA, "beta-pass", "Runs beta."),
    ];
    let identities = [BETA, ALPHA, GAMMA, ALPHA];

    let forward = resolve_exact(valid_registry(), &identities).unwrap();
    let reversed =
        resolve_exact(MirPassRegistry::new(&REVERSED_REGISTRATIONS), &identities).unwrap();

    assert_eq!(forward, reversed);
}

#[test]
fn equivalent_exclusion_inputs_resolve_deterministically() {
    let identities = [ALPHA, BETA, ALPHA, GAMMA];
    let first = resolve_identities(
        valid_registry(),
        &identities,
        ["gamma-2-pass", "alpha-pass"],
    )
    .unwrap();
    let second = resolve_identities(
        valid_registry(),
        &identities,
        ["alpha-pass", "gamma-2-pass", "alpha-pass"],
    )
    .unwrap();

    assert_eq!(first, second);
}

#[test]
fn exact_schedule_rejects_an_unregistered_identity() {
    assert_eq!(
        resolve_exact(valid_registry(), &[MirPassIdentity::new(99)]).unwrap_err(),
        MirPassScheduleError::UnknownIdentity {
            identity: MirPassIdentity::new(99),
        }
    );
}

#[test]
fn invalid_registry_is_reported_before_schedule_selection() {
    static REGISTRATIONS: [MirPassRegistration; 1] =
        [registration(ALPHA, BETA, "alpha-pass", "Runs alpha.")];

    assert!(matches!(
        resolve_exact(MirPassRegistry::new(&REGISTRATIONS), &[ALPHA]),
        Err(MirPassScheduleError::InvalidRegistry(_))
    ));
}

use super::{
    available_mir_passes,
    descriptor::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
    error::{MirPassRegistryError, MirPassScheduleError},
    identity::MirPassIdentity,
    profile::MirOptimizationProfile,
    registry::MirPassRegistry,
    resolve_exact_mir_pass_schedule, resolve_mir_pass_schedule,
    schedule::{resolve_exact, resolve_identities},
    MirPassStage,
};
use crate::passes::pipeline::execution::{
    MirFinalPassCapability, MirFinalPassOutcome, MirPassFailure, MirProofPassCapability,
    MirProofPassOutcome,
};
use crate::passes::pipeline::optimizations::{
    checked_integer_folding, conservative_cfg_cleanup, dead_pure_definition_elimination,
    post_proof_unreachable_block_elimination, primitive_algebraic_simplification,
    primitive_constant_folding, whole_world_reachability,
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
        MirPassDescriptor::new(
            descriptor_identity,
            MirPassStage::ProofRich,
            name,
            description,
        ),
        MirPassImplementation::proof_rich(implementation_identity, metadata_only_pass),
    )
}

fn metadata_only_pass(
    capability: MirProofPassCapability,
) -> Result<MirProofPassOutcome, MirPassFailure> {
    Ok(capability.unchanged())
}

const fn final_registration(identity: MirPassIdentity, name: &'static str) -> MirPassRegistration {
    MirPassRegistration::new(
        MirPassDescriptor::new(identity, MirPassStage::Final, name, "Runs final metadata."),
        MirPassImplementation::final_stage(identity, final_metadata_only_pass),
    )
}

fn final_metadata_only_pass(
    capability: MirFinalPassCapability,
) -> Result<MirFinalPassOutcome, MirPassFailure> {
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
    assert_eq!(none.normalization_position(), 0);

    let default =
        resolve_mir_pass_schedule(MirOptimizationProfile::Default, std::iter::empty()).unwrap();
    assert_eq!(
        default
            .iter()
            .map(|occurrence| (
                occurrence.position(),
                occurrence.identity(),
                occurrence.name(),
                occurrence.occurrence(),
            ))
            .collect::<Vec<_>>(),
        [
            (
                0,
                dead_pure_definition_elimination::IDENTITY,
                "dead-pure-definition-elimination",
                0,
            ),
            (
                1,
                primitive_constant_folding::IDENTITY,
                "primitive-constant-folding",
                0,
            ),
            (
                2,
                primitive_algebraic_simplification::IDENTITY,
                "primitive-algebraic-simplification",
                0,
            ),
            (
                3,
                primitive_constant_folding::IDENTITY,
                "primitive-constant-folding",
                1,
            ),
            (
                4,
                checked_integer_folding::IDENTITY,
                "checked-integer-constant-folding",
                0,
            ),
            (
                5,
                dead_pure_definition_elimination::IDENTITY,
                "dead-pure-definition-elimination",
                1,
            ),
            (
                6,
                conservative_cfg_cleanup::IDENTITY,
                "conservative-cfg-cleanup",
                0,
            ),
            (
                7,
                dead_pure_definition_elimination::IDENTITY,
                "dead-pure-definition-elimination",
                2,
            ),
            (
                8,
                post_proof_unreachable_block_elimination::IDENTITY,
                "post-proof-unreachable-block-elimination",
                0,
            ),
            (
                9,
                whole_world_reachability::IDENTITY,
                "whole-world-reachability",
                0,
            ),
        ]
    );

    let reachability_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["whole-world-reachability"],
    )
    .unwrap();
    assert_eq!(reachability_disabled.len(), 9);
    assert!(reachability_disabled
        .iter()
        .all(|occurrence| occurrence.identity() != whole_world_reachability::IDENTITY));

    let all_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        [
            "checked-integer-constant-folding",
            "conservative-cfg-cleanup",
            "dead-pure-definition-elimination",
            "post-proof-unreachable-block-elimination",
            "primitive-algebraic-simplification",
            "primitive-constant-folding",
            "whole-world-reachability",
        ],
    )
    .unwrap();
    assert_eq!(all_disabled, none);

    let checked_integer_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["checked-integer-constant-folding"],
    )
    .unwrap();
    assert_eq!(checked_integer_disabled.len(), 9);
    assert!(checked_integer_disabled
        .iter()
        .all(|occurrence| occurrence.identity() != checked_integer_folding::IDENTITY));

    assert!(resolve_exact_mir_pass_schedule(&[]).unwrap().is_empty());
    let exact =
        resolve_exact_mir_pass_schedule(&[dead_pure_definition_elimination::IDENTITY]).unwrap();
    assert_eq!(exact.len(), 1);
    assert_eq!(
        exact.as_slice()[0].name(),
        "dead-pure-definition-elimination"
    );
    let checked_exact =
        resolve_exact_mir_pass_schedule(&[checked_integer_folding::IDENTITY]).unwrap();
    assert_eq!(checked_exact.len(), 1);
    assert_eq!(
        checked_exact.as_slice()[0].name(),
        "checked-integer-constant-folding"
    );
    let final_only =
        resolve_exact_mir_pass_schedule(&[whole_world_reachability::IDENTITY]).unwrap();
    assert_eq!(final_only.normalization_position(), 0);
}

#[test]
fn production_exclusions_remove_every_repeated_occurrence_and_compose() {
    let constant_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["primitive-constant-folding"],
    )
    .unwrap();
    let duplicate_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        ["primitive-constant-folding", "primitive-constant-folding"],
    )
    .unwrap();
    assert_eq!(constant_disabled, duplicate_disabled);
    assert!(constant_disabled
        .iter()
        .all(|occurrence| occurrence.identity() != primitive_constant_folding::IDENTITY));

    let scalar_cleanup_disabled = resolve_mir_pass_schedule(
        MirOptimizationProfile::Default,
        [
            "dead-pure-definition-elimination",
            "primitive-constant-folding",
        ],
    )
    .unwrap();
    assert_eq!(
        scalar_cleanup_disabled
            .iter()
            .map(|occurrence| occurrence.name())
            .collect::<Vec<_>>(),
        [
            "primitive-algebraic-simplification",
            "checked-integer-constant-folding",
            "conservative-cfg-cleanup",
            "post-proof-unreachable-block-elimination",
            "whole-world-reachability",
        ]
    );
}

#[test]
fn available_passes_come_from_the_validated_registry_in_stable_name_order() {
    let passes = available_mir_passes();
    assert_eq!(passes.len(), 7);
    assert_eq!(
        passes
            .iter()
            .map(|descriptor| (descriptor.name(), descriptor.stage()))
            .collect::<Vec<_>>(),
        [
            ("checked-integer-constant-folding", MirPassStage::ProofRich),
            ("conservative-cfg-cleanup", MirPassStage::ProofRich),
            ("dead-pure-definition-elimination", MirPassStage::ProofRich),
            (
                "post-proof-unreachable-block-elimination",
                MirPassStage::Final
            ),
            (
                "primitive-algebraic-simplification",
                MirPassStage::ProofRich
            ),
            ("primitive-constant-folding", MirPassStage::ProofRich),
            ("whole-world-reachability", MirPassStage::Final),
        ]
    );
    assert_eq!(passes[0].identity(), checked_integer_folding::IDENTITY);
    assert_eq!(passes[0].stage(), MirPassStage::ProofRich);
    assert_eq!(passes[0].name(), "checked-integer-constant-folding");
    assert_eq!(
        passes[0].description(),
        "Folds exact successful checked-integer constant protocols."
    );
    assert_eq!(passes[1].identity(), conservative_cfg_cleanup::IDENTITY);
    assert_eq!(passes[1].stage(), MirPassStage::ProofRich);
    assert_eq!(passes[1].name(), "conservative-cfg-cleanup");
    assert_eq!(
        passes[1].description(),
        "Folds ordinary branches and removes unprotected unreachable MIR blocks."
    );
    assert_eq!(
        passes[2].identity(),
        dead_pure_definition_elimination::IDENTITY
    );
    assert_eq!(passes[2].name(), "dead-pure-definition-elimination");
    assert_eq!(
        passes[2].description(),
        "Removes unused non-failing scalar MIR definitions."
    );
    assert_eq!(
        passes[3].identity(),
        post_proof_unreachable_block_elimination::IDENTITY
    );
    assert_eq!(passes[3].stage(), MirPassStage::Final);
    assert_eq!(passes[3].name(), "post-proof-unreachable-block-elimination");
    assert_eq!(
        passes[3].description(),
        "Removes normalized MIR blocks unreachable from executable and permanent roots."
    );
    assert_eq!(
        passes[4].identity(),
        primitive_algebraic_simplification::IDENTITY
    );
    assert_eq!(passes[4].name(), "primitive-algebraic-simplification");
    assert_eq!(
        passes[4].description(),
        "Simplifies exact primitive MIR algebraic identities."
    );
    assert_eq!(passes[5].identity(), primitive_constant_folding::IDENTITY);
    assert_eq!(passes[5].name(), "primitive-constant-folding");
    assert_eq!(
        passes[5].description(),
        "Folds exact block-local primitive MIR constants."
    );
    assert_eq!(passes[6].identity(), whole_world_reachability::IDENTITY);
    assert_eq!(passes[6].stage(), MirPassStage::Final);
    assert_eq!(passes[6].name(), "whole-world-reachability");
    assert_eq!(
        passes[6].description(),
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
fn production_exact_schedules_preserve_regions_and_repeat_within_a_stage() {
    let dead = dead_pure_definition_elimination::IDENTITY;
    let reachability = whole_world_reachability::IDENTITY;
    let schedule =
        resolve_exact_mir_pass_schedule(&[dead, dead, reachability, reachability]).unwrap();

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
            (0, "dead-pure-definition-elimination", 0),
            (1, "dead-pure-definition-elimination", 1),
            (2, "whole-world-reachability", 0),
            (3, "whole-world-reachability", 1),
        ]
    );
    assert_eq!(schedule.proof_rich().count(), 2);
    assert_eq!(schedule.final_stage().count(), 2);
    assert_eq!(schedule.normalization_position(), 2);

    assert_eq!(
        resolve_exact_mir_pass_schedule(&[reachability, dead]).unwrap_err(),
        MirPassScheduleError::WrongStageOrder {
            proof_rich: dead,
            position: 1,
        }
    );
}

#[test]
fn normalization_is_never_a_selectable_or_registered_pass() {
    assert_eq!(
        resolve_mir_pass_schedule(
            MirOptimizationProfile::Default,
            ["proof-provenance-normalization"],
        )
        .unwrap_err(),
        MirPassScheduleError::MandatoryNormalizationSelection
    );

    static REGISTRATIONS: [MirPassRegistration; 1] = [registration(
        ALPHA,
        ALPHA,
        "proof-provenance-normalization",
        "Must remain implicit.",
    )];
    assert_eq!(
        MirPassRegistry::new(&REGISTRATIONS)
            .validate()
            .unwrap_err()
            .as_slice(),
        &[MirPassRegistryError::ReservedNormalizationName]
    );
}

#[test]
fn registry_rejects_descriptor_and_callback_stage_mismatch() {
    static REGISTRATIONS: [MirPassRegistration; 1] = [MirPassRegistration::new(
        MirPassDescriptor::new(ALPHA, MirPassStage::ProofRich, "alpha-pass", "Runs alpha."),
        MirPassImplementation::final_stage(ALPHA, final_metadata_only_pass),
    )];
    assert_eq!(
        MirPassRegistry::new(&REGISTRATIONS)
            .validate()
            .unwrap_err()
            .as_slice(),
        &[MirPassRegistryError::ImplementationStageMismatch {
            identity: ALPHA,
            descriptor: MirPassStage::ProofRich,
            implementation: MirPassStage::Final,
        }]
    );

    static MIXED: [MirPassRegistration; 2] = [
        registration(ALPHA, ALPHA, "alpha-pass", "Runs alpha."),
        final_registration(BETA, "beta-pass"),
    ];
    let schedule = resolve_exact(MirPassRegistry::new(&MIXED), &[ALPHA, BETA]).unwrap();
    assert_eq!(schedule.proof_rich().count(), 1);
    assert_eq!(schedule.final_stage().count(), 1);
    assert_eq!(schedule.normalization_position(), 1);
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

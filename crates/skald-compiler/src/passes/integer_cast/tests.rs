use crate::mir::{MirIntegerType, MirPrimitiveCast, MirPrimitiveCastKind, MirPrimitiveType};

use super::*;

const INTEGER_TYPES: [MirIntegerType; 3] =
    [MirIntegerType::I64, MirIntegerType::U64, MirIntegerType::U8];

const BOUNDARY_BITS: [u64; 12] = [
    0,
    1,
    0x7f,
    0x80,
    0xff,
    0x100,
    0x101,
    0x7fff_ffff_ffff_ffff,
    0x8000_0000_0000_0000,
    0xffff_ffff_ffff_ff00,
    0xffff_ffff_ffff_ff01,
    u64::MAX,
];

fn cast(source: MirIntegerType, target: MirIntegerType) -> MirPrimitiveCast {
    MirPrimitiveCast::new(source.into(), target.into())
}

fn append(transform: IntegerCastTransform, target: MirIntegerType) -> IntegerCastTransform {
    transform.then(cast(transform.target, target)).unwrap()
}

fn recipe_transform(source: MirIntegerType, recipe: IntegerCastRecipe) -> IntegerCastTransform {
    let mut transform = IntegerCastTransform::identity(source);
    match recipe {
        IntegerCastRecipe::Identity => {}
        IntegerCastRecipe::Direct(operation) => {
            transform = transform.then(operation).unwrap();
        }
        IntegerCastRecipe::NarrowThenWiden { narrow, widen } => {
            transform = transform.then(narrow).unwrap();
            transform = transform.then(widen).unwrap();
        }
    }
    transform
}

fn cast_bits(bits: u64, source: MirIntegerType, target: MirIntegerType) -> u64 {
    match target {
        MirIntegerType::U8 => bits & u64::from(u8::MAX),
        MirIntegerType::I64 | MirIntegerType::U64 if source == MirIntegerType::U8 => {
            bits & u64::from(u8::MAX)
        }
        MirIntegerType::I64 | MirIntegerType::U64 => bits,
    }
}

fn evaluate_recipe(
    bits: u64,
    source: MirIntegerType,
    recipe: IntegerCastRecipe,
) -> (u64, MirIntegerType) {
    match recipe {
        IntegerCastRecipe::Identity => (bits, source),
        IntegerCastRecipe::Direct(operation) => {
            let target = operation.target.integer_type().unwrap();
            (cast_bits(bits, source, target), target)
        }
        IntegerCastRecipe::NarrowThenWiden { narrow, widen } => {
            let narrow_target = narrow.target.integer_type().unwrap();
            let narrowed = cast_bits(bits, source, narrow_target);
            let target = widen.target.integer_type().unwrap();
            (cast_bits(narrowed, narrow_target, target), target)
        }
    }
}

fn enumerate_transforms(maximum_depth: usize) -> Vec<IntegerCastTransform> {
    let mut frontier = INTEGER_TYPES
        .into_iter()
        .map(IntegerCastTransform::identity)
        .collect::<Vec<_>>();
    let mut all = frontier.clone();
    for _ in 0..maximum_depth {
        let mut next = Vec::new();
        for transform in frontier {
            for target in INTEGER_TYPES {
                let composed = append(transform, target);
                if !all.contains(&composed) {
                    all.push(composed);
                    next.push(composed);
                }
            }
        }
        frontier = next;
        if frontier.is_empty() {
            break;
        }
    }
    all
}

#[test]
fn every_direct_integer_cast_pair_has_the_expected_canonical_recipe() {
    for source in INTEGER_TYPES {
        for target in INTEGER_TYPES {
            let operation = cast(source, target);
            let transform = IntegerCastTransform::from_operation(operation).unwrap();
            let expected = if source == target {
                IntegerCastRecipe::Identity
            } else {
                IntegerCastRecipe::Direct(operation)
            };
            assert_eq!(
                transform.canonical_recipe(),
                expected,
                "{source:?} -> {target:?}"
            );
        }
    }
}

#[test]
fn composition_is_closed_and_canonical_recipes_are_idempotent() {
    let transforms = enumerate_transforms(8);
    // `u8` has three all-bit target states. Each 64-bit root has two
    // all-bit 64-bit targets plus three low-byte target states.
    assert_eq!(transforms.len(), 13);

    for transform in transforms {
        for target in INTEGER_TYPES {
            let composed = append(transform, target);
            let recipe = composed.canonical_recipe();
            let canonical = recipe_transform(composed.source, recipe);
            assert_eq!(canonical, composed);
            assert_eq!(canonical.canonical_recipe(), recipe);
            assert!(recipe.length() <= 2);
        }
    }
}

#[test]
fn narrowed_and_widened_64_bit_transforms_require_two_casts() {
    for source in [MirIntegerType::I64, MirIntegerType::U64] {
        for target in [MirIntegerType::I64, MirIntegerType::U64] {
            let narrowed = append(IntegerCastTransform::identity(source), MirIntegerType::U8);
            let transform = append(narrowed, target);
            let recipe = transform.canonical_recipe();
            assert_eq!(recipe.length(), 2);

            let zero_cast = IntegerCastTransform::identity(source);
            assert_ne!(zero_cast, transform);
            for direct_target in INTEGER_TYPES {
                let one_cast = append(IntegerCastTransform::identity(source), direct_target);
                assert_ne!(one_cast, transform);
            }
        }
    }
}

#[test]
fn generated_chains_match_their_recipes_for_complete_u8_and_boundary_64_bit_inputs() {
    fn visit(
        transform: IntegerCastTransform,
        depth: usize,
        maximum_depth: usize,
        original_bits: u64,
        current_bits: u64,
    ) {
        let recipe = transform.canonical_recipe();
        assert_eq!(
            evaluate_recipe(original_bits, transform.source, recipe),
            (current_bits, transform.target)
        );
        if depth == maximum_depth {
            return;
        }
        for target in INTEGER_TYPES {
            visit(
                append(transform, target),
                depth + 1,
                maximum_depth,
                original_bits,
                cast_bits(current_bits, transform.target, target),
            );
        }
    }

    for bits in 0..=u64::from(u8::MAX) {
        visit(
            IntegerCastTransform::identity(MirIntegerType::U8),
            0,
            6,
            bits,
            bits,
        );
    }
    for source in [MirIntegerType::I64, MirIntegerType::U64] {
        for bits in BOUNDARY_BITS {
            visit(IntegerCastTransform::identity(source), 0, 8, bits, bits);
        }
    }
}

#[test]
fn non_integer_disconnected_and_inconsistent_operations_are_rejected() {
    let integer = IntegerCastTransform::identity(MirIntegerType::I64);
    assert_eq!(
        integer.then(MirPrimitiveCast::new(
            MirPrimitiveType::I64,
            MirPrimitiveType::Bool,
        )),
        None
    );
    assert_eq!(
        IntegerCastTransform::from_operation(MirPrimitiveCast::bit_reinterpretation(
            MirPrimitiveType::U64,
            MirPrimitiveType::F64,
        )),
        None
    );
    assert_eq!(
        integer.then(cast(MirIntegerType::U64, MirIntegerType::U8)),
        None
    );

    let mut malformed = cast(MirIntegerType::I64, MirIntegerType::U64);
    malformed.set_kind_for_test(MirPrimitiveCastKind::ToBool);
    assert_eq!(integer.then(malformed), None);
}

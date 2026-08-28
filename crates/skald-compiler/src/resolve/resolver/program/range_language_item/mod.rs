//! Canonical `std::range` declaration-bundle validation.

mod validation;

pub(super) use validation::{
    validate_range_language_item, validate_successor_language_item, RangeLanguageItemEvidence,
};

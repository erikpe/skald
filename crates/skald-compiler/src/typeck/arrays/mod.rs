//! Array type validation, lifecycle planning, and construction checking.

mod alias;
mod assignment;
mod capabilities;
mod construction;
mod place;
mod validation;

pub(super) use capabilities::lower_array_types;
pub use construction::{
    ARRAY_CAPABILITY_UNAVAILABLE, ARRAY_ELEMENT_LIST_UNAVAILABLE, ARRAY_LENGTH_OUT_OF_RANGE,
};
pub use place::ARRAY_PROJECTION_REQUIRES_ARRAY;
pub(super) use validation::resolved_type_contains_array;
pub(super) use validation::validate_array_types;
pub use validation::INVALID_ARRAY_ELEMENT;

//! Type checking and construction of typed HIR.

mod checker;

pub use checker::{
    type_check, TypeCheckOutput, INTEGER_LITERAL_OUT_OF_RANGE, INVALID_ENTRY_POINT,
    MISSING_ENTRY_POINT, MISSING_RETURN, TYPE_MISMATCH, WRONG_ARGUMENT_COUNT,
};

#[cfg(test)]
mod tests;

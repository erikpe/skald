//! Hand-written placements exercise acceptance without a placement producer.
use super::{super::*, check_with_round_oracle, fixtures::*};
use crate::backend::selected::*;

fn reject<P>(result: Result<CheckedPlacement<'_, '_, P>, CheckFailure>, reason: CheckReason) {
    match result {
        Ok(_) => panic!("corrupted placement accepted"),
        Err(error) => assert_eq!(error.reason, reason, "{error:?}"),
    }
}

mod calls;
mod control_flow;
mod registers;
mod transfers;

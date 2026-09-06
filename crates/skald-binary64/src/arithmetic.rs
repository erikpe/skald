use rustc_apfloat::{ieee::Double, Float, Round, Status, StatusAnd};

use crate::Binary64;

const ROUNDING: Round = Round::NearestTiesToEven;

impl Binary64 {
    /// Adds two binary64 values using round-to-nearest, ties-to-even.
    ///
    /// IEEE exception conditions are represented by their ordinary binary64
    /// results; they are not failures in the Skald language.
    #[must_use]
    // The facade intentionally uses named operations instead of Rust operator
    // traits so its fixed rounding and status policy remains explicit.
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, other: Self) -> Self {
        evaluate(self, other, |left, right| left.add_r(right, ROUNDING))
    }

    /// Subtracts two binary64 values using round-to-nearest, ties-to-even.
    ///
    /// IEEE exception conditions are represented by their ordinary binary64
    /// results; they are not failures in the Skald language.
    #[must_use]
    pub fn subtract(self, other: Self) -> Self {
        evaluate(self, other, |left, right| left.sub_r(right, ROUNDING))
    }

    /// Multiplies two binary64 values using round-to-nearest, ties-to-even.
    ///
    /// IEEE exception conditions are represented by their ordinary binary64
    /// results; they are not failures in the Skald language.
    #[must_use]
    pub fn multiply(self, other: Self) -> Self {
        evaluate(self, other, |left, right| left.mul_r(right, ROUNDING))
    }

    /// Divides two binary64 values using round-to-nearest, ties-to-even.
    ///
    /// IEEE exception conditions are represented by their ordinary binary64
    /// results; they are not failures in the Skald language.
    #[must_use]
    pub fn divide(self, other: Self) -> Self {
        evaluate(self, other, |left, right| left.div_r(right, ROUNDING))
    }
}

fn evaluate(
    left: Binary64,
    right: Binary64,
    operation: impl FnOnce(Double, Double) -> StatusAnd<Double>,
) -> Binary64 {
    let result = operation(to_apfloat(left), to_apfloat(right));
    consume_arithmetic_status(result.status);

    Binary64::from_bits(
        u64::try_from(result.value.to_bits())
            .expect("rustc_apfloat::ieee::Double produced more than 64 bits"),
    )
}

fn to_apfloat(value: Binary64) -> Double {
    Double::from_bits(u128::from(value.to_bits()))
}

fn consume_arithmetic_status(status: Status) {
    // These are all status combinations produced by binary arithmetic in the
    // pinned APFloat release. Rejecting anything else makes a future dependency
    // change require an explicit semantic review instead of silently discarding
    // new exception state.
    let overflow = Status::OVERFLOW | Status::INEXACT;
    let underflow = Status::UNDERFLOW | Status::INEXACT;
    let recognized = status == Status::OK
        || status == Status::INVALID_OP
        || status == Status::DIV_BY_ZERO
        || status == Status::INEXACT
        || status == overflow
        || status == underflow;

    assert!(
        recognized,
        "rustc_apfloat returned an unexpected binary64 arithmetic status: {status:?}"
    );
}

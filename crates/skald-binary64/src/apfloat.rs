use rustc_apfloat::{ieee::Double, Float};

use crate::Binary64;

pub(super) fn binary64_to_apfloat(value: Binary64) -> Double {
    Double::from_bits(u128::from(value.to_bits()))
}

pub(super) fn apfloat_to_binary64(value: Double) -> Binary64 {
    Binary64::from_bits(
        u64::try_from(value.to_bits())
            .expect("rustc_apfloat::ieee::Double produced more than 64 bits"),
    )
}

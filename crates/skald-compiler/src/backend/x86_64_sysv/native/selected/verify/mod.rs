//! Independent native validation over fields, catalog authority and memory flow.
mod callable;
mod numeric;
mod payload;
use super::super::NativeResources;
use super::{
    model::{FloatCondition, IntegerCondition},
    Instruction,
};
use crate::backend::{
    plan::{Abi, Architecture, Endianness, TargetProfile},
    selected::{SelectedDraft, TargetVerifier},
};
use std::sync::Arc;

pub(in crate::backend) struct Verifier {
    pub(super) profile: TargetProfile,
    pub(super) resources: Arc<NativeResources>,
}
impl Verifier {
    pub(in crate::backend) fn new(profile: TargetProfile) -> Result<Self, &'static str> {
        if profile.architecture != Architecture::X86_64
            || profile.abi != Abi::SysV
            || profile.data_layout.pointer_alignment != 8
            || profile.data_layout.pointer_bytes != 8
            || profile.data_layout.endianness != Endianness::Little
        {
            return Err("unsupported native target profile");
        }
        Ok(Self {
            profile,
            resources: Arc::new(NativeResources::new().map_err(|_| "invalid native resources")?),
        })
    }
}
pub(super) fn integer_condition(
    predicate: crate::primitive_comparison::PrimitiveComparisonPredicate,
    signed: bool,
) -> IntegerCondition {
    use crate::primitive_comparison::PrimitiveComparisonPredicate::*;
    match (predicate, signed) {
        (Equal, _) => IntegerCondition::Equal,
        (NotEqual, _) => IntegerCondition::NotEqual,
        (LessThan, true) => IntegerCondition::Less,
        (LessEqual, true) => IntegerCondition::LessEqual,
        (GreaterThan, true) => IntegerCondition::Greater,
        (GreaterEqual, true) => IntegerCondition::GreaterEqual,
        (LessThan, false) => IntegerCondition::Below,
        (LessEqual, false) => IntegerCondition::BelowEqual,
        (GreaterThan, false) => IntegerCondition::Above,
        (GreaterEqual, false) => IntegerCondition::AboveEqual,
    }
}
pub(super) fn float_condition(
    predicate: crate::primitive_comparison::PrimitiveComparisonPredicate,
) -> FloatCondition {
    use crate::primitive_comparison::PrimitiveComparisonPredicate::*;
    match predicate {
        Equal => FloatCondition::EqualOrdered,
        NotEqual => FloatCondition::NotEqualOrUnordered,
        LessThan => FloatCondition::BelowOrdered,
        LessEqual => FloatCondition::BelowEqualOrdered,
        GreaterThan => FloatCondition::Above,
        GreaterEqual => FloatCondition::AboveEqual,
    }
}
impl TargetVerifier<Instruction> for Verifier {
    fn profile(&self) -> TargetProfile {
        self.profile
    }
    fn verify_payload(&self, node: &Instruction, terminal: bool) -> Result<(), &'static str> {
        self.check_payload(node, terminal)
    }
    fn verify_callable(&self, draft: &SelectedDraft<'_, Instruction>) -> Result<(), &'static str> {
        self.check_callable(draft)
    }
}

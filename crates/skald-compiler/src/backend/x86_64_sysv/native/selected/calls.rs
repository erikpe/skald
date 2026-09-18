//! Simultaneous ABI events. Placement must secure the indirect address in R11
//! before marshalling inputs, and capture result definitions before later cells.
use super::ValueRef;
use crate::backend::{
    effects::{Effect, Effects},
    graph::SelectedObjectId,
    lir::{CallAttribution, CallTarget},
    plan::{service_effects, ArtifactCategory, ArtifactId, SignatureId},
    selected::AbiBinding,
};
#[derive(Clone, Debug)]
pub(in crate::backend) struct NativeCall {
    pub target: CallTarget<ValueRef>,
    pub signature: SignatureId,
    pub arguments: Vec<ValueRef>,
    pub results: Vec<ValueRef>,
    pub inputs: Vec<AbiBinding>,
    pub outputs: Vec<AbiBinding>,
    pub attribution: CallAttribution,
    /// A terminal call includes UD2 if a declared nonreturning callee returns.
    pub never: bool,
}
impl NativeCall {
    pub(super) fn operands(&self) -> Vec<(ValueRef, bool)> {
        let mut operands = self
            .arguments
            .iter()
            .map(|v| (*v, false))
            .collect::<Vec<_>>();
        if let CallTarget::Indirect(target) = self.target {
            operands.push((target, false));
        }
        operands.extend(self.results.iter().map(|v| (*v, true)));
        operands
    }
    pub(super) fn operands_mut(&mut self) -> Vec<(&mut ValueRef, bool)> {
        let mut operands = self
            .arguments
            .iter_mut()
            .map(|v| (v, false))
            .collect::<Vec<_>>();
        if let CallTarget::Indirect(target) = &mut self.target {
            operands.push((target, false));
        }
        operands.extend(self.results.iter_mut().map(|v| (v, true)));
        operands
    }
    pub(super) fn effects(&self) -> Effects<SelectedObjectId> {
        let base = match self.target {
            CallTarget::Direct(ArtifactId::Runtime(service)) => service_effects(service),
            _ => Effects::conservative_call(),
        };
        if self.never {
            Effects::new(base.iter().copied().chain([Effect::HardTrap]))
        } else {
            base
        }
    }
    pub(super) fn artifacts(&self) -> Vec<(ArtifactId, ArtifactCategory)> {
        let mut refs = vec![];
        if let CallTarget::Direct(target) = self.target {
            refs.push((target, target.category()));
        }
        if let CallAttribution::SourceOperation {
            location: Some(location),
            ..
        } = self.attribution
        {
            refs.push((location, location.category()));
        }
        refs
    }
}

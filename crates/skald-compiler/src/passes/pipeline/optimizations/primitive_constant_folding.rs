//! Convergent folding of exact integer and boolean primitive constants.

#[path = "primitive_constant_folding/plan.rs"]
mod plan;

use super::{
    super::{
        execution::{
            MirPassData, MirPassFailure, MirPassMeasurement, MirProofPassCapability,
            MirProofPassOutcome,
        },
        policy::{MirPassDescriptor, MirPassImplementation, MirPassRegistration},
        MirPassIdentity, MirPassStage,
    },
    local_constant::LocalConstantProvenance,
};
use plan::PrimitiveFoldPlan;

pub(in crate::passes::pipeline) const IDENTITY: MirPassIdentity = MirPassIdentity::new(2);
const NAME: &str = "primitive-constant-folding";
const DESCRIPTION: &str = "Folds exact convergently proven primitive MIR constants.";
const FOLDED_UNARY: &str = "folded unary assignments";
const FOLDED_BINARY: &str = "folded binary assignments";
const FOLDED_COMPARISONS: &str = "folded comparison assignments";
const FOLDED_CASTS: &str = "folded cast assignments";
const FOLDS_CROSSING_CARRIERS: &str = "folds crossing certified carriers";
const FOLDS_CROSSING_CHECKED: &str = "folds crossing checked protocols";
const FOLDS_CROSSING_LOGICAL: &str = "folds crossing logical selections";
const MAXIMUM_DEPENDENCY_DEPTH: &str = "maximum folded dependency depth";

pub(in crate::passes::pipeline) const REGISTRATION: MirPassRegistration = MirPassRegistration::new(
    MirPassDescriptor::new(IDENTITY, MirPassStage::ProofRich, NAME, DESCRIPTION),
    MirPassImplementation::proof_rich(IDENTITY, transform),
);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FoldCounts {
    unary: usize,
    binary: usize,
    comparisons: usize,
    casts: usize,
    crossing_carriers: usize,
    crossing_checked: usize,
    crossing_logical: usize,
    maximum_dependency_depth: usize,
}

impl FoldCounts {
    fn record(&mut self, kind: PrimitiveFoldKind, provenance: LocalConstantProvenance) {
        let count = match kind {
            PrimitiveFoldKind::Unary => &mut self.unary,
            PrimitiveFoldKind::Binary => &mut self.binary,
            PrimitiveFoldKind::Comparison => &mut self.comparisons,
            PrimitiveFoldKind::Cast => &mut self.casts,
        };
        *count = count.saturating_add(1);
        self.crossing_carriers = self
            .crossing_carriers
            .saturating_add(usize::from(provenance.crossed_carrier()));
        self.crossing_checked = self
            .crossing_checked
            .saturating_add(usize::from(provenance.crossed_checked()));
        self.crossing_logical = self
            .crossing_logical
            .saturating_add(usize::from(provenance.crossed_logical()));
        self.maximum_dependency_depth = self.maximum_dependency_depth.max(provenance.depth());
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PrimitiveFoldKind {
    Unary,
    Binary,
    Comparison,
    Cast,
}

fn transform(capability: MirProofPassCapability) -> Result<MirProofPassOutcome, MirPassFailure> {
    let plan = PrimitiveFoldPlan::prepare(capability.verified().program())
        .map_err(|error| MirPassFailure::execution(error.to_string()))?;
    if plan.is_empty() {
        return capability.unchanged_with(pass_data(plan.processed_callables(), 0, plan.counts()));
    }

    let changed_callables = plan.changed_callables();
    let rewritten = capability.rewrite(|callable, edit| plan.rewrite_callable(callable, edit))?;

    rewritten.finish(pass_data(0, changed_callables, plan.counts()))
}

fn pass_data(
    processed_callables: usize,
    changed_callables: usize,
    folds: FoldCounts,
) -> MirPassData {
    let data = if changed_callables == 0 {
        MirPassData::processed(processed_callables)
    } else {
        MirPassData::changed(changed_callables)
    };
    data.with_measurement(MirPassMeasurement::count(FOLDED_UNARY, count(folds.unary)))
        .with_measurement(MirPassMeasurement::count(
            FOLDED_BINARY,
            count(folds.binary),
        ))
        .with_measurement(MirPassMeasurement::count(
            FOLDED_COMPARISONS,
            count(folds.comparisons),
        ))
        .with_measurement(MirPassMeasurement::count(FOLDED_CASTS, count(folds.casts)))
        .with_measurement(MirPassMeasurement::count(
            FOLDS_CROSSING_CARRIERS,
            count(folds.crossing_carriers),
        ))
        .with_measurement(MirPassMeasurement::count(
            FOLDS_CROSSING_CHECKED,
            count(folds.crossing_checked),
        ))
        .with_measurement(MirPassMeasurement::count(
            FOLDS_CROSSING_LOGICAL,
            count(folds.crossing_logical),
        ))
        .with_measurement(MirPassMeasurement::count(
            MAXIMUM_DEPENDENCY_DEPTH,
            count(folds.maximum_dependency_depth),
        ))
}

fn count(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
#[path = "primitive_constant_folding/tests.rs"]
mod tests;

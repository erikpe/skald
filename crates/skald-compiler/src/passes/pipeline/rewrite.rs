//! Verified ownership transfer for target-independent MIR transformations.

use std::fmt;

use crate::{
    identity::CallableId,
    mir::{
        rewrite::{
            rewrite_program, MirCallableEdit, MirCallableRewriteResult, MirProgramRewriteResult,
            MirRewriteError,
        },
        MirProgram, MirVerificationErrors,
    },
};

use super::{verify_final_mir, MirPipelineStatistics, VerifiedFinalMirProgram};

/// Failure stage for one pipeline-owned synthetic transformation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum MirTransformPipelineError {
    InputVerification(MirVerificationErrors),
    Rewrite(MirRewriteError),
    OutputVerification(MirVerificationErrors),
}

impl fmt::Display for MirTransformPipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputVerification(errors) => {
                write!(
                    formatter,
                    "MIR transformation input verification failed: {errors}"
                )
            }
            Self::Rewrite(error) => write!(formatter, "MIR transformation failed: {error}"),
            Self::OutputVerification(errors) => {
                write!(
                    formatter,
                    "MIR transformation output verification failed: {errors}"
                )
            }
        }
    }
}

impl std::error::Error for MirTransformPipelineError {}

/// Measured result of exercising one transforming pass through the seal.
pub(crate) struct MeasuredMirTransform {
    pub(crate) result: Result<VerifiedFinalMirProgram, MirTransformPipelineError>,
    pub(crate) statistics: MirPipelineStatistics,
    /// Present after a structurally successful rewrite, even if resealing
    /// rejects its semantic result.
    pub(crate) callables: Option<Vec<MirCallableRewriteResult>>,
}

/// Consumes a verified program into the supported whole-program rewrite
/// coordinator. Dense raw MIR, maps, and change summaries remain unsealed.
///
/// This is the only bridge between the final-MIR seal and `mir::rewrite`.
/// It intentionally performs no verification itself: the surrounding pass
/// coordinator owns input and output verification policy.
pub(super) fn rewrite_verified_final_mir(
    verified: VerifiedFinalMirProgram,
    rewrite: impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
) -> Result<MirProgramRewriteResult, MirRewriteError> {
    rewrite_program(verified.invalidate_for_transformation(), rewrite)
}

/// Exercises the future non-empty pipeline shape without registering or
/// enabling a production optimization.
///
/// The input is verified before the callback can execute. A successful dense
/// commit is verified again immediately, which is also the supported debug
/// localization point for a single synthetic transformation.
pub(crate) fn run_transforming_mir_pipeline(
    program: MirProgram,
    rewrite: impl FnMut(CallableId, &mut MirCallableEdit) -> Result<(), MirRewriteError>,
) -> MeasuredMirTransform {
    let mut statistics = MirPipelineStatistics::default();
    statistics.record_verification();
    let verified = match verify_final_mir(program) {
        Ok(verified) => verified,
        Err(errors) => {
            return MeasuredMirTransform {
                result: Err(MirTransformPipelineError::InputVerification(errors)),
                statistics,
                callables: None,
            };
        }
    };

    statistics.record_pass_execution();
    let rewritten = match rewrite_verified_final_mir(verified, rewrite) {
        Ok(rewritten) => rewritten,
        Err(error) => {
            return MeasuredMirTransform {
                result: Err(MirTransformPipelineError::Rewrite(error)),
                statistics,
                callables: None,
            };
        }
    };
    statistics.record_rewrite(&rewritten);
    let MirProgramRewriteResult { program, callables } = rewritten;

    statistics.record_verification();
    let result = verify_final_mir(program).map_err(MirTransformPipelineError::OutputVerification);
    MeasuredMirTransform {
        result,
        statistics,
        callables: Some(callables),
    }
}

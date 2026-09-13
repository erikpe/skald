//! Private mechanics shared by compiler integration tests.

mod process;
mod temporary;

// Integration binaries name only the observation types they inspect directly.
#[allow(unused_imports)]
pub(crate) use process::{
    run_current_test_process, CurrentTestProcessError, CurrentTestProcessObservation,
    CurrentTestProcessRequest, TestProcessPolicy, TestProcessTermination,
};
// Each integration binary uses a different subset of the resource facade.
#[allow(unused_imports)]
pub(crate) use temporary::{TemporaryDirectory, TemporaryFile};

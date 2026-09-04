//! Request-local composition of borrowed compiler inspection services.

use crate::passes::{static_lifecycle::StaticActivationInspector, MirPipelineInspector};

/// Optional borrowed inspectors for one compilation invocation.
///
/// Inspection is deliberately separate from [`super::CompilationRequest`],
/// reporting, diagnostics, and generated artifacts. An empty service performs
/// no inspection work. Builder methods consume and return the service so one
/// invocation has one explicit set of borrowed callbacks.
///
/// Checkpoint products cannot escape their callback:
///
/// ```compile_fail
/// use skald_compiler::{
///     driver::CompilationInspectors,
///     passes::MirPipelineCheckpoint,
/// };
///
/// let mut retained: Option<MirPipelineCheckpoint<'static>> = None;
/// let mut inspector = |checkpoint: MirPipelineCheckpoint<'_>| {
///     retained = Some(checkpoint);
/// };
/// let _ = CompilationInspectors::new().with_mir_pipeline(&mut inspector);
/// ```
#[derive(Default)]
pub struct CompilationInspectors<'a> {
    static_activation: Option<&'a mut dyn StaticActivationInspector>,
    mir_pipeline: Option<&'a mut dyn MirPipelineInspector>,
}

impl<'a> CompilationInspectors<'a> {
    pub const fn new() -> Self {
        Self {
            static_activation: None,
            mir_pipeline: None,
        }
    }

    pub fn with_static_activation(
        mut self,
        inspector: &'a mut dyn StaticActivationInspector,
    ) -> Self {
        self.static_activation = Some(inspector);
        self
    }

    pub fn with_mir_pipeline(mut self, inspector: &'a mut dyn MirPipelineInspector) -> Self {
        self.mir_pipeline = Some(inspector);
        self
    }

    pub(super) fn take_static_activation(
        &mut self,
    ) -> Option<&'a mut dyn StaticActivationInspector> {
        self.static_activation.take()
    }

    pub(super) fn take_mir_pipeline(&mut self) -> Option<&'a mut dyn MirPipelineInspector> {
        self.mir_pipeline.take()
    }
}

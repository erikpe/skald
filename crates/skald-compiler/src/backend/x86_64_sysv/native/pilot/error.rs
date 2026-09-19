use crate::backend::{
    frame::FrameError,
    lir::ProgramError as InventoryError,
    pilot::{LowerError, PilotError},
    placement::CheckFailure,
};

use super::super::{
    physical::{PhysicalError, ProgramError as PhysicalProgramError, RealizeError},
    selected::SelectionError,
    AbiError,
};

#[derive(Debug)]
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) enum NativePilotError {
    Admission(PilotError),
    Discovery(InventoryError),
    Abi(AbiError),
    Lower(LowerError),
    Selection(SelectionError),
    SelectedProgram(InventoryError),
    Placement(CheckFailure),
    Frame(FrameError),
    Realization(RealizeError),
    Physical(PhysicalError),
    PhysicalProgram(PhysicalProgramError),
    Observation(std::fmt::Error),
}

impl From<LowerError> for NativePilotError {
    fn from(error: LowerError) -> Self {
        Self::Lower(error)
    }
}

impl std::fmt::Display for NativePilotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Admission(error) => write!(f, "native pilot admission failed: {error}"),
            Self::Discovery(error) => write!(f, "native pilot discovery failed: {error:?}"),
            Self::Abi(error) => write!(f, "native pilot ABI construction failed: {error:?}"),
            Self::Lower(error) => write!(f, "native pilot lowering failed: {error}"),
            Self::Selection(error) => write!(f, "native pilot selection failed: {error}"),
            Self::SelectedProgram(error) => {
                write!(f, "native pilot selected-program closure failed: {error:?}")
            }
            Self::Placement(error) => write!(f, "native pilot placement failed: {error:?}"),
            Self::Frame(error) => write!(f, "native pilot frame planning failed: {error:?}"),
            Self::Realization(error) => {
                write!(f, "native pilot physical realization failed: {error:?}")
            }
            Self::Physical(error) => {
                write!(f, "native pilot physical verification failed: {error:?}")
            }
            Self::PhysicalProgram(error) => {
                write!(f, "native pilot program closure failed: {error}")
            }
            Self::Observation(error) => write!(f, "native pilot observation failed: {error}"),
        }
    }
}

impl std::error::Error for NativePilotError {}

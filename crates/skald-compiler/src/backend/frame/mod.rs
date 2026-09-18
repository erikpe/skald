//! Deterministic checked frame layout over an exact checked placement.
//! Target policy supplies stack and displacement bounds; planning never allocates registers.
mod addressing;
mod layout;
mod model;
mod planning;

#[cfg_attr(not(test), allow(unused_imports))]
pub(in crate::backend) use model::{AddressRecipe, Base, Region};
pub(in crate::backend) use model::{FrameError, FramePlan, FramePolicy, ReturnAddress};
pub(in crate::backend) use planning::plan_frame;
#[cfg(test)]
mod tests;

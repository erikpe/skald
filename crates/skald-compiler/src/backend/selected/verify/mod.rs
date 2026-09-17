//! Shared legality and target completeness jointly own selected publication.
mod check;
mod descriptors;
mod failure;
mod program;
mod publication;

pub(in crate::backend) use check::{verify_selected, TargetVerifier};
pub(in crate::backend) use failure::{SelectedFailure, SelectedReason};
pub(in crate::backend) use program::{SelectedProgramBuilder, VerifiedSelectedProgram};
pub(in crate::backend) use publication::{SelectedReceipt, VerifiedSelectedCallable};

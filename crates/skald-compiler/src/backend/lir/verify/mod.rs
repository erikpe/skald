//! Full lowered verification and its private publication authority.
mod check;
mod domains;
mod failure;
mod lift;
mod memory;
mod publication;
mod trace;
pub(in crate::backend) use check::verify_callable;
pub(in crate::backend) use failure::{VerificationFailure, VerificationReason};
pub(in crate::backend) use publication::{CompletionReceipt, VerifiedCallable};

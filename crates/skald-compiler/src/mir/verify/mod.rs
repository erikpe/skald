//! Structural and type verification for MIR.
//!
//! This facade owns the stable verification API. Private modules share one
//! verifier context and ordered error sink; responsibility-specific checks can
//! move behind this boundary without changing callers.

use std::fmt;

use crate::identity::CallableId;

use super::model::{BlockId, MirProgram};

mod arguments;
mod body;
mod call;
mod cleanup;
mod context;
mod declarations;
mod instructions;
mod place;
mod sink;

use context::Verifier;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVerificationError {
    pub callable: Option<CallableId>,
    pub block: Option<BlockId>,
    pub message: String,
}

impl fmt::Display for MirVerificationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.callable, self.block) {
            (_, Some(block)) => write!(formatter, "MIR {block}: {}", self.message),
            (Some(callable), None) => write!(formatter, "MIR {callable}: {}", self.message),
            (None, None) => write!(formatter, "MIR program: {}", self.message),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirVerificationErrors {
    errors: Vec<MirVerificationError>,
}

impl MirVerificationErrors {
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &MirVerificationError> {
        self.errors.iter()
    }

    pub fn len(&self) -> usize {
        self.errors.len()
    }

    pub fn is_empty(&self) -> bool {
        self.errors.is_empty()
    }
}

impl fmt::Display for MirVerificationErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, error) in self.errors.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            error.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for MirVerificationErrors {}

pub fn verify_mir(program: &MirProgram) -> Result<(), MirVerificationErrors> {
    let mut verifier = Verifier::new(program);
    verifier.verify_program();
    let errors = verifier.into_errors();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors { errors })
    }
}

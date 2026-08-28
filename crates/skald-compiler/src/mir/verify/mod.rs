//! Structural and type verification for MIR.
//!
//! This facade owns the stable verification API. Private modules share one
//! verifier context and ordered error sink; responsibility-specific checks can
//! move behind this boundary without changing callers.

use std::fmt;

use crate::identity::CallableId;

use super::model::{BlockId, MirProgram, PreliminaryMirProgram};

mod arguments;
mod array;
mod body;
mod call;
mod cell_write;
mod checked_scalar;
mod cleanup;
mod closed_program;
mod context;
mod dataflow;
mod declarations;
mod dispatch;
mod final_write;
mod function_values;
mod inheritance;
mod instructions;
mod integer_division;
mod interfaces;
mod io;
mod lifetime;
mod logical;
mod optional;
mod optional_box;
mod path_conditions;
mod path_state;
mod place;
pub(crate) mod preliminary;
mod primitive_alias;
mod primitive_cast;
mod scalar_initialization;
mod shared;
mod shift;
mod sink;
mod strings;
mod type_operations;
mod view;

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
    pub(crate) fn new(errors: Vec<MirVerificationError>) -> Self {
        Self { errors }
    }

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

pub fn verify_preliminary_mir(
    program: &PreliminaryMirProgram,
) -> Result<(), MirVerificationErrors> {
    preliminary::verify(program)
}

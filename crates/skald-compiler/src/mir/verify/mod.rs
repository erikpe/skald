//! Structural and type verification for MIR.
//!
//! This facade owns the stable verification API. Private modules share one
//! verifier context and ordered error sink; responsibility-specific checks can
//! move behind this boundary without changing callers.

use std::{fmt, ops::Deref};

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
mod contract;
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

pub(crate) use contract::{classify_local_identity_site, MirIdentitySiteRole};

pub(crate) use checked_scalar::{
    dominates as checked_scalar_dominates, predecessors as checked_scalar_predecessors,
};

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

    pub(crate) fn program(message: impl Into<String>) -> Self {
        Self::new(vec![MirVerificationError {
            callable: None,
            block: None,
            message: message.into(),
        }])
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
    finish_verification(verifier)
}

/// Verifies the executable structural contract expected after proof
/// provenance has been consumed.
///
/// This is crate-private groundwork for the future normalizer and final seal.
/// It deliberately does not recreate path-sensitive proof dataflow.
pub(crate) fn check_normalized_mir(program: &MirProgram) -> Result<(), MirVerificationErrors> {
    let mut verifier = Verifier::new_normalized(program);
    verifier.verify_program();
    finish_verification(verifier)
}

fn finish_verification(verifier: Verifier<'_>) -> Result<(), MirVerificationErrors> {
    let errors = verifier.into_errors();
    if errors.is_empty() {
        Ok(())
    } else {
        Err(MirVerificationErrors { errors })
    }
}

pub(crate) fn check_preliminary_mir(
    program: &PreliminaryMirProgram,
) -> Result<(), MirVerificationErrors> {
    preliminary::verify(program)
}

/// Read-only preliminary MIR whose complete structure and identities have
/// passed preliminary-MIR verification.
///
/// Static lifecycle analysis accepts this seal instead of relying on callers
/// to uphold an undocumented verification precondition.
///
/// External code cannot forge the seal:
///
/// ```compile_fail
/// use skald_compiler::mir::{PreliminaryMirProgram, VerifiedPreliminaryMirProgram};
///
/// fn forge(program: PreliminaryMirProgram) -> VerifiedPreliminaryMirProgram {
///     VerifiedPreliminaryMirProgram { program }
/// }
/// ```
#[derive(Clone, Eq, PartialEq)]
pub struct VerifiedPreliminaryMirProgram {
    program: PreliminaryMirProgram,
}

impl VerifiedPreliminaryMirProgram {
    pub const fn program(&self) -> &PreliminaryMirProgram {
        &self.program
    }

    pub(crate) fn into_program(self) -> PreliminaryMirProgram {
        self.program
    }
}

impl fmt::Debug for VerifiedPreliminaryMirProgram {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VerifiedPreliminaryMirProgram")
            .field("program", &self.program)
            .finish()
    }
}

impl Deref for VerifiedPreliminaryMirProgram {
    type Target = PreliminaryMirProgram;

    fn deref(&self) -> &Self::Target {
        self.program()
    }
}

/// Verifies preliminary MIR and returns the opaque product required by static
/// lifecycle analysis and planning.
pub fn verify_preliminary_mir(
    program: PreliminaryMirProgram,
) -> Result<VerifiedPreliminaryMirProgram, MirVerificationErrors> {
    check_preliminary_mir(&program)?;
    Ok(VerifiedPreliminaryMirProgram { program })
}

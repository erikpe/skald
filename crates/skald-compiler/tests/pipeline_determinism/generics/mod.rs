//! Closed generic module fixtures grouped by the contract they exercise.

mod interface;
mod module;
mod operator;

pub(crate) use interface::{
    generic_interface_diagnostic_dump, generic_interface_module_phase_dump,
};
pub(crate) use module::generic_module_phase_dump;
pub(crate) use operator::generic_operator_module_phase_dump;

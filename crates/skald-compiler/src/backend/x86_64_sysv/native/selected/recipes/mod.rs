//! Selection-time semantic diamonds over concrete numeric cells.
use super::{
    numeric::{Cell, Domain, Numeric, Recipe},
    Instruction, Opcode, Origin, ValueRef,
};
use crate::{
    backend::{
        graph::{LocalHandle, SelectedBlockId, SelectedValueId},
        lir::{BinaryOperation, Constant, Conversion, DivisionResult, ScalarCheck, ShiftDirection},
        plan::ScalarType,
        selected::{Representation, SelectedBuildError, SelectedBuilder},
    },
    primitive_comparison::PrimitiveComparisonPredicate as Predicate,
};
use std::sync::Arc;
type Result<T> = std::result::Result<T, SelectedBuildError>;
type Block<'p> = LocalHandle<'p, SelectedBlockId>;
pub(super) struct Recipes<'a, 'p> {
    pub builder: &'a mut SelectedBuilder<'p, Instruction>,
    pub resources: &'a Arc<super::super::NativeResources>,
    pub origin: Origin,
}

mod conversion;
mod division;
mod emit;

//! Stored operation uses/results; scalar, guard and effect legality are separate checks.
use super::super::{Call, CallTarget, CallableDraft, Operation, TraceAction, TraceSite};
use crate::backend::graph::{GraphFailure, GraphLocation, GraphReason, TypedUse};
use crate::backend::plan::{ReturnShape, ScalarType};
#[cfg_attr(not(test), allow(dead_code))]
impl CallableDraft<'_> {
    pub(super) fn call_description(
        &self,
        call: &Call,
        terminal: bool,
        location: GraphLocation,
    ) -> Result<(Vec<TypedUse<ScalarType>>, Vec<ScalarType>), GraphFailure> {
        let view = self.owner.context();
        let signature = view
            .signature(
                view.signature_id(call.signature.index())
                    .map_err(|error| self.lookup_error(location, error))?,
            )
            .map_err(|error| self.lookup_error(location, error))?;
        if (signature.returns == ReturnShape::Never) != terminal
            || call.arguments.len() != signature.inputs.len()
        {
            return Err(self.graph_error(location, GraphReason::ArityMismatch));
        }
        let mut uses = Vec::new();
        if let CallTarget::Indirect(target) = call.target {
            uses.push(self.used(
                target,
                Some(ScalarType::CodeAddress(call.signature)),
                location,
            )?);
        }
        for (argument, component) in call.arguments.iter().zip(&signature.inputs) {
            if argument.role != component.role {
                return Err(
                    self.graph_error(location.operand(uses.len()), GraphReason::TypeMismatch)
                );
            }
            uses.push(self.used(
                argument.value,
                Some(component.ty),
                location.operand(uses.len()),
            )?);
        }
        Ok((
            uses,
            signature.results.iter().map(|result| result.ty).collect(),
        ))
    }
    pub(super) fn operation_description(
        &self,
        operation: &Operation,
        location: GraphLocation,
    ) -> Result<(Vec<TypedUse<ScalarType>>, Vec<ScalarType>), GraphFailure> {
        use Operation::*;
        use ScalarType::*;
        let mut uses = Vec::new();
        let mut used = |id, expected| {
            uses.push(self.used(id, expected, location.operand(uses.len()))?);
            Ok::<_, GraphFailure>(())
        };
        let types = match operation {
            Call(call) => return self.call_description(call, false, location),
            Constant(constant) => vec![constant
                .scalar_type()
                .map_err(|_| self.graph_error(location, GraphReason::TypeMismatch))?],
            Unary { value, .. } => {
                let ty = self.ty(*value, location)?;
                used(*value, Some(ty))?;
                vec![ty]
            }
            Binary { left, right, .. } | Compare { left, right, .. } => {
                let ty = self.ty(*left, location)?;
                used(*left, Some(ty))?;
                used(*right, Some(ty))?;
                vec![if matches!(operation, Compare { .. }) {
                    Bool
                } else {
                    ty
                }]
            }
            Divide {
                dividend,
                divisor,
                evidence,
                ..
            } => {
                let ty = self.ty(*dividend, location)?;
                used(*dividend, Some(ty))?;
                used(*divisor, Some(ty))?;
                self.evidence_references(evidence, location)?;
                vec![ty]
            }
            Shift {
                value,
                count,
                evidence,
                ..
            } => {
                let ty = self.ty(*value, location)?;
                used(*value, Some(ty))?;
                used(*count, Some(U64))?;
                self.evidence_references(evidence, location)?;
                vec![ty]
            }
            Convert {
                value,
                target,
                evidence,
                ..
            } => {
                used(*value, None)?;
                if let Some(evidence) = evidence {
                    self.evidence_references(evidence, location)?;
                }
                vec![*target]
            }
            SymbolAddress { ty, .. } => vec![*ty],
            ObjectAddress(object) => {
                self.object_index(*object, location)?;
                vec![DataAddress]
            }
            ByteOffset { base, offset } => {
                used(*base, Some(DataAddress))?;
                used(*offset, None)?;
                vec![DataAddress]
            }
            ScaledIndex { base, index, .. } => {
                used(*base, Some(DataAddress))?;
                used(*index, None)?;
                vec![DataAddress]
            }
            Load {
                address,
                representation,
            } => {
                used(*address, Some(DataAddress))?;
                vec![representation.scalar]
            }
            Store {
                address,
                value,
                representation,
            } => {
                used(*address, Some(DataAddress))?;
                used(*value, Some(representation.scalar))?;
                vec![]
            }
            Lifetime { object, .. } => {
                self.object_index(*object, location)?;
                vec![]
            }
            Trace(action) => {
                let record = match action {
                    TraceAction::PushFrame { record } | TraceAction::PopFrame { record } => record,
                    TraceAction::ReplaceLocation { record, site, .. } => {
                        match site {
                            TraceSite::Instruction { block, ordinal } => {
                                self.block_index(*block, location)?;
                                if *ordinal
                                    >= self
                                        .blocks
                                        .get_id(*block)
                                        .map_err(|error| self.lookup_error(location, error))?
                                        .instructions
                                        .len()
                                {
                                    return Err(
                                        self.graph_error(location, GraphReason::OutOfBounds)
                                    );
                                }
                            }
                            TraceSite::Terminator(block) => {
                                self.block_index(*block, location)?;
                            }
                        }
                        record
                    }
                };
                self.object_index(*record, location)?;
                vec![]
            }
        };
        Ok((uses, types))
    }
}

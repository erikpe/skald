//! Reclassify the checked signature and compare actual call fields. No producer
//! binding table or claim about secured marshaling confers verification authority.
use super::{
    super::{Instruction, Opcode},
    Verifier,
};
use crate::backend::{
    lir::CallTarget,
    plan::ArtifactId,
    selected::{Constraint, Payload, RepresentationKind, SelectionContext, Timing},
    x86_64_sysv::native::{classify, CallArity, Gpr},
};
impl Verifier {
    pub(in crate::backend::x86_64_sysv::native::selected) fn check_call(
        &self,
        context: &SelectionContext<'_>,
        node: &Instruction,
    ) -> Result<(), &'static str> {
        let Opcode::Call(call) = &node.opcode else {
            return Err("expected native call");
        };
        let abi = classify(
            context.catalog().plan(),
            call.signature,
            &self.resources,
            CallArity::Fixed,
        )
        .map_err(|_| "unsupported native call signature")?;
        if call.inputs != abi.call().inputs()
            || call.outputs != abi.call().results()
            || call.never != abi.noreturn()
        {
            return Err("noncanonical native call ABI");
        }
        match call.target {
            CallTarget::Direct(target) => {
                let signature = if let ArtifactId::Callable(key) = target {
                    Some(
                        context
                            .catalog()
                            .selection_binding(key)
                            .map_err(|_| "unknown native call target")?
                            .signature_id(),
                    )
                } else {
                    let plan = context.catalog().plan();
                    plan.artifact(
                        plan.artifact_id(target)
                            .map_err(|_| "unknown native call target")?,
                        target.category(),
                    )
                    .map_err(|_| "unknown native call target")?
                    .signature
                };
                if signature != Some(call.signature) {
                    return Err("native direct call signature mismatch");
                }
            }
            CallTarget::Indirect(target) => {
                if target.representation.kind != RepresentationKind::CodeAddress(call.signature)
                    || target.representation.bits() != 64
                {
                    return Err("native indirect call signature mismatch");
                }
                let desc = node.describe();
                let op = desc
                    .operands
                    .get(call.arguments.len())
                    .ok_or("missing native indirect target")?;
                if op.timing != Timing::Late
                    || !matches!(op.constraint, Constraint::Fixed(view) if Some(view) == self.resources.gpr(Gpr::R11, 64).ok())
                {
                    return Err("unsecured native indirect call target");
                }
            }
        }
        Ok(())
    }
}

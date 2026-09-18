//! One immutable selection namespace, with shapes scoped to checked signatures.
use super::super::{classify, AbiError, CallArity, NativeResources};
use crate::backend::{
    lir::TargetCatalog,
    selected::{AbiAreas, SelectionContext},
};
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::backend) fn selection_context<'p>(
    catalog: &'p TargetCatalog<'p>,
) -> Result<SelectionContext<'p>, AbiError> {
    let resources =
        NativeResources::new().map_err(|_| crate::backend::plan::PlanError::InvalidProfile)?;
    let mut context = SelectionContext::new(catalog, resources.catalog().clone());
    for (signature, _) in catalog.plan().signatures_with_ids() {
        let abi = match classify(catalog.plan(), signature, &resources, CallArity::Fixed) {
            Ok(abi) => abi,
            // Absent unsupported external declarations remain inspectable;
            // only a real selected consumer can grant executable authority.
            Err(AbiError::UnsupportedExternal) => continue,
            Err(error) => return Err(error),
        };
        context = context.with_abi_areas(
            signature,
            AbiAreas {
                incoming: abi.stack_slots().to_vec(),
                outgoing: abi.stack_slots().to_vec(),
                results: vec![],
            },
        )?;
    }
    Ok(context)
}

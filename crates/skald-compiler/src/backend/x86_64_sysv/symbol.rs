//! Collision-proof assembly symbols derived from canonical identities.

use crate::{
    identity::CallableId,
    mir::{MirFunctionLinkage, MirProgram},
};

pub(super) fn callable(program: &MirProgram, callable: CallableId) -> String {
    match callable {
        CallableId::Function(function) => {
            let declaration = program
                .declarations
                .get(function)
                .expect("verified function callable must be declared");
            match &declaration.linkage {
                MirFunctionLinkage::Internal => format!(".Lska_fn_{}", function.index()),
                MirFunctionLinkage::External { symbol } => symbol.clone(),
            }
        }
        CallableId::Initializer(initializer) => format!(
            ".Lska_class_{}_init_{}",
            initializer.class().index(),
            initializer.index()
        ),
        CallableId::Destructor(destructor) => format!(
            ".Lska_class_{}_destroy_{}",
            destructor.class().index(),
            destructor.index()
        ),
        CallableId::Method(method) => format!(
            ".Lska_class_{}_method_{}",
            method.class().index(),
            method.index()
        ),
    }
}

pub(super) fn local_label_stem(callable: CallableId) -> String {
    match callable {
        CallableId::Function(function) => format!("fn_{}", function.index()),
        CallableId::Initializer(initializer) => format!(
            "class_{}_init_{}",
            initializer.class().index(),
            initializer.index()
        ),
        CallableId::Destructor(destructor) => format!(
            "class_{}_destroy_{}",
            destructor.class().index(),
            destructor.index()
        ),
        CallableId::Method(method) => {
            format!("class_{}_method_{}", method.class().index(), method.index())
        }
    }
}

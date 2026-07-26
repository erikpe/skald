//! Collision-proof assembly symbols derived from canonical identities.

use crate::{
    identity::{ArrayTypeId, CallableId, ClassId},
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
        CallableId::CopyConstructor(copy) => {
            format!(".Lska_class_{}_copy_{}", copy.class().index(), copy.index())
        }
        CallableId::CopyAssignment(assignment) => format!(
            ".Lska_class_{}_assign_{}",
            assignment.class().index(),
            assignment.index()
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

pub(super) fn dispatch_table(class: ClassId) -> String {
    format!(".Lska_class_{}_dispatch", class.index())
}

pub(super) fn complete_finalizer(class: ClassId) -> String {
    format!(".Lska_class_{}_finalize_complete", class.index())
}

pub(super) fn class_copy_helper(class: ClassId) -> String {
    format!(".Lska_class_{}_copy_complete", class.index())
}

pub(super) fn array_initialize_element(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_initialize_element", array.index())
}

pub(super) fn array_copy_element(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_copy_element", array.index())
}

pub(super) fn array_clone(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_clone", array.index())
}

pub(super) fn array_destroy_element(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_destroy_element", array.index())
}

pub(super) fn array_release(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_release", array.index())
}

pub(super) fn local_label_stem(callable: CallableId) -> String {
    match callable {
        CallableId::Function(function) => format!("fn_{}", function.index()),
        CallableId::Initializer(initializer) => format!(
            "class_{}_init_{}",
            initializer.class().index(),
            initializer.index()
        ),
        CallableId::CopyConstructor(copy) => {
            format!("class_{}_copy_{}", copy.class().index(), copy.index())
        }
        CallableId::CopyAssignment(assignment) => format!(
            "class_{}_assign_{}",
            assignment.class().index(),
            assignment.index()
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

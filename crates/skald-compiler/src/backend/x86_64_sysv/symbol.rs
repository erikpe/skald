//! Collision-proof assembly symbols derived from canonical identities.

use crate::{
    identity::{ArrayTypeId, CallableId, ClassId, ModuleId, StaticFieldId},
    mir::{MirFunctionLinkage, MirProgram},
};

// Source identifiers and logical module components cannot contain dots, so
// dot-separated names retain their readable boundaries. Request-local
// declaration IDs remain as suffixes to preserve collision-proof identity.
pub(super) fn callable(program: &MirProgram, callable: CallableId) -> String {
    match callable {
        CallableId::Function(function) => {
            let declaration = program
                .declarations
                .get(function)
                .expect("verified function callable must be declared");
            match &declaration.linkage {
                MirFunctionLinkage::Internal => {
                    format!(".Lska.{}", callable_stem(program, callable))
                }
                MirFunctionLinkage::External { link } => program
                    .external_links
                    .get(*link)
                    .expect("verified external function must reference a link entry")
                    .symbol
                    .clone(),
                MirFunctionLinkage::Intrinsic { .. } => {
                    unreachable!("verified MIR must not call an intrinsic declaration")
                }
            }
        }
        _ => format!(".Lska.{}", callable_stem(program, callable)),
    }
}

pub(super) fn dispatch_table(program: &MirProgram, class: ClassId) -> String {
    format!(".Lska.{}.dispatch", class_stem(program, class))
}

pub(super) fn static_field(program: &MirProgram, field: StaticFieldId) -> String {
    format!(
        ".Lska.{}.static.s{}",
        class_stem(program, field.class()),
        field.index()
    )
}

pub(super) fn program_initializer() -> String {
    ".Lska.static.initialize".to_owned()
}

pub(super) fn program_finalizer() -> String {
    ".Lska.static.finalize".to_owned()
}

pub(super) fn complete_finalizer(program: &MirProgram, class: ClassId) -> String {
    format!(".Lska.{}.finalize_complete", class_stem(program, class))
}

pub(super) fn class_copy_helper(program: &MirProgram, class: ClassId) -> String {
    format!(".Lska.{}.copy_complete", class_stem(program, class))
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

pub(super) fn shared_array_metadata(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_shared_metadata", array.index())
}

pub(super) fn shared_array_finalizer(array: ArrayTypeId) -> String {
    format!(".Lska_array_{}_finalize_shared", array.index())
}

pub(super) fn literal_backing(pool_index: usize) -> String {
    format!(".Lska_literal_{pool_index}_backing")
}

pub(super) fn trace_byte_string(index: usize) -> String {
    format!(".Lska.trace.bytes.{index}")
}

pub(super) fn trace_context(program: &MirProgram, callable: CallableId) -> String {
    format!(".Lska.trace.context.{}", callable_stem(program, callable))
}

pub(super) fn trace_location(
    program: &MirProgram,
    callable: CallableId,
    line: u64,
    column: u64,
) -> String {
    format!(
        ".Lska.trace.location.{}.l{line}.c{column}",
        callable_stem(program, callable)
    )
}

pub(super) fn shared_handle_retain() -> String {
    ".Lska_shared_handle_retain".to_owned()
}

pub(super) fn shared_handle_release() -> String {
    ".Lska_shared_handle_release".to_owned()
}

pub(super) fn local_label_stem(program: &MirProgram, callable: CallableId) -> String {
    callable_stem(program, callable)
}

pub(super) fn class_label_stem(program: &MirProgram, class: ClassId) -> String {
    class_stem(program, class)
}

fn callable_stem(program: &MirProgram, callable: CallableId) -> String {
    match callable {
        CallableId::Function(function) => {
            let declaration = program
                .declarations
                .get(function)
                .expect("verified function callable must be declared");
            format!(
                "fn.{}.{}.f{}",
                module_stem(program, declaration.module),
                declaration.name,
                function.index()
            )
        }
        CallableId::StaticInitializer(initializer) => {
            let body = program
                .static_lifecycle
                .as_ref()
                .and_then(|coordinator| {
                    coordinator
                        .initializers()
                        .iter()
                        .find(|body| body.id == initializer)
                })
                .expect("verified static initializer callable must have a lifecycle body");
            format!(
                "{}.static.s{}.initialize",
                class_stem(program, body.field.class()),
                body.field.index()
            )
        }
        CallableId::Initializer(initializer) => format!(
            "{}.init.i{}",
            class_stem(program, initializer.class()),
            initializer.index()
        ),
        CallableId::CopyConstructor(copy) => {
            format!(
                "{}.copy.k{}",
                class_stem(program, copy.class()),
                copy.index()
            )
        }
        CallableId::CopyAssignment(assignment) => format!(
            "{}.assign.a{}",
            class_stem(program, assignment.class()),
            assignment.index()
        ),
        CallableId::Destructor(destructor) => format!(
            "{}.destroy.d{}",
            class_stem(program, destructor.class()),
            destructor.index()
        ),
        CallableId::Method(method) => {
            let class = program
                .classes
                .get(method.class())
                .expect("verified method callable must belong to a declared class");
            let declaration = class
                .method(method)
                .expect("verified method callable must be declared");
            format!(
                "{}.method.{}.m{}",
                class_stem(program, method.class()),
                declaration.name,
                method.index()
            )
        }
    }
}

fn class_stem(program: &MirProgram, class: ClassId) -> String {
    let declaration = program
        .classes
        .get(class)
        .expect("verified class symbol must reference a declared class");
    format!(
        "class.{}.{}.c{}",
        module_stem(program, declaration.module),
        declaration.name,
        class.index()
    )
}

fn module_stem(program: &MirProgram, module: ModuleId) -> String {
    program
        .modules
        .get(module)
        .expect("verified symbol owner must reference a loaded module")
        .module_path()
        .components()
        .collect::<Vec<_>>()
        .join(".")
}

//! Target-independent destruction-plan queries used at the DD4/DD5 boundary.

use crate::{
    identity::ClassId,
    mir::{MirDestructionStep, MirProgram, MirType},
};

/// Whether destroying `class` can execute user code.
///
/// DD4 cleanup for an operationally empty plan can safely select to no machine
/// instructions. DD5 will replace this boundary for non-trivial plans with
/// mechanical calls through the already-verified MIR plan.
pub(super) fn requires_runtime_work(program: &MirProgram, class: ClassId) -> bool {
    requires_runtime_work_inner(program, class, &mut Vec::new())
}

fn requires_runtime_work_inner(
    program: &MirProgram,
    class: ClassId,
    visiting: &mut Vec<ClassId>,
) -> bool {
    if visiting.contains(&class) {
        // Recursive inline layouts are rejected before instruction selection.
        return false;
    }
    visiting.push(class);
    let declaration = program
        .class(class)
        .expect("verified cleanup target must name a declared class");
    let result = declaration.destruction.steps.iter().any(|step| match step {
        MirDestructionStep::UserBody(_) => true,
        MirDestructionStep::Field(field) => {
            let field = declaration
                .field(*field)
                .expect("verified destruction step must name a declared field");
            match field.ty {
                MirType::Class(field_class) => {
                    requires_runtime_work_inner(program, field_class, visiting)
                }
                _ => false,
            }
        }
    });
    visiting.pop();
    result
}

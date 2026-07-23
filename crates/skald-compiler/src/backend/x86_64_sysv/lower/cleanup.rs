//! Mechanical lowering of verified complete-object destruction plans.

use crate::{
    backend::{BackendError, Target},
    identity::ClassId,
    mir::{MirCleanup, MirDestructionStep, MirPlace, MirType},
};

use super::InstructionSelector;

impl InstructionSelector<'_, '_> {
    pub(super) fn select_cleanup(&mut self, cleanup: &MirCleanup) -> Result<(), BackendError> {
        self.select_destruction_plan(cleanup.target, cleanup.destination.clone())
    }

    fn select_destruction_plan(
        &mut self,
        class: ClassId,
        destination: MirPlace,
    ) -> Result<(), BackendError> {
        let steps = self
            .program
            .class(class)
            .ok_or_else(|| {
                self.cleanup_error(format!("cleanup target names unknown class {class}"))
            })?
            .destruction
            .steps
            .clone();

        for step in steps {
            match step {
                MirDestructionStep::Base(_) => {
                    unreachable!("target legality rejects base destruction before lowering")
                }
                MirDestructionStep::UserBody(destructor) => {
                    self.select_destructor_call(destructor, &destination)?;
                }
                MirDestructionStep::Field(field) => {
                    let field_class = match self
                        .program
                        .field(field)
                        .ok_or_else(|| {
                            self.cleanup_error(format!(
                                "destruction plan for {class} names unknown field {field}"
                            ))
                        })?
                        .ty
                    {
                        MirType::Class(field_class) => field_class,
                        _ => {
                            return Err(self.cleanup_error(format!(
                                "destruction plan for {class} contains non-class field {field}"
                            )))
                        }
                    };
                    self.select_destruction_plan(
                        field_class,
                        destination.clone().project_field(field),
                    )?;
                }
            }
        }
        Ok(())
    }

    fn cleanup_error(&self, message: impl Into<String>) -> BackendError {
        BackendError::new(Target::X86_64SysV, Some(self.function.callable()), message)
    }
}

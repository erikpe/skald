//! Mechanical lowering of verified complete-object destruction plans.

use crate::{
    backend::BackendError,
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
        let step_count = self
            .program
            .class(class)
            .expect("verified cleanup target must name a declared class")
            .destruction
            .steps
            .len();

        for index in 0..step_count {
            let step = self
                .program
                .class(class)
                .expect("verified cleanup target must name a declared class")
                .destruction
                .steps[index];
            match step {
                MirDestructionStep::UserBody(destructor) => {
                    self.select_destructor_call(destructor, &destination)?;
                }
                MirDestructionStep::Field(field) => {
                    let field_class = match self
                        .program
                        .field(field)
                        .expect("verified destruction step must name a declared field")
                        .ty
                    {
                        MirType::Class(field_class) => field_class,
                        _ => unreachable!("verified destruction plan contains only class fields"),
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
}

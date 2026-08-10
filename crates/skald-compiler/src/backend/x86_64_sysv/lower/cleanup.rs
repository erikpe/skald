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

    pub(super) fn select_destruction_plan(
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
                crate::mir::MirDestructionStep::ArrayField(field) => {
                    let array = match self
                        .program
                        .field(field)
                        .ok_or_else(|| {
                            self.cleanup_error(format!(
                                "destruction plan for {class} names unknown field {field}"
                            ))
                        })?
                        .ty
                    {
                        MirType::Array(array) => array,
                        _ => {
                            return Err(self.cleanup_error(format!(
                                "destruction plan for {class} contains non-array field {field}"
                            )))
                        }
                    };
                    self.select_array_field_cleanup(
                        &destination.clone().project_field(field),
                        array,
                    )?;
                }
                MirDestructionStep::Base(base) => {
                    self.select_destruction_plan(base, destination.clone().project_base(base))?;
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
                MirDestructionStep::SharedField(field) => {
                    self.release_shared_place(
                        &destination.clone().project_field(field),
                        "cleanup_shared_field",
                    )?;
                }
                MirDestructionStep::OptionalSharedField(field) => {
                    self.release_optional_shared_place(
                        &destination.clone().project_field(field),
                        "cleanup_optional_shared_field",
                    )?;
                }
                MirDestructionStep::OptionalClassField(field) => {
                    let field_type = self
                        .program
                        .field(field)
                        .ok_or_else(|| {
                            self.cleanup_error(format!(
                                "destruction plan for {class} names unknown field {field}"
                            ))
                        })?
                        .ty;
                    let MirType::Optional(optional) = field_type else {
                        return Err(self.cleanup_error(format!(
                            "destruction plan for {class} contains non-optional-class field {field}"
                        )));
                    };
                    let field_class = self
                        .program
                        .optional_type(optional)
                        .and_then(crate::mir::MirOptionalType::inline_class)
                        .ok_or_else(|| {
                            self.cleanup_error(format!(
                            "destruction plan for {class} contains non-optional-class field {field}"
                        ))
                        })?;
                    self.select_class_optional_cleanup(&crate::mir::MirClassOptionalCleanup {
                        optional,
                        destination: destination.clone().project_field(field),
                        class: field_class,
                        span: self.function.span(),
                    })?;
                }
                MirDestructionStep::OptionalField { field, optional } => {
                    self.select_aggregate_optional_cleanup(
                        &crate::mir::MirAggregateOptionalCleanup {
                            optional,
                            destination: destination.clone().project_field(field),
                            span: self.function.span(),
                        },
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

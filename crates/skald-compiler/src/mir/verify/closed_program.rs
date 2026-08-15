//! Closed-program type identity verification.
//!
//! MIR has no parameter-bearing type variant. This audit additionally proves
//! that every concrete type identity occurring anywhere in the executable
//! product belongs to its canonical program table. Responsibility-specific
//! verifiers remain authoritative for storage legality and operation plans.

use crate::mir::{MirSharedTarget, MirType};

use super::context::Verifier;

impl Verifier<'_> {
    pub(super) fn verify_closed_type_references(&mut self) {
        let mut references = Vec::new();

        for function in self.program.function_types.iter() {
            collect_signature_types(
                &mut references,
                format!("function type {}", function.id),
                &function.parameters,
                function.result,
            );
        }

        for array in self.program.array_types.iter() {
            references.push((format!("array {} element", array.id), array.element));
        }
        for optional in self.program.optional_types.iter() {
            references.push((
                format!("optional {} payload", optional.id),
                optional.payload,
            ));
        }
        for interface in self.program.interfaces.iter() {
            for requirement in &interface.requirements {
                collect_signature_types(
                    &mut references,
                    format!("interface requirement {}", requirement.id),
                    &requirement.parameters,
                    requirement.return_type,
                );
            }
        }
        for class in self.program.classes.iter() {
            for field in &class.fields {
                references.push((format!("field {}", field.id), field.ty));
            }
            for field in &class.static_fields {
                references.push((format!("static field {}", field.id), field.ty));
            }
            for initializer in &class.initializers {
                collect_parameter_types(
                    &mut references,
                    format!("initializer {}", initializer.id),
                    &initializer.parameters,
                );
            }
            if let Some(copy) = &class.copy_constructor_declaration {
                collect_parameter_types(
                    &mut references,
                    format!("copy constructor {}", copy.id),
                    &copy.parameters,
                );
            }
            if let Some(assignment) = &class.copy_assignment_declaration {
                references.push((
                    format!("copy assignment {} parameter", assignment.id),
                    assignment.parameter.ty,
                ));
            }
            for method in &class.methods {
                collect_signature_types(
                    &mut references,
                    format!("method {}", method.id),
                    &method.parameters,
                    method.return_type,
                );
            }
        }
        for function in self.program.declarations.iter() {
            collect_signature_types(
                &mut references,
                format!("function {}", function.id),
                &function.parameters,
                function.return_type,
            );
        }
        for definition in self.program.executable_definitions() {
            for storage in definition.storage_entries() {
                references.push((format!("storage {}", storage.id), storage.ty));
            }
            for value in definition.values() {
                references.push((format!("value {}", value.id), value.ty));
            }
        }

        for (owner, ty) in references {
            if !self.type_is_declared(ty) {
                self.program_error(format!(
                    "{owner} references {ty}, which is absent from the closed MIR type tables"
                ));
            }
        }
    }

    fn type_is_declared(&self, ty: MirType) -> bool {
        match ty {
            MirType::Array(array) => self.program.array_type(array).is_some(),
            MirType::Function(function) => self.program.function_type(function).is_some(),
            MirType::Class(class) => self.program.class(class).is_some(),
            MirType::Interface(interface) => self.program.interface(interface).is_some(),
            MirType::Shared(target) => self.shared_target_is_declared(target),
            MirType::Optional(optional) => self.program.optional_type(optional).is_some(),
            MirType::I64
            | MirType::U64
            | MirType::U8
            | MirType::F64
            | MirType::Bool
            | MirType::Obj
            | MirType::Unit => true,
        }
    }

    fn shared_target_is_declared(&self, target: MirSharedTarget) -> bool {
        match target {
            MirSharedTarget::Class(class) => self.program.class(class).is_some(),
            MirSharedTarget::Interface(interface) => self.program.interface(interface).is_some(),
            MirSharedTarget::Array(array) => self.program.array_type(array).is_some(),
            MirSharedTarget::OptionalBox(optional_box) => {
                self.program.optional_box_type(optional_box).is_some()
            }
            MirSharedTarget::Obj => true,
        }
    }
}

fn collect_signature_types(
    references: &mut Vec<(String, MirType)>,
    owner: String,
    parameters: &[crate::mir::MirParameter],
    result: MirType,
) {
    collect_parameter_types(references, owner.clone(), parameters);
    references.push((format!("{owner} result"), result));
}

fn collect_parameter_types(
    references: &mut Vec<(String, MirType)>,
    owner: String,
    parameters: &[crate::mir::MirParameter],
) {
    references.extend(
        parameters
            .iter()
            .enumerate()
            .map(|(index, parameter)| (format!("{owner} parameter {index}"), parameter.ty)),
    );
}

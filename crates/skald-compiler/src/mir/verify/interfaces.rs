//! Interface declaration and effective class-conformance verification.

use std::collections::HashSet;

use crate::identity::{ClassId, InterfaceId, MethodId};

use super::{
    super::model::{
        MirInterfaceConformance, MirInterfaceRequirement, MirMethodDeclaration, MirParameter,
        MirParameterMode, MirType,
    },
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_interfaces(&mut self) {
        for (index, interface) in self.program.interfaces.iter().enumerate() {
            if interface.id.index() != index {
                self.program_error(format!(
                    "interface declaration table index {index} contains {}",
                    interface.id
                ));
            }
            let mut names = HashSet::new();
            for (requirement_index, requirement) in interface.requirements.iter().enumerate() {
                if requirement.id.interface() != interface.id
                    || requirement.id.index() != requirement_index
                {
                    self.program_error(format!(
                        "interface {} requirement table index {requirement_index} contains {}",
                        interface.id, requirement.id
                    ));
                }
                if !names.insert(&requirement.name) {
                    self.program_error(format!(
                        "interface {} contains duplicate requirement name `{}`",
                        interface.id, requirement.name
                    ));
                }
                self.verify_interface_requirement(requirement);
            }
        }

        for class in self.program.classes.iter() {
            let mut interfaces = HashSet::new();
            for conformance in &class.conformances {
                if !interfaces.insert(conformance.interface) {
                    self.program_error(format!(
                        "class {} contains duplicate conformance for {}",
                        class.id, conformance.interface
                    ));
                }
                self.verify_interface_conformance(class.id, conformance);
            }
            if let Some(base) = class.direct_base {
                let inherited: Vec<_> = self
                    .program
                    .class(base.class)
                    .into_iter()
                    .flat_map(|base| &base.conformances)
                    .map(|conformance| conformance.interface)
                    .collect();
                for interface in inherited {
                    if !interfaces.contains(&interface) {
                        self.program_error(format!(
                            "class {} omits inherited conformance for {interface}",
                            class.id
                        ));
                    }
                }
            }
        }
    }

    fn verify_interface_requirement(&mut self, requirement: &MirInterfaceRequirement) {
        let owner = format!("interface requirement {}", requirement.id);
        self.verify_interface_parameters(&owner, &requirement.parameters);
        self.verify_interface_type(&owner, "result", requirement.return_type);
        if matches!(
            requirement.return_type,
            MirType::Interface(_) | MirType::Obj
        ) {
            self.program_error(format!(
                "{owner} cannot return a non-owning interface or `Obj` type"
            ));
        }
    }

    fn verify_interface_parameters(&mut self, owner: &str, parameters: &[MirParameter]) {
        for (index, parameter) in parameters.iter().enumerate() {
            match parameter.mode {
                MirParameterMode::Value
                    if matches!(
                        parameter.ty,
                        MirType::Interface(_) | MirType::Obj | MirType::Unit
                    ) =>
                {
                    self.program_error(format!(
                        "{owner} value parameter {index} has a non-owning or payload-free type"
                    ));
                }
                MirParameterMode::ReadOnlyAlias | MirParameterMode::MutableAlias
                    if !matches!(
                        parameter.ty,
                        MirType::Class(_)
                            | MirType::Interface(_)
                            | MirType::Obj
                            | MirType::OptionalPrimitive(_)
                            | MirType::OptionalClass(_)
                    ) =>
                {
                    self.program_error(format!(
                        "{owner} alias parameter {index} must have object-view or inline-optional type"
                    ));
                }
                _ => {}
            }
            self.verify_interface_type(owner, &format!("parameter {index}"), parameter.ty);
        }
    }

    fn verify_interface_type(&mut self, owner: &str, position: &str, ty: MirType) {
        match ty {
            MirType::Class(class) if self.program.class(class).is_none() => {
                self.program_error(format!(
                    "{owner} {position} has undeclared class type {class}"
                ));
            }
            MirType::Interface(interface) if self.program.interface(interface).is_none() => {
                self.program_error(format!(
                    "{owner} {position} has undeclared interface type {interface}"
                ));
            }
            _ => {}
        }
    }

    fn verify_interface_conformance(
        &mut self,
        class: ClassId,
        conformance: &MirInterfaceConformance,
    ) {
        let Some(interface) = self.program.interface(conformance.interface) else {
            self.program_error(format!(
                "class {class} conforms to undeclared interface {}",
                conformance.interface
            ));
            return;
        };
        if conformance.implementations.len() != interface.requirements.len() {
            self.program_error(format!(
                "class {class} conformance to {} has {} implementations but requires {}",
                interface.id,
                conformance.implementations.len(),
                interface.requirements.len()
            ));
        }
        for (index, implementation) in conformance.implementations.iter().enumerate() {
            let Some(requirement) = interface.requirements.get(index) else {
                self.verify_extra_conformance_method(class, interface.id, implementation.method);
                continue;
            };
            if implementation.requirement != requirement.id {
                self.program_error(format!(
                    "class {class} conformance to {} implementation {index} names {} instead of {}",
                    interface.id, implementation.requirement, requirement.id
                ));
            }
            let Some(method) = self.program.method(implementation.method) else {
                self.program_error(format!(
                    "class {class} conformance to {} selects undeclared method {}",
                    interface.id, implementation.method
                ));
                continue;
            };
            if implementation.method.class() != class
                && !self
                    .program
                    .is_ancestor(implementation.method.class(), class)
            {
                self.program_error(format!(
                    "class {class} conformance to {} selects method {} outside its hierarchy",
                    interface.id, implementation.method
                ));
            }
            if self.effective_named_method(class, &requirement.name) != Some(method.id) {
                self.program_error(format!(
                    "class {class} conformance to {} does not select its effective `{}` method",
                    interface.id, requirement.name
                ));
            }
            if !requirement_matches_method(requirement, method) {
                self.program_error(format!(
                    "class {class} conformance to {} maps {} to a method with a different signature or receiver access",
                    interface.id, requirement.id
                ));
            }
            if self.program.member_definition(method.id.into()).is_none() {
                self.program_error(format!(
                    "class {class} conformance to {} maps {} to method {} without an executable definition",
                    interface.id, requirement.id, method.id
                ));
            }
        }
    }

    fn verify_extra_conformance_method(
        &mut self,
        class: ClassId,
        interface: InterfaceId,
        method: MethodId,
    ) {
        if self.program.method(method).is_none() {
            self.program_error(format!(
                "class {class} conformance to {interface} contains undeclared extra method {method}"
            ));
        } else {
            self.program_error(format!(
                "class {class} conformance to {interface} contains an extra implementation"
            ));
        }
    }

    fn effective_named_method(&self, mut class: ClassId, name: &str) -> Option<MethodId> {
        for _ in 0..self.program.classes.len() {
            let declaration = self.program.class(class)?;
            if let Some(method) = declaration
                .methods
                .iter()
                .find(|method| method.name == name)
            {
                return Some(method.id);
            }
            let base = declaration.direct_base?;
            class = base.class;
        }
        None
    }
}

fn requirement_matches_method(
    requirement: &MirInterfaceRequirement,
    method: &MirMethodDeclaration,
) -> bool {
    requirement.receiver_access == method.receiver_access
        && requirement.parameters == method.parameters
        && requirement.return_type == method.return_type
}

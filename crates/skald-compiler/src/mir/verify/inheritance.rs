//! Canonical MIR hierarchy and direct-base lifecycle verification.

use std::collections::HashSet;

use crate::identity::{CopyAssignmentId, InitializerId};

use super::{
    super::model::{MirBaseCopy, MirClassDeclaration},
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_class_hierarchy(&mut self) {
        for class in self.program.classes.iter() {
            let Some(base) = class.direct_base else {
                continue;
            };
            if self.program.class(base.class).is_none() {
                self.program_error(format!(
                    "class {} has undeclared direct base {}",
                    class.id, base.class
                ));
                continue;
            }
            if base.class == class.id {
                self.program_error(format!("class {} cannot be its own direct base", class.id));
                continue;
            }

            let mut visited = HashSet::from([class.id]);
            let mut current = base.class;
            for _ in 0..self.program.classes.len() {
                if !visited.insert(current) {
                    self.program_error(format!(
                        "class {} direct-base chain contains a cycle",
                        class.id
                    ));
                    break;
                }
                let Some(next) = self.program.direct_base(current) else {
                    break;
                };
                current = next;
            }
        }
    }

    pub(super) fn verify_constructor_base(
        &mut self,
        class: &MirClassDeclaration,
        actual: Option<MirBaseCopy<InitializerId>>,
    ) {
        let expected = class.direct_base.and_then(|direct| {
            self.program
                .class(direct.class)
                .and_then(|base| base.copy_constructor.selected())
                .map(|operation| MirBaseCopy {
                    base: direct.class,
                    operation,
                })
        });
        if actual != expected {
            self.program_error(format!(
                "class {} copy-construction plan has an invalid direct-base step",
                class.id
            ));
        }
    }

    pub(super) fn verify_assignment_base(
        &mut self,
        class: &MirClassDeclaration,
        actual: Option<MirBaseCopy<CopyAssignmentId>>,
    ) {
        let expected = class.direct_base.and_then(|direct| {
            self.program
                .class(direct.class)
                .and_then(|base| base.copy_assignment.selected())
                .map(|operation| MirBaseCopy {
                    base: direct.class,
                    operation,
                })
        });
        if actual != expected {
            self.program_error(format!(
                "class {} copy-assignment plan has an invalid direct-base step",
                class.id
            ));
        }
    }
}

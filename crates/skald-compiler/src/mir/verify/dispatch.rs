//! Virtual-family declaration verification.

use std::collections::HashSet;

use super::{
    super::model::{MirMethodDeclaration, MirVirtualFamily},
    context::Verifier,
};

impl Verifier<'_> {
    pub(super) fn verify_virtual_families(&mut self) {
        let mut roots = HashSet::new();
        let mut members = HashSet::new();
        for (index, family) in self.program.virtual_families.iter().enumerate() {
            if family.id.index() != index {
                self.program_error(format!(
                    "virtual-family table index {index} contains {}",
                    family.id
                ));
            }
            if family.slot.index() != index {
                self.program_error(format!(
                    "virtual family {} has non-canonical slot {}",
                    family.id, family.slot
                ));
            }
            if !roots.insert(family.root) {
                self.program_error(format!(
                    "virtual method {} roots more than one family",
                    family.root
                ));
            }
            if family.members.first() != Some(&family.root) {
                self.program_error(format!(
                    "virtual family {} must list its root first",
                    family.id
                ));
            }
            self.verify_virtual_family(family, &mut members);
        }
    }

    fn verify_virtual_family(
        &mut self,
        family: &MirVirtualFamily,
        all_members: &mut HashSet<crate::identity::MethodId>,
    ) {
        let Some(root) = self.program.method(family.root) else {
            self.program_error(format!(
                "virtual family {} root {} is not declared",
                family.id, family.root
            ));
            return;
        };
        if root.kind.receiver_access().is_none() {
            self.program_error(format!(
                "virtual family {} root {} is a static method",
                family.id, family.root
            ));
        }
        let mut family_members = HashSet::new();
        for member_id in &family.members {
            if !family_members.insert(*member_id) {
                self.program_error(format!(
                    "virtual family {} contains duplicate member {member_id}",
                    family.id
                ));
                continue;
            }
            if !all_members.insert(*member_id) {
                self.program_error(format!(
                    "virtual method {member_id} belongs to more than one family"
                ));
            }
            let Some(member) = self.program.method(*member_id) else {
                self.program_error(format!(
                    "virtual family {} member {member_id} is not declared",
                    family.id
                ));
                continue;
            };
            if member.kind.receiver_access().is_none() {
                self.program_error(format!(
                    "virtual family {} member {member_id} is a static method",
                    family.id
                ));
            }
            if *member_id != family.root
                && !self
                    .program
                    .is_ancestor(family.root.class(), member_id.class())
            {
                self.program_error(format!(
                    "virtual family {} member {member_id} is outside the root hierarchy",
                    family.id
                ));
            }
            if !same_signature(root, member) {
                self.program_error(format!(
                    "virtual family {} member {member_id} has a different signature or receiver access",
                    family.id
                ));
            }
        }
    }
}

fn same_signature(left: &MirMethodDeclaration, right: &MirMethodDeclaration) -> bool {
    left.kind.receiver_access().is_some()
        && left.kind.receiver_access() == right.kind.receiver_access()
        && left.parameters == right.parameters
        && left.return_type == right.return_type
}

//! Closed-world relations between exact objects and non-owning object views.

use crate::{hir::HirViewTarget, identity::ClassId, resolve::ResolvedProgram};

/// The compile-time outcome of asking whether one object source supplies a
/// target view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ObjectViewRelation {
    StaticSuccess,
    StaticFailure,
    Runtime,
}

/// The dynamic-class knowledge available at one object-view operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ObjectViewSource {
    /// Inline owning storage and inline subobjects have one exact class.
    ExactClass(ClassId),
    /// Forwarded aliases retain a static view and runtime dynamic metadata.
    Dynamic(HirViewTarget),
}

pub(super) fn classify_object_view_relation(
    program: &ResolvedProgram,
    source: ObjectViewSource,
    target: HirViewTarget,
) -> ObjectViewRelation {
    if target == HirViewTarget::Obj {
        return ObjectViewRelation::StaticSuccess;
    }
    if let ObjectViewSource::ExactClass(class) = source {
        return if class_provides_view(program, class, target) {
            ObjectViewRelation::StaticSuccess
        } else {
            ObjectViewRelation::StaticFailure
        };
    }

    let ObjectViewSource::Dynamic(source) = source else {
        unreachable!("exact object sources returned above");
    };
    if view_guarantees_target(program, source, target) {
        return ObjectViewRelation::StaticSuccess;
    }

    let mut any_success = false;
    let mut any_failure = false;
    for class in program.classes.iter().map(|class| class.id) {
        if !class_provides_view(program, class, source) {
            continue;
        }
        if class_provides_view(program, class, target) {
            any_success = true;
        } else {
            any_failure = true;
        }
    }

    match (any_success, any_failure) {
        (true, true) => ObjectViewRelation::Runtime,
        (true, false) => ObjectViewRelation::StaticSuccess,
        (false, _) => ObjectViewRelation::StaticFailure,
    }
}

pub(in crate::typeck) fn class_provides_view(
    program: &ResolvedProgram,
    class: ClassId,
    target: HirViewTarget,
) -> bool {
    match target {
        HirViewTarget::Class(target) => {
            program.hierarchy.is_subtype(class, target).unwrap_or(false)
        }
        HirViewTarget::Interface(interface) => class_conforms_to(program, class, interface),
        HirViewTarget::Obj => true,
    }
}

fn view_guarantees_target(
    program: &ResolvedProgram,
    source: HirViewTarget,
    target: HirViewTarget,
) -> bool {
    match (source, target) {
        (_, HirViewTarget::Obj) => true,
        (HirViewTarget::Class(class), target) => class_provides_view(program, class, target),
        (HirViewTarget::Interface(source), HirViewTarget::Interface(target)) => source == target,
        (HirViewTarget::Interface(_), HirViewTarget::Class(_))
        | (HirViewTarget::Obj, HirViewTarget::Class(_))
        | (HirViewTarget::Obj, HirViewTarget::Interface(_)) => false,
    }
}

fn class_conforms_to(
    program: &ResolvedProgram,
    class: ClassId,
    interface: crate::identity::InterfaceId,
) -> bool {
    std::iter::once(class)
        .chain(program.hierarchy.base_chain(class).into_iter().flatten())
        .any(|candidate| {
            program.class(candidate).is_some_and(|declaration| {
                declaration
                    .implemented_interfaces
                    .iter()
                    .any(|claim| claim.interface == interface)
            })
        })
}

#[cfg(test)]
mod tests;

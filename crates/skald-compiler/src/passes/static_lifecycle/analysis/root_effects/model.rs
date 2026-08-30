//! Analysis-owned lifecycle-root inventory and dependency derivation.

use std::collections::BTreeSet;

use crate::{
    identity::StaticFieldId,
    mir::{
        MirProgram, MirStaticFieldInitialization, MirStaticLifecycleDefinition,
        PreliminaryMirProgram, StaticEffectNode, StaticLifecycleAuthority,
    },
};

use super::super::roots::{destruction_roots, is_lifecycle_destination_or_published_self_parts};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum LifecycleRootKind {
    Initialization,
    Destruction,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct LifecycleRootUse {
    pub(super) owner: StaticFieldId,
    pub(super) kind: LifecycleRootKind,
    pub(super) node: StaticEffectNode,
}

pub(super) fn lifecycle_root_uses(program: &PreliminaryMirProgram) -> Vec<LifecycleRootUse> {
    let mut roots = Vec::new();
    for field in program.static_fields() {
        if let Some(initializer) = field.initializer {
            roots.push(LifecycleRootUse {
                owner: field.field,
                kind: LifecycleRootKind::Initialization,
                node: StaticEffectNode::callable(initializer.into()),
            });
        }
        roots.extend(
            destruction_roots(program.program(), field.ty)
                .into_iter()
                .map(|node| LifecycleRootUse {
                    owner: field.field,
                    kind: LifecycleRootKind::Destruction,
                    node,
                }),
        );
    }
    roots.sort_unstable();
    roots.dedup();
    roots
}

pub(super) fn lifecycle_root_uses_for_definitions(
    program: &MirProgram,
    definitions: &[MirStaticLifecycleDefinition],
) -> Vec<LifecycleRootUse> {
    let mut roots = Vec::new();
    for definition in definitions {
        if let MirStaticFieldInitialization::Explicit(initializer) = definition.initialization {
            roots.push(LifecycleRootUse {
                owner: definition.field,
                kind: LifecycleRootKind::Initialization,
                node: StaticEffectNode::callable(initializer.into()),
            });
        }
        roots.extend(
            destruction_roots(program, definition.ty)
                .into_iter()
                .map(|node| LifecycleRootUse {
                    owner: definition.field,
                    kind: LifecycleRootKind::Destruction,
                    node,
                }),
        );
    }
    roots.sort_unstable();
    roots.dedup();
    roots
}

#[cfg(test)]
pub(super) fn dependency_pairs(
    program: &PreliminaryMirProgram,
    authority: &StaticLifecycleAuthority,
) -> Result<BTreeSet<(StaticFieldId, StaticFieldId)>, StaticLifecycleRootEffectError> {
    dependency_pairs_for_roots(lifecycle_root_uses(program), authority)
}

pub(crate) fn dependency_pairs_for_definitions(
    program: &MirProgram,
    definitions: &[MirStaticLifecycleDefinition],
    authority: &StaticLifecycleAuthority,
) -> Result<BTreeSet<(StaticFieldId, StaticFieldId)>, StaticLifecycleRootEffectError> {
    dependency_pairs_for_roots(
        lifecycle_root_uses_for_definitions(program, definitions),
        authority,
    )
}

fn dependency_pairs_for_roots(
    roots: Vec<LifecycleRootUse>,
    authority: &StaticLifecycleAuthority,
) -> Result<BTreeSet<(StaticFieldId, StaticFieldId)>, StaticLifecycleRootEffectError> {
    let mut dependencies = BTreeSet::new();
    for root in roots {
        let summary = authority
            .root(root.node)
            .ok_or(StaticLifecycleRootEffectError::MissingRoot(root.node))?;
        for effect in summary.effects() {
            if root.kind == LifecycleRootKind::Initialization
                && is_lifecycle_destination_or_published_self_parts(
                    root.owner,
                    effect.target(),
                    effect.access(),
                    effect.phase(),
                    effect.is_lifecycle_owned(),
                )
            {
                continue;
            }
            dependencies.insert((effect.target(), root.owner));
        }
    }
    Ok(dependencies)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum StaticLifecycleRootEffectError {
    MissingRoot(StaticEffectNode),
    ForeignEdgeSource {
        node: StaticEffectNode,
        source: StaticEffectNode,
    },
    ForeignEdgeTarget {
        source: StaticEffectNode,
        target: StaticEffectNode,
    },
    ForeignStaticField {
        node: StaticEffectNode,
        field: StaticFieldId,
    },
}

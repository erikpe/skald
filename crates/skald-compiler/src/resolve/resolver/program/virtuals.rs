//! Virtual-root allocation and explicit override-family resolution.

use crate::{
    diagnostics::Diagnostic,
    identity::{ClassId, VirtualFamilyId, VirtualSlotId},
};

use super::*;

pub(super) fn resolve_virtual_families(
    ast: &syntax::CompilationUnit,
    work: &[ClassWorkItem],
    classes: &mut ResolvedClassDeclarationTable,
    class_symbols: &[ClassSymbols],
    hierarchy: &ResolvedClassHierarchy,
    diagnostics: &mut Diagnostics,
) -> ResolvedVirtualFamilyTable {
    let mut dispatch = classes
        .iter()
        .map(|class| vec![ResolvedMethodDispatch::Direct; class.methods.len()])
        .collect::<Vec<_>>();
    let mut families = Vec::new();

    for class in classes.iter() {
        for method in &class.methods {
            if matches!(method.modifier, ResolvedMethodModifier::Virtual { .. })
                && hierarchy.inherited_member(class.id, &method.name).is_none()
                && hierarchy.base_chain(class.id).is_some()
            {
                let family = VirtualFamilyId::new(families.len());
                let slot = VirtualSlotId::new(families.len());
                dispatch[class.id.index()][method.id.index()] =
                    ResolvedMethodDispatch::VirtualRoot { family, slot };
                families.push(ResolvedVirtualFamily {
                    id: family,
                    slot,
                    root: method.id,
                });
            }
        }
    }

    let mut states = vec![VisitState::Unvisited; classes.len()];
    for class in classes.iter() {
        resolve_class_overrides(class.id, classes, hierarchy, &mut dispatch, &mut states);
    }
    for class in classes.iter_mut() {
        for method in &mut class.methods {
            method.dispatch = dispatch[class.id.index()][method.id.index()];
        }
    }

    report_invalid_redeclarations(ast, work, classes, class_symbols, hierarchy, diagnostics);
    ResolvedVirtualFamilyTable::new(families)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum VisitState {
    Unvisited,
    Visiting,
    Complete,
}

fn resolve_class_overrides(
    class: ClassId,
    classes: &ResolvedClassDeclarationTable,
    hierarchy: &ResolvedClassHierarchy,
    dispatch: &mut [Vec<ResolvedMethodDispatch>],
    states: &mut [VisitState],
) {
    match states[class.index()] {
        VisitState::Complete | VisitState::Visiting => return,
        VisitState::Unvisited => states[class.index()] = VisitState::Visiting,
    }
    if hierarchy.base_chain(class).is_none() {
        states[class.index()] = VisitState::Complete;
        return;
    }
    if let Some(base) = hierarchy.direct_base(class) {
        resolve_class_overrides(base, classes, hierarchy, dispatch, states);
    }

    let declaration = classes.get(class).expect("hierarchy class must exist");
    for method in &declaration.methods {
        if !matches!(method.modifier, ResolvedMethodModifier::Override { .. }) {
            continue;
        }
        let Some(ResolvedClassMember::Method(overridden)) =
            hierarchy.inherited_member(class, &method.name)
        else {
            continue;
        };
        dispatch[class.index()][method.id.index()] =
            match dispatch[overridden.class().index()][overridden.index()] {
                ResolvedMethodDispatch::VirtualRoot { family, slot } => {
                    ResolvedMethodDispatch::Override {
                        family,
                        slot,
                        root: overridden,
                        overridden,
                    }
                }
                ResolvedMethodDispatch::Override {
                    family, slot, root, ..
                } => ResolvedMethodDispatch::Override {
                    family,
                    slot,
                    root,
                    overridden,
                },
                ResolvedMethodDispatch::Direct => ResolvedMethodDispatch::Direct,
            };
    }
    states[class.index()] = VisitState::Complete;
}

fn report_invalid_redeclarations(
    ast: &syntax::CompilationUnit,
    work: &[ClassWorkItem],
    classes: &ResolvedClassDeclarationTable,
    class_symbols: &[ClassSymbols],
    hierarchy: &ResolvedClassHierarchy,
    diagnostics: &mut Diagnostics,
) {
    for item in work {
        if hierarchy.base_chain(item.id).is_none() {
            continue;
        }
        let syntax::TopLevelDeclaration::Class(class) = &ast.declarations[item.ast_index] else {
            unreachable!("class work item must reference a class")
        };
        let symbols = &class_symbols[item.id.index()];

        for member in &class.members {
            let Some((name, name_span)) = ordinary_member_name(member) else {
                continue;
            };
            let Some(direct) = symbols
                .ordinary
                .get(&name.text)
                .filter(|symbol| symbol.name_span == name_span)
                .map(|symbol| resolved_member(symbol.kind))
            else {
                continue;
            };
            let inherited = hierarchy.inherited_member(item.id, &name.text);

            if let ResolvedClassMember::Method(method_id) = direct {
                let method = classes
                    .get(item.id)
                    .and_then(|class| class.method(method_id))
                    .expect("direct method symbol must have a declaration");
                if let ResolvedMethodModifier::Override { span } = method.modifier {
                    if matches!(method.dispatch, ResolvedMethodDispatch::Override { .. }) {
                        continue;
                    }
                    diagnostics.push(invalid_override_diagnostic(
                        classes, method, inherited, span,
                    ));
                    continue;
                }
            }

            if let Some(inherited) = inherited {
                diagnostics.push(inherited_collision_diagnostic(
                    classes, item.id, direct, inherited, &name.text, name_span,
                ));
            }
        }
    }
}

fn invalid_override_diagnostic(
    classes: &ResolvedClassDeclarationTable,
    method: &ResolvedMethodDeclaration,
    inherited: Option<ResolvedClassMember>,
    modifier_span: Span,
) -> Diagnostic {
    let mut diagnostic = match inherited {
        None => Diagnostic::error(
            INVALID_OVERRIDE,
            format!(
                "method `{}` is marked `override` but no inherited member has that name",
                method.name
            ),
        )
        .with_primary_label(modifier_span, "this override has no inherited target"),
        Some(ResolvedClassMember::Field(field)) => Diagnostic::error(
            INVALID_OVERRIDE,
            format!(
                "method `{}` cannot override an inherited field",
                method.name
            ),
        )
        .with_primary_label(modifier_span, "only virtual methods can be overridden")
        .with_secondary_label(
            member_name_span(classes, ResolvedClassMember::Field(field)),
            "inherited field declared here",
        ),
        Some(ResolvedClassMember::Method(overridden)) => Diagnostic::error(
            INVALID_OVERRIDE,
            format!(
                "method `{}` cannot override a non-virtual method",
                method.name
            ),
        )
        .with_primary_label(modifier_span, "the inherited method is not virtual")
        .with_secondary_label(
            member_name_span(classes, ResolvedClassMember::Method(overridden)),
            "inherited method declared here",
        ),
    };
    diagnostic = diagnostic.with_note(
        "an override must target the nearest inherited virtual method with the same name",
    );
    diagnostic
}

fn ordinary_member_name(member: &syntax::ClassMember) -> Option<(&syntax::Name, Span)> {
    match member {
        syntax::ClassMember::Field(field) => Some((&field.name, field.name.span)),
        syntax::ClassMember::Method(method) => Some((&method.name, method.name.span)),
        syntax::ClassMember::Initializer(_)
        | syntax::ClassMember::CopyConstructor(_)
        | syntax::ClassMember::CopyAssignment(_)
        | syntax::ClassMember::Destructor(_) => None,
    }
}

fn inherited_collision_diagnostic(
    classes: &ResolvedClassDeclarationTable,
    derived: ClassId,
    direct: ResolvedClassMember,
    inherited: ResolvedClassMember,
    name: &str,
    name_span: Span,
) -> Diagnostic {
    let derived_name = &classes.get(derived).expect("derived class must exist").name;
    let inherited_owner = inherited.declaring_class();
    let inherited_owner_name = &classes
        .get(inherited_owner)
        .expect("inherited member owner must exist")
        .name;

    Diagnostic::error(
        INHERITED_MEMBER_COLLISION,
        format!(
            "{} `{name}` in class `{derived_name}` conflicts with inherited {}",
            member_kind(direct),
            member_kind(inherited),
        ),
    )
    .with_primary_label(name_span, "redeclared in this derived class")
    .with_secondary_label(
        member_name_span(classes, inherited),
        format!(
            "inherited {} declared in class `{inherited_owner_name}`",
            member_kind(inherited)
        ),
    )
}

fn member_name_span(classes: &ResolvedClassDeclarationTable, member: ResolvedClassMember) -> Span {
    match member {
        ResolvedClassMember::Field(field) => {
            classes
                .get(field.class())
                .and_then(|class| class.field(field))
                .expect("inherited field must exist")
                .name_span
        }
        ResolvedClassMember::Method(method) => {
            classes
                .get(method.class())
                .and_then(|class| class.method(method))
                .expect("inherited method must exist")
                .name_span
        }
    }
}

const fn resolved_member(kind: OrdinaryMemberSymbolKind) -> ResolvedClassMember {
    match kind {
        OrdinaryMemberSymbolKind::Field(field) => ResolvedClassMember::Field(field),
        OrdinaryMemberSymbolKind::Method(method) => ResolvedClassMember::Method(method),
    }
}

const fn member_kind(member: ResolvedClassMember) -> &'static str {
    match member {
        ResolvedClassMember::Field(_) => "field",
        ResolvedClassMember::Method(_) => "method",
    }
}

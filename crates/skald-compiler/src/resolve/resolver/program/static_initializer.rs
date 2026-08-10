//! Delayed resolution of static declaration initializer expressions.

use super::*;

pub(super) struct ResolvedStaticInitializerUpdate {
    field: StaticFieldId,
    initializer: ResolvedStaticFieldInitializer,
}

pub(super) fn resolve_static_field_initializers(
    ast: &syntax::CompilationUnit,
    work_items: &[ClassWorkItem],
    classes: &ResolvedClassDeclarationTable,
    environment: BodyResolutionEnvironment<'_>,
    type_interner: &mut ResolvedTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedStaticInitializerUpdate> {
    let mut updates = Vec::new();
    for work in work_items {
        let syntax::TopLevelDeclaration::Class(class) = &ast.declarations[work.ast_index] else {
            continue;
        };
        for initializer_work in &work.static_initializer_members {
            let syntax::ClassMember::StaticField(field) =
                &class.members[initializer_work.member_index]
            else {
                continue;
            };
            let source = field
                .initializer
                .as_ref()
                .expect("static initializer work must reference an explicit initializer");
            let field_id = initializer_work.id.field();
            debug_assert!(classes
                .get(work.id)
                .and_then(|class| class.static_field(field_id))
                .is_some());

            let Some(expression) = resolve_static_initializer_expression(
                CallableResolutionContext::static_initializer(initializer_work.id.into(), work.id),
                &source.expression,
                environment,
                type_interner,
                diagnostics,
            ) else {
                continue;
            };
            updates.push(ResolvedStaticInitializerUpdate {
                field: field_id,
                initializer: ResolvedStaticFieldInitializer {
                    id: initializer_work.id,
                    equal_span: source.equal_span,
                    expression,
                    span: source.span,
                },
            });
        }
    }
    updates
}

pub(super) fn attach_static_field_initializers(
    classes: &mut ResolvedClassDeclarationTable,
    updates: Vec<ResolvedStaticInitializerUpdate>,
) {
    for update in updates {
        let field = classes
            .get_mut(update.field.class())
            .and_then(|class| class.static_field_mut(update.field))
            .expect("resolved static initializer must retain its declaration");
        debug_assert!(field.initializer.is_none());
        field.initializer = Some(update.initializer);
    }
}

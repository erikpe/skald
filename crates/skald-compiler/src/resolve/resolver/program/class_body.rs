//! Resolution of accepted class callable bodies.

use super::*;
use crate::resolve::resolver::body::BaseInitializationPolicy;

pub(super) fn resolve_class_bodies(
    ast: &syntax::CompilationUnit,
    work: &[ClassWorkItem],
    classes: &ResolvedClassDeclarationTable,
    environment: BodyResolutionEnvironment<'_>,
    array_types: &mut ArrayTypeInterner,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedClassDefinition> {
    let resolver = ClassBodyResolver {
        ast,
        classes,
        environment,
    };
    work.iter()
        .map(|item| resolver.resolve_class(item, array_types, diagnostics))
        .collect()
}

struct ClassBodyResolver<'program> {
    ast: &'program syntax::CompilationUnit,
    classes: &'program ResolvedClassDeclarationTable,
    environment: BodyResolutionEnvironment<'program>,
}

impl ClassBodyResolver<'_> {
    fn resolve_class(
        &self,
        item: &ClassWorkItem,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) -> ResolvedClassDefinition {
        let declaration = self
            .classes
            .get(item.id)
            .expect("class work and declaration table must agree");
        let syntax::TopLevelDeclaration::Class(class) = &self.ast.declarations[item.ast_index]
        else {
            unreachable!("class work item must reference a class")
        };

        let initializers = item
            .initializer_members
            .iter()
            .map(|work| {
                let syntax::ClassMember::Initializer(source) = &class.members[work.member_index]
                else {
                    unreachable!("initializer work must reference an initializer")
                };
                let metadata = declaration
                    .initializer(work.id)
                    .expect("accepted initializer work must retain its declaration identity");
                self.resolve_member(
                    CallableResolutionContext::member(
                        metadata.id.into(),
                        declaration
                            .direct_base
                            .map_or(BaseInitializationPolicy::Forbidden, |base| {
                                BaseInitializationPolicy::Required { base: base.class }
                            }),
                    ),
                    &metadata.parameters,
                    &source.body,
                    source.span,
                    array_types,
                    diagnostics,
                )
            })
            .collect();
        let copy_constructor = item.copy_constructor_member.map(|member_index| {
            let syntax::ClassMember::CopyConstructor(source) = &class.members[member_index] else {
                unreachable!("copy-constructor work must reference a copy constructor")
            };
            let metadata = declaration
                .copy_constructor_declaration
                .as_ref()
                .expect("accepted copy constructor must have declaration metadata");
            self.resolve_member(
                CallableResolutionContext::member(
                    metadata.id.into(),
                    BaseInitializationPolicy::Forbidden,
                ),
                &metadata.parameters,
                &source.body,
                source.span,
                array_types,
                diagnostics,
            )
        });
        let copy_assignment = item.copy_assignment_member.map(|member_index| {
            let syntax::ClassMember::CopyAssignment(source) = &class.members[member_index] else {
                unreachable!("copy-assignment work must reference copy assignment")
            };
            let metadata = declaration
                .copy_assignment_declaration
                .as_ref()
                .expect("accepted copy assignment must have declaration metadata");
            self.resolve_member(
                CallableResolutionContext::member(
                    metadata.id.into(),
                    BaseInitializationPolicy::Forbidden,
                ),
                std::slice::from_ref(&metadata.parameter),
                &source.body,
                source.span,
                array_types,
                diagnostics,
            )
        });
        let destructor = item.destructor_member.map(|member_index| {
            let syntax::ClassMember::Destructor(source) = &class.members[member_index] else {
                unreachable!("destructor work must reference a destructor")
            };
            let metadata = declaration
                .destructor
                .as_ref()
                .expect("accepted destructor must have declaration metadata");
            self.resolve_member(
                CallableResolutionContext::member(
                    metadata.id.into(),
                    BaseInitializationPolicy::Forbidden,
                ),
                &[],
                &source.body,
                source.span,
                array_types,
                diagnostics,
            )
        });
        let methods = item
            .method_members
            .iter()
            .enumerate()
            .map(|(method_index, member_index)| {
                let syntax::ClassMember::Method(source) = &class.members[*member_index] else {
                    unreachable!("method work must reference a method")
                };
                let metadata = &declaration.methods[method_index];
                self.resolve_member(
                    CallableResolutionContext::member(
                        metadata.id.into(),
                        BaseInitializationPolicy::Forbidden,
                    ),
                    &metadata.parameters,
                    &source.body,
                    source.span,
                    array_types,
                    diagnostics,
                )
            })
            .collect();

        ResolvedClassDefinition {
            class: item.id,
            initializers,
            copy_constructor,
            copy_assignment,
            destructor,
            methods,
            span: class.span,
        }
    }

    fn resolve_member(
        &self,
        context: CallableResolutionContext,
        parameters: &[ResolvedParameter],
        body: &syntax::Block,
        span: Span,
        array_types: &mut ArrayTypeInterner,
        diagnostics: &mut Diagnostics,
    ) -> ResolvedMemberDefinition {
        let callable = context.callable();
        let body = resolve_callable_body(
            context,
            parameters,
            body,
            self.environment,
            array_types,
            diagnostics,
        );
        ResolvedMemberDefinition {
            callable,
            locals: body.locals,
            body: body.body,
            span,
        }
    }
}

#[cfg(test)]
mod tests;

//! Resolution of accepted class callable bodies.

use super::*;
use crate::resolve::resolver::body::BaseInitializationPolicy;

pub(super) fn resolve_class_bodies(
    ast: &syntax::CompilationUnit,
    work: &[ClassWorkItem],
    classes: &ResolvedClassDeclarationTable,
    environment: BodyResolutionEnvironment<'_>,
    type_interner: &mut ResolvedTypeInterner,
    address_taken_callables: &mut ResolvedAddressTakenCallableTable,
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedClassDefinition> {
    let resolver = ClassBodyResolver {
        ast,
        classes,
        environment,
    };
    let mut state = ClassBodyResolutionState {
        type_interner,
        address_taken_callables,
        diagnostics,
    };
    work.iter()
        .map(|item| resolver.resolve_class(item, &mut state))
        .collect()
}

struct ClassBodyResolver<'program> {
    ast: &'program syntax::CompilationUnit,
    classes: &'program ResolvedClassDeclarationTable,
    environment: BodyResolutionEnvironment<'program>,
}

struct ClassBodyResolutionState<'state> {
    type_interner: &'state mut ResolvedTypeInterner,
    address_taken_callables: &'state mut ResolvedAddressTakenCallableTable,
    diagnostics: &'state mut Diagnostics,
}

impl ClassBodyResolver<'_> {
    fn resolve_class(
        &self,
        item: &ClassWorkItem,
        state: &mut ClassBodyResolutionState<'_>,
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
                    CallableResolutionContext::receiver_member(
                        metadata.id.into(),
                        declaration.id,
                        declaration
                            .direct_base
                            .map_or(BaseInitializationPolicy::Forbidden, |base| {
                                BaseInitializationPolicy::Required { base: base.class }
                            }),
                    ),
                    &metadata.parameters,
                    &source.body,
                    source.span,
                    state,
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
                CallableResolutionContext::receiver_member(
                    metadata.id.into(),
                    declaration.id,
                    BaseInitializationPolicy::Forbidden,
                ),
                &metadata.parameters,
                &source.body,
                source.span,
                state,
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
                CallableResolutionContext::receiver_member(
                    metadata.id.into(),
                    declaration.id,
                    BaseInitializationPolicy::Forbidden,
                ),
                std::slice::from_ref(&metadata.parameter),
                &source.body,
                source.span,
                state,
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
                CallableResolutionContext::receiver_member(
                    metadata.id.into(),
                    declaration.id,
                    BaseInitializationPolicy::Forbidden,
                ),
                &[],
                &source.body,
                source.span,
                state,
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
                        declaration.id,
                        metadata.kind.receiver_access().map(|_| declaration.id),
                        BaseInitializationPolicy::Forbidden,
                    ),
                    &metadata.parameters,
                    &source.body,
                    source.span,
                    state,
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
        state: &mut ClassBodyResolutionState<'_>,
    ) -> ResolvedMemberDefinition {
        let callable = context.callable();
        let body = resolve_callable_body(
            context,
            parameters,
            body,
            self.environment,
            state.type_interner,
            state.address_taken_callables,
            state.diagnostics,
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

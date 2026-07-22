//! Resolution of accepted class callable bodies.

use super::*;
use crate::identity::CallableId;

pub(super) fn resolve_class_bodies(
    ast: &syntax::CompilationUnit,
    top_levels: &HashMap<String, TopLevelSymbol>,
    work: &[ClassWorkItem],
    classes: &ResolvedClassDeclarationTable,
    class_symbols: &[ClassSymbols],
    diagnostics: &mut Diagnostics,
) -> Vec<ResolvedClassDefinition> {
    let resolver = ClassBodyResolver {
        ast,
        classes,
        environment: BodyResolutionEnvironment::new(top_levels, classes, class_symbols),
    };
    work.iter()
        .map(|item| resolver.resolve_class(item, diagnostics))
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

        let initializer = item.initializer_member.map(|member_index| {
            let syntax::ClassMember::Initializer(source) = &class.members[member_index] else {
                unreachable!("initializer work must reference an initializer")
            };
            let metadata = declaration
                .initializer
                .as_ref()
                .expect("accepted initializer must have declaration metadata");
            self.resolve_member(
                metadata.id.into(),
                item.id,
                &metadata.parameters,
                &source.body,
                source.span,
                diagnostics,
            )
        });
        let copy_constructor = item.copy_constructor_member.map(|member_index| {
            let syntax::ClassMember::Initializer(source) = &class.members[member_index] else {
                unreachable!("copy-constructor work must reference an initializer")
            };
            let metadata = declaration
                .copy_constructor_declaration
                .as_ref()
                .expect("accepted copy constructor must have declaration metadata");
            self.resolve_member(
                metadata.id.into(),
                item.id,
                &metadata.parameters,
                &source.body,
                source.span,
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
                metadata.id.into(),
                item.id,
                std::slice::from_ref(&metadata.parameter),
                &source.body,
                source.span,
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
                metadata.id.into(),
                item.id,
                &[],
                &source.body,
                source.span,
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
                    metadata.id.into(),
                    item.id,
                    &metadata.parameters,
                    &source.body,
                    source.span,
                    diagnostics,
                )
            })
            .collect();

        ResolvedClassDefinition {
            class: item.id,
            initializer,
            copy_constructor,
            copy_assignment,
            destructor,
            methods,
            span: class.span,
        }
    }

    fn resolve_member(
        &self,
        callable: CallableId,
        owner: ClassId,
        parameters: &[ResolvedParameter],
        body: &syntax::Block,
        span: Span,
        diagnostics: &mut Diagnostics,
    ) -> ResolvedMemberDefinition {
        let body = resolve_callable_body(
            callable,
            Some(owner),
            parameters,
            body,
            self.environment,
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

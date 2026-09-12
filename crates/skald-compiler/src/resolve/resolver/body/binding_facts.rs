//! Callable-local type facts indexed by resolved binding identity.

use super::*;

#[derive(Clone, Copy)]
struct BindingTypeFact {
    binding: BindingId,
    ty: ResolvedTypeKind,
}

pub(super) struct BindingTypeFacts {
    callable: CallableId,
    parameters: Vec<BindingTypeFact>,
    locals: Vec<ResolvedLocal>,
}

impl BindingTypeFacts {
    pub(super) fn new(callable: CallableId, parameters: &[ResolvedParameter]) -> Self {
        Self {
            callable,
            parameters: parameters
                .iter()
                .map(|parameter| BindingTypeFact {
                    binding: BindingId::Parameter(parameter.id),
                    ty: parameter.type_syntax.kind,
                })
                .collect(),
            locals: Vec::new(),
        }
    }

    pub(super) fn get(&self, binding: BindingId) -> Option<ResolvedTypeKind> {
        if binding.callable() != self.callable {
            return None;
        }
        match binding {
            BindingId::Receiver(_) => None,
            BindingId::Parameter(parameter) => self
                .parameters
                .get(parameter.index())
                .filter(|fact| fact.binding == binding)
                .map(|fact| fact.ty),
            BindingId::Local(local) => self
                .locals
                .get(local.index())
                .filter(|fact| BindingId::Local(fact.id) == binding)
                .map(|fact| fact.type_syntax.kind),
        }
    }

    pub(super) fn record_local(&mut self, local: &ResolvedLocal) {
        let binding = BindingId::Local(local.id);
        assert_eq!(
            binding.callable(),
            self.callable,
            "local type facts must belong to their callable"
        );
        assert_eq!(
            local.id.index(),
            self.locals.len(),
            "local type facts must follow dense identity order"
        );
        self.locals.push(local.clone());
    }

    pub(super) fn local_count(&self) -> usize {
        self.locals.len()
    }

    pub(super) fn into_locals(self) -> Vec<ResolvedLocal> {
        self.locals
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{FunctionId, LocalId, ParameterId};

    fn span() -> Span {
        let mut sources = crate::source::SourceDatabase::new();
        Span::empty(sources.add("binding_facts.ska", ""), 0)
    }

    fn parameter(callable: CallableId, index: usize, ty: ResolvedTypeKind) -> ResolvedParameter {
        let span = span();
        ResolvedParameter {
            id: ParameterId::new(callable, index),
            binding_mode: ResolvedParameterBindingMode::Value,
            name: format!("parameter_{index}"),
            name_span: span,
            type_syntax: ResolvedType { kind: ty, span },
            span,
        }
    }

    fn local(callable: CallableId, index: usize, ty: ResolvedTypeKind) -> ResolvedLocal {
        let span = span();
        ResolvedLocal {
            id: LocalId::new(callable, index),
            name: format!("local_{index}"),
            name_span: span,
            type_syntax: ResolvedType { kind: ty, span },
            span,
        }
    }

    #[test]
    fn indexes_callable_local_binding_types_by_identity() {
        let callable = CallableId::Function(FunctionId::new(1));
        let parameters = [
            parameter(callable, 0, ResolvedTypeKind::I64),
            parameter(callable, 1, ResolvedTypeKind::Bool),
        ];
        let mut facts = BindingTypeFacts::new(callable, &parameters);
        facts.record_local(&local(callable, 0, ResolvedTypeKind::U8));

        assert_eq!(
            facts.get(BindingId::Parameter(parameters[0].id)),
            Some(ResolvedTypeKind::I64)
        );
        assert_eq!(
            facts.get(BindingId::Parameter(parameters[1].id)),
            Some(ResolvedTypeKind::Bool)
        );
        assert_eq!(
            facts.get(BindingId::Local(LocalId::new(callable, 0))),
            Some(ResolvedTypeKind::U8)
        );
    }

    #[test]
    fn rejects_receiver_foreign_callable_and_out_of_range_identities() {
        let callable = CallableId::Function(FunctionId::new(1));
        let foreign = CallableId::Function(FunctionId::new(2));
        let parameters = [parameter(callable, 0, ResolvedTypeKind::I64)];
        let facts = BindingTypeFacts::new(callable, &parameters);

        assert_eq!(facts.get(BindingId::Receiver(callable)), None);
        assert_eq!(
            facts.get(BindingId::Parameter(ParameterId::new(foreign, 0))),
            None
        );
        assert_eq!(
            facts.get(BindingId::Parameter(ParameterId::new(callable, 1))),
            None
        );
        assert_eq!(facts.get(BindingId::Local(LocalId::new(callable, 0))), None);
    }

    #[test]
    fn rejects_a_same_callable_identity_stored_at_the_wrong_index() {
        let callable = CallableId::Function(FunctionId::new(1));
        let parameters = [parameter(callable, 1, ResolvedTypeKind::I64)];
        let facts = BindingTypeFacts::new(callable, &parameters);

        assert_eq!(
            facts.get(BindingId::Parameter(ParameterId::new(callable, 0))),
            None
        );
        assert_eq!(
            facts.get(BindingId::Parameter(ParameterId::new(callable, 1))),
            None
        );
    }
}

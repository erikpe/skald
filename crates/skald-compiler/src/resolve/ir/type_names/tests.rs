use crate::{
    identity::{
        ArrayTypeId, ClassId, ClassTemplateId, FunctionTypeId, InterfaceId, OptionalBoxTypeId,
        OptionalTypeId,
    },
    source::{SourceDatabase, Span},
};

use super::*;
use crate::resolve::ir::{
    ResolvedFunctionTypeParameter, ResolvedFunctionTypeParameterMode, ResolvedType,
};

#[derive(Default)]
struct TestContext {
    arrays: Vec<ResolvedArrayType>,
    functions: Vec<ResolvedFunctionType>,
    optionals: Vec<ResolvedOptionalType>,
    optional_boxes: Vec<ResolvedOptionalBoxType>,
    classes: Vec<(ClassId, String)>,
    specializations: Vec<(ClassId, GenericClassInstanceKey)>,
    templates: Vec<(ClassTemplateId, String)>,
    interfaces: Vec<(InterfaceId, String)>,
}

impl ResolvedTypeNameContext for TestContext {
    fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.arrays.get(id.index())
    }

    fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.functions.get(id.index())
    }

    fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.optionals.get(id.index())
    }

    fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.optional_boxes.get(id.index())
    }

    fn direct_class_name(&self, id: ClassId) -> Option<String> {
        self.classes
            .iter()
            .find(|(class, _)| *class == id)
            .map(|(_, name)| name.clone())
    }

    fn class_specialization(&self, id: ClassId) -> Option<&GenericClassInstanceKey> {
        self.specializations
            .iter()
            .find(|(class, _)| *class == id)
            .map(|(_, key)| key)
    }

    fn template_name(&self, id: ClassTemplateId) -> Option<String> {
        self.templates
            .iter()
            .find(|(template, _)| *template == id)
            .map(|(_, name)| name.clone())
    }

    fn interface_name(&self, id: InterfaceId) -> Option<String> {
        self.interfaces
            .iter()
            .find(|(interface, _)| *interface == id)
            .map(|(_, name)| name.clone())
    }
}

#[test]
fn renders_recursive_source_shape_with_modes_and_postfix_grouping() {
    let item = ClassId::new(0);
    let view = InterfaceId::new(0);
    let array = ArrayTypeId::new(0);
    let optional = OptionalTypeId::new(0);
    let function = FunctionTypeId::new(0);
    let context = TestContext {
        arrays: vec![ResolvedArrayType {
            id: array,
            element: resolved(ResolvedTypeKind::I64),
        }],
        functions: vec![ResolvedFunctionType {
            id: function,
            parameters: vec![
                parameter(
                    ResolvedFunctionTypeParameterMode::ReadOnlyAlias,
                    ResolvedTypeKind::Class(item),
                ),
                parameter(
                    ResolvedFunctionTypeParameterMode::MutableAlias,
                    ResolvedTypeKind::Array(array),
                ),
            ],
            result: resolved(ResolvedTypeKind::Optional(optional)),
            span: span(),
        }],
        optionals: vec![ResolvedOptionalType {
            id: optional,
            payload: resolved(ResolvedTypeKind::Shared(ResolvedSharedTarget::Interface(
                view,
            ))),
        }],
        classes: vec![(item, "model::Item".to_owned())],
        interfaces: vec![(view, "model::View".to_owned())],
        ..TestContext::default()
    };

    assert_eq!(
        ResolvedTypeNameRenderer::new(&context).render(ResolvedTypeKind::Function(function)),
        "fn(ref model::Item, mut ref i64[]) -> (shared model::View)?"
    );
}

#[test]
fn retains_the_class_cycle_guard_through_nested_function_arguments() {
    let class = ClassId::new(0);
    let template = ClassTemplateId::new(0);
    let function = FunctionTypeId::new(0);
    let context = TestContext {
        functions: vec![ResolvedFunctionType {
            id: function,
            parameters: vec![parameter(
                ResolvedFunctionTypeParameterMode::Value,
                ResolvedTypeKind::Class(class),
            )],
            result: resolved(ResolvedTypeKind::Unit),
            span: span(),
        }],
        specializations: vec![(
            class,
            GenericClassInstanceKey {
                template,
                arguments: vec![ResolvedTypeKind::Function(function)],
            },
        )],
        templates: vec![(template, "Node".to_owned())],
        ..TestContext::default()
    };

    assert_eq!(
        ResolvedTypeNameRenderer::new(&context).render(ResolvedTypeKind::Class(class)),
        "Node<fn(c0) -> unit>"
    );
}

#[test]
fn renders_exact_and_view_only_shared_optional_boxes() {
    let item = ClassId::new(0);
    let view = InterfaceId::new(0);
    let optional = OptionalTypeId::new(0);
    let exact_box = OptionalBoxTypeId::new(0);
    let view_box = OptionalBoxTypeId::new(1);
    let context = TestContext {
        optionals: vec![ResolvedOptionalType {
            id: optional,
            payload: resolved(ResolvedTypeKind::Class(item)),
        }],
        optional_boxes: vec![
            ResolvedOptionalBoxType {
                id: exact_box,
                optional: Some(optional),
                optional_depth: 1,
                object_leaf: Some(ResolvedObjectTarget::Class(item)),
                span: span(),
            },
            ResolvedOptionalBoxType {
                id: view_box,
                optional: None,
                optional_depth: 2,
                object_leaf: Some(ResolvedObjectTarget::Interface(view)),
                span: span(),
            },
        ],
        classes: vec![(item, "Item".to_owned())],
        interfaces: vec![(view, "View".to_owned())],
        ..TestContext::default()
    };

    assert_eq!(
        ResolvedTypeNameRenderer::new(&context).render(ResolvedTypeKind::Shared(
            ResolvedSharedTarget::OptionalBox(exact_box,)
        )),
        "shared Item?"
    );
    assert_eq!(
        ResolvedTypeNameRenderer::new(&context).render(ResolvedTypeKind::Shared(
            ResolvedSharedTarget::OptionalBox(view_box,)
        )),
        "shared View??"
    );
}

fn parameter(
    mode: ResolvedFunctionTypeParameterMode,
    kind: ResolvedTypeKind,
) -> ResolvedFunctionTypeParameter {
    ResolvedFunctionTypeParameter {
        mode,
        type_syntax: resolved(kind),
        span: span(),
    }
}

fn resolved(kind: ResolvedTypeKind) -> ResolvedType {
    ResolvedType { kind, span: span() }
}

fn span() -> Span {
    let mut sources = SourceDatabase::new();
    Span::empty(sources.add("type-names.ska", ""), 0)
}

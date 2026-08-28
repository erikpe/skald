//! Deterministic bottom-up interning of recursive resolved value types.

use std::collections::HashMap;

use crate::{
    identity::{ArrayTypeId, FunctionTypeId, OptionalBoxTypeId, OptionalTypeId},
    source::Span,
};

use super::{
    ResolvedArrayType, ResolvedArrayTypeTable, ResolvedFunctionType, ResolvedFunctionTypeParameter,
    ResolvedFunctionTypeParameterMode, ResolvedFunctionTypeTable, ResolvedObjectTarget,
    ResolvedOptionalBoxType, ResolvedOptionalBoxTypeTable, ResolvedOptionalType,
    ResolvedOptionalTypeTable, ResolvedParameter, ResolvedParameterBindingMode, ResolvedType,
    ResolvedTypeKind,
};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct FunctionTypeKey {
    parameters: Vec<(ResolvedFunctionTypeParameterMode, ResolvedTypeKind)>,
    result: ResolvedTypeKind,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct OptionalBoxKey {
    optional: Option<OptionalTypeId>,
    optional_depth: usize,
    object_leaf: Option<ResolvedObjectTarget>,
}

/// Owns canonical identities for the mutually recursive array/optional graph.
///
/// Resolution interns children before parents. `ResolvedTypeKind` contains
/// only semantic identities, so source spans never affect equality or keys.
#[derive(Clone, Default)]
pub(super) struct ResolvedTypeInterner {
    function_ids: HashMap<FunctionTypeKey, FunctionTypeId>,
    functions: Vec<ResolvedFunctionType>,
    array_ids: HashMap<ResolvedTypeKind, ArrayTypeId>,
    arrays: Vec<ResolvedArrayType>,
    optional_ids: HashMap<ResolvedTypeKind, OptionalTypeId>,
    optionals: Vec<ResolvedOptionalType>,
    optional_box_ids: HashMap<OptionalBoxKey, OptionalBoxTypeId>,
    optional_boxes: Vec<ResolvedOptionalBoxType>,
}

impl ResolvedTypeInterner {
    pub(super) fn intern_callable_signature(
        &mut self,
        parameters: &[ResolvedParameter],
        result: &ResolvedType,
        span: Span,
    ) -> FunctionTypeId {
        let parameters = parameters
            .iter()
            .map(|parameter| ResolvedFunctionTypeParameter {
                mode: match parameter.binding_mode {
                    ResolvedParameterBindingMode::Value => ResolvedFunctionTypeParameterMode::Value,
                    ResolvedParameterBindingMode::ReadOnlyAlias { .. } => {
                        ResolvedFunctionTypeParameterMode::ReadOnlyAlias
                    }
                    ResolvedParameterBindingMode::MutableAlias { .. } => {
                        ResolvedFunctionTypeParameterMode::MutableAlias
                    }
                },
                type_syntax: parameter.type_syntax.clone(),
                span: parameter.span,
            })
            .collect();
        self.intern_function(parameters, result.clone(), span)
    }

    pub(super) fn intern_function(
        &mut self,
        parameters: Vec<ResolvedFunctionTypeParameter>,
        result: ResolvedType,
        span: Span,
    ) -> FunctionTypeId {
        let key = FunctionTypeKey {
            parameters: parameters
                .iter()
                .map(|parameter| (parameter.mode, parameter.type_syntax.kind))
                .collect(),
            result: result.kind,
        };
        if let Some(id) = self.function_ids.get(&key) {
            return *id;
        }

        let id = FunctionTypeId::new(self.functions.len());
        self.function_ids.insert(key, id);
        self.functions.push(ResolvedFunctionType {
            id,
            parameters,
            result,
            span,
        });
        id
    }

    pub(super) fn function(&self, id: FunctionTypeId) -> Option<&ResolvedFunctionType> {
        self.functions
            .get(id.index())
            .filter(|entry| entry.id == id)
    }

    pub(super) fn intern_array(&mut self, element: ResolvedType) -> ArrayTypeId {
        if let Some(id) = self.array_ids.get(&element.kind) {
            return *id;
        }

        let id = ArrayTypeId::new(self.arrays.len());
        self.array_ids.insert(element.kind, id);
        self.arrays.push(ResolvedArrayType { id, element });
        id
    }

    pub(super) fn intern_optional(&mut self, payload: ResolvedType) -> OptionalTypeId {
        if let Some(id) = self.optional_ids.get(&payload.kind) {
            return *id;
        }

        let id = OptionalTypeId::new(self.optionals.len());
        self.optional_ids.insert(payload.kind, id);
        self.optionals.push(ResolvedOptionalType { id, payload });
        id
    }

    pub(super) fn array(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.arrays.get(id.index()).filter(|entry| entry.id == id)
    }

    pub(super) fn optional(&self, id: OptionalTypeId) -> Option<&ResolvedOptionalType> {
        self.optionals
            .get(id.index())
            .filter(|entry| entry.id == id)
    }

    pub(super) fn intern_optional_box(
        &mut self,
        optional: OptionalTypeId,
        span: Span,
    ) -> OptionalBoxTypeId {
        let (optional_depth, object_leaf) = self.optional_leaf(optional);
        self.intern_optional_box_key(
            OptionalBoxKey {
                optional: Some(optional),
                optional_depth,
                object_leaf,
            },
            span,
        )
    }

    pub(super) fn intern_optional_object_box_view(
        &mut self,
        optional_depth: usize,
        object_leaf: ResolvedObjectTarget,
        span: Span,
    ) -> OptionalBoxTypeId {
        self.intern_optional_box_key(
            OptionalBoxKey {
                optional: None,
                optional_depth,
                object_leaf: Some(object_leaf),
            },
            span,
        )
    }

    pub(super) fn intern_optional_object_box_cast_target(
        &mut self,
        optional_depth: usize,
        object_leaf: ResolvedObjectTarget,
        span: Span,
    ) -> OptionalBoxTypeId {
        if let ResolvedObjectTarget::Class(class) = object_leaf {
            let mut payload = ResolvedType {
                kind: ResolvedTypeKind::Class(class),
                span,
            };
            let mut outer = None;
            for _ in 0..optional_depth {
                let optional = self.intern_optional(payload);
                outer = Some(optional);
                payload = ResolvedType {
                    kind: ResolvedTypeKind::Optional(optional),
                    span,
                };
            }
            return self.intern_optional_box(
                outer.expect("an optional-box cast target has at least one optional layer"),
                span,
            );
        }
        self.intern_optional_object_box_view(optional_depth, object_leaf, span)
    }

    fn intern_optional_box_key(&mut self, key: OptionalBoxKey, span: Span) -> OptionalBoxTypeId {
        if let Some(id) = self.optional_box_ids.get(&key) {
            return *id;
        }

        let id = OptionalBoxTypeId::new(self.optional_boxes.len());
        self.optional_box_ids.insert(key, id);
        self.optional_boxes.push(ResolvedOptionalBoxType {
            id,
            optional: key.optional,
            optional_depth: key.optional_depth,
            object_leaf: key.object_leaf,
            span,
        });
        id
    }

    pub(super) fn optional_box(&self, id: OptionalBoxTypeId) -> Option<&ResolvedOptionalBoxType> {
        self.optional_boxes
            .get(id.index())
            .filter(|entry| entry.id == id)
    }

    fn optional_leaf(&self, optional: OptionalTypeId) -> (usize, Option<ResolvedObjectTarget>) {
        let mut depth = 1usize;
        let mut leaf = self
            .optional(optional)
            .expect("interned optional identity must exist")
            .payload
            .kind;
        while let ResolvedTypeKind::Optional(nested) = leaf {
            depth += 1;
            leaf = self
                .optional(nested)
                .expect("nested optional identity must exist")
                .payload
                .kind;
        }
        let object_leaf = match leaf {
            ResolvedTypeKind::Obj => Some(ResolvedObjectTarget::Obj),
            ResolvedTypeKind::Class(class) => Some(ResolvedObjectTarget::Class(class)),
            ResolvedTypeKind::Interface(interface) => {
                Some(ResolvedObjectTarget::Interface(interface))
            }
            ResolvedTypeKind::I64
            | ResolvedTypeKind::U64
            | ResolvedTypeKind::U8
            | ResolvedTypeKind::F64
            | ResolvedTypeKind::Bool
            | ResolvedTypeKind::Unit
            | ResolvedTypeKind::Function(_)
            | ResolvedTypeKind::Array(_)
            | ResolvedTypeKind::Shared(_) => None,
            ResolvedTypeKind::Optional(_) => unreachable!("optional leaf traversal is complete"),
        };
        (depth, object_leaf)
    }

    pub(super) fn finish(
        self,
    ) -> (
        ResolvedArrayTypeTable,
        ResolvedFunctionTypeTable,
        ResolvedOptionalTypeTable,
        ResolvedOptionalBoxTypeTable,
    ) {
        (
            ResolvedArrayTypeTable::new(self.arrays),
            ResolvedFunctionTypeTable::new(self.functions),
            ResolvedOptionalTypeTable::new(self.optionals),
            ResolvedOptionalBoxTypeTable::new(self.optional_boxes),
        )
    }
}

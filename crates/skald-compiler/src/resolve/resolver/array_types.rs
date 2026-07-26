//! Deterministic interning of exact recursive array element types.

use std::collections::HashMap;

use crate::identity::{ArrayTypeId, ClassId, InterfaceId};

use super::{
    ResolvedArrayType, ResolvedArrayTypeTable, ResolvedOptionalPayload, ResolvedSharedTarget,
    ResolvedType, ResolvedTypeKind,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum TypeKey {
    I64,
    U64,
    U8,
    F64,
    Bool,
    Unit,
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(ArrayTypeId),
    Shared(SharedTargetKey),
    Optional(ResolvedOptionalPayload),
    OptionalShared(SharedTargetKey),
}

impl TypeKey {
    fn from_resolved(kind: ResolvedTypeKind) -> Self {
        match kind {
            ResolvedTypeKind::I64 => Self::I64,
            ResolvedTypeKind::U64 => Self::U64,
            ResolvedTypeKind::U8 => Self::U8,
            ResolvedTypeKind::F64 => Self::F64,
            ResolvedTypeKind::Bool => Self::Bool,
            ResolvedTypeKind::Unit => Self::Unit,
            ResolvedTypeKind::Obj => Self::Obj,
            ResolvedTypeKind::Class(class) => Self::Class(class),
            ResolvedTypeKind::Interface(interface) => Self::Interface(interface),
            ResolvedTypeKind::Array(array) => Self::Array(array),
            ResolvedTypeKind::Shared(target) => Self::Shared(target.into()),
            ResolvedTypeKind::Optional { payload, .. } => Self::Optional(payload),
            ResolvedTypeKind::OptionalShared { target, .. } => Self::OptionalShared(target.into()),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum SharedTargetKey {
    Obj,
    Class(ClassId),
    Interface(InterfaceId),
    Array(ArrayTypeId),
}

impl From<ResolvedSharedTarget> for SharedTargetKey {
    fn from(target: ResolvedSharedTarget) -> Self {
        match target {
            ResolvedSharedTarget::Obj => Self::Obj,
            ResolvedSharedTarget::Class(class) => Self::Class(class),
            ResolvedSharedTarget::Interface(interface) => Self::Interface(interface),
            ResolvedSharedTarget::Array(array) => Self::Array(array),
        }
    }
}

#[derive(Default)]
pub(super) struct ArrayTypeInterner {
    ids: HashMap<TypeKey, ArrayTypeId>,
    entries: Vec<ResolvedArrayType>,
}

impl ArrayTypeInterner {
    pub(super) fn intern(&mut self, element: ResolvedType) -> ArrayTypeId {
        let key = TypeKey::from_resolved(element.kind);
        if let Some(id) = self.ids.get(&key) {
            return *id;
        }

        let id = ArrayTypeId::new(self.entries.len());
        self.ids.insert(key, id);
        self.entries.push(ResolvedArrayType { id, element });
        id
    }

    pub(super) fn finish(self) -> ResolvedArrayTypeTable {
        ResolvedArrayTypeTable::new(self.entries)
    }

    pub(super) fn get(&self, id: ArrayTypeId) -> Option<&ResolvedArrayType> {
        self.entries.get(id.index()).filter(|entry| entry.id == id)
    }
}

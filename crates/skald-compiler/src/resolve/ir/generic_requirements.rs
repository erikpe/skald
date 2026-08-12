//! Contextual mechanical requirements inferred for class templates.

use crate::source::Span;

use super::ResolvedTemplateType;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenericAliasAccess {
    ReadOnly,
    Mutable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenericCapability {
    FieldStorage,
    StaticStorage,
    ValueParameter,
    ValueResult,
    AliasTarget(GenericAliasAccess),
    OptionalPayload,
    ArrayElement,
    SharedTarget,
    DefaultConstructible,
    CopyConstructible,
    Assignable,
    Destroyable,
}

/// Stable source-level explanation for why a template needs a capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GenericRequirementReason {
    FieldDeclaration { member: usize },
    StaticFieldDeclaration { member: usize },
    ParameterDeclaration { member: usize, parameter: usize },
    MethodResult { member: usize },
    OptionalType,
    ArrayType,
    SharedType,
    StaticZeroInitialization { member: usize },
    ArrayLengthConstruction { member: usize },
    ExplicitArrayCopy { member: usize },
    ExplicitCopyConstruction { member: usize },
    StoredInitializationCopy { member: usize },
    Assignment { member: usize },
    SynthesizedDestruction { member: usize },
}

/// One inferred obligation over a structural template type term.
///
/// The term deliberately remains structural. In particular, requirements on
/// `T`, `T?`, and `T?[]` are different records and are closed only when a
/// specialization substitutes its arguments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GenericRequirement {
    pub(crate) type_term: ResolvedTemplateType,
    pub(crate) capability: GenericCapability,
    pub(crate) origin: Span,
    pub(crate) reason: GenericRequirementReason,
}

//! Definition-site resolution for non-executable generic class templates.

mod body;
mod bounds;
mod collection;
mod interface_resolution;
mod interface_validation;
mod requirements;
mod resolution;
mod type_resolution;

pub(super) use collection::{
    collect_generic_templates, ClassTemplateWorkItem, CollectedGenericTemplates,
    InterfaceTemplateWorkItem,
};
pub(super) use interface_resolution::resolve_interface_template_semantics;
pub(super) use resolution::{resolve_class_template_semantics, TemplateInterfaceEnvironment};

use crate::resolve::{
    ir::{
        ResolvedClassTemplateSemantics, ResolvedTemplateBound, ResolvedTemplateBoundRequirement,
        ResolvedTemplateConstructionMode, ResolvedTemplateDependentSelectionKind,
        ResolvedTemplateOperatorSelection, ResolvedTemplateOperatorSyntax,
        ResolvedTemplateSelection, ResolvedTemplateTypeUse, ResolvedTemplateTypeUseContext,
    },
    resolver::{
        name_lookup::{ModuleLookup, TopLevelLookup},
        TopLevelSymbol, TopLevelSymbolKind, AMBIGUOUS_GENERIC_BOUND_MEMBER,
        AMBIGUOUS_GENERIC_OPERATOR_APPLICATION, AMBIGUOUS_ITERABLE_APPLICATION, DUPLICATE_BINDING,
        DUPLICATE_GENERIC_BOUND, DUPLICATE_TYPE_PARAMETER, GENERIC_ARITY_MISMATCH,
        INCOMPATIBLE_GENERIC_OPERATOR_RHS, INVALID_CALL_TARGET, INVALID_GENERIC_APPLICATION,
        INVALID_GENERIC_BASE, INVALID_GENERIC_BOUND, INVALID_GENERIC_INTERFACE_REQUIREMENT,
        INVALID_INTERFACE_CLAIM, ITERATION_ITEM_TYPE_MISMATCH, MISSING_ITERABLE_APPLICATION,
        RAW_GENERIC_TYPE, TOP_LEVEL_USED_AS_VALUE, UNCONSTRAINED_TYPE_PARAMETER_MEMBER,
        UNKNOWN_MEMBER, UNKNOWN_NAME, UNKNOWN_TYPE, UNSUPPORTED_GENERIC_OPERATOR_APPLICATION,
        UNSUPPORTED_PARAMETER_CONSTRUCTION,
    },
};
use crate::{
    diagnostics::{Diagnostic, Diagnostics},
    identity::{ClassTemplateId, InterfaceTemplateId},
    resolve::{
        CanonicalOperatorProtocol, CanonicalOperatorProtocolShape, GenericAliasAccess,
        GenericCapability, GenericRequirement, GenericRequirementReason, ResolvedBinaryOperator,
        ResolvedClassTemplate, ResolvedClassTemplateTable, ResolvedFunctionTypeParameterMode,
        ResolvedInterfaceClaim, ResolvedInterfaceDeclarationTable, ResolvedInterfaceTemplate,
        ResolvedInterfaceTemplateBound, ResolvedInterfaceTemplateParameter,
        ResolvedInterfaceTemplateRequirement, ResolvedInterfaceTemplateRequirementSignature,
        ResolvedInterfaceTemplateSemanticTable, ResolvedInterfaceTemplateSemantics,
        ResolvedInterfaceTemplateTable, ResolvedInterfaceTemplateTypeUse,
        ResolvedInterfaceTemplateTypeUseContext, ResolvedInterfaceType,
        ResolvedIterableLanguageItem, ResolvedOperatorLanguageItem,
        ResolvedRangeEndpointProvenance, ResolvedTemplateFunctionTypeParameter,
        ResolvedTemplateType, ResolvedTemplateTypeKind, ResolvedTopLevelId, ResolvedTypeParameter,
        ResolvedTypeParameterTable, ResolvedTypeParameters, ResolvedUnaryOperator,
    },
    source::Span,
    syntax,
};

use super::resolver::{resolve_parameter_binding_mode, resolved_visibility};
pub(in crate::resolve::resolver) use type_resolution::TemplateTypeResolver;

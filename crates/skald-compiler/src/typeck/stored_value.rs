//! Shared selection for initializing previously uninitialized stored values.

use crate::{
    hir::{
        HirClassOptionalDestinationInitialization, HirObjectDestinationInitialization,
        HirStoredValueInitialization, Type,
    },
    resolve::{ResolvedExpression, ResolvedExpression::Absent},
};

use super::{
    function::{is_ungrouped_object_call, CallableChecker},
    optional_types::LegacyOptionalKind,
};

impl CallableChecker<'_, '_> {
    pub(super) fn check_stored_value_initialization(
        &mut self,
        expected: Type,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirStoredValueInitialization> {
        match expected {
            Type::Class(class) => self
                .check_object_destination_initialization(class, source, context)
                .map(HirStoredValueInitialization::Class),
            Type::Array(array) => self
                .check_array_initialize(array, source, context)
                .map(HirStoredValueInitialization::Array),
            Type::Shared(target) => self
                .check_shared_transfer(source, target, context)
                .map(HirStoredValueInitialization::Shared),
            Type::Optional(_) => match self
                .optional_kind(expected)
                .expect("enabled optional types must have legacy metadata")
            {
                LegacyOptionalKind::Primitive(payload) => self
                    .check_optional_source(source, payload, context)
                    .map(|source| HirStoredValueInitialization::OptionalPrimitive {
                        source,
                        payload,
                    }),
                LegacyOptionalKind::Class(class) => self
                    .check_optional_class_destination_initialization(class, source, context)
                    .map(HirStoredValueInitialization::OptionalClass),
                LegacyOptionalKind::Shared(target) => self
                    .check_optional_shared_initialize(target, source, context)
                    .map(HirStoredValueInitialization::OptionalShared),
                LegacyOptionalKind::Nested(_) => {
                    let Type::Optional(optional) = expected else {
                        unreachable!()
                    };
                    self.check_optional_value(optional, source, context)
                        .map(|value| HirStoredValueInitialization::Optional(Box::new(value)))
                }
            },
            _ => {
                let value = self.check_expression(source)?;
                self.require_exact_type(value.ty, expected, value.span, context)
                    .then_some(HirStoredValueInitialization::Primitive(value))
            }
        }
    }

    fn check_optional_class_destination_initialization(
        &mut self,
        class: crate::identity::ClassId,
        source: &ResolvedExpression,
        context: &'static str,
    ) -> Option<HirClassOptionalDestinationInitialization> {
        if let Absent(absent) = source {
            return Some(HirClassOptionalDestinationInitialization::Absent {
                class,
                span: absent.span,
            });
        }

        let direct_source = match source {
            ResolvedExpression::Present(present) => &*present.value,
            _ => source,
        };
        let direct_candidate = matches!(
            direct_source,
            ResolvedExpression::Construct(construction) if construction.class == class
        ) || (is_ungrouped_object_call(direct_source)
            && self.resolved_object_class(direct_source) == Some(class));
        if direct_candidate {
            let initialization =
                self.check_object_destination_initialization(class, direct_source, context)?;
            let HirObjectDestinationInitialization::Direct { producer, span } = initialization
            else {
                unreachable!("an exact ungrouped object producer must initialize directly")
            };
            return Some(HirClassOptionalDestinationInitialization::Direct {
                class,
                producer,
                span,
            });
        }

        let source = self.check_class_optional_source(source, class, context)?;
        let Some(operation) = self.copy_capabilities.constructor(class).selected() else {
            self.report_unavailable_copy_operation(class, true, source.span());
            return None;
        };
        let span = source.span();
        Some(HirClassOptionalDestinationInitialization::Copy {
            class,
            source,
            operation,
            span,
        })
    }
}

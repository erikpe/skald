//! Rendering for HIR arrays, optionals, and stored aggregate values.

use std::fmt::Display;

use super::super::ir::*;
use super::ownership::{optional_shared_target_name, shared_target_name};
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn array_receiver(&mut self, receiver: &crate::hir::HirArrayReceiver) {
        self.line(
            &format!(
                "ArrayReceiver {} {:?} access={:?} anchor={:?}",
                receiver.array, receiver.ownership, receiver.access, receiver.anchor
            ),
            receiver.span,
        );
        self.indented(|dumper| match &receiver.source {
            crate::hir::HirArrayReceiverSource::Inline(expression) => dumper.expression(expression),
            crate::hir::HirArrayReceiverSource::Shared(source) => dumper.shared_source(source),
        });
    }

    pub(super) fn array_element(&mut self, place: &crate::hir::HirArrayElementPlace) {
        self.line(
            &format!(
                "ArrayElementPlace : {} {:?}",
                place.element.name(),
                place.evaluation
            ),
            place.span,
        );
        self.indented(|dumper| {
            dumper.array_receiver(&place.receiver);
            dumper.line(
                &format!(
                    "Index normalization={:?} failure={:?}",
                    place.index.normalization, place.index.failure
                ),
                place.index.span,
            );
            dumper.indented(|dumper| dumper.expression(&place.index.value));
        });
    }

    pub(super) fn array_slice(&mut self, slice: &crate::hir::HirArraySlice) {
        self.line(
            &format!(
                "ArraySlice {} copy={} normalization={:?} failure={:?} {:?}",
                slice.array,
                slice
                    .element_copy
                    .map(array_copy_name)
                    .unwrap_or_else(|| "destination-only".to_owned()),
                slice.bounds.normalization,
                slice.bounds.failure,
                slice.evaluation
            ),
            slice.span,
        );
        self.indented(|dumper| {
            dumper.array_receiver(&slice.receiver);
            if let Some(start) = &slice.bounds.start {
                dumper.heading("Start");
                dumper.indented(|dumper| dumper.expression(start));
            } else {
                dumper.raw_line("Start omitted");
            }
            if let Some(end) = &slice.bounds.end {
                dumper.heading("End");
                dumper.indented(|dumper| dumper.expression(end));
            } else {
                dumper.raw_line("End omitted");
            }
        });
    }

    pub(super) fn array_place(&mut self, place: &crate::hir::HirArrayPlace) {
        match place {
            crate::hir::HirArrayPlace::Binding {
                binding,
                array,
                access,
                span,
            } => self.line(
                &format!("ArrayPlace binding {binding} {array} access={access:?}"),
                *span,
            ),
            crate::hir::HirArrayPlace::Field {
                place,
                array,
                access,
                span,
            } => {
                self.line(
                    &format!("ArrayPlace field {array} access={access:?}"),
                    *span,
                );
                self.indented(|dumper| dumper.field_place(place));
            }
            crate::hir::HirArrayPlace::Static { place, array, span } => {
                self.line(&format!("ArrayPlace static {} {array}", place.field), *span)
            }
            crate::hir::HirArrayPlace::Element(place) => self.array_element(place),
        }
    }

    pub(super) fn array_element_value(&mut self, value: &crate::hir::HirArrayElementValue) {
        match value {
            crate::hir::HirArrayElementValue::Value(value) => self.expression(value),
            crate::hir::HirArrayElementValue::Array(value) => self.array_initialize(value),
            crate::hir::HirArrayElementValue::Shared(value) => self.shared_transfer(value),
            crate::hir::HirArrayElementValue::OptionalShared(value) => {
                self.optional_shared_value(value)
            }
            crate::hir::HirArrayElementValue::Optional { source, .. } => {
                self.optional_source(source)
            }
            crate::hir::HirArrayElementValue::ClassOptional(value) => {
                self.class_optional_value(value)
            }
            crate::hir::HirArrayElementValue::AggregateOptional(value) => {
                self.aggregate_optional_value(value)
            }
            crate::hir::HirArrayElementValue::Object { source, operation } => {
                self.object_source(source);
                self.selected_copy_operation(*operation);
            }
        }
    }

    pub(super) fn array_construction(&mut self, construction: &HirArrayConstruction) {
        let ownership = match construction.ownership {
            HirArrayOwnership::Inline => "inline",
            HirArrayOwnership::Shared => "shared",
        };
        self.line(
            &format!("ArrayAllocation {ownership} {}", construction.array),
            construction.span,
        );
        self.indented(|dumper| match &construction.mode {
            HirArrayConstructionMode::Empty => dumper.raw_line("Empty"),
            HirArrayConstructionMode::DefaultLength { length, element } => {
                dumper.raw_line(&format!("DefaultElements {}", array_default_name(*element)));
                dumper.indented(|dumper| dumper.expression(length));
            }
            HirArrayConstructionMode::Copy { source, element } => {
                dumper.raw_line(&format!("CopyElements {}", array_copy_name(*element)));
                dumper.indented(|dumper| dumper.array_source(source));
            }
            HirArrayConstructionMode::Indexed(initializer) => {
                dumper.raw_line("IndexedElements");
                dumper.indented(|dumper| {
                    dumper.heading("Length");
                    dumper.indented(|dumper| dumper.expression(&initializer.length));
                    dumper.local(&initializer.binding, "IndexBinding read-only");
                    dumper.line(
                        &format!("Element : {}", initializer.element.element.name()),
                        initializer.element.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.stored_value_initialization(&initializer.element.value)
                    });
                });
            }
            HirArrayConstructionMode::Elements(list) => {
                dumper.raw_line(&format!(
                    "ElementList count={} commas={}",
                    list.elements.len(),
                    list.comma_spans.len()
                ));
                dumper.indented(|dumper| {
                    for (index, element) in list.elements.iter().enumerate() {
                        dumper.line(
                            &format!("Element {index} : {}", element.element.name()),
                            element.span,
                        );
                        dumper
                            .indented(|dumper| dumper.stored_value_initialization(&element.value));
                    }
                });
            }
        });
    }

    pub(super) fn stored_value_initialization(
        &mut self,
        value: &crate::hir::HirStoredValueInitialization,
    ) {
        match value {
            crate::hir::HirStoredValueInitialization::Scalar(value) => {
                self.raw_line("ScalarInitialization");
                self.indented(|dumper| dumper.expression(value));
            }
            crate::hir::HirStoredValueInitialization::Class(value) => {
                self.object_destination_initialization(value)
            }
            crate::hir::HirStoredValueInitialization::OptionalPrimitive { source, payload } => {
                self.raw_line(&format!(
                    "OptionalPrimitiveInitialization {}?",
                    payload.name()
                ));
                self.indented(|dumper| dumper.optional_source(source));
            }
            crate::hir::HirStoredValueInitialization::OptionalClass(value) => {
                self.class_optional_destination_initialization(value)
            }
            crate::hir::HirStoredValueInitialization::Array(value) => self.array_initialize(value),
            crate::hir::HirStoredValueInitialization::Shared(value) => self.shared_transfer(value),
            crate::hir::HirStoredValueInitialization::OptionalShared(value) => {
                self.optional_shared_value(value)
            }
            crate::hir::HirStoredValueInitialization::Optional(value) => {
                self.aggregate_optional_value(value)
            }
            crate::hir::HirStoredValueInitialization::OptionalBoxPointeeCopy {
                source,
                optional,
                operation,
                span,
            } => {
                self.line(
                    &format!("OptionalBoxPointeeCopy {optional} via {operation:?}"),
                    *span,
                );
                self.indented(|dumper| dumper.shared_source(source));
            }
        }
    }

    fn object_destination_initialization(
        &mut self,
        value: &crate::hir::HirObjectDestinationInitialization,
    ) {
        match value {
            crate::hir::HirObjectDestinationInitialization::Direct { producer, .. } => {
                self.raw_line("ClassInitialization direct");
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirObjectDestinationInitialization::Copy {
                source, operation, ..
            } => {
                self.raw_line("ClassInitialization copy");
                self.indented(|dumper| {
                    dumper.object_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    fn class_optional_destination_initialization(
        &mut self,
        value: &crate::hir::HirClassOptionalDestinationInitialization,
    ) {
        match value {
            crate::hir::HirClassOptionalDestinationInitialization::Absent { class, span } => self
                .line(
                    &format!("ClassOptionalInitialization class {class}? absent"),
                    *span,
                ),
            crate::hir::HirClassOptionalDestinationInitialization::Direct {
                class,
                producer,
                span,
            } => {
                self.line(
                    &format!("ClassOptionalInitialization class {class}? direct"),
                    *span,
                );
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirClassOptionalDestinationInitialization::Copy {
                class,
                source,
                operation,
                span,
            } => {
                self.line(
                    &format!("ClassOptionalInitialization class {class}? copy"),
                    *span,
                );
                self.indented(|dumper| {
                    dumper.class_optional_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    pub(super) fn array_source(&mut self, source: &HirArraySource) {
        let provenance = match source.provenance {
            HirArrayProvenance::Named => "named",
            HirArrayProvenance::Produced => "produced",
        };
        self.line(
            &format!("ArraySource {provenance} {}", source.array),
            source.span,
        );
        self.indented(|dumper| dumper.array_receiver(&source.receiver));
    }

    pub(super) fn array_initialize(&mut self, value: &HirArrayInitialize) {
        let operation = match value.operation {
            HirArrayTransfer::DeepCopy(element) => {
                format!("deep-copy {}", array_copy_name(element))
            }
            HirArrayTransfer::Adopt => "adopt".to_owned(),
        };
        self.line(&format!("ArrayInitialization {operation}"), value.span);
        self.indented(|dumper| dumper.array_source(&value.source));
    }

    pub(super) fn optional_source(&mut self, source: &crate::hir::HirOptionalSource) {
        match source {
            crate::hir::HirOptionalSource::Absent { span } => self.line("OptionalAbsent", *span),
            crate::hir::HirOptionalSource::Present(value) => {
                self.line("OptionalPresent", value.span);
                self.indented(|dumper| dumper.expression(value));
            }
            crate::hir::HirOptionalSource::Copy(place) => {
                self.line("OptionalCopy", place.span);
                self.indented(|dumper| dumper.optional_place(place));
            }
            crate::hir::HirOptionalSource::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    pub(super) fn class_optional_value(&mut self, value: &crate::hir::HirClassOptionalInitialize) {
        self.line(
            &format!("ClassOptionalInitialization class {}?", value.class),
            value.span,
        );
        self.indented(|dumper| dumper.class_optional_source(&value.source));
    }

    pub(super) fn class_optional_source(&mut self, source: &crate::hir::HirClassOptionalSource) {
        match source {
            crate::hir::HirClassOptionalSource::Absent { span } => {
                self.line("ClassOptionalAbsent", *span)
            }
            crate::hir::HirClassOptionalSource::Present(source) => {
                self.line("ClassOptionalPresent", source.span());
                self.indented(|dumper| dumper.object_source(source));
            }
            crate::hir::HirClassOptionalSource::Copy(place) => {
                self.line(
                    &format!("ClassOptionalCopy class {}?", place.class),
                    place.span,
                );
            }
            crate::hir::HirClassOptionalSource::Produced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    pub(super) fn optional_operand(&mut self, operand: &crate::hir::HirOptionalOperand) {
        match operand {
            crate::hir::HirOptionalOperand::Place(place) => self.optional_place(place),
            crate::hir::HirOptionalOperand::Produced(expression) => {
                self.line("OptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::ClassPlace(place) => {
                self.line(
                    &format!("ClassOptionalPlace class {}?", place.class),
                    place.span,
                );
                if let crate::hir::HirOptionalStorage::SharedPointee(pointee) = &place.storage {
                    self.indented(|dumper| {
                        dumper.optional_box_pointee("ClassOptionalBoxPointee", pointee)
                    });
                }
            }
            crate::hir::HirOptionalOperand::ClassProduced(expression) => {
                self.line("ClassOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::SharedPlace(place) => self.optional_shared_place(place),
            crate::hir::HirOptionalOperand::SharedProduced(expression) => {
                self.line("OptionalSharedProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            crate::hir::HirOptionalOperand::AggregatePlace(place) => {
                self.line(
                    &format!("AggregateOptionalPlace {}", place.optional),
                    place.span,
                );
                if let crate::hir::HirOptionalStorage::SharedPointee(pointee) = &place.storage {
                    self.indented(|dumper| {
                        dumper.optional_box_pointee("AggregateOptionalBoxPointee", pointee)
                    });
                }
            }
            crate::hir::HirOptionalOperand::AggregateProduced(expression) => {
                self.line("AggregateOptionalProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    pub(super) fn aggregate_optional_value(&mut self, value: &crate::hir::HirOptionalValue) {
        self.line(
            &format!("AggregateOptionalInitialization {}", value.optional),
            value.span,
        );
        self.indented(|dumper| match &value.source {
            crate::hir::HirOptionalValueSource::Absent => dumper.raw_line("Absent"),
            crate::hir::HirOptionalValueSource::Present(payload) => {
                dumper.raw_line("Present");
                dumper.indented(|dumper| dumper.stored_value_initialization(payload));
            }
            crate::hir::HirOptionalValueSource::Copy(place) => {
                dumper.line(&format!("Copy {}", place.optional), place.span);
            }
            crate::hir::HirOptionalValueSource::Produced(expression) => {
                dumper.line("Produced", expression.span);
                dumper.indented(|dumper| dumper.expression(expression));
            }
        });
    }

    pub(super) fn optional_place(&mut self, place: &crate::hir::HirOptionalPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.line(&format!("OptionalPlace {binding}"), place.span);
            }
            crate::hir::HirOptionalStorage::Static(root) => {
                self.line(&format!("OptionalStaticPlace {}", root.field), place.span);
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line("OptionalFieldPlace", place.span);
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line("OptionalArrayElementPlace", place.span);
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("OptionalBoxPointee", pointee);
            }
        }
    }

    pub(super) fn class_optional_place(&mut self, place: &crate::hir::HirClassOptionalPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => {
                self.line(&format!("ClassOptionalPlace {binding}"), place.span);
            }
            crate::hir::HirOptionalStorage::Static(root) => {
                self.line(
                    &format!("ClassOptionalStaticPlace {}", root.field),
                    place.span,
                );
            }
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line("ClassOptionalFieldPlace", place.span);
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line("ClassOptionalArrayElementPlace", place.span);
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("ClassOptionalBoxPointee", pointee);
            }
        }
    }
}

pub(super) fn array_default_name(element: HirArrayDefaultElement) -> String {
    match element {
        HirArrayDefaultElement::Primitive => "primitive-zero".to_owned(),
        HirArrayDefaultElement::OptionalAbsent => "optional-absent".to_owned(),
        HirArrayDefaultElement::Class { class, initializer } => {
            format!("class {class} via {initializer}")
        }
        HirArrayDefaultElement::ArrayEmpty(array) => format!("empty-array {array}"),
        HirArrayDefaultElement::SharedClass { class, initializer } => {
            format!("shared-class {class} via {initializer}")
        }
        HirArrayDefaultElement::SharedArrayEmpty(array) => {
            format!("shared-empty-array {array}")
        }
        HirArrayDefaultElement::SharedOptionalBoxAbsent(target) => {
            format!("shared-optional-box-absent {target}")
        }
    }
}

pub(super) fn array_copy_name(element: HirArrayCopyElement) -> String {
    match element {
        HirArrayCopyElement::Primitive => "primitive".to_owned(),
        HirArrayCopyElement::OptionalPrimitive => "optional-primitive".to_owned(),
        HirArrayCopyElement::Class { class, operation } => {
            format!("class {class} via {}", selected_operation_name(operation))
        }
        HirArrayCopyElement::OptionalClass { class, operation } => {
            format!(
                "optional-class {class} via {}",
                selected_operation_name(operation)
            )
        }
        HirArrayCopyElement::Array(array) => format!("array {array}"),
        HirArrayCopyElement::Shared(target) => shared_target_name(target),
        HirArrayCopyElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayCopyElement::Optional(optional) => format!("optional {optional}"),
    }
}

pub(super) fn array_assignment_name(element: HirArrayAssignElement) -> String {
    match element {
        HirArrayAssignElement::Primitive => "primitive".to_owned(),
        HirArrayAssignElement::OptionalPrimitive => "optional-primitive".to_owned(),
        HirArrayAssignElement::Class { class, operation } => {
            format!("class {class} via {}", selected_operation_name(operation))
        }
        HirArrayAssignElement::OptionalClass {
            class,
            copy_constructor,
            copy_assignment,
        } => {
            format!(
                "optional-class {class} construct-via {} assign-via {}",
                selected_operation_name(copy_constructor),
                selected_operation_name(copy_assignment)
            )
        }
        HirArrayAssignElement::Array(array) => format!("array {array}"),
        HirArrayAssignElement::Shared(target) => shared_target_name(target),
        HirArrayAssignElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayAssignElement::Optional(optional) => format!("optional {optional}"),
    }
}

fn selected_operation_name<I: Display>(operation: HirSelectedCopyOperation<I>) -> String {
    match operation {
        HirSelectedCopyOperation::User(id) => format!("user {id}"),
        HirSelectedCopyOperation::Synthesized(class) => format!("synthesized {class}"),
    }
}

pub(super) fn array_destruction_name(element: HirArrayDestroyElement) -> String {
    match element {
        HirArrayDestroyElement::Trivial => "trivial".to_owned(),
        HirArrayDestroyElement::Class(class) => format!("class {class}"),
        HirArrayDestroyElement::OptionalClass(class) => format!("optional-class {class}"),
        HirArrayDestroyElement::Array(array) => format!("array {array}"),
        HirArrayDestroyElement::Shared(target) => shared_target_name(target),
        HirArrayDestroyElement::OptionalShared(target) => optional_shared_target_name(target),
        HirArrayDestroyElement::Optional(optional) => format!("optional {optional}"),
    }
}

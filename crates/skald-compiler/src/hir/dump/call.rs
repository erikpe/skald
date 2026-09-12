//! Rendering for HIR construction and direct, indirect, and interface calls.

use crate::source::Span;

use super::super::ir::*;
use super::ownership::optional_shared_target_name;
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn construction(&mut self, construction: &HirConstruction) {
        match &construction.mode {
            HirConstructionMode::Initialize {
                initializer,
                arguments,
            } => {
                self.line(
                    &format!("Construct {} via {initializer}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    for argument in arguments {
                        dumper.call_argument(argument);
                    }
                });
            }
            HirConstructionMode::Copy { source, operation } => {
                self.line(
                    &format!("ExplicitCopyConstruct {}", construction.class),
                    construction.span,
                );
                self.indented(|dumper| {
                    dumper.object_source(source);
                    dumper.selected_copy_operation(*operation);
                });
            }
        }
    }

    pub(super) fn range_protocol_evidence(
        &mut self,
        name: &str,
        evidence: crate::hir::HirRangeProtocolEvidence,
        span: Span,
    ) {
        let crate::hir::HirRangeProtocolRealization::PrimitiveIntrinsic(ty) = evidence.realization;
        let realization = format!("primitive-{}", ty.name());
        self.line(
            &format!(
                "{name} interface={} requirement={} realization={realization}",
                evidence.interface, evidence.requirement,
            ),
            span,
        );
    }

    pub(super) fn object_call(&mut self, call: &HirObjectCall) {
        let target = match call.target {
            HirObjectCallTarget::Direct(function) => format!("function {function}"),
            HirObjectCallTarget::Static(method) => format!("static {method}"),
            HirObjectCallTarget::Method { target, .. } => {
                format!("method {}", method_target(&target))
            }
            HirObjectCallTarget::Interface { target, .. } => {
                format!("interface {} {}", target.interface, target.requirement)
            }
        };
        self.line(&format!("ObjectCall {target} -> {}", call.class), call.span);
        self.indented(|dumper| {
            if let HirObjectCallTarget::Method { receiver, .. } = &call.target {
                dumper.method_receiver(receiver);
            }
            if let HirObjectCallTarget::Interface { receiver, .. } = &call.target {
                dumper.interface_receiver(receiver);
            }
            for argument in &call.arguments {
                dumper.call_argument(argument);
            }
        });
    }

    pub(super) fn interface_receiver(&mut self, receiver: &HirInterfaceReceiver) {
        match receiver {
            HirInterfaceReceiver::View(view) => {
                self.call_argument(&HirCallArgument::View(view.clone()))
            }
            HirInterfaceReceiver::Checked(view) => {
                self.call_argument(&HirCallArgument::CheckedView(view.clone()))
            }
        }
    }

    pub(super) fn call_argument(&mut self, argument: &HirCallArgument) {
        match argument {
            HirCallArgument::Value(expression) => {
                self.line("ValueArgument", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
            HirCallArgument::Optional { source, payload } => {
                self.line(
                    &format!("OptionalArgument {}?", payload.name()),
                    source.span(),
                );
                self.indented(|dumper| dumper.optional_source(source));
            }
            HirCallArgument::ClassOptional(value) => {
                self.line(
                    &format!("ClassOptionalArgument class {}?", value.class),
                    value.span,
                );
                self.indented(|dumper| dumper.class_optional_source(&value.source));
            }
            HirCallArgument::OptionalShared(value) => {
                self.line(
                    &format!(
                        "OptionalSharedArgument {}",
                        optional_shared_target_name(value.target)
                    ),
                    value.span,
                );
                self.indented(|dumper| dumper.optional_shared_source(&value.source));
            }
            HirCallArgument::AggregateOptional(value) => {
                self.line(
                    &format!("AggregateOptionalArgument {}", value.optional),
                    value.span,
                );
                self.indented(|dumper| dumper.aggregate_optional_value(value));
            }
            HirCallArgument::OptionalPlace(place) => match place {
                crate::hir::HirOptionalAliasPlace::Primitive(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument {}?", place.payload.name()),
                        place.span,
                    );
                    self.indented(|dumper| dumper.optional_place(place));
                }
                crate::hir::HirOptionalAliasPlace::Class(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument class {}?", place.class),
                        place.span,
                    );
                    self.indented(|dumper| dumper.class_optional_place(place));
                }
                crate::hir::HirOptionalAliasPlace::Nested(place) => {
                    self.line(
                        &format!("OptionalPlaceArgument {}", place.optional),
                        place.span,
                    );
                }
            },
            HirCallArgument::OptionalSharedPlace(place) => {
                self.line("OptionalSharedPlaceArgument", place.span);
                self.indented(|dumper| dumper.optional_shared_place(place));
            }
            HirCallArgument::Place(place) => {
                self.line("PlaceArgument", place.span());
                self.indented(|dumper| dumper.object_place(place));
            }
            HirCallArgument::View(view) => {
                self.object_view("ViewArgument", view);
            }
            HirCallArgument::CheckedView(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.object_view(&format!("CheckedViewArgument {kind}"), &view.view);
            }
            HirCallArgument::Copy(copy) => {
                self.line("CopyArgument", copy.span);
                self.indented(|dumper| {
                    dumper.object_source(&copy.source);
                    dumper.selected_copy_operation(copy.operation);
                });
            }
            HirCallArgument::Shared(value) => {
                self.line("SharedArgument", value.span);
                self.indented(|dumper| dumper.shared_transfer(value));
            }
            HirCallArgument::SharedPlace(place) => {
                self.line("SharedPlaceArgument", place.span());
                self.indented(|dumper| {
                    dumper.shared_source(&crate::hir::HirSharedSource::Place(place.clone()));
                });
            }
            HirCallArgument::Array(value) => {
                self.line("ArrayArgument", value.span);
                self.indented(|dumper| dumper.array_initialize(value));
            }
            HirCallArgument::ArrayAlias(value) => {
                self.line(
                    &format!(
                        "ArrayAliasArgument : {} access={:?}",
                        value.target.name(),
                        value.access
                    ),
                    value.span,
                );
                self.indented(|dumper| match &value.source {
                    crate::hir::HirArrayAliasSource::Whole(receiver) => {
                        dumper.array_receiver(receiver)
                    }
                    crate::hir::HirArrayAliasSource::Element(place) => dumper.array_element(place),
                    crate::hir::HirArrayAliasSource::OptionalPayload {
                        source,
                        optional,
                        array,
                    } => {
                        dumper.line(
                            &format!(
                                "CheckedOptionalArrayPayload optional={optional} array={array}"
                            ),
                            value.span,
                        );
                        dumper.indented(|dumper| dumper.optional_operand(source));
                    }
                });
            }
            HirCallArgument::PrimitivePlace(place) => {
                let storage = match place.storage {
                    crate::hir::HirPrimitiveStorage::Binding(binding) => {
                        format!("binding {binding}")
                    }
                    crate::hir::HirPrimitiveStorage::Static(place) => {
                        format!("static {}", place.field)
                    }
                };
                self.line(&format!("PrimitivePlaceArgument {storage}"), place.span);
            }
            HirCallArgument::ProducedPrimitiveAlias(expression) => {
                self.line(
                    &format!("ProducedPrimitiveAliasArgument : {}", expression.ty.name()),
                    expression.span,
                );
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }
}

pub(super) fn method_target(target: &HirMethodCallTarget) -> String {
    match target {
        HirMethodCallTarget::Direct(method) => format!("Direct {method}"),
        HirMethodCallTarget::Virtual {
            family,
            slot,
            selected,
        } => format!("Virtual {family} slot {slot} selected {selected}"),
    }
}

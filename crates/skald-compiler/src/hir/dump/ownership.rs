//! Rendering for HIR ownership, object views, places, receivers, and origins.

use super::super::ir::*;
use super::HirDumper;

impl<'types> HirDumper<'types> {
    pub(super) fn shared_transfer(&mut self, value: &HirSharedTransfer) {
        let operation = match value.operation {
            HirOwnerTransfer::Copy => "Copy",
            HirOwnerTransfer::Adopt => "Adopt",
        };
        self.line(
            &format!(
                "SharedTransfer {operation} -> {}",
                shared_target_name(value.target)
            ),
            value.span,
        );
        self.indented(|dumper| dumper.shared_source(&value.source));
    }

    pub(super) fn shared_source(&mut self, source: &HirSharedSource) {
        match source {
            HirSharedSource::Place(HirSharedPlace::Binding {
                binding,
                target,
                span,
            }) => self.line(
                &format!("SharedBinding {binding} : {}", shared_target_name(*target)),
                *span,
            ),
            HirSharedSource::Place(HirSharedPlace::Field {
                place,
                target,
                span,
            }) => {
                self.line(
                    &format!(
                        "SharedField {} : {}",
                        place.field,
                        shared_target_name(*target)
                    ),
                    *span,
                );
                self.indented(|dumper| {
                    if let Some(receiver) = place.receiver.inspection_place() {
                        dumper.object_place(receiver);
                    } else {
                        dumper.field_place(place);
                    }
                });
            }
            HirSharedSource::Place(HirSharedPlace::ArrayElement {
                place,
                target,
                span,
            }) => {
                self.line(
                    &format!("SharedArrayElement : {}", shared_target_name(*target)),
                    *span,
                );
                self.indented(|dumper| dumper.array_element(place));
            }
            HirSharedSource::Place(HirSharedPlace::Static {
                place,
                target,
                span,
            }) => self.line(
                &format!(
                    "SharedStatic {} : {}",
                    place.field,
                    shared_target_name(*target)
                ),
                *span,
            ),
            HirSharedSource::Produced(HirSharedProducer::Allocation(allocation)) => {
                match &allocation.mode {
                    crate::hir::HirSharedAllocationMode::Initialize {
                        initializer,
                        arguments,
                    } => {
                        self.line(
                            &format!(
                                "SharedAllocation {} initialize via {}",
                                allocation.class, initializer
                            ),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            for argument in arguments {
                                dumper.call_argument(argument);
                            }
                        });
                    }
                    crate::hir::HirSharedAllocationMode::Copy { source, operation } => {
                        self.line(
                            &format!("SharedAllocation {} copy", allocation.class),
                            allocation.span,
                        );
                        self.indented(|dumper| {
                            dumper.selected_copy_operation(*operation);
                            dumper.object_source(source);
                        });
                    }
                }
            }
            HirSharedSource::Produced(HirSharedProducer::Call(call)) => {
                self.line("SharedCallResult", call.span);
                self.indented(|dumper| dumper.expression(call));
            }
            HirSharedSource::Produced(HirSharedProducer::Cast(cast)) => {
                let kind = match cast.kind {
                    crate::hir::HirSharedCastKind::Static => "static",
                    crate::hir::HirSharedCastKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!("SharedCast {kind} -> {}", shared_target_name(cast.target)),
                    cast.span,
                );
                self.indented(|dumper| dumper.shared_source(&cast.source));
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalUnwrap { operand, .. }) => {
                self.line("OptionalSharedUnwrap", operand.span());
                self.indented(|dumper| dumper.optional_operand(operand));
            }
            HirSharedSource::Produced(HirSharedProducer::ArrayAllocation(construction)) => {
                self.array_construction(construction);
            }
            HirSharedSource::Produced(HirSharedProducer::OptionalBoxAllocation(allocation)) => {
                self.line(
                    &format!(
                        "OptionalBoxAllocation exact={} target={} static={} order={:?} owner={:?}",
                        allocation.exact_optional,
                        allocation.exact_target,
                        allocation.static_target,
                        allocation.evaluation,
                        allocation.produced_owner,
                    ),
                    allocation.span,
                );
                self.indented(|dumper| {
                    dumper.stored_value_initialization(&allocation.initialization)
                });
            }
        }
    }

    pub(super) fn optional_shared_value(
        &mut self,
        value: &crate::hir::HirOptionalSharedInitialize,
    ) {
        self.line(
            &format!(
                "OptionalSharedInitialization {}",
                optional_shared_target_name(value.target)
            ),
            value.span,
        );
        self.indented(|dumper| dumper.optional_shared_source(&value.source));
    }

    pub(super) fn optional_shared_source(&mut self, source: &crate::hir::HirOptionalSharedSource) {
        match source {
            crate::hir::HirOptionalSharedSource::Absent { span } => {
                self.line("OptionalSharedAbsent", *span)
            }
            crate::hir::HirOptionalSharedSource::Present(source) => {
                self.line("OptionalSharedPresent", source.span());
                self.indented(|dumper| dumper.shared_source(source));
            }
            crate::hir::HirOptionalSharedSource::Copy(place) => {
                self.line("OptionalSharedCopy", place.span);
                self.indented(|dumper| dumper.optional_shared_place(place));
            }
            crate::hir::HirOptionalSharedSource::Produced(expression) => {
                self.line("OptionalSharedProduced", expression.span);
                self.indented(|dumper| dumper.expression(expression));
            }
        }
    }

    pub(super) fn optional_shared_place(&mut self, place: &crate::hir::HirOptionalSharedPlace) {
        match &place.storage {
            crate::hir::HirOptionalStorage::Binding(binding) => self.line(
                &format!(
                    "OptionalSharedPlace {binding} : {}",
                    optional_shared_target_name(place.target)
                ),
                place.span,
            ),
            crate::hir::HirOptionalStorage::Static(root) => self.line(
                &format!(
                    "OptionalSharedStaticPlace {} : {}",
                    root.field,
                    optional_shared_target_name(place.target)
                ),
                place.span,
            ),
            crate::hir::HirOptionalStorage::Field(field) => {
                self.line(
                    &format!(
                        "OptionalSharedFieldPlace : {}",
                        optional_shared_target_name(place.target)
                    ),
                    place.span,
                );
                self.indented(|dumper| dumper.field_place(field));
            }
            crate::hir::HirOptionalStorage::ArrayElement(element) => {
                self.line(
                    &format!(
                        "OptionalSharedArrayElementPlace : {}",
                        optional_shared_target_name(place.target)
                    ),
                    place.span,
                );
                self.indented(|dumper| dumper.array_element(element));
            }
            crate::hir::HirOptionalStorage::SharedPointee(pointee) => {
                self.optional_box_pointee("OptionalSharedBoxPointee", pointee);
            }
        }
    }

    pub(super) fn optional_box_pointee(
        &mut self,
        label: &str,
        pointee: &crate::hir::HirOptionalBoxPointee,
    ) {
        self.line(
            &format!("{label} {} -> {}", pointee.target, pointee.optional),
            pointee.span,
        );
        self.indented(|dumper| dumper.shared_source(&pointee.source));
    }

    pub(super) fn object_view(&mut self, label: &str, view: &HirObjectView) {
        self.line(
            &format!(
                "{label} -> {} {}",
                view_target_name(view.target),
                access_name(view.access)
            ),
            view.span,
        );
        self.indented(|dumper| {
            match &view.source {
                HirViewSource::Place(place) => dumper.object_place(place),
                HirViewSource::Static { place, projections } => {
                    dumper.line(&format!("StaticView {}", place.field), place.span);
                    dumper.indented(|dumper| {
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.heading(&format!("BaseProjection {base}"));
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper.heading(&format!("FieldProjection {field}"));
                                }
                            }
                        }
                    });
                }
                HirViewSource::ArrayElement(element) => dumper.array_element(element),
                HirViewSource::Produced {
                    producer,
                    projections,
                } => {
                    dumper.line("ProducedView", producer.span());
                    dumper.indented(|dumper| {
                        dumper.object_producer(producer);
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.line(&format!("BaseProjection {base}"), producer.span());
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper
                                        .line(&format!("FieldProjection {field}"), producer.span());
                                }
                            }
                        }
                    });
                }
                HirViewSource::Forwarded {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "ForwardedView {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::Shared {
                    binding,
                    target,
                    access,
                    span,
                    ..
                } => dumper.line(
                    &format!(
                        "SharedPointee {binding} : {} {}",
                        view_target_name(*target),
                        access_name(*access)
                    ),
                    *span,
                ),
                HirViewSource::AnchoredShared {
                    source,
                    target,
                    access,
                    span,
                    ..
                } => {
                    dumper.line(
                        &format!(
                            "AnchoredSharedPointee : {} {}",
                            view_target_name(*target),
                            access_name(*access)
                        ),
                        *span,
                    );
                    dumper.indented(|dumper| dumper.shared_source(source));
                }
                HirViewSource::OptionalPayload { view, projections } => {
                    dumper.line(
                        &format!(
                            "CheckedOptionalPayload class {} {}",
                            match &view.source {
                                HirOptionalOperand::ClassPlace(place) => place.class,
                                HirOptionalOperand::ClassProduced(expression) => {
                                    let Type::Optional(optional) = expression.ty else {
                                        unreachable!()
                                    };
                                    match self
                                        .optional_types
                                        .get(optional)
                                        .expect("optional view must name metadata")
                                        .storage
                                    {
                                        HirOptionalStorageCategory::InlineClass(class) => class,
                                        _ => unreachable!(
                                            "optional object view must use class metadata"
                                        ),
                                    }
                                }
                                _ => unreachable!("optional object view must use a class operand"),
                            },
                            access_name(view.access)
                        ),
                        view.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.optional_operand(&view.source);
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.heading(&format!("BaseProjection {base}"));
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper.heading(&format!("FieldProjection {field}"));
                                }
                            }
                        }
                    });
                }
                HirViewSource::OptionalBoxPayload { view, projections } => {
                    dumper.line(
                        &format!(
                            "CheckedOptionalBoxPayload {} -> {} {}",
                            view.box_target,
                            view_target_name(view.target),
                            access_name(view.access)
                        ),
                        view.span,
                    );
                    dumper.indented(|dumper| {
                        dumper.shared_source(&view.source);
                        for projection in projections {
                            match projection {
                                crate::object_path::ObjectProjection::Base(base) => {
                                    dumper.heading(&format!("BaseProjection {base}"));
                                }
                                crate::object_path::ObjectProjection::Field(field) => {
                                    dumper.heading(&format!("FieldProjection {field}"));
                                }
                            }
                        }
                    });
                }
            }
            dumper.object_origin(&view.origin);
        });
    }

    pub(super) fn object_source(&mut self, source: &crate::hir::HirObjectSource) {
        match source {
            crate::hir::HirObjectSource::Place(place) => self.object_place(place),
            crate::hir::HirObjectSource::Static { place, class } => {
                self.line(
                    &format!("StaticObjectSource {} : {class}", place.field),
                    place.span,
                );
            }
            crate::hir::HirObjectSource::ArrayElement(place) => self.array_element(place),
            crate::hir::HirObjectSource::Produced(producer) => {
                self.line("MaterializedSource", producer.span());
                self.indented(|dumper| dumper.object_producer(producer));
            }
            crate::hir::HirObjectSource::Checked(view) => {
                let kind = match view.kind {
                    HirCheckedObjectViewKind::Static => "static",
                    HirCheckedObjectViewKind::RuntimeTerminate => "runtime-terminate",
                };
                self.line(
                    &format!(
                        "CheckedSource {kind} -> {} {}",
                        view_target_name(view.consumer_target),
                        access_name(view.consumer_access)
                    ),
                    view.span,
                );
                self.indented(|dumper| dumper.object_view("SelectedView", &view.view));
            }
            crate::hir::HirObjectSource::Slice(slice) => {
                let path = slice
                    .bases
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(" -> ");
                self.line(
                    &format!("SliceSource [{path}] -> {}", slice.target),
                    slice.span,
                );
                self.indented(|dumper| dumper.object_source(&slice.source));
            }
        }
    }

    pub(super) fn object_producer(&mut self, producer: &crate::hir::HirObjectProducer) {
        match producer {
            crate::hir::HirObjectProducer::StringLiteral(literal) => {
                self.line(
                    &format!("StringLiteral {} class {}", literal.data, literal.class),
                    literal.span,
                );
            }
            crate::hir::HirObjectProducer::Construct(construction) => {
                self.construction(construction)
            }
            crate::hir::HirObjectProducer::Call(call) => self.object_call(call),
            crate::hir::HirObjectProducer::IndirectCall(call) => {
                self.line(
                    &format!(
                        "IndirectObjectCall type {} -> {}",
                        call.function_type,
                        call.result.name()
                    ),
                    call.span,
                );
                self.indented(|dumper| {
                    dumper.heading("Callee");
                    dumper.indented(|dumper| dumper.expression(&call.callee));
                    dumper.heading("Arguments");
                    dumper.indented(|dumper| {
                        for argument in &call.arguments {
                            dumper.call_argument(argument);
                        }
                    });
                });
            }
        }
    }

    pub(super) fn field_place(&mut self, place: &HirFieldPlace) {
        self.line(&format!("FieldPlace {}", place.field), place.span);
        self.indented(|dumper| {
            if place.write_authorization == Some(HirFieldWriteAuthorization::DeclaringClassCell) {
                dumper.raw_line(&format!(
                    "WriteAuthorization DeclaringClassCell {}",
                    place.field
                ));
            }
            if let Some(HirFieldWriteAuthorization::DeclaringClassFinalAssignment(operation)) =
                place.write_authorization
            {
                dumper.raw_line(&format!(
                    "WriteAuthorization DeclaringClassFinalAssignment {} {}",
                    place.field, operation
                ));
            }
            match &place.receiver {
                HirObjectReceiver::ArrayElement {
                    element,
                    place: receiver,
                    ..
                } => {
                    dumper.array_element(element);
                    dumper.object_place(receiver);
                }
                HirObjectReceiver::View { view, .. } => {
                    let label = receiver_view_label(view, "FieldReceiver");
                    dumper.object_view(&label, view);
                }
                HirObjectReceiver::Place {
                    place: receiver, ..
                }
                | HirObjectReceiver::Checked {
                    place: receiver, ..
                } => dumper.object_place(receiver),
            }
        });
    }

    pub(super) fn object_place(&mut self, place: &HirObjectPlace) {
        let access = match place.access {
            HirAccess::ReadOnly => "readonly",
            HirAccess::Mutable => "mutable",
        };
        self.line(
            &format!(
                "ObjectPlace {} : class {} {access}",
                place.path.render_identity(),
                place.class()
            ),
            place.span(),
        );
    }

    pub(super) fn method_receiver(&mut self, receiver: &HirMethodReceiver) {
        self.heading("Receiver");
        self.indented(|dumper| match receiver {
            HirObjectReceiver::ArrayElement {
                element,
                place,
                origin,
            } => {
                dumper.array_element(element);
                dumper.object_place(place);
                dumper.object_origin(origin);
            }
            HirObjectReceiver::View { view, .. } => {
                let label = receiver_view_label(view, "MethodReceiver");
                dumper.object_view(&label, view);
            }
            HirObjectReceiver::Place { place, origin }
            | HirObjectReceiver::Checked { place, origin, .. } => {
                dumper.object_place(place);
                dumper.object_origin(origin);
            }
        });
    }

    fn object_origin(&mut self, origin: &HirObjectOrigin) {
        match origin {
            HirObjectOrigin::Exact {
                complete,
                dynamic_class,
            } => {
                self.heading(&format!("Origin Exact dynamic {dynamic_class}"));
                self.indented(|dumper| dumper.object_place(complete));
            }
            HirObjectOrigin::Static {
                place,
                dynamic_class,
            } => self.line(
                &format!("Origin Static {} dynamic {dynamic_class}", place.field),
                place.span,
            ),
            HirObjectOrigin::Forwarded {
                binding,
                static_target,
                access,
                dispatch_limit,
                span,
            } => {
                let limit = dispatch_limit
                    .map(|class| format!(" limit {class}"))
                    .unwrap_or_default();
                self.line(
                    &format!(
                        "Origin Forwarded {binding} : {} {}{limit}",
                        view_target_name(*static_target),
                        access_name(*access)
                    ),
                    *span,
                );
            }
            HirObjectOrigin::Produced {
                dynamic_class,
                span,
            } => self.line(&format!("Origin Produced dynamic {dynamic_class}"), *span),
            HirObjectOrigin::Shared {
                binding,
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin Shared {binding} : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
            HirObjectOrigin::AnchoredShared {
                static_target,
                access,
                span,
            } => self.line(
                &format!(
                    "Origin AnchoredShared : {} {}",
                    view_target_name(*static_target),
                    access_name(*access)
                ),
                *span,
            ),
        }
    }
}

pub(super) fn view_target_name(target: HirViewTarget) -> String {
    match target {
        HirViewTarget::Class(class) => format!("class {class}"),
        HirViewTarget::Interface(interface) => format!("interface {interface}"),
        HirViewTarget::Obj => "Obj".to_owned(),
    }
}

pub(super) fn shared_target_name(target: HirSharedTarget) -> String {
    match target {
        HirSharedTarget::Class(class) => format!("shared class {class}"),
        HirSharedTarget::Interface(interface) => format!("shared interface {interface}"),
        HirSharedTarget::Obj => "shared Obj".to_owned(),
        HirSharedTarget::Array(array) => format!("shared array {array}"),
        HirSharedTarget::OptionalBox(target) => format!("shared optional-box {target}"),
    }
}

pub(super) fn optional_shared_target_name(target: HirSharedTarget) -> String {
    shared_target_name(target).replacen("shared ", "shared? ", 1)
}

fn receiver_view_label(view: &HirObjectView, suffix: &str) -> String {
    let provenance = match &view.source {
        HirViewSource::Shared { .. } | HirViewSource::AnchoredShared { .. } => "Shared",
        HirViewSource::OptionalPayload { .. } | HirViewSource::OptionalBoxPayload { .. } => {
            "Optional"
        }
        HirViewSource::Produced { .. } => "Produced",
        HirViewSource::ArrayElement(_) => "ArrayElement",
        HirViewSource::Static { .. } => "Static",
        HirViewSource::Place(_) | HirViewSource::Forwarded { .. } => "Object",
    };
    format!("{provenance}{suffix}")
}

pub(super) const fn access_name(access: HirAccess) -> &'static str {
    match access {
        HirAccess::ReadOnly => "readonly",
        HirAccess::Mutable => "mutable",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        identity::{BindingId, ClassId, FieldId, FunctionId, LocalId},
        object_path::ObjectPath,
        source::SourceDatabase,
    };

    #[test]
    fn object_place_dump_renders_the_complete_identity_path_exactly() {
        let mut sources = SourceDatabase::new();
        let source = sources.add("place.ska", "root.link.leaf");
        let span = sources.get(source).unwrap().span(0, 14).unwrap();
        let root = BindingId::Local(LocalId::new(FunctionId::new(0), 0));
        let path = ObjectPath::root(root, ClassId::new(2), span)
            .project_field(FieldId::new(ClassId::new(2), 0), ClassId::new(1), span)
            .project_field(FieldId::new(ClassId::new(1), 3), ClassId::new(0), span);
        let place = HirObjectPlace {
            path,
            access: HirAccess::ReadOnly,
        };
        let optional_types = HirOptionalTypeTable::default();
        let optional_box_types = HirOptionalBoxTypeTable::default();
        let function_types = HirFunctionTypeTable::default();
        let mut dumper = HirDumper::new(&function_types, &optional_types, &optional_box_types);

        dumper.object_place(&place);

        assert_eq!(
            dumper.output,
            "ObjectPlace f0:l0 -> c2:field0 -> c1:field3 : class c0 readonly @0..14\n"
        );
    }
}

//! Object places, receivers, origins, and dereferences.

use super::super::ir::*;
use super::ResolvedDumper;

impl ResolvedDumper<'_> {
    pub(super) fn object_place(&mut self, place: &ResolvedObjectPlace) {
        self.line(
            &format!("Receiver {} class {}", place.render_identity(), place.class),
            place.span,
        );
    }

    pub(super) fn object_receiver(&mut self, receiver: &ResolvedObjectReceiver) {
        match receiver {
            ResolvedObjectReceiver::BindingPath(path) => self.object_place(path),
            ResolvedObjectReceiver::StaticField {
                field,
                projections,
                class,
                span,
            } => {
                self.line(&format!("StaticFieldReceiver {field} class {class}"), *span);
                self.indented(|dumper| {
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
            ResolvedObjectReceiver::CastRelative {
                cast,
                projections,
                class,
                span,
            } => {
                self.line(&format!("CastRelativeReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.line(
                        &format!("CastTarget {}", dumper.render_type_kind(cast.target.kind)),
                        cast.target_span,
                    );
                    dumper.heading("Source");
                    dumper.indented(|dumper| dumper.expression(&cast.source));
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
            ResolvedObjectReceiver::Dereference {
                dereference,
                projections,
                class,
                span,
            } => {
                self.line(&format!("DereferenceReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.dereference(dereference);
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
            ResolvedObjectReceiver::OptionalPayload {
                unwrap,
                projections,
                class,
                span,
            } => {
                self.line(&format!("OptionalPayloadReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.heading("Optional");
                    dumper.indented(|dumper| dumper.expression(&unwrap.source));
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
            ResolvedObjectReceiver::ArrayElement {
                projection,
                projections,
                class,
                span,
            } => {
                self.line(&format!("ArrayElementReceiver class {class}"), *span);
                self.indented(|dumper| {
                    dumper.expression(&ResolvedExpression::ArrayProjection(projection.clone()));
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
            ResolvedObjectReceiver::Produced {
                producer,
                exact_class,
                projections,
                class,
                span,
            } => {
                self.line(
                    &format!("ProducedReceiver class {class} complete {exact_class}"),
                    *span,
                );
                self.indented(|dumper| {
                    dumper.heading("Producer");
                    dumper.indented(|dumper| dumper.expression(producer));
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
    }

    pub(super) fn dereference(&mut self, dereference: &ResolvedDereferenceExpr) {
        let operator = match dereference.operator {
            ResolvedDereferenceOperator::Star => "Star",
            ResolvedDereferenceOperator::Arrow => "Arrow",
        };
        self.line(
            &format!(
                "Dereference {operator} target {}",
                self.render_shared_target(dereference.target)
            ),
            dereference.span,
        );
        self.indented(|dumper| dumper.expression(&dereference.source));
    }
}

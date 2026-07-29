//! Object production, owned call arguments, and full-expression lifetimes.

use crate::{
    hir::{
        HirConstruction, HirConstructionMode, HirObjectCall, HirObjectCallTarget,
        HirObjectProducer, HirObjectSource,
    },
    identity::ClassId,
    source::Span,
};

use super::call::lower_method_target;
use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_object_producer(
        &mut self,
        producer: &HirObjectProducer,
        destination: MirPlace,
    ) {
        match producer {
            HirObjectProducer::StringLiteral(literal) => {
                self.lower_string_literal(literal, destination)
            }
            HirObjectProducer::Construct(construction) => {
                self.lower_construction(construction, destination);
            }
            HirObjectProducer::Call(call) => self.lower_object_call(call, destination),
        }
    }

    fn lower_string_literal(
        &mut self,
        literal: &crate::hir::HirStringLiteral,
        destination: MirPlace,
    ) {
        let item = self
            .input
            .string_language_item
            .expect("typed string literal requires language-item metadata");
        debug_assert_eq!(literal.class, item.class);
        let target = crate::hir::HirSharedTarget::Array(item.storage_array);
        let backing = self.new_shared_temporary(target, literal.span);
        self.emit(MirInstruction::SharedStatic(MirSharedStatic {
            destination: backing,
            data: literal.data,
            target: MirSharedTarget::Array(item.storage_array),
            origin: MirStaticAllocationOrigin::Immortal,
            span: literal.span,
        }));
        self.consume_shared_temporary(backing);
        let length = self
            .input
            .literal_data
            .get(literal.data)
            .map(|data| {
                u64::try_from(data.bytes.len())
                    .expect("literal byte length must fit the language u64 length")
            })
            .expect("typed literal must reference declared data");
        self.emit(MirInstruction::StringInitialize(MirStringInitialize {
            destination,
            data: literal.data,
            backing,
            class: item.class,
            storage_field: item.storage_field,
            start_field: item.start_field,
            length_field: item.length_field,
            start: 0,
            length,
            span: literal.span,
        }));
        self.full_expression_has_shared_effect = true;
    }

    pub(super) fn lower_construction(
        &mut self,
        construction: &HirConstruction,
        destination: MirPlace,
    ) {
        self.lower_construction_at(construction, destination, construction.span);
    }

    pub(super) fn lower_construction_at(
        &mut self,
        construction: &HirConstruction,
        destination: MirPlace,
        span: Span,
    ) {
        match &construction.mode {
            HirConstructionMode::Initialize {
                initializer,
                arguments,
            } => {
                let arguments = self.lower_call_arguments(arguments);
                self.emit(MirInstruction::Initialize(MirInitialize {
                    destination,
                    target: *initializer,
                    arguments,
                    span,
                }));
            }
            HirConstructionMode::Copy { source, operation } => {
                let optional_mark = self.optional_view_mark();
                let source = self.lower_object_source(source);
                self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                    destination,
                    source,
                    class: construction.class,
                    operation: lower_selected_copy_operation(*operation),
                    span,
                }));
                self.end_optional_views_from(optional_mark, span);
            }
        }
    }

    fn lower_object_call(&mut self, call: &HirObjectCall, destination: MirPlace) {
        let optional_mark = self.optional_view_mark();
        let (target, receiver) = match &call.target {
            HirObjectCallTarget::Direct(function) => (MirCallTarget::Direct(*function), None),
            HirObjectCallTarget::Static(method) => (MirCallTarget::Static(*method), None),
            HirObjectCallTarget::Method { receiver, target } => (
                MirCallTarget::Method(lower_method_target(*target)),
                Some(self.lower_method_receiver(receiver).into()),
            ),
            HirObjectCallTarget::Interface { receiver, target } => (
                MirCallTarget::Interface(MirInterfaceCallTarget {
                    interface: target.interface,
                    requirement: target.requirement,
                }),
                Some(
                    match receiver {
                        crate::hir::HirInterfaceReceiver::View(view) => {
                            self.lower_object_view(view)
                        }
                        crate::hir::HirInterfaceReceiver::Checked(view) => {
                            self.lower_checked_object_view(view)
                        }
                    }
                    .into(),
                ),
            ),
        };
        let arguments = self.lower_call_arguments(&call.arguments);
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            shared_result: None,
            destination: Some(destination),
            span: call.span,
        }));
        self.end_optional_views_from(optional_mark, call.span);
    }

    pub(super) fn lower_object_source(&mut self, source: &HirObjectSource) -> MirPlace {
        match source {
            HirObjectSource::Place(place) => self.lower_object_place(place),
            HirObjectSource::ArrayElement(element) => self.lower_array_element_place(element),
            HirObjectSource::Produced(producer) => {
                let storage = self.new_object_storage(
                    MirStorageKind::Temporary,
                    "temporary",
                    producer.class(),
                    producer.span(),
                );
                let destination = MirPlace::base(storage);
                self.lower_object_producer(producer, destination.clone());
                self.full_expression_temporaries
                    .push(FullExpressionTemporary::Inline(MirCleanup {
                        destination: destination.clone(),
                        target: producer.class(),
                        span: producer.span(),
                    }));
                destination
            }
            HirObjectSource::Checked(view) => self.lower_checked_object_view(view).source,
            HirObjectSource::Slice(slice) => slice.bases.iter().copied().fold(
                self.lower_object_source(&slice.source),
                MirPlace::project_base,
            ),
        }
    }

    pub(super) fn new_object_storage(
        &mut self,
        kind: MirStorageKind,
        name: &str,
        class: ClassId,
        span: Span,
    ) -> StorageId {
        debug_assert!(matches!(
            kind,
            MirStorageKind::Argument | MirStorageKind::Temporary
        ));
        let id = StorageId::new(self.input.callable, self.storage.len());
        self.storage.push(MirStorage {
            id,
            source: None,
            name: format!("{name}{}", id.index()),
            kind,
            ty: MirType::Class(class),
            span,
        });
        self.track_full_expression_storage(id, span);
        id
    }
}

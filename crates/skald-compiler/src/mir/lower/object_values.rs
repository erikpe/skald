//! Object production, owned call arguments, and full-expression lifetimes.

use crate::{
    hir::{
        HirCallArgument, HirConstruction, HirObjectCall, HirObjectCallTarget, HirObjectProducer,
        HirObjectSource,
    },
    identity::ClassId,
    source::Span,
};

use super::*;

impl BodyLowerer<'_> {
    pub(super) fn lower_object_producer(
        &mut self,
        producer: &HirObjectProducer,
        destination: MirPlace,
    ) {
        match producer {
            HirObjectProducer::Construct(construction) => {
                self.lower_construction(construction, destination);
            }
            HirObjectProducer::Call(call) => self.lower_object_call(call, destination),
        }
    }

    pub(super) fn lower_construction(
        &mut self,
        construction: &HirConstruction,
        destination: MirPlace,
    ) {
        let arguments = self.lower_call_arguments(&construction.arguments);
        self.emit(MirInstruction::Initialize(MirInitialize {
            destination,
            target: construction.initializer,
            arguments,
            span: construction.span,
        }));
    }

    fn lower_object_call(&mut self, call: &HirObjectCall, destination: MirPlace) {
        let (target, receiver) = match &call.target {
            HirObjectCallTarget::Direct(function) => (MirCallTarget::Direct(*function), None),
            HirObjectCallTarget::Method { receiver, method } => (
                MirCallTarget::Method(*method),
                Some(self.lower_object_place(receiver)),
            ),
        };
        let arguments = self.lower_call_arguments(&call.arguments);
        self.emit(MirInstruction::Call(MirCall {
            target,
            receiver,
            arguments,
            result: None,
            destination: Some(destination),
            span: call.span,
        }));
    }

    pub(super) fn lower_object_source(&mut self, source: &HirObjectSource) -> MirPlace {
        match source {
            HirObjectSource::Place(place) => self.lower_object_place(place),
            HirObjectSource::Produced(producer) => {
                let storage = self.new_object_storage(
                    MirStorageKind::Temporary,
                    "temporary",
                    producer.class(),
                    producer.span(),
                );
                let destination = MirPlace::base(storage);
                self.lower_object_producer(producer, destination.clone());
                self.full_expression_temporaries.push(MirCleanup {
                    destination: destination.clone(),
                    target: producer.class(),
                    span: producer.span(),
                });
                destination
            }
        }
    }

    pub(super) fn lower_call_arguments(
        &mut self,
        arguments: &[HirCallArgument],
    ) -> Vec<MirArgument> {
        arguments
            .iter()
            .map(|argument| match argument {
                HirCallArgument::Value(expression) => MirArgument::Value(
                    self.lower_expression(expression)
                        .expect("typed value argument must produce a scalar value"),
                ),
                HirCallArgument::Place(place) => MirArgument::Place(self.lower_object_place(place)),
                HirCallArgument::Copy(copy) => {
                    let source = self.lower_object_source(&copy.source);
                    let destination = self.new_object_storage(
                        MirStorageKind::Argument,
                        "argument",
                        copy.source.class(),
                        copy.span,
                    );
                    self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                        destination: MirPlace::base(destination),
                        source,
                        class: copy.source.class(),
                        operation: lower_selected_copy_operation(copy.operation),
                        span: copy.span,
                    }));
                    MirArgument::OwnedPlace(MirPlace::base(destination))
                }
            })
            .collect()
    }

    fn new_object_storage(
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
        id
    }

    pub(super) fn finish_full_expression(&mut self, span: Span) {
        if self.full_expression_temporaries.is_empty() {
            return;
        }
        let temporaries = self
            .full_expression_temporaries
            .drain(..)
            .rev()
            .map(|mut cleanup| {
                cleanup.span = span;
                cleanup
            })
            .collect();
        self.emit(MirInstruction::EndFullExpression(MirEndFullExpression {
            temporaries,
            span,
        }));
    }
}

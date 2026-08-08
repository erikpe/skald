//! Preliminary MIR lowering for explicit static declaration initializers.

use crate::{
    hir::{
        HirBlock, HirControlEffects, HirObjectDestinationInitialization, HirStaticFieldInitializer,
        HirStoredValueInitialization, Type,
    },
    mir::{
        MirCopyConstruction, MirInstruction, MirPlace, MirSharedFieldInitialize,
        MirStaticPublication, MirStore, MirTerminator, PreliminaryMirStaticField,
        PreliminaryMirStaticInitializer,
    },
};

use super::*;

pub(super) fn lower_static_initializers(
    hir: &HirProgram,
    string_language_item: Option<MirStringLanguageItem>,
) -> (
    Vec<PreliminaryMirStaticField>,
    Vec<PreliminaryMirStaticInitializer>,
) {
    let static_fields = hir
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .map(|field| PreliminaryMirStaticField {
            field: field.id,
            ty: lower_type(field.ty),
            initializer: field.initializer.as_ref().map(|initializer| initializer.id),
            span: field.span,
        })
        .collect();
    let static_initializers = hir
        .classes
        .iter()
        .flat_map(|class| &class.static_fields)
        .filter_map(|field| {
            field.initializer.as_ref().map(|initializer| {
                lower_static_initializer(hir, field.id, field.ty, initializer, string_language_item)
            })
        })
        .collect();
    (static_fields, static_initializers)
}

fn lower_static_initializer(
    hir: &HirProgram,
    field: crate::identity::StaticFieldId,
    destination_type: Type,
    initializer: &HirStaticFieldInitializer,
    string_language_item: Option<MirStringLanguageItem>,
) -> PreliminaryMirStaticInitializer {
    let source_body = HirBlock {
        statements: Vec::new(),
        effects: HirControlEffects::fallthrough(),
        span: initializer.span,
    };
    let mut lowerer = BodyLowerer::new(BodyLoweringInput {
        callable: initializer.id.into(),
        parameters: &[],
        locals: &[],
        source_body: &source_body,
        return_type: Type::Unit,
        receiver_class: None,
        string_language_item,
        literal_data: &hir.literal_data,
    });

    let destination = MirPlace::static_field(field);
    lowerer.lower_stored_value_initialization(
        destination,
        destination_type,
        &initializer.value,
        initializer.span,
    );

    // Split the CFG at publication. Static effect analysis can classify the
    // initializer side and post-publication temporary cleanup without
    // reconstructing an instruction position inside a basic block.
    let initialization_exit = lowerer.body.current();
    let cleanup_entry = lowerer.body.allocate_block(initializer.span);
    lowerer.terminate(MirTerminator::Goto {
        target: cleanup_entry,
        span: initializer.span,
    });
    lowerer
        .body
        .select_block(cleanup_entry)
        .expect("allocated static cleanup-entry block must be selectable");
    lowerer.finish_full_expression(initializer.span);
    lowerer.terminate(MirTerminator::Return {
        value: None,
        span: initializer.span,
    });

    PreliminaryMirStaticInitializer {
        id: initializer.id,
        field,
        destination_type: lower_type(destination_type),
        publication: MirStaticPublication {
            initialization_exit,
            cleanup_entry,
            span: initializer.span,
        },
        storage: lowerer.storage,
        values: lowerer.values,
        body: lowerer.body.finish(),
        span: initializer.span,
    }
}

impl BodyLowerer<'_> {
    fn lower_stored_value_initialization(
        &mut self,
        destination: MirPlace,
        destination_type: Type,
        initialization: &HirStoredValueInitialization,
        span: crate::source::Span,
    ) {
        match initialization {
            HirStoredValueInitialization::Primitive(expression) => {
                let value = self
                    .lower_expression(expression)
                    .expect("typed primitive static initializer must produce a MIR value");
                self.emit(MirInstruction::Store(MirStore {
                    destination,
                    value,
                    span,
                }));
            }
            HirStoredValueInitialization::Class(initialization) => {
                let Type::Class(class) = destination_type else {
                    unreachable!("class stored initialization requires exact-class storage")
                };
                match initialization {
                    HirObjectDestinationInitialization::Direct { producer, .. } => {
                        self.lower_object_producer(producer, destination);
                    }
                    HirObjectDestinationInitialization::Copy {
                        source, operation, ..
                    } => {
                        let optional_mark = self.optional_view_mark();
                        let source = self.lower_object_source(source);
                        self.emit(MirInstruction::CopyConstruct(MirCopyConstruction {
                            destination,
                            source,
                            class,
                            operation: lower_selected_copy_operation(*operation),
                            span,
                        }));
                        self.end_optional_views_from(optional_mark, span);
                    }
                }
            }
            HirStoredValueInitialization::OptionalPrimitive { source, .. } => {
                self.lower_optional_initialize_at(destination, source, span);
            }
            HirStoredValueInitialization::OptionalClass(initialization) => {
                self.lower_class_optional_destination_initialize(destination, initialization);
            }
            HirStoredValueInitialization::Array(initialization) => {
                self.lower_array_initialize(destination, initialization, false);
            }
            HirStoredValueInitialization::Shared(transfer) => {
                let source = self.new_shared_temporary(transfer.target, transfer.span);
                self.lower_shared_transfer(source, transfer);
                self.consume_shared_temporary(source);
                self.emit(MirInstruction::SharedFieldInitialize(
                    MirSharedFieldInitialize {
                        destination,
                        source,
                        span,
                    },
                ));
            }
            HirStoredValueInitialization::OptionalShared(initialization) => {
                self.lower_optional_shared_initialize_at(destination, initialization);
            }
        }
    }
}

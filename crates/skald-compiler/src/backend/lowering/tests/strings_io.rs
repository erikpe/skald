use crate::{
    backend::{
        lir::{CallAttribution, CallTarget, Constant, Operation, Terminator},
        lowering::lower_program,
        plan::{DataKey, DataPurpose, RuntimeService},
        planning::plan_program,
        BackendInput,
    },
    mir::test_fixtures::io_program_with_app_and_additional_bodies,
};

#[test]
fn repeated_and_empty_literals_share_immortal_typed_data() {
    let program = crate::passes::verify_final_mir(io_program_with_app_and_additional_bodies(
        concat!(
            "import std::io; import std::str;",
            "fn main()->i64{var first:std::str::Str=\"same\";",
            "var second:std::str::Str=\"same\";var empty:std::str::Str=\"\";",
            "return 0;}"
        ),
        "",
    ))
    .unwrap();
    let planned = plan_program(BackendInput::without_runtime_trace(&program)).unwrap();
    let resources = planned.plan().view().resources();
    let repeated = program
        .program()
        .literal_data
        .iter()
        .filter(|literal| literal.bytes == b"same")
        .map(|literal| literal.id)
        .collect::<Vec<_>>();
    assert_eq!(repeated.len(), 2);
    let canonical = resources.literal_backing(repeated[0]).unwrap();
    assert_eq!(resources.literal_backing(repeated[1]), Some(canonical));
    assert_eq!(
        resources
            .data
            .iter()
            .filter(|data| {
                data.purpose == DataPurpose::LiteralBacking
                    && matches!(data.key, DataKey::Literal(id) if id == canonical)
            })
            .count(),
        1
    );

    let mut saw_literal_address = false;
    let program = lower_program(&planned, |body| {
        saw_literal_address |= body.draft().blocks().any(|(_, block)| {
            block.instructions.iter().any(|instruction| {
                matches!(
                    instruction.operation,
                    Operation::SymbolAddress {
                        symbol: crate::backend::plan::ArtifactId::Data(DataKey::Literal(_)),
                        ..
                    }
                )
            })
        });
        Ok(())
    })
    .unwrap();
    assert!(saw_literal_address);
    assert_eq!(program.data().len(), resources.data.len());
}

#[test]
fn all_standard_io_operations_use_ordered_nonreporting_runtime_calls() {
    let program = crate::passes::verify_final_mir(io_program_with_app_and_additional_bodies(
        concat!(
            "import std::io; fn main()->i64{var bytes:u8[]=u8[](2u);",
            "var handle:i64=std::io::standard(1u8);",
            "var opened:i64=std::io::open(bytes,0u8);",
            "var read:i64=std::io::read(handle,bytes,0u);",
            "var written:i64=std::io::write(handle,bytes,0u);",
            "return opened+read+written+std::io::close(handle);}"
        ),
        "",
    ))
    .unwrap();
    {
        let planned = plan_program(BackendInput::without_runtime_trace(&program))
            .expect("the complete standard-I/O intrinsic surface is planned");
        let mut observed = std::collections::BTreeSet::new();
        lower_program(&planned, |body| {
            for (_, block) in body.draft().blocks() {
                for instruction in &block.instructions {
                    let Operation::Call(call) = &instruction.operation else {
                        continue;
                    };
                    let CallTarget::Direct(crate::backend::plan::ArtifactId::Runtime(service)) =
                        call.target
                    else {
                        continue;
                    };
                    if matches!(
                        service,
                        RuntimeService::IoStandardHandle
                            | RuntimeService::IoOpen
                            | RuntimeService::IoRead
                            | RuntimeService::IoWrite
                            | RuntimeService::IoClose
                    ) {
                        assert_eq!(call.attribution, CallAttribution::NonReporting);
                        assert!(call.arguments.iter().enumerate().all(|(index, argument)| {
                            argument.role
                                == crate::backend::plan::ComponentRole::RuntimeParameter(index)
                        }));
                        observed.insert(service);
                    }
                }
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(
            observed,
            [
                RuntimeService::IoStandardHandle,
                RuntimeService::IoOpen,
                RuntimeService::IoRead,
                RuntimeService::IoWrite,
                RuntimeService::IoClose,
            ]
            .into_iter()
            .collect()
        );
    }
}

#[test]
fn io_buffers_keep_empty_and_partial_range_control_flow() {
    let program = crate::passes::verify_final_mir(io_program_with_app_and_additional_bodies(
        "import std::io; fn main()->i64{var bytes:u8[]=u8[](2u);return std::io::exercise(1,bytes);}",
        "public fn exercise(handle:i64,mut ref bytes:u8[])->i64{var offset:u64=1u;var read:i64=_io_read(handle,bytes,offset);return read+_io_write(handle,bytes,offset);}",
    )).unwrap();
    let planned = plan_program(BackendInput::without_runtime_trace(&program)).unwrap();
    let mut branches = 0;
    let mut subtracts = 0;
    let mut nulls = 0;
    lower_program(&planned, |body| {
        for (_, block) in body.draft().blocks() {
            branches += usize::from(matches!(block.terminator, Some(Terminator::Branch { .. })));
            for instruction in &block.instructions {
                subtracts += usize::from(matches!(
                    instruction.operation,
                    Operation::Binary {
                        operation: crate::backend::lir::BinaryOperation::Subtract,
                        ..
                    }
                ));
                nulls += usize::from(matches!(
                    instruction.operation,
                    Operation::Constant(Constant::Null(_))
                ));
            }
        }
        Ok(())
    })
    .unwrap();
    assert!(branches >= 2 && subtracts >= 2 && nulls >= 2);
}

#[test]
fn source_panic_uses_the_checked_string_slice_and_source_attribution() {
    let expected = b"panic bytes";
    let program = crate::passes::verify_final_mir(io_program_with_app_and_additional_bodies(
        "import std::error; fn main()->i64{std::error::panic(\"panic bytes\");}",
        "",
    ))
    .unwrap();
    let planned = plan_program(BackendInput::without_runtime_trace(&program)).unwrap();
    assert!(planned.plan().view().resources().data.iter().any(|data| {
        data.purpose == DataPurpose::LiteralBacking
            && data.initializers.iter().any(|initializer| {
                matches!(initializer, crate::backend::plan::DataInitializerFact::Bytes(bytes) if bytes == expected)
            })
    }));

    let mut saw_panic = false;
    lower_program(&planned, |body| {
        for (_, block) in body.draft().blocks() {
            let Some(Terminator::NonReturningCall(call)) = &block.terminator else {
                continue;
            };
            if call.target
                == CallTarget::Direct(crate::backend::plan::ArtifactId::Runtime(
                    RuntimeService::Panic,
                ))
            {
                assert_eq!(call.arguments.len(), 2);
                assert!(call.arguments.iter().enumerate().all(|(index, argument)| {
                    argument.role == crate::backend::plan::ComponentRole::RuntimeParameter(index)
                }));
                assert!(matches!(
                    call.attribution,
                    CallAttribution::SourceOperation { .. }
                ));
                saw_panic = true;
            }
        }
        Ok(())
    })
    .unwrap();
    assert!(saw_panic);
}

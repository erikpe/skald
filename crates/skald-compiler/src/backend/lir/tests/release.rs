use super::calls::runtime_call;
use super::tracing::trace_catalog;
use super::*;
use crate::backend::effects::Effect;
use crate::backend::plan::{
    test_fixtures::runtime_declarations, ArtifactId, Convention, HelperFamily, HelperKey,
    LirCallableId, RuntimeService, SignatureFact,
};
use crate::backend::RuntimeTracePolicy;

/// Ordinary instructions express all paths and retain the header across an
/// arbitrary finalizer call. This fixture grants no native/verification proof.
#[test]
pub(super) fn shared_release_uses_explicit_memory_branches_finalizer_and_original_header_after_call(
) {
    for tracing in [false, true] {
        let mut supplied = facts();
        let services = runtime_declarations(&mut supplied);
        supplied.signatures[0].inputs = vec![Component {
            ty: ScalarType::DataAddress,
            role: ComponentRole::Parameter(0),
        }];
        let signature = supplied.callables[0].signature;
        let finalizer_signature = supplied
            .add_signature(SignatureFact {
                convention: Convention::Language,
                inputs: vec![Component {
                    ty: ScalarType::DataAddress,
                    role: ComponentRole::Parameter(0),
                }],
                results: vec![],
                returns: ReturnShape::Unit,
            })
            .unwrap();
        let layout = supplied
            .add_layout(LayoutFact {
                size: 32,
                alignment: 8,
                disposition: LayoutDisposition::Addressable,
            })
            .unwrap();
        let helper = LirCallableId::Helper(HelperKey {
            family: HelperFamily::Release,
            layout,
            signature,
        });
        supplied
            .callables
            .push(crate::backend::plan::CallableDeclaration {
                key: helper,
                signature,
                body: crate::backend::plan::BodyDisposition::Required,
            });
        let trace = tracing.then(|| trace_catalog(&mut supplied));
        let plan = CheckedPlan::check(supplied).unwrap();
        let mut builder = DraftBuilder::new(plan.view().callable(helper).unwrap()).unwrap();
        let entry = entry_block(&mut builder);
        let header = builder.inputs().next().unwrap();
        if let Some((context, location)) = trace {
            // An omitted generated helper inherits its caller; it has no frame actions.
            builder
                .declare_trace_plan(TracePlan {
                    frame_eligible: false,
                    record: None,
                    context,
                    locations: vec![location],
                })
                .unwrap();
        }
        let ordinary = block(&mut builder);
        let decrement = block(&mut builder);
        let last_owner = block(&mut builder);
        let done = block(&mut builder);
        let count_repr = MemoryRepresentation {
            scalar: ScalarType::U64,
            bytes: 8,
            alignment: 8,
        };
        let count = builder
            .append(
                entry,
                Operation::Load {
                    address: header,
                    representation: count_repr,
                },
            )
            .unwrap()[0];
        let immortal = constant(&mut builder, entry, Constant::U64(u64::MAX));
        let immortal = builder
            .append(
                entry,
                Operation::Compare {
                    predicate: Predicate::Equal,
                    left: count,
                    right: immortal,
                },
            )
            .unwrap()[0];
        builder
            .terminate(
                entry,
                Terminator::Branch {
                    condition: immortal,
                    true_edge: empty_edge(done),
                    false_edge: empty_edge(ordinary),
                },
            )
            .unwrap();
        let one = constant(&mut builder, ordinary, Constant::U64(1));
        let last = builder
            .append(
                ordinary,
                Operation::Compare {
                    predicate: Predicate::Equal,
                    left: count,
                    right: one,
                },
            )
            .unwrap()[0];
        builder
            .terminate(
                ordinary,
                Terminator::Branch {
                    condition: last,
                    true_edge: empty_edge(last_owner),
                    false_edge: empty_edge(decrement),
                },
            )
            .unwrap();
        let reduced = builder
            .append(
                decrement,
                Operation::Binary {
                    operation: BinaryOperation::Subtract,
                    left: count,
                    right: one,
                },
            )
            .unwrap()[0];
        builder
            .append(
                decrement,
                Operation::Store {
                    address: header,
                    value: reduced,
                    representation: count_repr,
                },
            )
            .unwrap();
        builder
            .terminate(decrement, Terminator::Jump(empty_edge(done)))
            .unwrap();
        let finalizer_offset = constant(&mut builder, last_owner, Constant::U64(8));
        let finalizer_address = builder
            .append(
                last_owner,
                Operation::ByteOffset {
                    base: header,
                    offset: finalizer_offset,
                },
            )
            .unwrap()[0];
        let finalizer = builder
            .append(
                last_owner,
                Operation::Load {
                    address: finalizer_address,
                    representation: MemoryRepresentation {
                        scalar: ScalarType::CodeAddress(finalizer_signature),
                        bytes: 8,
                        alignment: 8,
                    },
                },
            )
            .unwrap()[0];
        builder
            .append(
                last_owner,
                Operation::Call(Call {
                    target: CallTarget::Indirect(finalizer),
                    signature: finalizer_signature,
                    arguments: vec![CallArgument {
                        role: ComponentRole::Parameter(0),
                        value: header,
                    }],
                    attribution: CallAttribution::InheritedOperation {
                        boundary: source(0),
                    },
                }),
            )
            .unwrap();
        builder
            .append(
                last_owner,
                Operation::Call(runtime_call(
                    RuntimeService::Free,
                    services[&RuntimeService::Free],
                    &[header],
                    CallAttribution::HardDefectOnly,
                )),
            )
            .unwrap();
        builder
            .terminate(last_owner, Terminator::Jump(empty_edge(done)))
            .unwrap();
        builder.terminate(done, Terminator::Return(vec![])).unwrap();
        let draft = builder.finish();
        crate::backend::graph::check_graph(&draft).unwrap();
        assert_eq!(draft.blocks().len(), 5);
        assert_eq!(draft.trace_plan().is_some(), tracing);
        assert_eq!(
            draft.owner().context().runtime_trace(),
            if tracing {
                RuntimeTracePolicy::Enabled
            } else {
                RuntimeTracePolicy::Omitted
            }
        );
        let instructions = &draft.block(last_owner).unwrap().instructions;
        match &instructions[3].operation {
            Operation::Call(Call {
                target: CallTarget::Indirect(actual),
                arguments,
                ..
            }) => {
                assert_eq!(*actual, finalizer.id());
                assert_eq!(arguments[0].value, header.id());
            }
            _ => panic!("expected finalizer"),
        }
        match &instructions[4].operation {
            Operation::Call(Call {
                target: CallTarget::Direct(ArtifactId::Runtime(RuntimeService::Free)),
                arguments,
                ..
            }) => assert_eq!(arguments[0].value, header.id()),
            _ => panic!("expected free of original header"),
        }
        assert!(instructions[3].effects.contains(Effect::Call));
        assert!(instructions[4].effects.contains(Effect::Free));
        assert!(draft
            .blocks()
            .flat_map(|(_, block)| &block.instructions)
            .all(|instruction| !matches!(instruction.operation, Operation::Trace(_))));
        inspection::assert_dump(&verify_callable(draft).unwrap());
    }
}

//! Ordinary verified lower graphs become native virtual graphs, never stack code.
use super::super::{classify, AbiError, CallArity};
use super::{
    verify::{float_condition, integer_condition},
    Instruction, Opcode, Origin, Site, ValueRef, Verifier,
};
use crate::backend::{
    effects::MemoryRegion,
    graph::{LocalHandle, LoweredValueId, SelectedBlockId, SelectedValueId},
    lir::{AddressProvenance, Operation, Terminator, VerifiedCallable},
    plan::{LayoutDisposition, PlanError, ScalarType},
    selected::{
        verify_selected, Representation, SelectedBuildError, SelectedBuilder, SelectedFailure,
        SelectionContext, VerifiedSelectedCallable,
    },
};
use std::collections::BTreeMap;
#[derive(Debug)]
pub(in crate::backend) enum SelectionError {
    Unsupported(&'static str),
    Undiscovered(crate::backend::plan::ArtifactId),
    Inventory(crate::backend::lir::ProgramError),
    Abi(AbiError),
    Build(SelectedBuildError),
    Verify(Vec<SelectedFailure>),
    Plan(PlanError),
}
impl From<AbiError> for SelectionError {
    fn from(e: AbiError) -> Self {
        Self::Abi(e)
    }
}
impl From<SelectedBuildError> for SelectionError {
    fn from(e: SelectedBuildError) -> Self {
        Self::Build(e)
    }
}
impl From<PlanError> for SelectionError {
    fn from(e: PlanError) -> Self {
        Self::Plan(e)
    }
}

/// Numeric corrections, ABI events and trace memory cells precede publication.
pub(in crate::backend) fn select<'p>(
    context: &'p SelectionContext<'p>,
    body: &VerifiedCallable<'p>,
) -> Result<VerifiedSelectedCallable<'p, Instruction>, SelectionError> {
    let requests = super::requests::requests(body)?;
    for key in &requests {
        context.catalog().artifact(*key, key.category())?;
    }
    let lower = body.draft();
    for (_, block) in lower.blocks() {
        for instruction in &block.instructions {
            match instruction.operation {
                Operation::Convert {
                    conversion: crate::backend::lir::Conversion::PointerBits,
                    ..
                }
                | Operation::ScaledIndex { .. } => {
                    return Err(SelectionError::Unsupported(
                        "native pointer conversion/scaled address recipe",
                    ))
                }
                _ => {}
            }
        }
    }
    let owner = body.receipt().owner();
    let verifier = Verifier::new(owner.context().profile()).map_err(SelectionError::Unsupported)?;
    let abi = classify(
        owner.context(),
        owner.signature_id(),
        &verifier.resources,
        CallArity::Fixed,
    )?;
    let bindings = context.abi_bindings(
        owner.signature_id(),
        abi.entry().inputs().to_vec(),
        abi.entry().results().to_vec(),
    )?;
    let mut builder = SelectedBuilder::new(context, owner.key(), Some(body))?;
    let mut values = BTreeMap::new();
    let mut types = BTreeMap::new();
    let mut spans = BTreeMap::new();
    let mut provenance = BTreeMap::new();
    for (id, value) in lower.values() {
        let representation =
            Representation::from_scalar(value.ty, 64).ok_or(PlanError::InvalidSignature)?;
        let selected = builder.value(representation, value.origin)?;
        builder.map_origin(body, id, selected)?;
        values.insert(id, (selected, representation));
        types.insert(id, value.ty);
        spans.insert(id, value.origin);
        provenance.insert(id, value.provenance);
    }
    let mut objects = BTreeMap::new();
    let mut lifetimes = BTreeMap::new();
    for (id, object) in lower.objects() {
        if object.layout.disposition != LayoutDisposition::Addressable {
            continue;
        }
        let selected = builder.object(
            object.layout,
            if object.role == crate::backend::lir::ObjectRole::TraceRecord {
                crate::backend::selected::ObjectRole::Trace
            } else {
                crate::backend::selected::ObjectRole::Semantic
            },
            object.origin,
        )?;
        builder.map_object_origin(body, id, selected)?;
        objects.insert(id, selected);
        lifetimes.insert(id, object.lifetime);
    }
    let mut blocks = BTreeMap::new();
    for (id, block) in lower.blocks() {
        let params = block
            .parameters
            .as_ref()
            .expect("verified lower parameters")
            .iter()
            .map(|id| values[id].0)
            .collect::<Vec<_>>();
        let selected = builder.block(&params, None)?;
        builder.map_block_origin(body, id, selected, None)?;
        blocks.insert(id, selected);
    }
    let inputs = lower
        .inputs()
        .iter()
        .map(|id| values[id].0)
        .collect::<Vec<_>>();
    builder.entry(
        blocks[&lower.entry().expect("verified entry")],
        &inputs,
        bindings,
    )?;
    let value = |id: &LoweredValueId| {
        let (value, representation) = values[id];
        ValueRef {
            value: value.id(),
            representation,
        }
    };
    let region = |id: &LoweredValueId| match provenance[id] {
        AddressProvenance::Object { object, .. } => MemoryRegion::Object(objects[&object].id()),
        AddressProvenance::Static { field, .. } => MemoryRegion::Static(field),
        AddressProvenance::Unknown => MemoryRegion::Unknown,
    };
    for (id, block) in lower.blocks() {
        let mut selected = blocks[&id];
        for (ordinal, instruction) in block.instructions.iter().enumerate() {
            let out = instruction.results.first().map(value);
            let origin = Origin {
                site: Site::Instruction { block: id, ordinal },
                span: instruction.results.first().and_then(|id| spans[id]),
            };
            let domain = |relation, evidence: &crate::backend::lir::ScalarDomainEvidence| {
                super::numeric::Domain {
                    relation,
                    evidence: match evidence {
                        crate::backend::lir::ScalarDomainEvidence::ExactConstant(v) => {
                            crate::backend::lir::ScalarDomainEvidence::ExactConstant(
                                values[v].0.id(),
                            )
                        }
                        crate::backend::lir::ScalarDomainEvidence::SuccessCheck(b) => {
                            crate::backend::lir::ScalarDomainEvidence::SuccessCheck(blocks[b].id())
                        }
                    },
                }
            };
            match &instruction.operation {
                Operation::Trace(action) => {
                    let action = match action {
                        crate::backend::lir::TraceAction::PushFrame { record } => {
                            crate::backend::lir::TraceAction::PushFrame {
                                record: objects[record].id(),
                            }
                        }
                        crate::backend::lir::TraceAction::PopFrame { record } => {
                            crate::backend::lir::TraceAction::PopFrame {
                                record: objects[record].id(),
                            }
                        }
                        crate::backend::lir::TraceAction::ReplaceLocation {
                            record,
                            location,
                            site,
                        } => crate::backend::lir::TraceAction::ReplaceLocation {
                            record: objects[record].id(),
                            location: *location,
                            site: site.clone(),
                        },
                    };
                    super::recipes::Recipes {
                        builder: &mut builder,
                        resources: &verifier.resources,
                        origin,
                    }
                    .trace(
                        selected,
                        &action,
                        lower.trace_plan().and_then(|p| p.initial_location),
                    )?;
                    continue;
                }
                Operation::Divide {
                    result,
                    dividend,
                    divisor,
                    evidence,
                } => {
                    let proof = domain(
                        crate::backend::lir::ScalarCheck::NonZeroDivisor {
                            ty: types[dividend],
                            divisor: value(divisor).value,
                        },
                        evidence,
                    );
                    selected = super::recipes::Recipes {
                        builder: &mut builder,
                        resources: &verifier.resources,
                        origin,
                    }
                    .divide(
                        selected,
                        types[dividend],
                        *result,
                        [value(dividend), value(divisor), out.unwrap()],
                        proof,
                    )?;
                    continue;
                }
                Operation::Shift {
                    direction,
                    value: input,
                    count,
                    evidence,
                } => {
                    let proof = domain(
                        crate::backend::lir::ScalarCheck::ShiftCountBelowWidth {
                            count: value(count).value,
                            width: value(input).representation.bits() as u8,
                        },
                        evidence,
                    );
                    super::recipes::Recipes {
                        builder: &mut builder,
                        resources: &verifier.resources,
                        origin,
                    }
                    .shift(
                        selected,
                        *direction,
                        value(input),
                        value(count),
                        out.unwrap(),
                        proof,
                    )?;
                    continue;
                }
                Operation::Convert {
                    conversion,
                    value: input,
                    target,
                    evidence,
                } => {
                    let proof = evidence.as_ref().map(|e| {
                        domain(
                            crate::backend::lir::ScalarCheck::FiniteTruncatedF64InIntegerRange {
                                source: value(input).value,
                                target: *target,
                            },
                            e,
                        )
                    });
                    selected = super::recipes::Recipes {
                        builder: &mut builder,
                        resources: &verifier.resources,
                        origin,
                    }
                    .convert(
                        selected,
                        *conversion,
                        (types[input], *target),
                        value(input),
                        out.unwrap(),
                        proof,
                    )?;
                    continue;
                }
                _ => {}
            }
            let opcode = match &instruction.operation {
                Operation::Call(call) => Opcode::Call(Box::new(native_call(
                    owner.context(),
                    &verifier.resources,
                    call,
                    instruction.results.iter().map(value).collect(),
                    &value,
                )?)),
                Operation::Constant(constant) => Opcode::Constant {
                    constant: *constant,
                    out: out.expect("constant result"),
                },
                Operation::Unary {
                    operation,
                    value: input,
                } => Opcode::Unary {
                    operation: *operation,
                    input: value(input),
                    out: out.expect("unary result"),
                },
                Operation::Binary {
                    operation,
                    left,
                    right,
                } => Opcode::Alu {
                    operation: *operation,
                    left: value(left),
                    right: value(right),
                    out: out.expect("binary result"),
                },
                Operation::Compare {
                    predicate,
                    left,
                    right,
                } => {
                    if types[left] == ScalarType::F64 {
                        Opcode::FloatCompare {
                            condition: float_condition(*predicate),
                            predicate: *predicate,
                            left: value(left),
                            right: value(right),
                            out: out.expect("comparison result"),
                        }
                    } else {
                        let signed = types[left] == ScalarType::I64;
                        Opcode::IntegerCompare {
                            condition: integer_condition(*predicate, signed),
                            predicate: *predicate,
                            signed,
                            left: value(left),
                            right: value(right),
                            out: out.expect("comparison result"),
                        }
                    }
                }
                Operation::SymbolAddress { symbol, .. } => Opcode::SymbolAddress {
                    symbol: *symbol,
                    out: out.expect("symbol result"),
                },
                Operation::ObjectAddress(object) => Opcode::ObjectAddress {
                    object: objects[object].id(),
                    out: out.expect("object address result"),
                },
                Operation::ByteOffset { base, offset } => Opcode::ByteOffset {
                    base: value(base),
                    offset: value(offset),
                    out: out.expect("address result"),
                },
                Operation::Load {
                    address,
                    representation,
                } => Opcode::Load {
                    address: value(address),
                    out: out.expect("load result"),
                    bytes: representation.bytes,
                    alignment: representation.alignment,
                    region: region(address),
                },
                Operation::Store {
                    address,
                    value: input,
                    representation,
                } => Opcode::Store {
                    address: value(address),
                    value: value(input),
                    bytes: representation.bytes,
                    alignment: representation.alignment,
                    region: region(address),
                },
                Operation::Lifetime {
                    marker,
                    object,
                    site,
                } => Opcode::Lifetime {
                    object: objects[object].id(),
                    marker: *marker,
                    site: *site,
                    sites: match lifetimes[object] {
                        crate::backend::lir::LifetimeDisposition::Sites(sites) => sites,
                        crate::backend::lir::LifetimeDisposition::WholeCallable => {
                            unreachable!("verified lifetime site")
                        }
                    },
                },
                Operation::Trace(_)
                | Operation::Divide { .. }
                | Operation::Shift { .. }
                | Operation::Convert { .. }
                | Operation::ScaledIndex { .. } => {
                    unreachable!("numeric and trace recipes are consumed before ordinary cells")
                }
            };
            let origin = Origin {
                site: Site::Instruction { block: id, ordinal },
                span: instruction.results.first().and_then(|id| spans[id]),
            };
            builder.append(
                selected,
                Instruction::new(opcode, origin, &verifier.resources),
            )?;
        }
        let terminator = block.terminator.as_ref().expect("verified terminator");
        let (opcode, mut edges) = match terminator {
            Terminator::ScalarCheck {
                relation,
                success,
                failure,
            } => {
                let source = match relation {
                    crate::backend::lir::ScalarCheck::NonZeroDivisor { divisor, .. } => {
                        value(divisor)
                    }
                    crate::backend::lir::ScalarCheck::ShiftCountBelowWidth { count, .. } => {
                        value(count)
                    }
                    crate::backend::lir::ScalarCheck::FiniteTruncatedF64InIntegerRange {
                        source,
                        ..
                    } => value(source),
                };
                let relation = match *relation {
                    crate::backend::lir::ScalarCheck::NonZeroDivisor { ty, divisor } => {
                        crate::backend::lir::ScalarCheck::NonZeroDivisor {
                            ty,
                            divisor: value(&divisor).value,
                        }
                    }
                    crate::backend::lir::ScalarCheck::ShiftCountBelowWidth { count, width } => {
                        crate::backend::lir::ScalarCheck::ShiftCountBelowWidth {
                            count: value(&count).value,
                            width,
                        }
                    }
                    crate::backend::lir::ScalarCheck::FiniteTruncatedF64InIntegerRange {
                        source,
                        target,
                    } => crate::backend::lir::ScalarCheck::FiniteTruncatedF64InIntegerRange {
                        source: value(&source).value,
                        target,
                    },
                };
                let condition = super::recipes::Recipes {
                    builder: &mut builder,
                    resources: &verifier.resources,
                    origin: Origin {
                        site: Site::Terminator(id),
                        span: None,
                    },
                }
                .check(selected, &relation, source)?;
                (
                    Opcode::CheckBranch {
                        condition,
                        relation,
                    },
                    [success, failure]
                        .iter()
                        .map(|e| {
                            (
                                blocks[&e.target],
                                e.arguments.iter().map(|v| values[v].0).collect(),
                            )
                        })
                        .collect(),
                )
            }
            Terminator::NonReturningCall(call) => (
                Opcode::Call(Box::new(native_call(
                    owner.context(),
                    &verifier.resources,
                    call,
                    vec![],
                    &value,
                )?)),
                vec![],
            ),
            Terminator::HardTrap => (Opcode::HardTrap, vec![]),
            Terminator::ReportFailure { call, reason } => {
                let classified = classify(
                    owner.context(),
                    call.signature,
                    &verifier.resources,
                    CallArity::Fixed,
                )?;
                (
                    Opcode::Failure {
                        reason: *reason,
                        signature: call.signature,
                        arguments: call.arguments.iter().map(|a| value(&a.value)).collect(),
                        bindings: classified.call().inputs().to_vec(),
                        attribution: call.attribution.clone(),
                    },
                    vec![],
                )
            }
            Terminator::Jump(edge) => (
                Opcode::Jump,
                vec![(
                    blocks[&edge.target],
                    edge.arguments.iter().map(|id| values[id].0).collect(),
                )],
            ),
            Terminator::Branch {
                condition,
                true_edge,
                false_edge,
            } => (
                Opcode::Branch {
                    condition: value(condition),
                },
                [true_edge, false_edge]
                    .into_iter()
                    .map(|edge| {
                        (
                            blocks[&edge.target],
                            edge.arguments.iter().map(|id| values[id].0).collect(),
                        )
                    })
                    .collect::<Vec<(
                        LocalHandle<'p, SelectedBlockId>,
                        Vec<LocalHandle<'p, SelectedValueId>>,
                    )>>(),
            ),
            Terminator::Return(results) => (
                Opcode::Return {
                    values: results.iter().map(value).collect(),
                    bindings: abi.entry().results().to_vec(),
                },
                vec![],
            ),
        };
        if edges.len() > 1 {
            for (slot, (target, arguments)) in edges.iter_mut().enumerate() {
                if arguments.is_empty() {
                    continue;
                }
                let forward = builder.block(&[], None)?;
                let forwarded = std::mem::take(arguments);
                builder.terminate(
                    forward,
                    Instruction::new(
                        Opcode::Jump,
                        Origin {
                            site: Site::Edge { block: id, slot },
                            span: None,
                        },
                        &verifier.resources,
                    ),
                    &[(*target, forwarded)],
                )?;
                *target = forward;
            }
        }
        builder.terminate(
            selected,
            Instruction::new(
                opcode,
                Origin {
                    site: Site::Terminator(id),
                    span: None,
                },
                &verifier.resources,
            ),
            &edges,
        )?;
    }
    let selected = verify_selected(builder.finish(), &verifier).map_err(SelectionError::Verify)?;
    super::requests::check_references(&requests, selected.receipt().references())?;
    Ok(selected)
}

impl From<crate::backend::lir::ProgramError> for SelectionError {
    fn from(e: crate::backend::lir::ProgramError) -> Self {
        Self::Inventory(e)
    }
}
impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Undiscovered(key) => write!(
                f,
                "native selection introduced undiscovered artifact: {key:?}"
            ),
            Self::Unsupported(reason) => write!(f, "unsupported native selection: {reason}"),
            Self::Abi(error) => write!(f, "native ABI invariant: {error:?}"),
            Self::Build(error) => write!(f, "native selected construction: {error:?}"),
            Self::Verify(errors) => write!(f, "native selected publication: {errors:?}"),
            Self::Plan(error) => write!(f, "native planning invariant: {error:?}"),
            Self::Inventory(error) => write!(f, "native inventory invariant: {error:?}"),
        }
    }
}
impl std::error::Error for SelectionError {}

fn native_call(
    plan: crate::backend::plan::PlanView<'_>,
    resources: &super::super::NativeResources,
    call: &crate::backend::lir::Call,
    results: Vec<ValueRef>,
    value: &impl Fn(&LoweredValueId) -> ValueRef,
) -> Result<super::calls::NativeCall, SelectionError> {
    let abi = classify(plan, call.signature, resources, CallArity::Fixed)?;
    let target = match call.target {
        crate::backend::lir::CallTarget::Direct(target) => {
            crate::backend::lir::CallTarget::Direct(target)
        }
        crate::backend::lir::CallTarget::Indirect(target) => {
            crate::backend::lir::CallTarget::Indirect(value(&target))
        }
    };
    Ok(super::calls::NativeCall {
        target,
        signature: call.signature,
        arguments: call.arguments.iter().map(|a| value(&a.value)).collect(),
        results,
        inputs: abi.call().inputs().to_vec(),
        outputs: abi.call().results().to_vec(),
        attribution: call.attribution.clone(),
        never: abi.noreturn(),
    })
}

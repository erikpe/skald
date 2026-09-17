use super::*;
use crate::backend::{
    graph::{LocalHandle, SelectedBlockId, SelectedValueId},
    plan::{
        ArtifactCategory, ArtifactId, Component, ComponentRole, PlanFacts, ReturnShape, ScalarType,
        SignatureId,
    },
};
mod editing;
mod inspection;
mod malformed;
mod streaming;
mod target;
mod tracing;
mod witnesses;
use target::{Corruption, Node, Op, Shape, ValueRef, WitnessTarget};
type VH<'p> = LocalHandle<'p, SelectedValueId>;
type BH<'p> = LocalHandle<'p, SelectedBlockId>;
fn vf(value: VH<'_>, ty: Representation) -> ValueRef {
    ValueRef { id: value.id(), ty }
}
fn supplied(shape: Shape) -> PlanFacts {
    let mut f = facts();
    if shape == Shape::Three {
        f.profile.architecture = plan::Architecture::Aarch64;
        f.profile.abi = plan::Abi::Aapcs64;
    }
    f.signatures[0].inputs = (0..2)
        .map(|i| Component {
            ty: ScalarType::I64,
            role: ComponentRole::Parameter(i),
        })
        .collect();
    let mut callee = f.signatures[0].clone();
    callee.results = vec![Component {
        ty: ScalarType::I64,
        role: ComponentRole::Result,
    }];
    callee.returns = ReturnShape::Scalar(ScalarType::I64);
    f.callables[1].signature = f.add_signature(callee).unwrap();
    f
}
fn inventory<'p>(
    plan: &'p CheckedPlan,
) -> (lir::VerifiedProgram<'p>, [lir::VerifiedCallable<'p>; 2]) {
    let bodies = [lower(plan, source(0)), lower(plan, source(1))];
    let mut p = lir::ProgramBuilder::new(plan.view());
    for body in &bodies {
        p.begin(body.receipt().owner().key()).unwrap();
        p.complete(body, &body.receipt()).unwrap();
    }
    (p.finish().unwrap(), bodies)
}
fn resources() -> (ResourceCatalog, Vec<ViewId>, Vec<UnitId>) {
    let mut r = ResourceCatalog::default();
    let b = r.bank(BankKind::Integer);
    let mut views = vec![];
    let mut units = vec![];
    for _ in 0..4 {
        let u = r.unit().unwrap();
        units.push(u);
        views.push(r.view(b, 64, &[u], false).unwrap());
    }
    (r, views, units)
}
fn context<'p>(
    extension: &'p lir::TargetCatalog<'p>,
    resources: ResourceCatalog,
    inputs: Vec<Representation>,
) -> SelectionContext<'p> {
    SelectionContext::new(extension, resources)
        .with_abi_areas(AbiAreas {
            incoming: inputs.clone(),
            outgoing: inputs,
            results: vec![repr()],
        })
        .unwrap()
}
fn begin<'p>(
    ctx: &'p SelectionContext<'p>,
    body: &lir::VerifiedCallable<'p>,
) -> (SelectedBuilder<'p, Node>, BH<'p>, Vec<VH<'p>>) {
    let key = body.receipt().owner().key();
    let declaration = ctx
        .catalog
        .plan()
        .callables()
        .find(|d| d.key == key)
        .unwrap();
    let sig = body.receipt().owner().signature().unwrap();
    let inputs: Vec<_> = sig
        .inputs
        .iter()
        .enumerate()
        .map(|(index, c)| AbiBinding {
            component: *c,
            representation: ctx.abi_areas.incoming[index],
            location: AbiLocation::Slot {
                area: AbiArea::Incoming,
                index,
            },
        })
        .collect();
    let results: Vec<_> = sig
        .results
        .iter()
        .enumerate()
        .map(|(index, c)| AbiBinding {
            component: *c,
            representation: repr(),
            location: AbiLocation::Slot {
                area: AbiArea::Results,
                index,
            },
        })
        .collect();
    let bindings = ctx
        .abi_bindings(declaration.signature, inputs.clone(), results)
        .unwrap();
    let mut b = SelectedBuilder::new(ctx, key, Some(body)).unwrap();
    let block = b.block(&[], None).unwrap();
    let values = inputs
        .iter()
        .map(|c| b.value(c.representation, None).unwrap())
        .collect::<Vec<_>>();
    b.entry(block, &values, bindings).unwrap();
    (b, block, values)
}
fn ret<'p>(
    b: &mut SelectedBuilder<'p, Node>,
    block: BH<'p>,
    values: Vec<ValueRef>,
    bindings: Vec<AbiBinding>,
    views: &[ViewId],
) {
    b.terminate(
        block,
        Node::new(Op::Return { values, bindings }, views),
        &[],
    )
    .unwrap();
}
fn checked<'p>(b: SelectedBuilder<'p, Node>, shape: Shape) -> VerifiedSelectedCallable<'p, Node> {
    let draft = b.finish();
    let target = WitnessTarget {
        profile: draft.identity().target,
        shape,
        reject: false,
    };
    let body = verify_selected(draft, &target).unwrap();
    inspection::assert_dump(&body);
    body
}
fn call_node(
    ctx: &SelectionContext<'_>,
    signature: SignatureId,
    args: Vec<ValueRef>,
    out: Vec<ValueRef>,
    artifact: ArtifactId,
    views: &[ViewId],
    units: &[UnitId],
) -> Node {
    let view = ctx.catalog.plan();
    let sig = view
        .signature(view.signature_id(signature.index()).unwrap())
        .unwrap();
    let input = sig
        .inputs
        .iter()
        .enumerate()
        .map(|(index, c)| AbiBinding {
            component: *c,
            representation: args[index].ty,
            location: AbiLocation::Slot {
                area: AbiArea::Outgoing,
                index,
            },
        })
        .collect();
    let result = sig
        .results
        .iter()
        .enumerate()
        .map(|(index, c)| AbiBinding {
            component: *c,
            representation: out[index].ty,
            location: AbiLocation::Slot {
                area: AbiArea::Results,
                index,
            },
        })
        .collect();
    let abi = ctx.abi_bindings(signature, input, result).unwrap();
    let mut node = Node::new(
        Op::Call {
            target: None,
            args,
            out,
            signature,
            abi,
            artifact,
        },
        views,
    );
    node.clobbers = units.iter().map(|u| (Timing::Late, *u)).collect();
    node
}
fn reasons(
    result: Result<VerifiedSelectedCallable<'_, Node>, Vec<SelectedFailure>>,
) -> Vec<SelectedReason> {
    result
        .err()
        .expect("must reject")
        .into_iter()
        .map(|e| e.reason)
        .collect()
}

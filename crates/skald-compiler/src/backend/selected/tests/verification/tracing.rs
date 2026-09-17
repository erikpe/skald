use super::*;

#[test]
fn enabled_trace_dependencies_and_inherited_helper_attribution_survive_publication() {
    let mut f = supplied(Shape::Three);
    f.runtime_trace = crate::backend::RuntimeTracePolicy::Enabled;
    let signature = f.callables[0].signature;
    let helper = plan::LirCallableId::Helper(plan::HelperKey {
        family: plan::HelperFamily::Release,
        layout: f.add_layout(f.layouts[0]).unwrap(),
        signature,
    });
    f.callables.push(plan::CallableDeclaration {
        key: helper,
        signature,
        body: plan::BodyDisposition::Required,
    });
    let trace_layout = f.add_layout(f.layouts[0]).unwrap();
    let trace = ArtifactId::Data(plan::DataKey::TraceContext(0));
    for key in [trace, ArtifactId::TraceTls] {
        f.artifacts.push(plan::ArtifactDeclaration {
            key,
            signature: None,
            layout: Some(trace_layout),
        });
    }
    let p = CheckedPlan::check(f).unwrap();
    let bodies = [
        lower(&p, source(0)),
        lower(&p, source(1)),
        lower(&p, helper),
    ];
    let mut inventory = lir::ProgramBuilder::new(p.view());
    for body in &bodies {
        inventory.begin(body.receipt().owner().key()).unwrap();
        inventory.complete(body, &body.receipt()).unwrap();
    }
    inventory
        .define_data(lir::DataDefinition {
            key: plan::DataKey::TraceContext(0),
            initializers: vec![lir::DataInitializer::Zero(8)],
        })
        .unwrap();
    let program = inventory.finish().unwrap();
    let extension = lir::TargetDeclarations::new(&program).freeze().unwrap();
    let (r, views, _) = resources();
    let ctx = context(&extension, r, vec![repr(); 2]);
    let (mut b, entry, args) = begin(&ctx, &bodies[2]);
    let mut action = Node::new(Op::Trace, &views);
    action.refs = vec![
        (trace, trace.category()),
        (ArtifactId::TraceTls, ArtifactCategory::Tls),
    ];
    b.append(entry, action).unwrap();
    let result = b.value(repr(), Some(origin())).unwrap();
    let callee = p
        .view()
        .callables()
        .find(|c| c.key == source(1))
        .unwrap()
        .signature;
    let mut call = call_node(
        &ctx,
        callee,
        args.iter().map(|v| vf(*v, repr())).collect(),
        vec![vf(result, repr())],
        ArtifactId::Callable(source(1)),
        &views,
        &[],
    );
    call.attribution = lir::CallAttribution::InheritedOperation { boundary: helper };
    b.append(entry, call).unwrap();
    ret(&mut b, entry, vec![], vec![], &views);
    let product = checked(b, Shape::Three);
    assert!(
        matches!(product.draft().blocks.get(entry).unwrap().instructions[1].describe().call_attribution, Some(lir::CallAttribution::InheritedOperation { boundary }) if *boundary == helper)
    );
    assert!(product
        .receipt()
        .references()
        .contains(&ArtifactId::TraceTls));
    assert!(product.receipt().references().contains(&trace));
    assert!(product
        .receipt()
        .references()
        .contains(&ArtifactId::Callable(helper)));
}

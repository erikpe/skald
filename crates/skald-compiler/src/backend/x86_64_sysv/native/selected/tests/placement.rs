//! A genuine native selected callable can be checked without stack homes.
use super::*;
use crate::backend::{
    placement::{
        Assignment, CheckReason, Location, PlacementDraft, Site as PlacementSite, Storage,
        StorageLifetime, StoragePurpose, Transfer, TransferKind, TransferPoint, TransferValue,
    },
    x86_64_sysv::native::{
        check_native_placement,
        resources::{Gpr, NativeResources},
    },
};
#[test]
fn native_register_placement_is_checked_against_canonical_target_facts() {
    let mut facts = plan::test_fixtures::facts();
    facts.signatures[0].returns = plan::ReturnShape::Scalar(plan::ScalarType::I64);
    facts.signatures[0].results = vec![plan::Component {
        ty: plan::ScalarType::I64,
        role: plan::ComponentRole::Result,
    }];
    let plan = plan::CheckedPlan::check(facts).unwrap();
    let catalog = TargetDeclarations::new(plan.view()).freeze().unwrap();
    let context = selection_context(&catalog).unwrap();
    let mut lower = lir::DraftBuilder::new(
        plan.view()
            .callable(plan::test_fixtures::source(0))
            .unwrap(),
    )
    .unwrap();
    let entry = lower.reserve_block().unwrap();
    lower.define_block(entry, &[]).unwrap();
    lower.set_entry(entry).unwrap();
    let value = lower
        .append(entry, lir::Operation::Constant(lir::Constant::I64(7)))
        .unwrap()[0];
    lower
        .terminate(entry, lir::Terminator::Return(vec![value]))
        .unwrap();
    let lower = lir::verify_callable(lower.finish()).unwrap();
    let selected = select(&context, &lower).unwrap();
    let resources = NativeResources::for_profile(plan.view().profile()).unwrap();
    for register in [Gpr::Rax, Gpr::Rbx, Gpr::Rsp] {
        let mut draft = PlacementDraft::new(&selected);
        selected
            .visit(|fact| {
                match fact {
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => {
                        for slot in 0..payload.describe().operands.len() {
                            draft
                                .assign(
                                    Assignment::Operand {
                                        site: PlacementSite::Instruction { block, ordinal },
                                        slot,
                                    },
                                    Location::Resource(resources.gpr(register, 64)?),
                                )
                                .unwrap();
                        }
                    }
                    SelectedFact::Terminal {
                        block,
                        payload: Some(payload),
                        ..
                    } => {
                        for slot in 0..payload.describe().operands.len() {
                            draft
                                .assign(
                                    Assignment::Operand {
                                        site: PlacementSite::Terminal(block),
                                        slot,
                                    },
                                    Location::Resource(resources.gpr(Gpr::Rax, 64)?),
                                )
                                .unwrap();
                        }
                    }
                    _ => {}
                }
                Ok::<_, selected::ResourceError>(())
            })
            .unwrap();
        let result = check_native_placement(draft);
        match register {
            Gpr::Rax => {
                let checked = result.unwrap();
                let frame =
                    crate::backend::x86_64_sysv::native::plan_native_frame(&checked).unwrap();
                assert_eq!(frame.bytes(), 0);
                crate::backend::x86_64_sysv::native::realize_native(&selected, &checked, &frame)
                    .unwrap();
            }
            Gpr::Rsp => assert_eq!(result.err().unwrap().reason, CheckReason::Reserved),
            Gpr::Rbx => assert_eq!(result.err().unwrap().reason, CheckReason::MissingValue),
            _ => unreachable!(),
        }
    }
}

#[test]
fn genuine_native_call_results_are_established_after_caller_clobbers() {
    let resources = NativeResources::new().unwrap();
    let rax = resources.gpr(Gpr::Rax, 64).unwrap();
    let mut calls = 0;
    for_sources(
        "fn main() -> i64 { return 7; } fn invoke() -> i64 { return main(); }",
        |context, lower| {
            let selected = select(context, lower).unwrap();
            let mut draft = PlacementDraft::new(&selected);
            selected
                .visit(|fact| {
                    match fact {
                        SelectedFact::Instruction {
                            block,
                            ordinal,
                            payload,
                        } => {
                            if matches!(payload.opcode(), Opcode::Call(_)) {
                                calls += 1;
                            }
                            for slot in 0..payload.describe().operands.len() {
                                draft
                                    .assign(
                                        Assignment::Operand {
                                            site: PlacementSite::Instruction { block, ordinal },
                                            slot,
                                        },
                                        Location::Resource(rax),
                                    )
                                    .unwrap();
                            }
                        }
                        SelectedFact::Terminal {
                            block,
                            payload: Some(payload),
                            ..
                        } => {
                            for slot in 0..payload.describe().operands.len() {
                                draft
                                    .assign(
                                        Assignment::Operand {
                                            site: PlacementSite::Terminal(block),
                                            slot,
                                        },
                                        Location::Resource(rax),
                                    )
                                    .unwrap();
                            }
                        }
                        _ => {}
                    }
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            assert!(check_native_placement(draft).is_ok());
        },
    );
    assert_eq!(calls, 1);
}

#[test]
fn native_memory_move_recipes_validate_scratch_and_kill_its_old_contents() {
    let resources = NativeResources::new().unwrap();
    let rax = resources.gpr(Gpr::Rax, 64).unwrap();
    for_sources("fn main() -> i64 { return 7; }", |context, lower| {
        let selected = select(context, lower).unwrap();
        let mut constant = None;
        selected
            .visit(|fact| {
                if let SelectedFact::Instruction {
                    block,
                    ordinal,
                    payload,
                } = fact
                {
                    if let Opcode::Constant { out, .. } = payload.opcode() {
                        constant = Some((block, ordinal, *out));
                    }
                }
                Ok::<_, std::convert::Infallible>(())
            })
            .unwrap();
        let (block, ordinal, out) = constant.unwrap();
        for scratch in [None, Some(Gpr::R10), Some(Gpr::Rax), Some(Gpr::Rsp)] {
            let mut draft = PlacementDraft::new(&selected);
            draft
                .assign(
                    Assignment::Operand {
                        site: PlacementSite::Instruction { block, ordinal },
                        slot: 0,
                    },
                    Location::Resource(rax),
                )
                .unwrap();
            draft
                .assign(
                    Assignment::Operand {
                        site: PlacementSite::Terminal(block),
                        slot: 0,
                    },
                    Location::Resource(rax),
                )
                .unwrap();
            let home = Storage {
                representation: out.representation,
                bytes: 8,
                alignment: 8,
                purpose: StoragePurpose::Home(out.value),
                lifetime: StorageLifetime::WholeCallable,
            };
            let first = draft.storage(home.clone());
            let second = draft.storage(home);
            let point = TransferPoint::After(PlacementSite::Instruction { block, ordinal });
            let transfer = |source, destination, scratch| Transfer {
                value: TransferValue::Selected(out.value),
                source,
                destination,
                source_representation: out.representation,
                destination_representation: out.representation,
                kind: TransferKind::Copy,
                scratch,
            };
            draft.transfer(
                point,
                transfer(Location::Resource(rax), Location::Storage(first), vec![]),
            );
            draft.transfer(
                point,
                transfer(
                    Location::Storage(first),
                    Location::Storage(second),
                    scratch
                        .map(|register| resources.gpr(register, 64).unwrap())
                        .into_iter()
                        .collect(),
                ),
            );
            let result = check_native_placement(draft);
            match scratch {
                Some(Gpr::R10) => {
                    let checked = result.unwrap();
                    let frame =
                        crate::backend::x86_64_sysv::native::plan_native_frame(&checked).unwrap();
                    assert_eq!(frame.bytes(), 16);
                    assert_eq!(frame.storage(first).unwrap().offset, -8);
                    assert_eq!(frame.storage(second).unwrap().offset, -16);
                    crate::backend::x86_64_sysv::native::realize_native(
                        &selected, &checked, &frame,
                    )
                    .unwrap();
                }
                Some(Gpr::Rax) => {
                    assert_eq!(result.err().unwrap().reason, CheckReason::MissingValue)
                }
                _ => assert_eq!(result.err().unwrap().reason, CheckReason::Scratch),
            }
        }
    });
}

#[test]
fn baseline_accepts_complete_native_pilot_with_calls_pressure_tracing_and_entry() {
    use crate::backend::x86_64_sysv::native::place_native_baseline;
    let sources=[
        "extern fn foreign(a:i64,b:f64)->i64; fn identity(a:i64)->i64{return a;} fn invoke(f:fn(i64)->i64,a:i64)->i64{return f(a)+a;} fn main()->i64{return foreign(invoke(identity,7),2.5);}",
        "fn calculate(a:i64,b:i64,c:u64,f:f64)->i64 {var remaining:i64=a; var result:i64=1; while(remaining>0){result=result/b + result%b; result=result<<c; result=result+(i64)f; remaining=remaining-1;} return result;} fn main()->i64{return 0;}",
        "fn compare(a:f64,b:f64)->bool{return a != b;} fn negate(a:f64)->f64{return -a;} fn byte(a:u8,b:u8)->u8{return a*b;} fn main()->i64{return 0;}",
        "fn ordinary(a:u64,b:u64)->u64{return ((a&b)|(a^b))+(a-b)*b;} fn float_math(a:f64,b:f64)->f64{return (a+b)*(a-b)/b;} fn divide(a:u8,b:u8)->u8{return a/b+a%b;} fn unsigned(a:u64,b:u64)->u64{return a/b+a%b;} fn logical(a:bool)->bool{return !a;} fn main()->i64{return 0;}",
        "fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:f64,i:f64,j:f64,k:f64,l:f64,m:f64,n:f64,o:f64,p:f64)->f64{return (f64)g+p;} fn main()->i64{return (i64)pressure(1,2,3,4,5,6,7,1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0);}",
        "extern fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:f64,i:f64,j:f64,k:f64,l:f64,m:f64,n:f64,o:f64,p:f64)->f64; fn main()->i64{return (i64)pressure(1,2,3,4,5,6,7,1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0);}",
    ];
    for source in sources {
        for trace in [false, true] {
            super::calls::complete(source, trace, |context, lower| {
                let selected = select(context, lower).unwrap();
                let checked =
                    place_native_baseline(&selected).unwrap_or_else(|e| panic!("{e:?}\n{source}"));
                checked.require_selected(&selected).unwrap();
                let homes: Vec<_> = checked
                    .storage()
                    .iter()
                    .filter_map(|s| {
                        if let StoragePurpose::Home(v) = s.purpose {
                            Some(v)
                        } else {
                            None
                        }
                    })
                    .collect();
                let unique: std::collections::BTreeSet<_> = homes.iter().collect();
                assert_eq!(homes.len(), unique.len());
            });
        }
    }
    for from in ["i64", "u64", "u8", "bool", "f64"] {
        for to in ["i64", "u64", "u8", "bool", "f64"] {
            let source = format!(
                "fn convert(value:{from})->{to}{{return ({to})value;}} fn main()->i64{{return 0;}}"
            );
            super::calls::complete(&source, false, |context, lower| {
                let selected = select(context, lower).unwrap();
                place_native_baseline(&selected).unwrap_or_else(|e| panic!("{e:?}: {from}->{to}"));
            });
        }
    }
}

use super::super::{
    place_native_baseline, plan_native_frame, realize_native,
    selected::{select, selection_context},
};
use super::model::*;
use crate::backend::{
    frame::{plan_frame, FramePolicy, ReturnAddress},
    lir::TargetDeclarations,
    lowering::lower_program,
    planning::plan_program,
    selected::SelectedProgramBuilder,
    BackendInput,
};
use std::collections::BTreeMap;

#[derive(Clone, Copy)]
enum StoreFailure {
    Write,
    Read,
    Remove,
}
struct FailingStore {
    failure: StoreFailure,
    fragments: BTreeMap<crate::backend::plan::LirCallableId, String>,
}
impl super::program::FragmentStore for FailingStore {
    fn write(
        &mut self,
        key: crate::backend::plan::LirCallableId,
        text: &str,
    ) -> std::io::Result<()> {
        if matches!(self.failure, StoreFailure::Write) {
            return Err(std::io::Error::other("injected write failure"));
        }
        self.fragments.insert(key, text.to_owned());
        Ok(())
    }
    fn read(&self, key: crate::backend::plan::LirCallableId) -> std::io::Result<String> {
        if matches!(self.failure, StoreFailure::Read) {
            return Err(std::io::Error::other("injected read failure"));
        }
        Ok(self.fragments[&key].clone())
    }
    fn remove(&mut self, key: crate::backend::plan::LirCallableId) -> std::io::Result<()> {
        if matches!(self.failure, StoreFailure::Remove) {
            return Err(std::io::Error::other("injected cleanup failure"));
        }
        self.fragments.remove(&key);
        Ok(())
    }
}

pub(super) fn complete(
    source: &str,
    trace: bool,
    mut check: impl FnMut(&PhysicalDraft<'_, '_, '_, '_>),
) {
    let fixture = crate::test_support::lower_source_to_complete_final_mir_with_sources(
        "physical.ska",
        source,
    );
    let input = if trace {
        BackendInput::with_runtime_trace(&fixture.mir, &fixture.sources)
    } else {
        BackendInput::without_runtime_trace(&fixture.mir)
    };
    let planned = plan_program(input).unwrap();
    let catalog = TargetDeclarations::new(planned.plan().view())
        .freeze()
        .unwrap();
    let context = selection_context(&catalog).unwrap();
    let external_symbols = fixture
        .mir
        .program()
        .external_links
        .iter()
        .map(|link| (link.id, link.symbol.clone()))
        .collect::<BTreeMap<_, _>>();
    if !external_symbols.is_empty() {
        assert!(matches!(
            super::super::PhysicalProgramBuilder::temporary(&context),
            Err(super::ProgramError::MissingDefinition(
                crate::backend::plan::ArtifactId::External(_)
            ))
        ));
    }
    let mut selected_program = SelectedProgramBuilder::new(&context);
    let mut replacement_program = SelectedProgramBuilder::new(&context);
    let mut physical_program =
        super::super::PhysicalProgramBuilder::temporary_with_external_symbols(
            &context,
            external_symbols.clone(),
        )
        .unwrap();
    let mut wrong_parent_program =
        super::super::PhysicalProgramBuilder::temporary_with_external_symbols(
            &context,
            external_symbols.clone(),
        )
        .unwrap();
    let mut read_failure = super::program::PhysicalProgramBuilder::with_store(
        &context,
        FailingStore {
            failure: StoreFailure::Read,
            fragments: BTreeMap::new(),
        },
        external_symbols.clone(),
    );
    let mut cleanup_failure = super::program::PhysicalProgramBuilder::with_store(
        &context,
        FailingStore {
            failure: StoreFailure::Remove,
            fragments: BTreeMap::new(),
        },
        external_symbols.clone(),
    );
    let mut duplicate_checked = false;
    let mut write_failure = Some(super::program::PhysicalProgramBuilder::with_store(
        &context,
        FailingStore {
            failure: StoreFailure::Write,
            fragments: BTreeMap::new(),
        },
        external_symbols.clone(),
    ));
    let parent = lower_program(&planned, |lower| {
        let selected = select(&context, &lower).unwrap();
        selected_program
            .complete(&selected, &selected.receipt())
            .unwrap();
        let replacement = select(&context, &lower).unwrap();
        replacement_program
            .complete(&replacement, &replacement.receipt())
            .unwrap();
        let placement = place_native_baseline(&selected).unwrap();
        let frame = plan_native_frame(&placement).unwrap();
        let draft = realize_native(&selected, &placement, &frame).unwrap();
        let mut bounds = std::collections::BTreeMap::new();
        selected
            .visit::<()>(|fact| {
                use crate::backend::selected::{Bundle, Payload, SelectedFact};
                let located = match fact {
                    SelectedFact::Instruction {
                        block,
                        ordinal,
                        payload,
                    } => Some((
                        crate::backend::placement::Site::Instruction { block, ordinal },
                        payload,
                    )),
                    SelectedFact::Terminal {
                        block,
                        payload: Some(payload),
                        ..
                    } => Some((crate::backend::placement::Site::Terminal(block), payload)),
                    _ => None,
                };
                if let Some((site, payload)) = located {
                    if let Some(Bundle::Bounded { steps, .. }) = payload.describe().bundle {
                        bounds.insert(site, usize::from(steps.get()));
                    }
                }
                Ok(())
            })
            .unwrap();
        assert!(std::ptr::eq(draft.frame, &frame));
        assert_eq!(draft.blocks[draft.entry.0].origin, BlockOrigin::Entry);
        assert!(draft.blocks[draft.entry.0]
            .groups
            .iter()
            .any(|g| g.origin == Origin::Prologue));
        for (index, block) in draft.blocks.iter().enumerate() {
            assert_eq!(block.id, BlockId(index));
            for group in &block.groups {
                if let Origin::Selected(site) = group.origin {
                    if let Some(bound) = bounds.get(&site) {
                        assert!(group.instructions.len() <= *bound);
                    }
                }
                for op in &group.instructions {
                    match op {
                        Instruction::Jump(target)
                        | Instruction::JumpIf {
                            destination: target,
                            ..
                        } => assert!(target.0 < draft.blocks.len()),
                        _ => {}
                    }
                }
                for dep in &group.dependencies {
                    assert!(catalog.artifact(*dep, dep.category()).is_ok());
                }
            }
        }
        let verified =
            super::verify::check_native_physical(draft, &selected, &placement, &frame).unwrap();
        physical_program.complete(&verified).unwrap();
        wrong_parent_program.complete(&verified).unwrap();
        read_failure.complete(&verified).unwrap();
        cleanup_failure.complete(&verified).unwrap();
        if let Some(mut failing) = write_failure.take() {
            assert!(matches!(
                failing.complete(&verified),
                Err(super::ProgramError::Store(_))
            ));
        }
        if !duplicate_checked {
            assert!(matches!(
                physical_program.complete(&verified),
                Err(super::ProgramError::DuplicateDefinition)
            ));
            duplicate_checked = true;
        }
        assemble(&verified);
        check(verified.body());
        let mut quiet = String::new();
        verified
            .inspect(&mut quiet, super::super::Inspection::default())
            .unwrap();
        let mut again = String::new();
        verified
            .inspect(&mut again, super::super::Inspection::default())
            .unwrap();
        assert_eq!(quiet, again);
        assert!(!quiet.contains("\nentry b"));
        assert!(!quiet.contains("\nframe bytes="));
        assert!(!quiet.contains("\nassignment "));
        let receipt: super::super::PhysicalReceipt<'_> = verified.receipt();
        assert!(receipt.parent().same_snapshot(&selected.receipt()));
        let mut visits = 0;
        verified.visit(|fact| match fact {
            super::super::PhysicalFact::Entry(entry) => assert_eq!(entry, verified.body().entry.0),
            super::super::PhysicalFact::Block(id) => assert!(id < verified.body().blocks.len()),
            super::super::PhysicalFact::Instruction(instruction) => {
                let _ = format!("{instruction:?}");
                visits += 1;
            }
        });
        assert!(visits > 0);
        let mut observed = String::new();
        verified
            .inspect(
                &mut observed,
                super::super::Inspection {
                    physical: true,
                    placement: true,
                    frame: true,
                },
            )
            .unwrap();
        assert!(observed.contains("\nframe bytes="));
        assert!(observed.contains("\nassignment "));
        assert!(receipt.matches(&verified));
        assert_eq!(receipt.require_parent(&selected.receipt()), Ok(()));
        assert_eq!(receipt.references(), selected.receipt().references());
        let other = place_native_baseline(&selected).unwrap();
        assert_eq!(
            realize_native(&selected, &other, &frame).err(),
            Some(RealizeError::Frame(
                crate::backend::frame::FrameError::WrongPlacement
            ))
        );
        let another = select(&context, &lower).unwrap();
        let stale = realize_native(&selected, &placement, &frame).unwrap();
        assert_eq!(
            super::verify::check_native_physical(stale, &another, &placement, &frame)
                .err()
                .unwrap()
                .reason,
            super::verify::Reason::Provenance
        );
        assert_eq!(
            realize_native(&another, &placement, &frame).err(),
            Some(RealizeError::WrongSelected)
        );
        let unsupported = plan_frame(
            &placement,
            FramePolicy {
                alignment: 16,
                entry_remainder: 8,
                header_bytes: 8,
                incoming_base: 16,
                return_address: ReturnAddress::Stack {
                    offset: 8,
                    bytes: 8,
                },
                max_frame: i32::MAX as usize,
                direct_min: i32::MIN as i64,
                direct_max: i32::MAX as i64,
                materialization: Some((i32::MIN as i64, i32::MAX as i64, 2)),
                address_scratch_group: 0,
            },
        )
        .unwrap();
        assert_eq!(
            realize_native(&selected, &placement, &unsupported).err(),
            Some(RealizeError::Frame(
                crate::backend::frame::FrameError::InvalidPolicy
            ))
        );
        Ok(())
    })
    .unwrap();
    let selected_parent = selected_program.finish(&parent).unwrap();
    let replacement_parent = replacement_program.finish(&parent).unwrap();
    assert!(matches!(
        wrong_parent_program.finish(&replacement_parent),
        Err(super::ProgramError::Inventory(
            crate::backend::lir::ProgramError::StaleReceipt
        ))
    ));
    assert!(matches!(
        read_failure.finish(&selected_parent),
        Err(super::ProgramError::Store(_))
    ));
    assert!(matches!(
        cleanup_failure.finish(&selected_parent),
        Err(super::ProgramError::Store(_))
    ));
    let missing = super::super::PhysicalProgramBuilder::temporary_with_external_symbols(
        &context,
        external_symbols,
    )
    .unwrap();
    assert!(matches!(
        missing.finish(&selected_parent),
        Err(super::ProgramError::MissingDefinition(_))
    ));
    let assembly: super::super::VerifiedAssembly =
        physical_program.finish(&selected_parent).unwrap();
    assert!(assembly.as_str().starts_with(".intel_syntax noprefix\n"));
    let assembly = assembly.into_string();
    crate::test_support::assert_system_assembler_accepts(&assembly);
}
#[test]
fn concrete_scalar_calls_loops_transfers_and_frames_preserve_derivation() {
    let source = "fn sum(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{return a+b+c+d+e+f+g+h;} fn main()->i64{var a:i64=0;while(a<3){a=a+1;}return sum(a,2,3,4,5,6,7,8);}";
    for trace in [false, true] {
        let mut calls = 0;
        let mut returns = 0;
        complete(source, trace, |draft| {
            for block in &draft.blocks {
                for group in &block.groups {
                    for op in &group.instructions {
                        match op {
                            Instruction::Call(_) => calls += 1,
                            Instruction::Return => returns += 1,
                            _ => {}
                        }
                    }
                }
            }
        });
        assert!(calls >= 2);
        assert!(returns >= 3);
    }
}

#[test]
fn external_linker_names_are_explicit_typed_closure_inputs() {
    complete(
        "extern fn observe(value:i64)->i64; fn main()->i64{return observe(7);}",
        false,
        |_| {},
    );
}

#[test]
fn bounded_numeric_cells_and_trace_accesses_are_concrete() {
    let source = "fn numeric(a:i64,b:i64,c:u64,x:f64)->i64{var d:i64=a/b;var v:i64=a<<c;return d+v+(i64)x;} fn main()->i64{return numeric(-9,2,1u,3.5);}";
    for trace in [false, true] {
        let mut divides = 0;
        let mut conversions = 0;
        let mut tls = 0;
        let mut forwarding = 0;
        complete(source, trace, |draft| {
            for block in &draft.blocks {
                if matches!(block.origin, BlockOrigin::Forward { .. }) {
                    forwarding += 1;
                }
                for group in &block.groups {
                    for op in &group.instructions {
                        match op {
                            Instruction::Divide { .. } => divides += 1,
                            Instruction::Convert { .. } => conversions += 1,
                            Instruction::TlsOffset { .. } => tls += 1,
                            _ => {}
                        }
                    }
                }
            }
        });
        assert!(forwarding > 0);
        assert!(divides > 0);
        assert!(conversions > 0);
        assert_eq!(tls > 0, trace);
    }
}

// Verified callable formatting is an encoding witness, not program publication.
fn assemble(verified: &super::super::VerifiedPhysicalCallable<'_, '_, '_, '_>) {
    let draft = verified.body();
    use std::collections::BTreeSet;
    use std::fmt::Write;
    let dependencies = draft
        .blocks
        .iter()
        .flat_map(|b| &b.groups)
        .flat_map(|g| g.dependencies.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .enumerate()
        .map(|(i, id)| (id, format!("physical_symbol_{i}")))
        .collect::<std::collections::BTreeMap<_, _>>();
    let mut out = String::from(
        ".intel_syntax noprefix\n.text\n.type physical_probe,@function\nphysical_probe:\n",
    );
    writeln!(out, "jmp .Lphysical_{}", draft.entry.0).unwrap();
    for block in &draft.blocks {
        writeln!(out, ".Lphysical_{}:", block.id.0).unwrap();
        for group in &block.groups {
            for instruction in &group.instructions {
                super::format::format_instruction(
                    &mut out,
                    instruction,
                    |id| dependencies[&id].clone(),
                    |id| format!(".Lphysical_{}", id.0),
                )
                .unwrap();
                out.push('\n');
            }
        }
    }
    crate::test_support::assert_system_assembler_accepts(&out);
}

#[test]
fn byte_carriers_float_predicates_and_indirect_calls_use_declared_recipes() {
    let source = "fn calc(a:u8,b:u8,x:f64,y:f64)->i64{var c:u8=a*b;var f:f64=-x;if(f!=y){return (i64)c;}return (i64)f;} fn invoke(f:fn(u8,u8,f64,f64)->i64)->i64{return f(7u8,8u8,1.5,2.5);} fn main()->i64{return invoke(calc);}";
    let mut indirect = 0;
    let mut zero_extensions = 0;
    complete(source, false, |draft| {
        for block in &draft.blocks {
            for group in &block.groups {
                for instruction in &group.instructions {
                    match instruction {
                        Instruction::Call(CallTarget::Indirect(Register::Gpr(
                            super::super::Gpr::R11,
                        ))) => indirect += 1,
                        Instruction::Convert {
                            op: Convert::ZeroExtend,
                            ..
                        } => zero_extensions += 1,
                        _ => {}
                    }
                }
            }
        }
    });
    assert!(indirect > 0);
    assert!(zero_extensions >= 2);
}

#[test]
fn register_leaf_formatting_covers_preserved_and_extended_low_byte_views() {
    use std::fmt::Write;
    let mut output = String::from(".intel_syntax noprefix\n.text\n");
    for register in super::super::Gpr::ALL {
        for bits in [8, 16, 32, 64] {
            super::format::format_instruction(
                &mut output,
                &Instruction::Move {
                    kind: MoveKind::Integer,
                    bits,
                    source: Operand::Immediate(0),
                    destination: Operand::Register(Register::Gpr(register)),
                },
                |_| unreachable!(),
                |_| unreachable!(),
            )
            .unwrap();
            output.push('\n');
        }
    }
    for index in 0..16 {
        super::format::format_instruction(
            &mut output,
            &Instruction::Move {
                kind: MoveKind::Float,
                bits: 64,
                source: Operand::Register(Register::Xmm(index)),
                destination: Operand::Register(Register::Xmm(index)),
            },
            |_| unreachable!(),
            |_| unreachable!(),
        )
        .unwrap();
        writeln!(output).unwrap();
    }
    crate::test_support::assert_system_assembler_accepts(&output);
}

#[test]
fn primitive_cast_and_unsigned_arithmetic_recipes_assemble_from_checked_inputs() {
    let mut source = String::from("fn main()->i64{return 0;}");
    for input in ["i64", "u64", "u8", "f64", "bool"] {
        for output in ["i64", "u64", "u8", "f64", "bool"] {
            source.push_str(&format!(
                "fn cast_{input}_{output}(x:{input})->{output}{{return ({output})x;}}"
            ));
        }
    }
    source.push_str("fn unsigned(a:u64,b:u64,c:u64)->u64{return (a/b)+(a>>c);}");
    source.push_str("fn byte(a:u8,b:u8,c:u64)->u8{return (a/b)+(a<<c)+(a>>c);}");
    let mut signed_to_float = 0;
    complete(&source, false, |draft| {
        signed_to_float += draft
            .blocks
            .iter()
            .flat_map(|b| &b.groups)
            .flat_map(|g| &g.instructions)
            .filter(|i| {
                matches!(
                    i,
                    Instruction::Convert {
                        op: Convert::SignedToFloat,
                        ..
                    }
                )
            })
            .count();
    });
    assert!(signed_to_float > 0);
}

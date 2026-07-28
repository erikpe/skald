use super::*;
use crate::{
    identity::InitializerId,
    mir::{MirInitialize, MirInitializerDeclaration},
};

#[test]
fn malformed_f64_mir_is_a_structured_backend_error() {
    let mut program = f64_arithmetic_program();
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let MirInstruction::Assign(assignment) = &mut function.body.blocks[0].instructions[0] else {
        panic!("expected f64 constant assignment");
    };
    assignment.rvalue.ty = MirType::I64;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert!(error.message.contains("input MIR failed verification"));
    assert!(error.message.contains("f64 constant is not `f64`"));
}

#[test]
fn malformed_integer_comparisons_are_rejected_at_the_verifier_boundary() {
    let mut program = lower_source_to_mir(
        "fn compare() -> bool { return 1u < 2u; } fn main() -> i64 { return 0; }",
    );
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let operation = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(MirAssignment {
                rvalue:
                    MirRvalue {
                        kind: MirRvalueKind::IntegerComparison { operation, .. },
                        ..
                    },
                ..
            }) => Some(operation),
            _ => None,
        })
        .expect("comparison source must lower to a comparison rvalue");
    operation.operand = crate::mir::MirIntegerType::I64;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("comparison operand is not `i64`"));
}

#[test]
fn malformed_integer_casts_are_rejected_at_the_verifier_boundary() {
    let mut program =
        lower_source_to_mir("fn cast() -> u8 { return (u8) 258u; } fn main() -> i64 { return 0; }");
    let function = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .unwrap();
    let operation = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(MirAssignment {
                rvalue:
                    MirRvalue {
                        kind: MirRvalueKind::IntegerCast { operation, .. },
                        ..
                    },
                ..
            }) => Some(operation),
            _ => None,
        })
        .expect("cast source must lower to an integer-cast rvalue");
    operation.source = crate::mir::MirIntegerType::I64;

    let error = emit_assembly(Target::X86_64SysV, &program).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("integer cast source is not `i64`"));
}

#[test]
fn uses_no_unpreserved_callee_saved_scratch_registers() {
    let output = assembly("fn main() -> i64 { return (2 + 3) * 4; }");

    for register in ["rbx", "r12", "r13", "r14", "r15"] {
        assert!(!output.contains(register));
    }
    assert!(output.contains("push rbp"));
    assert!(output.contains("leave"));
}

#[test]
fn malformed_control_flow_is_a_structured_backend_error() {
    let mut mir = conditional_return_mir(true);
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let Some(MirTerminator::Branch { true_target, .. }) = &mut function.body.blocks[0].terminator
    else {
        panic!("expected branch terminator");
    };
    *true_target = BlockId::new(function.function, 99);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error
        .message()
        .contains("control-flow target f0:b99 is not declared"));
}

#[test]
fn unused_valid_object_metadata_is_accepted() {
    let mut mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: ClassId::new(0),
        module: crate::identity::ModuleId::new(0),
        name: "Empty".to_owned(),
        direct_base: None,
        conformances: vec![],
        fields: vec![],
        initializers: vec![],
        copy_constructor_declaration: None,
        copy_constructor: MirCopyCapability::Unavailable,
        copy_assignment_declaration: None,
        copy_assignment: MirCopyCapability::Unavailable,
        destruction: MirDestructionPlan::new(None, &[]),
        methods: vec![],
        span: mir.span,
    }]);

    assert!(verify_mir(&mir).is_ok());
    assert!(emit_assembly(Target::X86_64SysV, &mir).is_ok());
}

#[test]
fn recursively_non_trivial_source_cleanup_reaches_the_backend() {
    let mir = lower_source_to_mir(concat!(
        "class Resource { init() {} destroy {} }\n",
        "class Owner { resource: Resource; init() { self.resource = Resource(); } }\n",
        "fn main() -> i64 { var owner: Owner = Owner(); return 0; }\n",
    ));

    let output = emit_assembly(Target::X86_64SysV, &mir).unwrap();
    assert!(output.contains("call .Lska_class_0_destroy_0"));
    assert_system_assembler_accepts(&output);
}

#[test]
fn initializer_without_a_definition_is_rejected_at_the_backend_boundary() {
    let (mut mir, ids) = projected_object_program();
    let initializer = InitializerId::new(ids.container, 0);
    mir.classes.entries_mut_for_test()[ids.container.index()]
        .initializers
        .push(MirInitializerDeclaration {
            id: initializer,
            parameters: vec![],
            span: mir.span,
        });
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Initialize(MirInitialize {
            destination: ids.first.into(),
            target: initializer,
            arguments: vec![],
            span: mir.span,
        }));
    function.body.blocks[0]
        .instructions
        .push(MirInstruction::Cleanup(MirCleanup {
            destination: ids.first.into(),
            target: ids.container,
            span: mir.span,
        }));

    let errors = verify_mir(&mir).unwrap_err().to_string();
    assert!(errors.contains("initializer c1:init0 has no member definition"));
    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
}

#[test]
fn undeclared_initializer_target_is_rejected_at_the_backend_boundary() {
    let mut mir = lower_source_to_mir(concat!(
        "class Box { init(value: i64) {} }\n",
        "fn main() -> i64 { var value: Box = Box(1); return 0; }\n",
    ));
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Initialize(initialize) => Some(initialize),
            _ => None,
        })
        .unwrap();
    initialize.target = InitializerId::new(ClassId::new(0), 99);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error
        .message()
        .contains("initializer target c0:init99 is not declared"));
}

#[test]
fn object_bearing_external_signature_is_rejected_before_abi_lowering() {
    let mut mir = counter_member_program();
    let declaration = &mut mir.declarations.entries_mut_for_test()[0];
    declaration.parameters = MirParameter::values([MirType::Class(ClassId::new(0))]);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains(
        "external function cannot declare alias, object value, or shared-owner parameters"
    ));
}

#[test]
fn external_alias_signature_is_a_structured_verification_error() {
    let (mut mir, ids) = alias_counter_program();
    let declaration = &mut mir.declarations.entries_mut_for_test()[ids.add.index()];
    mir.external_links =
        crate::external::ExternalLinkTable::new(vec![crate::external::ExternalLink {
            id: crate::identity::ExternalLinkId::new(0),
            symbol: declaration.name.clone(),
            declarations: vec![declaration.id],
        }]);
    declaration.linkage = MirFunctionLinkage::External {
        link: crate::identity::ExternalLinkId::new(0),
    };
    mir.definitions.remove_for_test(ids.add);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains(
        "external function cannot declare alias, object value, or shared-owner parameters"
    ));
}

#[test]
fn external_interface_signature_is_rejected_before_instruction_selection() {
    let mut mir = lower_text(concat!(
        "interface Runner { fn run() -> i64; }\n",
        "class Worker implements Runner {\n",
        "  init() {}\n",
        "  fn run() -> i64 { return 1; }\n",
        "}\n",
        "fn invoke(ref value: Runner) -> i64 { return value.run(); }\n",
        "fn main() -> i64 { var value: Worker = Worker(); return invoke(value); }\n",
    ));
    let invoke = FunctionId::new(0);
    let declaration = &mut mir.declarations.entries_mut_for_test()[invoke.index()];
    mir.external_links =
        crate::external::ExternalLinkTable::new(vec![crate::external::ExternalLink {
            id: crate::identity::ExternalLinkId::new(0),
            symbol: declaration.name.clone(),
            declarations: vec![declaration.id],
        }]);
    declaration.linkage = MirFunctionLinkage::External {
        link: crate::identity::ExternalLinkId::new(0),
    };
    mir.definitions.remove_for_test(invoke);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains(
        "external function cannot declare alias, object value, or shared-owner parameters"
    ));
}

#[test]
fn external_shared_signatures_are_rejected_at_the_backend_boundary() {
    let mut parameter = counter_member_program();
    let declaration = &mut parameter.declarations.entries_mut_for_test()[0];
    declaration.parameters =
        MirParameter::values([MirType::Shared(MirSharedTarget::Class(ClassId::new(0)))]);

    let error = emit_assembly(Target::X86_64SysV, &parameter).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains(
        "external function cannot declare alias, object value, or shared-owner parameters"
    ));

    let mut result = counter_member_program();
    result.declarations.entries_mut_for_test()[0].return_type =
        MirType::Shared(MirSharedTarget::Class(ClassId::new(0)));

    let error = emit_assembly(Target::X86_64SysV, &result).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error
        .message()
        .contains("external function cannot return an object value or shared owner"));
}

#[test]
fn malformed_shared_lifetime_is_rejected_before_instruction_selection() {
    let mut mir = lower_source_to_mir(concat!(
        "class Resource { init() {} destroy {} }\n",
        "fn main() -> i64 {\n",
        "  var owner: shared Resource = new Resource();\n",
        "  return 0;\n",
        "}\n",
    ));
    let function = mir
        .definitions
        .get_mut_for_test(mir.entry_function)
        .unwrap();
    let initialize = function.body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::SharedInitialize(_)))
        .expect("shared allocation must have an initialization step");
    function.body.blocks[0].instructions.remove(initialize);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error
        .message()
        .contains("shared publication requires completed initialization"));
}

#[test]
fn recursive_inline_layout_is_a_structured_target_error() {
    let mut mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let class = ClassId::new(0);
    let recursive_field = FieldId::new(class, 0);
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        module: crate::identity::ModuleId::new(0),
        name: "Recursive".to_owned(),
        direct_base: None,
        conformances: vec![],
        fields: vec![MirFieldDeclaration {
            id: recursive_field,
            name: "self".to_owned(),
            ty: MirType::Class(class),
            span: mir.span,
        }],
        initializers: vec![],
        copy_constructor_declaration: None,
        copy_constructor: MirCopyCapability::Unavailable,
        copy_assignment_declaration: None,
        copy_assignment: MirCopyCapability::Unavailable,
        destruction: MirDestructionPlan::new(None, &[recursive_field]),
        methods: vec![],
        span: mir.span,
    }]);

    assert!(verify_mir(&mir).is_ok());
    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert!(error
        .message()
        .contains("recursive inline layout involving class c0"));
}

#[test]
fn incomplete_class_metadata_is_rejected_before_layout() {
    let mut mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let class = ClassId::new(0);
    let missing_field = FieldId::new(class, 0);
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        module: crate::identity::ModuleId::new(0),
        name: "Incomplete".to_owned(),
        direct_base: None,
        conformances: vec![],
        fields: vec![MirFieldDeclaration {
            id: missing_field,
            name: "missing".to_owned(),
            ty: MirType::Class(ClassId::new(1)),
            span: mir.span,
        }],
        initializers: vec![],
        copy_constructor_declaration: None,
        copy_constructor: MirCopyCapability::Unavailable,
        copy_assignment_declaration: None,
        copy_assignment: MirCopyCapability::Unavailable,
        destruction: MirDestructionPlan::new(None, &[missing_field]),
        methods: vec![],
        span: mir.span,
    }]);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("undeclared class type c1"));
}

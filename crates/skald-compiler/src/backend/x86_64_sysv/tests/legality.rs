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
fn uses_no_unpreserved_callee_saved_scratch_registers() {
    let output = assembly("fn main() -> i64 { return (2 + 3) * 4; }");

    for register in ["%rbx", "%r12", "%r13", "%r14", "%r15"] {
        assert!(!output.contains(register));
    }
    assert!(output.contains("pushq %rbp"));
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
fn unused_object_metadata_is_accepted_after_obj3() {
    let mut mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: ClassId::new(0),
        name: "Empty".to_owned(),
        fields: vec![],
        initializers: vec![],
        methods: vec![],
        span: mir.span,
    }]);

    assert!(verify_mir(&mir).is_ok());
    assert!(emit_assembly(Target::X86_64SysV, &mir).is_ok());
}

#[test]
fn initialization_remains_a_structured_obj4_capability_error() {
    let (mut mir, ids) = projected_object_program();
    let initializer = InitializerId::new(ids.container, 0);
    mir.classes.entries_mut_for_test()[ids.container.index()]
        .initializers
        .push(MirInitializerDeclaration {
            id: initializer,
            parameter_types: vec![],
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

    assert!(verify_mir(&mir).is_ok());
    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert_eq!(error.target(), Target::X86_64SysV);
    assert!(error
        .message()
        .contains("initialization and receiver calls require OBJ4 lowering"));
}

#[test]
fn recursive_inline_layout_is_a_structured_target_error() {
    let mut mir = lower_source_to_mir("fn main() -> i64 { return 0; }");
    let class = ClassId::new(0);
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        name: "Recursive".to_owned(),
        fields: vec![MirFieldDeclaration {
            id: FieldId::new(class, 0),
            name: "self".to_owned(),
            ty: MirType::Class(class),
            span: mir.span,
        }],
        initializers: vec![],
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
    mir.classes = MirClassDeclarationTable::new(vec![MirClassDeclaration {
        id: class,
        name: "Incomplete".to_owned(),
        fields: vec![MirFieldDeclaration {
            id: FieldId::new(class, 0),
            name: "missing".to_owned(),
            ty: MirType::Class(ClassId::new(1)),
            span: mir.span,
        }],
        initializers: vec![],
        methods: vec![],
        span: mir.span,
    }]);

    let error = emit_assembly(Target::X86_64SysV, &mir).unwrap_err();
    assert!(error.message().contains("input MIR failed verification"));
    assert!(error.message().contains("undeclared class type c1"));
}

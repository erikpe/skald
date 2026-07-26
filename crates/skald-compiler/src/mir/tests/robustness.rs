use std::panic::{self, AssertUnwindSafe};

use crate::{
    backend::{emit_assembly, Target},
    identity::{FieldId, FunctionId},
};

use super::{
    interface_fixtures::{first_interface_call_mut, interface_dispatch_mir},
    type_operation_fixtures::type_operation_mir,
    virtual_fixtures::*,
    *,
};

#[test]
fn structured_mutations_are_rejected_before_backend_lowering() {
    for mutation in mutation_corpus() {
        let errors = verify_mir(&mutation.program)
            .expect_err("structured mutation unexpectedly produced valid MIR");
        assert!(
            errors
                .iter()
                .any(|error| error.message.contains(mutation.expected_message)),
            "{} mutation produced the wrong verification errors:\n{errors}",
            mutation.name,
        );

        let backend = panic::catch_unwind(AssertUnwindSafe(|| {
            emit_assembly(Target::X86_64SysV, &mutation.program)
        }));
        let error = backend
            .unwrap_or_else(|_| panic!("backend panicked for {} mutation", mutation.name))
            .expect_err("invalid MIR must not pass the backend trust boundary");
        assert!(
            error.message().contains("input MIR failed verification"),
            "{} mutation reached backend lowering: {error}",
            mutation.name,
        );
    }
}

struct Mutation {
    name: &'static str,
    expected_message: &'static str,
    program: MirProgram,
}

fn mutation_corpus() -> Vec<Mutation> {
    vec![
        mutate_identity(),
        mutate_type(),
        mutate_ownership(),
        mutate_call(),
        mutate_place(),
        mutate_control_flow(),
        mutate_cleanup_state(),
        mutate_base_projection(),
        mutate_object_view(),
        mutate_virtual_slot(),
        mutate_virtual_receiver_origin(),
        mutate_interface_requirement(),
        mutate_type_operation_target(),
        mutate_cast_failure_edge(),
        mutate_optional_failure_edge(),
    ]
}

fn mutate_optional_failure_edge() -> Mutation {
    let mut program =
        valid_program("fn main() -> i64 { var value: i64? = none; return value!; }\n");
    let definition = entry_definition(&mut program);
    let failure_target = definition
        .body
        .blocks
        .iter()
        .find_map(|block| match block.terminator {
            Some(MirTerminator::OptionalUnwrap { failure_target, .. }) => Some(failure_target),
            _ => None,
        })
        .expect("optional fixture must contain checked unwrap");
    let span = definition.span;
    definition.body.blocks[failure_target.index()].terminator = Some(MirTerminator::Terminate {
        reason: MirTerminationReason::ObjectCastFailure,
        span,
    });
    Mutation {
        name: "optional unwrap failure edge",
        expected_message: "optional unwrap failure edge",
        program,
    }
}

fn mutate_cast_failure_edge() -> Mutation {
    let mut program = type_operation_mir();
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .expect("type-operation fixture must contain inspect");
    let (success_target, failure_target) = definition
        .body
        .blocks
        .iter_mut()
        .find_map(|block| match block.terminator.as_mut() {
            Some(MirTerminator::CheckedCast {
                success_target,
                failure_target,
                ..
            }) => Some((*success_target, failure_target)),
            _ => None,
        })
        .expect("type-operation fixture must contain a checked cast");
    *failure_target = success_target;
    Mutation {
        name: "checked-cast failure edge",
        expected_message: "success and failure edges must differ",
        program,
    }
}

fn mutate_interface_requirement() -> Mutation {
    let (mut program, _) = interface_dispatch_mir();
    let call = first_interface_call_mut(&mut program);
    let MirCallTarget::Interface(target) = &mut call.target else {
        unreachable!("interface fixture must contain an interface call")
    };
    target.requirement =
        crate::identity::InterfaceRequirementId::new(crate::identity::InterfaceId::new(99), 0);
    Mutation {
        name: "interface requirement",
        expected_message: "interface requirement target i99:requirement0 is not declared",
        program,
    }
}

fn mutate_type_operation_target() -> Mutation {
    let mut program = type_operation_mir();
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(1))
        .expect("type-operation fixture must contain inspect");
    let assignment = definition
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Assign(assignment)
                if matches!(assignment.rvalue.kind, MirRvalueKind::TypeTest { .. }) =>
            {
                Some(assignment)
            }
            _ => None,
        })
        .expect("type-operation fixture must contain a runtime test");
    let MirRvalueKind::TypeTest { target, .. } = &mut assignment.rvalue.kind else {
        unreachable!("runtime test selected above")
    };
    *target = MirViewTarget::Class(crate::identity::ClassId::new(99));
    Mutation {
        name: "runtime type-test target",
        expected_message: "type-test target is not declared",
        program,
    }
}

fn mutate_virtual_slot() -> Mutation {
    let (mut program, _) = virtual_dispatch_mir();
    program.virtual_families.entries_mut_for_test()[0].slot =
        crate::identity::VirtualSlotId::new(1);
    Mutation {
        name: "virtual slot",
        expected_message: "non-canonical slot",
        program,
    }
}

fn mutate_virtual_receiver_origin() -> Mutation {
    let (mut program, ids) = virtual_dispatch_mir();
    let call = first_virtual_call_mut(&mut program);
    let MirObjectOrigin::Forwarded { carrier, .. } = call
        .receiver
        .as_mut()
        .and_then(MirCallReceiver::as_method_mut)
        .unwrap()
        .origin
        .as_mut()
    else {
        unreachable!("virtual fixture receiver must be forwarded")
    };
    *carrier = StorageId::new(crate::identity::CallableId::Method(ids.relay), 2);
    Mutation {
        name: "virtual receiver origin",
        expected_message: "static place does not come from its forwarded carrier",
        program,
    }
}

fn mutate_base_projection() -> Mutation {
    let mut program = static_inheritance_mir();
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .expect("relay definition must exist");
    let view = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &mut call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .expect("relay must contain an ancestor view");
    view.source.projections[0] = MirPlaceProjection::Base(crate::identity::ClassId::new(0));
    Mutation {
        name: "base projection",
        expected_message: "is not the declared direct base",
        program,
    }
}

fn mutate_object_view() -> Mutation {
    let mut program = static_inheritance_mir();
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(2))
        .expect("relay definition must exist");
    let view = definition.body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => match &mut call.arguments[0] {
                MirArgument::View(view) => Some(view),
                _ => None,
            },
            _ => None,
        })
        .expect("relay must contain an ancestor view");
    view.access = MirAliasAccess::Mutable;
    Mutation {
        name: "object view",
        expected_message: "view grants mutable access",
        program,
    }
}

fn mutate_identity() -> Mutation {
    let mut program = valid_program("fn main() -> i64 { return 0; }");
    let definition = entry_definition(&mut program);
    definition.values[0].id = ValueId::new(FunctionId::new(99), 0);
    Mutation {
        name: "identity",
        expected_message: "owned by another callable body",
        program,
    }
}

fn mutate_type() -> Mutation {
    let mut program = valid_program("fn main() -> i64 { return 0; }");
    entry_definition(&mut program).values[0].ty = MirType::Bool;
    Mutation {
        name: "type",
        expected_message: "assignment type does not match",
        program,
    }
}

fn mutate_ownership() -> Mutation {
    let mut program = valid_program(concat!(
        "fn inspect(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return inspect(1); }\n",
    ));
    let definition = program
        .definitions
        .get_mut_for_test(FunctionId::new(0))
        .expect("inspect definition must exist");
    definition.storage[0].kind = MirStorageKind::Local;
    Mutation {
        name: "ownership",
        expected_message: "kind does not match its source binding",
        program,
    }
}

fn mutate_call() -> Mutation {
    let mut program = valid_program(concat!(
        "fn identity(value: i64) -> i64 { return value; }\n",
        "fn main() -> i64 { return identity(1); }\n",
    ));
    let call = entry_definition(&mut program).body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call) => Some(call),
            _ => None,
        })
        .expect("main must contain a call");
    call.arguments.clear();
    Mutation {
        name: "call",
        expected_message: "call has 0 arguments but requires 1",
        program,
    }
}

fn mutate_place() -> Mutation {
    let mut program = valid_program("fn main() -> i64 { var value: i64 = 0; return value; }");
    let store = entry_definition(&mut program).body.blocks[0]
        .instructions
        .iter_mut()
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) => Some(store),
            _ => None,
        })
        .expect("main must contain a store");
    store.destination = store
        .destination
        .clone()
        .project_field(FieldId::new(crate::identity::ClassId::new(0), 0));
    Mutation {
        name: "place",
        expected_message: "has a non-class base",
        program,
    }
}

fn mutate_control_flow() -> Mutation {
    let mut program = goto_join_mir();
    assert!(
        verify_mir(&program).is_ok(),
        "control-flow seed must be valid"
    );
    let definition = entry_definition(&mut program);
    definition.body.blocks[0].terminator = Some(MirTerminator::Goto {
        target: BlockId::new(definition.function, 99),
        span: definition.span,
    });
    Mutation {
        name: "control-flow edge",
        expected_message: "target f0:b99 is not declared",
        program,
    }
}

fn mutate_cleanup_state() -> Mutation {
    let mut program = valid_program(concat!(
        "class Resource { init() {} }\n",
        "fn main() -> i64 { var resource: Resource = Resource(); return 0; }\n",
    ));
    let definition = entry_definition(&mut program);
    let cleanup = definition.body.blocks[0]
        .instructions
        .iter()
        .find(|instruction| matches!(instruction, MirInstruction::Cleanup(_)))
        .cloned()
        .expect("resource lifetime must contain cleanup");
    definition.body.blocks[0].instructions.push(cleanup);
    Mutation {
        name: "cleanup state",
        expected_message: "cleanup destination is destroyed more than once",
        program,
    }
}

fn valid_program(source: &str) -> MirProgram {
    let program = lower_text(source);
    assert!(verify_mir(&program).is_ok(), "mutation seed must be valid");
    program
}

fn entry_definition(program: &mut MirProgram) -> &mut MirFunctionDefinition {
    program
        .definitions
        .get_mut_for_test(program.entry_function)
        .expect("entry definition must exist")
}

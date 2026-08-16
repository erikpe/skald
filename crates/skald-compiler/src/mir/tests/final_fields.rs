use super::*;
use crate::{
    identity::FunctionId,
    source::{Span, TextRange},
    test_support::lower_source_to_final_mir,
};

fn forge_first_static_as_final(program: &mut MirProgram) {
    let declaration = &mut program.classes.entries_mut_for_test()[0].static_fields[0];
    let start = declaration.span.range().start();
    let final_span = Span::new(
        declaration.span.source_id(),
        TextRange::new(start, start + 1).unwrap(),
    );
    declaration.final_span = Some(final_span);
    program
        .static_lifecycle
        .as_mut()
        .expect("explicit static initializer must have a lifecycle coordinator")
        .lifecycle_mut_for_test()
        .definitions_mut_for_test()[0]
        .final_span = Some(final_span);
}

const SOURCE: &str = concat!(
    "class Values {\n",
    "  final value: i64;\n",
    "  final static version: u64 = 1u;\n",
    "  init(value: i64) { self.value = value; }\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

const ASSIGNMENT_FAMILIES: &str = concat!(
    "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
    "fn identity(value: i64) -> i64 { return value; }\n",
    "class Values {\n",
    "  final scalar: i64; final object: Item; final maybe: i64?;\n",
    "  final optional_object: Item?; final owner: shared Item;\n",
    "  final values: i64[]; final callback: fn(i64) -> i64;\n",
    "  init(value: i64) { self.scalar = value; self.object = Item(value);\n",
    "    self.maybe = value; self.optional_object = Item(value);\n",
    "    self.owner = new Item(value); self.values = i64[]{value}; self.callback = identity; }\n",
    "  assign(ref other: Values) { self.scalar = other.scalar; self.object = other.object;\n",
    "    self.maybe = other.maybe; self.optional_object = other.optional_object;\n",
    "    self.owner = other.owner; self.values = other.values; self.callback = other.callback; }\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

fn copy_assignment(program: &MirProgram) -> &MirMemberDefinition {
    program
        .member_definition(crate::identity::CopyAssignmentId::new(ClassId::new(1), 0).into())
        .expect("user copy assignment")
}

fn copy_assignment_mut(program: &mut MirProgram) -> &mut MirMemberDefinition {
    program
        .member_definitions
        .get_mut_for_test(crate::identity::CopyAssignmentId::new(ClassId::new(1), 0).into())
        .expect("user copy assignment")
}

fn final_authorizations(function: &MirMemberDefinition) -> Vec<MirFinalWriteAuthorization> {
    function
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Store(operation) => operation.final_authorization,
            MirInstruction::CopyAssign(operation) => operation.final_authorization,
            MirInstruction::OptionalAssign(operation) => operation.final_authorization,
            MirInstruction::AggregateOptionalAssign(operation) => operation.final_authorization,
            MirInstruction::ClassOptionalAssign(operation) => operation.final_authorization,
            MirInstruction::OptionalSharedAssign(operation) => operation.final_authorization,
            MirInstruction::SharedFieldReplace(operation) => operation.final_authorization,
            MirInstruction::Array(MirArrayInstruction::Replace {
                final_authorization,
                ..
            }) => *final_authorization,
            _ => None,
        })
        .collect()
}

#[test]
fn lowers_exact_final_modifier_evidence_and_dumps_it() {
    let program = lower_source_to_final_mir(SOURCE);
    verify_mir(&program).unwrap();
    let values = program.class(ClassId::new(0)).unwrap();
    assert!(values.fields[0].final_span.is_some());
    assert!(values.static_fields[0].final_span.is_some());

    let dump = dump_mir(&program);
    assert!(
        dump.contains("Field c0:field0 final \"value\" : i64"),
        "{dump}"
    );
    assert!(
        dump.contains("StaticField c0:static0 final \"version\""),
        "{dump}"
    );
    assert_eq!(dump.matches("Final @").count(), 2, "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_malformed_final_declaration_metadata() {
    let program = lower_source_to_final_mir(SOURCE);

    let mut empty = program.clone();
    let field = &mut empty.classes.entries_mut_for_test()[0].fields[0];
    field.final_span = Some(Span::empty(
        field.span.source_id(),
        field.span.range().end(),
    ));
    let errors = verify_mir(&empty).unwrap_err().to_string();
    assert!(
        errors.contains("final modifier span must be nonempty"),
        "{errors}"
    );

    let mut outside = program.clone();
    let field = &mut outside.classes.entries_mut_for_test()[0].fields[0];
    field.final_span = Some(Span::new(
        field.span.source_id(),
        TextRange::new(field.span.range().end(), field.span.range().end() + 1).unwrap(),
    ));
    let errors = verify_mir(&outside).unwrap_err().to_string();
    assert!(
        errors.contains("contained by its declaration span"),
        "{errors}"
    );

    let mut incompatible = program.clone();
    let field = &mut incompatible.classes.entries_mut_for_test()[0].fields[0];
    field.cell_span = field.final_span;
    let errors = verify_mir(&incompatible).unwrap_err().to_string();
    assert!(
        errors.contains("cannot carry both cell and final metadata"),
        "{errors}"
    );

    let mut missing_initializer = program;
    let field = &mut missing_initializer.classes.entries_mut_for_test()[0].static_fields[0];
    field.initialization = MirStaticFieldInitialization::ZeroDefault;
    let errors = verify_mir(&missing_initializer).unwrap_err().to_string();
    assert!(
        errors.contains("must have explicit initialization"),
        "{errors}"
    );
}

#[test]
fn verifier_rejects_forged_final_static_root_writes_and_mutable_aliases() {
    let mut direct = lower_source_to_final_mir(concat!(
        "class State { static value: i64 = 1; init() {} }\n",
        "fn main() -> i64 { State.value = 2; return State.value; }\n",
    ));
    forge_first_static_as_final(&mut direct);
    assert!(verify_mir(&direct)
        .unwrap_err()
        .to_string()
        .contains("final static root cannot be replaced"));

    let mut alias = lower_source_to_final_mir(concat!(
        "fn replace(mut ref value: i64) -> unit { value = 2; }\n",
        "class State { static value: i64 = 1; init() {} }\n",
        "fn main() -> i64 { replace(State.value); return State.value; }\n",
    ));
    forge_first_static_as_final(&mut alias);
    let main = alias
        .definitions
        .get(FunctionId::new(1))
        .expect("main definition");
    assert!(main.body.blocks.iter().any(|block| block
        .instructions
        .iter()
        .any(|instruction| matches!(instruction, MirInstruction::Call(_)))));
    assert!(verify_mir(&alias)
        .unwrap_err()
        .to_string()
        .contains("cannot mutably alias a final static root"));
}

#[test]
fn lowers_and_verifies_exact_user_assignment_evidence_for_every_storage_family() {
    let checked = type_check_source(ASSIGNMENT_FAMILIES);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    verify_preliminary_mir(&preliminary).unwrap();

    let program = lower_hir(&hir);
    verify_mir(&program).unwrap();
    let authorizations = final_authorizations(copy_assignment(&program));
    assert_eq!(
        authorizations
            .iter()
            .map(|authorization| authorization.field.index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6]
    );
    assert!(authorizations.iter().all(|authorization| {
        authorization.operation == crate::identity::CopyAssignmentId::new(ClassId::new(1), 0)
    }));

    let dump = dump_mir(&program);
    assert_eq!(dump.matches("final-write c1:field").count(), 7, "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_missing_forged_nested_or_wrong_operation_final_evidence() {
    let program = lower_source_to_final_mir(ASSIGNMENT_FAMILIES);

    let mutate_scalar = |program: &mut MirProgram, mutation: &mut dyn FnMut(&mut MirStore)| {
        let store = copy_assignment_mut(program)
            .body
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
            .find_map(|instruction| match instruction {
                MirInstruction::Store(store) if store.final_authorization.is_some() => Some(store),
                _ => None,
            })
            .expect("authorized scalar store");
        mutation(store);
    };

    let mut missing = program.clone();
    mutate_scalar(&mut missing, &mut |store| store.final_authorization = None);
    assert!(verify_mir(&missing)
        .unwrap_err()
        .to_string()
        .contains("final field replacement lacks exact copy-assignment authorization"));

    let mut wrong_field = program.clone();
    mutate_scalar(&mut wrong_field, &mut |store| {
        store.final_authorization.as_mut().unwrap().field = FieldId::new(ClassId::new(1), 1);
    });
    assert!(verify_mir(&wrong_field)
        .unwrap_err()
        .to_string()
        .contains("final-update authorization does not match"));

    let mut wrong_operation = program.clone();
    mutate_scalar(&mut wrong_operation, &mut |store| {
        store.final_authorization.as_mut().unwrap().operation =
            crate::identity::CopyAssignmentId::new(ClassId::new(0), 0);
    });
    assert!(verify_mir(&wrong_operation)
        .unwrap_err()
        .to_string()
        .contains("final-update authorization does not match"));

    let mut nested = program;
    mutate_scalar(&mut nested, &mut |store| {
        store
            .destination
            .projections
            .push(MirPlaceProjection::Field(FieldId::new(ClassId::new(0), 0)));
    });
    assert!(verify_mir(&nested)
        .unwrap_err()
        .to_string()
        .contains("final-update authorization does not match"));
}

#[test]
fn synthesized_assignment_plan_carries_an_exact_ordered_final_field_set() {
    let source = ASSIGNMENT_FAMILIES.replace(
        "  assign(ref other: Values) { self.scalar = other.scalar; self.object = other.object;\n    self.maybe = other.maybe; self.optional_object = other.optional_object;\n    self.owner = other.owner; self.values = other.values; self.callback = other.callback; }\n",
        "",
    );
    let program = lower_source_to_final_mir(&source);
    verify_mir(&program).unwrap();
    let values = program.class(ClassId::new(1)).unwrap();
    let MirCopyCapability::Synthesized(assignment) = &values.copy_assignment else {
        panic!("expected synthesized copy assignment");
    };
    assert_eq!(
        assignment
            .final_fields
            .iter()
            .map(|field| field.index())
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3, 4, 5, 6]
    );
    let MirCopyCapability::Synthesized(constructor) = &values.copy_constructor else {
        panic!("expected synthesized copy constructor");
    };
    assert!(constructor.final_fields.is_empty());

    let mut missing = program.clone();
    let MirCopyCapability::Synthesized(plan) =
        &mut missing.classes.entries_mut_for_test()[1].copy_assignment
    else {
        unreachable!()
    };
    plan.final_fields.pop();
    assert!(verify_mir(&missing)
        .unwrap_err()
        .to_string()
        .contains("synthesized copy-assignment plan has invalid final-update evidence"));

    let mut reordered = program.clone();
    let MirCopyCapability::Synthesized(plan) =
        &mut reordered.classes.entries_mut_for_test()[1].copy_assignment
    else {
        unreachable!()
    };
    plan.final_fields.swap(0, 1);
    assert!(verify_mir(&reordered)
        .unwrap_err()
        .to_string()
        .contains("synthesized copy-assignment plan has invalid final-update evidence"));

    let mut forged_constructor = program;
    let MirCopyCapability::Synthesized(plan) =
        &mut forged_constructor.classes.entries_mut_for_test()[1].copy_constructor
    else {
        unreachable!()
    };
    plan.final_fields.push(FieldId::new(ClassId::new(1), 0));
    assert!(verify_mir(&forged_constructor)
        .unwrap_err()
        .to_string()
        .contains("invalid owner, field count, or final-update evidence"));
}

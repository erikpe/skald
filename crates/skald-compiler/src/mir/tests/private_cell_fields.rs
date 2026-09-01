use super::*;
use crate::source::Span;

const SOURCE: &str = concat!(
    "class Cache {\n",
    "  private cell value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  fn read() -> i64 { return self.value; }\n",
    "}\n",
    "fn main() -> i64 { var cache: Cache = Cache(7); return cache.read(); }\n",
);

const ASSIGNMENT_FAMILIES: &str = concat!(
    "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
    "class Cache {\n",
    "  ordinary: i64;\n",
    "  private cell scalar: i64;\n",
    "  private cell object: Item;\n",
    "  private cell maybe: i64?;\n",
    "  private cell owner: shared Item;\n",
    "  private cell values: i64[];\n",
    "  init() {\n",
    "    self.ordinary = 0; self.scalar = 0; self.object = Item(0);\n",
    "    self.maybe = none; self.owner = new Item(0); self.values = i64[]{};\n",
    "  }\n",
    "  fn replace(ref object: Item, owner: shared Item) -> unit {\n",
    "    self.scalar = 1; self.object = object; self.maybe = 2;\n",
    "    self.owner = owner; self.values = i64[]{3};\n",
    "  }\n",
    "  mut fn mutable_replace() -> unit { self.scalar = 4; }\n",
    "}\n",
    "fn main() -> i64 { return 0; }\n",
);

fn assignment_method(program: &MirProgram) -> &MirMemberDefinition {
    program
        .member_definition(MethodId::new(ClassId::new(1), 0).into())
        .expect("cell replacement method")
}

fn assignment_method_mut(program: &mut MirProgram) -> &mut MirMemberDefinition {
    program
        .member_definitions
        .get_mut_for_test(MethodId::new(ClassId::new(1), 0).into())
        .expect("cell replacement method")
}

fn scalar_store_mut(program: &mut MirProgram) -> &mut MirStore {
    assignment_method_mut(program)
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) if store.authorization.is_some() => Some(store),
            _ => None,
        })
        .expect("authorized scalar store")
}

fn verification_errors(program: &MirProgram) -> String {
    verify_mir(program).unwrap_err().to_string()
}

#[test]
fn lowers_exact_cell_modifier_evidence_and_dumps_it() {
    let program = lower_text(SOURCE);
    verify_mir(&program).unwrap();
    let field = &program.class(ClassId::new(0)).unwrap().fields[0];
    let cell_span = field.cell_span.expect("cell field must retain its span");
    assert!(!cell_span.range().is_empty());

    let dump = dump_mir(&program);
    assert!(
        dump.contains("Field c0:field0 cell \"value\" : i64"),
        "{dump}"
    );
    assert!(dump.contains("Cell @"), "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_malformed_cell_modifier_evidence() {
    let mut program = lower_text(SOURCE);
    let field = &mut program.classes.entries_mut_for_test()[0].fields[0];
    field.cell_span = Some(Span::empty(
        field.span.source_id(),
        field.span.range().end(),
    ));

    let errors = verify_mir(&program).unwrap_err().to_string();
    assert!(
        errors.contains("cell modifier span must be nonempty"),
        "{errors}"
    );
}

#[test]
fn lowers_and_verifies_every_cell_assignment_carrier_in_preliminary_and_final_mir() {
    let checked = type_check_source(ASSIGNMENT_FAMILIES);
    assert!(checked.diagnostics.is_empty(), "{:?}", checked.diagnostics);
    let hir = checked.hir.unwrap();
    let preliminary = lower_preliminary_hir(&hir);
    check_preliminary_mir(&preliminary).unwrap();

    let program = lower_hir(&hir);
    verify_mir(&program).unwrap();
    let method = assignment_method(&program);
    let authorized_fields = method
        .body
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction {
            MirInstruction::Store(operation) => operation.authorization,
            MirInstruction::CopyAssign(operation) => operation.authorization,
            MirInstruction::OptionalAssign(operation) => operation.authorization,
            MirInstruction::AggregateOptionalAssign(operation) => operation.authorization,
            MirInstruction::ClassOptionalAssign(operation) => operation.authorization,
            MirInstruction::OptionalSharedAssign(operation) => operation.authorization,
            MirInstruction::SharedFieldReplace(operation) => operation.authorization,
            MirInstruction::Array(MirArrayInstruction::Replace { authorization, .. }) => {
                *authorization
            }
            _ => None,
        })
        .map(|authorization| authorization.field.index())
        .collect::<Vec<_>>();
    assert_eq!(authorized_fields, vec![1, 2, 3, 4, 5]);

    let dump = dump_mir(&program);
    assert_eq!(dump.matches("cell-write c1:field").count(), 5, "{dump}");
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn verifier_rejects_missing_forged_or_overbroad_cell_authorization() {
    let program = lower_text(ASSIGNMENT_FAMILIES);

    let mut missing = program.clone();
    scalar_store_mut(&mut missing).authorization = None;
    assert!(verification_errors(&missing).contains("store destination requires mutable access"));

    let mut ordinary = program.clone();
    let store = scalar_store_mut(&mut ordinary);
    let ordinary_field = FieldId::new(ClassId::new(1), 0);
    *store.destination.projections.last_mut().unwrap() = MirPlaceProjection::Field(ordinary_field);
    store.authorization = Some(MirCellWriteAuthorization {
        field: ordinary_field,
    });
    assert!(verification_errors(&ordinary).contains("cell write authorization does not match"));

    let mut wrong_owner = program.clone();
    assignment_method_mut(&mut wrong_owner).class_owner = ClassId::new(0);
    assert!(verification_errors(&wrong_owner).contains("cell write authorization does not match"));

    let mut wrong_family = program.clone();
    let store = scalar_store_mut(&mut wrong_family);
    let object_field = FieldId::new(ClassId::new(1), 2);
    *store.destination.projections.last_mut().unwrap() = MirPlaceProjection::Field(object_field);
    store.authorization = Some(MirCellWriteAuthorization {
        field: object_field,
    });
    assert!(verification_errors(&wrong_family).contains("cell write authorization does not match"));

    let mut malformed_origin = program.clone();
    let receiver = assignment_method(&malformed_origin).receiver.unwrap();
    scalar_store_mut(&mut malformed_origin).destination.base =
        MirPlaceBase::AliasParameter(receiver);
    let errors = verification_errors(&malformed_origin);
    assert!(
        errors.contains("cell write authorization does not match"),
        "{errors}"
    );
    assert!(
        errors.contains("is not alias parameter storage"),
        "{errors}"
    );
}

#[test]
fn verifier_rejects_nested_or_mutable_destinations_and_retains_lifetime_checks() {
    let program = lower_text(ASSIGNMENT_FAMILIES);

    let mut nested = program.clone();
    let copy = assignment_method_mut(&mut nested)
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::CopyAssign(copy) if copy.authorization.is_some() => Some(copy),
            _ => None,
        })
        .unwrap();
    copy.destination
        .projections
        .push(MirPlaceProjection::Field(FieldId::new(ClassId::new(0), 0)));
    assert!(verification_errors(&nested).contains("cell write authorization does not match"));

    let mut mutable = program.clone();
    let method = mutable
        .member_definitions
        .get_mut_for_test(MethodId::new(ClassId::new(1), 1).into())
        .unwrap();
    let store = method
        .body
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.instructions)
        .find_map(|instruction| match instruction {
            MirInstruction::Store(store) => Some(store),
            _ => None,
        })
        .unwrap();
    store.authorization = Some(MirCellWriteAuthorization {
        field: FieldId::new(ClassId::new(1), 1),
    });
    assert!(verification_errors(&mutable).contains("cell write authorization does not match"));

    let mut dead = program;
    let method = assignment_method_mut(&mut dead);
    let receiver = method.receiver.unwrap();
    let block = &mut method.body.blocks[0];
    let store_index = block
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::Store(store) if store.authorization.is_some()))
        .unwrap();
    block.instructions.insert(
        store_index,
        MirInstruction::StorageDead(MirStorageDead {
            storage: receiver,
            span: block.span,
        }),
    );
    assert!(verification_errors(&dead).contains("outside a live lifetime epoch"));
}

#[test]
fn initialized_cell_optional_fields_support_checked_views_in_read_only_methods() {
    let program = lower_text(concat!(
        "class Item { value: i64; init(value: i64) { self.value = value; } }\n",
        "class Holder {\n",
        "  private cell item: Item?;\n",
        "  init() { self.item = Item(42); }\n",
        "  fn replace() -> i64 { self.item = Item(0); return 0; }\n",
        "  fn read() -> i64 { return self.item!.value; }\n",
        "}\n",
        "fn main() -> i64 { var holder: Holder = Holder(); return holder.read(); }\n",
    ));

    verify_mir(&program).expect("initialized receiver fields must seed checked optional views");
    let dump = dump_mir(&program);
    assert!(dump.contains("begin-optional-view"), "{dump}");
    assert!(dump.contains("cell-write c1:field0"), "{dump}");
}

use super::*;

#[test]
fn lowers_shared_fields_as_owner_edges_in_lifecycle_order() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class Inline {\n",
        "  edge: shared Item;\n",
        "  init(edge: shared Item) { self.edge = edge; }\n",
        "}\n",
        "class Holder {\n",
        "  left: shared Item;\n",
        "  middle: Inline;\n",
        "  right: shared Item;\n",
        "  init(value: shared Item) {\n",
        "    self.left = value;\n",
        "    self.middle = Inline(new Item());\n",
        "    self.right = new Item();\n",
        "  }\n",
        "  mut fn replace() -> unit {\n",
        "    self.middle.edge = self.right;\n",
        "    self.left = self.right;\n",
        "  }\n",
        "  fn snapshot() -> shared Item { return self.left; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("shared field lifecycle MIR must verify");

    let holder = program.class(ClassId::new(2)).unwrap();
    let MirCopyCapability::Synthesized(construction) = &holder.copy_constructor else {
        panic!("shared fields must retain synthesized copy construction");
    };
    assert!(matches!(
        construction.fields.as_slice(),
        [
            MirSynthesizedFieldCopy::Shared { .. },
            MirSynthesizedFieldCopy::Class { .. },
            MirSynthesizedFieldCopy::Shared { .. },
        ]
    ));
    assert!(matches!(
        holder.destruction.steps.as_slice(),
        [
            MirDestructionStep::SharedField(_),
            MirDestructionStep::Field(_),
            MirDestructionStep::SharedField(_),
        ]
    ));

    let dump = dump_mir(&program);
    assert!(dump.contains("shared-field-initialize"));
    assert!(dump.contains("shared-field-replace"));
    assert!(dump.contains("shared-field-copy"));
    assert!(dump.contains("Shared c2:field0"));
    assert!(dump.contains("SharedField c2:field2"));

    let assembly =
        emit_assembly(Target::X86_64SysV, &program).expect("verified shared fields must execute");
    assert!(assembly.contains("ownership_field_replace"));
    assert!(assembly.contains("field_2_2_release"));
}

#[test]
fn rejects_corrupt_shared_field_initialization_and_lifecycle_metadata() {
    let source = concat!(
        "class Item { init() {} }\n",
        "class Holder {\n",
        "  first: shared Item;\n",
        "  second: shared Item;\n",
        "  init() { self.first = new Item(); self.second = new Item(); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    );
    let program = lower_text(source);
    let initializer = InitializerId::new(ClassId::new(1), 0);

    let mut missing = program.clone();
    let body = &mut missing
        .member_definitions
        .get_mut_for_test(initializer.into())
        .unwrap()
        .body;
    let remove = body.blocks[0]
        .instructions
        .iter()
        .rposition(|instruction| matches!(instruction, MirInstruction::SharedFieldInitialize(_)))
        .unwrap();
    body.blocks[0].instructions.remove(remove);
    assert!(has_error(
        &missing,
        "shared receiver fields are not initialized exactly once"
    ));

    let mut duplicate = program.clone();
    let body = &mut duplicate
        .member_definitions
        .get_mut_for_test(initializer.into())
        .unwrap()
        .body;
    let initialize = body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::SharedFieldInitialize(initialize) => Some(initialize.clone()),
            _ => None,
        })
        .unwrap();
    let boundary = body.blocks[0]
        .instructions
        .iter()
        .position(|instruction| matches!(instruction, MirInstruction::EndFullExpression(_)))
        .unwrap();
    body.blocks[0]
        .instructions
        .insert(boundary, MirInstruction::SharedFieldInitialize(initialize));
    assert!(has_error(
        &duplicate,
        "shared field transfer source is not a live owner"
    ));
    assert!(has_error(
        &duplicate,
        "shared field is initialized more than once"
    ));

    let mut wrong_plan = program;
    let holder = &mut wrong_plan.classes.entries_mut_for_test()[1];
    holder.destruction.steps[0] = MirDestructionStep::Field(FieldId::new(holder.id, 1));
    assert!(verify_mir(&wrong_plan)
        .unwrap_err()
        .iter()
        .any(|error| error
            .message
            .contains("owning fields in reverse declaration order")));
}

#[test]
fn preserves_user_shared_field_lifecycle_across_inheritance() {
    let program = lower_text(concat!(
        "class Item { init() {} }\n",
        "class Base {\n",
        "  edge: shared Item;\n",
        "  init(edge: shared Item) { self.edge = edge; }\n",
        "  copy(ref source: Base) { self.edge = source.edge; }\n",
        "  assign(ref source: Base) { self.edge = source.edge; }\n",
        "}\n",
        "class Derived extends Base {\n",
        "  extra: shared Item;\n",
        "  init(edge: shared Item, extra: shared Item) {\n",
        "    super(edge); self.extra = extra;\n",
        "  }\n",
        "  copy(ref source: Derived) { self.extra = source.extra; }\n",
        "  assign(ref source: Derived) { self.extra = source.extra; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    verify_mir(&program).expect("inherited shared lifecycle must verify");

    let derived = program.class(ClassId::new(2)).unwrap();
    assert!(matches!(
        derived.copy_constructor,
        MirCopyCapability::User(_)
    ));
    assert!(matches!(
        derived.copy_assignment,
        MirCopyCapability::User(_)
    ));
    assert!(matches!(
        derived.destruction.steps.as_slice(),
        [
            MirDestructionStep::SharedField(_),
            MirDestructionStep::Base(_)
        ]
    ));
    let dump = dump_mir(&program);
    assert!(dump.contains("shared-field-initialize"));
    assert!(dump.contains("shared-field-replace"));
}

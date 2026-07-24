use super::*;
use crate::{
    hir::{HirBaseCopy, HirDestructionStep, HirObjectSource, HirStatement},
    mir::MirCopyCapability,
    resolve::ResolvedCopyOperation,
    typeck::{
        capabilities::CopyPathElement, AMBIGUOUS_INITIALIZER, FIELD_INITIALIZATION,
        NO_MATCHING_INITIALIZER,
    },
};

const BASE_AND_DERIVED: &str = concat!(
    "class Base {\n",
    "  value: i64;\n",
    "  init(value: i64) { self.value = value; }\n",
    "  destroy {}\n",
    "}\n",
    "class Derived extends Base {\n",
    "  child: Leaf;\n",
    "  init(value: i64) { super(value); self.child = Leaf(); }\n",
    "  destroy {}\n",
    "}\n",
    "class Leaf { init() {} destroy {} }\n",
    "fn main() -> i64 { return 0; }\n",
);

#[test]
fn hir_retains_ordered_complete_object_lifecycle_operations() {
    let output = check_text(BASE_AND_DERIVED);
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let base = hir.class(ClassId::new(0)).unwrap();
    let derived = hir.class(ClassId::new(1)).unwrap();

    let direct_base = derived.direct_base.as_ref().unwrap();
    assert_eq!(direct_base.class, base.id);

    let HirCopyCapability::Synthesized(copy) = &derived.copy_constructor else {
        panic!("derived copy construction should be synthesized");
    };
    assert_eq!(
        copy.base,
        Some(HirBaseCopy {
            base: base.id,
            operation: base.copy_constructor.selected().unwrap(),
        })
    );
    let HirCopyCapability::Synthesized(assignment) = &derived.copy_assignment else {
        panic!("derived copy assignment should be synthesized");
    };
    assert_eq!(
        assignment.base,
        Some(HirBaseCopy {
            base: base.id,
            operation: base.copy_assignment.selected().unwrap(),
        })
    );
    assert_eq!(
        derived.destruction.steps,
        [
            HirDestructionStep::UserBody(crate::identity::DestructorId::new(derived.id, 0,)),
            HirDestructionStep::Field(FieldId::new(derived.id, 0)),
            HirDestructionStep::Base(base.id),
        ]
    );

    let definition = hir.class_definitions.get(derived.id).unwrap();
    let HirStatement::BaseInitialization(initialization) =
        &definition.initializers[0].body.statements[0]
    else {
        panic!("derived initializer should begin with explicit base initialization");
    };
    assert_eq!(initialization.base, base.id);
    assert_eq!(initialization.initializer, InitializerId::new(base.id, 0));
    assert_eq!(initialization.arguments.len(), 1);
    let dump = crate::hir::dump_hir(&hir);
    assert!(dump.contains("BaseInitialization c0 via c0:init0"));
    assert!(dump.contains(
        "DestructionPlan\n        UserBody c1:destroy0\n        Field c1:field0\n        Base c0"
    ));
    assert_eq!(dump, crate::hir::dump_hir(&hir));
}

#[test]
fn deep_empty_base_chains_keep_one_direct_base_step_per_class() {
    let output = check_text(concat!(
        "class Root { init() {} }\n",
        "class Middle extends Root { init() { super(); } }\n",
        "class Leaf extends Middle { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();

    assert!(hir
        .class(ClassId::new(0))
        .unwrap()
        .destruction
        .steps
        .is_empty());
    assert_eq!(
        hir.class(ClassId::new(1)).unwrap().destruction.steps,
        [HirDestructionStep::Base(ClassId::new(0))]
    );
    assert_eq!(
        hir.class(ClassId::new(2)).unwrap().destruction.steps,
        [HirDestructionStep::Base(ClassId::new(1))]
    );
}

#[test]
fn user_and_synthesized_copy_operations_compose_in_both_directions() {
    let output = check_text(concat!(
        "class UserBase {\n",
        "  init() {}\n",
        "  copy(ref source: UserBase) {}\n",
        "  assign(ref source: UserBase) {}\n",
        "}\n",
        "class SynthDerived extends UserBase { init() { super(); } }\n",
        "class SynthBase { init() {} }\n",
        "class UserDerived extends SynthBase {\n",
        "  init() { super(); }\n",
        "  copy(ref source: UserDerived) {}\n",
        "  assign(ref source: UserDerived) {}\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();

    let synth_derived = hir.class(ClassId::new(1)).unwrap();
    let HirCopyCapability::Synthesized(construction) = &synth_derived.copy_constructor else {
        panic!("derived operation should be synthesized");
    };
    assert!(matches!(
        construction.base,
        Some(HirBaseCopy {
            operation: HirSelectedCopyOperation::User(_),
            ..
        })
    ));
    let HirCopyCapability::Synthesized(assignment) = &synth_derived.copy_assignment else {
        panic!("derived operation should be synthesized");
    };
    assert!(matches!(
        assignment.base,
        Some(HirBaseCopy {
            operation: HirSelectedCopyOperation::User(_),
            ..
        })
    ));

    let synth_base = hir.class(ClassId::new(2)).unwrap();
    let user_derived = hir.class(ClassId::new(3)).unwrap();
    let HirCopyCapability::User(user_construction) = &user_derived.copy_constructor else {
        panic!("derived operation should retain its user body");
    };
    assert_eq!(
        user_construction.base,
        Some(HirBaseCopy {
            base: synth_base.id,
            operation: synth_base.copy_constructor.selected().unwrap(),
        })
    );
    let HirCopyCapability::User(user_assignment) = &user_derived.copy_assignment else {
        panic!("derived operation should retain its user body");
    };
    assert_eq!(
        user_assignment.base,
        Some(HirBaseCopy {
            base: synth_base.id,
            operation: synth_base.copy_assignment.selected().unwrap(),
        })
    );

    let mir = lower_hir(&hir).expect("composed user and synthesized copies must lower");
    verify_mir(&mir).expect("composed base copy plans must verify");
    let MirCopyCapability::User(copy) = &mir.class(ClassId::new(3)).unwrap().copy_constructor
    else {
        panic!("MIR must retain the derived user copy operation");
    };
    assert_eq!(
        copy.base.as_ref().map(|step| step.base),
        Some(ClassId::new(2))
    );
}

#[test]
fn user_copy_operations_still_require_available_base_operations() {
    let mut program = resolve_text(concat!(
        "class Base { init() {} }\n",
        "class Derived extends Base {\n",
        "  init() { super(); }\n",
        "  copy(ref source: Derived) {}\n",
        "  assign(ref source: Derived) {}\n",
        "}\n",
        "fn main() -> i64 { var value: Derived = Derived(); return 0; }\n",
    ));
    program.classes.entries_mut_for_test()[0].copy_constructor = ResolvedCopyOperation::Unavailable;
    program.classes.entries_mut_for_test()[0].copy_assignment = ResolvedCopyOperation::Unavailable;

    let capabilities = CopyCapabilities::compute(&program);
    let derived = ClassId::new(1);
    assert_eq!(
        capabilities.constructor(derived),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.assignment(derived),
        &HirCopyCapability::Unavailable
    );
    assert_eq!(
        capabilities.constructor_failure(derived),
        Some([CopyPathElement::Base(ClassId::new(0))].as_slice())
    );
    assert_eq!(
        capabilities.assignment_failure(derived),
        Some([CopyPathElement::Base(ClassId::new(0))].as_slice())
    );

    let output = crate::typeck::type_check(&program);
    assert!(output.hir.is_none());
    let diagnostic = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == crate::typeck::COPY_OPERATION_UNAVAILABLE)
        .expect("using the derived class must diagnose its unavailable base copy");
    assert!(diagnostic
        .notes
        .iter()
        .any(|note| note.contains("base Base")));
}

#[test]
fn base_arguments_are_checked_before_derived_field_liveness() {
    let output = check_text(concat!(
        "class Base { init(value: bool) {} }\n",
        "class Derived extends Base {\n",
        "  value: i64;\n",
        "  init() { super(1); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let codes = output
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect::<Vec<_>>();
    assert_eq!(
        codes,
        [
            NO_MATCHING_INITIALIZER,
            crate::typeck::INVALID_INITIALIZER_BODY,
            FIELD_INITIALIZATION
        ]
    );
    assert!(output.hir.is_none());
}

#[test]
fn each_initializer_in_a_deep_chain_selects_its_base_overload_independently() {
    let output = check_text(concat!(
        "class Root {\n",
        "  init(value: i64) {}\n",
        "  init(value: bool) {}\n",
        "}\n",
        "class Middle extends Root {\n",
        "  init(value: i64) { super(value); }\n",
        "  init(value: bool) { super(value); }\n",
        "}\n",
        "class Leaf extends Middle {\n",
        "  init(value: i64) { super(value); }\n",
        "  init(value: bool) { super(value); }\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var integer: Leaf = Leaf(7);\n",
        "  var boolean: Leaf = Leaf(true);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    for (class, base) in [
        (ClassId::new(1), ClassId::new(0)),
        (ClassId::new(2), ClassId::new(1)),
    ] {
        let definition = hir.class_definitions.get(class).unwrap();
        let selected: Vec<_> = definition
            .initializers
            .iter()
            .map(|initializer| {
                let HirStatement::BaseInitialization(initialization) =
                    &initializer.body.statements[0]
                else {
                    panic!("derived initializer should begin with base initialization");
                };
                assert_eq!(initialization.base, base);
                initialization.initializer
            })
            .collect();
        assert_eq!(
            selected,
            [InitializerId::new(base, 0), InitializerId::new(base, 1)]
        );
    }
    let hir_dump = crate::hir::dump_hir(&hir);
    for selected in ["c0:init0", "c0:init1", "c1:init0", "c1:init1"] {
        assert!(hir_dump.contains(&format!("via {selected}")));
    }
    assert_eq!(hir_dump, crate::hir::dump_hir(&hir));

    let mir = lower_hir(&hir).expect("overloaded base initialization must lower");
    verify_mir(&mir).expect("overloaded base initialization must verify");
    for (class, base) in [
        (ClassId::new(1), ClassId::new(0)),
        (ClassId::new(2), ClassId::new(1)),
    ] {
        for ordinal in 0..2 {
            let definition = mir
                .member_definition(InitializerId::new(class, ordinal).into())
                .unwrap();
            assert!(definition.body.blocks.iter().any(|block| {
                block.instructions.iter().any(|instruction| {
                    matches!(
                        instruction,
                        MirInstruction::Initialize(initialization)
                            if initialization.target == InitializerId::new(base, ordinal)
                    )
                })
            }));
        }
    }
    let mir_dump = crate::mir::dump_mir(&mir);
    for selected in ["c0:init0", "c0:init1", "c1:init0", "c1:init1"] {
        assert!(mir_dump.contains(&format!("with {selected}(")));
    }
    assert_eq!(mir_dump, crate::mir::dump_mir(&mir));
}

#[test]
fn base_selection_uses_static_specificity_for_class_interface_and_obj_views() {
    let output = check_text(concat!(
        "interface Named {}\n",
        "class Animal { init() {} }\n",
        "class Dog extends Animal implements Named { init() { super(); } }\n",
        "class Root {\n",
        "  init(ref value: Obj) {}\n",
        "  init(ref value: Named) {}\n",
        "  init(ref value: Animal) {}\n",
        "  init(ref value: Dog) {}\n",
        "}\n",
        "class Derived extends Root {\n",
        "  init(ref value: Obj) { super(value); }\n",
        "  init(ref value: Animal) { super(value); }\n",
        "  init(ref value: Dog) { super(value); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let definition = hir.class_definitions.get(ClassId::new(3)).unwrap();
    let selected: Vec<_> = definition
        .initializers
        .iter()
        .map(|initializer| {
            let HirStatement::BaseInitialization(initialization) = &initializer.body.statements[0]
            else {
                panic!("expected base initialization");
            };
            initialization.initializer
        })
        .collect();
    assert_eq!(
        selected,
        [
            InitializerId::new(ClassId::new(2), 0),
            InitializerId::new(ClassId::new(2), 2),
            InitializerId::new(ClassId::new(2), 3),
        ]
    );
}

#[test]
fn base_selection_reports_deterministic_ambiguity_and_no_match_candidates() {
    let output = check_text(concat!(
        "interface Left {}\n",
        "interface Right {}\n",
        "class Both implements Left, Right { init() {} }\n",
        "class Base {\n",
        "  init(ref value: Left) {}\n",
        "  init(ref value: Right) {}\n",
        "  init(value: i64) {}\n",
        "}\n",
        "class Ambiguous extends Base { init(ref value: Both) { super(value); } }\n",
        "class Missing extends Base { init() { super(true); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    let ambiguous = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == AMBIGUOUS_INITIALIZER)
        .expect("unrelated interfaces should be ambiguous");
    assert!(ambiguous.message.contains("base initializer call"));
    assert_eq!(ambiguous.labels.len(), 3);
    assert!(ambiguous.labels[1].message.contains("init(ref Left)"));
    assert!(ambiguous.labels[2].message.contains("init(ref Right)"));

    let missing = output
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code == NO_MATCHING_INITIALIZER)
        .expect("incompatible argument should have no matching base initializer");
    assert!(missing.message.contains("base class `Base`"));
    assert_eq!(missing.labels.len(), 4);
    assert!(missing.labels[0].message.contains("supplied (bool)"));
}

#[test]
fn base_value_overloads_copy_and_slice_existing_and_produced_sources() {
    let output = check_text(concat!(
        "class ValueBase { init() {} }\n",
        "class ValueDerived extends ValueBase { init() { super(); } }\n",
        "class Base {\n",
        "  init(ref value: Obj) {}\n",
        "  init(value: ValueBase) {}\n",
        "}\n",
        "class FromExisting extends Base {\n",
        "  init(value: ValueDerived) { super(value); }\n",
        "}\n",
        "class FromProduced extends Base {\n",
        "  init() { super(ValueDerived()); }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    for (class, produced) in [(ClassId::new(3), false), (ClassId::new(4), true)] {
        let definition = hir.class_definitions.get(class).unwrap();
        let HirStatement::BaseInitialization(initialization) =
            &definition.initializers[0].body.statements[0]
        else {
            panic!("expected base initialization");
        };
        assert_eq!(
            initialization.initializer,
            InitializerId::new(ClassId::new(2), 1)
        );
        let HirCallArgument::Copy(argument) = &initialization.arguments[0] else {
            panic!("value overload should retain an owning copy argument");
        };
        let HirObjectSource::Slice(slice) = &argument.source else {
            panic!("derived source should slice into an exact base value");
        };
        assert_eq!(slice.target, ClassId::new(0));
        if produced {
            assert!(matches!(
                slice.source.as_ref(),
                HirObjectSource::Produced(_)
            ));
        } else {
            assert!(matches!(slice.source.as_ref(), HirObjectSource::Place(_)));
        }
    }
}

#[test]
fn object_argument_temporaries_remain_explicit_in_base_initialization_hir() {
    let output = check_text(concat!(
        "class Payload { init() {} }\n",
        "class Base { init(value: Payload) {} }\n",
        "class Derived extends Base { init() { super(make()); } }\n",
        "fn make() -> Payload { return Payload(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let definition = hir.class_definitions.get(ClassId::new(2)).unwrap();
    let HirStatement::BaseInitialization(initialization) =
        &definition.initializers[0].body.statements[0]
    else {
        panic!("expected base initialization");
    };
    let HirCallArgument::Copy(argument) = &initialization.arguments[0] else {
        panic!("object base argument must retain copy semantics");
    };
    assert!(matches!(argument.source, HirObjectSource::Produced(_)));
}

#[test]
fn derived_return_elision_keeps_the_composed_copy_selection() {
    let output = check_text(concat!(
        "class Base { init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn make() -> Derived { return Derived(); }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let derived = hir.class(ClassId::new(1)).unwrap();
    let definition = hir
        .definitions
        .get(FunctionId::new(0))
        .expect("expected object-returning function");
    let HirStatement::Return(result) = &definition.body.statements[0] else {
        panic!("expected return");
    };
    let Some(crate::hir::HirReturnValue::Object(crate::hir::HirObjectReturn::Construct {
        omitted_copy,
        ..
    })) = &result.value
    else {
        panic!("expected elided derived object result");
    };
    assert_eq!(*omitted_copy, derived.copy_constructor.selected().unwrap());
}

#[test]
fn base_arguments_cannot_read_incomplete_derived_fields() {
    let output = check_text(concat!(
        "class Base { init(value: i64) {} }\n",
        "class Derived extends Base {\n",
        "  value: i64;\n",
        "  init() { super(self.value); self.value = 1; }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == FIELD_INITIALIZATION
            && diagnostic.message.contains("used before initialization")
    }));
}

#[test]
fn mir_lowering_preserves_static_inheritance_semantics() {
    let output = check_text(concat!(
        "class Base { init() {} }\n",
        "class Derived extends Base { init() { super(); } }\n",
        "fn main() -> i64 { return 0; }\n",
    ));
    let hir = output.hir.unwrap();
    let mir = lower_hir(&hir).expect("static inheritance must lower to MIR");
    verify_mir(&mir).expect("lowered static inheritance must verify");
    let derived = mir.class(ClassId::new(1)).unwrap();
    assert_eq!(derived.direct_base.unwrap().class, ClassId::new(0));
    assert_eq!(
        derived.destruction.steps.last(),
        Some(&crate::mir::MirDestructionStep::Base(ClassId::new(0)))
    );
}

use super::*;
use crate::{
    hir::HirPrimitiveStorage,
    identity::{BindingId, ParameterId},
};

#[test]
fn checks_all_primitive_binding_assignment_types_exactly() {
    let output = check_text(concat!(
        "fn next(value: i64) -> i64 { return value + 1; }\n",
        "fn main() -> i64 {\n",
        "  var signed: i64 = 0;\n",
        "  var unsigned: u64 = 0u;\n",
        "  var byte: u8 = 0u8;\n",
        "  var float: f64 = 0.0;\n",
        "  var flag: bool = false;\n",
        "  signed = next(1);\n",
        "  (unsigned) = 2u;\n",
        "  byte = 3u8;\n",
        "  float = 4.0;\n",
        "  flag = true;\n",
        "  signed = signed + 5;\n",
        "  return signed;\n",
        "}\n",
    ));

    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let main = hir.definitions.get(hir.entry_function).unwrap();
    let assignments: Vec<_> = main
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::PrimitiveAssignment(assignment) => Some(assignment),
            _ => None,
        })
        .collect();
    assert_eq!(
        assignments
            .iter()
            .map(|assignment| assignment.source.ty)
            .collect::<Vec<_>>(),
        [
            Type::I64,
            Type::U64,
            Type::U8,
            Type::F64,
            Type::Bool,
            Type::I64
        ]
    );
    assert_eq!(
        assignments
            .iter()
            .map(|assignment| {
                let HirPrimitiveStorage::Binding(BindingId::Local(local)) =
                    assignment.destination.storage
                else {
                    panic!("expected local destination");
                };
                local.index()
            })
            .collect::<Vec<_>>(),
        [0, 1, 2, 3, 4, 0]
    );

    let dump = dump_hir(&hir);
    assert_eq!(dump, dump_hir(&hir));
    assert!(dump.contains("PrimitiveBindingAssignment f1:l0"));
    assert!(!dump.contains("PrimitiveBindingAssignment \"signed\""));
}

#[test]
fn checks_all_primitive_parameter_assignment_types_exactly() {
    let output = check_text(concat!(
        "fn update(signed: i64, unsigned: u64, byte: u8, float: f64, flag: bool) -> i64 {\n",
        "  signed = signed + 1;\n",
        "  unsigned = 2u;\n",
        "  byte = 3u8;\n",
        "  float = 4.0;\n",
        "  flag = true;\n",
        "  return signed;\n",
        "}\n",
        "fn main() -> i64 { return update(0, 0u, 0u8, 0.0, false); }\n",
    ));

    assert!(output.diagnostics.is_empty());
    let hir = output.hir.unwrap();
    let update = hir.definitions.get(FunctionId::new(0)).unwrap();
    let assignments: Vec<_> = update
        .body
        .statements
        .iter()
        .filter_map(|statement| match statement {
            HirStatement::PrimitiveAssignment(assignment) => Some(assignment),
            _ => None,
        })
        .collect();
    assert_eq!(
        assignments
            .iter()
            .map(|assignment| assignment.source.ty)
            .collect::<Vec<_>>(),
        [Type::I64, Type::U64, Type::U8, Type::F64, Type::Bool]
    );
    assert!(assignments.iter().enumerate().all(|(index, assignment)| {
        let parameter = ParameterId::new(FunctionId::new(0), index);
        assignment.destination.storage
            == HirPrimitiveStorage::Binding(BindingId::Parameter(parameter))
    }));
}

#[test]
fn rejects_each_cross_primitive_binding_assignment_without_conversion() {
    let output = check_text(concat!(
        "fn main() -> i64 {\n",
        "  var signed: i64 = 0;\n",
        "  var unsigned: u64 = 0u;\n",
        "  var byte: u8 = 0u8;\n",
        "  var float: f64 = 0.0;\n",
        "  var flag: bool = false;\n",
        "  signed = 1u;\n",
        "  unsigned = 1u8;\n",
        "  byte = 1;\n",
        "  float = true;\n",
        "  flag = 1.0;\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.hir.is_none());
    let mismatches: Vec<_> = output
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code == crate::typeck::TYPE_MISMATCH)
        .collect();
    assert_eq!(mismatches.len(), 5);
    assert!(mismatches
        .iter()
        .all(|diagnostic| diagnostic.message.contains("primitive binding assignment")));
}

#[test]
fn initializer_only_bodies_do_not_admit_primitive_binding_assignment() {
    let output = check_text(concat!(
        "class Value {\n",
        "  field: i64;\n",
        "  init(field: i64) {\n",
        "    var local: i64 = field;\n",
        "    local = 1;\n",
        "    self.field = field;\n",
        "  }\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == crate::typeck::INVALID_INITIALIZER_BODY
            && diagnostic.message.contains("initializer bodies")
    }));
}

use super::*;
use crate::{
    hir::{HirAccess, HirExpressionKind, HirStatement},
    identity::{BindingId, ClassId, FieldId, FunctionId},
    object_path::ObjectProjection,
};

#[test]
fn supports_nested_places_across_every_live_root_kind() {
    let output = check_text(concat!(
        "class Leaf {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  fn read() -> i64 { return self.value; }\n",
        "  mut fn add(amount: i64) -> i64 { self.value = self.value + amount; return self.value; }\n",
        "}\n",
        "class Branch { leaf: Leaf; init(value: i64) { self.leaf = Leaf(value); } }\n",
        "fn read(ref leaf: Leaf) -> i64 { return leaf.read(); }\n",
        "fn mutate(mut ref leaf: Leaf, amount: i64) -> i64 { return leaf.add(amount); }\n",
        "fn overlap(mut ref left: Leaf, mut ref right: Leaf) -> unit {}\n",
        "class Root {\n",
        "  branch: Branch; result: i64;\n",
        "  init(mut ref external: Leaf, value: i64) {\n",
        "    self.branch = Branch(value);\n",
        "    self.result = mutate(self.branch.leaf, 1) + read(self.branch.leaf) + external.add(1);\n",
        "  }\n",
        "  fn nested() -> i64 { return self.branch.leaf.read() + self.branch.leaf.value; }\n",
        "  mut fn edit() -> i64 {\n",
        "    self.branch.leaf.value = self.branch.leaf.value + 1;\n",
        "    return mutate(self.branch.leaf, 1);\n",
        "  }\n",
        "}\n",
        "fn through_ref(ref root: Root) -> i64 {\n",
        "  return read(root.branch.leaf) + root.branch.leaf.read();\n",
        "}\n",
        "fn through_mut(mut ref root: Root) -> i64 {\n",
        "  root.branch.leaf.value = root.branch.leaf.value + 1;\n",
        "  return mutate(root.branch.leaf, 1);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var external: Leaf = Leaf(1);\n",
        "  var root: Root = Root(external, 10);\n",
        "  root.branch.leaf.value = root.branch.leaf.value + 1;\n",
        "  overlap(root.branch.leaf, root.branch.leaf);\n",
        "  return through_ref(root) + through_mut(root) + root.nested() + root.edit() + root.branch.leaf.value;\n",
        "}\n",
    ));

    assert!(!output.has_errors(), "{:?}", output.diagnostics);
    let hir = output.hir.unwrap();
    let root = hir.class(ClassId::new(2)).unwrap();
    let branch_projection = FieldId::new(root.id, 0);
    let leaf_projection = FieldId::new(ClassId::new(1), 0);

    let through_ref = hir.definitions.get(FunctionId::new(3)).unwrap();
    let HirStatement::Return(statement) = &through_ref.body.statements[0] else {
        panic!("expected return statement");
    };
    let HirReturnValue::Scalar(value) = statement.value.as_ref().expect("expected return value")
    else {
        panic!("expected scalar return");
    };
    let HirExpressionKind::Binary { left, right, .. } = &value.kind else {
        panic!("expected nested read expression");
    };
    let HirExpressionKind::DirectCall { arguments, .. } = &left.kind else {
        panic!("expected nested alias call");
    };
    let (_, alias) = class_alias_view(&arguments[0]);
    assert_eq!(
        alias.root(),
        BindingId::Parameter(hir.declarations.get(FunctionId::new(3)).unwrap().parameters[0].id)
    );
    assert_eq!(
        alias.projections(),
        [
            ObjectProjection::Field(branch_projection),
            ObjectProjection::Field(leaf_projection)
        ]
    );
    assert_eq!(alias.access, HirAccess::ReadOnly);

    let HirExpressionKind::MethodCall { receiver, .. } = &right.kind else {
        panic!("expected nested method call");
    };
    assert_eq!(
        receiver.place.projections(),
        [
            ObjectProjection::Field(branch_projection),
            ObjectProjection::Field(leaf_projection)
        ]
    );
    assert_eq!(receiver.place.access, HirAccess::ReadOnly);

    let through_mut = hir.definitions.get(FunctionId::new(4)).unwrap();
    let HirStatement::FieldAssignment(assignment) = &through_mut.body.statements[0] else {
        panic!("expected nested field assignment");
    };
    assert_eq!(
        assignment.place.receiver.projections(),
        [
            ObjectProjection::Field(branch_projection),
            ObjectProjection::Field(leaf_projection)
        ]
    );
    assert_eq!(assignment.place.receiver.access, HirAccess::Mutable);

    let dump = dump_hir(&hir);
    assert!(
        dump.contains("ObjectPlace f4:p0 -> c2:field0 -> c1:field0 : class c0 mutable @1049..1065"),
        "{dump}"
    );
    assert!(
        dump.contains("ObjectPlace f3:p0 -> c2:field0 -> c1:field0 : class c0 readonly @956..972"),
        "{dump}"
    );
}

#[test]
fn enforces_one_access_matrix_through_nested_paths() {
    let output = check_text(concat!(
        "class Leaf {\n",
        "  value: i64;\n",
        "  init() { self.value = 0; }\n",
        "  mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
        "}\n",
        "class Branch { leaf: Leaf; init() { self.leaf = Leaf(); } }\n",
        "fn mutate(mut ref leaf: Leaf) -> unit {}\n",
        "class Root {\n",
        "  branch: Branch;\n",
        "  init() { self.branch = Branch(); }\n",
        "  fn reject() -> unit {\n",
        "    self.branch.leaf.value = 1;\n",
        "    self.branch.leaf.add(1);\n",
        "    mutate(self.branch.leaf);\n",
        "  }\n",
        "}\n",
        "fn reject(ref root: Root) -> unit {\n",
        "  root.branch.leaf.value = 1;\n",
        "  root.branch.leaf.add(1);\n",
        "  mutate(root.branch.leaf);\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == READ_ONLY_RECEIVER)
            .count(),
        4
    );
    assert_eq!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INSUFFICIENT_ALIAS_ACCESS)
            .count(),
        2
    );
}

#[test]
fn accepts_nested_copy_sources_but_rejects_scalar_and_alias_replacement_contexts() {
    let output = check_text(concat!(
        "class Leaf { value: i64; init() { self.value = 0; } }\n",
        "class Branch { leaf: Leaf; init() { self.leaf = Leaf(); } }\n",
        "class Root { branch: Branch; init() { self.branch = Branch(); } }\n",
        "fn scalar(value: i64) -> unit {}\n",
        "fn take_branch(ref branch: Branch) -> unit {}\n",
        "fn invalid_return(ref root: Root) -> i64 { return root.branch.leaf; }\n",
        "fn invalid_uses(mut ref root: Root, ref other: Leaf) -> unit {\n",
        "  scalar(root.branch.leaf);\n",
        "  take_branch(root.branch.leaf);\n",
        "  var copy: Leaf = root.branch.leaf;\n",
        "  root.branch.leaf = other;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert!(output.hir.is_none());
    assert!(
        output
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == INVALID_OBJECT_CONTEXT)
            .count()
            >= 2
    );
    assert!(output
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == TYPE_MISMATCH));
    assert!(output.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == INVALID_OBJECT_CONTEXT
            && diagnostic.message.contains("alias-rooted object")
    }));
}

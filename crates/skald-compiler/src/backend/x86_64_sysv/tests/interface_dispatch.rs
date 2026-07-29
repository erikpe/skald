use super::*;

#[test]
fn emits_witness_entries_in_typed_identity_order() {
    let output = assembly(concat!(
        "interface Ordered {\n",
        "  fn later(value: u64) -> u64;\n",
        "  fn earlier() -> u64;\n",
        "}\n",
        "interface Other { fn other() -> u64; }\n",
        "class Worker implements Other, Ordered {\n",
        "  init() {}\n",
        "  fn earlier() -> u64 { return 1u; }\n",
        "  fn other() -> u64 { return 2u; }\n",
        "  fn later(value: u64) -> u64 { return value; }\n",
        "}\n",
        "fn invoke(ref value: Ordered) -> u64 {\n",
        "  return value.later(7u) + value.earlier();\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var value: Worker = Worker();\n",
        "  var result: u64 = invoke(value);\n",
        "  return 0;\n",
        "}\n",
    ));

    assert!(output.contains(concat!(
        ".Lska.class.main.Worker.c0.dispatch:\n",
        "    .quad .Lska.class.main.Worker.c0.method.later.m2\n",
        "    .quad .Lska.class.main.Worker.c0.method.earlier.m0\n",
        "    .quad .Lska.class.main.Worker.c0.method.other.m1\n",
    )));
    assert_eq!(output.matches("call r11").count(), 2);
    assert_system_assembler_accepts(&output);
}

#[test]
fn executes_multiple_interfaces_mutation_and_deep_overrides_through_self() {
    let source = concat!(
        "interface Readable { fn read() -> i64; }\n",
        "interface Mutable { mut fn bump(amount: i64) -> i64; }\n",
        "class Root implements Mutable, Readable {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "  virtual fn read() -> i64 { return self.value; }\n",
        "  virtual mut fn bump(amount: i64) -> i64 {\n",
        "    self.value = self.value + amount;\n",
        "    return self.value;\n",
        "  }\n",
        "  mut fn bump_via_self(amount: i64) -> i64 {\n",
        "    return invoke_mutable(self, amount);\n",
        "  }\n",
        "}\n",
        "class Middle extends Root {\n",
        "  init(value: i64) { super(value); }\n",
        "}\n",
        "class Leaf extends Middle {\n",
        "  init(value: i64) { super(value); }\n",
        "  override fn read() -> i64 { return self.value + 20; }\n",
        "  override mut fn bump(amount: i64) -> i64 {\n",
        "    self.value = self.value + amount + 10;\n",
        "    return self.value;\n",
        "  }\n",
        "}\n",
        "fn invoke_mutable(mut ref value: Mutable, amount: i64) -> i64 {\n",
        "  return value.bump(amount);\n",
        "}\n",
        "fn observe(ref value: Obj) -> unit {}\n",
        "fn invoke_readable(ref value: Readable) -> i64 {\n",
        "  observe(value);\n",
        "  return value.read();\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var middle: Middle = Middle(1);\n",
        "  var leaf: Leaf = Leaf(1);\n",
        "  var first: i64 = middle.bump_via_self(2);\n",
        "  var second: i64 = leaf.bump_via_self(2);\n",
        "  return first + second + invoke_readable(middle) + invoke_readable(leaf);\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(52));
}

#[test]
fn preserves_interface_metadata_through_forwarding_recursion_and_stack_pressure() {
    let source = concat!(
        "interface Runner {\n",
        "  fn run(flag: bool, value: i64, a: i64, b: i64, c: i64,\n",
        "      d: i64, e: i64, x: f64, y: f64) -> i64;\n",
        "}\n",
        "class Base implements Runner {\n",
        "  init() {}\n",
        "  virtual fn run(flag: bool, value: i64, a: i64, b: i64, c: i64,\n",
        "      d: i64, e: i64, x: f64, y: f64) -> i64 {\n",
        "    if (flag) { return forward(self, false, value, a, b, c, d, e, x, y); }\n",
        "    else { return value; }\n",
        "  }\n",
        "}\n",
        "class Middle extends Base { init() { super(); } }\n",
        "class Leaf extends Middle {\n",
        "  init() { super(); }\n",
        "  override fn run(flag: bool, value: i64, a: i64, b: i64, c: i64,\n",
        "      d: i64, e: i64, x: f64, y: f64) -> i64 {\n",
        "    if (flag) { return forward(self, false, value + 7, a, b, c, d, e, x, y); }\n",
        "    else { return value; }\n",
        "  }\n",
        "}\n",
        "fn forward(ref runner: Runner, flag: bool, value: i64, a: i64, b: i64,\n",
        "    c: i64, d: i64, e: i64, x: f64, y: f64) -> i64 {\n",
        "  return runner.run(flag, value, a, b, c, d, e, x, y);\n",
        "}\n",
        "fn main() -> i64 {\n",
        "  var middle: Middle = Middle();\n",
        "  var leaf: Leaf = Leaf();\n",
        "  return forward(middle, true, 10, 1, 2, 3, 4, 5, 1.0, 2.0)\n",
        "      + forward(leaf, true, 10, 1, 2, 3, 4, 5, 1.0, 2.0);\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(27));
}

#[test]
fn interface_object_results_reuse_the_hidden_destination_path() {
    let source = concat!(
        "interface Factory { fn make(value: i64) -> Product; }\n",
        "class Product {\n",
        "  value: i64;\n",
        "  init(value: i64) { self.value = value; }\n",
        "}\n",
        "class Maker implements Factory {\n",
        "  init() {}\n",
        "  fn make(value: i64) -> Product { return Product(value + 9); }\n",
        "}\n",
        "fn invoke(ref factory: Factory) -> Product { return factory.make(30); }\n",
        "fn main() -> i64 {\n",
        "  var maker: Maker = Maker();\n",
        "  var product: Product = invoke(maker);\n",
        "  return product.value;\n",
        "}\n",
    );

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(39));
}

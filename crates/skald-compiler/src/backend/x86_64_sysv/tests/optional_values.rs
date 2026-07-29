use super::super::layout;
use super::*;

#[test]
fn class_optional_layout_reserves_an_aligned_exact_payload() {
    let program = lower_text(
        "class Value { byte: u8; wide: i64; init() { self.byte = 1u8; self.wide = 2; } }\n\
         fn main() -> i64 { var value: Value? = none; return 0; }\n",
    );
    let layouts = layout::DataLayout::compute(&program).unwrap();
    let payload = layouts.class(ClassId::new(0)).unwrap().ty();
    let optional = layouts.optional_class(ClassId::new(0)).unwrap();

    assert_eq!(optional.payload_offset() % payload.alignment(), 0);
    assert!(optional.ty().size() >= optional.payload_offset() + payload.size());
    assert_eq!(optional.ty().alignment(), payload.alignment().max(8));
}

#[test]
fn optional_shared_owner_uses_the_zero_niche_one_word_layout() {
    let program = lower_text(
        "class Value { init() {} }\n\
         class Holder { value: shared? Value; init() { self.value = none; } }\n\
         fn main() -> i64 { var value: shared? Value = none; return 0; }\n",
    );
    let layouts = layout::DataLayout::compute(&program).unwrap();
    let field = layouts.field(FieldId::new(ClassId::new(1), 0)).unwrap();
    let optional = layouts
        .ty(MirType::OptionalShared(MirSharedTarget::Class(
            ClassId::new(0),
        )))
        .unwrap();

    assert_eq!(field.offset, 0);
    assert_eq!(optional.size(), 8);
    assert_eq!(optional.alignment(), 8);
}

#[test]
fn optional_shared_lifecycle_fields_calls_copy_self_assignment_and_unwrap_execute() {
    let source = concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  fn read() -> i64 { return self.marker; }\n",
        "  destroy { ska_rt_println_i64(self.marker); }\n",
        "}\n",
        "class Holder {\n",
        "  value: shared? Value;\n",
        "  init(value: shared? Value) { self.value = value; }\n",
        "}\n",
        "fn forward(value: shared? Value) -> shared? Value { return value; }\n",
        "fn main() -> i64 {\n",
        "  var first: shared? Value = new Value(42);\n",
        "  var second: shared? Value = first;\n",
        "  first = first;\n",
        "  var holder: Holder = Holder(forward(second));\n",
        "  var copied: Holder = holder;\n",
        "  copied = copied;\n",
        "  first = none;\n",
        "  second = none;\n",
        "  return copied.value!->read();\n",
        "}\n",
    );
    let mir_dump = crate::mir::dump_mir(&lower_text(source));
    assert!(mir_dump.contains("optional-shared-initialize"));
    assert!(mir_dump.contains("optional-shared-assign"));
    assert!(mir_dump.contains("optional-shared-cleanup"));
    assert!(mir_dump.contains("optional-shared-unwrap"));
    assert!(mir_dump.contains("return-optional-shared"));

    let mut output = assembly(source);
    assert!(output.contains("shared_copy_overflow"));
    assert!(output.contains("shared_copy_invalid"));
    assert!(output.contains("shared_unwrap_overflow"));
    assert!(output.contains("shared_unwrap_invalid"));
    output.push_str(optional_ownership_stubs());
    output.push_str(println_i64_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42), "{output}");
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn optional_shared_parameters_results_and_stack_pressure_execute() {
    let source = "class Value { marker: i64; init(marker: i64) { self.marker = marker; } }\n\
        fn choose(a: shared? Value, b: shared? Value, c: shared? Value,\n\
                  d: shared? Value, e: shared? Value, f: shared? Value,\n\
                  g: shared? Value) -> shared? Value {\n\
          if (g is some) { return g; }\n\
          return a;\n\
        }\n\
        fn main() -> i64 {\n\
          var result: shared? Value = choose(none, none, none, none, none, none, new Value(42));\n\
          return result!->marker;\n\
        }\n";
    let mut output = assembly(source);
    output.push_str(optional_ownership_stubs());

    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn optional_shared_unwrap_secures_anchors_and_composes_with_runtime_casts() {
    let source = "interface Readable { fn read() -> i64; }\n\
        class Value implements Readable {\n\
          marker: i64;\n\
          init(marker: i64) { self.marker = marker; }\n\
          fn read() -> i64 { return self.marker; }\n\
        }\n\
        class Holder {\n\
          value: shared? Value;\n\
          init(value: shared? Value) { self.value = value; }\n\
          mut fn clear() -> i64 { self.value = none; return 0; }\n\
        }\n\
        fn consume(ref value: Value, ignored: i64) -> i64 {\n\
          return value.marker + ignored;\n\
        }\n\
        fn main() -> i64 {\n\
          var concrete: shared? Value = new Value(42);\n\
          var readable: shared? Readable = concrete;\n\
          var object: shared? Obj = readable;\n\
          var recovered: shared Value = (shared Value) object!;\n\
          var holder: Holder = Holder(recovered);\n\
          return consume(*holder.value!, holder.clear());\n\
        }\n";
    let mut output = assembly(source);
    output.push_str(optional_ownership_stubs());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn absent_optional_shared_unwrap_terminates() {
    let mut output = assembly(
        "class Value { init() {} fn read() -> i64 { return 42; } }\n\
         fn main() -> i64 {\n\
           var value: shared? Value = none;\n\
           return value!->read();\n\
         }\n",
    );
    output.push_str(optional_ownership_stubs());

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn primitive_optionals_execute_present_payloads_for_every_primitive() {
    let output = assembly(
        "fn main() -> i64 {\n\
           var signed: i64? = 40;\n\
           var unsigned: u64? = 7u;\n\
           var byte: u8? = 8u8;\n\
           var float: f64? = 1.5;\n\
           var flag: bool? = true;\n\
           var signed_value: i64 = signed!;\n\
           var unsigned_value: u64 = unsigned!;\n\
           var byte_value: u8 = byte!;\n\
           var float_value: f64 = float!;\n\
           var flag_value: bool = flag!;\n\
           if (flag_value) {\n\
             if (unsigned is some) {\n\
               if (byte is some) {\n\
                 if (float is some) { return signed_value + 2; }\n\
               }\n\
             }\n\
           }\n\
           return 0;\n\
         }\n",
    );

    assert!(output.contains("call ska_rt_panic"));
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn primitive_optional_absence_presence_copy_assignment_and_joins_execute() {
    let source = "fn main() -> i64 {\n\
        var value: i64? = none;\n\
        var copy: i64? = value;\n\
        if (copy is none) { value = 19; } else { value = 1; }\n\
        copy = value;\n\
        if (copy is some) { return copy! + 4; }\n\
        return 0;\n\
    }\n";
    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(23));
}

#[test]
fn absent_primitive_optional_unwrap_terminates() {
    let output = assembly(
        "fn main() -> i64 {\n\
           var value: i64? = none;\n\
           return value!;\n\
         }\n",
    );

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn checked_class_payload_views_execute_mutation_and_overlap() {
    let source = "class Item {\n\
        value: i64;\n\
        init(value: i64) { self.value = value; }\n\
        mut fn set(value: i64) -> unit { self.value = value; }\n\
    }\n\
    class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
    fn sum(ref left: Item, ref right: Item) -> i64 { return left.value + right.value; }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(Item(20));\n\
        holder.item!.set(21);\n\
        return sum(holder.item!, holder.item!);\n\
    }\n";

    let output = assembly(source);
    assert!(
        output.contains("0xffffffffffffffff"),
        "guard overflow must be checked without a runtime helper"
    );
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn saturated_optional_guard_state_terminates_on_checked_view_begin() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
        class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
        fn main() -> i64 {\n\
            var holder: Holder = Holder(Item(42));\n\
            return holder.item!.value;\n\
        }\n";
    let output = assembly(source);
    let begin_view = concat!(
        "call .Lska_class_1_init_0\n",
        "    mov rax, qword ptr [rbp - 16]"
    );
    assert_eq!(
        output.matches(begin_view).count(),
        1,
        "fixture must identify the checked container state load\n{output}"
    );
    let saturated = output.replacen(
        begin_view,
        concat!(
            "call .Lska_class_1_init_0\n",
            "    mov rax, -1\n",
            "    mov qword ptr [rbp - 16], rax\n",
            "    mov rax, qword ptr [rbp - 16]"
        ),
        1,
    );

    assert!(!run_native_assembly(&saturated).success(), "{saturated}");
}

#[test]
fn dynamically_pinned_optional_state_terminates_before_container_destruction() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
        class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
        fn main() -> i64 {\n\
            var holder: Holder = Holder(Item(42));\n\
            return 42;\n\
        }\n";
    let output = assembly(source);
    let initialized = concat!(
        "call .Lska_class_1_init_0\n",
        "    mov rax, 42\n",
        "    mov qword ptr [rbp - 48], rax"
    );
    assert_eq!(
        output.matches(initialized).count(),
        1,
        "fixture must identify the container state before cleanup\n{output}"
    );
    let pinned = output.replacen(
        initialized,
        concat!(
            "call .Lska_class_1_init_0\n",
            "    mov rax, 2\n",
            "    mov qword ptr [rbp - 16], rax\n",
            "    mov rax, 42\n",
            "    mov qword ptr [rbp - 48], rax"
        ),
        1,
    );

    assert!(!run_native_assembly(&pinned).success(), "{pinned}");
}

#[test]
fn checked_class_payload_view_terminates_before_later_argument_can_clear_it() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
    class Holder {\n\
        item: Item?;\n\
        init(item: Item?) { self.item = item; }\n\
        mut fn clear() -> i64 { self.item = none; return 0; }\n\
    }\n\
    fn consume(ref item: Item, ignored: i64) -> i64 { return item.value + ignored; }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(Item(42));\n\
        return consume(holder.item!, holder.clear());\n\
    }\n";

    assert!(!run_native_assembly(&assembly(source)).success());
}

#[test]
fn checked_class_payload_view_rejects_reentrant_clearing() {
    let source = "class Holder {\n\
        item: Item?;\n\
        init(item: Item?) { self.item = item; }\n\
        mut fn clear() -> unit { self.item = none; }\n\
    }\n\
    class Item {\n\
        owner: shared Holder;\n\
        init(owner: shared Holder) { self.owner = owner; }\n\
        mut fn set_owner(owner: shared Holder) -> unit { self.owner = owner; }\n\
        mut fn clear_owner() -> unit { self.owner->clear(); }\n\
    }\n\
    fn main() -> i64 {\n\
        var bootstrap: shared Holder = new Holder(none);\n\
        var holder: shared Holder = new Holder(Item(bootstrap));\n\
        holder->item!.set_owner(holder);\n\
        holder->item!.clear_owner();\n\
        return 42;\n\
    }\n";
    let mut output = assembly(source);
    output.push_str(optional_ownership_stubs());

    assert!(!run_native_assembly(&output).success());
}

#[test]
fn copied_class_payload_is_unpinned_before_later_arguments() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
    class Holder {\n\
        item: Item?;\n\
        init(item: Item?) { self.item = item; }\n\
        mut fn clear() -> i64 { self.item = none; return 0; }\n\
    }\n\
    fn consume(item: Item, ignored: i64) -> i64 { return item.value + ignored; }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(Item(42));\n\
        return consume(holder.item!, holder.clear());\n\
    }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

#[test]
fn absent_checked_class_payload_view_terminates() {
    let source = "class Item { value: i64; init(value: i64) { self.value = value; } }\n\
    class Holder { item: Item?; init(item: Item?) { self.item = item; } }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(none);\n\
        return holder.item!.value;\n\
    }\n";

    assert!(!run_native_assembly(&assembly(source)).success());
}

#[test]
fn optional_fields_calls_results_and_stack_pressure_execute() {
    let source = "class Holder {\n\
        value: i64?;\n\
        init(value: i64?) { self.value = value; }\n\
        mut fn replace(value: i64?) -> i64? {\n\
            self.value = self.value;\n\
            self.value = value;\n\
            return self.value;\n\
        }\n\
    }\n\
    fn choose(a: i64?, b: i64?, c: i64?, d: i64?, e: i64?, f: i64?, g: i64?) -> i64? {\n\
        if (g is some) { return g; }\n\
        return a;\n\
    }\n\
    fn main() -> i64 {\n\
        var holder: Holder = Holder(none);\n\
        var result: i64? = holder.replace(choose(1, 2, 3, 4, 5, 6, 7));\n\
        var copy: Holder = holder;\n\
        copy = holder;\n\
        return copy.value!;\n\
    }\n";
    let output = assembly(source);

    assert!(output.contains("call .Lska_fn_"));
    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(7), "{output}");
}

#[test]
fn optional_values_execute_through_virtual_and_interface_calls() {
    let source = "interface Maybe {\n\
        fn forward(value: i64?) -> i64?;\n\
    }\n\
    class Base {\n\
        init() {}\n\
        virtual fn forward(value: i64?) -> i64? { return value; }\n\
    }\n\
    class Derived extends Base implements Maybe {\n\
        init() { super(); }\n\
        override fn forward(value: i64?) -> i64? { return value; }\n\
    }\n\
    fn through_interface(ref source: Maybe, value: i64?) -> i64? {\n\
        return source.forward(value);\n\
    }\n\
    fn through_virtual(ref source: Base, value: i64?) -> i64? {\n\
        return source.forward(value);\n\
    }\n\
    fn main() -> i64 {\n\
        var item: Derived = Derived();\n\
        var left: i64? = through_interface(item, 20);\n\
        var right: i64? = through_virtual(item, 22);\n\
        return left! + right!;\n\
    }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

#[test]
fn class_optional_lifecycle_executes_through_calls_fields_and_assignment() {
    let source = concat!(
        "extern fn ska_rt_println_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  destroy { ska_rt_println_i64(self.marker); }\n",
        "}\n",
        "class Holder {\n",
        "  value: Value?;\n",
        "  init(value: Value?) { self.value = value; }\n",
        "}\n",
        "fn forward(value: Value?) -> Value? { return value; }\n",
        "fn main() -> i64 {\n",
        "  var first: Value? = Value(42);\n",
        "  var second: Value? = first;\n",
        "  first = none;\n",
        "  first = forward(Value(42));\n",
        "  var holder: Holder = Holder(first);\n",
        "  if (holder.value is some) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(println_i64_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42));
    assert_eq!(result.stdout, b"42\n42\n42\n42\n42\n42\n42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn class_optional_parameters_and_results_execute_under_stack_pressure() {
    let source = "class Value { init() {} }\n\
        fn choose(a: Value?, b: Value?, c: Value?, d: Value?, e: Value?, f: Value?, g: Value?) -> Value? {\n\
          if (g is some) { return g; }\n\
          return a;\n\
        }\n\
        fn main() -> i64 {\n\
          var result: Value? = choose(none, none, none, none, none, none, Value());\n\
          if (result is some) { return 42; }\n\
          return 0;\n\
        }\n";
    let output = assembly(source);

    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn optional_container_aliases_execute_forward_mutate_and_stack_pressure() {
    let source = "class Value {\n\
          marker: i64;\n\
          init(marker: i64) { self.marker = marker; }\n\
          fn read() -> i64 { return self.marker; }\n\
        }\n\
        class Holder {\n\
          value: i64?;\n\
          init(value: i64?) { self.value = value; }\n\
        }\n\
        fn read(ref value: i64?) -> i64 {\n\
          if (value is some) { return value!; }\n\
          return 0;\n\
        }\n\
        fn forward(ref value: i64?) -> i64 { return read(value); }\n\
        fn clear(mut ref value: i64?) -> unit { value = none; }\n\
        fn replace(mut ref value: Value?) -> unit { value = Value(9); }\n\
        fn pressure(ref a: i64?, ref b: i64?, ref c: i64?, ref d: i64?,\n\
                    ref e: i64?, ref f: i64?, ref g: i64?) -> i64 {\n\
          return read(a) + read(b) + read(c) + read(d) + read(e) + read(f) + read(g);\n\
        }\n\
        fn main() -> i64 {\n\
          var a: i64? = 1;\n\
          var b: i64? = 2;\n\
          var c: i64? = 3;\n\
          var d: i64? = 4;\n\
          var e: i64? = 5;\n\
          var f: i64? = 6;\n\
          var g: i64? = 7;\n\
          var item: Value? = none;\n\
          var holder: Holder = Holder(7);\n\
          replace(item);\n\
          var first: i64 = pressure(a, b, c, d, e, f, g);\n\
          var second: i64 = forward(g);\n\
          var third: i64 = item!.read();\n\
          var fourth: i64 = forward(holder.value);\n\
          var result: i64 = first + second + third + fourth;\n\
          clear(g);\n\
          clear(holder.value);\n\
          if (g is none) {\n\
            if (holder.value is none) { return result - 9; }\n\
          }\n\
          return 0;\n\
        }\n";
    let mir = lower_text(source);
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("ref i64?"));
    assert!(dump.contains("mut ref class"));
    assert!(dump.contains("indirect("));

    let output = assembly(source);
    assert!(output.contains("mov qword ptr [rsp]"));
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn optional_container_alias_cannot_clear_a_checked_payload() {
    let source = "class Value { init() {} }\n\
        fn consume(ref value: Value, ignored: i64) -> i64 { return ignored; }\n\
        fn clear(mut ref value: Value?) -> i64 { value = none; return 0; }\n\
        fn main() -> i64 {\n\
          var value: Value? = Value();\n\
          return consume(value!, clear(value));\n\
        }\n";
    let output = assembly(source);

    let status = run_native_assembly(&output);
    assert!(!status.success(), "{output}");
    assert_ne!(status.code(), Some(0), "{output}");
}

#[test]
fn optional_alias_signatures_execute_through_virtual_and_interface_dispatch() {
    let source = "interface Reader { fn read(ref value: i64?) -> i64; }\n\
        class Base {\n\
          init() {}\n\
          virtual fn read(ref value: i64?) -> i64 {\n\
            if (value is some) { return value!; }\n\
            return 0;\n\
          }\n\
        }\n\
        class Derived extends Base implements Reader {\n\
          init() { super(); }\n\
          override fn read(ref value: i64?) -> i64 {\n\
            if (value is some) { return value!; }\n\
            return 0;\n\
          }\n\
        }\n\
        fn through_interface(ref source: Reader, ref value: i64?) -> i64 {\n\
          return source.read(value);\n\
        }\n\
        fn through_virtual(ref source: Base, ref value: i64?) -> i64 {\n\
          return source.read(value);\n\
        }\n\
        fn main() -> i64 {\n\
          var source: Derived = Derived();\n\
          var value: i64? = 21;\n\
          return through_interface(source, value) + through_virtual(source, value);\n\
        }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

fn optional_ownership_stubs() -> &'static str {
    concat!(
        "\n.text\n",
        ".globl ska_rt_alloc\n",
        ".type ska_rt_alloc, @function\n",
        "ska_rt_alloc:\n",
        "    jmp malloc@PLT\n",
        ".size ska_rt_alloc, .-ska_rt_alloc\n",
        ".globl ska_rt_free\n",
        ".type ska_rt_free, @function\n",
        "ska_rt_free:\n",
        "    jmp free@PLT\n",
        ".size ska_rt_free, .-ska_rt_free\n",
    )
}

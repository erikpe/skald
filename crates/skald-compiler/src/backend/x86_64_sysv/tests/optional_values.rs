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
    let optional_id = program
        .optional_for_payload(MirType::Class(ClassId::new(0)))
        .unwrap();
    let optional = layouts.optional_type(optional_id).unwrap();

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
        .ty(MirType::Optional(
            program
                .optional_for_payload(MirType::Shared(MirSharedTarget::Class(ClassId::new(0))))
                .unwrap(),
        ))
        .unwrap();

    assert_eq!(field.offset, 0);
    assert_eq!(optional.size(), 8);
    assert_eq!(optional.alignment(), 8);
}

#[test]
fn every_deep_optional_layer_has_distinct_aligned_state_and_payload_storage() {
    let ty = format!("i64{}", "?".repeat(12));
    let source = format!("fn main() -> i64 {{ var value: {ty} = none; return 0; }}\n");
    let program = lower_text(&source);
    let layouts = layout::DataLayout::compute(&program).unwrap();
    assert_eq!(program.optional_types.iter().len(), 12);

    let mut previous_size = 0;
    for optional in program.optional_types.iter() {
        let current = layouts.optional_type(optional.id).unwrap();
        assert_eq!(current.state_offset(), 0);
        assert_eq!(
            current.payload_offset() % current.ty().alignment().min(8),
            0
        );
        assert!(current.ty().size() > previous_size);
        previous_size = current.ty().size();
    }
}

#[test]
fn optional_shared_lifecycle_fields_calls_copy_self_assignment_and_unwrap_execute() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  fn read() -> i64 { return self.marker; }\n",
        "  destroy { test_record_i64(self.marker); }\n",
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
    output.push_str(native_allocator());
    output.push_str(record_i64_stub());
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
    output.push_str(native_allocator());

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
    output.push_str(native_allocator());

    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
}

#[test]
fn optional_shared_unwrap_directly_initializes_locals_across_source_contexts() {
    let source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  destroy { test_record_i64(self.marker); }\n",
        "}\n",
        "class Holder {\n",
        "  value: shared? Value;\n",
        "  init(value: shared? Value) { self.value = value; }\n",
        "}\n",
        "fn forward(value: shared? Value) -> shared? Value { return value; }\n",
        "fn recover(value: shared? Value) -> shared Value {\n",
        "  var owner: shared Value = value!;\n",
        "  return owner;\n",
        "}\n",
        "fn inspect(value: shared Value) -> i64 { return value->marker; }\n",
        "fn main() -> i64 {\n",
        "  var seed: shared Value = new Value(42);\n",
        "  var maybe: shared? Value = seed;\n",
        "  var values: (shared? Value)[] = (shared? Value)[]{maybe};\n",
        "  var holder: Holder = Holder(maybe);\n",
        "  var from_element: shared Value = values[0]!;\n",
        "  var from_field: shared Value = holder.value!;\n",
        "  var from_optional_result: shared Value = forward(maybe)!;\n",
        "  var from_parameter: shared Value = recover(maybe);\n",
        "  if (inspect(from_element) != 42) { return 1; }\n",
        "  if (from_field->marker != 42) { return 2; }\n",
        "  if (from_optional_result->marker != 42) { return 3; }\n",
        "  return from_parameter->marker;\n",
        "}\n",
    );
    let mut output = assembly(source);
    output.push_str(native_allocator());
    output.push_str(record_i64_stub());
    let result = run_native_assembly_output(&output);

    assert_eq!(result.status.code(), Some(42), "{output}");
    assert_eq!(result.stdout, b"42\n");
    assert!(result.stderr.is_empty());
}

#[test]
fn absent_optional_shared_unwrap_terminates() {
    let mut output = assembly(
        "class Value { init() {} fn read() -> i64 { return 42; } }\n\
         fn main() -> i64 {\n\
           var value: shared? Value = none;\n\
           var owner: shared Value = value!;\n\
           return owner->read();\n\
         }\n",
    );
    output.push_str(native_allocator());

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
fn byte_sized_optional_unwraps_write_canonical_scalar_homes() {
    let output = assembly(
        "fn main() -> i64 {\n\
           var flag: bool? = false;\n\
           var byte: u8? = 42u8;\n\
           var selected: bool = !flag!;\n\
           if (selected) { return (i64) byte!; }\n\
           return 0;\n\
         }\n",
    );

    let lines = output.lines().map(str::trim).collect::<Vec<_>>();
    assert!(
        lines.windows(2).any(|window| {
            window[0].starts_with("movzx rax, byte ptr [rbp")
                && window[1].starts_with("mov qword ptr [rbp")
        }),
        "byte-sized optional payloads must clear the complete MIR scalar home\n{output}"
    );
    assert_eq!(run_native_assembly(&output).code(), Some(42), "{output}");
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
        "call .Lska.class.main.Holder.c1.init.i0\n",
        "    mov rax, qword ptr [rbp - 16]"
    );
    assert_eq!(
        output.matches(begin_view).count(),
        1,
        "fixture must identify the checked container state load\n{output}"
    );
    let mut saturated = output.replacen(
        begin_view,
        concat!(
            "call .Lska.class.main.Holder.c1.init.i0\n",
            "    mov rax, -1\n",
            "    mov qword ptr [rbp - 16], rax\n",
            "    mov rax, qword ptr [rbp - 16]"
        ),
        1,
    );
    saturated.push_str(native_panic_reporter());

    let result = run_native_assembly_output(&saturated);
    assert_eq!(result.status.code(), Some(1), "{saturated}");
    assert!(result.stdout.is_empty());
    assert_eq!(result.stderr, b"panic: optional presence guard overflow\n");
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
        "call .Lska.class.main.Holder.c1.init.i0\n",
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
            "call .Lska.class.main.Holder.c1.init.i0\n",
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
    output.push_str(native_allocator());

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

    assert!(output.contains("call .Lska.fn."));
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
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Value {\n",
        "  marker: i64;\n",
        "  init(marker: i64) { self.marker = marker; }\n",
        "  destroy { test_record_i64(self.marker); }\n",
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
    output.push_str(record_i64_stub());
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
    assert!(dump.contains("ref optional o0"));
    assert!(dump.contains("mut ref optional o1"));
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

#[test]
fn nested_optional_outer_presence_copy_assignment_fields_and_arrays_execute() {
    let source = "class Holder {\n\
          value: i64??;\n\
          init() { self.value = some(none); }\n\
        }\n\
        fn main() -> i64 {\n\
          var absent: i64?? = none;\n\
          var present_absent: i64?? = some(none);\n\
          var present_present: i64?? = some(some(7));\n\
          var holder: Holder = Holder();\n\
          var copied: Holder = holder;\n\
          copied = copied;\n\
          var values: i64??[] = i64??[]{none, some(none), some(some(9))};\n\
          values[0] = values[2];\n\
          absent = present_present;\n\
          present_present = none;\n\
          if (absent is some) {\n\
            if (present_absent is some) {\n\
              if (present_present is none) {\n\
                if (copied.value is some) {\n\
                  if (values[0] is some) { return 42; }\n\
                }\n\
              }\n\
            }\n\
          }\n\
          return 0;\n\
        }\n";
    let mir = lower_text(source);
    let dump = crate::mir::dump_mir(&mir);
    assert!(dump.contains("nested-optional-initialize"));
    assert!(dump.contains("nested-optional-assign"));
    assert!(dump.contains("nested-optional-cleanup"));
    let mut output = assembly(source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn nested_optional_class_and_shared_payload_lifecycles_execute_recursively() {
    let class_source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Value { marker: i64; init(marker: i64) { self.marker = marker; } destroy { test_record_i64(self.marker); } }\n",
        "fn main() -> i64 {\n",
        "  var first: Value?? = some(some(Value(42)));\n",
        "  var second: Value?? = first;\n",
        "  first = none;\n",
        "  second = some(none);\n",
        "  if (second is some) { return 42; }\n",
        "  return 0;\n",
        "}\n",
    );
    let mut output = assembly(class_source);
    output.push_str(record_i64_stub());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(42), "{output}");
    assert_eq!(result.stdout, b"42\n42\n");

    let shared_source = concat!(
        "extern fn test_record_i64(value: i64) -> unit;\n",
        "class Value { marker: i64; init(marker: i64) { self.marker = marker; } destroy { test_record_i64(self.marker); } }\n",
        "fn main() -> i64 {\n",
        "  var first: (shared? Value)? = some(new Value(42));\n",
        "  var second: (shared? Value)? = first;\n",
        "  first = none;\n",
        "  second = none;\n",
        "  return 42;\n",
        "}\n",
    );
    let mut output = assembly(shared_source);
    output.push_str(native_allocator());
    output.push_str(record_i64_stub());
    let result = run_native_assembly_output(&output);
    assert_eq!(result.status.code(), Some(42), "{output}");
    assert_eq!(result.stdout, b"42\n");
}

#[test]
fn nested_optional_values_cross_calls_and_unwrap_one_layer_at_a_time() {
    let source = "fn forward(value: i64??) -> i64?? { return value; }\n\
        fn read(value: i64?) -> i64 {\n\
          if (value is some) { return value!; }\n\
          return 0;\n\
        }\n\
        fn main() -> i64 {\n\
          var outer: i64?? = some(some(42));\n\
          var inner: i64? = forward(outer)!;\n\
          return read(inner) + forward(outer)!!;\n\
        }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(84));
}

#[test]
fn nested_optional_aliases_mutate_and_read_exact_containers() {
    let source = "fn replace(mut ref value: i64??) -> unit {\n\
          value = some(some(42));\n\
        }\n\
        fn read(ref value: i64??) -> i64 { return value!!; }\n\
        fn main() -> i64 {\n\
          var value: i64?? = none;\n\
          replace(value);\n\
          return read(value);\n\
        }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

#[test]
fn nested_optionals_cross_dispatch_recursion_initializers_and_abi_pressure() {
    let source = "interface Reader { fn read(value: i64??) -> i64??; }\n\
        class Base {\n\
          init() {}\n\
          virtual fn read(value: i64??) -> i64?? { return value; }\n\
        }\n\
        class Derived extends Base implements Reader {\n\
          init() { super(); }\n\
          override fn read(value: i64??) -> i64?? { return value; }\n\
        }\n\
        class Holder {\n\
          value: i64??;\n\
          init(value: i64??) { self.value = value; }\n\
          fn direct(value: i64??) -> i64?? { return value; }\n\
          static fn pass(value: i64??) -> i64?? { return value; }\n\
        }\n\
        fn through_interface(ref source: Reader, value: i64??) -> i64?? {\n\
          return source.read(value);\n\
        }\n\
        fn through_virtual(ref source: Base, value: i64??) -> i64?? {\n\
          return source.read(value);\n\
        }\n\
        fn recurse(a: i64, b: i64, c: i64, d: i64, e: i64, f: i64,\n\
                   value: i64??, depth: i64) -> i64?? {\n\
          if (depth == 0) { return value; }\n\
          return recurse(a, b, c, d, e, f, value, depth - 1);\n\
        }\n\
        fn main() -> i64 {\n\
          var source: Derived = Derived();\n\
          var value: i64?? = some(some(7));\n\
          var holder: Holder = Holder(value);\n\
          return through_interface(source, value)!!\n\
            + through_virtual(source, value)!!\n\
            + holder.direct(value)!!\n\
            + Holder.pass(value)!!\n\
            + recurse(1, 2, 3, 4, 5, 6, value, 2)!!\n\
            + holder.value!!;\n\
        }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

#[test]
fn chained_class_and_shared_unwraps_preserve_owned_payloads_across_later_mutation() {
    let class_source = "class Value {\n\
          marker: i64;\n\
          init(marker: i64) { self.marker = marker; }\n\
        }\n\
        fn clear(mut ref value: Value??) -> i64 { value = none; return 0; }\n\
        fn consume(value: Value, later: i64) -> i64 { return value.marker + later; }\n\
        fn main() -> i64 {\n\
          var value: Value?? = some(some(Value(42)));\n\
          return consume(value!!, clear(value));\n\
        }\n";
    assert_eq!(
        run_native_assembly(&assembly(class_source)).code(),
        Some(42)
    );

    let shared_source = "class Value {\n\
          marker: i64;\n\
          init(marker: i64) { self.marker = marker; }\n\
        }\n\
        fn clear(mut ref value: (shared Value)??) -> i64 { value = none; return 0; }\n\
        fn consume(value: shared Value, later: i64) -> i64 {\n\
          return value->marker + later;\n\
        }\n\
        fn main() -> i64 {\n\
          var value: (shared Value)?? = some(some(new Value(42)));\n\
          return consume(value!!, clear(value));\n\
        }\n";
    let mut output = assembly(shared_source);
    output.push_str(native_allocator());
    assert_eq!(run_native_assembly(&output).code(), Some(42));
}

#[test]
fn chained_nested_unwrap_fails_at_each_absent_layer() {
    for initializer in ["none", "some(none)"] {
        let source =
            format!("fn main() -> i64 {{ var value: i64?? = {initializer}; return value!!; }}\n");
        let status = run_native_assembly(&assembly(&source));
        assert!(!status.success(), "{source}");
        assert_ne!(status.code(), Some(0), "{source}");
    }
}

#[test]
fn nested_optional_presence_and_unwrap_compose_with_short_circuit_logic() {
    let source = "fn read(value: i64??) -> i64 {\n\
          if ((value is some) && (value! is some)) { return value!!; }\n\
          return 0;\n\
        }\n\
        fn main() -> i64 {\n\
          return read(none) + read(some(none)) + read(some(some(42)));\n\
        }\n";

    assert_eq!(run_native_assembly(&assembly(source)).code(), Some(42));
}

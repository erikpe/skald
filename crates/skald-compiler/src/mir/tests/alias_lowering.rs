use super::*;

const COUNTER_CLASS: &str = concat!(
    "class Counter {\n",
    "    value: i64;\n",
    "    init(value: i64) { self.value = value; }\n",
    "    mut fn add(amount: i64) -> unit { self.value = self.value + amount; }\n",
    "    fn get() -> i64 { return self.value; }\n",
    "    mut fn add_to(mut ref other: Counter) -> unit {\n",
    "        increment(self, other.value);\n",
    "        increment(other, self.value);\n",
    "    }\n",
    "}\n",
);

#[test]
fn source_alias_mir_dump_is_exact() {
    let program = lower_text(concat!(
        "class Cell { value: i64; init(value: i64) { self.value = value; } }\n",
        "fn copy(ref source: Cell, mut ref destination: Cell) -> unit {\n",
        "    destination.value = source.value;\n",
        "}\n",
        "fn main() -> i64 { return 0; }\n",
    ));

    assert_eq!(
        dump_mir(&program),
        concat!(
            "MirProgram @0..202\n",
            "  Entry f1\n",
            "  Classes\n",
            "    Class c0 \"Cell\" @0..67\n",
            "      Field c0:field0 \"value\" : i64 @13..24\n",
            "      Initializer c0:init0(i64) @25..65\n",
            "  Declarations\n",
            "    Declaration f0 \"copy\" internal @68..170\n",
            "      Signature (ref class c0, mut ref class c0) -> unit\n",
            "    Declaration f1 \"main\" internal @171..201\n",
            "      Signature () -> i64\n",
            "  Definitions\n",
            "    Definition f0 @68..170\n",
            "      Parameters f0:s0 f0:s1\n",
            "      Storage\n",
            "        f0:s0 ref-parameter f0:p0 \"source\" : class c0 @76..92\n",
            "        f0:s1 mut-ref-parameter f0:p1 \"destination\" : class c0 @94..119\n",
            "      Values\n",
            "        f0:v0 : i64 @155..167\n",
            "      EntryBlock f0:b0\n",
            "      Blocks\n",
            "        f0:b0 @129..170\n",
            "          f0:v0 = load indirect(f0:s0).field(c0:field0) : i64 @155..167\n",
            "          store indirect(f0:s1).field(c0:field0), f0:v0 @135..168\n",
            "          return @129..170\n",
            "    Definition f1 @171..201\n",
            "      Parameters\n",
            "      Storage\n",
            "      Values\n",
            "        f1:v0 : i64 @197..198\n",
            "      EntryBlock f1:b0\n",
            "      Blocks\n",
            "        f1:b0 @188..201\n",
            "          f1:v0 = const.i64 0 : i64 @197..198\n",
            "          return f1:v0 @190..199\n",
            "  MemberDefinitions\n",
            "    MemberDefinition c0:init0 @25..65\n",
            "      Receiver c0:init0:s0\n",
            "      Parameters c0:init0:s1\n",
            "      Storage\n",
            "        c0:init0:s0 receiver c0:init0:self \"self\" : class c0 @42..65\n",
            "        c0:init0:s1 parameter c0:init0:p0 \"value\" : i64 @30..40\n",
            "      Values\n",
            "        c0:init0:v0 : i64 @57..62\n",
            "      EntryBlock c0:init0:b0\n",
            "      Blocks\n",
            "        c0:init0:b0 @42..65\n",
            "          c0:init0:v0 = load c0:init0:s1 : i64 @57..62\n",
            "          store c0:init0:s0.field(c0:field0), c0:init0:v0 @44..63\n",
            "          return @42..65\n",
        )
    );
}

#[test]
fn lowers_alias_parameters_and_supported_place_sources() {
    let program = lower_text(&format!(
        "{COUNTER_CLASS}{}",
        concat!(
            "class Snapshot {\n",
            "    value: i64;\n",
            "    init(ref source: Counter) { self.value = inspect(source); }\n",
            "}\n",
            "fn inspect(ref counter: Counter) -> i64 { return counter.get(); }\n",
            "fn increment(mut ref counter: Counter, amount: i64) -> unit { counter.add(amount); }\n",
            "fn forward(mut ref counter: Counter, amount: i64) -> unit { increment((counter), amount); }\n",
            "fn touch_twice(mut ref left: Counter, mut ref right: Counter) -> unit {\n",
            "    increment(left, 1);\n",
            "    increment(right, 1);\n",
            "}\n",
            "fn main() -> i64 {\n",
            "    var counter: Counter = Counter(40);\n",
            "    forward(counter, 1);\n",
            "    touch_twice(counter, counter);\n",
            "    counter.add_to(counter);\n",
            "    var snapshot: Snapshot = Snapshot(counter);\n",
            "    return snapshot.value;\n",
            "}\n",
        )
    ));

    verify_mir(&program).unwrap();
    let dump = dump_mir(&program);

    assert!(dump.contains("Signature (ref class c0) -> i64"));
    assert!(dump.contains("Signature (mut ref class c0, i64) -> unit"));
    assert!(dump.contains("ref-parameter"));
    assert!(dump.contains("mut-ref-parameter"));
    assert!(dump.contains("place(indirect("));
    assert!(dump.contains("place(f"));
    assert!(dump.contains("Initializer c1:init0(ref class c0)"));

    let main = program.definitions.get(program.entry_function).unwrap();
    let overlap_call = main.body.blocks[0]
        .instructions
        .iter()
        .find_map(|instruction| match instruction {
            MirInstruction::Call(call)
                if call.target == MirCallTarget::Direct(FunctionId::new(3)) =>
            {
                Some(call)
            }
            _ => None,
        })
        .expect("main must call touch_twice");
    let [MirArgument::Place(left), MirArgument::Place(right)] = overlap_call.arguments.as_slice()
    else {
        panic!("touch_twice must receive two places");
    };
    assert_eq!(left, right);
    assert_eq!(dump, dump_mir(&program));
}

#[test]
fn lowers_mixed_alias_and_scalar_arguments_in_source_evaluation_order() {
    let program = lower_text(&format!(
        "{COUNTER_CLASS}{}",
        concat!(
            "fn increment(mut ref counter: Counter, amount: i64) -> unit { counter.add(amount); }\n",
            "fn integer() -> i64 { return 7; }\n",
            "fn decimal() -> f64 { return 2.5; }\n",
            "fn mixed(ref source: Counter, scale: f64, amount: i64, mut ref destination: Counter) -> unit {\n",
            "    destination.add(amount);\n",
            "}\n",
            "fn conditional(mut ref counter: Counter, flag: bool) -> unit {\n",
            "    if (flag) {\n",
            "        mixed(counter, decimal(), integer(), counter);\n",
            "    } else {\n",
            "        mixed((counter), decimal(), integer(), counter);\n",
            "    }\n",
            "}\n",
            "fn main() -> i64 {\n",
            "    var counter: Counter = Counter(0);\n",
            "    conditional(counter, true);\n",
            "    return counter.value;\n",
            "}\n",
        )
    ));

    verify_mir(&program).unwrap();
    let conditional = program.definitions.get(FunctionId::new(4)).unwrap();
    let mut ordered_call_blocks = 0;
    for block in &conditional.body.blocks {
        let calls: Vec<_> = block
            .instructions
            .iter()
            .filter_map(|instruction| match instruction {
                MirInstruction::Call(call) => Some(call),
                _ => None,
            })
            .collect();
        if calls.len() == 3 {
            ordered_call_blocks += 1;
            assert_eq!(calls[0].target, MirCallTarget::Direct(FunctionId::new(2)));
            assert_eq!(calls[1].target, MirCallTarget::Direct(FunctionId::new(1)));
            assert_eq!(calls[2].target, MirCallTarget::Direct(FunctionId::new(3)));
            assert!(matches!(
                calls[2].arguments.as_slice(),
                [
                    MirArgument::Place(_),
                    MirArgument::Value(_),
                    MirArgument::Value(_),
                    MirArgument::Place(_),
                ]
            ));
        }
    }
    assert_eq!(ordered_call_blocks, 2);

    let dump = dump_mir(&program);
    assert!(dump.contains("Signature (ref class c0, f64, i64, mut ref class c0) -> unit"));
    assert_eq!(dump.matches("call f3(").count(), 2);
    assert!(dump.contains("branch"));
}

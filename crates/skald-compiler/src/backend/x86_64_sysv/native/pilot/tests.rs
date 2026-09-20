use crate::{
    backend::RuntimeTracePolicy,
    test_support::{
        lower_source_to_final_mir_with_sources, lower_source_to_minimal_final_mir_with_sources,
        run_native_assembly_output, run_native_assembly_with_c_probe,
        run_native_assembly_with_runtime_trace_probe, FinalMirWithSources,
    },
};
use std::fmt;
use std::process::Command;

#[derive(Clone, Copy, Debug)]
enum MirMode {
    Default,
    Minimal,
}

fn fixture(mode: MirMode, source: &str) -> FinalMirWithSources {
    match mode {
        MirMode::Default => lower_source_to_final_mir_with_sources("native-pilot.ska", source),
        MirMode::Minimal => {
            lower_source_to_minimal_final_mir_with_sources("native-pilot.ska", source)
        }
    }
}

fn compile(
    fixture: &FinalMirWithSources,
    trace: RuntimeTracePolicy,
    reachable: bool,
) -> Result<String, super::NativePilotError> {
    let mut input = fixture.backend_input(trace);
    if reachable {
        input = input.with_reachable_artifacts_only();
    }
    super::super::compile_native_pilot(input)
}

fn inspect(
    fixture: &FinalMirWithSources,
    options: super::NativePilotInspection,
) -> Result<(String, String), super::NativePilotError> {
    let mut inspection = String::new();
    let assembly = super::compile_native_pilot_inspected(
        fixture.backend_input(RuntimeTracePolicy::Omitted),
        options,
        &mut inspection,
    )?;
    Ok((assembly, inspection))
}

fn inspect_with_policy(
    fixture: &FinalMirWithSources,
    trace: RuntimeTracePolicy,
    reachable: bool,
    options: super::NativePilotInspection,
) -> Result<(String, String), super::NativePilotError> {
    let mut input = fixture.backend_input(trace);
    if reachable {
        input = input.with_reachable_artifacts_only();
    }
    let mut inspection = String::new();
    let assembly = super::compile_native_pilot_inspected(input, options, &mut inspection)?;
    Ok((assembly, inspection))
}

fn assert_exit(assembly: &str, trace: RuntimeTracePolicy, expected: i32) {
    let output = match trace {
        RuntimeTracePolicy::Enabled => run_native_assembly_with_runtime_trace_probe(assembly),
        RuntimeTracePolicy::Omitted => run_native_assembly_output(assembly),
    };
    assert_eq!(output.status.code(), Some(expected), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn assert_runtime_exit(assembly: &str, trace: RuntimeTracePolicy, expected: i32) {
    let output = match trace {
        RuntimeTracePolicy::Enabled => run_native_assembly_with_runtime_trace_probe(assembly),
        RuntimeTracePolicy::Omitted => run_native_assembly_with_c_probe(assembly, ""),
    };
    assert_eq!(output.status.code(), Some(expected), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

fn aggregate_lifecycle_checkpoint_source() -> &'static str {
    concat!(
        "interface Readable { fn read()->i64; ",
        "fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64; }",
        "class Root implements Readable { value:i64; init(value:i64){self.value=value;} ",
        "virtual fn read()->i64{return self.value;} ",
        "virtual fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{",
        "return a+b+c+d+e+f+g+h;} destroy {} }",
        "class Leaf extends Root { extra:i64; init(value:i64,extra:i64){",
        "super(value);self.extra=extra;} override fn read()->i64{return self.value+self.extra;} ",
        "override fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{",
        "return a+b+c+d+e+f+g+h+self.extra;} }",
        "class Holder { inline:Leaf; edge:shared Leaf; ",
        "init(inline:Leaf,edge:shared Leaf){self.inline=inline;self.edge=edge;} ",
        "mut fn replace(edge:shared Leaf)->unit{self.edge=edge;} }",
        "fn forward(value:Holder)->Holder{return value;}",
        "fn main()->i64{",
        "var owner:shared Leaf=new Leaf(2,3);var copy:shared Leaf=owner;",
        "var holder:Holder=Holder(Leaf(5,7),owner);",
        "var forwarded:Holder=forward(holder);forwarded=forwarded;",
        "forwarded.replace(forwarded.edge);",
        "var erased:shared Obj=copy;",
        "var readable:shared Readable=(shared Readable)erased;",
        "return forwarded.inline.read()+readable->pressure(1,2,3,4,5,6,7,8);}",
    )
}

#[test]
fn source_execution_matrix_covers_mir_trace_and_artifact_policies() {
    let source = concat!(
        "fn increment(value:i64)->i64{return value+1;}",
        "fn invoke(callback:fn(i64)->i64,value:i64)->i64{return callback(value);}",
        "fn main()->i64{",
        "var n:i64=0;var sum:i64=0;",
        "while(n<4){n=n+1;sum=sum+n;}",
        "var adjusted:f64=(f64)sum+2.0;",
        "return invoke(increment,(i64)adjusted)/1;}",
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
            for reachable in [false, true] {
                let (assembly, observed) = if reachable {
                    inspect_with_policy(
                        &fixture,
                        trace,
                        reachable,
                        super::NativePilotInspection {
                            selected: true,
                            physical: true,
                            ..Default::default()
                        },
                    )
                    .unwrap_or_else(|error| {
                        panic!("{mode:?}/{trace:?}/reachable={reachable}: {error}")
                    })
                } else {
                    (
                        compile(&fixture, trace, reachable).unwrap_or_else(|error| {
                            panic!("{mode:?}/{trace:?}/reachable={reachable}: {error}")
                        }),
                        String::new(),
                    )
                };
                assert_eq!(
                    assembly.contains("ska_rt_trace_top@tpoff"),
                    trace == RuntimeTracePolicy::Enabled
                );
                assert!(!assembly.contains("ska_rt_trace_top:\n"));
                if reachable {
                    assert!(observed.contains("stage=selected"));
                    assert!(observed.contains("stage=physical"));
                    assert_eq!(
                        observed.contains("TraceTls"),
                        trace == RuntimeTracePolicy::Enabled
                    );
                }
                assert_exit(&assembly, trace, 13);
            }
        }
    }
}

#[test]
fn aggregate_lifecycle_checkpoint_crosses_every_policy_and_schedule() {
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, aggregate_lifecycle_checkpoint_source());
        for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
            for reachable in [false, true] {
                let (assembly, inspection) = inspect_with_policy(
                    &fixture,
                    trace,
                    reachable,
                    super::NativePilotInspection {
                        lowered: true,
                        selected: true,
                        physical: true,
                        placement: true,
                        frame: true,
                    },
                )
                .unwrap_or_else(|error| {
                    panic!("{mode:?}/{trace:?}/reachable={reachable}: {error}")
                });
                for marker in [
                    "stage=lowered",
                    "stage=selected",
                    "stage=physical",
                    "assignment ",
                    "frame bytes=",
                    "ClassDispatch",
                    "ResultDestination",
                    "ReceiverMetadata",
                    "RuntimeParameter",
                    "InheritedOperation",
                    "ReportFailure",
                    "HardTrap",
                    "Allocate",
                    "Free",
                ] {
                    assert!(
                        inspection.contains(marker),
                        "missing {marker}: {inspection}"
                    );
                }
                assert_eq!(
                    inspection.contains("TraceTls"),
                    trace == RuntimeTracePolicy::Enabled
                );
                assert_eq!(
                    assembly.contains("ska_rt_trace_top@tpoff"),
                    trace == RuntimeTracePolicy::Enabled
                );
                assert_runtime_exit(&assembly, trace, 51);
            }
        }
    }
}

#[test]
fn primitive_and_shared_optionals_execute_through_the_verified_path() {
    let primitive = concat!(
        "fn choose(a:i64?,b:i64?,c:i64?,d:i64?,e:i64?,f:i64?,g:i64?)->i64?{",
        "if(g is some){return g;}return a;}",
        "fn main()->i64{",
        "var signed:i64?=1;var unsigned:u64?=2u;var byte:u8?=3u8;",
        "var floating:f64?=4.0;var truth:bool?=true;",
        "signed=signed;var selected:i64?=choose(none,none,none,none,none,none,6);",
        "if(unsigned is none){return 1;}if(byte is none){return 2;}",
        "if(floating is none){return 3;}if(truth is none){return 4;}",
        "return signed!+selected!+35;}",
    );
    let shared = concat!(
        "class Value { marker:i64; init(marker:i64){self.marker=marker;} }",
        "class Holder { value:shared? Value; init(value:shared? Value){self.value=value;} }",
        "fn forward(a:shared? Value,b:shared? Value,c:shared? Value,d:shared? Value,",
        "e:shared? Value,f:shared? Value,g:shared? Value)->shared? Value{",
        "if(g is some){return g;}return a;}",
        "fn main()->i64{",
        "var owner:shared? Value=new Value(35);var copy:shared? Value=owner;",
        "copy=copy;var holder:Holder=Holder(copy);var copied:Holder=holder;",
        "owner=none;copy=none;",
        "var result:shared? Value=forward(none,none,none,none,none,none,copied.value);",
        "return result!->marker+7;}",
    );
    for (family, source) in [("primitive", primitive), ("shared", shared)] {
        for mode in [MirMode::Default, MirMode::Minimal] {
            let fixture = fixture(mode, source);
            for reachable in [false, true] {
                let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, reachable)
                    .unwrap_or_else(|error| {
                        panic!("{family}/{mode:?}/reachable={reachable}: {error}")
                    });
                assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 42);
            }
        }
    }
}

#[test]
fn primitive_array_allocation_positions_and_length_execute_through_the_verified_path() {
    let source = concat!(
        "fn main()->i64{var values:i64[]=i64[](3u);",
        "values[0]=4;values[-1]=8;",
        "return (i64)values.len();}"
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true)
            .unwrap_or_else(|error| panic!("{mode:?}: {error}"));
        assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 3);
    }
}

#[test]
fn empty_inline_array_uses_its_null_representation_without_header_access() {
    let fixture = fixture(
        MirMode::Default,
        "fn main()->i64{var values:i64[]=i64[]();return (i64)values.len();}",
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 0);
}

#[test]
fn nontrivial_array_element_lifecycle_executes_through_generated_helpers() {
    let fixture = fixture(
        MirMode::Default,
        concat!(
            "class Item{value:i64;init(){self.value=0;}",
            "copy(ref other:Item){self.value=other.value;}",
            "assign(ref other:Item){self.value=other.value;}destroy{}}",
            "fn main()->i64{var source:Item[]=Item[](2u);source[0].value=7;",
            "var copied:Item[]=source;copied[1]=source[0];return copied[1].value;}"
        ),
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 7);
}

#[test]
fn indexed_arrays_and_array_aliases_execute_through_the_verified_path() {
    let fixture = fixture(
        MirMode::Default,
        concat!(
            "fn next(mut ref effects:i64,index:i64)->i64{",
            "effects=effects+1;return index+1;}",
            "fn mutate(mut ref values:i64[])->unit{values[0]=values[0]+1;}",
            "fn main()->i64{var effects:i64=0;",
            "var empty:i64[]=i64[](0u;index=>1/index);",
            "var values:i64[]=i64[](4u;index=>next(effects,index));",
            "mutate(values);return values[0]+values[3]+effects",
            "+(i64)empty.len();}"
        ),
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 10);
}

#[test]
fn copied_array_slices_execute_through_the_verified_path() {
    let fixture = fixture(
        MirMode::Default,
        concat!(
            "fn main()->i64{var values:i64[]=i64[]{1,2,3,4};",
            "var snapshot:i64[]=values[0:3];",
            "return snapshot[0]+snapshot[2];}"
        ),
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 4);
}

#[test]
fn primitive_slice_assignment_executes_through_the_verified_path() {
    let fixture = fixture(
        MirMode::Default,
        concat!(
            "fn main()->i64{var destination:i64[]=i64[]{0,0,0};",
            "var source:i64[]=i64[]{4,8,16};destination[:]=source;",
            "return destination[0]+destination[2];}"
        ),
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 20);
}

#[test]
fn object_initialization_dispatch_and_checked_views_execute_through_the_verified_path() {
    let source = concat!(
        "interface Readable { fn read() -> i64; fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64; }",
        "class Root implements Readable {",
        "value:i64; init(value:i64){self.value=value;}",
        "virtual fn read()->i64{return self.value;}",
        "virtual fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{",
        "return a+b+c+d+e+f+g+h;}}",
        "class Leaf extends Root {",
        "extra:i64; init(value:i64,extra:i64){super(value);self.extra=extra;}",
        "override fn read()->i64{return self.value+self.extra;}",
        "override fn pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64,h:i64)->i64{",
        "return a+b+c+d+e+f+g+h+self.extra;}}",
        "fn observe(ref exact:Leaf,ref ancestor:Root,ref readable:Readable,ref object:Obj)->i64{",
        "if(object is Leaf){return exact.read()+ancestor.read()+readable.read()+((Leaf)object).read()",
        "+readable.pressure(1,2,3,4,5,6,7,8);}",
        "return 99;}",
        "fn main()->i64{return observe(Leaf(1,2),Leaf(3,4),Leaf(5,6),Leaf(7,8));}",
    );
    let fixture = fixture(MirMode::Default, source);
    let (assembly, lowered) = inspect_with_policy(
        &fixture,
        RuntimeTracePolicy::Omitted,
        true,
        super::NativePilotInspection {
            lowered: true,
            ..Default::default()
        },
    )
    .unwrap();

    assert!(lowered.contains("ClassDispatch"), "{lowered}");
    assert!(lowered.contains("target: Indirect("), "{lowered}");
    assert!(lowered.contains("Compare { predicate: Equal"), "{lowered}");
    assert_exit(&assembly, RuntimeTracePolicy::Omitted, 78);
}

#[test]
fn failed_checked_cast_reports_at_its_source_operation() {
    let fixture = fixture(
        MirMode::Default,
        concat!(
            "class Root { init() {} }",
            "class Leaf extends Root { init(){super();} fn value()->i64{return 1;} }",
            "fn force(ref object:Obj)->i64{return ((Leaf)object).value();}",
            "fn main()->i64{return force(Root());}",
        ),
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Enabled, true).unwrap();
    let output = run_native_assembly_with_runtime_trace_probe(&assembly);
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("checked object cast failed"), "{stderr}");
    assert!(stderr.contains("native-pilot.ska"), "{stderr}");
}

#[test]
fn scalar_c_calls_return_through_c_and_cross_register_pressure_boundaries() {
    let source = concat!(
        "extern fn c_integer_pressure(a:i64,b:i64,c:i64,d:i64,e:i64,f:i64,g:i64)->i64;",
        "extern fn c_float_pressure(a:f64,b:f64,c:f64,d:f64,e:f64,f:f64,g:f64,h:f64,i:f64)->f64;",
        "fn increment(value:i64)->i64{return value+1;}",
        "fn invoke(callback:fn(i64)->i64,value:i64)->i64{return callback(value);}",
        "fn preserve_live_tie(a:i64,count:u64)->i64{",
        "var shifted:i64=a<<count;",
        "var called:i64=c_integer_pressure(1,2,3,4,5,6,7);",
        "return shifted+a+called;}",
        "fn main()->i64{return ",
        "invoke(increment,1)+",
        "c_integer_pressure(1,2,3,4,5,6,7)+",
        "(i64)c_float_pressure(1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0)+",
        "preserve_live_tie(3,2u);}",
    );
    let fixture = fixture(MirMode::Default, source);
    let (assembly, inspection) = inspect_with_policy(
        &fixture,
        RuntimeTracePolicy::Omitted,
        false,
        super::NativePilotInspection {
            physical: true,
            placement: true,
            frame: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(inspection.contains("\nassignment "));
    assert!(inspection.contains("\nframe bytes="));
    assert!(inspection
        .lines()
        .filter_map(|line| line.split_once(" outgoing="))
        .filter_map(|(_, suffix)| suffix.split_once(' '))
        .any(|(bytes, _)| bytes.parse::<usize>().is_ok_and(|bytes| bytes > 0)));
    let output = run_native_assembly_with_c_probe(
        &assembly,
        concat!(
            "#include <stdint.h>\n",
            "#include <stdlib.h>\n",
            "extern int64_t skald_entry(void) __asm__(\"main\");\n",
            "int64_t c_integer_pressure(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f,int64_t g) { return a+b+c+d+e+f+g; }\n",
            "double c_float_pressure(double a,double b,double c,double d,double e,double f,double g,double h,double i) { return a+b+c+d+e+f+g+h+i; }\n",
            "__attribute__((constructor)) static void call_skald_from_c(void) { if (skald_entry() != 118) _Exit(99); }\n",
        ),
    );
    assert_eq!(output.status.code(), Some(118), "{output:?}");
}

#[test]
fn adversarial_numeric_boundaries_execute_through_the_verified_path() {
    let cases = [
        (
            "NaN predicates",
            concat!(
                "fn main()->i64{var zero:f64=0.0;var nan:f64=zero/zero;",
                "if(nan==nan){return 1;}if(!(nan!=nan)){return 2;}",
                "if(nan<1.0||nan<=1.0||nan>1.0||nan>=1.0){return 3;}return 0;}"
            ),
        ),
        (
            "f64 to upper u64",
            concat!(
                "fn main()->i64{var high:u64=(u64)9223372036854775808.0;",
                "if(high!=9223372036854775808u){return 1;}return 0;}"
            ),
        ),
        (
            "maximum u64 to f64",
            concat!(
                "fn main()->i64{var rounded:f64=(f64)18446744073709551615u;",
                "if(rounded!=18446744073709551616.0){return 1;}return 0;}"
            ),
        ),
        (
            "signed minimum overflow",
            "fn main()->i64{var minimum:i64=-9223372036854775808;if(minimum/-1!=minimum){return 1;}return 0;}",
        ),
        (
            "signed floor division",
            "fn main()->i64{if(-7/3!=-3||-7%3!=2){return 1;}return 0;}",
        ),
        (
            "live destructive tie",
            "fn main()->i64{var input:i64=3;var shifted:i64=input<<2u;if(shifted+input!=15){return 1;}return 0;}",
        ),
    ];
    for (name, source) in cases {
        let fixture = fixture(MirMode::Minimal, source);
        let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
        let output = run_native_assembly_output(&assembly);
        assert_eq!(output.status.code(), Some(0), "{name}: {output:?}");
        assert!(output.stderr.is_empty(), "{name}: {output:?}");
    }
}

#[test]
fn full_width_shift_count_reports_before_native_cl_narrowing() {
    for (source, trace) in [
        (
            "fn main()->i64{var count:u64=64u;return 1<<count;}",
            RuntimeTracePolicy::Enabled,
        ),
        (
            "fn main()->i64{var count:u64=256u;return (i64)(1u8<<count);}",
            RuntimeTracePolicy::Omitted,
        ),
    ] {
        let fixture = fixture(MirMode::Minimal, source);
        let assembly = compile(&fixture, trace, true).unwrap();
        let output = run_native_assembly_with_c_probe(&assembly, "");
        assert!(!output.status.success());
        assert!(String::from_utf8(output.stderr)
            .unwrap()
            .contains("panic: shift count out of range"));
    }
}

#[test]
fn requested_checkpoints_are_independent_deterministic_and_quiet_by_default() {
    let source = "fn twice(value:i64)->i64{return value+value;} fn main()->i64{return twice(7);}";
    let fixture = fixture(MirMode::Default, source);
    let quiet = compile(&fixture, RuntimeTracePolicy::Omitted, false).unwrap();

    let cases = [
        (
            super::NativePilotInspection {
                lowered: true,
                ..Default::default()
            },
            "stage=lowered",
            [
                "stage=selected",
                "stage=physical",
                "assignment ",
                "frame bytes=",
            ],
        ),
        (
            super::NativePilotInspection {
                selected: true,
                ..Default::default()
            },
            "stage=selected",
            [
                "stage=lowered",
                "stage=physical",
                "assignment ",
                "frame bytes=",
            ],
        ),
        (
            super::NativePilotInspection {
                physical: true,
                ..Default::default()
            },
            "stage=physical",
            [
                "stage=lowered",
                "stage=selected",
                "assignment ",
                "frame bytes=",
            ],
        ),
        (
            super::NativePilotInspection {
                placement: true,
                ..Default::default()
            },
            "assignment ",
            [
                "stage=lowered",
                "stage=selected",
                "\nentry b",
                "frame bytes=",
            ],
        ),
        (
            super::NativePilotInspection {
                frame: true,
                ..Default::default()
            },
            "frame bytes=",
            [
                "stage=lowered",
                "stage=selected",
                "\nentry b",
                "assignment ",
            ],
        ),
    ];
    for (options, expected, absent) in cases {
        let (assembly, observed) = inspect(&fixture, options).unwrap();
        assert_eq!(assembly, quiet);
        assert!(observed.contains(expected), "{observed}");
        for marker in absent {
            assert!(
                !observed.contains(marker),
                "unexpected {marker}: {observed}"
            );
        }
        let (_, repeated) = inspect(&fixture, options).unwrap();
        assert_eq!(observed, repeated);
    }
}

#[test]
fn post_admission_observation_failure_is_terminal() {
    struct FailingWriter;
    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }

    let fixture = fixture(MirMode::Default, "fn main()->i64{return 23;}");
    let admitted =
        crate::backend::planning::admit(fixture.backend_input(RuntimeTracePolicy::Omitted))
            .expect("fixture must cross the whole-program admission boundary");
    let error = super::pipeline::compile_admitted(
        &admitted,
        super::NativePilotInspection {
            lowered: true,
            ..Default::default()
        },
        Some(&mut FailingWriter),
    )
    .expect_err("post-admission failure must remain terminal");

    assert!(matches!(error, super::NativePilotError::Observation(_)));
}

#[test]
fn private_pilot_artifact_and_checkpoints_are_deterministic_across_processes() {
    const CHILD: &str = "SKALD_NATIVE_PILOT_DETERMINISM_CHILD";
    const BEGIN: &str = "NATIVE-PILOT-BEGIN\n";
    const END: &str = "NATIVE-PILOT-END\n";
    if std::env::var_os(CHILD).is_some() {
        let fixture = fixture(
            MirMode::Default,
            "fn calculate(a:i64,b:u64)->i64{return (a<<b)+a;} fn main()->i64{return calculate(3,2u);}",
        );
        let (assembly, inspection) = inspect(
            &fixture,
            super::NativePilotInspection {
                lowered: true,
                selected: true,
                physical: true,
                placement: true,
                frame: true,
            },
        )
        .unwrap();
        print!("{BEGIN}{inspection}{assembly}{END}");
        return;
    }

    let run = || {
        let output = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "backend::x86_64_sysv::native::pilot::tests::private_pilot_artifact_and_checkpoints_are_deterministic_across_processes",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8(output.stdout).unwrap();
        stdout
            .split_once(BEGIN)
            .and_then(|(_, rest)| rest.split_once(END))
            .map(|(payload, _)| payload.to_owned())
            .expect("child emitted native pilot evidence")
    };
    assert_eq!(run(), run());
}

#[test]
fn primitive_integer_float_byte_boolean_and_cast_cells_execute() {
    let source = concat!(
        "fn primitive(byte:u8,bits:u64,flag:bool,float:f64)->i64{",
        "var shifted:u64=bits>>1u;var remainder:u8=byte%3u8;",
        "if(!flag){return 0;}return (i64)shifted+(i64)remainder+(i64)float;}",
        "fn main()->i64{return primitive(8u8,10u,true,2.5);}",
    );
    let fixture = fixture(MirMode::Default, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_exit(&assembly, RuntimeTracePolicy::Omitted, 9);
}

#[test]
fn checked_division_failure_preserves_runtime_reporting_and_trace_policy() {
    let source = "fn divide(value:i64,divisor:i64)->i64{return value/divisor;} fn main()->i64{return divide(7,0);}";
    for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
        let fixture = fixture(MirMode::Minimal, source);
        let assembly = compile(&fixture, trace, true).unwrap();
        let output = run_native_assembly_with_c_probe(&assembly, "");
        assert!(!output.status.success());
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(
            stderr.contains("panic: integer division by zero"),
            "{stderr}"
        );
        assert_eq!(
            stderr.contains("stacktrace:"),
            trace == RuntimeTracePolicy::Enabled
        );
    }
}

#[test]
fn absent_optional_unwrap_preserves_runtime_reporting_and_trace_policy() {
    for source in [
        "fn main()->i64{var value:i64?=none;return value!;}",
        concat!(
            "class Value { init(){} fn read()->i64{return 1;} }",
            "fn main()->i64{var value:shared? Value=none;return value!->read();}",
        ),
    ] {
        for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
            let fixture = fixture(MirMode::Minimal, source);
            let assembly = compile(&fixture, trace, true).unwrap();
            let output = run_native_assembly_with_c_probe(&assembly, "");
            assert!(!output.status.success());
            let stderr = String::from_utf8(output.stderr).unwrap();
            assert!(
                stderr.contains("panic: optional value is absent"),
                "{stderr}"
            );
            assert_eq!(
                stderr.contains("stacktrace:"),
                trace == RuntimeTracePolicy::Enabled
            );
            if trace == RuntimeTracePolicy::Enabled {
                assert!(stderr.contains("main::main"), "{stderr}");
                assert!(stderr.contains("native-pilot.ska"), "{stderr}");
            }
        }
    }
}

#[test]
fn class_copy_cleanup_and_nested_finalizers_execute_through_the_verified_path() {
    let source = concat!(
        "class Leaf { value:i64; init(value:i64){self.value=value;} ",
        "copy(ref other:Leaf){self.value=other.value+10;} ",
        "assign(ref other:Leaf){self.value=other.value+20;} destroy {} }",
        "class Base { base:i64; init(base:i64){self.base=base;} ",
        "copy(ref other:Base){self.base=other.base+30;} ",
        "assign(ref other:Base){self.base=other.base+40;} destroy {} }",
        "class Pair extends Base { left:Leaf; right:Leaf; ",
        "init(base:i64,left:i64,right:i64){super(base);self.left=Leaf(left);self.right=Leaf(right);} }",
        "fn main()->i64{",
        "var source:Pair=Pair(1,2,3);var destination:Pair=source;",
        "destination.left=destination.left;destination.right=source.left;source=source;",
        "return destination.base+destination.left.value+destination.right.value+source.base;}",
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
            let assembly = compile(&fixture, trace, true)
                .unwrap_or_else(|error| panic!("{mode:?}/{trace:?}: {error}"));
            // Copy construction produces (31, 12, 13); assignment changes
            // the destination leaves to (32, 22), while Pair self assignment
            // changes the source to (41, 22, 23).
            assert_exit(&assembly, trace, 126);
        }
    }
}

#[test]
fn shared_owners_execute_allocation_transfer_fields_casts_and_finalization() {
    let source = concat!(
        "interface Readable { fn read()->i64; }",
        "class Leaf implements Readable { value:i64; init(value:i64){self.value=value;} ",
        "fn read()->i64{return self.value;} destroy {} }",
        "class Holder { edge:shared Leaf; init(edge:shared Leaf){self.edge=edge;} ",
        "mut fn replace(edge:shared Leaf)->unit{self.edge=edge;} }",
        "fn forward(value:shared Leaf)->shared Leaf{return value;}",
        "fn main()->i64{",
        "var first:shared Leaf=new Leaf(7);",
        "var copied:shared Leaf=first;",
        "var moved:shared Leaf=forward(copied);",
        "var holder:Holder=Holder(first);",
        "holder.replace(holder.edge);",
        "var erased:shared Obj=moved;",
        "var readable:shared Readable=(shared Readable)erased;",
        "return readable->read()+holder.edge->read();}",
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        for trace in [RuntimeTracePolicy::Enabled, RuntimeTracePolicy::Omitted] {
            let assembly = compile(&fixture, trace, true)
                .unwrap_or_else(|error| panic!("{mode:?}/{trace:?}: {error}"));
            assert_runtime_exit(&assembly, trace, 14);
        }
    }
}

#[test]
fn last_shared_owner_finalizes_once_before_releasing_the_original_allocation() {
    let source = concat!(
        "extern fn record(value:i64)->unit;extern fn observed()->i64;",
        "class Leaf { value:i64; init(value:i64){self.value=value;} ",
        "destroy {record(self.value);} }",
        "fn main()->i64{{var first:shared Leaf=new Leaf(7);",
        "var copy:shared Leaf=first;}return observed();}",
    );
    let fixture = fixture(MirMode::Default, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    let output = run_native_assembly_with_c_probe(
        &assembly,
        concat!(
            "#include <stdint.h>\n",
            "static int64_t value;\n",
            "void record(int64_t next) { value = value * 10 + next; }\n",
            "int64_t observed(void) { return value; }\n",
        ),
    );
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn shared_finalizer_failure_preserves_source_attribution_and_stops_before_free() {
    let source = concat!(
        "class Bomb { value:i64; init(value:i64){self.value=value;} ",
        "destroy {var zero:i64=self.value-self.value;self.value=self.value/zero;} }",
        "fn main()->i64{var bomb:shared Bomb=new Bomb(7);return 0;}",
    );
    let fixture = fixture(MirMode::Default, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Enabled, true).unwrap();
    let output = run_native_assembly_with_runtime_trace_probe(&assembly);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("panic: integer division by zero"),
        "{stderr}"
    );
    assert!(stderr.contains("Bomb.destroy"), "{stderr}");
    assert!(stderr.contains("main::main"), "{stderr}");
    assert!(!stderr.contains("Release"), "{stderr}");
}

#[test]
fn destructor_failure_keeps_the_user_body_and_cleanup_site_in_the_trace() {
    let source = concat!(
        "class Bomb { value:i64; init(value:i64){self.value=value;} ",
        "destroy {var zero:i64=self.value-self.value;self.value=self.value/zero;} }",
        "fn main()->i64{{var bomb:Bomb=Bomb(7);}return 0;}",
    );
    let fixture = fixture(MirMode::Default, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Enabled, true).unwrap();
    let output = run_native_assembly_with_runtime_trace_probe(&assembly);
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("panic: integer division by zero"),
        "{stderr}"
    );
    assert!(stderr.contains("Bomb.destroy"), "{stderr}");
    assert!(stderr.contains("main::main"), "{stderr}");
    assert!(!stderr.contains("ClassFinalizer"), "{stderr}");
}

#[test]
fn aggregate_and_class_optional_lifecycle_executes_through_the_verified_path() {
    let source = concat!(
        "class Pending { init(){} }",
        "class Item { value:i64; pending:Pending?; ",
        "init(value:i64){self.value=value;self.pending=none;} ",
        "fn read()->i64{return self.value;} }",
        "fn main()->i64{var item:Item=Item(7);var pending:Pending?=Pending();",
        "pending=none;item.pending=Pending();item.pending=none;return item.read();}",
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true)
            .unwrap_or_else(|error| panic!("{mode:?}: {error}"));
        assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 7);
    }
}

#[test]
fn nested_optional_copy_executes_through_the_verified_path() {
    let fixture = fixture(
        MirMode::Minimal,
        "fn main()->i64{var deep:i64??=some(some(21));var copy:i64??=deep;copy=deep;return copy!!+21;}",
    );
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, true).unwrap();
    assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 42);
}

#[test]
fn optional_boxes_publish_access_and_finalize_through_the_verified_path() {
    let source = concat!(
        "interface Marker { fn mark()->i64; }",
        "class Value implements Marker { marker:i64; init(marker:i64){self.marker=marker;} fn mark()->i64{return self.marker;} destroy {} }",
        "fn read(box:shared Marker?)->i64{return (*box)!.mark();}",
        "fn main()->i64{var present:shared Value?=new Value?(Value(42));",
        "var marker:shared Marker?=present;return read(marker);}"
    );
    for mode in [MirMode::Default, MirMode::Minimal] {
        let fixture = fixture(mode, source);
        for reachable in [false, true] {
            let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, reachable)
                .unwrap_or_else(|error| panic!("{mode:?}/reachable={reachable}: {error}"));
            assert_runtime_exit(&assembly, RuntimeTracePolicy::Omitted, 42);
        }
    }
}

#[test]
fn guarded_optional_mutation_reports_the_language_failure() {
    let source = concat!(
        "class Item { value:i64; init(value:i64){self.value=value;} }",
        "class Holder { item:Item?; init(){self.item=Item(42);} ",
        "mut fn clear()->i64{self.item=none;return 0;} }",
        "fn consume(ref item:Item,ignored:i64)->i64{return item.value+ignored;}",
        "fn main()->i64{var holder:Holder=Holder();return consume(holder.item!,holder.clear());}"
    );
    let fixture = fixture(MirMode::Minimal, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Enabled, true).unwrap();
    let output = run_native_assembly_with_runtime_trace_probe(&assembly);
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("cannot mutate a guarded optional value"),
        "{stderr}"
    );
    assert!(stderr.contains("native-pilot.ska"), "{stderr}");
}

#[test]
fn literal_strings_and_standard_io_execute_through_the_verified_path() {
    let program = crate::passes::verify_final_mir(
        crate::mir::test_fixtures::io_program_with_app_and_additional_bodies(
            concat!(
                "import std::io; import std::str;",
                "fn main()->i64{var first:std::str::Str=\"same\";",
                "var second:std::str::Str=\"same\";var empty:std::str::Str=\"\";",
                "var bytes:u8[]=u8[]{65u8,0u8,255u8};",
                "var output:i64=std::io::standard(1u8);",
                "return std::io::write(output,bytes,1u);}"
            ),
            "",
        ),
    )
    .unwrap();
    let assembly = super::super::compile_native_pilot(
        crate::backend::BackendInput::without_runtime_trace(&program),
    )
    .unwrap();
    let output = run_native_assembly_with_c_probe(&assembly, "");
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_eq!(output.stdout, b"\0\xff");
    assert!(output.stderr.is_empty(), "{output:?}");
    assert!(assembly.contains(".section .data.rel.ro.local,\"aw\",@progbits"));
    assert!(!assembly.contains(".zero 0\n"));
}

#[test]
fn standard_io_open_read_and_close_errors_execute_through_the_verified_path() {
    let program = crate::passes::verify_final_mir(
        crate::mir::test_fixtures::io_program_with_app_and_additional_bodies(
            concat!(
                "import std::io; fn main()->i64{",
                "var path:u8[]=u8[]{47u8,100u8,101u8,118u8,47u8,110u8,117u8,108u8,108u8};",
                "var bytes:u8[]=u8[](1u);var handle:i64=std::io::open(path,0u8);",
                "if(handle<0){return 1;}var closed:i64=std::io::close(handle);",
                "if(closed<0){return 2;}var result:i64=std::io::read(handle,bytes,0u);",
                "if(result<0){return 0;}return 3;}"
            ),
            "",
        ),
    )
    .unwrap();
    let assembly = super::super::compile_native_pilot(
        crate::backend::BackendInput::without_runtime_trace(&program),
    )
    .unwrap();
    let output = run_native_assembly_with_c_probe(&assembly, "");
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn source_panic_string_bytes_execute_with_both_trace_policies() {
    let fixture = crate::mir::test_fixtures::verified_io_fixture_with_sources(
        "import std::error; fn main()->i64{std::error::panic(\"panic bytes\");}",
        "",
    );
    for trace in [RuntimeTracePolicy::Omitted, RuntimeTracePolicy::Enabled] {
        let assembly = compile(&fixture, trace, false).unwrap();
        let output = match trace {
            RuntimeTracePolicy::Omitted => run_native_assembly_with_c_probe(&assembly, ""),
            RuntimeTracePolicy::Enabled => run_native_assembly_with_runtime_trace_probe(&assembly),
        };
        assert!(!output.status.success(), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains("panic bytes"), "{stderr}");
        assert_eq!(
            assembly.contains("ska_rt_trace_top@tpoff"),
            trace == RuntimeTracePolicy::Enabled
        );
    }
}

#[test]
fn static_lifecycle_executes_through_both_artifact_and_trace_policies() {
    let source = concat!(
        "class Item {value:i64;init(value:i64){self.value=value;}destroy{}}",
        "class State {static item:Item=Item(5);static maybe:Item?=Item(6);",
        "static owner:shared Item=new Item(7);",
        "static maybe_owner:shared? Item=new Item(8);",
        "static values:i64[]=i64[]{9};init(){}}",
        "fn main()->i64{return State.item.value+State.maybe!.value+",
        "State.owner->value+State.maybe_owner!->value+State.values[0]+7;}"
    );
    for trace in [RuntimeTracePolicy::Omitted, RuntimeTracePolicy::Enabled] {
        for reachable in [false, true] {
            let fixture = fixture(MirMode::Default, source);
            let assembly = compile(&fixture, trace, reachable)
                .unwrap_or_else(|error| panic!("{trace:?}/reachable={reachable}: {error}"));
            assert!(assembly.contains(".section .bss\n"));
            assert_runtime_exit(&assembly, trace, 42);
        }
    }
}

#[test]
fn static_lifecycle_failures_keep_source_frames_and_exclude_coordinators() {
    let cases = [
        (
            concat!(
                "fn fail()->i64{var zero:i64=0;return 1/zero;}",
                "class State {static value:i64=fail();init(){}}",
                "fn main()->i64{return State.value;}"
            ),
            "integer division by zero",
            "main::fail",
            "main::main",
        ),
        (
            concat!(
                "class Bomb {value:i64;init(){self.value=1;} destroy {var zero:i64=0;self.value=self.value/zero;}}",
                "class State {static bomb:Bomb=Bomb();init(){}}",
                "fn main()->i64{var value:i64=State.bomb.value;var absent:i64?=none;return value+absent!;}"
            ),
            "optional value is absent",
            "main::main",
            "Bomb.destroy",
        ),
        (
            concat!(
                "class Bomb {value:i64;init(){self.value=1;} destroy {var zero:i64=0;self.value=self.value/zero;}}",
                "class State {static bomb:Bomb=Bomb();init(){}}",
                "fn main()->i64{return State.bomb.value+41;}"
            ),
            "integer division by zero",
            "Bomb.destroy",
            "Coordinator",
        ),
    ];

    for (source, message, included_frame, excluded_frame) in cases {
        let fixture = fixture(MirMode::Default, source);
        let assembly = compile(&fixture, RuntimeTracePolicy::Enabled, true).unwrap();
        let output = run_native_assembly_with_runtime_trace_probe(&assembly);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(stderr.contains(message), "{stderr}");
        assert!(stderr.contains(included_frame), "{stderr}");
        assert!(!stderr.contains(excluded_frame), "{stderr}");
        assert!(!stderr.contains("Coordinator"), "{stderr}");
    }
}

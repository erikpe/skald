use crate::{
    backend::{RuntimeTracePolicy, Target},
    test_support::{
        lower_source_to_final_mir_with_sources, lower_source_to_minimal_final_mir_with_sources,
        run_native_assembly, run_native_assembly_output, run_native_assembly_with_c_probe,
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

    struct FailingWriter;
    impl fmt::Write for FailingWriter {
        fn write_str(&mut self, _: &str) -> fmt::Result {
            Err(fmt::Error)
        }
    }
    assert!(matches!(
        super::compile_native_pilot_inspected(
            fixture.backend_input(RuntimeTracePolicy::Omitted),
            super::NativePilotInspection {
                lowered: true,
                ..Default::default()
            },
            &mut FailingWriter,
        ),
        Err(super::NativePilotError::Observation(_))
    ));
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
fn unsupported_source_rejects_without_changing_the_public_backend() {
    let source = concat!(
        "class Item { value:i64; init(value:i64){self.value=value;} ",
        "fn read()->i64{return self.value;} }",
        "fn main()->i64{var item:Item=Item(7);return item.read();}",
    );
    let fixture = fixture(MirMode::Default, source);
    assert!(matches!(
        compile(&fixture, RuntimeTracePolicy::Omitted, true),
        Err(super::NativePilotError::Admission(_))
    ));
    let legacy = fixture
        .emit_assembly(Target::X86_64SysV, RuntimeTracePolicy::Omitted)
        .unwrap();
    assert_eq!(run_native_assembly(&legacy).code(), Some(7));
}

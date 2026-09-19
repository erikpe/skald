use crate::{
    backend::{RuntimeTracePolicy, Target},
    test_support::{
        lower_source_to_final_mir_with_sources, lower_source_to_minimal_final_mir_with_sources,
        run_native_assembly, run_native_assembly_output, run_native_assembly_with_c_probe,
        run_native_assembly_with_runtime_trace_probe, FinalMirWithSources,
    },
};

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
                let assembly = compile(&fixture, trace, reachable).unwrap_or_else(|error| {
                    panic!("{mode:?}/{trace:?}/reachable={reachable}: {error}")
                });
                assert_eq!(
                    assembly.contains("ska_rt_trace_top@tpoff"),
                    trace == RuntimeTracePolicy::Enabled
                );
                assert!(!assembly.contains("ska_rt_trace_top:\n"));
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
        "fn main()->i64{return ",
        "invoke(increment,1)+",
        "c_integer_pressure(1,2,3,4,5,6,7)+",
        "(i64)c_float_pressure(1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0);}",
    );
    let fixture = fixture(MirMode::Default, source);
    let assembly = compile(&fixture, RuntimeTracePolicy::Omitted, false).unwrap();
    let output = run_native_assembly_with_c_probe(
        &assembly,
        concat!(
            "#include <stdint.h>\n",
            "#include <stdlib.h>\n",
            "extern int64_t skald_entry(void) __asm__(\"main\");\n",
            "int64_t c_integer_pressure(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f,int64_t g) { return a+b+c+d+e+f+g; }\n",
            "double c_float_pressure(double a,double b,double c,double d,double e,double f,double g,double h,double i) { return a+b+c+d+e+f+g+h+i; }\n",
            "__attribute__((constructor)) static void call_skald_from_c(void) { if (skald_entry() != 75) _Exit(99); }\n",
        ),
    );
    assert_eq!(output.status.code(), Some(75), "{output:?}");
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

use super::fixture::Fixture;

pub(crate) fn write_native_spec(fixture: &Fixture, mode: &str, run_body: &str) {
    fixture.write("program.ska", "fn main() -> i64 { return 0; }\n");
    fixture.write(
        "native.golden.toml",
        format!(
            r#"schema=1
[[test]]
name="native"
mode="run"
source="program.ska"
compiler_args=["--fake-mode", "{mode}", "--base", "middle"]
[[test.run]]
name="run"
{run_body}
"#
        ),
    );
}

pub(crate) fn write_compile_fail_spec(fixture: &Fixture, mode: &str, expected: &str) {
    fixture.write("failure.ska", "fn main() -> i64 { return missing(); }\n");
    fixture.write(
        "failure.golden.toml",
        format!(
            r#"schema=1
[[test]]
name="failure"
mode="compile-fail"
source="failure.ska"
compiler_args=["--fake-mode", "{mode}"]
expect={{stderr={{match="contains", inline="{expected}"}}}}
"#
        ),
    );
}

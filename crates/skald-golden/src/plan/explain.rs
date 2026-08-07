use super::model::{
    PlannedLeafKind, ResolvedArgs, ResolvedByteSource, ResolvedStreamExpectation,
    ResolvedWorkingDirectory, TestPlan,
};
use std::{ffi::OsStr, fmt::Write as _};

impl TestPlan {
    /// Renders one fully resolved leaf without preparing runtime or artifacts.
    pub fn explain(&self, id: &str) -> Option<String> {
        let leaf = self.leaf(id)?;
        let build = self
            .build(leaf.build_id())
            .expect("planned leaf should reference a planned build");
        let test = self
            .test(leaf.test_id())
            .expect("planned leaf should reference a planned test");
        let mut output = String::new();
        writeln!(output, "id = {}", leaf.id()).unwrap();
        writeln!(output, "spec = {}", leaf.spec_relative_path()).unwrap();
        writeln!(output, "test = {}", test.id()).unwrap();
        writeln!(output, "build = {}", build.id()).unwrap();
        writeln!(output, "variant = {}", build.variant()).unwrap();
        match test.source() {
            Some(source) => writeln!(output, "source = {}", source.display()).unwrap(),
            None => writeln!(output, "source = <logical-entry>").unwrap(),
        }
        writeln!(
            output,
            "artifact-directory = {}",
            build.artifact_directory().display()
        )
        .unwrap();
        write_arguments(&mut output, "base-args", build.base_args());
        write_arguments(&mut output, "variant-args", build.variant_args());
        write_arguments(&mut output, "command-line-args", build.command_line_args());
        match build.compiler_working_directory() {
            Some(path) => writeln!(output, "compiler-cwd = {}", path.display()).unwrap(),
            None => writeln!(output, "compiler-cwd = <runner-default>").unwrap(),
        }
        writeln!(output, "compile-timeout = {:?}", build.timeout_seconds()).unwrap();
        writeln!(output, "compile-serial = {}", build.serial()).unwrap();
        writeln!(output, "compile-resources = {:?}", build.resources()).unwrap();

        match leaf.kind() {
            PlannedLeafKind::Compile(expectation) => {
                writeln!(output, "kind = compile").unwrap();
                write_stream(&mut output, "stderr", expectation.stderr());
                if let Some(prefix) = expectation.stderr_prefix_to_strip() {
                    write!(output, "stderr-prefix-to-strip = ").unwrap();
                    write_bytes_literal(&mut output, prefix);
                    output.push('\n');
                }
                writeln!(output, "dependencies = []").unwrap();
            }
            PlannedLeafKind::Run(run) => {
                writeln!(output, "kind = run").unwrap();
                writeln!(output, "run = {}", run.name()).unwrap();
                match run.args() {
                    ResolvedArgs::Utf8(arguments) => {
                        writeln!(output, "args = {arguments:?}").unwrap()
                    }
                    ResolvedArgs::File(path) => {
                        writeln!(output, "argv-file = {}", path.display()).unwrap()
                    }
                }
                write_bytes(&mut output, "stdin", run.stdin());
                for file in run.input_files() {
                    write!(output, "input-file.{} = ", file.name()).unwrap();
                    write_byte_value(&mut output, file.contents());
                    output.push('\n');
                }
                match run.cwd() {
                    ResolvedWorkingDirectory::Private => {
                        writeln!(output, "cwd = <private>").unwrap()
                    }
                    ResolvedWorkingDirectory::Fixture(path) => {
                        writeln!(output, "cwd = {}", path.display()).unwrap()
                    }
                }
                writeln!(output, "env = {:?}", run.env()).unwrap();
                writeln!(output, "run-timeout = {:?}", run.timeout_seconds()).unwrap();
                writeln!(output, "run-serial = {}", run.serial()).unwrap();
                writeln!(output, "run-resources = {:?}", run.resources()).unwrap();
                writeln!(output, "exit = {:?}", run.expectation().exit()).unwrap();
                write_stream(&mut output, "stdout", run.expectation().stdout());
                write_stream(&mut output, "stderr", run.expectation().stderr());
                for file in run.expectation().output_files() {
                    write!(output, "output-file.{} = ", file.name()).unwrap();
                    write_byte_value(&mut output, file.contents());
                    output.push('\n');
                }
                writeln!(output, "dependencies = [{:?}]", build.id()).unwrap();
            }
        }
        Some(output)
    }
}

fn write_bytes_literal(output: &mut String, bytes: &[u8]) {
    output.push_str("b\"");
    for byte in bytes {
        for escaped in std::ascii::escape_default(*byte) {
            output.push(char::from(escaped));
        }
    }
    output.push('"');
}

fn write_arguments(output: &mut String, label: &str, arguments: &[std::ffi::OsString]) {
    write!(output, "{label} = [").unwrap();
    for (index, argument) in arguments.iter().enumerate() {
        if index != 0 {
            output.push_str(", ");
        }
        write_os_string(output, argument);
    }
    output.push_str("]\n");
}

fn write_os_string(output: &mut String, value: &OsStr) {
    output.push('"');
    for byte in value.as_encoded_bytes() {
        for escaped in std::ascii::escape_default(*byte) {
            output.push(char::from(escaped));
        }
    }
    output.push('"');
}

fn write_bytes(output: &mut String, label: &str, source: &ResolvedByteSource) {
    write!(output, "{label} = ").unwrap();
    write_byte_value(output, source);
    output.push('\n');
}

fn write_byte_value(output: &mut String, source: &ResolvedByteSource) {
    match source {
        ResolvedByteSource::Inline(contents) => write!(output, "inline {contents:?}").unwrap(),
        ResolvedByteSource::File(path) => write!(output, "file {}", path.display()).unwrap(),
    }
}

fn write_stream(output: &mut String, label: &str, expectation: &ResolvedStreamExpectation) {
    match expectation {
        ResolvedStreamExpectation::Ignore => writeln!(output, "{label} = ignore").unwrap(),
        ResolvedStreamExpectation::Match { mode, expected } => {
            write!(output, "{label} = {mode:?} ").unwrap();
            write_byte_value(output, expected);
            output.push('\n');
        }
    }
}

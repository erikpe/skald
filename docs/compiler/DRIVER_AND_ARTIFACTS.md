# Driver and Artifacts

Status: authoritative for compiler orchestration, command-line behavior,
target and toolchain selection, runtime archive selection, output publication,
and driver failure boundaries. Compiler phases are owned by
[Phases and IR](PHASES_AND_IR.md), target emission by the
[Backend and Target Contract](BACKEND.md), and the linked C surface by the
[Runtime ABI](RUNTIME_ABI.md).

## Driver facade

The repository-internal `skald_compiler::driver` facade exposes three ways to
compose the compiler:

- `compile_source_to_assembly` runs one in-memory source through the complete
  frontend, MIR pipeline, selected backend, and assembly emission;
- `Toolchain::link_assembly` sends assembly to a configured host compiler
  driver and publishes the linked executable; and
- `run_cli` owns process arguments, source and output I/O, diagnostic
  rendering, toolchain selection, and process exit status.

`crates/skac` is deliberately only a process entry point: it forwards
`args_os()` to `run_cli` and exits with the returned status. The compiler crate
is unpublished and does not promise a version-stable API outside this
repository; see the [compiler crate API policy](README.md#compiler-crate-api-policy).

## Compilation orchestration

`compile_source_to_assembly(path, text, target)` creates a request-local source
database and runs lexing, parsing, resolution, type checking, MIR lowering,
the verified MIR pass pipeline, and target assembly emission in order. A
source-phase error stops later phases and returns the accumulated source
database and structured diagnostics. HIR lowering, MIR verification, and
backend failures remain distinct structured error categories. Static
inheritance, inherited access, class/`Obj` alias views, and inline slicing
reach verified target-independent MIR and execute through the current x86-64
base layout and internal static-view calling convention.

The path supplied to this entry point labels diagnostics; the function does
not read it. It performs no source I/O, host tool invocation, runtime linking,
or artifact publication. On success it returns assembly together with the
request report.

## Command-line modes

`skac --help` is the exact option reference. One invocation accepts one input
path with the canonical `.ska` suffix and selects one of two output modes:

| Mode | Selection | Default output |
|---|---|---|
| Native executable | default | input path with the `.ska` suffix removed |
| Textual assembly | `--emit asm` | input path with the suffix replaced by `.s` |

`-o` or `--output` selects another destination. Assembly mode runs the same
frontend and backend but does not require a runtime archive or invoke the host
toolchain. `--version`, `-h`, and `--help` complete without compilation.

The CLI reads source as UTF-8 text. A missing file, invalid UTF-8, or another
read failure is an input I/O error. Relative diagnostic paths are preserved;
an absolute input below the current directory is rendered relative to that
directory where possible, keeping diagnostics stable across checkout paths.

## Target selection

`--target <name>` is resolved through the public backend registry. Omitting it
uses `backend::DEFAULT_TARGET_NAME`; an unsupported name is a usage error and
never silently falls back. The current registry and target-specific behavior
are authoritative in the [backend contract](BACKEND.md#backend-interface-and-target-registry).

## Host toolchain and runtime selection

Executable mode streams generated assembly to the configured C compiler
driver through standard input. It constructs a subprocess directly rather
than a shell command. The invocation treats stdin as assembler input, passes
the runtime archive as a link input, and asks the tool to write to the pending
executable path.

The default configuration is:

| Setting | Default | Override |
|---|---|---|
| Host compiler driver | `cc` | `CC` |
| Runtime archive | `build/runtime/libskald_runtime.a` | `SKALD_RUNTIME_ARCHIVE` |

`CC` names one executable path; it is not parsed as a shell fragment or a list
of flags. The runtime path must identify an existing regular file before the
tool is started. Runtime ABI compatibility is then enforced by the
[version-specific link marker](RUNTIME_ABI.md#version-and-link-compatibility).

The driver captures tool stdout and stderr. Start, input-write, wait, nonzero
termination, and publication failures are returned as structured
`ToolchainError` categories. A nonzero tool result includes its exit status or
signal state and captured details in the user-facing error.

## Input protection and artifact publication

An explicit output is rejected when existing file metadata shows that it is
the input itself, a symbolic link to the input, or a hard link to the input.
The check compares the resolved Unix device and inode, so it protects source
contents rather than only comparing path spellings.

Assembly and executable outputs use the same publication protocol:

1. reserve a unique temporary file in the destination directory;
2. write assembly there or direct the host toolchain to that path;
3. leave any existing destination untouched until work succeeds; and
4. publish with one same-directory rename.

Ordinary failure and unwind paths remove the unpublished temporary through its
owner. Compilation, linking, and publication failures therefore preserve an
existing destination; no partial result is intentionally published. The
destination directory must already exist and permit temporary-file creation
and rename.

## Diagnostics and exit status

Source diagnostics use the compiler's structured renderer. A valid feature
whose next IR stage is not implemented is reported as a compiler-stage
limitation. Invalid MIR or backend failures are reported as internal compiler
failures, while host-tool and artifact errors retain their driver category.
User-controlled failures do not become compiler panics.

The CLI process statuses are:

| Status | Meaning |
|---:|---|
| `0` | Help, version, or compilation completed successfully. |
| `1` | Source compilation, internal verification/backend processing, or host toolchain failed. |
| `2` | Command usage, target selection, source suffix, or input/output alias was invalid. |
| `74` | Source or artifact I/O failed, including failure to write command output. |

Exact diagnostics are tested at their owning source, CLI, artifact, or
toolchain boundary. Host operating-system and tool messages are retained as
details and are not portable compiler wording.

## Verification

Driver tests are divided by responsibility:

- CLI tests cover help, version, argument rejection, suffix and target rules;
- pipeline tests compose public phases and structured failures;
- artifact tests cover assembly output, source alias rejection, preservation,
  and temporary cleanup;
- toolchain tests cover missing archives, process failures, unresolved
  externals, ABI mismatch, captured status, and executable preservation; and
- `crates/skac` integration tests exercise the real binary entry point.

Complete native golden cases additionally cover the real compiler process,
runtime archive, linker, published executable, stdout, stderr, and process
status.

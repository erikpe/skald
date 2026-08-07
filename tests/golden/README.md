# Golden Test Fixtures

Golden tests exercise complete source-to-diagnostic or source-to-executable
behavior through the Rust `skald-golden` runner. General test-layer guidance
lives in the [testing guide](../../docs/development/TESTING.md); this file owns
the fixture format and runner workflow.

## Organization and ownership

The runner recursively discovers `**/*.golden.toml` below this directory and
loads repository variants from `config.toml`. Each spec groups related source
programs under a language, runtime, or toolchain feature directory. A test
program belongs to exactly one spec; use named runs when several data cases can
share one compilation without obscuring the behavior under test.

Planning validates every spec and referenced path before filtering. It also
audits the complete fixture tree: every source, external expectation, and data
file must have exactly one spec owner. Compiler module and standard-library
roots own their contained provider files, and read-only fixture working
directories own their contained data. `oracles/` is the sole non-discovered
area; it contains independent generators for checked-in corpora, and each
generator names the spec and external files it updates.

Feature-local READMEs describe observation ownership and useful filters.
Provider trees belong below their feature's `cases/` directory so supporting
`.ska` files remain part of one typed compiler invocation rather than becoming
independent tests.

## Spec format

Every spec starts with `schema = 1`. Repository variants are declared in
`config.toml`; omitted `variants` selects `default`.

A native test compiles one source once per selected variant and may execute it
through several named runs:

```toml
schema = 1

[[test]]
name = "format"
mode = "run"
source = "format.ska"
compiler_args = ["--no-stdlib"]
variants = ["default"]

[[test.run]]
name = "small"
args = ["12"]
stdin = { inline = "input" }
expect = { exit = 0, stdout = { file = "data/small.stdout" } }

[[test.run]]
name = "large"
argv_file = "data/large.argv"
expect = { exit = 0, stdout = { match = "contains", inline = "done" } }
```

A compile-fail test requires compiler status 1 and empty stdout. Its stderr
expectation is explicit:

```toml
[[test]]
name = "wrong_type"
mode = "compile-fail"
source = "wrong_type.ska"

[test.expect.stderr]
match = "starts-with"
inline = """error[TYP005]: return value has the wrong type
 --> tests/golden/example/wrong_type.ska:2:12"""
```

Logical module entries omit `source` and express the invocation with typed
compiler arguments:

```toml
[[test]]
name = "modules"
mode = "run"
compiler_args = [
  "--entry", "app::main",
  "--module-root", "cases/modules",
  "--no-stdlib",
]

[[test.run]]
name = "default"
expect = { exit = 0 }
```

Source, module-root, standard-library-root, working-directory, input, and
expectation paths are relative to the containing spec and must remain inside
the golden root. Unknown compiler arguments pass through unchanged.

## Inputs, processes, and expectations

Inline data is UTF-8. External data files and `argv_file` are loaded byte for
byte without newline, encoding, whitespace, zero-byte, or terminal-escape
normalization. An argument file is a sequence of NUL-terminated byte strings;
consecutive delimiters preserve empty arguments, and every nonempty file must
end in NUL.

Omitted stdout and stderr expectations mean exact empty bytes. Stream
expectations support `exact` (the default), `starts-with`, `contains`, or
`ignore = true`; partial fragments must be nonempty. Inline and file data work
with every match mode. Use exact matching for complete output ownership and a
reviewed partial matcher for a stable diagnostic or panic fragment that may
gain richer surrounding context.

Each run receives a private mode-`0700` temporary directory. Declared
`input_files` are written there, `output_files` are compared afterward, and
`{tmp:name}` in arguments or stdin expands to an absolute named path. The
private directory is the default working directory. A
`cwd = { fixture = "..." }` directory is shared and read-only; the runner never
populates it.

Child environments are rebuilt from the toolchain allowlist plus declared
values and a private `TMPDIR`. Stdin writing and output capture proceed
concurrently. Each process has a timeout, and Linux timeouts terminate the
complete child process group. `serial = true` requests exclusive execution;
equal names in `resources = ["..."]` prevent only those nodes from overlapping.

The runtime is prepared once when a selection contains native tests.
Independent compiler, linker, and run nodes share the bounded worker pool.
Failed prerequisites cancel dependents without stopping unrelated work unless
`--fail-fast` is selected. Final reporting remains in canonical ID order.

## Selection and determinism

Canonical leaf IDs have the form
`<spec-without-.golden.toml>::<test>::<variant>::<run>`; compile-fail leaves end
in `::<compile>`. `--filter` and `--exclude` are repeatable, `*` stays within a
path or identity component, and `**` crosses components. `--exact` selects one
leaf, `--variant` restricts variants, and an empty selection is an error unless
`--allow-empty` is explicit.

Determinism defaults to `off`: each compiler and native process runs once.
`compile` repeats compiler processes and compares assembly or diagnostics.
`full` also repeats native processes and compares their complete observations.

Common commands from the repository root are:

```text
make golden-test
make golden-filter GOLDEN_FILTER='syntax/**'
make golden-exact GOLDEN_ID='primitive_strings/values::values::default::bytes_slices_and_concatenation'
make golden-determinism-test
scripts/golden.sh --list --filter 'modules/**'
scripts/golden.sh --explain '<canonical-leaf-id>'
scripts/golden.sh --determinism compile --filter 'declarations/**'
scripts/golden.sh --format json --filter 'runtime/**'
```

`--jobs 1` is useful for sequential debugging. `--show-output` includes passing
observations, `--slowest N` reports stable timings, and `--format json` or
`--format junit` emits machine-readable results. There is no blessing or
implicit expectation-update mode.

Passing run sandboxes are deleted unless `--keep-all-artifacts` is selected.
Build products and failed or incompletely prepared sandboxes remain under
`build/golden/` and are identified in failure reports. These artifacts are
disposable.

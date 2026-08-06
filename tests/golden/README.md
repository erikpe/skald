# Golden Test Fixtures

Golden tests exercise complete compiler behavior. General guidance on when to
use this layer is in the [testing guide](../../docs/development/TESTING.md);
this file defines discovery and expectation formats.

## Spec planning and inspection

The Rust `skald-golden` tool discovers `**/*.golden.toml` files below this
directory and loads repository variants from `config.toml`. It validates every
discovered spec and referenced fixture before applying filters, resolves all
fixture paths below this golden root, and expands stable spec, test, build, and
leaf IDs without creating artifacts or starting processes.

The currently implemented interface is read-only:

```text
cargo run --locked -p skald-golden -- --list --allow-empty
cargo run --locked -p skald-golden -- --list-tests --filter 'language/**' --allow-empty
cargo run --locked -p skald-golden -- --explain '<canonical-leaf-id>'
```

`--filter` and `--exclude` are repeatable. `*` stays within a path or identity
component and `**` crosses components. `--exact` selects one canonical leaf,
`--variant` restricts build variants, and a selection matching nothing is an
error unless `--allow-empty` is explicit. Canonical leaf IDs have the form
`<spec-without-.golden.toml>::<test>::<variant>::<run>`; compile-fail leaves
end in `::<compile>`.

Legacy cases are not part of spec discovery, so an empty spec selection still
requires the explicit `--allow-empty` policy. The legacy runner and sidecar
contract below remain the executable golden-test authority.

The runner recursively discovers two case families:

- `run/**/*.ska` requires a same-named `.exit` sidecar containing either an
  exact process status in `0..=255` or `failure` when the contract promises
  only unsuccessful termination. `failure` accepts a nonzero status or signal
  without freezing platform trap details. Optional `.stdout` and `.stderr`
  sidecars contain the exact expected bytes for their respective streams;
  absence means that stream must be empty. An optional `.stdin` sidecar
  contains the exact bytes supplied to the executable; absence means immediate
  EOF. An optional `.argv` sidecar contains the additional executable
  arguments after element zero as NUL-terminated byte strings; absence or an
  empty file means no additional arguments.
- `compile_fail/**/*.ska` requires a same-named `.stderr` sidecar containing
  exact rendered diagnostics.

Existing single-file cases keep this convention unchanged. A directory that
contains `case.args` is instead one multi-file case. The runner does not
descend into that directory, so supporting `.ska` files are never mistaken for
independent cases. Each nonempty, non-comment line in `case.args` is one exact
`skac` argument; arguments are not split on whitespace. This compiler manifest
does not supply arguments to the generated executable. Paths are relative to
the case directory because the compiler runs with that directory as its
working directory. This makes the entry mode and all root/standard-library
choices explicit:

```text
--entry
app::main
--module-root
application modules
--module-root
dependency modules
--stdlib-root
sdk modules
```

Use a source path line instead of `--entry` plus a logical path for positional
entry cases. Multi-file sidecars are named `case.exit`, optional `case.argv`,
optional `case.stdin`, optional `case.stdout`, or optional `case.stderr`. Keep
all module roots and support files below the case directory so fixtures remain
hermetic and relocatable. In multi-file diagnostic snapshots, the required
`case.stderr` contains compiler diagnostics; the runner removes the absolute
case-directory prefix so source and provider paths begin with the relative
fixture path such as `modules/app.ska`.

Each `.argv` record ends with one NUL byte. Consecutive NUL bytes encode empty
arguments, so one NUL encodes one empty argument and `value\0\0` encodes
`value` followed by an empty argument. The terminal delimiter completes the
last record; it is not an extra argument. A nonempty `.argv` without a final
NUL is invalid fixture data. The runner constructs Unix arguments directly
from each record, preserving spaces, line feeds, empty values, and non-UTF-8
bytes exactly. The operating system supplies element zero from the executable
path; `.argv` contains only the suffix.

Sidecars are byte-for-byte inputs or expectations. Whitespace, line endings,
trailing line feeds, zero bytes, and non-UTF-8 stream or argument bytes are not
normalized. Compile failures must produce no stdout and exit with compiler
status 1.

`oracles/` contains non-discovered, rerunnable generators for checked-in golden
corpora whose expected values need an implementation independent of the code
under test. Generators print labelled fixture sections and never participate
in ordinary golden discovery.

The root Makefile builds the runtime before the runner invokes the real `skac`
binary. Single-file cases compile and run from the repository root; multi-file
cases compile and run from the directory containing `case.args`, so relative
fixture resources have one deterministic base. The runner compiles each run
case to assembly in two independent processes and compares the bytes before
linking and execution. It likewise compiles each failure twice and compares
stderr before checking its snapshot. Each native executable runs twice with
the same working directory, exact arguments, and stdin; both status and output
must agree before stdout, stderr, and process status are checked independently.
Nonempty stdin is written concurrently with output collection, so inputs
larger than host pipe capacity cannot deadlock the runner.
Disposable artifacts are written under `build/golden/`.

Keep each source focused and give related cases descriptive names. Put
source-visible, target-independent expectations in the source and sidecars;
keep target-specific instruction assertions in backend tests. Run all cases
from the repository root with:

```text
make golden-test
```

`make golden-test` includes the sidecar loader and mismatch-reporting suite.
Run only that focused suite with:

```text
make golden-expectations-test
```

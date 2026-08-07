# Golden Test Fixtures

Golden tests exercise complete compiler behavior. General guidance on when to
use this layer is in the [testing guide](../../docs/development/TESTING.md);
this file defines discovery and expectation formats.

## Spec planning, inspection, and parallel execution

The Rust `skald-golden` tool discovers `**/*.golden.toml` files below this
directory and loads repository variants from `config.toml`. It validates every
discovered spec and referenced fixture before applying filters, resolves all
fixture paths below this golden root, and expands stable spec, test, build, and
leaf IDs without creating artifacts or starting processes.

The runner can inspect or execute the mixed new-format and legacy suite:

```text
cargo run --locked -p skald-golden -- --list --allow-empty
cargo run --locked -p skald-golden -- --list-tests --filter 'language/**' --allow-empty
cargo run --locked -p skald-golden -- --explain '<canonical-leaf-id>'
cargo run --locked -p skald-golden -- --compiler target/debug/skac --exact '<canonical-leaf-id>'
cargo run --locked -p skald-golden -- --jobs 1 --filter 'runner/**'
cargo run --locked -p skald-golden -- --filter 'standard_io/**'
cargo run --locked -p skald-golden -- --determinism full --filter 'runtime/panic**'
cargo run --locked -p skald-golden -- --format json --filter 'language/**'
cargo run --locked -p skald-golden -- --slowest 10 --show-output --filter 'runner/**'
```

`--filter` and `--exclude` are repeatable. `*` stays within a path or identity
component and `**` crosses components. `--exact` selects one canonical leaf,
`--variant` restricts build variants, and a selection matching nothing is an
error unless `--allow-empty` is explicit. Canonical leaf IDs have the form
`<spec-without-.golden.toml>::<test>::<variant>::<run>`; compile-fail leaves
end in `::<compile>`.

During migration, the same planner also adapts `run/` and `compile_fail/`
sidecar cases without moving them. A legacy expectation stem such as
`run/strings.ska` becomes `run/strings::default::<run>`; a `case.args` stem
such as `compile_fail/modules_privacy/case.args` becomes
`compile_fail/modules_privacy/case::default::<compile>`. Artifact directory
names combine a readable prefix with a stable hash, so flattened legacy paths
cannot collide.

Execution locates `skac` beside `skald-golden` by default or accepts an
explicit `--compiler PATH`. `--compiler-arg` appends an argument after spec
and variant arguments. `--determinism off` is the default; `compile` repeats
compiler processes and compares assembly or diagnostics, while `full` also
repeats native processes and compares their complete observations. `--jobs N`
bounds all active compiler, runtime-preparation, linker, and generated-program
processes. It defaults to host available parallelism; `--jobs 1` is the stable
single-worker debugging mode. `--fail-fast` stops starting unrelated work after
the first observed failure while already active work completes or times out.
`--timeout SECONDS` overrides the default compiler, linker, and native-process
bound; a timeout declared by a test still takes precedence. Use
`--keep-all-artifacts` to retain passing run sandboxes as well as the failed
sandboxes and build products retained by default.

Human reports print the determinism mode and resolved ownership counts before
execution, then emit canonical-ID-ordered results and distinct spec, source
test, build, run, compiler-process, link, execution, failure, cancellation, and
duration counts. Failures include the stage, byte-safe command, working and
artifact directories, termination, every mismatch, exact byte lengths, match
policy and successful partial-match offset, escaped binary data, and bounded
UTF-8 diffs. `--show-output` expands passing stage observations and
`--slowest N` prints a stable duration ranking. `--format json` and
`--format junit` encode the same IDs, stages, durations, statuses, and failure
details as single machine-readable documents. An early pipe consumer exit is
treated as successful output termination. The runner has no blessing or
implicit expectation-update mode.

The compatibility adapter preserves the sidecar contract below: ordinary
sources use the repository as compiler and executable working directory,
`case.args` cases use their case directory, optional stream files still mean
exact empty bytes when absent, and compile-fail case-directory prefixes are
removed before diagnostic determinism and expectation checks. Legacy native
runs hold a resource lock for their shared working directory, preventing the
parallel scheduler from exposing old fixtures to races. The adapter remains
until every sidecar fixture has migrated to a feature spec.

Migrated feature specs currently live in `calls/`, `control_flow/`, `modules/`,
`operators/`, `primitives/`, `private_initializers/`, `primitive_strings/`,
`process/`, `runtime/`, `standard_io/`, `standard_library/`, `standard_test/`,
and `static_fields/`. Their local READMEs describe observation ownership and
focused filters. Checked files in
`migrations/` map every replaced legacy leaf to its authoritative spec leaf;
an integration test requires every old leaf to be absent, every replacement
to be present exactly once, and the complete 290-leaf observation count to
remain unchanged.

## New-format process and expectation contract

The Rust runner's compiler-independent execution layer implements the process
semantics used by new-format runs. Inline stream data is UTF-8 from TOML;
external data files and Unix argument manifests are loaded byte for byte.
Nothing normalizes newlines, encoding, zero bytes, whitespace, or terminal
escapes. Omitted stdout and stderr expectations mean exact empty bytes.
Explicit stream expectations support `exact` (the default), `starts-with`,
`contains`, or `ignore = true`; partial fragments must be nonempty. Both inline
and file data work with every matching policy.

For compile-fail tests with module or replacement-standard-library roots, the
planner removes the canonical common provider-owner prefix before determinism
and expectation checks. Diagnostics therefore retain stable relative paths
such as `modules/app.ska` and `first/shared.ska` even though compiler commands
use canonical contained provider roots.

An `argv_file` is a sequence of NUL-terminated byte strings. Its empty form has
no arguments, while consecutive delimiters preserve empty arguments. Every
nonempty file must end in NUL. This representation preserves non-UTF-8 Unix
arguments and is the same encoding described for legacy `.argv` sidecars
below.

Each run gets a mode-`0700` private directory under the configured temporary
root. Named input files are written there before execution, named output files
are compared as exact bytes afterward, and `{tmp:name}` in arguments or stdin
expands to the absolute named path. The private directory is the default
working directory. `cwd = { fixture = "..." }` instead selects a contained
fixture directory that the case must treat as read-only; the runner never
populates or modifies that shared directory.

Child environments are cleared and reconstructed from the toolchain allowlist
(`PATH`, `CC`, `SKALD_RUNTIME_ARCHIVE`, and `SKALD_STDLIB_ROOT`), the private
`TMPDIR`, and declared per-run values. Stdin writing and stdout/stderr capture
run concurrently. Each process has its own timeout; on Linux a timeout kills
the complete child process group and remains distinct from an exit code or
signal. Passing sandboxes are deleted unless all artifacts were requested;
failed or incompletely prepared sandboxes are retained for inspection.

The dependency coordinator prepares the runtime through `make runtime` once
and only when native leaves are selected. Independent compiler nodes, links,
and named runs enter one fixed worker pool as their prerequisites become ready.
A failed prerequisite cancels only its dependent link or run; unrelated builds
continue unless `--fail-fast` was selected. Final results remain in canonical
leaf-ID order regardless of completion order.

`serial = true` gives its compiler/link or run node exclusive use of the worker
pool. A node declaring `resources = ["name"]` cannot overlap another active
node holding the same name, but may overlap nodes using unrelated resources.
The coordinator never holds its scheduling state while a worker executes a
process. Passing run sandboxes are removed, while unique build products and
failed sandboxes remain under `build/golden/` and are identified in failure
reports.

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

The root Makefile builds `skac` and `skald-golden`; native selections then
prepare the runtime archive exactly once. Single-file cases compile and run
from the repository root; multi-file cases compile and run from the directory
containing `case.args`, so relative fixture resources have one deterministic
base. Ordinary execution observes every compiler and native process once.
`make golden-determinism-test` repeats successful and failing compiler
processes, compares assembly or diagnostic bytes, and repeats native
executions with the same working directory, arguments, and stdin before
checking expectations. Nonempty stdin is written concurrently with output
collection, so inputs larger than host pipe capacity cannot deadlock the
runner. Disposable artifacts are written under `build/golden/`.

Keep each source focused and give related cases descriptive names. Put
source-visible, target-independent expectations in the source and sidecars;
keep target-specific instruction assertions in backend tests. Run all cases
from the repository root with:

```text
make golden-test
```

Run the focused byte, sidecar-adaptation, and mismatch-reporting tests with:

```text
make golden-expectations-test
```

Run common filtered or exact selections through Make:

```text
make golden-filter GOLDEN_FILTER='compile_fail/**'
make golden-exact GOLDEN_ID='run/strings::default::<run>'
```

For multiple filters, inspection, machine reports, or other runner options,
use the argument-forwarding wrapper documented in
[`scripts/README.md`](../../scripts/README.md).

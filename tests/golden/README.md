# Golden Test Fixtures

Golden tests exercise complete compiler behavior. General guidance on when to
use this layer is in the [testing guide](../../docs/development/TESTING.md);
this file defines discovery and expectation formats.

The runner recursively discovers two case families:

- `run/**/*.ska` requires a same-named `.exit` sidecar containing either an
  exact process status in `0..=255` or `failure` when the contract promises
  only unsuccessful termination. `failure` accepts a nonzero status or signal
  without freezing platform trap details. Optional `.stdout` and `.stderr`
  sidecars contain the exact expected bytes for their respective streams;
  absence means that stream must be empty.
- `compile_fail/**/*.ska` requires a same-named `.stderr` sidecar containing
  exact rendered diagnostics.

Existing single-file cases keep this convention unchanged. A directory that
contains `case.args` is instead one multi-file case. The runner does not
descend into that directory, so supporting `.ska` files are never mistaken for
independent cases. Each nonempty, non-comment line in `case.args` is one exact
`skac` argument; arguments are not split on whitespace. Paths are relative to
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
entry cases. Multi-file sidecars are named `case.exit`, optional `case.stdout`,
or optional `case.stderr`. Keep all module roots and support files below the
case directory so fixtures remain hermetic and relocatable. In multi-file
diagnostic snapshots, the required `case.stderr` contains compiler
diagnostics; the runner removes the absolute case-directory prefix so source
and provider paths begin with the relative fixture path such as
`modules/app.ska`.

Sidecars are byte-for-byte expectations. Whitespace, line endings, trailing
line feeds, and non-UTF-8 stream bytes are not normalized. Compile failures
must produce no stdout and exit with compiler status 1.

The root Makefile builds the runtime before the runner invokes the real `skac`
binary. Single-file cases compile and run from the repository root; multi-file
cases compile and run from the directory containing `case.args`, so relative
fixture resources have one deterministic base. The runner compiles each run case to assembly
in two independent processes and compares the bytes before linking and
execution. It likewise compiles each failure twice and compares stderr before
checking its snapshot. Each native executable runs twice; both status and
output must agree before stdout, stderr, and process status are checked
independently. Disposable artifacts are written under `build/golden/`.

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

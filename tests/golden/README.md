# Golden Test Fixtures

Golden tests exercise complete compiler behavior. General guidance on when to
use this layer is in the [testing guide](../../docs/development/TESTING.md);
this file defines discovery and expectation formats.

The runner recursively discovers two case families:

- `run/**/*.ska` requires a same-named `.exit` sidecar containing a Linux
  process status in `0..=255`. An optional `.stdout` sidecar contains exact
  expected stdout bytes; absence means empty stdout.
- `compile_fail/**/*.ska` requires a same-named `.stderr` sidecar containing
  exact rendered diagnostics.

Sidecars are byte-for-byte expectations. Whitespace, line endings, trailing
line feeds, and non-UTF-8 stdout are not normalized. Successful native cases
must produce empty stderr. Compile failures must produce no stdout and exit
with compiler status 1.

The runner builds the runtime and invokes the real `skac` binary from the
repository root. It compiles each run case to assembly in two independent
processes and compares the bytes before linking and execution. It likewise
compiles each failure twice and compares stderr before checking its snapshot.
Each native executable runs twice; both status and output must agree before
stdout, stderr, and process status are checked independently. Disposable
artifacts are written under `build/golden/`.

Keep each source focused and give related cases descriptive names. Put
source-visible, target-independent expectations in the source and sidecars;
keep target-specific instruction assertions in backend tests. Run all cases
from the repository root with:

```text
make golden-test
```

The sidecar loader and mismatch reporting have a separate focused suite:

```text
cargo test --locked -p skac --test golden-expectations
```

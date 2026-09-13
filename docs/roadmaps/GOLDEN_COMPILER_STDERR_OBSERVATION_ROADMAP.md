# Raw Compiler Stderr Observation Roadmap

Status: planned; G01 is next.

The golden runner currently removes the fixture path prefix directly from a
`ProcessObservation` after the compiler exits. That makes the process model
look raw while silently replacing its stderr bytes, and it gives reporting no
way to distinguish captured output from the bytes used for portable diagnostic
matching. This roadmap preserves the exact bounded child output and moves path
normalization into an explicit compiler-comparison view without changing
compile-fail expectations, determinism policy, or report schemas.

## Scope and invariants

- `ProcessObservation` owns the exact stdout and stderr bytes retained by the
  process runner. Compilation, matching, determinism, and reporting must not
  mutate those bytes after capture.
- Compiler diagnostic normalization is byte-oriented and removes every
  non-overlapping occurrence of the planned absolute fixture path prefix. This
  is the existing behavior, including repeated occurrences and occurrences
  inside diagnostic message text; it is path normalization rather than a
  leading-prefix operation.
- Missing and empty normalization prefixes are identity operations. Non-UTF-8
  bytes before, between, and after path occurrences remain byte-for-byte
  intact.
- Compile-fail matchers continue to consume normalized stderr. Match modes,
  independent matcher outcomes, offsets, diagnostic ordering, and expected
  status remain unchanged.
- Compile determinism continues to compare termination, stdout, normalized
  stderr, pipe failures, and capture overflows. Successful compilation still
  compares assembly and treats any raw stderr as unexpected.
- `CompilerObservation::process()` remains the raw observation boundary. A
  separate compiler-owned accessor exposes the stderr comparison view for each
  completed compiler process. Avoid parallel vectors or index-coupled side
  tables in `CompilationExecution`.
- `StreamComparison::actual()` remains the authoritative bytes against which
  matcher offsets were computed. Compiler-stage reports use the explicit
  comparison view and retain their current human, JSON, and JUnit shapes and
  portable normalized stderr content.
- Capture limits apply to raw pipe bytes before normalization. This work does
  not change process lifetime, truncation, matcher, schema, or diagnostic-code
  contracts and does not introduce a general text-normalization framework.

## Progress

- [ ] G01 — Separate raw capture from compiler diagnostic comparison
- [ ] G02 — Harden reporting, compatibility, and documentation

## PR-sized implementation sequence

### G01 — Separate raw capture from compiler diagnostic comparison

**Purpose:** establish one honest ownership boundary from captured compiler
bytes through normalization, matching, determinism, and report projection.

- [ ] Add a private compiler-diagnostic view beside compiler invocation and
  observation. Represent an unchanged raw view without copying where practical,
  and own normalized bytes only when path removal changes the stream.
- [ ] Rename the planned normalization input and its explain output around an
  absolute diagnostic path prefix. Document that all non-overlapping byte
  occurrences are removed; do not describe the operation as stripping one
  leading prefix.
- [ ] Remove `ProcessObservation::strip_stderr_prefix` and the process-layer
  byte replacement helper. Process observations must be immutable after
  `run_process` returns.
- [ ] Build the diagnostic comparison view for every completed compiler
  repetition. Use it for compile-fail stderr matching and normalized diagnostic
  determinism while leaving status, stdout, pipe-failure, overflow, successful
  compilation, and assembly checks unchanged.
- [ ] Make `CompilerObservation` expose raw process stderr through `process()`
  and the explicit comparison bytes through a narrowly named accessor. Keep
  normalization state attached to its owning observation rather than relying
  on positional state in `CompilationExecution`.
- [ ] Project compiler stderr reports from the comparison view. Assert that the
  first reported comparison uses the same bytes as `StreamComparison::actual()`
  so matcher offsets cannot be paired with raw bytes accidentally.
- [ ] Add focused binary-safe tests for absent and empty prefixes, repeated and
  adjacent path occurrences, path text embedded in a diagnostic message,
  non-UTF-8 surrounding bytes, and inputs with no matching occurrence.

**Tests:** Run `cargo test --locked -p skald-golden process::`, the focused
compiler invocation/model tests, `cargo test --locked -p skald-golden --test
sequential_execution`, and `cargo test --locked -p skald-golden --test
reporting`. Run workspace all-target Clippy with warnings denied and
`git diff --check`.

**Exit criteria:** every completed compiler process retains its exact captured
stderr; compile-fail matching, determinism, and compiler-stage reporting all
consume one explicit normalized view; repeated paths and arbitrary bytes retain
the established comparison behavior; and no process-layer normalization
method remains.

### G02 — Harden reporting, compatibility, and documentation

**Purpose:** prove the new ownership boundary across public observations and
all report formats, then leave A36 closed with current documentation.

- [ ] Add an end-to-end compile-fail fixture whose stderr contains the absolute
  diagnostic prefix more than once, matching message text, and binary bytes.
  Assert raw `ProcessObservation` bytes separately from normalized comparison
  and report bytes.
- [ ] Cover two compiler repetitions whose raw diagnostic paths differ only in
  normalized occurrences. Confirm the existing determinism policy compares the
  normalized views and still detects differences outside those occurrences.
- [ ] Verify human, JSON, and JUnit reporting retain normalized portable output,
  matching policies, matcher order, and offsets without changing serialized
  report field names or structure.
- [ ] Verify raw capture overflow and pipe failures are still reported from the
  process observation and cannot be erased by normalization.
- [ ] Update golden-runner testing documentation to name the raw observation
  and normalized compiler-diagnostic views, their byte-removal semantics, and
  their roles in matching, determinism, and reporting.
- [ ] Remove obsolete names and comments, record the delivered implementation
  and validation under A36 in the cleanup audit, and mark every roadmap
  checkbox complete only after the full gates pass.
- [ ] When all exit criteria hold, archive this completed roadmap, update the
  active and archive indexes, and repair incoming links.

**Tests:** Run `cargo test --locked -p skald-golden`, `make check`, `make
msrv-check`, `cargo fmt --all -- --check`, the documentation checker, and `git
diff --check`. Run the focused compile-determinism case with the real report
projection in each supported format.

**Exit criteria:** public process observations preserve exact child stderr;
portable matcher and report behavior remains stable; determinism compares the
documented normalized view; raw capture defects remain visible; living
documentation describes the final contract; A36 is complete; and the roadmap
is archived.

## Ordering and dependencies

G01 owns the representation and must land before broader reporting assertions
can depend on it. It keeps raw bytes and comparison bytes on the same compiler
observation, avoiding a temporary or permanent index-coupled execution model.
G02 then freezes the cross-layer contract in integration and format tests and
closes the documentation. The bounded process work from A01 through A04 is
already complete; this roadmap changes only post-capture ownership and has no
dependency on the remaining compiler or standard-library cleanup findings.

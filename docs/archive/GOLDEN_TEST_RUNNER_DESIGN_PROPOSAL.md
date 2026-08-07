# Golden Test Runner Design Proposal

Status: G1 through G12 confirmed, frozen, and promoted on 2026-08-05; the
[implementation roadmap](GOLDEN_TEST_RUNNER_ROADMAP.md) completed and was
archived on 2026-08-07. This proposal remains the historical design and
decision record; living documentation owns implemented behavior.

This proposal replaces Skald's organically grown golden-test harness with a
spec-driven Rust runner. The design adopts the useful organization and
execution features demonstrated by the sibling Niflheim repository while
preserving Skald's stronger exact-byte, diagnostic, process, and determinism
contracts.

The designed runner is a repository tool, not part of the Skald language or a
new compiler API. Current behavior remains authoritative in
[`tests/golden/README.md`](../../tests/golden/README.md) until an approved
implementation changes it and updates the living testing guide.

## Intended outcome

The completed redesign should provide:

- feature-oriented, spec-driven golden fixtures;
- multiple named executions of one compiled program;
- named compiler-flag variants that reuse the same executions and
  expectations;
- exact input and expected data written inline or stored in separate files;
- bounded parallel compilation, linking, and execution;
- one runtime build reused by every selected native case;
- filtering down to a spec, source test, compiler variant, or individual run;
- isolated build and runtime artifacts without filename collisions;
- timeouts, deterministic reporting, and actionable mismatch diagnostics;
- a gradual migration path from every existing sidecar and `case.args`
  fixture; and
- no loss of Skald's exact-byte assertions or ability to request complete
  cross-process determinism coverage.

## Current boundary and audit evidence

### Skald

The existing runner discovers `.ska` files recursively below the separate
`tests/golden/run` and `tests/golden/compile_fail` roots. A directory containing
`case.args` becomes one multiple-file case and stops recursive discovery below
that point.

At the 2026-08-05 audit point, discovery produced:

| Category | Cases |
|---|---:|
| Native execution | 150 |
| Compile failure | 138 |
| Total | 288 |

Every native case is compiled to assembly twice, compiled and linked to an
executable once more, and executed twice. Every compile-failure case is
compiled twice. The complete suite therefore performs 726 compiler process
invocations and 300 generated-program process invocations, all serially.

A warm `make golden-run-test` completed successfully in 136.09 seconds on the
audit host, which exposed 16 logical CPUs. Cargo preparation was already warm;
the runner dominated the elapsed time.

Important current strengths are:

- successful output, diagnostic output, stdin, and executable arguments are
  byte exact;
- missing stdout, stderr, stdin, or argument sidecars mean exact empty data;
- executable arguments preserve whitespace, empty values, and non-UTF-8 Unix
  bytes;
- compile failures require compiler status 1, empty stdout, and an exact
  stderr snapshot;
- successful assembly, failure diagnostics, and native observations must be
  deterministic across independent processes;
- stdin is written concurrently with output collection so large inputs cannot
  deadlock; and
- the root Makefile builds `build/runtime/libskald_runtime.a` once and the
  compiler links that archive into every native case.

The redesign must preserve these behaviors unless this proposal explicitly
changes them.

### Niflheim

The audited Niflheim runner discovers only `test_*_spec.yaml` files. Its
current corpus contains 80 specs, 233 source-backed test entries, 107 run-mode
programs, 126 compile-failure programs, and 1,032 named runtime executions.
One compiled program has as many as 61 executions, and 18 spec/source pairs
compile the same source under more than one flag configuration.

Useful Niflheim features are:

- one spec may collect multiple sources and both run and compile-fail modes;
- one successful compilation may feed many named runs;
- arguments, stdin, stdout, stderr, working directories, and compiler flags
  are declarative;
- stdin and expected streams may be inline or loaded from files;
- temporary input files, expected output files, and temporary-path
  substitution are supported;
- the runtime is built once per runner invocation and passed to every link;
- specs run concurrently under a configurable worker limit;
- filtering matches spec and source paths; and
- mismatch output includes bounded unified text diffs.

The Niflheim design also exposes boundaries Skald should improve:

- specs, rather than independently runnable build and run nodes, are the
  parallel scheduling unit;
- executions within one source test remain serial;
- omitted stdout, stderr, and exit fields are unchecked rather than strict;
- subprocesses use text mode and cannot express all of Skald's byte contracts;
- compile failures expose one text substring check rather than explicit
  byte-oriented exact, prefix, and containment policies;
- unknown fields are not rejected systematically;
- a filter matching no tests exits successfully;
- compiler and generated-program processes have no timeout; and
- a compile-fail entry without a requested substring mistakenly accepts
  successful compilation in the audited implementation.

These limitations are evidence for an independent Skald design rather than a
reason to port the Python code directly.

## Scope and invariants

The design includes:

- a Rust workspace tool with a reusable library and a thin command-line
  binary;
- a versioned declarative fixture schema;
- discovery, validation, expansion, filtering, scheduling, execution,
  comparison, and reporting;
- runtime preparation and assembly linkage for native cases;
- compatibility loading for the current fixture format during migration; and
- unit and integration coverage for the runner itself.

The following invariants apply:

1. The runner invokes the real `skac` executable in a fresh process for every
   compilation observation.
2. A successful build variant is compiled independently of every other build
   variant.
3. One compiled executable may serve any number of named runs with distinct
   data.
4. All independent work may run concurrently unless an explicit dependency or
   declared resource requires serialization.
5. Every selected process has a timeout and captured stdout, stderr, and exit
   observation.
6. Missing stream expectations mean exact empty bytes, never "do not check."
7. All referenced fixtures and working directories remain below the golden
   root after canonicalization unless a future explicit escape mechanism is
   separately designed.
8. Build and temporary artifacts never become source inputs.
9. Human-readable output is stable enough to compare between runs even when
   process completion order differs.
10. Machine-readable output identifies every leaf case by one stable canonical
    ID.

This proposal does not:

- replace compiler unit, integration, backend, binary CLI, runtime, or
  documentation tests;
- turn the runner into a general build system;
- introduce distributed execution;
- cache compiler products across separate runner invocations in the first
  implementation;
- make target-specific assembly snapshots ordinary native golden tests;
- update expectations implicitly during an ordinary run;
- add repository CI configuration; or
- change source-language behavior.

## Design principles

1. **Specs describe intent.** Directory layout and sidecar naming no longer
   infer whether every source file is an independent case.
2. **Assertions are explicit.** Omitted output means exact empty bytes. A
   declared expectation chooses exact, prefix, containment, or ignored
   matching deliberately.
3. **Compilation and execution are different work.** One build variant owns an
   executable; its named runs own process inputs and observations.
4. **Specs do not constrain scheduling.** A spec is an ownership and reporting
   collection, not a serial execution lock.
5. **Parallel safety is structural.** Unique artifact directories and private
   runtime sandboxes remove collisions instead of relying on test authors to
   avoid them.
6. **The real process boundary matters.** Golden tests continue to cover CLI
   parsing, diagnostic rendering, tool invocation, and native behavior.
7. **Determinism is deliberate.** Ordinary runs avoid repeated processes;
   compile and full determinism remain explicit audit modes for scheduled
   automation and changes that can affect reproducibility.
8. **Migration is incremental.** The current corpus remains runnable while
   feature groups move to specs.

## Decision register

Every direction below was confirmed on 2026-08-05. The implementation roadmap
must deliver the complete design without reopening these decisions implicitly.

| ID | Decision | Confirmed direction | State |
|---|---|---|---|
| [G1](#g1--runner-ownership) | Runner ownership | Separate `skald-golden` workspace tool invoking real `skac` processes | **Confirmed** |
| [G2](#g2--specification-format) | Specification format | Strict, versioned TOML in `*.golden.toml` files | **Confirmed** |
| [G3](#g3--dependency-policy) | Dependency policy | Permit narrowly scoped schema and machine-report dependencies only in the runner tool | **Confirmed** |
| [G4](#g4--fixture-organization) | Fixture organization | Organize by feature, mixing successful and failing cases in one collection | **Confirmed** |
| [G5](#g5--build-variants) | Compiler configurations | Named repository variants plus test-local compiler arguments | **Confirmed** |
| [G6](#g6--input-and-expectation-semantics) | Inputs and expectations | Byte-oriented exact or stable partial matching; inline UTF-8 or external byte files | **Confirmed** |
| [G7](#g7--execution-planning-and-parallelism) | Parallelism | Dependency-aware leaf scheduling under one bounded worker budget | **Confirmed** |
| [G8](#g8--runtime-and-link-ownership) | Runtime and linking | Build runtime once; link the first checked assembly through the shared toolchain owner | **Confirmed** |
| [G9](#g9--determinism-modes) | Determinism | Default to one observation; run compile or full audits explicitly when warranted | **Confirmed** |
| [G10](#g10--filtering-and-identities) | Filtering | Stable leaf IDs with repeatable include/exclude globs and exact selection | **Confirmed** |
| [G11](#g11--process-isolation-and-timeouts) | Isolation | Per-run temporary roots, unique build roots, bounded execution, explicit shared resources | **Confirmed** |
| [G12](#g12--migration-boundary) | Migration | Legacy adapter first, then feature-by-feature spec migration | **Confirmed** |

## G1 — Runner ownership

**Question:** Where should the runner live, and how should it reach the
compiler?

**Confirmed decision:** Add a separate `crates/skald-golden` workspace package
with a reusable library and a thin binary. The package is repository tooling,
not a production compiler dependency. It invokes the real `skac` executable
rather than compiling through an in-process compiler API.

The default executable lookup is a sibling `skac` binary beside the running
`skald-golden` binary. `--compiler <path>` overrides it. The root Makefile
builds `skac` before launching the runner, preserving the Makefile as the
shared local and external automation interface.

The confirmed internal facade is:

```text
skald-golden
  cli             command parsing and user-facing options
  spec            serialized schema and semantic validation
  discovery       filesystem discovery and legacy adaptation
  selection       canonical IDs, include filters, and exclusions
  plan            expanded build/run dependency graph
  process         bounded subprocess execution and timeouts
  compile         skac invocation and deterministic product checks
  execute         native process inputs, sandboxes, and observations
  expectation     byte match policies and diffs
  report          ordered human, JSON, and JUnit results
```

The crate root exposes cohesive facade types. Implementation details remain in
responsibility-named submodules rather than accumulating in one runner file.

## G2 — Specification format

**Question:** Which file format should describe collections, tests, variants,
and runs?

**Confirmed decision:** Discover `tests/golden/**/*.golden.toml`. Every spec
contains `schema = 1` and one or more `[[test]]` entries. TOML provides
multiline strings, inline tables for short runs, arrays of tables for longer
runs, familiar Cargo-adjacent syntax, and predictable typed deserialization.

YAML is excluded. Its anchors are convenient for Niflheim's repeated run
lists, but named variants remove that repetition directly. YAML's implicit
typing, aliases, permissive unknown structure, and need for a separate
normalizer do not improve Skald's contract.

The parser rejects:

- an unknown schema version;
- unknown keys at every level;
- duplicate test, variant, or run names in one ownership scope;
- an empty test or run collection;
- incompatible fields for the selected mode;
- both inline and file forms of one value;
- both UTF-8 arguments and an exact-byte argument file;
- path traversal or a canonical path outside the golden root; and
- a referenced source, input, expectation, or working directory that does not
  exist.

Schema errors report the spec path and the most specific available field path.
They exit with status 2, distinct from test failure status 1.

## G3 — Dependency policy

**Question:** May a repository tool add third-party Rust dependencies when the
current workspace has none?

**Confirmed decision:** Permit maintained TOML parsing, typed
serialization/deserialization, and machine-report encoding dependencies only
in `skald-golden`. Production compiler and documentation crates remain free of
a new dependency on the runner or its schema and reporting libraries.

Command parsing, the fixed worker pool, process management, byte comparison,
temporary-directory ownership, glob matching, and human diff rendering should
use the standard library unless implementation evidence demonstrates that a
small focused dependency materially reduces risk.

Implementing G3 changes the current repository-wide statement that the workspace
has no third-party crate dependencies. The implementation task that changes
the manifests must update the development workflow to state the narrower
production-versus-tooling boundary and must run `make msrv-check`.

A handwritten TOML subset is rejected: maintaining a second parser would cost
more and be less trustworthy than isolating a parser dependency in a
development tool.

## G4 — Fixture organization

**Question:** Should successful and failing tests remain in separate global
trees?

**Confirmed decision:** Organize new specs by feature and allow one spec to
contain run and compile-fail entries together.

The intended shape is:

```text
tests/golden/
  config.toml
  language/
    arrays/
      arrays.golden.toml
      arrays.ska
      error_optional_payload.ska
      data/
    optionals/
    objects/
  modules/
  runtime/
  std/
    io/
    strings/
  compiler/
    diagnostics/
```

Small features may keep one spec and source directly in a category directory.
A feature directory is preferred when it has helpers, data, multiple sources,
or both accepted and rejected behavior.

The spec is the discovery source of truth. Unreferenced `.ska` files are
supporting data or dead fixtures; they are never promoted implicitly into
tests.

## G5 — Build variants

**Question:** How should one program and run list be checked under multiple
compiler flag sets?

**Confirmed decision:** Define reusable named variants in
`tests/golden/config.toml`. A test selects one or more variant names. Its base
compiler arguments are followed by the selected variant's arguments and then
explicit command-line arguments supplied to the runner.

Conceptually:

```toml
schema = 1

[variant.default]
compiler_args = []

[variant.optimized]
compiler_args = ["--optimize"]
```

`default` always exists, even when it is omitted from the configuration file.
The illustrative `optimized` arguments become valid only when the compiler
implements such an option; the runner treats arguments generically and does
not define compiler flags.

A test may add base arguments:

```toml
[[test]]
name = "replacement_standard_library"
mode = "run"
source = "application/main.ska"
compiler_args = ["--stdlib-root", "replacement-sdk"]
variants = ["default"]
```

For logical module entry, a test may omit `source` and provide a complete
`compiler_args` list beginning with `--entry`. Exactly one entry-selection
form must be present after expansion. Paths in recognized path-bearing
arguments are resolved relative to the spec directory before invocation.
Unknown compiler arguments remain exact strings and are validated by `skac`.

Each `(test, variant)` pair is one independently compiled build variant. All
selected runs consume the resulting executable and share its semantic
expectations unless a later proposal introduces variant-specific output.

## G6 — Input and expectation semantics

**Question:** How are program data and expected observations represented
without losing Skald's byte contracts?

**Confirmed decision:** Every stream-like value uses one of:

```toml
stdin = { inline = "sample input\n" }
```

or:

```toml
stdin = { file = "data/full-input.bin" }
```

Inline content encodes its TOML string as UTF-8. File content is loaded as
exact bytes without newline, encoding, or zero-byte normalization.

One program with multiple inputs is expressed as:

```toml
schema = 1

[[test]]
name = "integer_division"
mode = "run"
source = "integer_division.ska"
variants = ["default", "optimized"]

[[test.run]]
name = "positive_values"
args = ["12", "3"]
expect = { exit = 0, stdout = { inline = "4\n" } }

[[test.run]]
name = "corpus"
stdin = { file = "data/division.stdin" }
expect = { exit = 0, stdout = { file = "data/division.stdout" } }
```

For run mode:

- omitted exit means exact status 0;
- omitted stdout means exact empty bytes;
- omitted stderr means exact empty bytes;
- `exit = "failure"` accepts a nonzero code or signal without freezing the
  platform mechanism;
- a present stdout or stderr expectation chooses `match = "exact"`,
  `match = "starts-with"`, or `match = "contains"`; omitted `match` means
  `exact`;
- `{ ignore = true }` must be written explicitly for an unchecked stream and
  cannot coexist with a match policy or expected data; and
- every mismatch is reported in one result rather than stopping at the first
  observation.

All three match policies remain byte-oriented:

- `exact` requires the complete actual stream to equal the expected bytes;
- `starts-with` requires the actual stream to begin with one nonempty expected
  byte sequence and permits additional trailing bytes; and
- `contains` requires one nonempty expected byte sequence to occur contiguously
  anywhere in the actual stream and permits surrounding bytes.

The match policy and expected-byte source are independent. Every policy
accepts either inline TOML text or an external byte file:

| Policy | Inline expected data | File expected data |
|---|---|---|
| Exact | `stderr = { match = "exact", inline = "..." }` | `stderr = { match = "exact", file = "expected.stderr" }` |
| Starts with | `stderr = { match = "starts-with", inline = "..." }` | `stderr = { match = "starts-with", file = "expected-prefix.stderr" }` |
| Contains | `stderr = { match = "contains", inline = "..." }` | `stderr = { match = "contains", file = "expected-fragment.stderr" }` |

The same matrix applies to native stdout, native stderr, and compile-fail
stderr. Omitting `match` is shorthand for `match = "exact"` in both the
`inline` and `file` forms. Exactly one of `inline`, `file`, or `ignore = true`
is allowed in one stream expectation.

No policy normalizes paths, whitespace, line endings, encoding, or terminal
escapes. Empty partial expectations are invalid because they would match every
stream. Regular expressions and unordered fragment sets are outside the
initial schema; stable literal fragments are easier to understand and keep
byte semantics consistent.

UTF-8 `args = [...]` covers ordinary arguments. `argv_file` retains the
current NUL-terminated exact-byte encoding for empty, whitespace, line-feed,
and non-UTF-8 Unix arguments. The two forms are mutually exclusive.

Native runtime failures should normally use `starts-with` or `contains` for a
stable panic message when later stack traces or other context are intentionally
allowed. Exact stderr remains available for cases that own the complete panic
rendering. For example:

```toml
[[test.run]]
name = "out_of_bounds"

[test.run.expect]
exit = "failure"

[test.run.expect.stderr]
match = "contains"
inline = "array index is out of bounds"
```

Compile-fail mode requires exact compiler status 1, empty compiler stdout, and
one nonempty stderr expectation. Rich compiler diagnostics should normally pin
a stable prefix or contained fragment consisting of the diagnostic identity,
primary message, and relevant primary location. Against the current legacy-path
alias mismatch diagnostic, a sufficient matcher is:

```toml
[test.expect.stderr]
match = "starts-with"
inline = """error[TYP005]: alias argument has type `Right`, expected `Left`
 --> tests/golden/compile_fail/alias_exact_type_mismatch.ska:8:13"""
```

This remains strict about the diagnostic code, primary wording, and source
location while allowing richer labels, notes, suggestions, or other context
after the matched prefix. `match = "contains"` is available when stable
context may precede the diagnostic. Exact snapshots remain appropriate for
the focused cases that intentionally verify the complete diagnostic renderer.

Runs may declare temporary input and expected output files. Each file has a
logical name restricted to safe path components. `{tmp:name}` placeholders in
arguments and stdin expand to its absolute per-run path. Input files are
written before execution; expected output files are loaded and compared after
execution. All temporary files are byte-oriented.

For example:

```toml
[[test.run]]
name = "file_round_trip"
args = ["{tmp:input}", "{tmp:output}"]
input_files = [{ name = "input", contents = { file = "data/payload.bin" } }]
expect = { exit = 0, output_files = [{ name = "output", contents = { file = "data/payload.bin" } }] }
```

A run may set `cwd = { fixture = "relative/directory" }` to use a read-only
directory below the golden root instead of its default private temporary
working directory. `env = { NAME = "value" }` declares case-specific
environment values; inherited environment remains controlled by G11.

## G7 — Execution planning and parallelism

**Question:** What is the unit of concurrency?

**Confirmed decision:** Discovery and validation produce an immutable expanded
plan. The plan contains dependency nodes rather than one opaque task per spec:

```text
runtime preparation ───────────────────────────────┐
                                                   v
test + variant ─> compile observations ─> link executable
                                              ├─> run A observations
                                              ├─> run B observations
                                              └─> run C observations

compile-fail test + variant ─> diagnostic observations
```

Independent nodes enter one bounded fixed worker pool. `--jobs N` controls the
maximum number of simultaneously active external processes; its default is
the host's available parallelism. `--jobs 1` provides a deterministic
single-worker debugging mode.

The initial scheduler does not need general work stealing. It needs:

- a stable queue of ready node IDs;
- dependency counts and dependent-node lists;
- one result channel from workers;
- cancellation of dependent nodes after a prerequisite failure;
- continued execution of unrelated nodes by default; and
- optional `--fail-fast` cancellation before new unrelated nodes start.

One spec may therefore compile several sources concurrently, and independent
runs of one executable may execute concurrently. A shared executable is
read-only.

## G8 — Runtime and link ownership

**Question:** How should native cases avoid rebuilding runtime code and avoid a
third successful compiler invocation solely for linking?

**Confirmed decision:** After selection, the runner prepares the runtime archive
exactly once if and only if the plan contains native run nodes. It invokes the
repository's ordinary runtime Make target and validates the expected archive.
Compile-fail-only selections never build the runtime.

For a successful build variant, the compiler emits assembly in two independent
processes when compile determinism is enabled. After the assembly bytes match,
the runner links the first product to the already-built runtime archive through
the compiler crate's public `Toolchain` owner. This reduces the current three
successful compiler invocations to two without duplicating C compiler or
runtime-archive policy in the runner.

Focused `skac` binary integration tests continue to own direct executable
artifact creation through the CLI. Golden tests own source-to-assembly,
linkage through the same toolchain owner, and native observation.

If implementation evidence shows that a runner dependency on `skald-compiler`
is untenable, stop and amend this design explicitly rather than silently
retaining a third `skac -o <executable>` process or duplicating the link command
in `skald-golden`.

## G9 — Determinism modes

**Question:** Which determinism policy belongs in ordinary runs, and how are
the repeated-process checks retained?

**Confirmed decision:** Support three explicit modes:

| Mode | Successful compilation | Compile failure | Native run |
|---|---|---|---|
| `full` | Emit twice and compare | Compile twice and compare | Execute twice and compare |
| `compile` | Emit twice and compare | Compile twice and compare | Execute once |
| `off` | Emit once | Compile once | Execute once |

The runner defaults to `off`. `make golden-test` and the ordinary `make check`
gate use that default: each build or run is observed once. This makes the
common command primarily a semantic and integration test rather than a
reproducibility audit.

`--determinism compile` and `--determinism full` opt into repeated
observations. The Makefile provides `make golden-determinism-test` as the
convenient complete `full` audit. External automation may schedule that target
without adding repository CI configuration.

Recommended selection is:

- use `off` for ordinary focused and complete development runs;
- use `compile` after substantial frontend, diagnostic, IR, optimization,
  code-generation, discovery-order, or compiler-concurrency changes; and
- use `full` after substantial runtime, linking, process-I/O, runner
  scheduling, or isolation changes, at release checkpoints, and in periodic
  external automation.

The mode is always printed in the run header and summary. No environment
variable or host detection silently promotes or demotes the selected policy.
The current always-repeated behavior therefore remains available and tested,
but no longer taxes every ordinary golden run.

When repeated observations disagree, the case fails before snapshot matching
and reports both observations. Each repetition gets a fresh process and fresh
runtime temporary directory.

## G10 — Filtering and identities

**Question:** How can contributors select a useful scope unambiguously?

**Confirmed decision:** Every leaf has a canonical ID:

```text
<spec path without .golden.toml>::<test>::<variant>::<run>
```

Compile-fail leaves end in `::<compile>` rather than a run name. For example:

```text
language/integers/division::integer_division::optimized::corpus
language/integers/division::invalid_division_types::default::<compile>
```

`--filter <glob>` is repeatable and unions its matches. `--exclude <glob>` is
repeatable and subtracts matches. Globs match canonical IDs, spec-relative
paths, and referenced source-relative paths. `*` stays within one path or ID
component; `**` crosses components. `--exact <id>` selects one exact leaf.

Additional discovery commands are:

- `--list` for canonical leaf IDs;
- `--list-tests` for source test and build-variant IDs; and
- `--explain <id>` for resolved source, arguments, variant flags, inputs,
  expectations, dependencies, working directory, and artifact path.

A nonempty filter set that matches no leaf exits with status 2. An explicit
`--allow-empty` supports scripts that intentionally tolerate an empty
platform selection.

Filtering happens after complete schema validation but before runtime
preparation or process execution. A malformed unselected spec therefore still
fails discovery rather than silently rotting.

## G11 — Process isolation and timeouts

**Question:** How are parallel cases kept hermetic and bounded?

**Confirmed decision:** Every build variant receives a unique directory below
`build/golden/cases/`. Its readable prefix derives from the canonical build ID
and its suffix is a stable hash of the complete ID. Flattened names alone are
insufficient because path separators and legal underscores can collide.

Every new-format run receives a private temporary directory below
`build/golden/tmp/`. Its default working directory is that private directory.
Fixture paths should be passed through resolved arguments or temporary input
files. A run may request a read-only working directory below the golden root
for programs whose behavior genuinely depends on relative fixture lookup.

Writing into a shared fixture directory is rejected by default. A later case
that genuinely needs a shared external resource must declare a named resource
lock or `serial = true`; the scheduler holds that lock for the complete run.

Legacy cases retain their current working directories during migration and
are conservatively serialized by working-directory resource when a case may
write there. Migration should move writable behavior to private temporary
paths before enabling run-level concurrency.

The default timeout applies separately to each compiler, linker, and generated
program process. Tests may request a longer bounded timeout. Timeout handling
terminates the child, closes owned pipes, collects available output, and
reports the elapsed limit. On Linux, process-group termination prevents a
timed-out case from leaving descendants running.

Environment inheritance is allowlisted. Toolchain variables required by the
compiler remain available; case-specific environment values must be declared
in the spec. This prevents ambient test-order dependencies.

## G12 — Migration boundary

**Question:** How can 288 cases move without one risky fixture rewrite?

**Confirmed decision:** The first runner version includes a legacy loader that
maps the existing tree into the new internal model:

- one ordinary `run/**/*.ska` fixture becomes one test, the implicit `default`
  variant, and one run;
- one `compile_fail/**/*.ska` fixture becomes one compile-fail test;
- a `case.args` directory remains one source test and supporting `.ska` files
  remain undiscovered;
- existing `.argv`, `.stdin`, `.stdout`, `.stderr`, and `.exit` meanings remain
  unchanged; and
- legacy canonical IDs derive from current relative expectation stems.

New-format specs live outside the legacy `run` and `compile_fail` roots during
migration. This prevents duplicate discovery. Feature migration moves both
successful and failing fixtures into a feature directory and adds one
authoritative spec in the same change.

Migration should proceed in these semantic groups:

1. representative simple native and compile-fail fixtures;
2. exact stdin, stdout, stderr, and non-UTF-8 argument fixtures;
3. multiple-file entry, module-root, and replacement-standard-library cases;
4. runtime failures and working-directory file inputs;
5. features that benefit immediately from multiple runs or build variants;
6. the remaining language, runtime, standard-library, and diagnostic corpus;
   and
7. removal of legacy discovery, old sidecar-only documentation, and the old
   runner after the legacy count reaches zero.

Each migration must preserve behavior before combining cases. Similar sources
should share one compiled selector program only when the resulting source and
run list remain easier to understand than separate focused programs.
Legacy stderr sidecars continue to compare exactly until their owning feature
is migrated. Migration may deliberately shorten one full snapshot to a stable
`starts-with` or `contains` expectation after reviewing which diagnostic or
runtime-error portion the case actually owns.

## Command-line interface

The intended ordinary interface is:

```text
make golden-test
```

The Makefile builds `skac` and the runner, then invokes the Rust runner with
its default `off` determinism policy. A thin `scripts/golden.sh` may offer
direct argument forwarding, but it must not become the only entry point.

The complete reproducibility audit is:

```text
make golden-determinism-test
```

Representative focused forms are:

```text
scripts/golden.sh --filter 'language/arrays/**'
scripts/golden.sh --filter '**::optimized::*' --jobs 8
scripts/golden.sh --exact 'std/io/read::binary::default::embedded-zero'
scripts/golden.sh --list --filter 'modules/**'
scripts/golden.sh --determinism compile --filter 'language/strings/**'
```

The complete confirmed surface is:

| Option | Meaning |
|---|---|
| `--jobs N` | Maximum active external processes |
| `--filter GLOB` | Include matching leaves; repeatable |
| `--exclude GLOB` | Exclude matching leaves; repeatable |
| `--exact ID` | Select one canonical leaf |
| `--variant NAME` | Restrict selected build variants; repeatable |
| `--compiler PATH` | Override the `skac` executable |
| `--compiler-arg ARG` | Append one compiler argument to selected builds |
| `--determinism MODE` | Select `off` (default), `compile`, or `full` |
| `--timeout SECONDS` | Override the default per-process timeout |
| `--fail-fast` | Stop scheduling unrelated work after the first failure |
| `--list` | Print canonical leaf IDs without execution |
| `--list-tests` | Print source/build IDs without execution |
| `--explain ID` | Print the fully resolved plan for one leaf |
| `--show-output` | Show captured output for passing cases too |
| `--slowest N` | Report the slowest completed leaves |
| `--format FORMAT` | Select `human`, `json`, or `junit` reporting |
| `--allow-empty` | Permit an empty selection |

Expectation update or `--bless` behavior is intentionally deferred until the
read-only runner and migration are stable. When designed, ordinary execution
must remain read-only and updates must be explicit, reviewable, and limited to
declared external snapshots.

## Reporting

Workers never print directly. They return structured results to the
coordinator, which may show concise live progress on a terminal but emits final
results in canonical ID order.

The default successful summary distinguishes ownership levels. For example:

```text
golden: 42 specs, 176 tests, 138 compile-fail builds, 214 run builds, 691 runs passed
golden: 352 compilations, 214 links, 691 executions; 9.21s
```

Counts reflect the selected determinism mode and do not label compile-fail
entries as runtime runs. Failure output includes:

- canonical leaf ID;
- failing stage and command with safely escaped arguments;
- working and artifact directories;
- exit code, signal, or timeout;
- independently observed mismatches;
- exact byte lengths;
- the selected stream match policy and, for containment, the matching byte
  offset;
- readable escaped bytes for binary data;
- bounded unified text diffs when both values are UTF-8; and
- artifact retention instructions.

Temporary directories for passing runs are deleted. Failed-run sandboxes and
build products remain by default for inspection and are removed by the next
ordinary clean operation. `--keep-all-artifacts` may retain passing sandboxes
for debugging.

JSON and JUnit contain the same canonical IDs, stages, durations, status, and
failure details as the human model. Reporting format never changes test
semantics or scheduling.

## Runner verification strategy

The runner library owns focused Rust tests for:

- every valid and invalid schema form;
- unknown fields, duplicate names, incompatible unions, and version errors;
- canonical and symlink-based path escape attempts;
- inline and external exact bytes, including NUL and non-UTF-8 data;
- exact-byte Unix argument decoding;
- canonical IDs, stable artifact hashing, and deliberate collision inputs;
- include, exclude, exact, variant, empty, and explanation selection;
- variant and run expansion without duplicate or missing leaves;
- dependency readiness, bounded concurrency, failure cancellation, fail-fast,
  and resource locks;
- compiler, linker, and generated-program timeout behavior;
- large stdin with simultaneous stdout and stderr collection;
- deterministic result ordering under deliberately reversed completion order;
- exact, starts-with, contains, ignored, failure, signal, and output-file
  expectations, including rejection of empty partial matches;
- text and binary mismatch presentation;
- all determinism levels; and
- legacy discovery and sidecar parity.

Process tests should use small fake compiler, linker, and executable modes
provided by a Rust test helper rather than depending on shell behavior.

Repository-level acceptance includes:

- the legacy adapter discovers exactly 150 native and 138 compile-fail cases at
  the recorded baseline;
- old and new runners agree on every case in `full` mode before the Makefile
  switches;
- `--jobs 1` and the default parallel mode produce identical ordered semantic
  results;
- the complete current golden suite passes in the default `off` mode;
- the complete current golden suite passes in `full` determinism mode;
- `make golden-test`, `make golden-determinism-test`, `make check`,
  `make msrv-check`, and `git diff --check` pass when their responsible
  implementation tasks complete; and
- documentation links and command inventories remain valid.

The performance goals, not cross-host correctness gates, are a warm default
run of at most 15 seconds and a warm `full` run of at most 30 seconds on the
16-logical-CPU audit host. Performance reporting should also record compiler,
linker, and run stage totals so later regressions have an owner.

## Promotion result and implementation boundary

Review confirmed G1 through G12 on 2026-08-05, including:

- TOML rather than YAML;
- the narrowly scoped third-party dependency exception;
- linking checked assembly through `skald_compiler::driver::Toolchain`;
- `off` determinism for ordinary runner, Makefile, and `make check` execution,
  with a dedicated `full` Makefile audit target;
- strict empty-by-default stream semantics;
- explicit exact, starts-with, and contains stderr matching for native and
  compile-fail cases;
- per-run temporary working directories for new fixtures; and
- a legacy adapter rather than a big-bang migration.

Freezing this design did not itself make it implemented. The completed
[golden test runner roadmap](GOLDEN_TEST_RUNNER_ROADMAP.md) divided delivery by
stable boundaries: typed model and parser, selection and planning, process
execution, runtime/link integration, parallel scheduling, reporting, legacy
parity, Makefile cutover, feature-group migrations, and legacy removal.
Each task keeps the complete suite runnable and updates living testing
documentation when its user-visible contract changes.

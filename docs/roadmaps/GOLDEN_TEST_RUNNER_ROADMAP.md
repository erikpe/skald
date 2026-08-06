# Spec-Driven Parallel Golden Test Runner Roadmap

Status: in progress; GR4 is next.

This roadmap implements the frozen
[golden test runner design](../archive/GOLDEN_TEST_RUNNER_DESIGN_PROPOSAL.md).
It replaces the serial source-discovered harness with a Rust workspace tool,
retains every current fixture through a compatibility boundary, cuts repository
commands over only after behavioral parity, and then migrates the corpus into
feature-oriented TOML specs.

Current behavior remains authoritative in
[`tests/golden/README.md`](../../tests/golden/README.md) until the responsible
tasks below implement and document each new contract. The design record owns
G1 through G12; this roadmap owns delivery and must not reopen those decisions
implicitly.

## Scope and invariants

- Add a separate `skald-golden` workspace package with a reusable library and
  thin binary.
- Invoke the real `skac` executable for every compiler observation.
- Parse strict, versioned `*.golden.toml` specs and repository build variants.
- Preserve byte-exact inputs, non-UTF-8 Unix arguments, strict empty stream
  defaults, and exact, starts-with, and contains output matching from inline or
  external expected data.
- Build the runtime once when selected native cases need it and link emitted
  assembly through `skald_compiler::driver::Toolchain`.
- Default ordinary execution to determinism `off`; retain explicit `compile`
  and `full` audits and a full-audit Make target.
- Schedule independent compilation, linking, and execution nodes concurrently
  under one bounded process budget.
- Keep result meaning and final reporting deterministic regardless of process
  completion order.
- Preserve all 150 native and 138 compile-fail legacy cases at the recorded
  baseline until their owning fixtures migrate.
- Keep every intermediate revision runnable; do not require a big-bang fixture
  move.
- Keep third-party schema and report dependencies private to repository
  tooling. Production compiler and generated-program dependencies do not
  change.
- Keep the root Makefile as the shared local and external automation boundary;
  do not add repository CI configuration.
- Do not change the Skald language, compiler semantics, runtime ABI, diagnostic
  renderer, or test ownership between unit, integration, golden, and runtime
  layers.
- Do not add cross-invocation compilation caching, distributed execution,
  regex output matching, implicit snapshot updates, or a general build-system
  abstraction.

## Progress

- [x] GR0 — Establish the Rust tool and frozen schema
- [x] GR1 — Discover, validate, expand, and select spec cases
- [x] GR2 — Implement byte expectations and isolated process execution
- [x] GR3 — Compile, link, and execute sequential plans
- [ ] GR4 — Schedule the dependency graph in parallel
- [ ] GR5 — Complete reporting and the command-line surface
- [ ] GR6 — Adapt legacy fixtures and prove behavioral parity
- [ ] GR7 — Cut repository commands over to the Rust runner
- [ ] GR8 — Migrate process, I/O, panic, and runtime-observation fixtures
- [ ] GR9 — Migrate module and replacement-standard-library fixtures
- [ ] GR10 — Migrate primitive, operator, call, and control-flow fixtures
- [ ] GR11 — Migrate class, object, alias, and polymorphism fixtures
- [ ] GR12 — Migrate array, optional, and shared-ownership fixtures
- [ ] GR13 — Migrate remaining diagnostics and remove legacy discovery
- [ ] GR14 — Harden, validate, document, and close

## PR-sized implementation sequence

### GR0 — Establish the Rust tool and frozen schema

**Purpose:** Create the isolated repository-tool boundary and encode the frozen
TOML contract before discovery or process behavior depends on it.

- [x] Add `crates/skald-golden` to the workspace with a library, thin binary,
      workspace lints, and Rust 1.82-compatible syntax.
- [x] Establish facade-oriented modules for CLI, spec, discovery, selection,
      plan, process, compile, execute, expectation, and report responsibilities;
      leave unimplemented owners narrow rather than placing logic in `main.rs`.
- [x] Add maintained TOML parsing and typed serialization/deserialization
      dependencies confined to the runner package and record the complete
      locked dependency graph.
- [x] Represent schema version 1, repository variants, run and compile-fail
      tests, compiler arguments, named runs, byte sources, stream match modes,
      temporary files, working directories, environment entries, timeouts,
      serial/resource declarations, and expectations as typed data.
- [x] Reject unknown keys through typed deserialization and validate duplicate
      names, empty collections, mode-incompatible fields, mutually exclusive
      unions, missing expected data, and empty partial matchers.
- [x] Parse every TOML example from the frozen design as a focused test and add
      malformed counterparts for every schema boundary.
- [x] Update the development workflow's dependency statement to distinguish
      dependency-free production crates from narrowly scoped repository-tool
      dependencies.

**Tests:** `cargo test --locked -p skald-golden` for schema parsing and
validation; `cargo fmt --all -- --check`; workspace Clippy with warnings denied;
`make docs-check`; `make msrv-check`; and `git diff --check`.

**Exit criteria:** The new package builds on stable and Rust 1.82, valid frozen
examples deserialize into typed values, every invalid schema shape fails with
a spec and field path, production crates do not depend on the runner, and no
external process is executed yet.

### GR1 — Discover, validate, expand, and select spec cases

**Purpose:** Turn validated specs into stable executable identities before
process execution or concurrency is introduced.

- [x] Discover `tests/golden/**/*.golden.toml` in deterministic path order and
      load `tests/golden/config.toml` separately from discovered specs.
- [x] Canonicalize every source, data file, expected file, and fixture working
      directory; reject lexical and symlink escapes from the golden root.
- [x] Resolve source shorthand and recognized path-bearing compiler arguments
      relative to the spec directory while leaving unknown compiler arguments
      byte-for-byte unchanged for `skac` validation.
- [x] Expand test base arguments, repository variants, command-line arguments,
      named runs, and compile leaves into one immutable plan model.
- [x] Assign stable spec, test, build, and leaf IDs and collision-resistant
      artifact directory names with readable prefixes and stable full-ID
      hashes.
- [x] Implement repeatable include and exclude globs, exact leaf selection,
      variant restriction, empty-selection rejection, and explicit
      `--allow-empty`.
- [x] Implement read-only `--list`, `--list-tests`, and `--explain` operations
      over the resolved plan without preparing runtime or starting processes.
- [x] Validate all discovered specs before applying filters so malformed
      unselected fixtures cannot rot silently.

**Tests:** Focused discovery trees covering deterministic order, nested specs,
duplicate IDs, deliberate flattened-name collisions, missing paths, lexical
and symlink escapes, logical entry arguments, every glob form, exact selection,
variant restriction, empty selection, and stable explanation output; runner
unit tests; formatting; Clippy; MSRV; docs check; and diff hygiene.

**Exit criteria:** A synthetic golden tree expands into the expected stable
leaf IDs and artifact paths, every selection operation is deterministic and
side-effect free, and invalid paths or schemas fail before selection.

### GR2 — Implement byte expectations and isolated process execution

**Purpose:** Establish one reliable subprocess and byte-observation substrate
before compiler-specific stages or parallel scheduling use it.

- [x] Implement inline UTF-8 and external exact-byte loading with no newline,
      encoding, zero-byte, or terminal-escape normalization.
- [x] Implement exact, starts-with, contains, and ignored stream policies;
      preserve strict empty defaults and reject empty partial fragments.
- [x] Decode the existing NUL-terminated exact-byte Unix argument format and
      retain whitespace, line-feed, empty, and non-UTF-8 arguments.
- [x] Create private per-run temporary directories, write declared byte input
      files, substitute `{tmp:name}` paths in arguments and stdin, and compare
      declared byte output files after execution.
- [x] Support the default private working directory and explicit read-only
      fixture working directories below the golden root.
- [x] Construct an allowlisted inherited environment plus declared per-case
      values without ambient test-order state.
- [x] Feed stdin concurrently while capturing stdout and stderr so input or
      output beyond host pipe capacity cannot deadlock.
- [x] Enforce a separate timeout for each child process, terminate its Linux
      process group, close owned pipes, collect available observations, and
      report timeout distinctly from code or signal termination.
- [x] Add a Rust fake-process test helper with modes for arguments, binary
      streams, large pipes, output files, sleep, signals, descendants, and
      controlled failures.

**Tests:** Runner tests for every byte-source/matcher combination on stdout and
stderr, empty partial rejection, binary arguments and streams, temporary input
and output files, cleanup and retention, working directories, environment
allowlisting, large-pipe progress, exact code/signal/failure observations,
timeouts, and descendant termination; formatting; Clippy; MSRV; and diff
hygiene.

**Exit criteria:** The process layer can run arbitrary fixture executables
hermetically, returns complete structured observations without deadlock, and
compares every frozen input and expectation policy without compiler knowledge.

### GR3 — Compile, link, and execute sequential plans

**Purpose:** Compose the typed plan and process substrate into a complete
sequential golden runner before introducing concurrency.

- [x] Locate a sibling `skac` by default, accept `--compiler`, and fail clearly
      when the selected executable is absent or unusable.
- [x] Construct real compiler commands for positional sources and logical
      entries from base, variant, and command-line compiler arguments.
- [x] Implement compile-fail execution with exact compiler status 1, empty
      stdout, selected stderr matching, and deterministic diagnostic comparison
      in `compile` and `full` modes.
- [x] Emit successful assembly once in `off` mode and twice in `compile` or
      `full` mode; reject unexpected compiler output and compare repeated
      assembly bytes before linkage.
- [x] Prepare the runtime through the repository Make target exactly once and
      only when the selected plan contains native runs.
- [x] Link the first emitted assembly through an explicitly configured
      `skald_compiler::driver::Toolchain` and the prepared runtime archive;
      retain atomic artifact publication and captured linker failures.
- [x] Execute each selected run once in `off` and `compile` modes or twice in
      `full`; compare repeated exit, stdout, stderr, and output-file
      observations before expectation matching.
- [x] Retain unique build products and failed-run sandboxes under
      `build/golden/` while deleting passing temporary run directories.
- [x] Execute the complete dependency plan sequentially and cancel only nodes
      whose prerequisite failed.

**Tests:** Fake-compiler tests for argument ordering, compile success/failure,
unexpected output, deterministic and nondeterministic assembly/diagnostics,
missing runtime, runtime-once behavior, linker failure, each determinism mode,
run cancellation, and artifact publication; focused real `skac` run and
compile-fail cases; runner unit tests; formatting; Clippy; MSRV; and diff
hygiene.

**Exit criteria:** New-format specs compile through real process boundaries,
link the exact selected assembly, execute all named runs sequentially, and
produce correct structured results for every determinism mode.

### GR4 — Schedule the dependency graph in parallel

**Purpose:** Reduce elapsed time without changing leaf meaning, artifact
ownership, failure propagation, or final result ordering.

- [ ] Implement a fixed worker pool, stable ready queue, dependency counts,
      dependent-node lists, and one structured result channel to the
      coordinator.
- [ ] Bound all compiler, linker, and generated-program processes under the
      single `--jobs` limit and default it to host available parallelism.
- [ ] Allow independent sources in one spec and independent runs of one linked
      executable to proceed concurrently.
- [ ] Cancel dependent work after prerequisite failure while continuing
      unrelated work by default.
- [ ] Implement `--fail-fast` so no new unrelated work starts after the first
      observed failure while already active processes finish or time out.
- [ ] Implement named resource locks and `serial = true` without holding
      scheduler-global locks during external process execution.
- [ ] Keep workers silent and collect results independently of completion
      order for deterministic coordinator reporting.
- [ ] Convert worker panic or channel failure into an internal runner failure
      with active and pending node IDs rather than hanging.

**Tests:** Deterministic fake workloads that prove the process bound, dependency
readiness, run-level overlap, resource exclusion, serial execution,
prerequisite cancellation, unrelated continuation, fail-fast, reverse
completion order, worker failure, and semantic equivalence between `--jobs 1`
and parallel execution; runner unit tests; formatting; Clippy; MSRV; and diff
hygiene.

**Exit criteria:** Parallel and single-worker plans return identical ordered
semantic results, no run or build artifact collides, and the coordinator cannot
deadlock when work fails, times out, or panics.

### GR5 — Complete reporting and the command-line surface

**Purpose:** Make the runner practical for focused development, external
automation, and failure diagnosis before it becomes the repository default.

- [ ] Complete command parsing and help for jobs, include/exclude/exact
      selection, variants, compiler and compiler arguments, determinism,
      timeout, fail-fast, list/explain, output display, slowest results, report
      format, empty selection, and artifact retention.
- [ ] Print the selected determinism mode and resolved counts before execution.
- [ ] Emit final human results in canonical ID order with stage, safely escaped
      command, working/artifact directories, status or signal, timeout, all
      mismatches, byte lengths, match policy and offset, binary escaping, and
      bounded UTF-8 unified diffs.
- [ ] Report spec, source test, compile-fail build, successful build, named run,
      compiler process, link, execution, failure, cancellation, and duration
      counts without conflating them.
- [ ] Record stage durations and implement `--slowest N` over stable leaf IDs.
- [ ] Add narrowly scoped machine-report encoding dependencies if needed and
      emit JSON and JUnit with the same IDs, stages, durations, status, and
      failure details as the human model.
- [ ] Keep ordinary execution read-only; do not add blessing or implicit
      expectation updates.
- [ ] Test broken-pipe behavior so piping list or report output to an early
      consumer exit does not produce a misleading test failure.

**Tests:** Golden-runner unit and snapshot tests for help, every option
combination, ordered success/failure output, binary and text diffs, truncation,
all summary counts, timings, JSON decoding, JUnit parsing, artifact retention,
broken pipes, and stable reports under randomized completion order; formatting;
Clippy; MSRV; docs check; and diff hygiene.

**Exit criteria:** Contributors can select, run, understand, and machine-parse
new-format cases through the complete frozen command surface, and all formats
describe identical results.

### GR6 — Adapt legacy fixtures and prove behavioral parity

**Purpose:** Make the new engine cover the entire existing corpus without
moving fixtures, then prove it is safe to replace the old runner.

- [ ] Implement legacy discovery below `run` and `compile_fail`, including the
      `case.args` recursion stop and supporting-source exclusion.
- [ ] Translate `.exit`, `.argv`, `.stdin`, `.stdout`, and `.stderr` sidecars
      into the new internal model without changing byte, missing-file, status,
      compiler-argument, working-directory, or path-prefix semantics.
- [ ] Derive stable legacy IDs and collision-resistant artifacts from current
      expectation stems.
- [ ] Apply conservative working-directory resource locks wherever a legacy
      case cannot be proven read-only; do not expose old fixtures to new races.
- [ ] Move current native expectation loader and mismatch tests into the new
      expectation owner or replace them with equivalent stronger coverage.
- [ ] Assert the recorded baseline of exactly 150 native and 138 compile-fail
      legacy cases before migration begins.
- [ ] Run old and new runners in `full` mode over the complete corpus and
      compare pass/fail outcome, repeated assembly or diagnostic checks,
      process observations, and summary ownership.
- [ ] Resolve every discrepancy in the new runner or record an explicit design
      amendment; do not normalize parity failures away.

**Tests:** Legacy loader unit tests, all current expectation tests, targeted
single- and multiple-file fixtures, complete old/new `full` parity, parallel
versus single-worker equivalence, runner test suite, formatting, Clippy, MSRV,
docs check, and diff hygiene.

**Exit criteria:** The Rust runner discovers 288 legacy cases, passes the
complete current corpus with the same full-determinism observations as the old
runner, and no fixture has moved or changed meaning.

### GR7 — Cut repository commands over to the Rust runner

**Purpose:** Make the parity-proven runner the ordinary repository interface
while retaining the legacy loader for incremental fixture migration.

- [ ] Update the Makefile to build `skac` and `skald-golden`, run ordinary
      goldens with determinism `off`, include that target in `make check`, and
      expose `make golden-determinism-test` for the `full` audit.
- [ ] Add a thin `scripts/golden.sh` argument-forwarding convenience wrapper
      while keeping every validation responsibility available through Make.
- [ ] Update `make help` with ordinary, expectation-focused, filtered, and
      determinism-audit commands without retaining stale Cargo-test syntax.
- [ ] Delete the old harness-free runner and its superseded expectation test
      target only after their coverage is owned by `skald-golden`.
- [ ] Update the living development workflow, testing guide, golden fixture
      guide, scripts guide, and driver/toolchain references for the implemented
      runner, default determinism policy, filtering, artifacts, and legacy
      compatibility boundary.
- [ ] Measure warm default and `full` elapsed time and stage counts on the
      16-logical-CPU audit host; meet or explain deviations from the 15-second
      default and 30-second full goals before cutover.
- [ ] Confirm compile-fail-only filtered execution does not build the runtime
      and every selected native execution shares one runtime build.

**Tests:** `cargo test --locked -p skald-golden`; representative filtered and
exact commands through the script and Makefile; complete default and full
goldens; `make help`; `make check`; `make msrv-check`; `git diff --check`; and
documentation validation.

**Exit criteria:** `make golden-test` and `make check` use the Rust runner in
default `off` mode, `make golden-determinism-test` passes in `full`, the old
runner is gone, all 288 legacy fixtures remain covered, and living docs describe
only implemented interfaces.

### GR8 — Migrate process, I/O, panic, and runtime-observation fixtures

**Purpose:** Exercise the new data, partial-stderr, temporary-file, and native
process model on the fixtures that benefit most before broad mechanical moves.

- [ ] Create feature-oriented specs for process arguments, standard I/O,
      primitive printing/parsing/formatting corpora, explicit panic behavior,
      allocation and bounds failures, and standard-test assertion failures.
- [ ] Preserve exact binary stdin, stdout, argument, empty-stream, status, and
      signal observations through inline or external byte sources as
      appropriate.
- [ ] Replace full native stderr snapshots with reviewed starts-with or contains
      fragments where the case owns a stable panic message but should permit
      later stack traces or richer context; retain exact mode when complete
      rendering is the assertion.
- [ ] Move writable file behavior to named per-run temporary paths and express
      read-only fixture working directories explicitly.
- [ ] Consolidate multiple data selections under one compiled source only when
      named runs make the source and expectations clearer; retain focused
      programs otherwise.
- [ ] Record the legacy-to-spec leaf mapping and assert that no observation is
      lost or discovered twice.
- [ ] Update feature-local fixture documentation and filtering examples in the
      same change.

**Tests:** Filters for each migrated process/runtime/std group in default and
targeted full mode; exact binary argument and stream tests; large-input and
temporary-file cases; legacy count delta and no-duplicate assertions; runner
tests; `make golden-test`; `make check`; docs check; and diff hygiene.

**Exit criteria:** The migrated process-facing corpus uses authoritative TOML
specs, exercises inline and file data plus partial native stderr matching,
leaves no corresponding legacy case, and retains every prior observation.

### GR9 — Migrate module and replacement-standard-library fixtures

**Purpose:** Replace `case.args` manifests with typed feature specs while
preserving hermetic multiple-file provider and diagnostic-path behavior.

- [ ] Migrate successful and failing module entry, root, import, cycle,
      visibility, provider-collision, positional-entry, no-standard-library,
      replacement-standard-library, static-field module, and I/O intrinsic
      fixture directories.
- [ ] Express positional source or logical entry, module roots, standard-library
      selection, and compiler arguments in the frozen spec model.
- [ ] Keep supporting `.ska` files undiscovered and every provider tree below
      its feature directory.
- [ ] Preserve relative rendered diagnostic paths and choose reviewed exact,
      starts-with, or contains stderr policies according to each case's owner.
- [ ] Move and repair compiler unit-test `include_str!` references that reuse
      module-cycle golden sources without creating a production dependency on
      the golden tree.
- [ ] Remove each migrated `case.args` and sidecar set only after its spec leaf
      passes and the legacy count falls by the corresponding case.
- [ ] Audit canonical paths and symlink containment for every moved provider
      root.

**Tests:** Filtered module and replacement-standard-library specs in default,
compile-determinism, and single-worker modes; affected resolver/compiler tests;
legacy count delta and supporting-file non-discovery tests; `make golden-test`;
`make check`; docs check; and diff hygiene.

**Exit criteria:** No module-owned fixture depends on `case.args`, all supporting
sources remain hermetic and undiscovered, diagnostics retain intentional path
spelling, and affected compiler tests reference their new stable owners.

### GR10 — Migrate primitive, operator, call, and control-flow fixtures

**Purpose:** Reorganize the broad value-and-control corpus by language feature
and use named runs where they remove redundant compilation without obscuring
the source behavior.

- [ ] Migrate primitive literals, conversions, arithmetic, comparisons,
      bitwise operations, shifts, division, remainder, booleans, locals,
      reassignment, direct and nested calls, register/stack arguments,
      conditionals, loops, break, continue, and short-circuit fixtures.
- [ ] Place accepted and rejected cases together under feature-oriented specs
      while keeping compile-fail sources focused on one diagnostic owner.
- [ ] Use external corpus data for large byte expectations and inline data for
      short readable runs.
- [ ] Convert rich compile diagnostics to stable prefix or contained fragments
      where full renderer ownership is unnecessary; retain exact snapshots for
      intentional renderer coverage.
- [ ] Use named runs for meaningful data matrices and preserve separate sources
      for distinct evaluation-order, cleanup, ABI-pressure, or failure paths.
- [ ] Exercise repository build variants only with compiler flags that actually
      exist at migration time; do not invent optimization behavior to satisfy
      the runner design.
- [ ] Record the legacy-to-spec mapping and remove migrated legacy sidecars only
      after filtered parity.

**Tests:** Feature filters in default mode, compile-determinism filters for
diagnostic and code-generation-sensitive groups, representative full runs for
effectful control flow, affected compiler/backend suites, legacy count deltas,
`make golden-test`, `make check`, docs check, and diff hygiene.

**Exit criteria:** Primitive and control-flow goldens are feature-owned specs,
repeated data cases compile once where appropriate, focused semantic cases stay
separate, and all previous observations remain represented.

### GR11 — Migrate class, object, alias, and polymorphism fixtures

**Purpose:** Move object-model coverage without weakening lifecycle,
evaluation-order, ABI, privacy, dispatch, or diagnostic observations.

- [ ] Migrate inline and class object construction, fields, nested values,
      initializers, explicit copy construction, object parameters/results,
      temporaries, destruction, private/static members, inheritance,
      polymorphism, virtual behavior, object casts, and checked failures.
- [ ] Migrate alias parameter, produced-alias, primitive-alias, exact-type,
      access, mutability, external, and misuse coverage alongside the feature
      that owns each source behavior.
- [ ] Preserve exact stdout lifecycle traces, process status, evaluation order,
      and failure-before-later-effect observations.
- [ ] Use stable partial compile-fail stderr expectations for diagnostic code,
      primary message, and primary location unless a case intentionally owns
      the complete multi-label renderer.
- [ ] Keep source programs separate when combining them would hide ownership,
      cleanup, register pressure, or dynamic dispatch boundaries.
- [ ] Record mapping, filtered parity, and legacy count changes before deleting
      each old fixture set.

**Tests:** Feature filters in default and compile-determinism modes; full mode
for lifecycle and failure-order groups; affected type-check, HIR, MIR,
verifier, backend, and CLI tests; legacy count deltas; `make golden-test`;
`make check`; docs check; and diff hygiene.

**Exit criteria:** Object-model and alias goldens are organized by feature,
their exact lifecycle and dynamic observations are unchanged, rich diagnostics
pin only their owned stable portions, and no migrated legacy fixture remains.

### GR12 — Migrate array, optional, and shared-ownership fixtures

**Purpose:** Move the resource-heavy corpus while preserving cleanup,
allocation, anchor, cycle, bounds, ABI-pressure, and failure-order coverage.

- [ ] Migrate primitive and object arrays, inline and shared array storage,
      indexing, slicing, aliases, allocation and length failures, and array ABI
      pressure.
- [ ] Migrate primitive, inline, shared, pinned, alias, presence, unwrap,
      conversion, lifecycle, and containment optional coverage.
- [ ] Migrate shared calls/results, fields, owners, casts, checked places,
      allocation, exact lifetime, outer arrays, element ownership,
      polymorphism, and strong-cycle coverage.
- [ ] Preserve exact lifecycle stdout, panic identity, status/failure,
      skipped-effect, all-success cleanup, and failure-before-cleanup
      observations.
- [ ] Use partial native stderr for stable panic messages that may gain stack
      traces and partial compile stderr for owned diagnostic identity/message/
      location; retain exact mode where full output is intentional.
- [ ] Keep ABI-pressure, graph, lifecycle-order, and selected-path sources
      focused rather than over-consolidating them into selector programs.
- [ ] Record mapping, filtered parity, and legacy count changes before deleting
      each old fixture set.

**Tests:** Array, optional, and shared filters in default mode; compile mode for
diagnostic/code-generation groups; full mode for lifecycle, cycle, and failure
order; affected HIR/MIR/verifier/backend/runtime suites; legacy count deltas;
`make golden-test`; `make check`; docs check; and diff hygiene.

**Exit criteria:** Resource-owning feature specs preserve every prior native and
diagnostic observation, no migration introduces shared writable state, and all
corresponding legacy fixtures are removed.

### GR13 — Migrate remaining diagnostics and remove legacy discovery

**Purpose:** Finish the corpus, prove spec-only discovery completeness, and
delete compatibility code only when nothing consumes it.

- [ ] Inventory every remaining legacy source, sidecar, manifest, oracle, and
      supporting file and assign it to a feature owner or documented
      non-discovered oracle role.
- [ ] Migrate remaining parser, resolver, type, declaration, malformed-token,
      unsupported-form, and miscellaneous success/failure cases without
      weakening their owning observations.
- [ ] Require every discovered test source to be referenced by exactly one
      spec and report unreferenced source/expectation candidates for audit.
- [ ] Confirm legacy discovery reports zero native and zero compile-fail cases
      and that no spec leaf duplicates a migrated legacy identity.
- [ ] Remove the legacy loader, sidecar naming branches, working-directory
      compatibility locks, baseline-count assertions, and old `run` and
      `compile_fail` directory assumptions.
- [ ] Retain independent oracle generators as explicitly non-discovered tools
      and update their output instructions to the new owning specs.
- [ ] Rewrite the golden fixture guide as a concise description of the current
      spec-only format, feature organization, match policies, variants,
      filtering, determinism audits, artifacts, and migration-free commands.

**Tests:** Complete default, compile, and full suites; source/spec/orphan audit;
zero-legacy assertions; every runner unit test; `make golden-test`;
`make golden-determinism-test`; `make check`; `make msrv-check`; docs check;
and diff hygiene.

**Exit criteria:** Discovery is exclusively spec-driven, every golden source
and expectation has an intentional owner, legacy code and directories are
gone, oracles remain non-discovered, and living documentation contains no
migration instructions.

### GR14 — Harden, validate, document, and close

**Purpose:** Audit the completed runner by responsibility, meet performance and
quality gates from an artifact-free snapshot, and leave only current behavior
in living documentation.

- [ ] Audit runner facades and large modules for cohesive ownership; split
      schema, planning, process, scheduling, expectation, or reporting hotspots
      only where a repeated responsibility justifies it.
- [ ] Audit dependency versions, licenses, transitive packages, Rust 1.82
      support, and the boundary preventing runner dependencies from entering
      production compiler artifacts.
- [ ] Re-run collision, symlink escape, timeout descendant, large-pipe,
      randomized completion, resource-lock, fail-fast, binary-data, partial
      matcher, and machine-report stress coverage.
- [ ] Measure default and full warm performance and stage counts on the audit
      host; resolve material regressions or document a confirmed environmental
      explanation before closeout.
- [ ] Search code, tests, Make output, scripts, and living documentation for
      roadmap codes, old runner names, sidecar discovery, stale Cargo-test
      commands, and rollout language; remove every stale reference.
- [ ] Update the development, testing, scripts, and driver/toolchain documents
      to their concise final authorities without duplicating schema details
      outside the golden fixture guide.
- [ ] Run the complete repository gate and full determinism audit from an
      artifact-free snapshot, plus MSRV and extended robustness validation.
- [ ] Mark every roadmap item complete, archive this roadmap, update active and
      archive indexes, and retain the frozen design as its historical input.

**Tests:** `cargo test --locked -p skald-golden`; `make golden-test`;
`make golden-determinism-test`; `make check`; `make msrv-check`;
`make robustness-long`; `git diff --check`; documentation validation; and an
artifact-free final repetition of the complete ordinary and determinism gates.

**Exit criteria:** The Rust runner and spec-only corpus satisfy the frozen G1
through G12 design, default and audit modes are reliable and performant,
production dependency boundaries remain intact, living documentation describes
only current behavior, no high-priority maintainability issue remains, and the
completed roadmap is archived.

## Ordering and dependencies

GR0 freezes the serialized representation in code before any discovery logic
consumes it. GR1 then establishes paths, identities, expansion, and selection
as pure planning behavior. GR2 owns byte and process semantics before GR3
composes compiler, runtime, linker, and native stages. GR4 introduces
parallelism only after the complete sequential behavior is testable, and GR5
finishes user-facing reporting over stable structured results.

GR6 adapts the existing corpus after the new engine is complete so compatibility
logic cannot distort the core model. GR7 changes repository commands only after
old/new full-mode parity. GR8 through GR12 migrate coherent feature owners and
may inform one another, but they should land in order so fixture moves and
legacy counts remain easy to review. GR13 removes compatibility only after all
feature migrations report zero legacy cases. GR14 is the final responsibility,
performance, documentation, and artifact-free quality audit.

No task depends on another active roadmap. The frozen design proposal and the
current compiler driver, public `Toolchain`, runtime Make target, real `skac`
binary, legacy runner, and 288-case corpus are the implementation baseline.

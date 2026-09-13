# Test Plumbing Ownership Roadmap

Status: in progress; TP03 is next.

This roadmap implements the accepted
[Test Plumbing Ownership Design](TEST_PLUMBING_OWNERSHIP_DESIGN_PROPOSAL.md)
for [cleanup finding A33](CODEBASE_CLEANUP_AUDIT.md#a33--consolidate-test-plumbing-while-preserving-independent-checks).
It restructures Skald's largest Rust test suites and consolidates repeated
mechanical setup while retaining the independent boundaries that detect phase,
process, ownership, determinism, and native-execution defects.

The work is intentionally staged. The first task establishes an independently
tested subprocess and temporary-resource foundation. Determinism generators
then move in three bounded groups without changing their case registry or
observations. Driver reporting, driver compilation, and golden integration
fixtures move in separate tasks because they have different owners and failure
contracts. A final inventory and full-determinism audit is required before the
finding can close.

## Scope and invariants

- Preserve language behavior, compiler products, diagnostics, dumps, report
  events, metrics, generated assembly, runtime behavior, golden schema, and
  command-line behavior exactly.
- Preserve the meanings of `make check`, `make check-long`, and every focused
  Make target. Cargo's workspace manifest remains the sole Rust package
  inventory used by repository gates.
- Keep compiler unit, compiler integration, and golden integration support as
  separate compilation and ownership domains. Do not create a workspace-wide
  test utility crate.
- Keep all 53 current `pipeline_determinism` leaf test names, their two-process
  execution, byte comparisons, and permutation behavior.
- Preserve existing focused test commands where they are part of living
  documentation or automation. Record and update any unavoidable unit-test
  module-prefix change.
- Preserve independently useful overlap between local phase tests, public API
  tests, cross-process determinism, optimization equivalence, golden behavior,
  and native execution.
- Combine tests only when owner, process boundary, configuration, fixture,
  assertion, and caught failure are all equivalent.
- Shared source-to-phase helpers may assert only that prerequisite phases
  succeeded. The caller remains responsible for the named boundary.
- Keep hand-built and mutated MIR fixtures able to express invalid identities,
  types, ownership, lifetimes, control flow, and proof state. They must not use
  the verifier or proof being challenged to manufacture their negative input.
- Test helpers must make standard-library injection, optimization selection,
  runtime tracing, normalization, deadlines, and success policy explicit.
- Do not widen a production API or production visibility to share test code.
- Use recursive modules and concise facades where a suite has several cohesive
  responsibilities. Do not split files solely to meet a line-count target.
- Keep every intermediate task runnable through ordinary Cargo test discovery.
- Record larger unrelated findings in a dedicated, indexed discoveries file
  only when evidence is found; do not create an empty backlog.

## Progress

- [x] TP01 — Establish the integration subprocess and resource boundary
- [x] TP02 — Move module-backed determinism fixtures behind owned generators
- [ ] TP03 — Move value and ownership determinism fixtures
- [ ] TP04 — Complete determinism-suite ownership and registry consolidation
- [ ] TP05 — Split driver reporting tests by observation responsibility
- [ ] TP06 — Split driver pipeline tests by compilation responsibility
- [ ] TP07 — Consolidate golden integration resources by dependency level
- [ ] TP08 — Reconcile inventories, validate every boundary, and close A33

## PR-sized implementation sequence

### TP01 — Establish the integration subprocess and resource boundary

**Purpose:** characterize the current test inventory and replace repeated,
unbounded current-test-executable mechanics with one narrow compiler-integration
foundation before moving any large fixture family.

- [x] Record the starting revision and working-tree state. Capture sorted Rust
  test inventories for the compiler library, compiler determinism integration
  binary, and golden integration binaries. Record the 53 determinism leaf names
  and every living Makefile, script, and documentation reference to focused
  test paths.
- [x] Add a private `crates/skald-compiler/tests/support/` facade with only the
  temporary-resource and current-test-process responsibilities demonstrated by
  current integration consumers. Do not add a general source pipeline module
  until a later task identifies at least two exact consumers.
- [x] Give temporary files and directories collision-resistant ownership,
  create-new semantics, path access without public mutation, and cleanup on
  normal return and unwinding. Keep resources under the host temporary
  directory and make cleanup failure non-fatal during unwinding.
- [x] Define a current-test-process request that names the selected test,
  explicit environment, timeout, and diagnostic-output limit. Redirect stdout
  and stderr to owned temporary files so parent waiting cannot deadlock on full
  pipes. Poll to the caller-provided deadline, kill and reap on timeout, and
  return bounded output plus a typed completion or timeout observation rather
  than asserting a compiler result inside the utility. Keep spawn, wait,
  kill/reap, and output-read failures distinguishable with their source errors.
- [x] Use a 64 KiB default diagnostic limit only through an explicitly named
  test-process policy; callers may select a different finite limit. Report the
  complete file length when output exceeds the retained prefix so overflow is
  never mistaken for complete diagnostics.
- [x] Add focused support coverage for parallel resource creation, cleanup on
  unwind, successful and failed child status, timeout and reap, bounded
  stdout/stderr overflow, and concurrent invocations with isolated environment
  and outputs. Helper children must have their own finite fallback lifetime so
  a broken timeout test cannot hang the repository gate.
- [x] Migrate `expression_depth_robustness` and the determinism suite's child
  launching to the shared process primitive. Preserve the expression suite's
  15-second watchdog and failure context. Preserve two independent children
  per determinism case.
- [x] Replace the determinism suite's per-case output environment variables
  with one private helper-output key and one optional permutation key. An
  absent helper-output key selects the parent path; malformed helper state is
  an explicit test failure.
- [x] Keep all generators and test registrations in their current file during
  this task except for small moves required to give the harness one owner.

**Tests:** Add a focused integration support test binary or equally isolated
owner tests for the process/resource contract. Run
`cargo test --locked -p skald-compiler --test expression_depth_robustness`,
`cargo test --locked -p skald-compiler --test pipeline_determinism`, and run
the determinism integration binary with multiple test threads. Compare the
captured determinism inventory byte-for-byte. Run `cargo fmt --all -- --check`,
all-target Clippy, `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** compiler integration tests have one bounded current-executable
primitive and one owned temporary-resource implementation; expression-depth and
all determinism children use it; all 53 determinism leaf names and observations
remain intact; helper failures cannot hang on a pipe or wait beyond the chosen
deadline; and no production visibility or dependency changed.

#### TP01 baseline and delivery record

The starting revision was
`f1bdae7171a7a2219cd4d33c5ffdb1828a871b48`, with a clean working tree. Sorted
Cargo inventories captured before editing had these counts and SHA-256 hashes:

| Inventory | Tests | SHA-256 |
| --- | ---: | --- |
| `skald-compiler --lib` | 3,148 | `7736a44dc47b03db98eccdae074b7ca6c50f9faf2e65f424a2df714d528a0f0f` |
| `pipeline_determinism` | 53 | `c04091410676bb92fc34d395ad06ec986502f726e84a3a03d8982e2287f2e1f0` |
| `skald-golden --tests`, including the library harness Cargo runs for this target selection | 112 | `dd391f6d459f5f0222e7ef0e04b5f25dd979eddbd2f3455e9b0a9b9cd01e53c2` |
| Golden integration binaries only | 79 | `6b2c168d09b26666397729c2c76ca8f7642f453cc43c5a6b9543c4921a8f3649` |

The 53 determinism leaf names were:

```text
array_element_list_phase_products_are_deterministic_across_processes
array_phase_products_are_deterministic_across_processes
eager_boolean_diagnostics_are_deterministic_across_processes
eager_boolean_phase_products_are_deterministic_across_processes
final_field_diagnostics_are_deterministic_across_processes
final_field_phase_products_are_deterministic_across_processes
floating_comparison_diagnostics_are_deterministic_across_processes
floating_comparison_phase_products_are_deterministic_across_processes
floating_division_diagnostics_are_deterministic_across_processes
floating_division_phase_products_are_deterministic_across_processes
function_value_composition_products_are_deterministic_across_processes
general_iteration_diagnostics_are_deterministic_across_processes
general_iteration_phase_products_are_deterministic_across_processes
generic_interface_diagnostics_are_deterministic_across_processes
generic_interface_phase_products_are_deterministic_across_processes
generic_module_phase_products_are_deterministic_across_processes
generic_operator_phase_products_are_deterministic_across_processes
imported_unused_static_products_are_deterministic_across_processes
indexed_array_frontend_products_are_deterministic_across_processes
integer_bitwise_and_shift_diagnostics_are_deterministic_across_processes
integer_bitwise_and_shift_phase_products_are_deterministic_across_processes
integer_division_diagnostics_are_deterministic_across_processes
integer_division_phase_products_are_deterministic_across_processes
integer_operation_phase_products_are_deterministic_across_processes
io_phase_products_are_deterministic_across_processes
io_provider_diagnostics_are_deterministic_across_processes
mir_pipeline_checkpoints_are_deterministic_across_processes
module_diagnostics_are_deterministic_across_processes
module_phase_products_are_deterministic_across_processes
object_lifetime_phase_products_are_deterministic_across_processes
optional_value_phase_products_are_deterministic_across_processes
polymorphism_phase_products_are_deterministic_across_processes
primitive_cast_diagnostics_are_deterministic_across_processes
primitive_cast_phase_products_are_deterministic_across_processes
primitive_operator_profile_phase_products_are_deterministic_across_processes
private_cell_diagnostics_are_deterministic_across_processes
private_cell_phase_products_are_deterministic_across_processes
private_initializer_diagnostics_are_deterministic_across_processes
private_initializer_phase_products_are_deterministic_across_processes
produced_alias_phase_products_are_deterministic_across_processes
produced_field_phase_products_are_deterministic_across_processes
produced_receiver_phase_products_are_deterministic_across_processes
range_phase_products_are_deterministic_across_processes
range_syntax_diagnostics_are_deterministic_across_processes
shared_ownership_phase_products_are_deterministic_across_processes
short_circuit_source_products_are_deterministic_across_processes
static_field_diagnostics_are_deterministic_across_processes
static_field_module_products_are_deterministic_across_processes
static_field_phase_products_are_deterministic_across_processes
static_initializer_lifecycle_products_are_deterministic_across_processes
static_lifetime_cycle_diagnostics_are_deterministic_across_processes
string_language_item_diagnostics_are_deterministic_across_processes
string_phase_products_are_deterministic_across_processes
```

The living focused-reference scan recorded 65 matches with SHA-256
`42560e4b8adf547f35d42ef898aa87d2bea79f7198792acab695c0ef62f47e0e`.
Its owners were the `Makefile`, `scripts/README.md`, development `README.md`,
`TESTING.md`, and `DEBUGGING.md`, the range, generic-interface, and operator
test matrices, `GENERIC_INTERFACES.md`, and the active cleanup audit, design,
and roadmap. The concrete determinism filters in `DEBUGGING.md` select generic
modules, generic interfaces, function values, private initializers, strings,
and private cells. No living focused path changed in TP01.

TP01 introduced one private compiler-integration support facade with separate
process and temporary-resource owners. The process request makes its selected
test, environment, deadline, and diagnostic limit explicit; stdout and stderr
go to owned files, and completion, timeout, overflow, and system errors remain
typed observations for the calling suite. The expression-depth suite retains
its 15-second watchdog. Determinism uses a 60-second child deadline, two child
processes per case, and one output key plus one optional permutation key. Its
post-change sorted inventory was byte-identical to the 53-name baseline.

Validation passed the focused support and expression-depth suites, complete
determinism with default and eight-thread scheduling, formatting, all-target
workspace Clippy with warnings denied, `make check`, `make msrv-check`, and
`git diff --check`.

### TP02 — Move module-backed determinism fixtures behind owned generators

**Purpose:** establish the recursive determinism-suite layout on the fixture
families with the most shared module-tree, provider-order, path-normalization,
and permutation mechanics.

- [x] Keep `pipeline_determinism.rs` as the only Cargo integration-test entry
  file. Introduce a private `pipeline_determinism/` tree with a concise harness
  facade and responsibility modules; do not create additional top-level
  integration binaries for feature families.
- [x] Introduce explicit root-level case registration for module graph,
  generic module, generic interface, generic operator, range, and general
  iteration cases. Use small declaration macros only where they preserve the
  existing leaf name and visibly state label, same-input versus permutation
  mode, and generator function.
- [x] Move module-tree creation, source writing, directory linking, provider
  permutation, fixture-path replacement, and span normalization to the
  narrowest module-backed support owner. Keep filesystem normalization separate
  from semantic dump generation.
- [x] Move module graph and closed generic generators into cohesive child
  modules. Keep range and iteration generators together only where they share
  the same canonical module/provider fixture; keep their diagnostic generators
  distinct from successful phase-product generation.
- [x] Preserve every emitted section, section order, trailing newline,
  diagnostic, source/provider permutation, selected entry, and normalized byte
  comparison exactly.
- [x] Keep generator functions narrow and `pub(super)` only where the root case
  registry must call them. Do not make fixture internals visible across
  unrelated determinism families.
- [x] Remove moved constants, imports, helpers, and source text from the root
  file in the same task. Do not leave compatibility forwarding inside test
  modules.

**Tests:** Run the module, generic-module, generic-interface, generic-operator,
range, and iteration determinism filters individually, then the complete
`pipeline_determinism` binary with default and multi-threaded execution. Compare
the complete 53-name inventory and the moved cases' child counts and output
bytes with TP01's baseline. Run documentation links, formatting, all-target
Clippy, `make check`, `make msrv-check`, and `git diff --check`.

**Exit criteria:** module-backed generators, fixtures, and normalization have
clear private owners; the root integration file is the unchanged test binary
and explicit registry; all moved leaf names, subprocess counts, permutations,
and bytes match the baseline; and the remaining inline families continue to
run through the same harness.

#### TP02 delivery record

`pipeline_determinism.rs` remains the sole Cargo integration entry and now
registers the moved cases through declarations that expose each unchanged leaf
name, label, execution mode, and generator. Its private recursive module tree
has separate owners for the subprocess harness, module fixtures, filesystem
and span normalization, module graph cases, closed generic contracts, ranges,
and general iteration. Diagnostic generation remains distinct from successful
phase-product generation. The remaining inline families use the same harness
and module-fixture facade while their generators await TP03 and TP04.

Rust requires an item re-exported through two private module levels to be
declared crate-visible at its defining leaf. Those definitions sit behind
private modules, and the facade exposes only the selected functions to its
parent with `pub(super)`; no production or cross-binary API was added.

The post-change sorted inventory retained all 53 names and the TP01 SHA-256
`c04091410676bb92fc34d395ad06ec986502f726e84a3a03d8982e2287f2e1f0`.
An isolated build of the TP01 revision produced every moved generator output
for both permutation variants and one output for each same-input case. All 18
artifacts, totaling 1,077,743 bytes, matched the new implementation
byte-for-byte. The ten parent cases still make two independent child calls,
including both variants for the eight permutation cases.

Validation passed every focused module, generic-module, generic-interface,
generic-operator, range, and iteration filter; complete default and
eight-thread determinism runs; documentation links; formatting; all-target
workspace Clippy with warnings denied; `make check`; `make msrv-check`; and
`git diff --check`.

### TP03 — Move value and ownership determinism fixtures

**Purpose:** move the large object, value, ownership, array, and static fixture
families as one semantic group while the proven harness continues to serve
both moved and inline generators.

- [ ] Group the remaining object, polymorphism, produced-view, ownership,
  optional, array, static, private-initializer, private-cell, final-field, and
  function-value generators by semantic responsibility. Prefer a few cohesive
  family modules over one file per test or a miscellaneous remainder module.
- [ ] Keep successful phase products and diagnostic-only products separate when
  they run different pipelines or normalize different evidence.
- [ ] Extract a compiler-integration source-to-phase helper only where at least
  two moved generators perform the exact same prerequisite sequence. Give the
  helper a boundary-specific name, explicit standard-library inputs, and a
  returned product or observation; do not create a second general compiler
  driver in test support.
- [ ] Keep test source text and feature-specific output composition beside the
  generator that owns their semantic purpose. Do not deduplicate source solely
  because two layers happen to start from the same spelling.
- [ ] Review every normalization operation. Retain only normalization required
  for deliberately unstable filesystem paths, source spellings, or spans; each
  retained normalization must name the instability it removes.
- [ ] Remove moved constants, imports, helpers, and source text from the root
  registry. Leave primitive, service, and checkpoint generators runnable in
  place for TP04.

**Tests:** Run the object, polymorphism, produced-value, ownership, optional,
array, static, private-member, final-field, and function-value determinism
filters individually, then the complete integration binary with default and
multi-threaded execution. Compare the complete 53-name inventory, child counts,
and moved bytes with TP01. Run public API tests, `make check`,
`make msrv-check`, all-target Clippy, documentation checks, and
`git diff --check`.

**Exit criteria:** value and ownership generators have cohesive private owners;
the root registry no longer contains their implementation; all 53 tests remain
discoverable; every moved case retains its exact two-process or permutation
behavior and bytes; and the remaining inline generators still use the same
harness.

### TP04 — Complete determinism-suite ownership and registry consolidation

**Purpose:** move the remaining primitive, standard-service, and MIR checkpoint
generators, leaving one navigable case registry and removing obsolete plumbing
only after every family uses the recursive structure.

- [ ] Group integer and floating operations, casts, eager and short-circuit
  booleans, strings, I/O, and MIR checkpoint generators by semantic
  responsibility. Keep successful phase products and diagnostic products
  separate where their pipelines differ.
- [ ] Reassess any compiler-integration source-to-phase helper introduced in
  TP03 against these final consumers. Retain it only when the prerequisite
  sequence and policy are exact; otherwise prefer small family-local helpers.
- [ ] Finish the root case registry while preserving all 53 leaf test names,
  labels, same-input/permutation modes, and generator associations. Remove all
  superseded constants, imports, source text, and inline harness mechanics.
- [ ] Review every remaining normalization operation. Each retained path,
  spelling, or span normalization must name the deliberate instability it
  removes and must not conceal semantic output differences.
- [ ] Confirm the recursive module facades use explicit, narrow imports and
  `pub(super)` visibility only where the registry or a sibling responsibility
  requires it.
- [ ] Document the resulting determinism layout and focused selection examples
  in the testing and debugging guides without exposing internal helper names as
  stable production API.

**Tests:** Run every primitive-operation, cast, boolean, string, I/O, and MIR
checkpoint filter individually, every focused determinism command referenced
by living documentation, and the complete integration binary with default and
multi-threaded execution. Compare the sorted 53-test inventory with TP01 and
verify through harness-owned observations that every parent still launches
exactly two children. Run public API tests, `make check`, `make msrv-check`,
all-target Clippy, documentation checks, and `git diff --check`.

**Exit criteria:** `pipeline_determinism.rs` is a concise, explicit registry;
every generator and fixture has a cohesive private owner; the suite retains one
integration binary, all 53 names, two-process execution, permutation semantics,
and byte-identical observations; and no broad helper hides phase or feature
policy.

### TP05 — Split driver reporting tests by observation responsibility

**Purpose:** make the 1,704-line reporting suite navigable around its actual
contracts while preserving exact phase, metric, inspection, failure, and
observer assertions.

- [ ] Capture the reporting test names, fully qualified filters, and incoming
  references before moving code. Identify actively documented focused commands
  that need a root wrapper or an updated authoritative path.
- [ ] Convert `driver/tests/reporting.rs` into a recursive module with a concise
  facade. Give separate owners to phase lifecycle/order, phase- and pass-owned
  metrics, inspection and writer behavior, failure boundaries, and observer
  isolation/determinism.
- [ ] Keep the success-phase vocabulary and checkpoint expectations with the
  phase/metric owners that interpret them. Avoid a global constants file that
  recreates implicit parent imports.
- [ ] Place request construction, phase-pair assertions, metric lookup, event
  normalization, and malformed target inputs beside their narrowest common
  consumers. Use explicit imports in child modules rather than `use super::*`
  when that would hide production dependencies.
- [ ] Preserve every exact phase sequence, outcome, metric owner/name/value,
  event order, writer error, panic propagation, artifact, and source diagnostic
  assertion. Structural moves do not justify rewriting broad expectations into
  weaker containment checks.
- [ ] Keep malformed MIR/backend fixtures direct and defect-specific. Do not
  construct them through final verification or inspection seals that the test
  is intended to reject.
- [ ] Update living references to moved test paths and record the reviewed
  rename map for any fully qualified name that cannot remain unchanged.

**Tests:** Run every new reporting child module separately, then
`cargo test --locked -p skald-compiler driver::tests::reporting`. Compare the
pre/post reporting test count and leaf names, allowing only recorded module
prefix changes. Run public API and pipeline determinism tests, all-target
Clippy, `make check`, `make msrv-check`, documentation checks, and
`git diff --check`.

**Exit criteria:** reporting tests are grouped by the observations they own;
shared helpers remain mechanical and local; exact reporting behavior and
negative fixtures are unchanged; every previous test has a traceable
destination; and focused documentation selects the intended tests.

### TP06 — Split driver pipeline tests by compilation responsibility

**Purpose:** separate request/provider policy, complete compiler composition,
and malformed-source robustness so new driver behavior has an obvious test
owner without collapsing complete-pipeline checks into earlier phase tests.

- [ ] Capture the pipeline test inventory, fully qualified filters, source
  fixture groups, and incoming references before moving code.
- [ ] Convert `driver/tests/pipeline.rs` into a recursive module with owners for
  request and provider selection, complete source-to-backend composition,
  malformed/excessive-source robustness, and cohesive feature-composition
  groups where needed.
- [ ] Keep request-local filesystem setup and provider roots with request tests.
  Share source-to-assembly adapters through existing driver/test-support
  boundaries rather than rebuilding the compilation pipeline in the suite.
- [ ] Preserve configuration-versus-source failure categories, disabled and
  replacement standard-library behavior, unreachable-source behavior,
  artifact contents, target selection, optimization profiles, and exact
  diagnostic rendering.
- [ ] Preserve `catch_unwind` tests that protect recoverable malformed inputs,
  and keep the process-isolated expression-depth watchdog as a distinct
  integration test for abort/nontermination behavior that `catch_unwind`
  cannot observe.
- [ ] Retain complete feature-composition tests even where a golden or phase
  test uses similar source. Move a case to golden ownership only as a separate
  reviewed behavioral change with equivalent observations, never as part of
  this structural task.
- [ ] Use explicit child imports and narrow helper visibility. Remove obsolete
  forwarding and broad parent imports after all consumers move.
- [ ] Update living focused commands and record any reviewed module-prefix
  changes.

**Tests:** Run every new pipeline child module, the complete
`driver::tests::pipeline` suite, `expression_depth_robustness`, public API,
phase boundaries, and pipeline determinism. Compare pre/post test inventories
and exact diagnostics/artifacts. Run all-target Clippy, `make check`,
`make msrv-check`, documentation checks, and `git diff --check`.

**Exit criteria:** request/provider, complete-pipeline, robustness, and feature
composition tests have clear owners; every prior case remains mapped and
equally strong; process-isolated robustness remains independent; and the
driver test facade contains only shared orchestration and module declarations.

### TP07 — Consolidate golden integration resources by dependency level

**Purpose:** share collision-resistant temporary resources and focused fixture
writers across golden integration binaries without making planning or process
tests depend on the high-level execution harness they are meant to test.

- [ ] Capture all golden integration test names, test-binary boundaries,
  temporary path shapes, cleanup behavior, fake-tool dependencies, and focused
  Make targets before changing support.
- [ ] Split `crates/skald-golden/tests/support/` into cohesive temporary,
  high-level fixture, fake-tool, and spec-writer responsibilities. Keep a
  concise facade for widely shared primitives and allow low-level tests to
  include a purpose-specific support file without compiling the complete
  execution fixture.
- [ ] Extract one collision-resistant temporary workspace primitive supporting
  owned roots, required associated artifact paths, file/tree writing, and
  cleanup on normal return and unwind. Preserve absolute/canonical path
  behavior used by diagnostic normalization and sandbox assertions.
- [ ] Compose the existing high-level `Fixture` from the temporary primitive
  while preserving compiler, runtime, linker, selection, timeout, environment,
  determinism, counter, assembly-log, and retention configuration exactly.
- [ ] Migrate the planning fixture to the temporary primitive while retaining
  direct `build_plan` and `select` calls. Do not initialize execution state.
- [ ] Migrate the process-execution fixture while retaining direct
  `run_process` and `execute_run` calls, explicit limits and deadlines, and
  access to fake-process behavior. Do not route process tests through
  `execute_sequential`.
- [ ] Keep report, sequential, parallel, and compiler-diagnostic fixtures on
  the high-level composition they already exercise. Split spec writers and
  fake binary paths only where multiple such consumers exist.
- [ ] Remove or narrow the broad test-support `dead_code` allowance after each
  integration root compiles only the support it needs. Do not add dummy uses to
  satisfy lints.
- [ ] Preserve the A02–A04 full-pipe, descendant, timeout, binary capture,
  overflow, signal, failure precedence, raw diagnostic, and cleanup
  regressions exactly.

**Tests:** Run `cargo test --locked -p skald-golden` plus the focused planning,
process execution, compiler diagnostic, sequential, parallel, and reporting
integration binaries. Run `make golden-expectations-test` and compare the
complete golden Rust test inventory. Run all-target Clippy, `make check`,
`make msrv-check`, documentation checks, and `git diff --check`.

**Exit criteria:** golden integration tests share low-level resources without
sharing higher-level policy; every test still calls the intended library
boundary; temporary and cleanup behavior is characterized; the broad support
allowance is removed or narrowly justified; and all process robustness
observations remain intact.

### TP08 — Reconcile inventories, validate every boundary, and close A33

**Purpose:** prove that the restructuring improved ownership without weakening
coverage, remove migration residue, update authoritative guidance, and close
the audit finding only after the complete independent checks pass.

- [ ] Compare final sorted compiler library, determinism integration, and golden
  integration inventories with TP01. For every count or fully qualified name
  change, record the old-to-new mapping and reason. Restore any missing or
  accidentally weakened test before proceeding.
- [ ] Confirm all 53 determinism leaf names remain unchanged, all cases launch
  two independent processes, and every permutation case still uses distinct
  source/provider input order.
- [ ] Audit the touched support and suite facades for mixed responsibilities,
  broad imports, broad lint allowances, unused compatibility forwarding,
  hidden defaults, and helpers with only one trivial consumer. Resolve small
  local issues; record larger unrelated work in an indexed discoveries file.
- [ ] Audit `mir::test_fixtures` by responsibility. Retain its existing
  corruption-capable boundary and recursive child modules; split an additional
  family only if the completed test moves reveal a concrete mixed owner. File
  size alone is not a reason to churn it.
- [ ] Confirm A01–A06 coverage remains selected by the ordinary gate: expression
  depth, full-duplex linker I/O, deadlines through pipe completion, bounded
  captures/files, capacity arithmetic, and manifest-derived workspace tests.
- [ ] Update `docs/development/TESTING.md`, debugging commands, and any focused
  compiler test matrices to describe current ownership and paths. Remove
  rollout language and roadmap task codes from living documentation.
- [ ] Update A33 in the cleanup audit with its completion status, delivered
  ownership, preserved coverage evidence, test inventory results, and exact
  validation commands.
- [ ] Mark every roadmap checkbox complete only after its exit criteria pass.
  Set the design and roadmap status to complete, move both documents to
  `docs/archive/`, update the active and archive indexes, and repair all links.

**Tests:** Run every focused suite named in TP01–TP07, then run from a
repository state containing only the intended task changes:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
make check
make golden-determinism-test
make msrv-check
cargo run --quiet --locked -p skald-docs-check -- .
git diff --check
```

Inspect `git status --short`, the complete diff, and documentation links after
archival. Confirm no generated test output or temporary artifact is tracked.

**Exit criteria:** all planned ownership boundaries are implemented; test
inventories are reconciled with no accidental loss; independent phase,
process, malformed-input, optimization, golden, and native observations remain;
all focused and repository gates pass; living documentation describes the new
layout; the design and roadmap are archived; and A33 is marked complete with
evidence.

## Ordering and dependencies

TP01 comes first because every later determinism move depends on a bounded,
characterized child-process contract and a recorded inventory. TP02 establishes
the recursive layout with module-backed fixtures whose shared mechanics are
already visible. TP03 moves the value and ownership families after that pattern
is proven, while TP04 finishes primitive, service, and checkpoint families and
closes the determinism layout. This split keeps each large mechanical move
reviewable while retaining one binary and one harness throughout.

TP05 and TP06 remain separate because structured reporting and compilation
composition exercise different driver contracts and carry different negative
fixtures. They depend on the inventory discipline from TP01 but not on the
determinism generators' internal file placement. TP07 stays inside the golden
crate and can begin after TP01's resource contract is understood; it must not
reuse compiler integration code across crate boundaries.

TP08 is the only closure task. It depends on every structural migration,
reconciles all inventories, runs full golden determinism, updates the audit,
and archives the accepted design and completed roadmap. No earlier task may
mark A33 complete.

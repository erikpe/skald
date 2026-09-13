# Test Plumbing Ownership Design Proposal

Status: complete frozen decision record. Accepted and fully implemented on
2026-09-13 through the archived
[Test Plumbing Ownership Roadmap](TEST_PLUMBING_OWNERSHIP_ROADMAP.md).

This proposal addresses
[cleanup finding A33](../roadmaps/CODEBASE_CLEANUP_AUDIT.md#a33--consolidate-test-plumbing-while-preserving-independent-checks).
It makes Skald's largest Rust test suites easier to navigate and removes
repeated mechanical setup while preserving the independent observations that
catch compiler, process, and ownership defects. The change is internal to test
code and test documentation. It does not change the language, compiler output,
runtime behavior, golden schema, or repository quality gate.

## Intended outcome

- Keep each test at the narrowest layer that owns the behavior it observes.
- Turn the 2,913-line compiler determinism integration test into a concise case
  registry, a single subprocess protocol, and cohesive fixture families.
- Split the large driver pipeline and reporting test modules by responsibility
  while sharing only mechanical setup within the driver test owner.
- Give golden integration tests a small temporary-resource foundation without
  forcing planning and process tests through the higher-level execution
  fixture.
- Preserve deliberately malformed MIR builders as independent adversarial
  inputs rather than deriving them from production verification or proof APIs.
- Preserve the existing test inventory, exact output assertions, independent
  process boundaries, native observations, and failure-path coverage.
- Leave production facades and dependencies unchanged.

## Current architecture and evidence

The repository already separates test layers well, but several files have
accumulated unrelated fixture families and repeated setup:

| Owner | Current evidence | Main issue |
| --- | --- | --- |
| Compiler integration determinism | `crates/skald-compiler/tests/pipeline_determinism.rs` is 2,913 lines and owns 53 tests | Dozens of per-case output-environment constants and all generators, module fixtures, normalization, subprocess execution, and assertions share one file. |
| Driver reporting | `crates/skald-compiler/src/driver/tests/reporting.rs` is 1,704 lines | Phase ordering, metrics, inspection, failure boundaries, observer isolation, malformed MIR, and assertion utilities share one module. |
| Driver pipeline | `crates/skald-compiler/src/driver/tests/pipeline.rs` is 1,279 lines | Request/provider behavior, malformed-source robustness, and many feature-composition fixtures share one module. |
| Compiler unit support | `crates/skald-compiler/src/test_support.rs` is 741 lines | It is a useful `cfg(test)` foundation, but its crate-private visibility cannot serve integration-test crates without widening production API. |
| MIR fixtures | `crates/skald-compiler/src/mir/test_fixtures.rs` is 994 lines with focused child modules | Its explicit constructors intentionally permit invalid states and must remain independent of verifier proof construction. |
| Golden integration support | `crates/skald-golden/tests/support/mod.rs` is shared by four integration binaries | It combines temporary paths, complete planning/execution setup, fake tools, and spec writers under a broad test-only dead-code allowance. Planning and process integration tests retain separate temporary fixture implementations because they exercise lower boundaries. |

The determinism suite currently launches its own test executable twice with
`--exact`, passes a case-specific environment variable naming an output file,
and compares the resulting bytes. Permutation cases launch two independent
processes with different input variants. This is a valuable boundary: it
catches process-local iteration or identity instability that two calls inside
one process would miss. Consolidation must retain those child processes.

The existing foundations also establish useful limits:

- `test_support` supplies source-to-phase helpers, temporary files and
  directories, native assembly execution, and canonical standard-library
  sources for compiler unit tests.
- `mir::test_fixtures` uses explicit identities, types, ownership modes, spans,
  blocks, and instructions. Its helpers remove structural boilerplate without
  deciding semantic validity.
- golden tests already use real fake-compiler, fake-linker, and fake-process
  binaries. Their process tests observe deadlines, descendant cleanup, binary
  capture bounds, full pipes, and failure precedence.
- A01–A05 already delivered process-isolated depth limits, full-duplex linker
  coverage, deadline-through-pipe-completion regressions, bounded-capture
  regressions, and capacity-overflow golden cases. A33 must retain these tests;
  it does not need another copy of them in a general harness.
- A06 made the ordinary Rust gate manifest-derived. A33 must keep workspace
  discovery authoritative rather than adding a second package or test list.

## Scope and invariants

- This work may move, split, rename, and consolidate test-only Rust code and
  update test documentation. Production modules, public Rust paths, compiler
  phase products, and the golden schema are outside scope.
- `make check`, `make check-long`, and focused Make targets retain their current
  meanings. Workspace test discovery remains manifest-derived.
- Every existing test has a traceable destination. Similar-looking tests may
  remain when they observe different layers, processes, configurations, or
  failure categories.
- Cross-process determinism continues to compare bytes produced by separate
  executable processes. Permutation cases continue to vary source or provider
  order across those processes.
- Native equivalence remains a source-to-executable observation. It is not
  replaced by MIR or assembly equality.
- Exact diagnostics, dumps, report events, metrics, statuses, stdout, stderr,
  and artifact assertions remain exact where they are exact today.
- Shared source-to-phase helpers assert success only for phases before the
  product named by the helper. The owning test remains responsible for the
  boundary under examination.
- Negative MIR fixtures remain able to construct and mutate malformed products
  directly. A helper must not run the same verifier or consume the same proof
  whose rejection the test is intended to challenge.
- Test helpers do not silently add the standard library, choose an optimization
  profile, normalize output, select runtime tracing, or assert success unless
  that policy is explicit in the helper's name or typed input.
- No helper becomes public production API solely to cross a Rust test-crate
  boundary.
- File length is evidence for mixed responsibilities, not an independent
  acceptance metric. The implementation should split by behavior and owner.

## Design principles

### Keep support local to its compilation boundary

Use three support domains:

1. `skald-compiler::test_support` remains crate-private and `cfg(test)` for
   compiler unit tests that need private compiler owners.
2. `crates/skald-compiler/tests/support/` owns only public-API plumbing shared
   by compiler integration-test binaries.
3. `crates/skald-golden/tests/support/` owns golden integration resources and
   fake-tool configuration.

Do not introduce a workspace `test-utils` crate. The current repetition does
not justify another versioned dependency surface, and such a crate could not
legitimately expose compiler-private fixtures. A helper should move upward only
after at least two consumers at the same compilation boundary demonstrate the
same responsibility.

### Share mechanics, retain assertions

Good shared responsibilities include collision-resistant temporary resources,
writing fixture trees, spawning the current test executable with a deadline,
capturing child failure context, and appending an already selected phase dump.
Expected diagnostics, semantic identities, phase order, metric values,
ownership traces, and corruption steps stay in the test family that asserts
them.

Helpers should return observations or explicit products. They should avoid
large `assert_valid_program` or `compile_everything` functions that would make
tests pass by relying on the same behavior they are meant to inspect.

### Preserve independently useful redundancy

Two tests are candidates for combination only when all of these are equal:

- owning layer and API boundary;
- process boundary and configuration;
- fixture semantics;
- asserted observation; and
- failure mode caught when the assertion breaks.

Shared setup alone is not grounds for deleting a test. In particular, local
phase invariants, complete golden behavior, process determinism, optimization
equivalence, and native execution remain separate even when they begin with
the same source text.

## Proposed ownership model

### Compiler integration support

Add a private integration-test support tree:

```text
crates/skald-compiler/tests/
├── support/
│   ├── mod.rs
│   ├── process.rs
│   ├── source.rs
│   └── temporary.rs
├── pipeline_determinism.rs
└── pipeline_determinism/
    ├── harness.rs
    ├── modules.rs
    ├── ownership.rs
    ├── primitives.rs
    └── statics.rs
```

The exact child names may follow the responsibilities found during migration;
they are not a required taxonomy. `support/mod.rs` should remain a concise
test-only facade. `process` owns a bounded current-executable invocation,
`temporary` owns unique cleanup-safe paths, and `source` may expose only exact
public-API phase sequences that have more than one consumer.

This support is compiled independently into each integration-test binary. It
must therefore remain small and dependency-free beyond `skald-compiler` and
the standard library. Unit tests continue using `crate::test_support`; copying
a tiny adapter across the unit/integration compilation boundary is preferable
to widening compiler visibility.

### Determinism case registry

Keep `pipeline_determinism.rs` as the integration-test entry file and case
registry. Use a small declaration macro or equally explicit registration
function to retain each current root test name while moving generators into
responsibility modules. Each registration states:

- the existing test function name;
- a short artifact label;
- whether both child processes use the same input or distinct variants; and
- the generator function.

All cases use one private helper-output environment key. Because each parent
passes environment directly to its own child and `--exact` selects one test,
case-specific environment keys add no isolation. Permutation input uses one
separate variant key. The harness must reject malformed helper state instead
of silently running a different mode; an absent key continues to select the
ordinary parent invocation.

The determinism harness composes `support::process` and owns:

- two collision-resistant temporary output files;
- the selected test name, helper mode, and optional permutation variant for
  each child invocation;
- byte comparison with the case label in the assertion.

The lower-level process helper owns current-executable spawn, a finite child
deadline with kill/reap on timeout, and stdout/stderr context for spawn,
timeout, status, and capture failures. Child output is redirected to owned
temporary files so waiting cannot deadlock on full pipes, then read with a
small explicit diagnostic limit. The helper returns a process observation to
the determinism harness or expression-depth caller; it does not compare
compiler products.

The generator modules own source text, module trees, phase selection, path/span
normalization, and expected semantic sections. Normalization helpers remain
specific: filesystem-path replacement and intentionally unstable span
normalization must not become a general way to hide output differences.

Retaining one integration-test binary avoids multiplying the link and startup
cost of the large compiler dependency and keeps the current-executable helper
protocol straightforward. Splitting this suite into many top-level integration
tests is not part of the design.

### Driver tests

Keep `driver/tests/mod.rs` as the test facade and its existing common CLI
adapters. Convert `pipeline.rs` and `reporting.rs` into recursive modules whose
children own coherent behavior:

- request construction, provider selection, and compilation failure categories;
- complete source-to-backend composition and malformed-source robustness;
- phase ordering and lifecycle outcomes;
- phase-owned and pass-owned metrics;
- inspection boundaries and writer failures; and
- observer isolation and determinism.

Common functions such as request construction, phase-pair assertions, metric
lookup, and explicit malformed backend inputs should live beside the narrowest
set of consumers. A malformed fixture may be shared by several driver tests,
but its constructor must state the intended defect and must not rely on public
verification to create its evidence.

Moving a unit test into a child module changes its fully qualified test-filter
path. Before moving it, inventory references in living documentation,
Makefiles, scripts, and roadmaps. Preserve a root wrapper where a stable focused
command is actively documented; otherwise update the authoritative reference
and record the rename in the implementation review.

### MIR fixtures

Retain `mir::test_fixtures` as a MIR-owned unit-test facility. Continue its
existing recursive-module direction by extracting cohesive construction
families when touched. Do not merge its constructors with source-to-MIR
helpers: source-derived fixtures prove lowering, while hand-built fixtures
permit states that source and validated builders cannot produce.

Each constructor must keep identities, types, ownership, spans, and control
flow visible at the call site unless a repeated closed protocol has one clear
name. Semantic defaults that make malformed states impossible are rejected for
this owner.

### Golden integration support

Split the current golden support by dependency level:

```text
crates/skald-golden/tests/support/
├── mod.rs
├── fixture.rs
├── specs.rs
├── temporary.rs
└── tools.rs
```

`temporary` provides a cleanup-safe test directory and associated paths.
Planning and low-level process tests may use it without constructing compiler,
runtime, linker, selection, or execution configuration. `fixture` composes the
temporary root with the existing high-level golden plan and execution setup.
`tools` owns fake binary paths and typed environment setup. `specs` owns the
shared TOML writers.

The shared facade should re-export only compact primitives used by most
consumers. An integration root may include a purpose-specific support file
directly when it does not need the high-level fixture, allowing the current
broad `allow(dead_code)` on the combined support facade to be removed or
narrowed. A low-level process test must continue calling `run_process` directly;
using the high-level `Fixture::execute` merely to share temporary paths would
erase the boundary being tested.

## Characterization and migration

Before the first move, capture sorted test inventories from:

```text
cargo test --locked -p skald-compiler --lib -- --list
cargo test --locked -p skald-compiler --test pipeline_determinism -- --list
cargo test --locked -p skald-golden --tests -- --list
```

The current determinism baseline is 53 named cases. The implementation records
the exact pre-migration inventory in its review artifact or roadmap delivery
notes, not in a permanent second manifest that could drift from Rust's own test
discovery.

Migrate in these semantic stages:

1. Introduce compiler integration temporary/process support and characterize
   its timeout, child failure, cleanup, and concurrent invocation behavior.
2. Move determinism generators by family while keeping one binary, the 53 root
   test names, byte comparisons, and variant behavior. Remove per-case
   environment constants after the final family moves.
3. Split driver pipeline and reporting tests, moving common helpers only after
   their consumers reveal a stable owner. Compare the unit-test inventory and
   update intentional filter paths.
4. Extract the golden temporary-resource base and compose the current
   higher-level fixture from it. Migrate planning and process fixtures without
   changing which library boundary each test calls.
5. Audit remaining repetition by responsibility, update the testing guide, and
   close A33 only after the inventories and repository gates match.

Each stage should be independently reviewable and leave all test binaries
runnable. Structural moves should avoid changing assertions and helper behavior
in the same diff unless a focused characterization test first pins the helper
contract.

## Validation strategy

### Helper contracts

- Temporary resources are unique under parallel creation, remain inside the
  host temporary directory, and are removed on normal return and unwinding.
- The integration subprocess helper reports spawn failure, child failure,
  timeout, unreadable output, and diagnostic-output overflow with the selected
  test and bounded captured context.
- Concurrent determinism parents cannot observe each other's helper output or
  variants.
- Source-to-phase helpers stop at their named boundary and surface earlier
  diagnostics rather than manufacturing a later product.
- Golden low-level fixtures do not initialize higher-level compiler, runtime,
  linker, or scheduler state.

### Preserved observations

- All 53 determinism cases remain discoverable under their current leaf names
  and still execute two independent processes.
- Permutation cases still compare distinct source/provider orderings rather
  than two identical invocations.
- Driver phase order, metrics, reports, failures, inspection, and public request
  composition retain their current exact assertions.
- MIR verifier tests retain direct malformed-input construction and rejection.
- Golden process tests retain full-pipe, retained-descendant, timeout, binary
  capture, overflow, signal, and cleanup observations from A02–A04.
- The external-watchdog expression-depth regression and capacity-overflow
  goldens from A01/A05 remain selected by the ordinary repository gate.
- Native and golden equivalence cases are neither deleted nor replaced by
  earlier-phase assertions.

### Repository gates

Every implementation stage runs focused owner tests plus:

```text
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
make check
make msrv-check
git diff --check
```

The closing stage also runs the full determinism gate because it changes test
execution infrastructure:

```text
make golden-determinism-test
```

Compare sorted pre/post test inventories. Any count or name change requires a
reviewed mapping and an explanation; accidental loss is a failed migration.

## Risks and controls

| Risk | Control |
| --- | --- |
| A shared helper hides the behavior under test. | Helpers return observations or earlier products; semantic assertions remain with owners. |
| Splitting determinism into multiple binaries increases build cost or breaks self-spawning. | Retain one top-level integration binary and use child modules for generators. |
| Parallel tests collide through environment or temporary paths. | Pass environment on each `Command`, use one child-selected protocol, and allocate collision-resistant owned resources. |
| A timeout helper repeats production process-runner complexity. | Keep it test-only and limited to current-executable spawn, deadline, kill/reap, and diagnostics; do not model process groups or general pipes. |
| Moving tests silently changes focused commands. | Capture inventories and references first; retain wrappers or update authoritative commands deliberately. |
| Generalized MIR builders stop producing invalid states. | Keep explicit MIR-owned constructors and mutation access independent of verification proofs. |
| Golden fixture reuse raises a low-level test to the wrong abstraction. | Share only temporary resources downward; compose high-level planning/execution state upward. |
| Cleanup work deletes overlapping but independent checks. | Require equality of owner, process, configuration, fixture, assertion, and caught failure before combining tests. |

## Alternatives considered

### One workspace-wide test utility crate

Rejected for the current scope. It would add a dependency and visibility
surface, could not access compiler-private owners without API changes, and
would encourage unrelated crates to converge on abstractions they do not
share.

### Split every large file into a separate integration-test binary

Rejected. Cargo would compile and link more large binaries, the self-spawning
determinism protocol would fragment, and file size would improve without
necessarily clarifying ownership. Recursive modules inside the existing test
binary provide the navigation benefit.

### Replace cross-process determinism with repeated in-process calls

Rejected. In-process repetition cannot expose process-randomized iteration,
fresh global state, or independent initialization differences.

### Move complete behavior into golden tests

Rejected. Golden tests are the right owner for source-to-diagnostic and
source-to-native behavior, but they cannot replace direct phase dumps,
structured report events, malformed MIR, or public Rust API composition.

### Generate a permanent test manifest

Rejected. Rust and the golden planner already own discovery. A second checked
inventory would drift. Capture and compare inventories during migration, then
rely on the normal discovery and repository gates.

### Deduplicate source text across all layers

Rejected as a goal. A shared source file can couple tests that need distinct
mutations, phase boundaries, or readability. Share a fixture only when its
semantic identity is intentionally common and independently documented.

## Decision summary

| Question | Decision |
| --- | --- |
| Is A33 a production architecture change? | No. It changes test-only ownership, plumbing, and documentation. |
| Is there one shared test framework? | No. Compiler unit, compiler integration, and golden integration support remain separate domains. |
| How is the determinism suite split? | One integration binary with a concise root case registry, one bounded subprocess harness, and responsibility-specific generator modules. |
| Are current test names preserved? | Preserve all 53 determinism leaf names; inventory other focused paths and retain or deliberately document changes. |
| What may helpers assert? | Only prerequisites before their named boundary and mechanical resource/process contracts. Semantic expectations stay in owner tests. |
| What happens to malformed MIR fixtures? | They remain explicit, MIR-owned, and capable of bypassing the production proofs they challenge. |
| How do golden tests share setup? | A low-level temporary-resource base is composed into higher-level fixtures; low-level tests retain direct APIs. |
| Are A01–A05 regressions reimplemented? | No. Their existing owning tests are preserved and included in the migration inventory. |
| May similar tests be deleted? | Only when owner, process, configuration, fixture, assertion, and failure-detection role are all the same. |
| What proves completion? | Characterized helper behavior, preserved inventories and observations, full repository and MSRV gates, and full golden determinism. |

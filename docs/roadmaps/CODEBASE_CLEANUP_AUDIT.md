# Codebase Cleanup Audit

Status: actionable audit; A01–A06, A35, and A38 completed on 2026-09-09. Turn
the chosen architectural findings into separate PR-sized implementation
roadmaps. No implementation roadmap depends on this document yet.

Audited: 2026-09-09, revision `ad4feb920d4b`.

Skald's most valuable cleanup is to strengthen ownership of decisions and
reduce the number of places that must change together. The repository already
has useful phase facades, stable identities, explicit lifecycle plans,
independent MIR verification, deterministic products, and extensive tests.
Preserve those foundations. A wholesale rewrite or indiscriminate merging of
IRs would discard much of what is working.

The immediate findings include reproducible compiler/process failures, a
standard-library nontermination case, and a missing test-suite gate. The next
architectural priorities are the resolver/type-checker dependency cycle,
resolver staging, and common MIR graph queries. Larger changes to optional
representations, alias analysis, and register allocation need explicit designs
and measurements.

## Scope, method, and limits

The audit covers every repository component: the six Rust crates, compiler
frontend and all subsequent phases, module loading, driver and CLI, backend,
C runtime, standard library, tests, benchmarks, scripts, samples, documentation,
and build orchestration. It combines a tracked-file inventory, dependency and
hotspot searches, focused reading of implementations and consumers, inspection
of tests and contracts, and bounded reproducers.

This is a repository-wide engineering audit, not a claim that every line of
every source file received a manual correctness review. Large algorithmic and
representation proposals are candidates, not demonstrated bugs. Performance
benefits are hypotheses unless explicitly identified as observed behavior;
there are no measured speedup claims here. This audit did not run `make check`
or the full golden corpus.

The tracked inventory gives a useful scale, including comments and tests:

| Area | Files | Lines |
| --- | ---: | ---: |
| Compiler `src/`, including colocated tests | 985 | 307,357 |
| Compiler integration tests | 3 | 4,239 |
| Golden runner crate | 69 | 13,951 |
| Binary64 crate | 13 | 1,259 |
| MIR measurement crate | 15 | 2,690 |
| Documentation checker crate | 6 | 675 |
| CLI crate | 4 | 750 |
| Runtime | 5 | 481 |
| Standard library, documentation, and licenses | 25 | 3,813 |
| Scripts | 5 | 725 |
| Shared test corpora and runtime harnesses | 727 | 38,789 |
| Samples | 10 | 267 |
| Documentation, including archives | 202 | 92,767 |

Counts come from `git ls-files` and text line counts, excluding build outputs.
File size guided inspection; it is not itself evidence of poor design. For
example, the MIR identity mapper already deliberately shares mutable and
immutable traversal, and the C runtime is small and cohesive.

The living [architecture](../compiler/README.md),
[phase contracts](../compiler/PHASES_AND_IR.md),
[grammar](../language/GRAMMAR.md), focused language contracts, and
[testing policy](../development/TESTING.md) are the baseline. The previous
[maintainability roadmap](../archive/MAINTAINABILITY_ROADMAP.md) already delivered
many basic decompositions; those are not proposed again as missing work.
The sibling Niflheim repository's overview and repository structure were read
for comparison. Its separation of semantic operations and backend IR is useful
inspiration, but its GC model and retained legacy backend are not templates for
Skald's cleanup.

## Grading

- **Priority:** P0 = address first because a modest source input aborts the
  compiler; P1 = next cleanup tranche; P2 = worthwhile after prerequisites or
  measurement; P3 = opportunistic or explicitly deferred. These are engineering
  priorities, not security severity labels.
- **Impact:** 5 = repository-wide or foundational; 4 = substantial component
  benefit; 3 = meaningful localized benefit; 2 = modest benefit; 1 = cosmetic.
  Impact includes future change cost, not just execution speed.
- **Effort:** XS = up to half a day; S = roughly 1–2 engineer-days; M = 3–5;
  L = 1–3 weeks; XL = multiple staged projects. These estimates include focused
  tests and documentation, assume repository familiarity, and are not delivery
  commitments. L/XL findings must be split before implementation.
- **Risk:** Low/Medium/High describes regression risk from the proposed change.
- **Evidence:** R = reproduced; O = directly observed in source/build wiring;
  C = design or performance candidate grounded in observed code. Confidence in
  an observation does not establish the payoff of its proposed replacement.
- **Benefits:** M = maintainability, E = extensibility, R = robustness,
  C = compiler/tool execution efficiency, N = generated-program efficiency.

## Ranked inventory

Identifiers are finding references within this audit, not implementation tasks.
Status is `Complete` only after the corresponding section records delivered
work and its validation; findings without that record remain `Open`.

| ID | Improvement | Status | Priority | Impact | Effort | Risk | Evidence | Benefits |
| --- | --- | --- | --- | ---: | --- | --- | --- | --- |
| [A01](#a01--bound-actual-expression-tree-depth-and-stack-usage) | Bound actual expression-tree depth and stack usage | Complete | P0 | 5 | M–L | Medium | R | R, E |
| [A02](#a02--drain-linker-pipes-concurrently) | Drain linker pipes concurrently | Complete | P1 | 4 | M | Medium | R | R, M |
| [A03](#a03--enforce-process-deadlines-through-pipe-completion) | Enforce process deadlines through pipe completion | Complete | P1 | 4 | M | Medium | R | R |
| [A04](#a04--bound-captured-process-and-output-file-bytes) | Bound captured process and output-file bytes | Complete | P1 | 4 | M | Medium | O | R, C |
| [A05](#a05--check-container-capacity-arithmetic) | Check container capacity arithmetic | Complete | P1 | 4 | S | Low | R | R |
| [A06](#a06--include-binary64-tests-in-repository-gates) | Include binary64 tests in repository gates | Complete | P1 | 4 | XS | Low | O | R |
| [A07](#a07--move-module-entry-selection-out-of-the-driver-layer) | Move module entry selection out of the driver layer | Complete | P1 | 3 | S | Low | O | M, E |
| [A08](#a08--remove-resolutions-dependency-on-type-checking) | Remove resolution's dependency on type checking | Complete | P1 | 5 | L | High | O | M, E, R |
| [A09](#a09--give-resolver-stages-explicit-products-and-publication) | Give resolver stages explicit products and publication | Complete | P1 | 5 | L | High | O | M, E, R |
| [A10](#a10--isolate-and-measure-semantic-range-discovery) | Isolate and measure semantic range discovery | Open | P2 | 4 | M–L | High | C | M, C |
| [A11](#a11--make-provisional-expression-type-queries-explicit) | Make provisional expression-type queries explicit | Open | P2 | 4 | M | Medium | O | M, E, R, C |
| [A12](#a12--consolidate-language-item-discovery-plumbing) | Consolidate language-item discovery plumbing | Open | P2 | 4 | M | Medium | O | M, E |
| [A13](#a13--share-structural-ast-walking-where-responsibilities-repeat) | Share structural AST walking where responsibilities repeat | Open | P2 | 3 | M | Medium | O | M, E, R |
| [A14](#a14--separate-object-view-planning-from-alias-argument-checking) | Separate object-view planning from alias-argument checking | Open | P1 | 4 | M–L | Medium | O | M, E, R |
| [A15](#a15--reassess-overlapping-optionalplace-families) | Reassess overlapping optional/place families | Open | P2 | 5 | XL | High | C | M, E, R |
| [A16](#a16--share-identical-primitive-semantic-descriptors) | Share identical primitive semantic descriptors | Open | P2 | 3 | M | Medium | C | M, E |
| [A17](#a17--reduce-copy-capability-fixed-point-reconstruction) | Reduce copy-capability fixed-point reconstruction | Open | P2 | 4 | M–L | Medium | C | C, M |
| [A18](#a18--reuse-structural-cfg-and-dominance-queries) | Reuse structural CFG and dominance queries | Open | P1 | 4 | M | Medium | O | M, C, R |
| [A19](#a19--reuse-analyses-within-an-immutable-mir-snapshot) | Reuse analyses within an immutable MIR snapshot | Open | P2 | 4 | L | High | C | C, M |
| [A20](#a20--factor-pipeline-observation-bookkeeping) | Factor pipeline observation bookkeeping | Open | P2 | 3 | M | Medium | O | M, R |
| [A21](#a21--make-the-shared-mir-traversal-easier-to-navigate) | Make the shared MIR traversal easier to navigate | Open | P2 | 3 | M | Medium | O | M, E, R |
| [A22](#a22--introduce-virtual-register-target-ir-when-justified) | Introduce virtual-register target IR when justified | Open | P3 | 5 | XL | High | C | N, E |
| [A23](#a23--develop-conservative-shared-effectalias-queries) | Develop conservative shared effect/alias queries | Open | P3 | 5 | XL | High | C | N, E, R |
| [A24](#a24--cache-provider-directory-listings-per-request) | Cache provider directory listings per request | Open | P2 | 3 | M | Medium | C | C, M |
| [A25](#a25--use-identity-indexed-lookup-for-resolved-bindings) | Use identity-indexed lookup for resolved bindings | Open | P2 | 3 | S–M | Low | O | C, M |
| [A26](#a26--split-large-dump-renderers-by-responsibility) | Split large dump renderers by responsibility | Open | P2 | 3 | M | Low | O | M, E |
| [A27](#a27--render-diagnostics-into-one-output-buffer) | Render diagnostics into one output buffer | Open | P3 | 2 | S | Low | O | C, M |
| [A28](#a28--restore-concise-facades-in-selected-hotspots) | Restore concise facades in selected hotspots | Open | P2 | 3 | M | Low | O | M, E |
| [A29](#a29--remove-obsolete-rollout-comments-and-broad-allowances) | Remove obsolete rollout comments and broad allowances | Open | P2 | 2 | S | Low | O | M, R |
| [A30](#a30--share-standard-library-bounds-normalization) | Share standard-library bounds normalization | Open | P2 | 3 | S–M | Medium | O | M, R |
| [A31](#a31--avoid-mandatory-string-to-array-copies-for-output) | Avoid mandatory string-to-array copies for output | Open | P2 | 4 | L | High | C | N, M |
| [A32](#a32--measure-and-reduce-mapvec-copy-traffic) | Measure and reduce Map/Vec copy traffic | Open | P2 | 4 | L | High | C | N |
| [A33](#a33--consolidate-test-plumbing-while-preserving-independent-checks) | Consolidate test plumbing while preserving independent checks | Open | P2 | 4 | M | Medium | O | M, R, C |
| [A34](#a34--establish-reproducible-cleanup-measurements) | Establish reproducible cleanup measurements | Complete | P1 | 4 | M | Low | O | C, N, R |
| [A35](#a35--preserve-snapshot-aggregations-saturation-flag) | Preserve snapshot aggregation's saturation flag | Complete | P2 | 2 | XS | Low | O | R |
| [A36](#a36--preserve-raw-compiler-stderr-in-golden-observations) | Preserve raw compiler stderr in golden observations | Open | P2 | 3 | S–M | Medium | O | R, M |
| [A37](#a37--simplify-literal-selection-and-bound-glob-matching) | Simplify literal selection and bound glob matching | Open | P3 | 2 | S | Low | O | C, R |
| [A38](#a38--refresh-current-behavior-and-shorten-active-indexes) | Refresh current behavior and shorten active indexes | Complete | P1 | 4 | S–M | Low | O | M, E |
| [A39](#a39--define-and-test-the-documentation-checkers-markdown-subset) | Define and test the documentation checker's Markdown subset | Open | P2 | 3 | S–M | Low | O | R, M |
| [A40](#a40--reconsider-the-measurement-tools-private-sha-256) | Reconsider the measurement tool's private SHA-256 | Open | P3 | 2 | S | Low | C | M, R |
| [A41](#a41--make-runtime-build-configuration-visible-in-artifacts) | Make runtime build configuration visible in artifacts | Open | P2 | 3 | S–M | Low | O | R, M |
| [A42](#a42--rename-sequential-execution-products-used-by-both-schedulers) | Rename sequential execution products used by both schedulers | Open | P3 | 2 | S | Low | O | M |
| [A43](#a43--add-narrow-automated-phase-dependency-checks) | Add narrow automated phase-dependency checks | Complete | P1 | 4 | M | Low | C | M, E, R |

## Immediate robustness and validation

### A01 — Bound actual expression-tree depth and stack usage

**Status:** Complete (2026-09-09).

**Evidence:** [expression parsing](../../crates/skald-compiler/src/syntax/parser/expression.rs)
previously built unbounded nested expression trees from flat operator chains.
[Parser limits](../../crates/skald-compiler/src/syntax/parser.rs) bounded
recursive grammar nesting, while the former logical-depth walk counted logical
operations rather than all expression-tree edges. Those limits did not
establish a bound on every tree consumed downstream.

**Observed failure:** the current debug `skac` aborts with stack overflow on
`fn main() -> i64 { return 1 + 1 + ...; }` with 1,000 terms. A 10,000-term case
also aborts. No nested parentheses, imports, or large allocation are required.
The reproducer establishes a whole-pipeline failure; it does not pinpoint the
first overflowing traversal or establish release-build thresholds.

**Change:** first give all accepted expression shapes a bounded traversal and
destruction strategy, or reject excessive actual tree depth with a structured
diagnostic before recursive consumers run. Audit construction, validation,
dumping, cloning, and dropping: a late depth check can itself drop an unsafe
tree. Then consider iterative walkers or arena storage only where they remove
the demonstrated limitation without making ordinary code harder to follow.

**First PR / validation:** retain flat arithmetic and postfix-chain cases in
subprocess robustness tests with an external watchdog. Test just below and
above the selected limit, malformed partial trees, and debug/golden/release
builds. `catch_unwind` cannot turn a stack-overflow abort into a diagnostic.
Preserve evaluation order and existing precedence.

**Delivered:** the parser now uses an exhaustive iterative expression walk for
both actual tree depth and logical depth. Binary and logical reductions plus
repeated unary and postfix wrappers are checked as they are constructed, so
even rejected temporary trees remain bounded; completed outer expressions
receive a defensive whole-tree check before leaving syntax. The existing
`PAR005` recovery omits the complete declaration, keeping partial trees out of
later phases. Unit tests fix the accepted/rejected boundary for additive,
multiplicative, bitwise, grouped, and postfix trees. A process-isolated
public-pipeline regression test gives 10,000-term arithmetic and postfix
chains, a nested mixed postfix shape, and malformed arithmetic a 15-second
external watchdog. The regression passes in debug and release profiles.
The full golden suite remains unchanged and passes all 626 leaves.

### A02 — Drain linker pipes concurrently

**Status:** Complete (2026-09-09).

**Evidence:** the former inline
[`execute_link`](../../crates/skald-compiler/src/driver/toolchain.rs) wrote all
assembly to child stdin before calling `wait_with_output` to drain stdout and
stderr. A child that filled stderr before consuming stdin could block against
the parent, which was blocked writing stdin.

**Observed failure:** a fake linker that writes 1 MiB to stderr, then reads
stdin, deadlocks when passed 1 MiB of assembly. The audit's external two-second
watchdog killed the process group.

**Change:** own input writing and both output drains concurrently, with cleanup
on every error path. Preserve the existing useful rule that a failed process
status takes precedence over a coincident broken stdin pipe. The existing
`LinkInvocation`/`LinkObservation` seam is the right policy boundary.

**First PR / validation:** exercise full pipes in both directions, early
rejection, successful early stdin closure, spawn failure, and output
publication failure. Share process mechanics only through a lower-level
utility if warranted; never make the compiler depend on `skald-golden`.

**Delivered:** host-linker process mechanics now live in a private toolchain
submodule. Scoped workers write assembly and drain stdout and stderr
concurrently while the owner waits for the child. Wait failures kill and reap
before worker collection, worker startup and I/O failures remain structured,
and a failed process status still takes precedence over a coincident broken
stdin pipe. Real-process tests cover 1 MiB in both output directions before
reading 1 MiB of input under an external watchdog, early rejection with
captured stderr, successful early stdin closure, spawn failure, and cleanup
after output publication failure. The full compiler suite, workspace static
checks, Rust 1.82.0 check, and all 626 golden leaves pass.

### A03 — Enforce process deadlines through pipe completion

**Status:** Complete (2026-09-09).

**Evidence:** [`run_process`](../../crates/skald-golden/src/process/runner.rs)
times out the direct child, then joins blocking stdin/stdout/stderr threads.
Once the child exits normally, the deadline is no longer enforced. Descendants
can still own inherited pipe handles. Wait/reap errors also return before
joining workers or completing child cleanup.

**Observed failure:** `/bin/sh -c '/bin/sleep 0.3 & exit 0'` with a 20 ms timeout
returns `Code(0)` after about 302 ms. Replacing the finite sleep with a
long-lived descendant can stall a worker indefinitely.

**Change:** define the deadline across child execution and pipe completion;
use an owning cleanup guard and cancellable/bounded I/O completion. Preserve
process-group termination on Linux, report cleanup failures, and specify how
descendants retaining pipes are classified. Killing a group only when the
direct child times out is insufficient.

**First PR / validation:** descendants retaining each output pipe, descendants
retaining stdin, large input, a child exiting before timeout, a real timeout,
and failure paths. Verify completion and cleanup, not only the exit enum.

**Delivered:** the golden process owner now treats direct-child termination and
all three pipe workers as one bounded operation. If any part remains incomplete
at the deadline, the observation is `TimedOut` even when the direct child
already exited successfully. On Linux, cleanup targets the saved process group,
then reaps the direct child and collects every worker; wait, termination, and
reap errors take the same cleanup path, with secondary cleanup failures retained
on `ProcessError`. Real-process regressions cover descendants retaining stdin,
stdout, and stderr, including 2 MiB of blocked input. Each descendant has a
finite two-second lifetime to bound the regression itself, while the fixed
runner returns within one second of a 100 ms deadline and verifies that the
descendant was removed. Existing tests continue to cover successful 2 MiB
bidirectional pipe traffic, ordinary early child exit, direct-child timeout,
signals, and process-group termination. The full runner suite, workspace static
checks, Rust 1.82.0 check, and all 626 golden leaves pass.

### A04 — Bound captured process and output-file bytes

**Status:** Complete (2026-09-09).

**Evidence:** [`read_pipe`](../../crates/skald-golden/src/process/runner.rs)
uses unbounded `read_to_end`; output-file comparison in
[`sandbox.rs`](../../crates/skald-golden/src/execute/sandbox.rs) uses `fs::read`.
A bounded worker count and time limit do not bound memory consumed by a noisy
compiler or generated program. Reports can retain additional copies.

**Change:** introduce explicit capture limits and an overflow observation,
with either bounded draining or spill-to-file storage. Continue draining or
terminate the process when a limit is crossed; merely stopping the reader can
create a pipe deadlock. Apply a separate limit to expected/observed files.

**First PR / validation:** sustained output on both pipes, binary output,
output exactly at the limit, and oversized files. Never silently truncate and
then allow an exact-byte expectation to pass. Choose defaults against the
existing corpus before making them part of the runner contract.

**Delivered:** every process now retains at most 4 MiB from each stdout and
stderr pipe while continuing to drain both streams through EOF. A typed
overflow observation records the pipe, limit, and complete byte count and is
propagated as an explicit compiler, linker, runtime, or native-run failure;
matching the retained prefix can no longer produce a passing result. Generated
assembly and each declared output-file expectation and observation have a
separate 32 MiB bounded read. Exact-boundary files pass, while oversized files
and assembly fail explicitly without exposing truncated assembly as a valid
artifact. Library callers may override limits through the owning process,
compiler, and execution options. The defaults leave substantial headroom over
the measured corpus maxima of 129,944 captured stream bytes and 9,515,537
assembly bytes. Unit and real-process tests cover binary prefixes, exact limits,
2 MiB on both pipes after the retained prefix fills, oversized expected and
observed files, prefix-equal expectations, failure propagation, and oversized
compiler assembly. The complete golden-runner suite, `make check`, Rust 1.82.0
check, and all 628 golden leaves pass.

### A05 — Check container capacity arithmetic

**Status:** Complete (2026-09-09).

**Evidence:** [`Map._normalized_capacity`](../../std/std/map.ska) doubles a
`u64` until it reaches the request. For a request above `2^63`, doubling wraps
to zero and never terminates. The audit compiled and ran
`Map<BoxU64, i64>.with_capacity(18446744073709551615u)`; execution exceeded the
one-second watchdog. This happens before the first backing-array allocation.

[`Vec._ensure_capacity`](../../std/std/vec.ska) also doubles unchecked, and
Map's load-factor/growth calculations multiply capacities. These neighboring
risks are source observations, not separately reproduced reachable failures.
[`io._next_read_capacity`](../../std/std/io.ska) already demonstrates explicit
overflow rejection.

**Change:** define representable element-count limits, check before addition,
multiplication, and conversion to `i64`, and fail with stable container-specific
panics. Compute normalized Map capacity once per constructor instead of four
times. Preserve the distinction between impossible capacity and allocation
failure.

**First PR / validation:** huge capacity requests must terminate without huge
allocations; retain zero/small capacity and power-of-two boundary behavior.
Extract arithmetic-only helpers if needed to test otherwise unreachable
growth boundaries. Do not alter Skald's wrapping integer language semantics.

**Delivered:** `Map` now accepts only capacities that normalize to an
`i64`-representable power of two: its largest table capacity is `2^62`, and a
larger request or growth step terminates with `Map: capacity too large` before
allocation. Capacity normalization is evaluated once by `with_capacity` and
passed into the private initializer. The three-quarter load check and
tombstone rehash check use subtraction/division comparisons instead of
overflow-prone multiplication. `Vec` accepts exact requested capacities
through `i64::MAX`, rejects larger counts with `Vec: capacity too large`,
checks before incrementing a full logical length, and saturates its final
geometric growth step at the representable maximum rather than wrapping.
Representable requests still reach the built-in array byte-layout and host
allocation checks, preserving their separate failures. Golden tests retain
zero and small exact Vec capacities, Map's minimum-eight and next-power-of-two
behavior, and ordinary growth. Maximum-`u64` Map and Vec requests now fail with
the container-specific panic under one-second process deadlines. The full
`make check` gate passes, including 3,089 compiler tests and all 628 golden
leaves.

### A06 — Include binary64 tests in repository gates

**Evidence:** [`Cargo.toml`](../../Cargo.toml) has six members, but
[`Makefile`](../../Makefile) `test-core` invokes behavioral tests for only five.
`skald-binary64` is compiled as a dependency, but its own unit, integration,
and documentation tests are not executed by `make check` or `make check-long`.

**Change:** add a named binary64 test target to `test-core`, help, and testing
guidance. Add a small workspace-member/gate coverage check, or adopt workspace
test execution if the existing suite-specific commands can be preserved
without duplicate runs.

**Validation:** the explicit binary64 command passed during this audit: 29
unit/integration tests and two compile-fail documentation tests. Inspect the
complete gate's command expansion to verify inclusion; compiling test targets
with `cargo check --all-targets` does not execute them.

**Delivered:** `test-core` now runs a single `cargo test --locked --workspace`
target plus the C runtime suite. Cargo therefore derives complete Rust test
coverage from the workspace manifest, including future members, without a
second package inventory or duplicate package runs. The existing focused
package targets remain independently runnable, and `make binary64-test` is now
listed in `make help` and the testing guidance. A dry run of `test-core`
confirmed the workspace-wide command, the focused binary64 target passed all
29 unit/integration tests and both compile-fail documentation tests, and the
full `make check` gate passed with all six workspace members, 3,089 compiler
unit tests, the direct runtime suite, and all 628 golden leaves.

## Frontend and semantic ownership

### A07 — Move module entry selection out of the driver layer

**Status:** Complete (2026-09-09).

**Evidence:** module graph
[`load.rs`](../../crates/skald-compiler/src/module/graph/load.rs) and
[`entry.rs`](../../crates/skald-compiler/src/module/graph/entry.rs) import
`driver::EntrySelector`, while the
[driver request model](../../crates/skald-compiler/src/driver/request.rs)
depends on module paths/providers. A lower-level loader therefore depends on
the orchestration layer for its input vocabulary.

**Change:** let `module` own the file/logical entry selector or a focused
module-loading request. Re-export it from `driver` where existing public paths
are useful. Keep artifact policy and optimization options in the driver.

**First PR / validation:** move the type and imports without semantic changes;
exercise positional, logical, singleton, ambiguous, and invalid entries.
This is a small, low-risk boundary improvement independent of A08.

**Delivered:** the `module` facade now owns `EntrySelector` and
`EntrySelectionError` in a focused entry model beside logical paths, providers,
and graph loading. Module graph implementation and test support import the
module-owned vocabulary directly, leaving the complete `module` tree free of
driver dependencies. `CompilationRequest` consumes that selector while the
`driver` facade re-exports both types so existing workspace callers retain
their public paths. The selector's option-validation test moved to its module
owner, and public API coverage verifies that the module and compatibility
driver paths are the same types. Living architecture and driver documentation
now state the ownership and compatibility boundary.

All 34 module graph tests pass, including positional/logical equivalence,
outside-root singleton behavior, singleton ambiguity, overlapping-root
ambiguity, and invalid positional entries. The public API suite,
documentation validation, full `make check` gate with all 628 golden leaves,
and the Rust 1.82.0 workspace check pass.

### A08 — Remove resolution's dependency on type checking

**Status:** Complete (2026-09-09).

**Original evidence:** specialization
[`validation.rs`](../../crates/skald-compiler/src/resolve/resolver/program/specialization/validation.rs)
called `crate::typeck::failed_specialization_requirements` and named
`typeck::CopyPathElement`;
[`interface_validation.rs`](../../crates/skald-compiler/src/resolve/resolver/program/specialization/interface_validation.rs)
called the interface equivalent. The query lived in type checking, used HIR
types, and lazily computed copy capabilities. Resolution consequently reached
into a later phase even though the intended architecture is forward.

**Change:** make closed-type eligibility and lifecycle capability facts a
semantic service with phase-neutral results, or add an explicit closure
validation stage between resolution and HIR construction. Keep HIR plan
construction in type checking. Extend the existing
[`type_capabilities`](../../crates/skald-compiler/src/type_capabilities/mod.rs)
vocabulary where suitable rather than duplicating the language matrix.

**First PR:** define the neutral query/result boundary, move one capability
family, and preserve diagnostics and atomic rejection of invalid generated
declarations. Subsequent PRs migrate lifecycle facts and remove reverse imports.
**Validation:** class/interface bounds, recursive aggregate capabilities,
repeated application diagnostics, ordinary/generic parity, and HIR erasure.
Do not simply move the whole type checker under a new module name.

**Delivered:** `type_capabilities` now owns the closed-subject query,
phase-neutral class/array lifecycle availability, and lifecycle diagnostic
paths over `ResolvedProgram`. Class and interface specialization validation
call that service directly; the former type-checker query module and all
production resolver imports of `typeck` are gone. Type checking retains
concrete HIR copy/assignment plan construction and consumes the shared
phase-neutral diagnostic path vocabulary. Obsolete type-check-only query
helpers were removed.

The migrated capability tests cover declaration roles, default construction,
shared targets, class/interface subjects, nested optionals, arrays, and
unavailable lifecycle operations. A parity test compares every resolved class
and canonical array capability with HIR plan availability, including recursive
optional/array dependencies. A source-boundary integration test prevents new
production resolution dependencies on type checking. Architecture and generic
class/interface documentation now record the neutral service and the HIR plan
boundary.

The full `make check` gate passed with all six workspace members, 3,090
compiler unit tests, documentation tests, the direct runtime suite, and all
628 golden leaves. The Rust 1.82.0 workspace all-target check also passed.

### A09 — Give resolver stages explicit products and publication

**Evidence:** [`ProgramResolver::resolve`](../../crates/skald-compiler/src/resolve/resolver/program/resolver.rs)
is roughly 600 lines of coupled ordering: collect declarations and bindings,
validate language items, discover and materialize specializations, rebuild
hierarchy/dispatch, resolve bodies, finish interning, and validate publication.
Specialization failure restores saved ordinary tables and clears generated
definitions/virtual families in
[`validation.rs`](../../crates/skald-compiler/src/resolve/resolver/program/specialization/validation.rs).

**Change:** introduce a few named stage products around existing responsibility
boundaries, such as collected declarations, closed candidate declarations,
resolved bodies, and validated publication. Keep candidate state separate from
the published result so rollback does not require remembering every mutated
table. Factor repeated construction of body/language-item environments.

**First PR:** extract collection and its product without changing IDs; follow
with candidate publication and then body orchestration. Coordinate the contract
with A08 before moving lifecycle validation. **Validation:** failed dependent
specializations, ordinary declarations surviving errors, dispatch consistency,
module-order determinism, and byte-identical successful resolved dumps.

**Delivered:** whole-program resolution now passes a named collected-declaration
product into the remaining resolver orchestration and packages completed
function and class bodies before final assembly. One shared body-resolution
stage constructs the declaration, module-context, literal, iteration,
operator, and range environments used by static initializers, specialized
bodies, ordinary functions, and ordinary classes.

Final specialization selection is owned by a consuming candidate-publication
boundary. Class and interface validators inspect immutable candidates and
return validation results; the publication owner alone restores saved ordinary
class, interface, and hierarchy products, clears rejected body and dispatch
products, and marks rejected identities failed. Selection remains ordered by
dependency: rejected class products invalidate dependent interfaces, while
independent valid class or interface products remain published.

Regression coverage verifies that an invalid interface candidate preserves an
independent valid class specialization and its bodies. Existing coverage also
exercises failed dependent specializations, restoration of ordinary
declarations, dispatch-product clearing, source-order permutation, and stable
resolved dumps. The full `make check` gate passed with all workspace tests,
3,091 compiler unit tests, documentation and runtime checks, the 53-process
determinism suite, and all 628 golden leaves. The Rust 1.82.0 workspace
all-target check also passed.

### A10 — Isolate and measure semantic range discovery

**Evidence:**
[`complete_semantic_range_specializations`](../../crates/skald-compiler/src/resolve/resolver/program/semantic_range_requests.rs)
builds provisional classes/hierarchy, clones the type interner in a loop, and
runs diagnostic-isolated semantic discovery before final body resolution.
This is a concrete example of repeated frontend work and delicate provisional
state, not evidence that the current range semantics are wrong.

**Change:** make discovery output an explicit request delta/worklist with a
termination invariant. Measure rounds, bodies revisited, and interner copies.
If material, revisit only affected bodies and reuse immutable declaration
inputs. Keep discovery diagnostics separate from authoritative diagnostics.

**First PR / validation:** add measurements and document the fixed-point
contract; optimize only after a baseline. Test local endpoint bindings,
generated bodies, nested class/interface applications, repeated origins, and
no false range classification for ordinary expressions. Depends on A09's
staging decisions; do not create a second specialization engine.

### A11 — Make provisional expression-type queries explicit

**Evidence:** [`resolved_expression_type`](../../crates/skald-compiler/src/resolve/resolver/body/call.rs)
recursively predicts types for member/protocol selection. For example, an
unselected arithmetic binary expression inherits the left operand's kind;
`Option<ResolvedTypeKind>` also represents cases with no available answer.
The type checker later performs authoritative validation.

**Change:** name and document this as a provisional shape/type query. Distinguish
unknown, known candidate, and invalid selection where callers need the
distinction. Centralize the query and cache binding facts rather than letting
each new protocol add another informal mini type checker. Some name selection
needs type information; the goal is explicit ownership, not removing it blindly.

**First PR / validation:** characterize invalid operands used as receivers,
optional injection, overloaded operators, and generic witnesses. Preserve the
owner and order of resulting diagnostics. Follow with A25's indexed lookup.

### A12 — Consolidate language-item discovery plumbing

**Evidence:**
[`resolver.rs`](../../crates/skald-compiler/src/resolve/resolver/program/resolver.rs)
has separate span collection for iteration, operators, and ranges; canonical
paths also appear in
[`compiler_dependency_path`](../../crates/skald-compiler/src/module/graph/load.rs).
String, iterable, operator, range, and
[intrinsic validation](../../crates/skald-compiler/src/resolve/resolver/program/intrinsic_registry.rs)
have overlapping discovery/provenance scaffolding.

**Change:** use a small typed catalog for canonical paths, dependency kinds,
and diagnostic origin collection. Keep each protocol's actual structural
validator separate: their requirements differ. Build one body language-item
context after validation and pass it consistently.

**First PR / validation:** centralize canonical identity metadata, then migrate
one repeated collection path. Cover disabled/replaced standard-library roots,
malformed canonical declarations, imports versus implicit dependencies, and
the `std::str`/`std::error` cycle. Lower phases must continue consuming IDs and
plans, never canonical source-name lookup.

### A13 — Share structural AST walking where responsibilities repeat

**Evidence:**
[compiler dependency collection](../../crates/skald-compiler/src/module/graph/compiler_dependencies.rs),
[expression-depth validation](../../crates/skald-compiler/src/syntax/parser/expression_depth.rs),
and the specialization
[source request scanner](../../crates/skald-compiler/src/resolve/resolver/program/specialization/requests/source_request_scanner.rs)
each traverse source structure. Every new statement/expression form expands
the review surface for omissions.

**Change:** provide small syntax-owned structural walkers with explicit child
order and prune/continue control where at least two consumers agree. Keep
binding scopes, depth accounting, and dependency meaning with the consumer.
Prefer iterative traversal where it also addresses A01.

**First PR / validation:** migrate two non-semantic walkers and use a source
containing every current child-bearing expression/statement family. Preserve
lexical origin order and source spans. Avoid a generic visitor framework for
all compiler IRs.

### A14 — Separate object-view planning from alias-argument checking

**Evidence:**
[`typeck/expression/alias.rs`](../../crates/skald-compiler/src/typeck/expression/alias.rs)
is 1,689 lines and owns primitive aliases, shared-owner aliases, optional
aliases, casts, produced views, iteration views, ancestor projections, and
diagnostic rendering. The existing
[`object_view_relation`](../../crates/skald-compiler/src/typeck/expression/object_view_relation.rs)
is a useful narrower seam.

**Change:** separate source classification, target/access relation, lifetime
and anchor planning, and context-specific diagnostics. Reuse a typed view plan
for receivers, aliases, and iteration where their rules agree, with explicit
context rather than new booleans for every feature.

**First PR / validation:** extract produced/borrowed view-source planning behind
the current API; then migrate one consumer. Preserve non-exclusive aliases,
read-only versus mutable access, owner anchoring, evaluation order, and exact
destruction timing. Existing alias, cast, produced-receiver, and iteration
matrices should constrain the refactor.

### A15 — Reassess overlapping optional/place families

**Evidence:** [HIR optionals](../../crates/skald-compiler/src/hir/ir/optional.rs)
contain primitive, class, shared, and canonical-identity optional places and
sources. [MIR instructions](../../crates/skald-compiler/src/mir/model/instruction.rs)
retain generic, aggregate, class, and shared optional instruction families;
[`mir/lower/optional.rs`](../../crates/skald-compiler/src/mir/lower/optional.rs)
and target lowering handle their intersections. Recursive optional plans
already exist, so this is an overlap audit, not a proposal to invent them.

**Change:** inventory which distinctions encode different runtime work and
which are historical convenience wrappers around the same canonical plan.
Consolidate only the latter. Keep scalar values, aggregate storage, shared
ownership, checked views, and publication/cleanup evidence distinct wherever
they carry different invariants.

**First PR:** produce a representation/producer/consumer matrix and select one
redundant family for migration. **Validation:** recursive payloads, tagged
arrays, zero-niche owners, aliases, copies, failure paths, and malformed MIR.
Do not hide ownership differences inside a universal untyped place enum.

### A16 — Share identical primitive semantic descriptors

**Evidence:** HIR and MIR each define primitive cast kinds, integer kinds,
comparison predicates, shift direction, and division semantics; see
[HIR primitives](../../crates/skald-compiler/src/hir/ir/primitive.rs),
[MIR primitives](../../crates/skald-compiler/src/mir/model/primitive.rs), and
[cast lowering](../../crates/skald-compiler/src/mir/lower/primitive.rs).
Several mappings translate identical semantic cases one-for-one.

**Change:** consider phase-neutral descriptors for genuinely identical
operations and arithmetic rules, with explicit re-exports to retain useful
paths. Keep source operators, HIR operand plans, MIR value/control-flow
operations, and target instructions separate. The existing binary64 facade is
a successful example of one semantic authority.

**First PR / validation:** migrate one small descriptor, such as comparison
predicate or shift direction, and confirm exhaustive coverage. Keep integer
wrapping, checked failure behavior, NaN handling, and dump spellings unchanged.
Stop if the abstraction increases coupling more than it removes duplication.

### A17 — Reduce copy-capability fixed-point reconstruction

**Evidence:** [`CopyCapabilities::compute`](../../crates/skald-compiler/src/typeck/capabilities.rs)
clones capability sets and rebuilds array lifecycle tables in separate
constructor/assignment convergence loops. Generic requirement queries can
compute this information before ordinary HIR checking computes it again.

**Change:** after A08 fixes ownership, expose immutable capability views so
provisional array analysis does not require owned clones. Measure iterations
and cloned records; consider dependency-driven invalidation only if chains of
classes, arrays, and optionals make repeated full reconstruction material.

**First PR / validation:** remove avoidable provisional ownership while keeping
the current solver, then compare facts and failure paths on recursive/missing
copy operations. Preserve deterministic diagnostic paths and constructor-before-
assignment dependencies. Do not introduce an arbitrary iteration cap.

## MIR, passes, and backend

### A18 — Reuse structural CFG and dominance queries

**Evidence:**
[`mir/verify/checked_scalar.rs`](../../crates/skald-compiler/src/mir/verify/checked_scalar.rs)
builds predecessor sets and answers dominance with two graph searches per
query. [Logical topology](../../crates/skald-compiler/src/passes/pipeline/optimizations/logical_topology.rs)
has another predecessor builder. Rich edge-preserving facts already exist in
[`mir/rewrite/cfg.rs`](../../crates/skald-compiler/src/mir/rewrite/cfg.rs).

**Change:** extract structural graph facts into a neutral MIR analysis owner
that both verification and rewrite planning can consume. Cache dominance per
callable when repeated queries justify it. Retain successor-edge identity:
two edges from one conditional to the same target are not interchangeable
with one predecessor-set member for every transformation.

**First PR / validation:** share predecessor construction with explicit
set-versus-edge APIs; then replace repeated dominance searches. Verify
disconnected components, malformed IDs, loops, self-edges, and protected roots.
Verifier inputs are untrusted; never require an already verified rewrite
snapshot merely to perform initial structural verification.

### A19 — Reuse analyses within an immutable MIR snapshot

**Evidence:** the
[default schedule](../../crates/skald-compiler/src/passes/pipeline/policy/profile.rs)
contains repeated folding and cleanup passes. Individual
[algebraic simplification](../../crates/skald-compiler/src/passes/pipeline/optimizations/primitive_algebraic_simplification.rs)
and constant-folding plans compute local constant solutions; changed outputs
are reverified by the
[runner](../../crates/skald-compiler/src/passes/pipeline/execution/runner.rs).
[Final sealing](../../crates/skald-compiler/src/passes/pipeline/seal.rs)
recomputes reachability and lifecycle checks.

**Change:** profile pass analysis versus transformation versus verification.
Start with borrowed, lazy facts scoped to one immutable callable/program
snapshot. Drop them on mutation. Only later consider explicit revision keys
and invalidation summaries for unaffected callables.

**First PR / validation:** measure recomputation and reuse one read-only fact
inside a pass. Verify that changed CFGs, identities, static effects, and
retention cannot reuse stale facts. Preserve mandatory verification, fresh
reachability, pass selection, checkpoints, and independent seals. This is not
permission to skip verification or remove repeated passes without evidence.

### A20 — Factor pipeline observation bookkeeping

**Evidence:**
[`execution/runner.rs`](../../crates/skald-compiler/src/passes/pipeline/execution/runner.rs)
repeats timing, statistics, occurrence-record construction, and failure
handling across proof-rich and final-stage loops. Their transformation and
resealing authorities are correctly different.

**Change:** extract small observation/failure-recording helpers while keeping
stage-specific execution functions and capability types explicit. Avoid a
generic runner that erases proof consumption or permits invalid stage order.

**First PR / validation:** factor record creation; compare enabled/disabled
reporting, changed/unchanged/error outcomes, occurrence numbering, inspection
order, and observer-independent artifacts.

### A21 — Make the shared MIR traversal easier to navigate

**Evidence:** [`mir/rewrite/map.rs`](../../crates/skald-compiler/src/mir/rewrite/map.rs)
is 2,492 lines, but importantly already defines one structural inventory for
mutable mapping and read-only observation. It includes typed identity roles
that rewrites and analyses depend on.

**Change:** divide the inventory into cohesive callable, instruction, place,
and attachment sections/modules if this improves navigation. Preserve one
source for the structural inventory, exhaustive matches/destructuring, and
distinct definition/use/authorization roles. Avoid separately maintained
mutable and immutable visitors.

**First PR / validation:** extract one family with identity-preserving output;
run mapper/observer parity, identity remapping, and malformed-reference tests.
Coordinate public fact ownership with A18. Do not replace explicit safety
classification with a permissive default branch.

### A22 — Introduce virtual-register target IR when justified

**Evidence:** the target
[machine model](../../crates/skald-compiler/src/backend/x86_64_sysv/machine.rs)
is already typed, but uses physical registers;
[frame planning](../../crates/skald-compiler/src/backend/x86_64_sysv/frame.rs)
assigns fixed homes before instruction selection. The limitation is not
unstructured assembly strings. It is the absence of a representation suitable
for register allocation and liveness-based placement.

**Direction:** retain typed virtual registers, explicit memory/ABI effects,
calls, trace barriers, and ownership operations before final physical
assignment. Begin with scalar leaf functions and a measurable stack-traffic
baseline. This is a substantial performance project, not a prerequisite for
ordinary cleanup.

**Planning/validation:** use the existing
[architecture discovery](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md#5-direct-physical-register-backend-lowering)
as the authoritative design backlog. Require ABI, register-clobber, alignment,
trace, native equivalence, and spill tests before enabling allocation.

### A23 — Develop conservative shared effect/alias queries

**Evidence:** static-lifecycle and reachability passes already describe
callable dependencies, while local redundancy analyses have their own
barriers. Skald's aliases are deliberately non-exclusive, and destruction
and shared ownership are observable.

**Direction:** start with conservative reusable summaries of reads, writes,
calls, allocation, termination, and ownership effects. Add alias precision
incrementally. Consumers should ask focused queries rather than each inventing
their own safety model. Do not merge independent verification with the analysis
it is intended to check.

**Planning/validation:** tracked in the existing
[alias/effect discovery](OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md#7-conservative-alias-effect-and-ownership-knowledge)
and [optimization catalog](OPTIMIZATION_CANDIDATE_CATALOG.md). A first milestone
needs conservative fallback and overlapping-alias/destructor/callback tests.
This audit does not propose changing alias semantics, whole-world checking,
or the single-threaded language contract to make optimization easier.

## Local efficiency and code organization

### A24 — Cache provider directory listings per request

**Evidence:** [`probe_provider`](../../crates/skald-compiler/src/module/provider/lookup.rs)
reads each directory component for every logical module/provider lookup;
`select_directory_component` sorts the entire listing before selection.
Related modules repeatedly inspect common roots. A successful probe also
opens the file for readability before loading later reads its contents.

**Change:** measure filesystem operations for many sibling/nested imports,
then cache directory-name indexes for the duration of loading. Preserve exact
case preference, case-mismatch/collision reporting, provider ambiguity, error
precedence, symlink behavior, and deterministic ordering. Define the request's
filesystem-change semantics before caching failures or reusing open handles.

**First PR / validation:** cache successful directory listings only, with the
existing provider tests plus many-sibling imports and filesystem errors.
Avoid a persistent filesystem cache or import-order-dependent first-match
shortcut. No measured speedup is claimed.

### A25 — Use identity-indexed lookup for resolved bindings

**Evidence:** the binding branch of
[`resolved_expression_type`](../../crates/skald-compiler/src/resolve/resolver/body/call.rs)
searches all scope-map values for an already selected `BindingId`. Name lookup
and identity lookup are different responsibilities. This repeated scan sits
on member/protocol resolution paths.

**Change:** retain a callable-local table of parameter/local type facts indexed
by their stable IDs; use scope maps only to select names. Keep receiver facts
explicit. For module path lookup, the separate linear
[`ProgramModuleTable::find`](../../crates/skald-compiler/src/module/metadata.rs)
is another measurement candidate, but does not justify a repository-wide
replacement of vectors with maps.

**First PR / validation:** index binding facts and check shadowing, nested
scopes, generic substitutions, and invalid IDs. Preserve deterministic
allocation and avoid duplicating authoritative mutable type state.

### A26 — Split large dump renderers by responsibility

**Evidence:** [HIR dumping](../../crates/skald-compiler/src/hir/dump.rs) has
3,122 lines, [resolved dumping](../../crates/skald-compiler/src/resolve/dump.rs)
2,747, and [MIR dumping](../../crates/skald-compiler/src/mir/dump.rs) 2,100.
Declaration, type, expression, lifecycle, and body formatting have grown
together. Shared byte/span/indentation helpers already exist in
[`dump_format.rs`](../../crates/skald-compiler/src/dump_format.rs).

**Change:** split each phase's renderer by these responsibilities behind its
current entry point. Reuse small formatting primitives, but do not create one
generic dump model that obscures phase-specific semantics.

**First PR / validation:** split one phase at a time; require byte-identical
dumps and independent-process determinism. File length alone is not a reason
to redesign the dump format or replace exact expectations.

### A27 — Render diagnostics into one output buffer

**Evidence:** [`render_diagnostics`](../../crates/skald-compiler/src/diagnostics/render.rs)
renders each diagnostic into a separate `String`, collects a vector, then
joins it. All renderers already use `fmt::Write` internally.

**Change:** add a private append/write entry point and keep the public
single-diagnostic `String` convenience wrapper. Append separators in the
outer loop. Profile source-location column scans separately before considering
additional caches.

**First PR / validation:** exact multi-diagnostic output, UTF-8, tabs, empty
diagnostics, and invalid-span fallback. This is a small allocation/readability
improvement, not an expected major compiler speedup.

### A28 — Restore concise facades in selected hotspots

**Evidence:** [`typeck/program/mod.rs`](../../crates/skald-compiler/src/typeck/program/mod.rs)
mixes orchestration, diagnostic codes, type conversion, and declaration
validation. Resolver child files often inherit implementation imports through
`use super::*`; for example,
[`program/mod.rs`](../../crates/skald-compiler/src/resolve/resolver/program/mod.rs).
This makes ownership less visible during an unfamiliar change.

**Change:** extract cohesive implementations and keep the facade's public API
and stage outline easy to scan. Prefer explicit imports at responsibility
boundaries; avoid converting every small sibling module into a verbose import
list. Keep implementation files private and avoid widening visibility merely
to make a move compile.

**First PR / validation:** extract program diagnostics/type conversion as one
bounded change, or apply this during A09/A14. Compile all public paths and run
owner tests. Do not impose one-type-per-file or zero-logic `mod.rs` rules.

### A29 — Remove obsolete rollout comments and broad allowances

**Evidence:** [`passes/mod.rs`](../../crates/skald-compiler/src/passes/mod.rs)
says reachability is awaiting its first retention/backend consumers, which
already exist. It and [`mir/mod.rs`](../../crates/skald-compiler/src/mir/mod.rs)
have broad `allow(dead_code, unused_imports)` attributes. Generic template and
identity comments also describe already activated consumers. The lexer still
exports an integer-only
[compatibility name](../../crates/skald-compiler/src/lexer/scanner.rs) for the
numeric-literal error.

**Change:** remove stale staging comments, try removing broad allowances, and
scope any legitimate remaining suppression to its item with a current reason.
Inventory compatibility aliases and their callers before removal; the crate
is repository-internal, but downstream workspace tools still matter.

**First PR / validation:** one owner at a time, all-target Clippy and public API
tests. Preserve diagnostic codes and documented names even when internal Rust
names change. Historical wording in archives should remain historical.

## Standard library, tests, and repository tools

### A30 — Share standard-library bounds normalization

**Evidence:** [`Vec`](../../std/std/vec.ska) repeats negative-index and slice
bound normalization in get/set operations; [`Str`](../../std/std/str.ska)
contains the same signed-index policy with different ownership afterward.

**Change:** introduce a small dependency-light helper returning normalized
indices/bounds, or first share helpers within each class. Preserve each public
operation's error message and the difference between copied Vec slices and
shared Str slices. Avoid pulling container dependencies into low-level helpers
and creating new canonical-module cycles.

**First PR / validation:** consolidate Vec's own bounds logic, then evaluate
cross-module reuse. Cover negative extremes, omitted endpoints, empty/end
slices, reversed bounds, overlapping assignment, and exact diagnostics.

### A31 — Avoid mandatory string-to-array copies for output

**Evidence:** [`write_stdout` and `write_stderr`](../../std/std/io.ska) call
`Str.to_bytes()` before every write; [`to_bytes`](../../std/std/str.ska) returns
an array slice copy. `_write_stdout_line` repeats this path for the newline.
Reading also constructs an intermediate byte array before `Str.from_bytes`.

**Change:** investigate a bounded read-only byte view or a focused internal
string I/O bridge that can retain the owner and pass a pointer/length without
exposing mutable backing or raw pointers to ordinary source. The C runtime
already consumes pointer/length; the missing contract is above that boundary.

**First PR:** count allocations/copies and specify the lifetime/private-storage
contract before implementation. **Validation:** empty and sliced strings,
embedded NUL and non-UTF-8 bytes, partial writes, failed writes, and owner
lifetime. Keep public `to_bytes` copy semantics unchanged. This may require
language-item/intrinsic work and is not just a local replacement of one call.

### A32 — Measure and reduce Map/Vec copy traffic

**Evidence:** [`Map._rehash`](../../std/std/map.ska) snapshots four backing
arrays, unwraps entries into owning locals, and passes them into `_insert_new`;
`_insert_new` takes an owning key even when some callers hold a read-only alias.
[`Vec`](../../std/std/vec.ska) grows and slices through array copy/assignment.
Skald's copy constructors can be observable, so fewer source temporaries do
not automatically mean a valid optimization.

**Change:** benchmark primitive and owning element types, count actual
copy/destruction operations, and remove only proven redundant traffic. A
read-only key parameter for internal insertion is a candidate, not a blanket
solution; overload and lifecycle behavior must be checked. Bulk ownership
transfer would need an explicit language/compiler contract.

**First PR / validation:** add a focused Map rehash benchmark beside the
existing Vec benchmark and freeze collision, tombstone, replacement,
self-aliasing, and destructor observations. Fix A05 first. Keep representation
changes separate from generic optimizer work in the catalog.

### A33 — Consolidate test plumbing while preserving independent checks

**Evidence:** compiler
[pipeline determinism tests](../../crates/skald-compiler/tests/pipeline_determinism.rs)
span 2,913 lines; pipeline tests and reporting tests also contain large
feature-specific fixture groups. Compiler
[`test_support`](../../crates/skald-compiler/src/test_support.rs) and
[MIR fixtures](../../crates/skald-compiler/src/mir/test_fixtures.rs) already
provide reusable foundations. Golden process tests retain additional temporary
fixture and subprocess setup.

**Change:** split large suites by responsibility and share repeated source-to-
checkpoint, temporary directory, subprocess, and assertion plumbing within the
appropriate test owner. Keep negative MIR builders capable of deliberate
corruption; tests must not become consumers of the same production proof that
they are supposed to challenge. Use complete golden behavior for cross-phase
contracts and local tests for phase invariants.

**First PR / validation:** migrate one repeated fixture family and compare the
discovered test count. Add subprocess crash/nontermination cases from A01–A05.
Preserve intentionally independent-process determinism and native equivalence
checks; do not delete similar-looking tests solely to shorten the suite.

### A34 — Establish reproducible cleanup measurements

**Status:** Complete (2026-09-09).

**Evidence:** [range](../../scripts/measure_range_loops.py) and
[Vec](../../scripts/measure_generic_vec.py) scripts duplicate timing and error
helpers, use fixed build directories, and launch subprocesses without
timeouts. The [trace benchmark](../../scripts/measure_panic_runtime_trace.py)
has a separate harness. Current Make benchmark targets build debug `skac`.
The [MIR measurement tool](../../crates/skald-mir-measure/src/measure.rs)
already offers structured checkpoints and optional operational context.

**Change:** retain workload-specific metrics but share minimal process/timing/
artifact helpers. Record compiler profile, revision/dirty status, target,
runtime trace policy, resolved pass schedule, input size, and repetitions.
Use unique run directories and watchdogs. Pair or alternate native variants
to reduce order effects; record dispersion as well as medians.

**First PR:** capture a baseline with small source, many modules, many generic
applications, large callable CFGs, nested ownership, and the existing native
workloads. Separate compiler wall time/peak memory from native time and code
size. **Validation:** repeated runs preserve semantic results and deterministic
metrics, timing noise does not fail ordinary `make check`, and operational
metadata is excluded from deterministic projections. Use this before A10,
A17, A19, A22, A24, A31, or A32.

**Delivered:** a documented `make cleanup-baseline` procedure now measures 19
reviewed compile and native workloads with the optimized assertion-enabled
compiler. Reports record repository/compiler identity, target, trace policy,
source inventory, the resolved 17-occurrence MIR schedule, repetition policy,
deterministic assembly hashes and sizes, and repeated native-result digests.
Compiler wall time and peak RSS, executable-build cost, and native timing live
in a separate operational section with medians and median absolute deviations.
Every invocation uses a unique artifact directory and every child process has
a process-group watchdog. Paired native variants alternate execution order.
The existing range, generic-Vec, and panic-trace tools reuse the same bounded
process, timing, ordering, and artifact helpers while retaining their focused
metrics; Make targets now consistently use the `golden` compiler profile.

The helper unit suite covers timing dispersion, alternating order, unique run
directories, process-group deadlines, and exclusion of operational metadata
from deterministic projections. The complete 19-workload baseline preserved
repeated assembly and native semantics. Focused measurements for all three
existing scripts, documentation validation, `git diff --check`, and the full
`make check` gate pass, including all 628 golden leaves.

### A35 — Preserve snapshot aggregation's saturation flag

**Status:** Complete (2026-09-09).

**Evidence:** [`merge_snapshot`](../../crates/skald-mir-measure/src/aggregate.rs)
passes `&mut target.saturated` to `merge_named_pairs`, then overwrites the flag
with an OR of child-category and source flags. Overlap-only overflow can thus
produce a saturated count with `saturated == false`. Existing tests exercise
the arithmetic helper and candidate aggregation, not this complete path.

**Change / first PR:** make snapshot saturation sticky, including flags set
while merging overlaps and earlier snapshots. **Validation:** merge overlap
counts `u64::MAX` and `1` with otherwise unsaturated inputs, then merge zero;
both the snapshot and total report must retain the flag. This is a directly
observed arithmetic-reporting defect, not a claim that normal corpora approach
this count.

**Delivered:** snapshot aggregation now accumulates the existing saturation
state when folding structure, candidate, overlap, and source flags. An overlap
regression drives the complete workload-to-totals path with `u64::MAX`, then
`1`, then `0`; the count remains clamped, and both the checkpoint snapshot and
top-level totals remain marked saturated. The measurement contract documents
that sticky behavior. The focused measurement suite, full `make check` gate,
Rust 1.82.0 check, and all 628 golden leaves pass.

### A36 — Preserve raw compiler stderr in golden observations

**Evidence:** compile-fail handling in
[`compile/invoke.rs`](../../crates/skald-golden/src/compile/invoke.rs) mutates
`ProcessObservation` through `strip_stderr_prefix`. The
[implementation](../../crates/skald-golden/src/process/model.rs) replaces every
non-overlapping occurrence of the bytes, rather than just one leading prefix.
The stored observation therefore loses the original child bytes.

**Change:** preserve raw process output and make normalized comparison bytes
an explicit expectation-layer view. Give the replacement operation an honest
name and document whether it is path normalization or general byte removal.
Do not silently change existing matcher semantics during the ownership move.

**First PR / validation:** repeat path occurrences, binary bytes, matching text
inside diagnostic messages, and raw versus normalized report views. Preserve
current golden expectations and deterministic comparison behavior.

### A37 — Simplify literal selection and bound glob matching

**Evidence:** [`selection/glob.rs`](../../crates/skald-golden/src/selection/glob.rs)
allocates a `(pattern length + 1) × (value length + 1)` memo table for every
match and recurses over byte positions, including literal patterns.

**Change:** add direct equality for patterns without stars. If profiling or
long-input tests justify more, tokenize patterns once and use bounded iterative
matching with rolling state. Keep the deliberately small `*`/`**` grammar and
its `/` and `:` component boundaries; a third-party glob language may have
different semantics.

**First PR / validation:** literal, empty, component, recursive, repeated-star,
and long patterns, plus existing canonical leaf selection tests. Avoid a
large selection framework for a small local optimization.

### A38 — Refresh current behavior and shorten active indexes

**Status:** Complete (2026-09-09).

**Evidence:** the root [README](../../README.md) says the compiler accepts one
UTF-8 file and still describes declaration-wide eager statics awaiting a
reachability cutover. The [static-field contract](../language/STATIC_FIELDS.md)
and implemented driver describe the delivered behavior. The
[active roadmap index](README.md) was 248 lines at the audited revision, much
of it narrating completed
deliveries despite having no implementation roadmap in progress.

**Change:** make the root overview accurately describe one selected entry and
its reachable modules and the current reachability-gated lifecycle. Keep the
active index to actionable work, status, next step, and dependencies; link to
the archive for completed records. Trim repeated rollout/status inventories
in living architecture documents, keeping one authoritative contract per fact.

**First PR / validation:** correct the root overview and simplify the index,
then tackle living documents by owner. Preserve archive history and incoming
anchors or provide compatible anchors during moves. Run `make docs-check` and
manually compare behavior claims with source: a passing link checker does not
detect stale semantics.

**Delivered:** the root overview now describes selection of one positional or
logical entry module, reachable import-closure loading, whole-world declaration
checking, and the implemented entry-rooted active-static lifecycle. The active
roadmap index is reduced from 256 lines of repeated completed-delivery history
to a 26-line index of the three actionable records, each with status, purpose,
next step, and dependencies; the archive remains the history owner. Related
language, phase, driver, reporting, and backend summaries no longer present
the delivered static-activation cutover as future work. Their current-contract
headings and all incoming links, including links from archived records, were
updated together. The claims were compared with the request, module-graph, and
static-activation owners and their existing tests. No product behavior changed,
so no new behavior test was added. `make docs-check` and the full `make check`
gate pass, including all 628 golden leaves.

### A39 — Define and test the documentation checker's Markdown subset

**Evidence:** [`markdown.rs`](../../crates/skald-docs-check/src/markdown.rs)
implements its own link/fence/heading parser. Fence state remembers only the
marker character, so three backticks can close a four-backtick fence; closing
line syntax is not distinguished from opening syntax. Reference definitions
are checked, but reference uses are not resolved as a complete Markdown model.

**Change:** state the accepted subset and fix demonstrated fence/heading/link
edges, or evaluate a maintained Markdown parser if broader syntax is wanted.
Keep repository index policy separate from syntax parsing. A new dependency
should be justified by less maintenance, not merely completeness.

**First PR / validation:** long fences containing shorter fences, closing
markers with trailing text, escaped/nested labels, reference uses, and duplicate
heading slugs. The checker must neither invent links inside code nor silently
skip real repository links.

### A40 — Reconsider the measurement tool's private SHA-256

**Evidence:** [`digest.rs`](../../crates/skald-mir-measure/src/digest.rs)
implements SHA-256 locally and checks the empty and `abc` vectors. This is a
standard primitive unrelated to compiler semantics, unlike the intentionally
owned binary64 facade.

**Change:** decide whether a small maintained hash dependency is preferable
to carrying the implementation. If retaining it, add multi-block and padding
boundary vectors and clarify that the digest identifies measurement bytes;
this audit does not identify a cryptographic vulnerability.

**First PR / validation:** compare lengths around 55/56/63/64 bytes and long
inputs, preserving the existing digest format. Do not change the report schema
or use an unstable general-purpose hash as a replacement. Check MSRV if adding
a dependency.

### A41 — Make runtime build configuration visible in artifacts

**Evidence:** [`runtime/Makefile`](../../runtime/Makefile) builds objects and
the archive under one default directory, with prerequisites based on source
and header timestamps. Changing `CC`/`CFLAGS` does not itself make existing
objects stale. This can invalidate comparisons between ordinary, instrumented,
or alternate-compiler builds.

**Change:** use distinct configured build directories or a small compiler/flags
stamp prerequisite. Document the artifact configuration in benchmark output.
Keep `make check` as the automation interface; adding repository CI is not
needed. The runtime's allocation, panic, and I/O split is already appropriate.

**First PR / validation:** changing flags must rebuild the intended archive;
an identical invocation should stay incremental. Retain existing ABI version,
symbol inventory, C layout assertions, allocation-failure, I/O, and
allocation-free panic tests. Do not centralize the deliberately different
panic and ordinary I/O error policies just because both use `write`.

### A42 — Rename sequential execution products used by both schedulers

**Evidence:** the
[parallel scheduler](../../crates/skald-golden/src/execute/scheduler/coordinator.rs)
accepts `SequentialOptions` and returns `SequentialExecution`. These types
describe common stage policy/results as well as sequential execution, so their
names now obscure their ownership.

**Change:** choose behavior-oriented names such as stage execution options and
suite execution, keeping scheduler policy distinct. Use temporary re-exports
only if they materially simplify callers; this is repository tooling.

**First PR / validation:** rename types/imports and documentation without
changing scheduling or JSON/JUnit fields. Run both scheduling paths and report
tests. Keep this opportunistic; it should not block A03's deadline repair.

### A43 — Add narrow automated phase-dependency checks

**Status:** Complete (2026-09-11).

**Evidence:** A07/A08 contradict the documented forward-dependency goal while
compiling successfully inside one crate. The
[binary64 API tests](../../crates/skald-binary64/tests/public_api.rs) already
demonstrate focused checks that keep implementation dependencies confined.
Rust `pub(crate)` alone cannot express every intended phase relationship.

**Change:** establish a small allowed-dependency policy for phase roots and
check production imports with a Rust-aware mechanism or a deliberately limited,
tested source check. Cover grouped imports, qualified references, and test
exclusions. Use compiler-enforced visibility/compile-fail tests for seals and
mutation authority, where those are stronger than source scanning.

**First PR / validation:** encode current intended boundaries and documented
temporary exceptions, then remove A07/A08 exceptions as their fixes land.
Ensure the checker detects a deliberately introduced reverse edge without
flagging legitimate lowering inputs or test-only dependencies. Consider crate
splitting only after boundaries stabilize and build measurements justify it.

**Delivered:** the compiler's phase-boundary integration test now enforces an
explicit direct-dependency allowlist across the source, lexer, syntax, module,
resolution, HIR, type-checking, MIR, pass, backend, reporting, and driver roots.
The policy rejects the former module-to-driver and resolution-to-type-checking
directions while admitting documented phase inputs such as module products in
HIR and MIR. Neutral support modules remain outside the phase-root matrix.

The deliberately limited scanner recognizes direct and grouped `crate` paths,
raw identifiers, and `super` paths that escape the owning phase. It ignores
comments and literals and excludes repository test and fixture conventions.
Focused tests cover those parsing limits, a synthetic reverse edge, legitimate
lowering inputs, test-source classification, and exception scope. Violations
include source lines, and the guard reports an exception once its matching edge
becomes stale.

One exact temporary exception permits `mir/retain/mod.rs` to consume the
pass-owned, sealed whole-program reachability result. The exception cannot
spread to another MIR file and is documented in the phase contract. Rust
visibility and existing compile-fail documentation tests continue to enforce
opaque seals and mutation authority where compiler enforcement is stronger.

Validation passed through the focused seven-test phase-boundary suite, the full
`make check` gate with 3,091 compiler unit tests, 53 process-determinism tests,
runtime and documentation checks, 21 compile-fail documentation tests, and all
628 golden leaves, plus the Rust 1.82.0 workspace all-target check.

## Recommended implementation order

This is a selection guide, not a promise that an entire tranche fits one PR.
Each selected change should acquire an owner, focused test plan, measurable
exit criteria, and any necessary contract decisions in its own roadmap.

1. **Repair demonstrated failures and gate coverage.** Start with A01, then
   A02/A03, A05, and A06. A04 follows the process-lifetime design so capture
   limits cannot reintroduce deadlocks. A35 is a separate tiny reporting fix.
2. **Restore reliable guidance and gather baselines.** A38 and A34 can proceed
   independently. Capture compile/native evidence before performance-sensitive
   changes, and retain all reproducers as regressions in their owning suites.
3. **Fix dependency ownership before moving large implementations.** A07 is
   independent. Settle A08's neutral capability contract and A09's publication
   boundary together, but implement them in separate PRs. Add A43's checks.
4. **Reduce repeated frontend and MIR work.** Use A09 to guide A10/A11/A12;
   apply A25 locally. Implement A18 before cross-pass analysis reuse in A19.
   Revisit A17 after the semantic capability owner is stable.
5. **Decompose code along those established responsibilities.** A14, A20,
   A21, A26, A28, A29, and A33 should preserve current products and diagnostics.
   A13 must cooperate with the actual tree-depth fix rather than merely hiding
   recursion in a helper.
6. **Select measured runtime and representation investments.** A30 is bounded;
   A31/A32 need ownership observations. A15 needs its inventory/design first.
   A22/A23 remain distinct larger projects tracked with the existing
   optimization work. The remaining tool improvements are independent.

For every implementation PR, preserve stable identity allocation, declaration-
wide semantic checking, exact reachable execution roots, proof-to-final seals,
evaluation/cleanup order, observable copies and destruction, permissive alias
semantics, ABI, and deterministic diagnostics/dumps unless that PR explicitly
changes an agreed contract. Expected-output updates must explain behavior
changes; they are not evidence of equivalence by themselves.

Use focused owner tests while developing, then the repository's `make check`
gate for implementation delivery. Run `make msrv-check` when manifests, Rust
APIs/syntax, or supported toolchains change. Structural Rust moves require
formatting and all-target compilation; source/native changes require relevant
goldens and runtime checks. Keep timing thresholds in controlled benchmark
runs rather than ordinary correctness tests.

## Work that should not become cleanup by default

- Do not merge AST, resolved IR, HIR, and MIR. Shared semantic descriptors are
  different from shared phase products.
- Do not replace independent verifiers with producer assertions, remove seals,
  or skip normalization to reduce compile time.
- Do not rewrite the small C runtime, move ordinary library behavior into C,
  or change its ABI to accommodate a cosmetic refactor.
- Do not add SSA, incremental compilation, a universal query system, a plugin
  pass registry, or more targets without a separately chosen objective. The
  [optimization catalog](OPTIMIZATION_CANDIDATE_CATALOG.md) already records the
  relevant larger possibilities and dependencies.
- Do not substitute broad string interning, arenas, pervasive boxing, or map
  replacement for measured allocation/lookup hotspots.
- Do not judge test duplication or large files solely by line count. Repeated
  independent semantic checks and exhaustive IR matches are often valuable.

## Audit validation and reproducibility

The audit built the current debug compiler and golden library with
`cargo build --offline --locked -p skald-golden --lib -p skac`, ran the explicit
binary64 suite, and ran `make docs-check`. Source/build observations above are
distinct from those executed checks. Probe sources and outputs were placed in
temporary directories; no compiler, library, runtime, or test implementation
was changed as part of this audit.

The process reproducers linked a temporary Rust program against the just-built
workspace libraries. The golden case used the public `ProcessCommand` and
`run_process` API; the linker case used `Toolchain::link_assembly`, an existing
temporary file as its archive input, and an executable fake linker. The latter
tested process mechanics rather than a real link. External watchdogs terminated
the intentionally stuck processes; compiler abort probes disabled core dumps.

The expression case can be reconstructed by generating a validly named
`chain.ska` containing 1,000 `1` terms joined by ` + ` in `main`'s return
expression and invoking debug `skac --emit asm`. Run it under a process
watchdog, not inside an in-process panic assertion.

The container case was:

```ska
from std::map import Map;
from std::u64 import BoxU64;

fn main() -> i64 {
    var values: Map<BoxU64, i64> =
        Map<BoxU64, i64>.with_capacity(18446744073709551615u);
    return (i64) values.capacity();
}
```

Compilation succeeded; the resulting program exceeded a one-second watchdog.
The unchecked doubling loop explains the nontermination independently of that
timing observation. Performance candidates elsewhere in the audit require
their own baselines and do not inherit a performance claim from these probes.

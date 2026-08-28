# Testing

Status: authoritative for test ownership, placement, and selection. Language,
compiler, backend, driver, and runtime contracts remain in their focused
documents; test guides describe how those contracts are verified, not what
the contracts mean.

## Test layers

Choose the narrowest layer that observes the behavior at its owning boundary.

Generic-interface coverage follows the complete compiler pipeline. Syntax
tests own source shape, punctuation, nested closers, and recovery. Resolution
tests own template and requirement identities, structural applications,
specialization caches, recursion, module lookup, bounds, closed declarations,
conformance, and exact diagnostics. Type-check/HIR tests own closed calls,
bound-member selection, casts, tests, and the absence of unresolved template
terms. MIR, verifier, backend, and native golden tests own witness dispatch,
ownership, metadata, ABI behavior, and execution. Cross-process integration
tests compile mixed generic classes and interfaces through every inspectable
phase while permuting source and provider-root order. Generic templates must
never consume ordinary `InterfaceId` values; only successful closed
applications do.

Operator-overloading coverage follows the same owner boundaries without
introducing an operator-specific lower-IR suite. Resolution tests own the
canonical `std::ops` bundle and exact identities; type-check/HIR tests own
primitive priority, nominal selection, aliases, outputs, exclusions, and
erasure to existing operations or interface calls. Existing MIR and backend
owners verify those realized forms, while native goldens and cross-process
integration tests own call equivalence, failure traces, artifacts, ABI
neutrality, and determinism. The
[operator-overloading conformance matrix](../compiler/OPERATOR_OVERLOADING_TEST_MATRIX.md)
is the authoritative traceability map and should be extended by linking the
narrowest new owner test rather than repeating a scenario at every layer.

Concise-range coverage follows the same phase ownership. Lexer tests
pin longest-match `..` beside decimal and member punctuation. Syntax tests own
the non-associative lowest-precedence node, spans, recovery, traversal, and
nesting budget. Module and resolution tests own compiler dependency evidence,
generic specialization requests, exact endpoint matching, canonical
initializer/protocol identities, primitive versus class realizations, and
stable dumps. Type-check and HIR tests own exact construction-origin
correspondence, explicit-versus-syntax distinction, ordinary-versus-fused
execution-plan selection, excluded boundaries, and mutation rejection. MIR
tests own the scalar fused shape, endpoint and item epochs,
advance-before-body order, control targets, cleanup, and absence of
protocol/optional traffic. Existing construction, iteration, lifecycle, and
backend owners verify both erased paths. Native goldens cover primitive and
class ranges, stored and direct use, evaluation order, boundaries, mixed
nesting, all exits, and panic attribution; cross-process tests pin complete
phase determinism.

Optional-value coverage spans type/capability/containment tests, HIR and MIR
shape and verifier tests, target layout tests, and native lifecycle tests.
Exact-class optional native tests use side-effect-visible destructors to catch
extra temporaries, missed conditional cleanup, and incorrect argument/result
ownership. Checked-view tests additionally cover bounded consumers, nested
guards, invalidating later arguments, shared-root anchor order, and failure
traps.

Conditional full-expression coverage starts with tracker and MIR ownership
tests and continues through the accepted logical source surface. Tracker tests
cover ordered unconditional, parent, child, and
later registrations plus an activation with no selected resource. MIR fixture
tests cover selected and skipped inline temporaries, multiple resources under
one condition, reverse cleanup, a secured scalar result, conditional storage
death, activation death, and final convergence. Mutations remove, duplicate,
reorder, move, or place cleanup on the skipped path. Nested and sibling graph
tests check parent-before-child decisions and local continuation sharing, and
the shared fixture must also pass deterministic MIR dumping and system
assembler acceptance.

Structured logical-HIR lifetime tests extend that boundary through inline-object
construction and call results, value/copy arguments, direct/static/instance
calls, field-derived booleans, exact-class optional results and arguments, and
later enclosing consumers. Native destructor traces distinguish selected from
skipped work and prove global reverse completion order. Optional verifier
mutations additionally cover lost or duplicate conditional cleanup and
initialization state that remains incompatible when a path condition ends.
These source-to-MIR tests exercise the optional initialization facade across
its private propagation, checking, and state-transition owners; the split
deliberately exposes no new test API. Focused refactors should run the
optional-value, logical object-lifetime, logical shared/array-lifetime, and
general MIR verifier suites together.
Optional shared-owner coverage additionally checks the one-word zero niche,
copy/adopt/move and conditional release, field and callable ownership,
self-assignment, target lifting and casts after unwrap, secured-anchor lifetime,
ABI register/stack pressure, absent-access failure, and exactly-once
last-owner finalization.
The compositional optional profile's recursive-syntax, canonical-owner, and
optional-array coverage, plus its identity, lifecycle, MIR,
verification, layout, ABI, native, robustness, and determinism obligations, are
owned by the
[optional-values compiler test matrix](../compiler/OPTIONAL_VALUES.md#compositional-test-matrix).
Focused compile failures remain for excluded optional payload categories and
unsupported external optional signatures.
Shared-verifier tests exercise its private propagation, transition,
use-validation, and state owners through the unchanged MIR verification
facade. Focused structural refactors should run the complete `mir::tests::shared`
module, logical shared/array lifetime mutations, general MIR verifier tests,
and shared-ownership process-determinism case; the split exposes no additional
test API.

Logical MIR verification mutations live in
`mir/tests/logical_verification.rs`. They independently corrupt exact boolean
types, result-carrier lifetimes and stores, selection and join edges,
right-only reachability, path-condition declaration and reuse, right-result
uniqueness, and failure isolation. Existing cleanup, optional, shared, array,
view, and guard verifier modules continue to own their resource-specific
mutations.

The private path-state algebra has direct tests under
`mir/verify/path_state/tests.rs`. They compact 256 equivalent selected
conditions into one predicate cube, preserve selected-versus-missing and
parent-active distinctions, retain genuinely different resource states,
reopen a condition identity in a later loop epoch, recompact converged states,
and split only the affected truth-table subset when a later merge overlaps an
already compacted cube.

`mir/tests/logical_stress.rs` exercises a mixed chain at the accepted logical
depth boundary, right nesting, and effectful selected-path cleanup. It pins
linear chain growth, the intended quadratic bound for ancestor-conditioned
cleanup decisions, deterministic MIR and assembly, and deterministic
over-budget syntax diagnostics.

| Layer | Location | Use it for |
|---|---|---|
| Compiler unit tests | `crates/skald-compiler/src/` beside the owner | Private algorithms, diagnostics, exact phase dumps, MIR verification, target legality, and lowering |
| Compiler integration tests | `crates/skald-compiler/tests/` | Public paths, cross-phase composition, cross-process determinism, and frontend robustness |
| Binary integration tests | `crates/skac/tests/` | The real `skac` entry point and process-visible CLI behavior |
| Golden tests | `tests/golden/` | Complete source-to-diagnostic or source-to-native-observation behavior |
| Runtime tests | `tests/runtime/` | The C runtime contract independently of compiler code generation |
| Documentation tests | `crates/skald-docs-check/` | Repository-local Markdown links, anchors, and required indexes |

Reusable non-Rust compiler corpus data belongs under `tests/compiler/`.
Production crates must not depend on the top-level test tree at runtime.

## Selecting coverage

Add a colocated unit test when a private owner can state the invariant directly.
Use a compiler integration test only when the public repository-internal API or
multiple phase facades are the subject. Use a binary integration test for CLI
argument, stream, status, or artifact behavior that must pass through the real
executable.

Add a golden case when the required observation crosses the complete compiler
boundary: exact diagnostics, deterministic assembly, linking, runtime output,
or process status. Add a direct C runtime case when compiler output is
irrelevant to the ABI behavior. A change may need more than one layer, but do
not repeat the same assertion at every layer.

For a new accepted source form, normally cover its smallest phase-specific
contract and its source-visible result. For a rejection, assert the diagnostic
at the phase that owns it and add a compile-failure golden when the rendered
diagnostic is part of the regression. Use exact matching when the complete
output is the contract; use independently named partial matchers when one
source owns several primary diagnostics whose labels, notes, or other context
may evolve. Backend tests should prefer assembly shape or legality assertions;
native goldens are for behavior that assembly text alone does not establish.

Primitive binding reassignment is covered at each owning boundary: syntax tests
retain direct and grouped assignment shapes; resolver tests pin `LocalId`
and `ParameterId` selection, shadowing, and source-diagnostic recovery;
type-check and HIR tests cover all five exact primitive types and deterministic
identity-only dumps for locals and value parameters; MIR tests establish
binding-storage selection, source-before-store ordering, and post-store
temporary cleanup; backend and native goldens exercise canonical integer,
byte, boolean, and floating storage plus exact rendered failures.

Primitive-cast coverage starts with the complete twenty-five-cell type-check,
HIR, and MIR matrix. A test-only oracle independently derives pure results and
checked post-truncation validity for dense boundaries and deterministic raw-bit
samples; backend tests compare those expectations with native execution.
Checked-cell MIR tests own range diamonds, evaluation order, success joins,
and failure isolation. Literal/dynamic parity and the non-transforming pass
boundary are pinned separately. Native goldens cover all three checked targets
with exact panic output and status, selected-path behavior, later-effect
suppression, and successful full-expression cleanup. Cross-process tests cover
token, AST, resolved, HIR, MIR, diagnostic, and assembly products; the golden
runner repeats compilation and execution to compare stdout, stderr, and
status in full-determinism audits. Generated-object inspection owns the
unchanged runtime marker, reporter, and absence of conversion helpers.
Exact binary64 bit-representation coverage separately checks the canonical
`std::f64` intrinsic identities and signatures, primitive-keyword module path,
distinct HIR/MIR bit-reinterpretation semantics, verifier type relation,
inline cross-register-class moves, arbitrary `u64` round trips including NaN
payloads, both zero signs, infinities, and absence of runtime helpers.
Primitive-box native coverage imports every type-named module and pins exact
equality, representative and boundary hashes, cross-class rejection,
per-class domain separation, interface dispatch, and generic
`Equatable`/`Hashable` constraints. These are ordinary library composition
tests and require no compiler-specific box fixture.

## Focused commands

`make help` is the complete command inventory. Useful focused forms include:

```text
cargo test --locked -p skald-compiler lexer::tests
cargo test --locked -p skald-compiler mir::verify
cargo test --locked -p skald-compiler --test public_api
make golden-runner-test
make cli-test
make golden-test
make runtime-test
make compiler-test
```

Rust test-name filters match substrings and may select more than one test; use
`--exact` only after obtaining the complete test path from `cargo test -- --list`.
Before handoff, run the full validation described in the
[development workflow](README.md#change-validation).

The Rust golden runner executes versioned feature specs through a bounded
dependency scheduler. The ordinary and common focused interfaces are:

```text
make golden-test
make golden-filter GOLDEN_FILTER='operators/**'
make golden-exact GOLDEN_ID='calls/functions::direct_call::default::return_value'
make golden-determinism-test
scripts/golden.sh --list --filter 'runner/**'
scripts/golden.sh --explain '<canonical-leaf-id>'
scripts/golden.sh --format json --filter 'syntax/**'
scripts/golden.sh --filter 'standard_io/**'
scripts/golden.sh --determinism full --filter 'runtime/panic**'
scripts/golden.sh --determinism compile --filter 'modules/**'
scripts/golden.sh --determinism compile --filter 'primitives/**'
```

The [golden fixture guide](../../tests/golden/README.md) owns the versioned
schema, stream-matcher semantics, filtering, and canonical-ID contracts.

The runner's compiler-independent process tests use its Rust fake-process
binary to cover exact and partial byte expectations, non-UTF-8 Unix arguments,
temporary files, environment isolation, large simultaneous pipes, signals,
timeouts, and Linux descendant termination:

```text
cargo test --locked -p skald-golden --test process_execution
```

Execution defaults to determinism `off`. Use `--determinism compile` to compare
two compiler products or `--determinism full` to compare both compiler and
native-process observations. Native selections prepare the runtime once,
link checked assembly through the compiler driver's `Toolchain`, and retain
failed sandboxes for inspection. Compile-fail-only selections do not prepare
the runtime. The scheduler defaults to host available parallelism under one
process budget. Use `--jobs 1` for single-worker diagnosis, `--fail-fast` to
stop starting unrelated work after an observed failure, and spec `serial` or
named `resources` for explicit exclusion. Results remain in canonical ID order.
Compiler, linker, and native processes default to a 30-second timeout.
`--timeout SECONDS` changes that bound; an explicit per-test timeout remains
authoritative. Human output is the default, while `--format json` and
`--format junit` emit single
machine-readable documents with the same canonical leaf IDs, stages, statuses,
durations, and failures. `--show-output` includes passing streams in human
reports, `--slowest N` ranks completed leaves with stable ID tie-breaking, and
`--keep-all-artifacts` retains passing run sandboxes. Ordinary execution never
updates expectations.

The focused orchestration suites use bounded fake compiler, runtime, linker,
and native processes:

```text
cargo test --locked -p skald-golden --test sequential_execution
cargo test --locked -p skald-golden --test parallel_execution
cargo test --locked -p skald-golden --test reporting
```

## Fixtures and expectations

Keep small source fixtures in the test module that consumes them. Shared Rust
fixtures belong in a responsibility-named test module, not a general bag of
defaults. Test-only compiler pipelines stop at the boundary named by the
helper and assert only that earlier phases succeeded.

Compiler tests obtain the canonical standard-library module closure from one
shared fixture catalog. Malformed-provider tests pass explicit per-module
overrides, while determinism tests may reorder the returned closure before
writing it; neither case maintains a second canonical module inventory.

Choose primitive literal spellings that communicate the fixture's intent.
Use single-quoted `u8` literals for textual bytes, hexadecimal integers for
masks and bit patterns, and decimal integers for counts, arithmetic examples,
range-focused cases, and tests that deliberately cover decimal syntax. Tests
of literal equivalence may retain multiple spellings side by side.

MIR verifier tests may use the crate-visible constructors and mutation
accessors under `cfg(test)`. These deliberately preserve explicit identities,
types, ownership modes, and spans; they are not production API. Start from the
smallest valid MIR, mutate one invariant, and assert the structured verifier
failure and, where relevant, backend rejection.

Top-level corpus and fixture formats are documented locally:

- [compiler and robustness corpus](../../tests/compiler/README.md);
- [golden specs and discovery](../../tests/golden/README.md); and
- [runtime harnesses](../../tests/runtime/README.md).

The implemented [standard I/O compiler contract](../compiler/IO.md#verification-obligations)
assigns coverage across these same layers. Direct runtime harnesses now own
the version-9 handle and one-transfer byte boundary. Phase tests own canonical
identities, types, access modes, anchors, and deterministic dumps; backend
tests own pointer/length lowering and exact symbols; private-standard-library
native goldens own checked calls and host failures. Public I/O goldens and
native probes cover exact writes, EOF, geometric read growth, working-directory
files, binary partial transfers, invalid progress, close behavior, and stable
failures. A source-native golden covers every primitive `println_<type>` helper,
including extrema, signed zero, infinities, and NaN, through exact stdout bytes
without a scalar runtime observer. Golden specs accept inline or external
exact-byte stdin, arguments, and expectations and feed the same inputs to both
deterministic executions. The [golden fixture guide](../../tests/golden/README.md)
owns the NUL-terminated `argv_file` encoding. Process-argument goldens cover a
no-suffix invocation, repeated fresh snapshots, element-zero position without
freezing its path, and exact ordinary, whitespace, empty, and non-UTF-8 suffix
values.

Multi-file goldens declare logical entries, module roots, and standard-library
roots directly in their owning spec. Provider trees live below feature-local
`cases/` directories; the ownership audit associates their supporting `.ska`
files with the spec rather than treating them as independent test programs.

Exact dump and diagnostic expectations should remain readable and intentional.
When an expectation changes, inspect the semantic difference before updating
it. Do not introduce a second renderer solely to make a test convenient.

Cyclic-module tests are split by owner. Graph tests prove reachable-closure
loading and canonical module identities without recursion. Resolver and
type-check tests prove declaration-first lookup across cycles and retain
inheritance, containment, interface, external-ABI, privacy, and direct-import
diagnostics at their existing semantic phases. The cross-process determinism
suite permutes source discovery, provider spelling, and import order while
comparing graph, resolved, HIR, MIR, diagnostic, and assembly products.
Native and compile-failure goldens remain the source-visible end-to-end
observations.

### Module-system coverage map

The required diagnostics in the
[module compiler contract](../compiler/MODULE_SYSTEM.md#required-diagnostic-coverage)
have explicit owners:

| Contract area | Owning evidence |
|---|---|
| Entry selectors, logical spelling, root/standard-library options, suffixes, output defaults, and process status | driver CLI unit tests and `crates/skac/tests/cli.rs` through the real binary |
| Root equivalence, provider ambiguity independent of contents or physical target, symlink traversal and failure, exact case, unreadable/non-regular candidates, and positional containment | `module::provider::tests` and `module::graph::tests` filesystem matrices |
| Missing and ambiguous imports, import-source aliases, direct self-imports, malformed reached sources, binding conflicts, privacy, direct-import enforcement, unknown/wrong-kind declarations, selected `main`, and incompatible external ABI declarations | feature-owned partial or exact diagnostics under `tests/golden/modules/`, plus structured graph and resolver tests |
| Two-module, longer, selected-entry, synthetic string, and deep cyclic dependency graphs | successful native goldens plus structured `module::graph::tests` coverage |
| Qualified/selective lookup, opposite-side declarations, non-module semantic cycles, and access diagnostics inside cyclic graphs | `resolve::tests::cyclic_imports`, the `modules_cycle` native golden, and the `modules_cycle_diagnostics` exact compile-failure golden |
| Multi-segment aliases, wildcard imports, and trailing selective-import commas | exact parser goldens and syntax recovery tests |
| Ordering and independent-process stability, including semantic cycles and alternate selected entries with one closure | `pipeline_determinism`, `resolve::tests::cyclic_imports`, graph/provider permutation tests, and the two-process golden runner |

Host-dependent filesystem wording stays asserted structurally at the provider
boundary; portable source diagnostics use byte-exact golden snapshots. This
keeps every required failure owned without freezing operating-system error
text.

Explicit-copy tests should distinguish `T(copy source)` from ordinary
`T(copy)` and `T(copy, other)`, cover static and runtime target selection,
assert one source evaluation and one selected copy, and verify that explicit
copy is not recorded as constructor elision. Corrupt the lowered copy target
or operation in a verifier test rather than relying only on successful native
execution.

Ordinary-constructor coverage should compose value, `ref`, and `mut ref`
binding with exact, ancestor, interface, and `Obj` relations. Exercise selected
initializer identities in local, field, argument, result, temporary, and
direct-base contexts. Verifier mutations should independently cover table
density, declaration/definition agreement, selected target and signature,
source lifetime, and undeclared call targets.

Shared-ownership coverage is intentionally distributed by responsibility.
Type-check and MIR tests cover owners in local, field, call, result, cast,
receiver, alias, ordinary-allocation, and copy-allocation positions. Backend
tests cover the header, count updates, hidden anchors, dynamic finalization,
cascading field release, cycles, and malformed-input rejection. Native goldens
observe destruction order and exact failure stderr; direct C runtime tests own
allocator success, invalid-input defects, and reported host exhaustion. The
process-determinism suite
compares all phase products for a representative shared copy-allocation
program.

Explicit shared-dereference coverage must keep owner operations and pointee
operations separate. Positive cases use `*owner` or `owner->member`; exact
compile-failure goldens cover raw-handle member access, alias arguments,
checked casts, type tests, inline copies, non-shared `*`, and unsupported
whole-pointee assignment. Diagnostics for member selection recommend `->`;
general object-place diagnostics recommend `*`.

Array coverage is distributed by the operation owner. Syntax, resolution, and
type-check tests cover recursive grouping, exact identity, capabilities,
access, diagnostics, and deferred forms. MIR mutation tests break structural,
storage, ownership, projection, slice-order, and anchor invariants one at a
time. Backend tests own checked byte arithmetic, layout, helper labels,
internal ABI pressure, every terminating failure family, and native lifecycle.
Golden cases own complete successful and unsuccessful process observations.
Because array failures promise only non-return, their specs use
`exit = "failure"`; tests must not depend on a particular signal or numeric
status.

While-loop coverage follows the phase boundary that owns each invariant.
Lexer tests keep `while`, `break`, and `continue` reserved without reserving
identifier prefixes. Syntax tests own mandatory loop punctuation and
loop-exit recovery and spans. Resolution and type-check tests own
source-ordered `LoopId`s, nearest-loop exit selection, outside-loop rejection,
enclosing-condition and child body scopes, exact-`bool` conditions,
conservative fallthrough, distinct targeted effects, and structured HIR
dumps. Source-to-MIR tests prove generic cyclic graphs and cleanup-to-exit or
latch edges, while the internal-HIR lifecycle matrix and verifier mutations
cover every current storage family without duplicating those cases at the
parser boundary. Native goldens own zero, one, and repeated iterations,
immediate and conditional exits, nested blocks and loops, enclosing mutation,
condition/body/break/continue cleanup, ownership-heavy exits, mixed
fallthrough/return/panic behavior, and return from a body. The golden runner's
full audit retains repeated assembly as the source-to-target determinism check.

The complete source-observation matrix has these owners:

| Loop lifecycle area | Source-to-observation evidence |
|---|---|
| Condition false and condition-owned compiler temporaries | `while_loops.ska` and `loop_lifecycle_matrix.ska` |
| Primitive mutation, normal body fallthrough, and fresh body epochs | `while_loops.ska` |
| Inline objects, shared owners, primitive/class/shared optionals, arrays, checked views, aliases, and shared-backed anchors | `loop_lifecycle_matrix.ska` |
| Immediate, conditional, nested-scope, ownership-heavy, and nested-loop `break` | `break_loops.ska` and `loop_lifecycle_matrix.ska` |
| Immediate, conditional, nested-scope, ownership-heavy, nearest-loop, and condition-effect `continue` | `continue_loops.ska` and `loop_lifecycle_matrix.ska` |
| Return cleanup and mixed fallthrough, break, continue, return, and panic effects | `while_loops.ska`, `continue_loops.ska`, and `loop_lifecycle_matrix.ska` |
| Core nominal iteration call order, zero/repeated attempts, normal completion, continue, break, and return | `general_iteration.ska` |
| Produced-once, shared-owner replacement, optional-root mutation, and detached array-backed iteration receivers | `general_iteration_receivers.ska` |
| Stored-value state/item families and nested optional termination | `general_iteration_value_matrix.ska` |
| Item/body/receiver cleanup on normal, continue, break, and return exits | `general_iteration_lifecycle.ska` |
| Inherited, specialized, generic-bound, nested, ownership-heavy, and ordinary Vec iteration | `standard_vec/vec_iteration.ska` and the remaining `standard_vec` iteration cases |

The backend iteration test owns the final boundary assertion that generated
MIR contains only ordinary calls, optional operations, storage and cleanup,
and that its deterministic assembly executes natively. Direct runtime tests
separately pin the exact version-9 archive symbol set; iteration adds no
runtime harness.

Colocated MIR hardening tests separately prove that condition-owned storage is
dead before body or exit, body-owned storage is dead before latch, header, or
exit, and redirected edges cannot skip cleanup. Bounded generators cover both
source loops through the complete pipeline and target-independent cyclic CFGs
through deterministic verification. Equivalent split and renumbered loop CFGs
must survive the pass boundary and x86-64 lowering without canonical-layout or
source-loop recognition.

The frozen [generic-range contracts](../language/RANGES.md) have an ordered
coverage layer. Colocated resolution and primitive-registry tests own
canonical successor and range declaration validation, identity and dump
determinism, lookalike exclusion, and the exact `u8`/`u64`/`i64` static
realization matrix.
Type and HIR tests own class-witness versus primitive-intrinsic closure and
reject unsupported types, owners, views, wrong applications, and direct
primitive members. `tests/golden/ranges` owns native manual successor behavior,
explicit half-open primitive and opted-in class ranges, boundary and exit
semantics, lifecycle effects, and capability failures through ordinary
iteration. The pipeline-determinism suite permutes source and provider order
through assembly. Syntax tests will own
`..` longest match, lowest precedence, non-associativity, recovery, and exact
endpoint typing. HIR tests will distinguish ordinary construction carrying
canonical syntax provenance from explicit construction. Fused-loop MIR and
assembly tests will own the call-free, optional-free scalar shape, while
`tests/benchmarks/range_loop` will record the separate matched-`while` median
without joining `make check`. The active
[implementation roadmap](../roadmaps/GENERIC_RANGES_ROADMAP.md) assigns each
remaining layer to its delivery task.

Primitive integer operation coverage keeps the closed matrices explicit.
Type-check and MIR tests enumerate all eighteen same-type comparisons and all
nine casts, while rejection tests enumerate every ordered mixed integer pair,
predicate, noninteger operand family, and cast target. Backend tests own
condition signedness, canonical scalar results, instruction shape, and
malformed-MIR rejection. Golden cases own exact diagnostics, extrema and
modulo observations, and range-check-before-cast composition with array
positions.

Pure bitwise coverage keeps source and downstream responsibilities distinct.
Lexer, syntax, resolution, and type-check tests own punctuation distinction,
precedence, source shape, exact selection, mixed-type rejection, and focused
actual-type diagnostics. Typed-HIR and verified-MIR tests cover the complete
operation/type matrix, deterministic dumps, right-control-effect spilling,
and one-invariant verifier mutations. General expression-stabilization tests
also compose checked optional unwraps with later calls and cover owned
argument preparation that creates control flow. Backend tests own selector
shape, `u8` canonicalization, and assembler acceptance. Native and
compile-failure goldens own edge patterns, left-to-right exactly-once effects,
arbitrary eager consumers, and exact rendered diagnostics.

Checked-shift lexer, parser, resolution, and type-check tests own longest
match, precedence, direction, exact left kind, fixed `u64` count, source-order
checking, and focused diagnostics. HIR/MIR tests own secured-carrier order,
deterministic dumps, the checked diamond, and malformed
correspondence/dominance mutations. Backend tests own
unsigned width checks before `rcx`/`cl`, `shl`/`sar`/`shr` selection, `u8`
canonicalization, stable static-message pooling, assembler acceptance, native
edge results, and exact excessive-count stderr. Native goldens additionally
own arbitrary operands, every consumer, evaluation and cleanup order, and
failure-before-check behavior. A combined independent-process snapshot covers
tokens, AST, resolved IR, typed HIR, verified MIR, assembly, and focused type
diagnostics. The golden runner's full audit independently recompiles every
case before comparing assembly and independently executes native cases twice
before checking values, panic bytes, and process status.

Checked integer-division coverage follows the same source-to-control-flow
boundary. Lexer tests distinguish `/`, `%`, `//`, and spaced `/ /`; parser and
resolution tests own the shared multiplicative tier, source identity, spans,
grouping, and recovery. Type-check tests enumerate exact same-type
`i64`/`u64`/`u8` selection, noninteger and mixed-type rejection, source-order
diagnostics, and arbitrary control-affecting operands. HIR/MIR tests own the
secured operands, explicit zero-check diamond, operation-specific failure,
successful result carrier, and verifier mutations. Backend tests own unsigned
and signed instruction shape, floor correction, the signed-minimum guard,
`u8` canonicalization, and static-message stability. Native and
compile-failure goldens own sign and boundary results, exact zero failures,
failure-before-check, every consumer, evaluation order, and cleanup.

Floating-division coverage keeps its non-failing boundary explicit. Type-check
tests own exact `f64 / f64` selection, mixed and nonnumeric rejection,
source-order diagnostics, and arbitrary operands and consumers. HIR/MIR and
backend tests own the portable `div.f64` identity, exact operand/result types,
ordinary eager lowering, `divsd` realization, and absence of an integer
zero-check path. Native goldens observe signed zero, infinity, subnormal,
overflow, and underflow through canonical shortest text, execute NaN
production without freezing its payload, and trace
exactly-once evaluation plus reverse full-expression cleanup. Backend-native
oracles retain exact-bit checks where representation identity is the tested
contract. Cross-process snapshots cover phase products and focused diagnostics.

Floating-comparison coverage owns the complete source-to-native boundary.
Type-check matrices retain all six predicates for exact `f64` operands,
reject mixed and unsupported pairs before HIR, and exercise arbitrary operands
and boolean/control-flow consumers. Direct HIR and MIR matrices plus verifier
mutations cover exact flavors, types, canonical `bool` results, and definition
order. Backend tests require scalar unordered comparison, explicit parity
gating, canonical zero extension, deterministic assembly, and assembler
acceptance. Native fixtures cover finite less/equal/greater values, signed
zero, both infinities, NaN in either operand position, source order, cleanup,
and short-circuit skipping. Cross-process snapshots cover source phase
products and focused diagnostics.

The complete primitive-operator profile has an additional cross-family
closure layer. A compact backend property table checks exact floating-division
bits, unordered NaN rows, ordered trichotomy, signed-zero equality, valid
predicate duals, canonical booleans, and repeated assembly emission across
multiple NaN encodings. One source-native golden composes floating division
and comparison with wrapping arithmetic, checked integer operations, bitwise
and shifts, optional unwrap, arrays, calls, inline and shared object receivers,
allocation, assignment, conditions, loops, skipped failures, and reverse
full-expression cleanup. Its complete token-through-assembly products are
also compared across independent compiler processes.

Eager boolean operator coverage follows the same phase boundary. Lexer and
syntax tests own `!`/`!=`, prefix/postfix position, precedence, nesting, and
recovery. Resolution and type-check tests own source-shaped negation, exact
`bool` selection, boolean equality, invalid ordering, and focused actual-type
diagnostics. HIR/MIR dumps and verifier tests own selected eager scalar
operations; backend and native goldens own canonical truth tables,
left-to-right exactly-once evaluation, optional-unwrapped booleans, calls,
fields, assignments, conditions, returns, and composition with unrelated
integer casts. Cross-process tests compare the complete token, AST, resolved,
HIR, MIR, diagnostic, and assembly observations. Short-circuit tests add
complete truth-table, precedence, arbitrary-operand, every-consumer, external
ABI, skipped-failure, and selected-path cleanup goldens. Destructor traces
prove inactive resources never become live and active resources clean in
reverse completion order.

The MIR-only path-condition foundation has a separate verifier matrix.
Hand-built fixtures cover selected and skipped scalar-storage epochs, nested
parent conditions, independently active siblings, repeated loop epochs,
explicit cleanup branches, deterministic dumps, and strict ordinary joins.
Mutations cover noncanonical or missing activation stores, invalid parents,
reads outside the selected parent, wrong cleanup conditions, unresolved
conditional state, and activation leakage. Source-to-native goldens exercise
the same verified representation through ordinary compilation.

## Private initializer coverage

Private ordinary initializer tests follow the owning phase instead of relying
on one end-to-end factory case:

- syntax and resolution tests own contextual modifier parsing, recovery,
  declaration spans, per-overload visibility, and stable initializer
  identities;
- type-check tests own unique-most-specific selection followed by access,
  including the no-public-fallback rule, exact-class body categories,
  inaccessible derived `super(...)`, direct/shared construction, and inline
  and shared default-length arrays;
- HIR tests prove that successful selection retains only the authorized
  `InitializerId` and erases visibility, including independently mutated
  resolved declarations;
- MIR verifier tests mutate selected initializer identities and array default
  plans independently because lower phases deliberately have no visibility
  metadata;
- determinism tests compare mixed public/private selection, rendered `TYP040`
  diagnostics, HIR, MIR, and assembly across independent processes; and
- native factory goldens prove exact-class private construction while compile
  failures cover foreign and derived callers.

When adding a construction form, test its authorization at the type-check
consumer and its selected identity at the nearest lower-phase verifier. Do not
make a global array capability table caller-dependent: authorize its stable
default plan where the source array expression is checked.

## Produced object alias argument coverage

Produced read-only object alias arguments have evidence at each owning
boundary. Type-check and HIR tests own accepted producer families, static
class/interface/`Obj` relations, the read-only restriction, and exclusions.
MIR tests own one-time materialization, ordinary `MirArgument::View` origins,
source order, liveness, and reverse full-expression cleanup. The x86-64
backend test owns ordinary object-alias marshaling, assembler acceptance,
native dispatch, and the unchanged runtime symbol/version boundary.

The `produced_alias_arguments` native golden composes exact-class, ancestor,
interface, and `Obj` targets across direct, static, instance, interface, and
initializer calls. Its exact stdout records construction effects, later
arguments, callee observations, nested reverse cleanup, and an owning copy
that outlives its source. The existing `produced_alias_invalid_sources`
compile-failure golden keeps mutable aliases and excluded producer families at
their type-check diagnostics. `pipeline_determinism` compares the successful
case's token-through-assembly products across independent compiler processes;
the golden runner separately compares assembly, stdout, stderr, and status
across repeated compiles and executions. Primitive parsing and standard-I/O
goldens additionally exercise the standard library's direct literal and
factory-result aliases, preventing a return to source-only staging locals.

## Produced exact-class method receiver coverage

The complete supported produced exact-class method-receiver path is
implemented. Focused coverage proves that construction, literal,
direct/static/instance/interface result, grouping, and closed-generic
producers become one read-only view with no fake binding and lower through
verified MIR.

The implemented representation baseline has one exhaustive typed member-
receiver carrier. Focused type-check coverage distinguishes stable places,
checked casts, general shared/optional object views, and checked array
elements; it also proves that field receivers reuse the same carrier. The full
compiler unit suite freezes existing HIR/MIR dumps, anchors, guards, dispatch,
cleanup, control effects, and native behavior. Compile-failure coverage keeps
primitive, unit, optional, array, shared-owner, field, and mutable-method
categories outside the accepted boundary.

Current resolver and type-check tests cover construction, literal,
direct/static/instance/interface result producers, grouping, inherited
projection, closed-generic bound authorization, and read-only versus `mut fn`
diagnostics. HIR and MIR tests cover one explicit producer, no fake binding,
receiver-before-argument ordering, chained results without receiver copies,
closed-generic interface dispatch, exactly-once completion, result securing,
selected and skipped logical paths, conditional conditions, repeated loop
epochs, return expressions, and reverse cleanup. Verifier mutations reject
wrong storage kind, mutable produced access, mismatched complete origin,
invalid projection, missing or premature cleanup, post-cleanup use, duplicate
production or cleanup, and skipped-path leakage.

Focused native tests trace receiver-before-argument evaluation,
nested receiver chains, `if`/`elif`, selected and skipped short-circuit paths,
loop epochs, return expressions, reverse cleanup, and non-unwinding failure
before or after receiver publication. Backend and native conformance covers
exact, inherited, virtual, and closed-bound interface selection; direct,
static, instance, and interface producers under recursive register/stack
pressure; raw-byte string composition, slicing, parsing, and observation;
`Vec<Str>`, nested generic results, and owning results that outlive receiver
cleanup. Compile-failure goldens retain mutable methods, optional and array
producers, raw shared dot access, unrelated types, and escaping uses. Produced
field reads have separate success and read-only-rejection coverage below.

`pipeline_determinism` compares token-through-assembly phase products across
independent compiler processes. The `produced_receivers` golden group repeats
successful assembly, exact stdout, lifecycle order, stderr, and process status
and separately repeats resolver- and type-check-stage diagnostics. Backend
surface assertions freeze ordinary receiver marshaling, class layout, the
runtime-call set, ABI version 9, and `ska_rt_abi_v9` without a receiver-specific
runtime harness.

## Produced-object field-read coverage

Produced-object field reads are implemented, with every readable field
category typed through verified MIR and native execution. Resolver tests admit
eligible reads without `RES009`, while
the produced-receiver compile-failure golden retains only the excluded
optional, array, and raw shared-owner roots. A write-shaped produced field must
retain the same receiver and fail through the ordinary read-only type-checking
diagnostic.

Resolver coverage must prove one retained producer, canonical inherited-base
and nested inline-field projections, declaring-class privacy, structural
getter and closed-generic composition, deterministic dumps, and unchanged
diagnostics for primitive, optional, array, raw shared-owner, and `unit` roots.
Write-shaped forms remain rejection tests rather than executable success
cases.

Type-check and HIR coverage must prove one read-only produced `View` with no
inspection place or fake binding. Primitive endpoints load values; class
endpoints feed only existing receiver, `ref`, checked-view, copy, owning
argument, assignment-source, and return-copy contexts. Optional, inline-array,
shared-owner, optional-owner, and shared-array endpoints must exercise their
ordinary type-specific consumers rather than a generic untyped field path.

MIR lowering and verifier tests own exactly-once materialization, completion
before projection, canonical field paths, complete-object origin, selected-
path liveness, scalar spilling, subordinate guard and anchor order, result
securing, and one reverse full-expression cleanup. Mutation tests must reject
missing, premature, duplicate, wrong-path, and post-cleanup use, plus mutable
access or an invalid origin/projection.

The dedicated produced-field MIR suite also mutates cleanup order, moves a
consumer before initialization and after cleanup, redirects an exact origin
to the root or a sibling field, corrupts a projection, and leaks cleanup onto
a skipped logical path. Its valid control-flow fixture covers `if`/`elif`,
short-circuit selection, loop epochs, returns, and both producer-side and
consumer-side abrupt failure. Terminating blocks remain non-unwinding: they
must not acquire a synthesized `EndFullExpression`.

Native coverage must compare named-place and produced-root behavior for every
implemented field category, including later argument effects, lifecycle calls,
logical paths, loops, returns, failure, dispatch, ABI pressure, structural
getters, and `Vec<Str>`. Backend assertions retain ordinary layouts, receiver
and alias marshaling, runtime calls, ABI version 9, and `ska_rt_abi_v9`; no
field-read-specific runtime harness or symbol is permitted.

`produced_fields/fields::lifecycle_order` is the focused generated-code
lifetime trace. It observes explicit field copy securing before root destruction,
later argument effects, call execution, reverse cleanup of multiple produced
roots, nested field destruction, and exactly-once destruction of the secured
copy. The two rejection leaves in the same group freeze read-only, mutable-
alias, privacy, and invalid-member diagnostics across repeated compilation.

## String coverage

String coverage follows the
[language](../language/STRINGS.md) and
[compiler](../compiler/STRINGS.md#diagnostics-and-test-obligations) contracts:

- lexer and syntax tests own decoded bytes, every escape and malformed-literal
  category, spans, recovery, and nesting limits;
- module/provider tests own synthetic `std::str` reachability, explicit-edge
  coalescing, missing/ambiguous/exact-case lookup, malformed and non-UTF-8
  providers, cyclic dependencies, replacement roots, and disabled
  standard-library lookup;
- resolver and type-check tests own exact language-item identity, the complete
  four-field descriptor/private-cell/privacy/lifecycle rejection matrix,
  `Equatable`/`Hashable` conformance, produced-value contexts,
  canonical private descriptor construction, the canonical
  `std::str`/`std::error` cycle and panic statements, and the rule that
  ordinary initializer and method names have no compiler meaning;
- MIR tests mutate each string declaration, literal-data, static-owner,
  descriptor-publication, and ownership invariant independently;
- backend tests own immutable bytes, pooling, alignment, relocations,
  sentinel-aware retain/release, dynamic reclamation, malformed-input
  rejection, absent literal-cache publication, and the unchanged runtime ABI;
- native goldens own copying, assignment, arguments, results, temporaries,
  repeated default construction over one private static empty backing, signed
  and negative byte positions, half-open and negative slice bounds, checked
  failures, factory isolation, conversion, concatenation, embedded zero/high
  bytes, byte equality and equal-hash consistency across literals, dynamic
  strings, slices, copies, interfaces, and generic bounds, canonical boolean
  and integer formatting through
  `std::io`, every integer width boundary, powers of ten, exact optional
  boolean and integer parsing, exhaustive `u8` round trips, malformed and
  arbitrary-length input, correctly rounded binary64 parsing against a
  checked-in exact-fraction midpoint oracle, shortest binary64 formatting
  across every exponent field and bounded exhaustive significand sweeps
  against an independent generated oracle, bit-identical formatter/parser
  round trips, repeated execution, and reuse of the statically initialized
  compact Ryū tables throughout the 2,929-input formatting corpus;
- backend allocation probes prove that repeated `Str()` construction adds no
  allocation or early free beyond the one private empty-backing static;
  ordinary static-shutdown coverage owns the root owner's final release; and
- `pipeline_determinism` compares canonical graph, diagnostics, resolved HIR,
  verified MIR, and assembly across independent processes and provider/source
  permutations.

Focused resolver, MIR, and backend tests use one shared test-only helper that
loads the canonical `std::str` and `std::error` sources as a complete
dependency closure. Cross-process determinism and driver provider tests copy
both real sources into their filesystem fixtures. This keeps the public
surface, panic dependency, and lifecycle behavior from drifting independently
of compiler coverage.

## Runtime-trace coverage

Runtime traces are implemented; the version-9 runtime foundation and x86-64
requested metadata emission, activation-frame maintenance, source-call
replacement, central reporter-edge replacement, and generated-helper/runtime
attribution are enabled by default. Coverage remains split by owner:

- source and source-database tests own one-based line and Unicode-scalar
  column mapping at span starts;
- backend metadata tests own callable-name construction, semantic initializer
  signatures, provider-relative path escaping, interning, deterministic
  ordering, relocation-read-only placement, and omission of unused records;
- x86-64 frame and assembly tests own exact push/pop/replacement counts and
  placement, every return path, direct/static/virtual/interface/external and
  lifecycle calls, generated array/ownership/allocation attribution,
  failure-only reporter and ownership-overflow updates, transient scratch
  clobbers, helper suppression, a raw-call construction audit, local-exec TLS
  relocations, and zero-cost omission;
- direct C runtime tests own exact empty, single, nested, replaced, capped,
  over-cap, cyclic-chain cap, and failed-write behavior, plus separately
  link-wrapped proof that valid rendering performs no allocation;
- driver and CLI tests own default enablement, the value-free
  `--omit-runtime-trace` option, source-database handoff, and repeated-option
  rejection;
- native goldens own exact direct, recursive, virtual/interface, standard-
  library, lifecycle, and static-initializer traces plus explicit panic, every
  static termination family, allocation failure, and ownership overflow; and
- independent-process tests own identical metadata, assembly, paths, stderr,
  and status across different temporary provider roots.

Generated lifecycle, array, ownership, finalization, coordinator, wrapper, and
target helpers are covered as omitted frames whose failure remains attributed
to the initiating source operation. Source-authored standard-library and
lifecycle bodies are visible. Representative omitted builds preserve the
single-line panic output and contain no trace-only frame bytes, instructions,
symbols, relocations, metadata, or source lookup.

The [runtime-trace performance procedure](PANIC_RUNTIME_TRACE_PERFORMANCE.md)
compares enabled and omitted builds for call-heavy recursion, a pure tight
loop, allocation-heavy execution, and a representative golden. It records
instruction counts, code size, and repeated wall time without turning noisy
host timing into a correctness gate. The repository gates remain `make check`,
`make msrv-check`, and `git diff --check`.

## Function-value coverage

Function-value coverage follows the exact owner of each invariant. Parser and
resolution tests own recursive type shape, canonical identity, access,
shadowing, exact targets, and excluded callable families. Specialization tests
own recursive substitution and separate closed-static targets. Type/HIR tests
own trivial storage, every internal callable family, exact compatibility, and
excluded containers, aliases, casts, and comparisons. MIR and verifier tests
own stabilized callee order, every argument/result carrier, path and loop
lifetimes, non-null provenance, result security, and malicious mutations.
Static-lifecycle and backend tests own exact-signature candidate expansion,
retention, effects, layout, ABI classification, symbol addresses, and indirect
instruction selection.

The `function_values` golden group is the source-facing conformance matrix. It
covers imported and private references, storage and reassignment,
virtual/interface transport, closed generic target identity, recursive and
chained calls, mixed register/stack pressure, every ownership/result family,
callee-before-argument failure suppression, reverse cleanup, panic traces,
static effects, exclusions, and exact mismatch families. Full golden
determinism repeats compilation, linking, execution, diagnostics, streams, and
status. The cross-process `function_value_composition` test independently
compares token-through-assembly products including planned static lifecycle
and runtime-trace metadata.

## Determinism and process isolation

Phase dump tests call the same renderer repeatedly and compare exact text.
`pipeline_determinism` compares tokens, AST, resolved, HIR, MIR, and assembly
products for representative object-lifetime, polymorphism, shared-ownership,
optional-value, array, primitive-integer-operation, and string programs from
two independent test processes. Its module cases additionally permute root
option order, equivalent root spellings, source creation order, import
declaration order, and logical versus positional selection of the same rooted
entry, then compare canonical graph, resolved, HIR, MIR, assembly, and
diagnostic products. The ordinary golden target invokes each compiler and
native process once. `make golden-determinism-test` invokes `skac` twice for
every successful assembly and compile failure, comparing assembly or
diagnostic bytes, and executes every native case twice before evaluating the
checked-in expectations.
External stdout and stderr files are exact byte expectations unless their spec
selects a reviewed partial matcher; an omitted stream expectation requires
empty output. `argv_file` records become byte-preserving Unix arguments and are
applied identically to both executions; the operating system remains
responsible for element zero. The focused `make golden-expectations-test` suite
owns byte matching, fixture ownership, argument decoding, and escaped mismatch
reporting independently of real compiler execution.

Panic goldens cover every failure that a compact source program can trigger,
including explicit dynamic messages, cast and optional failures, array and
string bounds, invalid allocation requests, and valid host-allocation
exhaustion. Counter saturation cannot be reached by a tractable source fixture:
backend native tests inject the optional-guard and ownership-count boundary
states into otherwise compiler-generated assembly, use the same exact stderr
expectations, and separately prove that invalid ownership states remain silent
hard traps. The exhaustive termination-selector test covers every
`MirTerminationReason`, requires one reporter call and no `ud2`, and therefore
keeps new compiler-known failures inside the common reporting boundary.

Preserve this process isolation for behavior affected by identity allocation,
table traversal, filesystem paths, labels, diagnostics, or formatting. A
single-process equality check is useful but does not replace it.

## Robustness

`make compiler-test` includes the fixed-seed bounded hostile frontend inputs
and structured MIR mutations with the rest of the compiler suite.
`make robustness-long` reruns the generated frontend cases with a larger
`SKALD_ROBUSTNESS_CASES` value. It is intended for less frequent external,
scheduled, or pre-release validation and remains reproducible.

When robustness testing finds a defect, retain the smallest focused regression
at the owning layer. Add corpus data only when the bytes or source are clearer
and more reusable than constructing the case in Rust.

## Structural bracket coverage

Structural indexing and slicing coverage follows the normalization boundary.
Syntax tests own neutral bracket shapes, punctuation, spans, recovery, and AST
dumps. Resolver tests own array precedence, class/interface protocol shape,
privacy, static receiver classification, canonical identities, and conversion
to ordinary calls. Type-check tests own receiver access, argument modes and
types, optional slice-bound injection, dispatch, produced/shared receiver
anchors, and deterministic HIR. MIR and static-lifecycle tests prove ordinary
call ownership, cleanup, verification, and effects without structural IR.
The `tests/golden/structural_indexing/` and `tests/golden/standard_vec/`
groups own complete native dispatch, evaluation, failure, `Str`, and `Vec<T>`
observations.

Use these focused commands:

```text
cargo test --locked -p skald-compiler syntax::tests::bracket_projections
cargo test --locked -p skald-compiler resolve::tests::structural_indexing
cargo test --locked -p skald-compiler typeck::tests::structural_indexing
cargo test --locked -p skald-compiler mir::tests::structural_indexing
cargo test --locked -p skald-compiler passes::static_lifecycle::tests::structural_indexing
./scripts/golden.sh --determinism full --filter 'structural_indexing/**'
./scripts/golden.sh --determinism full --filter 'standard_vec/**'
```

The phase-dump regression requires `BracketProjection` only in the AST and
the same canonical ordinary method identities in resolved IR, HIR, and MIR.
Diagnostic regressions preserve separate owners for unsupported receivers,
missing or malformed protocols, privacy, explicit dereference, mutability, and
ordinary call compatibility.

## Generic-class and vector coverage

Focused generic tests exercise the same public closed pipeline used for
ordinary classes. Resolution tests own template terms,
requirements, cache transitions, provenance, qualified semantic names, and
atomic failed-publication behavior. Type-check tests assert exact HIR
identities and lifecycle plans. Preliminary/planned/final MIR and static-plan
tests assert that generated classes use ordinary closed operations. MIR
mutation tests reject concrete identities absent from the closed program
tables, and backend/native tests cover substituted register, stack, and hidden
result behavior, layouts, symbols, dispatch, allocation, finalization, and
cleanup. The matrix covers primitive and
exact-class arguments, user and synthesized lifecycle, unavailable copy and
assignment operations, aliases and value boundaries, stored fields and
statics, nested arrays and recursive optionals, shared exact/base/interface/
`Obj` owners, optional owners, shared optional boxes, destruction order,
index failure plans, and backing anchors.

`tests/golden/generic_classes/` freezes definition, arity, wrong-kind, bound,
contextual, lifecycle-path, recursion, module-lookup, and privacy failures, as
well as native lifecycle/ownership behavior, checked array failure, and
multi-module execution.
The pipeline-determinism suite recompiles a multi-module nested specialization
in independent processes while permuting source creation and provider roots;
it compares graph, diagnostics, resolved IR, HIR, planned MIR, final MIR, and
static lifecycle plans without depending on assembly. Bounded frontend
robustness mutates angle brackets, commas, `where`, constraints, and nested
applications and requires deterministic recovery on every mutation.

Generic-interface syntax coverage lives with the interface and shared generic
parser owners. It checks parameters, interface-level bounds, applied claims,
nested applications and closers, punctuation-preserving dumps, contextual
`where`, malformed-list recovery, and unchanged comparison/shift parsing.
Resolver and type-check tests cover canonical closed identities, materialized
requirements, exact ordinary and specialized class conformance, inherited
overrides, generic class/interface bounds, definition-site template
requirement selection, explicit closed requirement mappings, ordinary bound
dispatch, ownership-sensitive results, diagnostics, modules, and dumps.

`tests/golden/standard_vec/` owns `std::vec::Vec<T>` behavior. The matrix covers
primitives, `Str`, nested optionals, exact inline lifecycle, nested arrays,
shared strings, shared interface and heterogeneous `shared Obj` owners,
copy/assignment independence, prompt inline and last-owner cleanup, capacity
growth, signed logical indexing, all slice omission and bounds shapes,
independent slice results, self/overlap snapshot replacement, nested closed
specializations, all six public panic boundaries, a template and argument
split across modules, bare-interface rejection, and unavailable element
lifecycle. A compiler-owner test additionally pins ordinary specialized
resolved/HIR selection. Full golden determinism repeats every successful
compile and native run and every compile failure. The non-gating
`make generic-vec-benchmark` procedure measures representative growth, copy,
pop, and clear behavior without accepting or rejecting host timing.

## Optional-value coverage

The implemented
[optional-values compiler contract](../compiler/OPTIONAL_VALUES.md#test-obligations)
requires coverage at every owning layer. Current lexer and parser tests own
tokens, contextual words, spans, precedence, bounded nesting, reserved forms,
and recovery. Current resolution tests own recursive interned identities and
source-shaped expression nodes. Optional type-check and HIR tests own
expected-type-directed `none`, exact injection, initializer ranking, fields,
calls/results, exact signatures, copy, assignment, presence, unwrap,
truthiness rejection, external rejection, checked class payload consumers,
inline optional-container alias access/forwarding, and reserved-form
boundaries. MIR tests own initialized places, explicit
operations, CFG joins, aggregate calls, synthesized field lifecycle, checked
view/anchor order, and exact failure edges; verifier mutations break one
invariant at a time, including missing, mismatched, leaked, and reordered
guards. Backend tests own layout, instruction selection, guard counts, hidden
destinations, register/stack pressure, traps, and native execution.

Optional-owner coverage includes shared class/interface/`Obj` up-views,
zero-niche realization, secured-owner unwrap, and virtual/interface dispatch
while preserving the inline optional and checked-view matrix.
Native golden specs use `exit = "failure"` when the contract requires only
unsuccessful termination; exact trap signals and shell-normalized statuses are
not portable language observations. Optional full-phase determinism and the
MIR mutation corpus cover dumps, verification, and backend rejection, while
the extended robustness suite mutates optional punctuation deterministically.

### Shared optional box coverage

Positive syntax, resolution, type-check/HIR, MIR, backend, and native coverage
exercises construction, exact metadata, unpublished initialization,
publication/adoption, secure replacement, stored and array positions, internal
calls, cleanup, malformed protocol states, and unsupported external signatures.
Complete implementation evidence includes:

- source type grouping, arbitrary outer optional depth, `shared? P?`
  provenance, `new P?()`/`new P?(expression)` precedence, recovery, spans, and
  deterministic syntax/resolved dumps;
- exact optional allocation identities, polymorphic class/base/interface/`Obj`
  box-view identities, invariant non-object targets, casts, impossible
  relations, and deterministic cross-module interning;
- absent, injected, `some`, named-copy, produced-transfer, nested optional,
  optional-array, and optional-shared-owner box construction with allocation
  only after failure-capable source checks and publication only after complete
  wrapper initialization;
- named owner copy versus independent `new P?(*box)` allocation, owner
  replacement that leaves aliases on the old box, deterministic last-owner
  optional cleanup, and compile-time rejection of whole-pointee assignment or
  mutable whole-wrapper aliases through exact and polymorphic views;
- explicit presence, owning copy, checked unwrap, primitive extraction,
  read-only exact-wrapper aliases, owner anchors, optional guards, contained
  object mutation, and absence/guard failures;
- base, interface, and `Obj` up-views, checked downcasts, type tests, virtual and
  interface dispatch, exact dynamic class retention while absent or present,
  and deliberate slicing only when a complete object wrapper is copied to an
  eligible exact inline optional destination;
- locals, inherited fields, internal arguments/results, methods, interfaces,
  overrides, initializer overloads, explicitly initialized statics,
  temporaries, arbitrary outer optional box-owner layers, synthesized
  lifecycle, invariant inline/shared-outer arrays, optional owner elements,
  compatible object-box views, and distinct absent default boxes per slot;
- malformed MIR for target confusion, allocation origin, initialization and
  publication order, owner loss, pre-publication access, metadata/finalizer
  mismatch, guard/anchor imbalance, mutable access, and duplicate cleanup;
- x86-64 layout/alignment/overflow, descriptor and finalizer determinism,
  register/stack pressure, allocation failure, native lifecycle traces,
  assembly acceptance, and unchanged runtime ABI version 9; and
- focused phase tests, positive/compile-failure/runtime-failure goldens,
  independent-process determinism, `make check`, `make msrv-check`, and
  `make robustness-long` at the roadmap tasks that alter Rust syntax or
  frontend recovery.

## Private cell field coverage

Private cell tests follow the authorization across its trust boundaries.
Syntax and resolution tests own contextual spelling, exact spans, identities,
privacy, and specialization. Type-checking tests own the exact declaring-class
whole-field decision and neighboring read-only exclusions. MIR tests require
explicit evidence on every assignment carrier and mutate its field, endpoint,
owner, access, family, origin, and liveness independently in preliminary and
final products. Backend tests retain ordinary place addressing, layout,
callable ABI, deterministic assembly, and runtime ABI version 9. The focused
native matrix covers scalar, class, optional, shared-owner, array, lifecycle,
alias-anchor, inheritance, checked-view, virtual/interface, eligible function-
value, closed-generic, and cross-module composition. Compile-failure goldens
preserve privacy, modifier, and nested-mutation exclusions. Independent
process tests compare complete phase products and diagnostics.

## Final-field coverage

Implemented declaration tests follow final evidence through each phase owner.
Syntax and resolution cover contextual canonical modifier order, recovery,
exact spans, ordinary identities, and closed specialization for instance and
static declarations. HIR/MIR tests cover marker propagation and malformed
metadata; backend tests confirm unchanged layout inputs and private writable
static slots. Driver tests exercise final statics through verified lifecycle
synthesis and target emission.

Capability, HIR, and MIR tests must cover synthesized base-first and direct-
field assignment plans plus explicit authorization on every scalar, class,
optional, shared-owner, and array carrier. Verifier mutation tests independently
forge marker, endpoint, lifecycle owner, directness, family, initialization,
liveness, guard, anchor, ownership, and cleanup facts in preliminary and final
products. Static-lifecycle tests require explicit initialization, prohibit the
zero-default route and later root writes, and retain dependency, publication,
failure, and reverse-shutdown certificates.

Native and compile-failure goldens distinguish direct final-field writes
from mutable complete-value replacement; cover user assignment with zero,
repeated, conditional, and loop-carried writes; and compose inheritance,
generics, aliases, produced reads, dispatch, optionals, arrays, shared owners,
function values, and static initialization. Backend tests retain ordinary
layout, addressing, calling conventions, symbols, deterministic assembly, and
runtime ABI version 9. Standard-library migration tests expose each primitive
box payload as a public final field while preserving direct reads, whole-box
assignment, exact equality, and domain-separated hashing.

## Static-field coverage

Static-field tests are divided by responsibility: syntax recovery owns
modifier and declaration shape; resolution owns identities, collisions,
privacy, inheritance, shadowing, qualification, and cyclic modules; type
checking and HIR own separate zero-default and explicit stored-value matrices.
Preliminary, planned, and final MIR tests own publication, ownership, effects,
evidenced dependencies, deterministic plan indices, and reverse destruction;
mutation tests break each certificate invariant independently. Backend tests
own private symbols, layout, relocations, sections, startup/finalizer calls,
result preservation, native order, and the absence of per-access guards.
Compile-failure goldens cover syntax, storage, access, overloads, wrong-kind
uses, direct/transitive initialization cycles, destructor cycles, and
conservative dynamic dispatch. Native goldens cover every supported stored
family, imports, inherited aliases, privacy, side-effect and dependency order,
replacement, and reverse destruction. Cross-process phase tests and the golden
runner's full audit compare deterministic diagnostics, products, assembly,
stdout, stderr, and status; passing sandboxes are removed after each run.

# Golden Stream Matcher Lists Roadmap

Status: complete.

This roadmap lets one captured process stream own any number of independent
byte-match contracts. It preserves the existing singular expectation syntax
as shorthand, adds an explicit matcher-list form, and applies the same model to
native stdout, native stderr, compile-fail stdout, and compile-fail stderr.
The result lets multi-error compile-fail programs retain every intentional
invalid case without freezing unrelated diagnostic detail.

## Scope and invariants

- Keep schema version 1 frozen and fully supported. Introduce matcher lists and
  explicit compile-fail stdout expectations in schema version 2.
- Accept the existing singular stream syntax in both schema versions and
  normalize it to a one-element matcher collection internally.
- Add `matches` as a nonempty AND-list of independent matchers. Every matcher
  examines the same complete captured byte stream, declaration order affects
  reporting only, and all matchers must succeed.
- Let every matcher select `exact` (the default), `starts-with`, or `contains`
  and exactly one inline or external byte source. Preserve byte-exact loading
  without UTF-8, newline, path, whitespace, or escape normalization.
- Permit mixed matcher modes and mixed inline/file sources. An exact matcher
  may coexist with partial matchers even though it normally dominates them.
- Allow an optional nonempty matcher `name`; require names to be unique within
  one stream and otherwise identify matchers by stable declaration index.
- Evaluate every matcher independently. Compatible matchers may be satisfied
  by the same actual bytes, and one mismatch or load failure must not prevent
  the runner from reporting other matcher outcomes.
- Keep `ignore = true` as a singular whole-stream policy. Reject `ignore`
  inside `matches`, an empty matcher list, a matcher without exactly one byte
  source, duplicate names, empty partial fragments, and simultaneous singular
  and list forms.
- Apply matcher collections symmetrically to native-run stdout and stderr and
  compile-fail compiler stdout and stderr. Omitted native streams retain exact
  empty defaults; omitted compile-fail stdout defaults to exact empty;
  compile-fail stderr remains required, nonempty, and non-ignored.
- Preserve successful-compilation silence as a compiler orchestration
  invariant rather than exposing per-run expectations for a build shared by
  multiple native runs.
- Preserve exact-only temporary output-file expectations. They are files
  observed after execution, not captured process streams.
- Keep determinism independent from golden matching: repeated processes still
  compare complete status, stdout, stderr, assembly, and output-file
  observations before matcher expectations are evaluated.
- Preserve canonical test IDs, selection, scheduling, fixture ownership,
  process execution, and artifact retention behavior.
- Collect every failed matcher in deterministic stream and declaration order
  across human, JSON, and JUnit reports.
- Do not add regular expressions, normalization, ordered matcher groups,
  OR/NOT expressions, blessing, implicit snapshot updates, or diagnostic-aware
  parsing in this roadmap.
- Keep the root Makefile as the local and external automation boundary; do not
  add repository CI configuration or new runner dependencies.

The canonical schema-2 list form is:

```toml
[test.expect.stderr]

[[test.expect.stderr.matches]]
name = "wrong alias type"
match = "contains"
inline = """error[TYP005]: alias argument has type `Right`, expected `Left`
 --> tests/golden/aliases/example.ska:8:13"""

[[test.expect.stderr.matches]]
name = "invalid result"
match = "starts-with"
file = "expected/invalid-result.stderr"
```

The equivalent inline-table spelling remains valid TOML for compact data:

```toml
[test.run.expect.stdout]
matches = [
  { match = "starts-with", inline = "header" },
  { match = "contains", file = "expected/result-fragment.stdout" },
]
```

## Progress

- [x] GM0 — Build the independent matcher engine
- [x] GM1 — Publish the symmetric schema-2 stream contract
- [x] GM2 — Complete matcher-aware planning and reporting
- [x] GM3 — Migrate multi-error compile-fail contracts
- [x] GM4 — Harden, validate, document, and close

## PR-sized implementation sequence

### GM0 — Build the independent matcher engine

**Purpose:** Establish one byte-comparison abstraction and structured outcome
model before schema, compiler, native execution, or reporting depends on
matcher collections.

- [x] Introduce cohesive matcher and matcher-outcome types under the
      expectation facade, including optional names, declaration indices,
      match mode, expected bytes, matched offsets, mismatches, and load
      failures.
- [x] Implement collection comparison as an AND operation over the same actual
      bytes without short-circuiting, while preserving current exact,
      starts-with, contains, ignored, and binary-byte behavior.
- [x] Keep the current singular comparison entry point as a compatibility
      wrapper until all consumers move to the collection result.
- [x] Define deterministic outcome ordering and make successful, mismatched,
      and unloadable matchers independently inspectable without duplicating
      captured actual bytes in every in-memory result.
- [x] Keep substantial comparison algorithms and tests in cohesive files
      behind the concise expectation-module facade; avoid compiler- or
      native-specific matching helpers.

**Tests:** Focused unit tests for zero-byte exact matching, nonempty partial
matching, arbitrary collection length, mixed modes, shared satisfying bytes,
multiple simultaneous mismatches, matcher names and indices, binary expected
data, and independent load failures; `cargo test --locked -p skald-golden`;
formatting; Clippy; and diff hygiene.

**Exit criteria:** One generic API can evaluate and retain every outcome from a
nonempty matcher collection, existing singular callers behave identically,
and no process-stage module contains byte-search logic.

### GM1 — Publish the symmetric schema-2 stream contract

**Purpose:** Extend the versioned specification and carry matcher collections
through planning and execution for every declarative captured stream.

- [x] Separate specification-version validation from repository-configuration
      version validation: specs accept versions 1 and 2, while repository
      configuration remains version 1 until its own contract changes.
- [x] Add strict raw and validated matcher-list data with optional names and
      typed byte sources. Keep singular `match`/`inline`/`file` syntax as
      shorthand in schema 2 and retain its exact existing behavior in schema
      1.
- [x] Reject schema-2-only `matches` and compile-fail stdout fields from schema
      1 with precise field paths rather than silently widening the frozen
      contract.
- [x] Validate the stream invariants in one shared path used by native stdout,
      native stderr, compile-fail stdout, and compile-fail stderr; preserve the
      stricter nonempty and non-ignored compile-fail stderr rules.
- [x] Replace singular validated and resolved stream representations with
      nonempty matcher collections plus the whole-stream ignore case; resolve
      every external source canonically and preserve singular convenience
      access only where it cannot conceal multiplicity.
- [x] Add compile-fail stdout to the typed expectation and resolved plan,
      default it to exact empty, and compare it through the same matcher engine
      as compile-fail stderr instead of the current unconditional unexpected-
      stdout branch.
- [x] Move native stdout/stderr and compile-fail stdout/stderr execution to the
      generic collection comparator, retaining all matcher mismatches and load
      failures while leaving full-output determinism checks unchanged.
- [x] Keep the crate facades selective and update public API exports without
      exposing raw schema types or process-specific matcher variants.

**Tests:** Schema integration tests for version separation, schema-1
compatibility and rejection, schema-2 singular shorthand, inline-table and
array-of-table lists, arbitrary list length, mixed modes and sources, optional
unique names, every invalid union, exact-empty behavior, and compile-fail
stdout defaults; planning and fake-process tests for all four stream positions;
focused golden-runner tests; formatting; Clippy; MSRV; and diff hygiene.

**Exit criteria:** Schema-2 specs can use singular or list expectations in all
four declared stream positions, schema 1 remains frozen, every matcher affects
pass/fail status, and existing schema-1 fixtures run without migration.

### GM2 — Complete matcher-aware planning and reporting

**Purpose:** Make matcher collections fully observable and diagnosable through
the runner's planning, ownership, human, and machine interfaces.

- [x] Teach the fixture ownership audit to own every file-backed matcher and
      preserve deterministic orphan and duplicate-owner diagnostics.
- [x] Render every matcher in `--explain` with its stream, declaration index,
      optional name, mode, and canonical inline/file source.
- [x] Extend execution and report models with ordered per-matcher results while
      retaining one captured stdout or stderr payload per process observation.
- [x] Report every mismatch and expectation-load failure together, using the
      optional matcher name when present and the stable declaration index
      otherwise; keep expected/actual byte escaping and bounded diffs.
- [x] Extend JSON additively with structured matcher results. Preserve legacy
      singular policy and offset fields for one-matcher streams, and define
      their documented neutral representation for matcher collections.
- [x] Emit deterministic JUnit failures for every failed matcher and keep
      human output concise by showing shared actual stream data once when
      several matchers fail.
- [x] Cover passing stream details under `--show-output`, cancellation,
      fail-fast, scheduler ordering, and retained-artifact reports without
      changing canonical leaf order.

**Tests:** Planning tests for all referenced matcher files and explain output;
report-model and renderer tests for named and unnamed matches, mixed successes
and failures, multiple load errors, binary data, exact mixed with partial
matching, human truncation, additive JSON fields, XML escaping, JUnit failure
counts, and canonical ordering; `make golden-expectations-test`; runner tests;
formatting; Clippy; and diff hygiene.

**Exit criteria:** Every planning and reporting surface represents matcher
collections without ambiguity, all failures are visible in stable order, and
singular schema-1 reports retain their established meaning.

### GM3 — Migrate multi-error compile-fail contracts

**Purpose:** Use matcher lists for the existing large invalid-case programs so
every intentional error remains source-to-diagnostic coverage rather than
being silently present after one asserted leading diagnostic.

- [x] Audit the complete compiler stderr for
      `integer_bitwise_operator_types`, `eager_boolean_operator_types`, and
      `short_circuit_boolean_operator_types`; add one named contains matcher
      per intentional invalid operator case without deleting source cases.
- [x] Audit `malformed_byte_literals`, `malformed_hexadecimal_literals`, and
      `malformed_numeric_literals`; match every intended primary lexical or
      parse diagnostic while avoiding incidental recovery notes.
- [x] Audit `produced_alias_invalid_sources` and `primitive_type_errors`; match
      every intended primary type error while retaining their complete invalid
      matrices.
- [x] Upgrade only the owning specifications to schema 2 and keep unrelated
      singular expectations unchanged.
- [x] Use stable fragments that own diagnostic code, primary message, and
      source location while allowing richer labels, notes, suggestions, and
      stack-like context to evolve.
- [x] Confirm that the matcher set describes intentional diagnostics rather
      than merely snapshotting every cascading message emitted today; add
      lower-layer unit coverage if the audit exposes an unowned diagnostic
      matrix responsibility.

**Tests:** Focused filters for aliases, operators, and primitives; explicit
assertions that removing or changing any one matcher makes its leaf fail;
compile determinism for the migrated leaves; `make golden-test`; runner schema
and planning suites; documentation checks; and diff hygiene.

**Exit criteria:** All eight multi-error sources remain intact, every intended
invalid case has an explicit named diagnostic matcher, no case relies on
unasserted trailing stderr, and the complete ordinary golden suite passes.

### GM4 — Harden, validate, document, and close

**Purpose:** Audit the completed stream abstraction, document only current
behavior, prove repository-wide compatibility, and archive the roadmap.

- [x] Audit the spec, plan, expectation, compile, execute, and report facades
      for duplicated collection traversal or process-specific matching logic;
      extract only cohesive repeated responsibilities and keep implementation
      modules private behind selective re-exports.
- [x] Stress arbitrary matcher counts, large captured streams, large external
      fragments, non-UTF-8 bytes, overlapping compatible matches, mixed exact
      and partial policies, missing external data, parallel completion, and
      deterministic final ordering.
- [x] Update the golden fixture guide as the authoritative schema-2 syntax and
      semantics reference, including singular shorthand, independent AND
      behavior, naming, compile-fail stdout defaults, exact dominance, and
      appropriate partial-diagnostic ownership.
- [x] Update development testing and debugging guidance where matcher lists
      change contributor workflows; keep one authoritative schema description
      and link to it elsewhere.
- [x] Search code, tests, Make output, and living documentation for assumptions
      that a stream has exactly one matcher or that compile-fail stdout is
      unconditionally empty; remove stale wording and compatibility helpers.
- [x] Run the complete repository gate, full determinism audit, MSRV check,
      documentation check, formatting, Clippy, and diff hygiene from an
      artifact-free snapshot.
- [x] Mark every task complete, archive this roadmap, update active and archive
      indexes and links, and record any genuinely deferred matcher algebra in
      a separate indexed discoveries document rather than expanding scope.

**Tests:** `make golden-runner-test`; `make golden-expectations-test`;
`make golden-test`; `make golden-determinism-test`; `make check`;
`make msrv-check`; documentation validation; and `git diff --check`.

**Exit criteria:** Matcher lists are a documented, symmetric, deterministic,
and maintainable stream contract; schema-1 fixtures retain their behavior;
the migrated diagnostic matrices own every intended error; all repository and
supported-toolchain gates pass; and the completed roadmap is archived.

## Ordering and dependencies

GM0 isolates literal byte matching and result ownership before representation
or process code changes. GM1 then publishes the versioned syntax and moves all
declared process streams together, avoiding a compile-fail-only abstraction.
GM2 completes ownership and reporting before the corpus relies on many
matchers for useful failures. GM3 performs the behavior-preserving fixture
migration only after the runner contract is fully observable. GM4 audits the
result and closes the roadmap after both old shorthand and new matcher lists
have passed ordinary and determinism gates.

No active roadmap blocks this work. GM0 must precede GM1; GM2 depends on the
resolved and execution models from GM1; GM3 depends on the complete reporting
surface from GM2; and GM4 follows the corpus migration.

# Panic Runtime Trace Roadmap

Status: in progress; TRACE2 is next.

Implement the frozen panic runtime-trace design on Linux x86-64 so every
source-authored callable maintains an allocation-free shadow frame, panic
reports exact source locations and newest-first call chains, and
`--omit-runtime-trace` removes the complete compiler-generated cost. The
[frozen design record](../archive/PANIC_RUNTIME_TRACE_DESIGN_PROPOSAL.md) owns
the reviewed decisions; this roadmap owns implementation order and evidence.

## Scope and invariants

- Implement runtime ABI version 9 with marker `ska_rt_abi_v9`, the hidden C11
  thread-local trace top, fixed trace records, and allocation-free rendering.
- Keep `_Noreturn ska_rt_panic(const uint8_t*, uint64_t)` unchanged and retain
  panic's non-returning, non-unwinding, uncatchable behavior.
- Add no source syntax, AST/HIR/MIR trace operation, exception behavior,
  native unwinding, DWARF walking, or foreign-frame inspection.
- Keep hard compiler/runtime defects silent; they never acquire a panic trace.
- Give each eligible source callable exactly one 16-byte native-frame record;
  generated wrappers and helpers receive none.
- Use inline Linux x86-64 local-exec TLS access with `r11` as transient
  caller-saved scratch: six push instructions, two pop instructions, and two
  instructions per executed location replacement.
- Reserve no general-purpose register. Every register remains available to a
  future allocator outside the short trace sequences.
- Trust generated linked-pop bookkeeping without a normal-path equality or
  underflow check. Null `previous` is the valid outermost state.
- Emit deterministic semantic context names, escaped stable source paths, and
  only the contexts and locations actually referenced by generated code.
- Attribute generated-helper and runtime failures to their initiating source
  operation. Ordinary source-authored standard-library and lifecycle bodies
  remain visible; the bodyless panic intrinsic remains absent.
- Render at most 256 newest frames and append the uncounted outer-frame marker
  when a further link remains.
- Keep the production compiler on complete omission while backend trace
  support is incomplete. Make tracing default-on only after frame maintenance
  and every panic-observable location family are covered.
- Treat Linux AArch64 as a future target. This roadmap adds no target entry,
  instruction sequence, or AArch64 test obligation.
- Use the Makefile as the repository automation interface; add no CI system.

## Progress

- [x] TRACE0 — Establish runtime ABI version 9 and trace rendering
- [x] TRACE1 — Add explicit backend trace input and deterministic metadata
- [ ] TRACE2 — Maintain source-callable trace frames with inline TLS
- [ ] TRACE3 — Record source calls and reporter failure locations
- [ ] TRACE4 — Complete generated-helper and runtime-failure attribution
- [ ] TRACE5 — Expose default-on tracing and migrate native observations
- [ ] TRACE6 — Measure overhead, harden determinism, and close the rollout

## PR-sized implementation sequence

### TRACE0 — Establish runtime ABI version 9 and trace rendering

**Purpose:** Land the independently testable runtime half first. A null TLS
top preserves current program output, so the incompatible ABI transition can
be made atomically without requiring compiler-generated frames in the same PR.

- [x] Add the fixed `SkaRtTraceContext`, `SkaRtTraceLocation`, and
  `SkaRtTraceFrame` layouts plus the zero-initialized hidden
  `_Thread_local SkaRtTraceFrame* ska_rt_trace_top` definition.
- [x] Advance the runtime header, runtime definition, backend link reference,
  contract tests, link-mismatch tests, and all current version assertions from
  ABI 8/`ska_rt_abi_v8` to ABI 9/`ska_rt_abi_v9` in one change.
- [x] Preserve the existing `ska_rt_panic` signature and first-line payload
  behavior, including embedded zeroes/newlines and null-with-zero-length.
- [x] Extend the reporter to walk newest first, write length-delimited name and
  path bytes, convert `u64` line/column values with fixed local buffers, cap at
  256 rows, and emit the exact omitted-outer-frames marker.
- [x] Reuse the existing retrying direct-write path for every fragment; do not
  introduce `stdio`, heap allocation, recursion, a capacity vector, or a
  recoverable output failure.
- [x] Expand the direct C harness to construct empty, single, nested, replaced,
  256-frame, longer-than-cap, and cyclic chains, and to cover output failure
  after trace rendering begins.
- [x] Prove reporter allocation independence with a direct harness whose
  allocation hooks hard-fail if rendering calls them.
- [x] Update the runtime ABI and related living documents to distinguish the
  implemented version-9 reporter from not-yet-emitted compiler trace state.

**Tests:** `make runtime-test`; focused compiler/backend marker and
link-mismatch tests via
`cargo test --locked -p skald-compiler runtime_abi`; `make docs-check`;
`make check`; `make msrv-check`; `git diff --check`.

**Exit criteria:** Version-9 compiler output links only with the version-9
runtime; direct runtime callers with null top retain the exact single-line
panic; every valid synthetic trace scenario renders exact bytes without
allocation; all precondition defects and failed writes remain non-returning.

### TRACE1 — Add explicit backend trace input and deterministic metadata

**Purpose:** Establish one source-aware target boundary and one cohesive
metadata owner before frame or call lowering depends on trace identities.

- [x] Replace the backend's MIR-only emission argument with an explicit input
  carrying final verified MIR, read-only source access when enabled, and a
  typed runtime-trace policy.
- [x] Thread that input through both request and singleton driver pipelines
  without copying source text into MIR or exposing AST, resolved IR, or HIR to
  the backend.
- [x] Keep production driver emission explicitly omitted during this staged
  task; enabled metadata paths are exercised through focused backend tests
  until location coverage is complete.
- [x] Introduce a target-private trace metadata owner that maps callable and
  span identities to stable symbols and emits only requested records.
- [x] Format semantic names for functions, instance/static methods,
  initializer parameter signatures and modes, user copy/assignment/destruction
  bodies, and explicit static-field initializer bodies. Do not derive display
  names from mangled symbols or source-order initializer ordinals.
- [x] Select module-provider-relative paths from provenance; retain the
  configured positional display spelling outside roots and the frozen absolute
  fallback where necessary.
- [x] For the in-memory singleton pipeline, retain the API-supplied source path
  as its trace display path instead of replacing it with the synthetic
  `main.ska` module fallback.
- [x] Escape backslash, LF, CR, tab, other control bytes, and non-UTF-8 host
  bytes into one safe display line before records are emitted.
- [x] Resolve only used span starts through `SourceDatabase`, convert checked
  one-based line and Unicode-scalar column values to `u64`, and return a
  structured backend error rather than truncating an unrepresentable value.
- [x] Extend the target assembly model and emitter with deterministic interned
  byte strings, 32-byte contexts, and 24-byte locations in
  relocation-read-only data, ordered by semantic identity/location rather
  than address or hash traversal.
- [x] Preserve MIR dumps and verification exactly: no trace record ID,
  rendered path, TLS operation, or frame offset enters MIR.
- [x] Add shared backend test support that can emit the same MIR with tracing
  enabled or omitted and retains the associated `SourceDatabase` for precise
  assertions.

**Tests:** Source-location unit tests including Unicode columns; focused
metadata, naming, path escaping, interning, section, relocation, unused-record,
and deterministic-order tests via
`cargo test --locked -p skald-compiler runtime_trace_metadata`; public backend
and driver composition tests; system assembler acceptance; `make check`;
`make msrv-check`; `git diff --check`.

**Exit criteria:** Enabled backend tests produce byte-exact deterministic
context/location metadata from existing MIR spans and source provenance;
omitted tests perform no trace-only lookup and emit no trace bytes, symbols,
records, or relocations; phase products and dumps remain target-independent.

### TRACE2 — Maintain source-callable trace frames with inline TLS

**Purpose:** Implement the fixed-cost linked activation protocol independently
of interior location coverage, while the production driver still requests
complete omission.

- [ ] Classify every `MirProgram::executable_definitions` source body as an
  eligible context and keep generated array, ownership, copy, finalization,
  static coordinator, process wrapper, and target thunk functions ineligible.
- [ ] Extend `FrameLayout` with an optional aligned 16-byte record whose first
  word is `previous` and second word is `location`; include it before final
  16-byte frame rounding and retain checked displacement/size failures.
- [ ] Use the callable definition span as the initial location until a more
  precise panic-observable operation replaces it.
- [ ] Add explicit target-machine instructions and Intel-syntax emission for
  local-exec TLS load/store and RIP-relative location-address materialization;
  require `R_X86_64_TPOFF32` against hidden `ska_rt_trace_top`.
- [ ] Emit the six-instruction push after incoming parameters are preserved and
  before body execution. Publish the top pointer only after both record words
  are initialized.
- [ ] Emit an unchecked two-instruction pop on every normal return before the
  final scalar, floating, shared, optional-shared, unit, or caller-destination
  result reload/return sequence.
- [ ] Model `r11` as a local trace-sequence clobber rather than global target
  reservation and preserve the existing no-unpreserved-callee-saved-register
  invariant.
- [ ] Add recursive and mixed-return native probes using the real runtime to
  prove newest-first activation order, balanced normal returns, null outermost
  restoration, and intact ABI results.
- [ ] Require omitted output to preserve the pre-trace frame sizes and exact
  assembly shape, including no TLS relocation or scratch constraint.

**Tests:** Frame allocator and exact assembly tests via
`cargo test --locked -p skald-compiler runtime_trace_frame`; assembler and ELF
relocation inspection; recursive/mixed-return native backend tests; omission
comparisons; `make check`; `make msrv-check`; `git diff --check`.

**Exit criteria:** Every eligible source activation publishes exactly one
valid linked record with the frozen six/two instruction sequences, every normal
return restores TLS without changing results, generated functions publish no
record, and omitted assembly is byte-for-byte free of trace effects.

### TRACE3 — Record source calls and reporter failure locations

**Purpose:** Make all explicit source-call and central reporter boundaries
precise before auditing lower-level generated helper calls.

- [ ] Add one target-private location-replacement primitive that emits the
  frozen RIP-relative `lea` plus frame-home store and is unavailable to
  ineligible generated functions.
- [ ] Replace the active location before direct, static, virtual, interface,
  and external calls using the originating MIR call span.
- [ ] Cover ordinary initializer, user copy constructor, copy assignment,
  destructor, and other source-call operations selected from MIR while
  preserving receiver-before-arguments and left-to-right evaluation.
- [ ] For indirect calls, emit the location replacement before loading the
  dispatch target into `r11`; prove the trace scratch cannot destroy that
  target or marshalled arguments.
- [ ] Replace the location on the taken failure edge immediately before both
  dynamic explicit-panic reporting and every static `MirTerminationReason`
  reporter call.
- [ ] Keep successful checked casts, optional access, bounds, division,
  remainder, shifts, and checked primitive casts free of failure-site stores.
- [ ] Preserve hard-trap paths without trace replacement or reporter calls.
- [ ] Add focused native chains that distinguish callee failure locations from
  caller call sites across recursion, direct/static methods, virtual/interface
  dispatch, external boundaries, and source-authored lifecycle bodies.

**Tests:** Focused call, terminator, cast, optional, array-bound, shift,
division, primitive-cast, dispatch, and lifecycle tests via
`cargo test --locked -p skald-compiler runtime_trace_location`; exact
instruction-order and successful-path absence assertions; native real-runtime
tests; `make check`; `make msrv-check`; `git diff --check`.

**Exit criteria:** Every source-level call and central reporter edge has the
exact frozen two-instruction replacement in the correct order, every checked
success path avoids the store, and native traces show callee failure followed
by the exact caller call site without artificial intrinsic or runtime frames.

### TRACE4 — Complete generated-helper and runtime-failure attribution

**Purpose:** Audit all target-generated calls and backend-owned failure edges
so enabling tracing cannot expose stale locations in less obvious array,
ownership, allocation, or static-lifecycle paths.

- [ ] Inventory every direct and indirect call emitted outside ordinary call
  lowering and classify it as source-attributed, non-reporting, source-body
  entry from an omitted helper, or hard-defect-only.
- [ ] Route target-generated call construction through a narrow audited helper
  that requires the appropriate trace-attribution classification, leaving only
  documented process-wrapper/ABI exceptions.
- [ ] Record the initiating MIR operation before every generated array,
  ownership, copy, destruction, finalization, anchoring, and static-lifecycle
  helper that may transitively enter a source body or reporter.
- [ ] Record the source allocation operation before every valid
  `ska_rt_alloc` call, including nested array/shared helper paths, so host
  exhaustion reports the initiating source location.
- [ ] Propagate source spans through target-private helper selection only as
  needed for attribution; do not create synthetic MIR operations or visible
  generated contexts.
- [ ] Add failure-edge replacement for inline ownership-count overflow and
  ensure overflow inside an omitted helper observes the source caller's
  already-established location.
- [ ] Keep `ska_rt_free`, known non-reporting helpers, process entry, static
  coordination, and hard-defect `ud2` paths free of unnecessary updates.
- [ ] Test source-authored standard-library calls as visible frames while the
  canonical bodyless panic intrinsic and private runtime C operations remain
  absent.
- [ ] Add a backend audit test that fails when a new call-emission site bypasses
  the classified helper without joining the documented exception set.

**Tests:** Focused allocation, arrays, ownership, copy, destruction,
finalization, static initialization/shutdown, and standard-library attribution
tests via
`cargo test --locked -p skald-compiler runtime_trace_attribution`; native host
allocation-failure and injected ownership-overflow tests; raw-call audit and
system assembler acceptance; `make check`; `make msrv-check`;
`git diff --check`.

**Exit criteria:** Every generated call site is explicitly classified, every
panic-capable generated/runtime path reports its initiating source operation,
source-authored callees push normally, no generated helper appears in output,
and hard traps remain silent.

### TRACE5 — Expose default-on tracing and migrate native observations

**Purpose:** Publish the complete behavior through the typed request/CLI
surface only after all trace locations are accurate.

- [ ] Add trace policy to `ArtifactOptions`/`CompilationRequest` with enabled
  as the programmatic and CLI default for executable and assembly output.
- [ ] Parse and document the value-free `--omit-runtime-trace` option, reject
  repetition as usage error, and pass the selected policy through request
  compilation. The in-memory singleton convenience API uses the enabled
  default; direct backend tests retain explicit enabled/omitted control.
- [ ] Update help, request, CLI, pipeline, artifact, real-binary, and public API
  tests without coupling trace policy to runtime environment variables.
- [ ] Convert representative panic goldens from prefix matching to exact
  enabled stderr for explicit panic and every compiler-known termination
  family, including multiline application, standard-library, lifecycle,
  static-initializer, allocation, and ownership chains.
- [ ] Add enabled direct/recursive/virtual/interface/lifecycle/static-library
  chain goldens and confirm the bodyless panic intrinsic is omitted.
- [ ] Add omitted variants that retain exact current single-line panic output
  and inspect assembly for absence of frame bytes, TLS references, location
  updates, metadata, and strings.
- [ ] Cover provider-relative application and standard-library paths,
  positional outside-root fallback, escaped hostile path bytes, semantic
  initializer signatures, and one-based Unicode columns end to end.
- [ ] Extend independent-process determinism coverage across different
  temporary provider roots for enabled assembly, metadata, stderr, and status.
- [ ] Promote living language, phase, backend, runtime, driver, debugging,
  testing, and status text from frozen/not-yet-implemented to the exact
  implemented boundary; keep AArch64 and exceptions deferred.

**Tests:** `make cli-test`; focused backend/driver tests; focused goldens with
`make golden-filter GOLDEN_FILTER='runtime/**'` plus affected operator,
optional, object, primitive, standard-I/O, standard-library, shared-ownership,
and static-field filters; targeted full-determinism runs with
`scripts/golden.sh --determinism full`; `make docs-check`; `make check`;
`make msrv-check`; `git diff --check`.

**Exit criteria:** Normal compilation emits complete traces by default,
`--omit-runtime-trace` removes the entire generated feature, every frozen
stderr example and frame-visibility rule is observed end to end, and living
documentation describes implementation rather than planned direction.

### TRACE6 — Measure overhead, harden determinism, and close the rollout

**Purpose:** Validate the performance premise and repository-wide stability,
then close the roadmap only with measured and reproducible evidence.

- [ ] Add or document a narrow reproducible benchmark procedure comparing
  enabled and omitted builds for tiny call-heavy recursion, a pure tight loop,
  allocation-heavy execution, and representative golden workloads.
- [ ] Record code-size, instruction-count, and repeated wall-time observations;
  confirm the six/two/two sequences and explain any material regression before
  accepting the default-on result.
- [ ] Apply only semantics-preserving target-private improvements supported by
  measurements, such as removing a replacement already established on every
  incoming path. Correctness and test expectations must not depend on such an
  optimization.
- [ ] Run complete independent-process determinism with tracing enabled and
  omitted, including alternate temporary roots and native stderr/status.
- [ ] Audit source-callable eligibility, all raw call sites, every normal
  return form, reporter families, ABI/header symbol sets, current-version
  wording, and stale frozen rollout language.
- [ ] Audit changed backend/runtime modules by responsibility; resolve
  high-priority maintainability issues and place bounded lower-priority
  findings in an indexed discoveries document instead of expanding closeout.
- [ ] Run the full repository and MSRV gates from an artifact-free snapshot,
  verify link/index/diff hygiene, mark all progress complete, archive this
  roadmap, and repair incoming links.

**Tests:** Focused benchmark procedure; exact assembly and generated-object
inspection; `make golden-determinism-test`; `make check`; `make msrv-check`;
`make docs-check`; `make docs-test`; `git diff --check`.

**Exit criteria:** The frozen minimal-overhead design is supported by recorded
measurements, enabled and omitted builds are deterministic, all implementation
and documentation audits are clean or have bounded indexed follow-ups, every
roadmap checkbox and test obligation is complete, and the roadmap is archived.

## Ordering and dependencies

TRACE0 is independent of generated code except for the mandatory ABI marker
transition and gives later native tests a real reporter. TRACE1 then creates
the sole source-aware backend boundary and deterministic metadata vocabulary.
TRACE2 consumes that vocabulary for activation lifetime without yet claiming
precise interior locations. TRACE3 covers centralized source calls and
reporter edges; TRACE4 follows with the broader raw-call/helper audit that
depends on those primitives.

The production driver deliberately retains complete omission through TRACE4,
so no intermediate compiler release claims partially correct default traces.
TRACE5 switches the already-complete implementation to the frozen default,
adds the public omission flag, migrates exact native observations, and updates
living status. TRACE6 is last because performance and determinism measurements
are meaningful only after the full enabled path and representative programs
exist. No task depends on Linux AArch64 or recoverable exceptions.

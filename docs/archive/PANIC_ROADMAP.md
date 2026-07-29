# Panic and Unrecoverable Failure Reporting Roadmap

Status: complete; P0 through P6 are implemented and validated.

This roadmap adds a source-level `std::error::panic` function and replaces
silent hard traps for every compiler-known, source-reachable unrecoverable
failure with one length-delimited runtime reporter. It preserves distinct
target-independent failure reasons for verification, keeps process
termination non-recoverable and non-unwinding, and leaves hard traps only for
invalid states that correct compiler output cannot reach.

The completed profile should compile:

```ska
from std::error import panic;

fn main() -> i64 {
    panic("configuration is missing");
}
```

and write these exact bytes to stderr before terminating unsuccessfully:

```text
panic: configuration is missing
```

## Scope and invariants

- The canonical source API is the public intrinsic function
  `std::error::panic(message: std::str::Str) -> unit`.
- `panic` is imported and qualified through the ordinary module system; it is
  not an automatic prelude name and no unqualified spelling is recognized
  below resolution.
- The canonical standard-library declaration uses a new bodyless
  `intrinsic fn` form. Only compiler-recognized intrinsic identities are
  accepted; this roadmap authorizes only `std::error::panic`.
- A valid panic call is a non-returning call statement. It satisfies definite
  return without adding a general `never` type or allowing arbitrary
  expression-position divergence.
- The message uses ordinary exact-class value-argument evaluation and is
  evaluated exactly once. If message production or copying fails, that
  earlier failure wins. Once reporting begins, no remaining Skald cleanup is
  guaranteed.
- HIR and MIR represent explicit panic separately from compiler-known static
  termination reasons. Every static reason remains distinct through MIR so
  verification and dumps retain semantic precision.
- The only public reporting ABI is
  `_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length)`.
  Generated code extracts byte address and length from the validated
  `std::str::Str` descriptor; the C runtime does not learn Skald string,
  array, class, or ownership layouts.
- The reporter writes the exact byte sequence `panic: `, the supplied bytes,
  and one LF to stderr, then terminates unsuccessfully with `_Exit`. Embedded
  zero bytes are data rather than terminators. The exact numeric process
  status is not a portable language guarantee.
- A failed reporter write still terminates immediately and does not recurse
  through the reporter.
- Every compiler-known, source-reachable unrecoverable failure calls the same
  reporter with the static message frozen below.
- A source-reachable ownership-count overflow is a reported language failure.
  Null handles, zero live counts, reference-count underflow, missing dynamic
  metadata or finalizers, double finalization, and other impossible verified
  states are compiler/runtime defects and remain hard traps.
- Generated-code hard traps continue to emit `ud2`. Runtime ABI contract
  violations use a private hard-failure path and never masquerade as a Skald
  panic.
- Normal `main() -> i64` return behavior remains unchanged.
- Panic is not catchable and does not add an exceptional control-flow edge,
  unwinding, cleanup, or failed-construction recovery.
- Shadow trace stacks, source-location rendering, stacktrace printing, and
  trace-related command-line policy are explicitly deferred.
- Recoverable or checked exceptions remain outside this roadmap.

## Frozen static messages

The authoritative closed catalog now lives in the
[language panic contract](../language/ERRORS.md#frozen-panic-design).
Subsystem documents and this implementation roadmap intentionally do not
duplicate its exact bytes.

## Progress

- [x] P0 — Freeze panic and unrecoverable-failure contracts
- [x] P1 — Establish runtime reporting and stderr observability
- [x] P2 — Introduce the canonical intrinsic declaration
- [x] P3 — Execute explicit source panic end to end
- [x] P4 — Report target-independent static failures
- [x] P5 — Separate ownership overflow from compiler-defect traps
- [x] P6 — Harden the completed panic boundary

## PR-sized implementation sequence

### P0 — Freeze panic and unrecoverable-failure contracts

**Purpose:** Make the source, flow, cleanup, message, runtime, and defect
boundaries authoritative before several compiler phases and the runtime encode
them independently.

- [x] Update the language error contract with the frozen
      `std::error::panic` API, non-returning call-statement behavior,
      left-to-right message evaluation, unsuccessful termination, and absence
      of remaining cleanup.
- [x] Specify that panic remains uncatchable and distinct from future checked
      exceptions.
- [x] Add the frozen design to the language status matrix without claiming
      compiler availability.
- [x] Specify the bodyless intrinsic declaration model, canonical
      `std::error::panic` identity and signature, standard-library dependency
      on `std::str::Str`, and rejection of unrecognized intrinsic
      declarations.
- [x] Specify the length-delimited reporter ABI, exact stderr record, embedded
      zero behavior, runtime ABI version transition, and failed-report-write
      behavior.
- [x] Update the array, optional, shared-ownership, object-cast, string, and
      backend contracts so their source-reachable failures name the common
      reporting policy without duplicating the message catalog.
- [x] Document the hard-trap boundary for invalid ownership state, impossible
      verified MIR, invalid runtime ABI inputs, and other compiler defects.
- [x] Keep trace stacks, locations, stacktraces, and exceptions explicitly
      deferred in living documentation.

**Tests:** Run `make docs-check` and `make docs-test`. Manually check that the
source example, exact messages, and runtime signature agree across the
language, compiler, and ABI documents.

**Exit criteria:** Living documentation has one authoritative panic contract,
one message catalog, one runtime reporter ABI, and an explicit hard-trap
boundary. No compiler or runtime behavior has changed.

### P1 — Establish runtime reporting and stderr observability

**Purpose:** Land and directly test the process-observable reporting
foundation before generated code depends on it.

- [x] Extend native golden expectations with an optional exact `.stderr`
      sidecar; a missing sidecar continues to require empty stderr.
- [x] Compare stderr as unmodified bytes and report escaped expected/actual
      mismatches, including embedded zero and trailing-LF differences.
- [x] Add focused golden-expectation tests for empty stderr, exact stderr,
      missing or extra bytes, non-UTF-8 data, and combined status/stdout/stderr
      mismatches.
- [x] Add
      `_Noreturn void ska_rt_panic(const uint8_t* bytes, uint64_t length)` to
      the public C header and implement allocation-free direct writes of the
      prefix, payload, and LF followed by unsuccessful `_Exit`.
- [x] Permit a null message pointer only when the length is zero; treat every
      other invalid reporter input as a private runtime contract defect rather
      than printing a user panic.
- [x] Add direct C tests that capture exact stderr and verify empty, ordinary,
      embedded-zero, and embedded-newline messages, non-return, and
      reporter-output failure.
- [x] Advance the runtime ABI and link marker from version 5 to version 6 in
      the compiler, public header, documentation, runtime tests, backend
      tests, toolchain mismatch tests, and string/runtime consistency checks.
- [x] Update golden and runtime testing guidance with the `.stderr` convention
      and reporter harness ownership.

**Tests:** Run `make golden-expectations-test`, `make runtime-test`, relevant
driver/toolchain and backend marker tests, then `make docs-check`.

**Exit criteria:** The version-6 runtime exports one tested length-delimited
non-returning reporter, exact stderr is expressible in native goldens, and no
compiler-generated path calls the reporter yet.

### P2 — Introduce the canonical intrinsic declaration

**Purpose:** Give panic an honest function-like source declaration and stable
semantic identity without widening the foreign ABI or recognizing a source
name in lower phases.

- [x] Add contextual `intrinsic fn` parsing as a bodyless top-level function
      declaration with ordinary visibility, name, parameter, result, module,
      and source-span information.
- [x] Preserve intrinsic declarations distinctly from Skald definitions and
      external declarations through AST and resolved IR; they have neither a
      Skald body nor an external link symbol.
- [x] Add `std/std/error.ska` with the exact public
      `panic(message: std::str::Str) -> unit` intrinsic declaration and an
      ordinary explicit dependency on `std::str`.
- [x] Resolve direct, selective-imported, aliased-module, and fully qualified
      uses to one stable function and intrinsic identity.
- [x] Validate the canonical module, declaration name, public visibility,
      single by-value exact `Str` parameter, `unit` result, and absence of a
      body or external linkage.
- [x] Reject intrinsic declarations at every unrecognized path and diagnose
      every malformed canonical signature before HIR.
- [x] Prevent an intrinsic declaration from serving as `main`, a method,
      initializer, lifecycle member, interface requirement, override, or
      external ABI declaration.
- [x] Retain a focused temporary diagnostic for a valid panic call until
      executable lowering lands; do not allow accepted source to reach
      incomplete HIR or MIR.
- [x] Update implemented grammar, module/interoperation documentation, phase
      dumps, and debugging guidance for the declaration shape without claiming
      executable panic support.

**Tests:** Lexer/parser recovery and dump tests; module graph tests for ordinary
`std::error` and `std::str` reachability and replacement roots; resolver tests
for every valid qualification/import form; exact diagnostics for malformed,
private, wrong-signature, body-bearing, external, duplicate, and
noncanonical intrinsic declarations; and deterministic resolved dumps.

**Exit criteria:** The compiler and canonical standard library represent one
well-formed panic intrinsic by identity, all other intrinsic declarations are
rejected, the foreign ABI is unchanged, and attempted panic execution stops
with the documented temporary diagnostic.

### P3 — Execute explicit source panic end to end

**Purpose:** Make the canonical panic call a fully executable, non-returning
language operation with a dynamic `Str` message.

- [x] Recognize the resolved intrinsic identity during call-statement type
      checking and produce a dedicated `HirPanic` statement rather than an
      ordinary call.
- [x] Require exactly one ordinary exact-class `Str` value argument, reuse
      existing value-argument production/copy rules, and reject panic in
      expression position with a focused diagnostic.
- [x] Extend `BlockFlow::Terminates` so a panic statement satisfies definite
      return and later unreachable statements do not change that summary.
- [x] Preserve exact once-only message evaluation, failure-before-reporting
      order, full message span, and the absence of cleanup after reporting in
      HIR lowering.
- [x] Add `MirTerminator::Panic` with one fully initialized exact `Str` message
      place and source span; it has no successors and is distinct from
      `MirTerminationReason`.
- [x] Verify the canonical string language item, message place type,
      initialization/liveness, no-successor shape, and absence of residual
      intrinsic call instructions.
- [x] Add deterministic HIR and MIR dump forms that identify panic without
      reproducing source spelling or target ABI details.
- [x] Lower the message descriptor through verified field identities and
      target layout, compute backing bytes plus start and length without
      copying into a NUL-terminated buffer, and call `ska_rt_panic`.
- [x] Preserve the runtime's ignorance of `Str`, shared-array headers, and
      ownership. Do not add a specialized string or array panic ABI.
- [x] Remove the temporary execution diagnostic and update language status,
      error, function-flow, string, compiler-phase, backend, standard-library,
      and runtime ABI documentation to implemented behavior.

**Tests:** Type-check tests for call-statement flow, definite return, grouping,
message type mismatch, invalid expression use, and evaluation ordering; HIR
and MIR dump tests; verifier mutation tests for wrong class, wrong place,
uninitialized message, successor corruption, and residual intrinsic calls;
backend assembly tests for descriptor extraction and one reporter call; and
native goldens for literal, empty, embedded-zero, sliced, dynamic, imported,
and qualified messages with exact stderr and unsuccessful termination.

**Exit criteria:** Valid Skald code can call the canonical panic function with
any supported `Str` value, exact bytes reach stderr, the process does not
return or clean remaining values, and no lower phase selects panic by source
spelling.

### P4 — Report target-independent static failures

**Purpose:** Route every existing MIR-level language failure through the same
reporter while retaining its exact target-independent reason.

- [x] Add one target-private panic-message enum and deterministic static byte
      pool owned by termination lowering; emit every used message once in
      stable enum order.
- [x] Centralize `MirTerminator::Panic` and `MirTerminator::Terminate`
      instruction selection in a cohesive termination module rather than
      retaining reason-specific traps in array, optional, and type-operation
      selectors.
- [x] Map every `MirTerminationReason` exhaustively to the frozen static
      catalog and emit the same `ska_rt_panic` ABI call used by explicit panic.
- [x] Keep object-cast, optional-access, guard-overflow, pinned-mutation,
      array-allocation, array-index, slice-bound, and slice-length reasons
      distinct in MIR, verification, and dumps.
- [x] Route host allocation failure for a valid `ska_rt_alloc` request through
      the reporter with the catalog's host-allocation message.
- [x] Keep zero-byte or host-unrepresentable allocator inputs on the private
      runtime contract-defect path; correct generated code must never pass
      them.
- [x] Preserve output-write failure as its existing immediate unsuccessful
      runtime boundary; it is not a compiler-known source failure and must not
      recursively invoke the panic reporter.
- [x] Replace existing failure goldens' signal-only expectations with exact
      static stderr sidecars while retaining `failure` as the process-status
      expectation.
- [x] Update focused language/compiler contracts and test guidance in the same
      change; link them to the central catalog rather than duplicating text.

**Tests:** MIR dump and verifier tests proving reasons remain exact; backend
tests proving all eight reasons load the correct static record and call the
single reporter without `ud2`; runtime allocation-failure tests distinguishing
valid allocation exhaustion from invalid ABI input; and native cast,
optional, array, string-bounds, and allocation goldens with exact stderr.

**Exit criteria:** Every target-independent source failure and valid-request
host allocation failure reports its frozen static message through
`ska_rt_panic`; their MIR reasons remain unchanged; no such path emits a hard
trap.

### P5 — Separate ownership overflow from compiler-defect traps

**Purpose:** Report legal runtime count exhaustion without turning corrupted
or impossible ownership states into user-facing panics.

- [x] Refactor shared retain lowering to expose separate overflow and invalid
      edges instead of the current combined failure label.
- [x] Route a dynamic count of `u64::MAX - 1` to
      the catalog's ownership-overflow reason, preserve verified immortal
      `u64::MAX` as a no-op, and hard-trap null handles or zero counts.
- [x] Apply the split retain contract to direct shared copies, casts, fields,
      assignments, optional-owner copies and unwrap, generated array element
      lifecycle helpers, and hidden anchors.
- [x] Split inline-array backing retain checks so maximum-count exhaustion
      reports the catalog's ownership-overflow reason while zero or otherwise
      invalid live counts hard-trap.
- [x] Keep release underflow, null release, missing dynamic metadata,
      missing finalizers, invalid optional-owner state, and repeated
      finalization on hard-trap paths.
- [x] Keep redundant backend checks after verified optional-mutation guards as
      defensive hard traps; the source-reachable guarded-mutation edge must
      already terminate through its MIR reason.
- [x] Give reporter and hard-trap labels distinct semantic names so later
      maintenance cannot merge overflow and corruption accidentally.
- [x] Audit every generated `Instruction::Trap` site and record its
      verifier-defended or corrupted-runtime-state invariant in the owning
      code and focused tests.
- [x] Update shared-ownership, array, optional, string, error, backend, and
      debugging contracts to distinguish reported exhaustion from compiler
      defects consistently.

**Tests:** Backend count-transition tests at zero, one, ordinary positive
counts, `u64::MAX - 1`, and the immortal sentinel; focused tests for every
retain consumer family; native or assembled fixtures proving count overflow
uses exact stderr and invalid retain/release/metadata states still execute
`ud2`; and existing ownership lifecycle, array, optional, and string suites.

**Exit criteria:** Every legal ownership-count overflow reaches the shared
reporter, every invalid ownership state remains a hard trap, all combined
overflow/invalid labels are gone, and each remaining generated hard trap has
one documented invariant owner.

### P6 — Harden the completed panic boundary

**Purpose:** Close cross-layer gaps, remove rollout scaffolding, and prove that
the final reporter/trap partition is complete and deterministic.

- [x] Audit lexer through backend, runtime, standard library, tests, samples,
      and living documentation for stale claims that panic is unavailable or
      language failures use an illegal-instruction boundary.
- [x] Remove temporary diagnostics, compatibility branches, duplicate panic
      emitters, duplicated static messages, task codes outside roadmap
      documents, and dead version-5 runtime assumptions.
- [x] Add one representative panic sample and concise standard-library usage
      documentation without presenting panic as an exception or ordinary FFI.
- [x] Prove deterministic assembly, static-message pooling, stderr, and
      process status across repeated compilations and executions.
- [x] Add a regression audit that every `MirTerminationReason` maps
      exhaustively to the reporter and every remaining hard trap is excluded
      from source-reachable semantic-failure tests.
- [x] Confirm complete golden stderr coverage for explicit panic, every static
      message, host allocation failure, and ownership-count overflow.
- [x] Confirm internal-defect fixtures still hard-trap without printing a
      `panic:` record.
- [x] Keep shadow trace stack, source locations, stacktrace output, exception
      syntax, unwinding, and exceptional cleanup absent from runtime symbols,
      compiler options, and source claims.
- [x] Run the complete repository gate from an artifact-free snapshot and
      inspect repository status, links, and diff hygiene before closeout.

**Tests:** Run focused suites while auditing, then `make check`,
`make msrv-check`, and `git diff --check`. Re-run `make golden-test` and
`make runtime-test` from an artifact-free snapshot when validating closeout.

**Exit criteria:** The complete source-to-process panic profile is documented,
deterministic, and tested; all compiler-known failures use the one reporter;
all remaining hard traps denote compiler/runtime defects; and no deferred
trace or exception machinery has entered the implementation.

## Ordering and dependencies

P0 settles contracts before representation work. P1 establishes the reporter,
ABI marker, and stderr observation independently of compiler lowering. P2
introduces the canonical declaration and identity while preventing premature
execution. P3 consumes that identity to implement dynamic source panic. P4
then centralizes and migrates the already explicit MIR termination reasons
without conflating them. P5 follows because current retain lowering must first
be split into legal overflow and invalid-state edges across several generated
helper families. P6 performs the final cross-layer audit and repository gates.

No task depends on a shadow trace stack or exception roadmap. A later trace
roadmap may consume the spans already retained by panic and static termination
MIR, but it must define source metadata, runtime trace state, rendering, and
command-line policy independently.

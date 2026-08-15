# Capture-Free Function Values Roadmap

Status: complete; all tasks are implemented and closure validation passed.

This roadmap implements the frozen
[capture-free function-value language contract](../language/FUNCTION_VALUES.md)
and its [compiler contract](../compiler/FUNCTION_VALUES.md). It adds canonical
recursive function types, eligible internal callable references, trivial
non-null storage, indirect calls through the complete internal ABI, closed
generic composition, verified whole-program effects, and one-word x86-64
realization without adding closures or changing runtime ABI version 9.

The confirmed decisions are preserved in the
[archived design record](FUNCTION_VALUES_DESIGN_PROPOSAL.md). Tasks
below implement those decisions without reopening them.

## Scope and invariants

- Implement recursive `fn(...) -> ...` types whose exact identity includes
  ordered value, `ref`, and `mut ref` parameter modes plus the result type.
- Permit references only to accessible internal top-level functions and
  ordinary or closed-specialization static methods.
- Preserve direct named calls unless lexical value lookup shadows the callee;
  calls through function-typed expressions are explicit indirect calls.
- Treat function values as non-null, trivially copied scalar values with no
  cleanup, allocation, owner, receiver, or environment.
- Support locals, reassignment, instance fields, explicitly initialized static
  fields, internal value parameters/results, all internal dispatch families,
  and contextual closed generic-class storage.
- Reuse ordinary argument/result ownership, alias modes, caller-owned
  aggregate destinations, evaluation order, failure, cleanup, and panic-trace
  behavior for indirect calls.
- Preserve exact `CallableId` targets separately from canonical
  `FunctionTypeId` signatures, including closed generic static methods.
- Expand static effects soundly to every address-taken exact-signature target
  and retain every referenced body and symbol.
- Keep all public compiler entry points deterministic and free of internal
  panics while work is staged. Until a source form reaches the next phase, an
  explicit phase-owned diagnostic must stop it before unsupported IR.
- Do not add optional or array function values, callback-slot aliases,
  `shared` function values, casts, equality, bound methods, closures, foreign
  pointers, or adapters during this roadmap.
- Keep `(f)(argument)` under the existing object-cast precedence rule.
- Do not change the external C ABI, runtime API, compatibility marker,
  `SKALD_RUNTIME_ABI_VERSION`, or `ska_rt_abi_v9`.

## Progress

- [x] FVI0 — Parse and intern canonical closed function types
- [x] FVI1 — Resolve eligible callable references and frozen exclusions
- [x] FVI2 — Close function types and references during generic specialization
- [x] FVI3 — Integrate trivial function values with stored and callable HIR
- [x] FVI4 — Type indirect calls through the complete internal signature
- [x] FVI5 — Lower and verify callable addresses and indirect MIR calls
- [x] FVI6 — Realize code pointers and indirect calls on x86-64
- [x] FVI7 — Make indirect calls sound for static effects, retention, and traces
- [x] FVI8 — Harden composition, publish implementation, and close the roadmap

## PR-sized implementation sequence

### FVI0 — Parse and intern canonical closed function types

**Purpose:** Establish the recursive source shape and one name-independent
signature identity before any value or call semantics depend on it.

- [x] Add source AST nodes for function types and unnamed value, read-only
      alias, and mutable-alias parameter modes with exact punctuation and
      complete spans.
- [x] Parse zero-parameter, multi-parameter, alias-mode, nested-parameter, and
      nested-result signatures in every ordinary type context while preserving
      declaration parsing and `(f)(argument)` cast precedence.
- [x] Add `FunctionTypeId`, canonical resolved signature records and tables,
      and bottom-up interning keyed by exact ordered modes, closed child types,
      and result type.
- [x] Extend recursive type rendering, syntax/resolved dumps, depth limits,
      equality, and declaration validation without treating a function type as
      a primitive, object, optional payload, array element, or shared target.
- [x] Diagnose malformed modes, missing delimiters/arrows/results, unsupported
      function-valued roles, and any temporarily gated semantic use with exact
      phase-owned spans; no accepted source may reach an unsupported lower
      phase.
- [x] Update the implemented grammar and status wording only for the syntax and
      semantic boundary that actually ships in this task.

**Tests:** Parser success/recovery and nesting-budget tests; resolved interner
identity tests for grouping, shorthand, nested signatures, modes, arrays,
optionals, classes, modules, and equal/different keys; syntax/resolved dump
determinism; compile-failure goldens for malformed and deliberately gated
uses; `cargo test --locked -p skald-compiler syntax::`; focused resolution
tests; `make check`; `make msrv-check`.

**Exit criteria:** Every closed function type has one deterministic
`FunctionTypeId` and exact source-facing rendering, malformed syntax is
structured, all existing declaration/cast parses are unchanged, and the
public pipeline stops unsupported function-value programs without panic.

### FVI1 — Resolve eligible callable references and frozen exclusions

**Purpose:** Introduce one explicit, access-checked value-formation operation
without conflating it with direct calls or receiver-bearing method selection.

- [x] Add resolved function-reference IR carrying exact `CallableId`,
      `FunctionTypeId`, and span; construct it only for internal top-level
      functions and eligible static methods.
- [x] Preserve existing direct function/static calls when their callee is a
      declaration name, and prefer lexical value bindings or fields when they
      shadow that name in value/callee position.
- [x] Reuse module qualification, selective imports, ordinary class/static
      selection, and declaring-class privacy at reference formation; keep raw
      template and closed-specialization references behind an explicit gate
      until FVI2.
- [x] Reject instance, virtual, interface, initializer, lifecycle, generated,
      external, intrinsic, raw-template, unclosed, inaccessible, and wrong-kind
      targets with distinct deterministic diagnostics.
- [x] Record address-taken exact targets in resolved program metadata for later
      liveness/effect ownership without assigning effects during formation.
- [x] Expose reference targets and address-taken sets in resolved dumps while
      retaining an explicit lower-phase gate until stored HIR/MIR support is
      complete.

**Tests:** Resolution tests for local/imported/qualified/private/static
references, entry recursion, shadowing, direct-call preservation, target/type
identity, access sites, every excluded callable family, and the temporary
generic gate; exact diagnostic goldens across modules; deterministic resolved
dumps and address-taken order; `make check`; `make msrv-check`.

**Exit criteria:** Every eligible reference resolves once to an exact target
and canonical signature, every ineligible family is rejected before HIR, and
ordinary direct calls retain their current resolved shape.

### FVI2 — Close function types and references during generic specialization

**Purpose:** Make parameter-bearing signatures and static selections ordinary
closed semantic inputs before stored HIR is enabled.

- [x] Add a structural function node to template types and recurse through
      parameters/results for dependency discovery, canonical type uses, dumps,
      and diagnostic provenance.
- [x] Substitute nested parameter, array, optional, shared, generic-class, and
      function terms child-first, then intern the complete closed function
      signature.
- [x] Retain parameter-bearing top-level and static callable selections in
      template bodies and turn eligible selections into the FVI1 resolved
      reference form with an exact specialized `MethodId` during body
      specialization.
- [x] Permit function types as contextual closed generic arguments when their
      actual stored/value requirements succeed; reject optional-payload,
      array-element, shared-target, and alias-target requirements at the
      originating template use.
- [x] Preserve separate target identities for same-signature static methods on
      different closed specializations while reusing one `FunctionTypeId`.
- [x] Extend specialization caching, recursion, failure caching, address-taken
      metadata, diagnostics, and phase dumps without introducing runtime
      generic callable metadata.

**Tests:** Template resolution and substitution tests for nested function terms,
definition/application modules, changed and equal specialized signatures,
same-type/different-target methods, contextual requirement success/failure,
recursion, cache reuse, address-taken targets, and deterministic IDs; generic
compile-failure goldens; cross-process generic phase-product determinism;
`make check`; `make msrv-check`.

**Exit criteria:** No parameter term reaches ordinary resolved function-type
metadata or callable references, every requested closed specialization has
stable exact target/type identities in the FVI1 reference form, and invalid
contextual compositions fail at their existing requirement owner.

### FVI3 — Integrate trivial function values with stored and callable HIR

**Purpose:** Make function references ordinary non-null scalar values across
all frozen storage and transport boundaries before adding indirect execution.

- [x] Add the function type and reference expression to HIR with explicit
      canonical metadata and exact eligible target identity.
- [x] Generalize implementation vocabulary from primitive-only to neutral
      scalar operations wherever existing load/store/initialize/assign/return
      machinery already applies to trivial values; keep primitive semantics
      and diagnostics distinct.
- [x] Support explicitly initialized locals, reassignment, instance fields,
      explicit static initializers, direct/synthesized field copy construction
      and assignment, value arguments, and value results with no cleanup step.
- [x] Permit function values in internal top-level, static, initializer,
      instance, virtual, and interface signatures; compare exact canonical
      types for override and conformance checks.
- [x] Reject initializer-free function-valued statics, callback-slot aliases,
      optionals, arrays, shared targets, casts, comparisons, explicit object
      copy construction, and external signatures at their ordinary owners.
- [x] Extend stored-value capability, layout-independent type, callable
      signature, static-initializer, lifecycle, and HIR dump models while the
      driver retains a structured pre-MIR gate for executable references.

**Tests:** Type-checking tests for every stored/callable position, exact
signature mismatch by arity/mode/parameter/result/nesting/identity,
initialization and synthesized lifecycle, virtual/interface conformance,
static rules, frozen exclusions, and generic holders; exact HIR dumps; focused
compile-failure goldens; `make check`; `make msrv-check`.

**Exit criteria:** HIR can represent and type every frozen storage/transport
form as a non-null trivial scalar, no function field contributes cleanup, all
signature boundaries are exact, and no source reaches unsupported MIR.

### FVI4 — Type indirect calls through the complete internal signature

**Purpose:** Select one indirect-call HIR form while reusing the ordinary
argument/result planners and exact evaluation contract.

- [x] Resolve and type calls whose arbitrary callee expression has a function
      type, including bindings, instance/static fields, returned values, and
      produced-object field chains.
- [x] Add explicit indirect-call HIR containing the checked callee expression,
      canonical signature, ordinary checked arguments, exact result, and
      complete span; it has no receiver carrier.
- [x] Evaluate the callee semantically once before explicit left-to-right
      arguments, and preserve direct/static call forms for declaration-name
      callees.
- [x] Reuse ordinary value/alias argument checking, object and array
      copy/adoption, optional and shared-owner transfer, caller-owned aggregate
      destinations, function-valued results, and result securing.
- [x] Diagnose non-callable expressions, exact arity/type/mode failures, and
      excluded grouped-call ambiguity without inferring a target or receiver
      from argument count.
- [x] Extend control-effect analysis and HIR dumps for effectful callee
      expressions, argument failures, nested indirect calls, and results while
      retaining the explicit pre-MIR execution gate.

**Tests:** Type/HIR tests for all callee source forms, every internal argument
and result family, alias access, mixed valid/invalid arguments, callee-first
and left-to-right effects, nested/function-valued results, direct-call
preservation, and non-callable diagnostics; focused HIR determinism and
compile-failure goldens; `make check`; `make msrv-check`.

**Exit criteria:** Every frozen indirect call is completely typed through the
same ownership and result plans as an equivalent direct call, with one
callee-before-arguments semantic shape and no receiver inference.

### FVI5 — Lower and verify callable addresses and indirect MIR calls

**Purpose:** Establish the target-independent executable trust boundary before
the backend accepts function values.

- [x] Add explicit MIR function-type tables, `MirType::Function`, callable-
      address rvalues, and receiverless indirect `MirCallTarget` metadata.
- [x] Lower function references through ordinary scalar storage and stabilize
      each indirect callee in a `ValueId` before lowering any argument.
- [x] Reuse existing call argument/result, aggregate destination, owner
      transfer, alias, cleanup, conditional-path, loop-epoch, and abrupt-
      termination lowering rather than creating callback-specific variants.
- [x] Verify type-table density and recursive references; target eligibility
      and exact signatures; callee definition/type/dataflow; receiver absence;
      complete arguments/results; and non-null callable-address provenance.
- [x] Add malicious MIR mutations for unknown or missing targets, mismatched
      type IDs, arbitrary scalar/pointer construction, wrong callee values,
      implicit receivers, corrupt ownership carriers, lost result securing,
      and call-order violations.
- [x] Make preliminary/final MIR dumps, static initializer MIR, public phase
      APIs, and backend legality gates deterministic for the new model.

**Tests:** Focused MIR lowering/dump tests for reference storage and indirect
calls across every value family; verifier mutation matrices; conditional,
loop, failure, and reverse-cleanup cases; public phase-product determinism;
backend must still reject the not-yet-enabled feature structurally rather than
panic; `make check`; `make msrv-check`.

**Exit criteria:** Function-value programs lower into explicit verified MIR,
every invalid trusted input is a structured verification error, callee order
and result lifetimes are mechanically visible, and backend gating is the only
remaining execution boundary.

### FVI6 — Realize code pointers and indirect calls on x86-64

**Purpose:** Execute the verified target-independent model through the complete
existing internal ABI without runtime support.

- [x] Give function values checked eight-byte size/alignment and the SysV
      integer ABI class in locals, fields, statics, parameters, results,
      temporaries, and aggregate layouts.
- [x] Materialize exact callable and specialized static-method symbol addresses
      with the existing position-independent address instruction and store/load
      them through neutral scalar machinery.
- [x] Extend call planning with a receiverless indirect target that reuses
      hidden aggregate results, integer/SSE register classes, stack overflow
      arguments, aliases, owners, optionals, and scalar result normalization.
- [x] Preserve the stabilized callee through argument preparation and tracing,
      load it into a designated scratch register, and emit deterministic
      register-indirect `call` assembly.
- [x] Support explicitly initialized function-valued static slots and preserve
      distinct symbols for same-signature closed generic targets.
- [x] Remove the staged backend/driver gate only when end-to-end verified
      reference storage and indirect execution are supported; assert unchanged
      runtime symbols and ABI marker.

**Tests:** Layout/ABI unit tests; deterministic assembly for symbol addresses
and register-indirect calls; native primitive, mixed integer/floating,
register/stack-pressure, alias, inline object/array, optional/shared-owner,
aggregate result, function-result, field/static, and generic-static calls;
assembler acceptance; runtime-call inventory and ABI-marker assertions;
focused full-determinism goldens; `make check`; `make msrv-check`.

**Exit criteria:** Every verified indirect call executes natively with the
ordinary internal ABI, address-taken targets have exact collision-free
symbols, no runtime helper or ABI change exists, and public source support no
longer depends on a staging gate.

### FVI7 — Make indirect calls sound for static effects, retention, and traces

**Purpose:** Close whole-program correctness obligations that direct call edges
previously supplied implicitly.

- [x] Build deterministic address-taken target sets keyed by exact
      `FunctionTypeId` and expand each indirect call to every matching eligible
      target during static-effect extraction.
- [x] Add evidenced indirect-call edge kinds and reuse transitive static read,
      write, initialization, shutdown, cycle, and planning diagnostics.
- [x] Retain and emit every address-taken callable body and symbol even when no
      direct call reaches it; reject references to missing or non-emittable
      definitions at the appropriate trust boundary.
- [x] Preserve independent static effects for same-signature methods on
      different closed generic specializations.
- [x] Attribute panic traces to the indirect call site while retaining the
      exact selected target's ordinary runtime activation frame and semantic
      callable name.
- [x] Dump candidate sets, effect edges, retention decisions, and trace
      locations in stable identity order; prove function reference formation
      alone has no static effect.

**Tests:** Static-effect extraction/solve/plan/certificate tests; hidden
dependencies reachable only through indirect targets; self/cycle diagnostics;
same-signature generic targets; uncalled address-taken symbol retention;
runtime trace direct/static/generic and panic cases; deterministic MIR/effect/
assembly products across processes; `make check`; `make msrv-check`.

**Exit criteria:** No indirect call can hide a possible static dependency or
lose its target body, trace attribution is exact and deterministic, and taking
or copying an address remains effect-free.

### FVI8 — Harden composition, publish implementation, and close the roadmap

**Purpose:** Exercise the complete frozen surface together, remove rollout
gates and stale prose, and leave a durable implemented contract.

- [x] Add native conformance spanning top-level/imported/private/static/generic
      references; locals, reassignment, fields, statics, parameters/results;
      virtual/interface transport; returned and chained callees; all supported
      ownership/ABI families; recursion; and same-signature distinct targets.
- [x] Freeze callee-before-argument, exactly-once, failure suppression, result
      securing, conditional and loop paths, reverse cleanup, ABI pressure,
      panic traces, static effects, and symbol retention with deterministic
      source-to-native goldens.
- [x] Complete negative coverage for every frozen exclusion and exact mismatch
      family; remove temporary unsupported diagnostics and staging gates.
- [x] Publish only implemented behavior across grammar, function values,
      functions/control flow, types/values, generics, modules/interoperation,
      compiler phases/IR, backend, runtime ABI, debugging, testing, and status
      documentation.
- [x] Audit touched modules by responsibility; make small clarity fixes in
      scope and place material follow-ups in a separately indexed function-
      value discoveries record.
- [x] Confirm no closure environment, optional/array callback container,
      alias-slot facility, foreign adapter, runtime operation, ABI version, or
      compatibility-marker change entered the implementation.
- [x] Run focused full-determinism suites, cross-process phase determinism,
      `make check`, `make msrv-check`, documentation validation, and diff
      hygiene; then archive this completed roadmap and update both indexes.

**Tests:** Complete function-value success/rejection golden matrices; parser,
resolution, specialization, type/HIR, MIR/verifier, static-effect, backend,
trace, driver, robustness, and public-API suites; repeated full deterministic
function-value goldens; `make check`; `make msrv-check`.

**Exit criteria:** Capture-free function values are an implemented,
documented, deterministic source-to-native feature across the complete frozen
storage, generic, ownership, effect, and internal ABI surface; every exclusion
remains enforced; no staging gate remains; and the roadmap is archived with no
unindexed actionable discovery.

## Ordering and dependencies

FVI0 establishes syntax and canonical closed identity before any consumer can
compare signatures. FVI1 freezes ordinary target eligibility and the resolved
reference form before FVI2 closes generic terms and specialized selections
into that form; ordinary resolved/HIR phases never see parameters. FVI3
integrates the trivial value category before FVI4 asks calls to transport every
existing ownership family. FVI5 establishes verified executable semantics
before FVI6 adds target realization. FVI7 follows executable calls because its
candidate expansion, retention, and trace obligations need stable MIR and
symbols. FVI8 removes staging gates only after all correctness owners compose.

The roadmap depends on the implemented module system, static methods and
fields, generic specialization, internal aggregate/owner ABI, verified MIR,
static lifecycle planning, runtime traces, and x86-64 backend. It has no
dependency on future closures, callback containers, alias values, foreign
adapters, or a runtime ABI revision.

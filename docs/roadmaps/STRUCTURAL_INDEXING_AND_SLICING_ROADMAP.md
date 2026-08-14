# Structural Indexing and Slicing Implementation Roadmap

Status: **in progress**. IS0 through IS5 are complete; IS6 is next.

This roadmap implements the frozen
[language](../language/INDEXING_AND_SLICING.md) and
[compiler](../compiler/INDEXING_AND_SLICING.md) contracts. The confirmed
[design record](../archive/STRUCTURAL_INDEXING_AND_SLICING_DESIGN_PROPOSAL.md)
is historical context; this roadmap owns implementation order and acceptance.

## Scope and invariants

The implementation will make existing bracket read and assignment syntax
select ordinary `index_get`, `index_set`, `slice_get`, and `slice_set` methods
on eligible class and interface receivers. Built-in arrays retain precedence
and their complete intrinsic pipeline. Structural operations normalize to
ordinary calls before HIR, preserve explicit shared dereference and ordinary
access/dispatch/ownership rules, evaluate every present expression once in
source order, and add no MIR instruction, backend operation, or runtime ABI.

The implementation depends only on the implemented array, optional-value,
generic-class, receiver, ownership, virtual-dispatch, and interface-dispatch
baselines. No other active roadmap blocks IS0.

## Progress

- [x] IS0 — Neutral bracket syntax representation
- [x] IS1 — Structural index selection and normalization
- [x] IS2 — Structural slice selection and normalization
- [x] IS3 — Receiver and dynamic-dispatch integration
- [x] IS4 — Ownership, lifecycle, and evaluation hardening
- [x] IS5 — Read-only `Str` adoption
- [ ] IS6 — Complete `Vec<T>` adoption
- [ ] IS7 — Diagnostics, documentation, and release closure

## IS0 — Neutral bracket syntax representation

Purpose: remove the parser's array-only vocabulary without changing accepted
syntax or current array semantics.

Checklist:

- [x] rename source-only array-projection AST types and visitors to neutral
  bracket or subscript names;
- [x] update parser construction, assignment-target handling, recovery, source
  spans, dumps, generic-template traversal, and test helpers;
- [x] preserve ordinary versus `->` projection and all index/slice bound shapes;
- [x] keep resolved array identities and every downstream array representation
  unchanged; and
- [x] update compiler documentation in the same change.

Tests:

- [x] parser and AST dump coverage for reads, assignments, all four slice-bound
  shapes, postfix chains, grouping, and shared arrows;
- [x] malformed bracket/colon recovery and span assertions; and
- [x] existing array compile and native goldens unchanged apart from intentional
  source-AST vocabulary.

Exit: source syntax has neutral names everywhere, while every valid and
invalid program has the same semantic result as before IS0.

## IS1 — Structural index selection and normalization

Purpose: implement both right-side and left-side structural indexing through
one resolver-owned protocol selector.

Checklist:

- [x] centralize the four frozen protocol names in one resolver concern;
- [x] classify array, class, interface, shared-arrow, and unsupported receivers,
  retaining intrinsic arrays before structural lookup;
- [x] select inherited accessible `index_get` and `index_set` declarations using
  ordinary hierarchy and privacy services;
- [x] validate getter/setter access, arity, parameter modes, and exact `unit`
  setter result;
- [x] support arbitrary signature-selected key types and independent getter result
  and setter replacement types;
- [x] normalize reads to ordinary calls and writes to unit-call statements without
  requiring a getter; and
- [x] update language status and current compiler boundaries for exactly the
  capability delivered.

Tests:

- [x] direct and inherited class reads/writes, getter-only and setter-only types,
  different read/write value types, non-`i64` keys, and closed generics;
- [x] fields/static members, wrong access, arity, result, and alias modes;
- [x] mutable-receiver rejection, private selection in and out of the declaring
  class, and explicit method-call equivalence;
- [x] arrays proving unchanged precedence and resolved/HIR representation; and
- [x] deterministic resolved and HIR dumps.

Exit: eligible class indexing works on both sides of assignment as ordinary
calls, arrays remain intrinsic, and malformed protocols fail at their owning
source spans.

## IS2 — Structural slice selection and normalization

Purpose: implement read and assignment slicing with independently omitted
bounds and no hidden receiver use.

Checklist:

- [x] select and validate `slice_get` and `slice_set` through the IS1 selector;
- [x] require exact `i64?` value parameters for the two bounds, inject supplied
  `i64` values once, and create typed `none` operands for omissions;
- [x] preserve all four bound shapes in read and assignment positions;
- [x] append the replacement only for `slice_set`, require mutable receiver and
  exact `unit`, and never select `slice_get` for a write;
- [x] leave bound normalization, logical length, copy/view, overlap, and failure
  policy to method bodies; and
- [x] promote implemented slice semantics into grammar, phase, and status docs.

Tests:

- [x] all omission shapes, supplied-bound effects, getter/setter independence,
  and distinct getter result/replacement categories;
- [x] malformed bound parameter types/modes, wrong arity/access/result, missing
  members, and immutable receivers;
- [x] effectful receiver evaluated once and no synthetic `len()` call;
- [x] unchanged array slice read/write semantics and snapshot goldens; and
- [x] deterministic resolution/HIR dumps with typed omitted bounds.

Exit: eligible class slicing works on both sides of assignment, each supplied
operand evaluates once, and no structural node survives into HIR.

## IS3 — Receiver and dynamic-dispatch integration

Purpose: make all four operations compose with the existing object model
rather than forming a direct-class-only feature.

Checklist:

- [x] support requirements selected through interface-typed receivers;
- [x] preserve virtual families, overrides, direct private calls, and interface
  witness selection exactly as explicit calls do;
- [x] support read-only and mutable aliases, fields, statics, `self` paths, and
  explicit shared `->` or `*` receiver forms within their existing access;
- [x] support getter calls on produced exact-class receivers and reject setters
  on unnamed produced inline values;
- [x] preserve checked-view and optional-owner unwrap requirements; and
- [x] keep generic specialization complete before HIR.

Tests:

- [x] exact, inherited, virtual override, interface, shared-interface, private,
  and closed-generic calls for index and slice operations;
- [x] read-only receiver write rejection and produced receiver read/write split;
- [x] raw shared and optional shared rejection versus explicit crossing success;
- [x] stable/replaceable/produced shared owners with later argument effects; and
- [x] native direct, virtual, and interface dispatch goldens.

Exit: bracket selection has the same receiver eligibility, access, and dispatch
matrix as the equivalent explicit ordinary call.

## IS4 — Ownership, lifecycle, and evaluation hardening

Purpose: prove that call normalization preserves every supported argument,
result, temporary, and evaluation-order contract.

Checklist:

- [x] exercise primitive, class, array, optional, shared-owner, and specialized
  generic getter results and setter replacements;
- [x] preserve receiver-before-operands and replacement-last order with one
  evaluation per present expression;
- [x] verify target-directed copies, produced-value adoption, owner retention or
  transfer, alias anchors, selected-path temporaries, and reverse cleanup;
- [x] extend preliminary/final MIR checks only when an existing ordinary-call
  invariant lacks coverage, without adding structural IR; and
- [x] confirm reachability, static-effect inference, backend call lowering, and
  runtime ABI remain ordinary-call paths.

Tests:

- [x] effect counters and failure ordering for receiver, key, bounds, replacement,
  call body, and cleanup;
- [x] owning result consumption and discard, assignment source transfer/copy,
  self-aliasing arguments, and checked receiver anchors;
- [x] representative verifier rejection tests for malformed ordinary-call MIR;
- [x] source-to-native coverage for every supported value family; and
- [x] focused compiler and full-artifact determinism checks.

Exit: the supported ownership matrix executes through existing verified call
machinery with no new lower-IR, backend, or runtime operation.

## IS5 — Read-only `Str` adoption

Purpose: expose natural bracket reads while retaining `Str` immutability and
ordinary-library status.

Checklist:

- [x] add public `index_get(index: i64) -> u8` over the existing checked byte
  implementation;
- [x] add `slice_get(start: i64?, end: i64?) -> Str`, mapping omission to logical
  beginning/end before the existing normalized constant-time slice path;
- [x] keep the backing private and add neither setter;
- [x] avoid compiler special cases or new string language-item requirements; and
- [x] update the string contract and examples to implemented status.

Tests:

- [x] literal, named, produced, copied, shared, and interface-mediated read forms;
- [x] every slice omission shape, empty/full slices, normalized negative bounds,
  and current panic behavior;
- [x] independent descriptor and backing-lifetime behavior; and
- [x] compile failures for index/slice assignment.

Exit: `Str` bracket reads match its ordinary byte/slice methods and writes are
rejected because no setter exists. Complete; IS6 is next.

## IS6 — Complete `Vec<T>` adoption

Purpose: make the standard generic vector demonstrate all four protocols with
documented vector-specific slice semantics.

Checklist:

- add protocol entry points while retaining `get`/`set` as compatibility
  wrappers if useful and sharing one normalization/bounds implementation;
- implement logical-length `slice_get` returning an independent `Vec<T>`;
- implement equal-length `slice_set` without changing destination length;
- secure or copy the complete replacement before the first increasing-order
  destination write, including self-aliasing and overlap; and
- update vector API, capability, cost, failure, and cleanup documentation.

Tests:

- reads/writes and slice reads/writes for representative primitive, class,
  shared-owner, optional, and nested eligible element types;
- logical length versus capacity, empty/full/omitted/negative bounds, bad
  bounds, and mismatched replacement length;
- result independence, self replacement, overlap snapshot behavior, prompt
  replacement cleanup, growth after slicing, and destruction; and
- closed-specialization resolved/HIR/native and determinism goldens.

Exit: `Vec<T>` implements the frozen four-method profile without exposing
capacity as logical data or weakening element lifecycle.

## IS7 — Diagnostics, documentation, and release closure

Purpose: close the feature with coherent diagnostics, observability, current
documentation, and repository-wide validation.

Checklist:

- audit diagnostics for unsupported receivers, missing/malformed/inaccessible
  protocols, mutability, explicit dereference, and ordinary call failures;
- ensure AST, resolved, HIR, and MIR dumps expose the intended phase boundary
  with deterministic canonical identities;
- update grammar, language/compiler overviews, arrays, strings, vectors,
  classes, ownership, phases, testing guidance, and status to current behavior;
- confirm runtime ABI documentation records no change and no structural symbol;
- remove stale planned wording, mark this roadmap complete, and archive it.

Tests and gates:

- run focused syntax, resolver, type-check, call, receiver, interface, generic,
  array, optional, shared-ownership, `Str`, and `Vec<T>` suites;
- run `make docs-check` and `make check`;
- run `make msrv-check` because the roadmap changes Rust compiler code;
- run `make golden-determinism-test`; and
- inspect the final diff for stale names, contracts, roadmap state, and ABI
  claims.

Exit: all frozen behavior is implemented and documented, repository-wide
quality gates pass, the runtime ABI is unchanged, and the completed roadmap is
archived.

## Ordering and discoveries

IS0 is intentionally semantic-neutral. IS1 establishes the selector and
indexing path reused by IS2. IS3 composes selection with dispatch and receiver
forms before IS4 closes ownership risk. IS5 and IS6 may proceed independently
after IS4; IS7 requires both.

Implementation discoveries that would change a frozen language decision do
not silently expand an active task. Record them separately, update this
roadmap only after the contract is deliberately amended, and keep the active
task bounded to its stated exit criteria.

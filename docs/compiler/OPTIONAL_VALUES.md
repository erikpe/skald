# Optional-Values Compiler Contract

Status: frozen compiler design with primitive and exact inline-class optional
owning locals, fields, and internal callable boundaries implemented through
HIR, MIR verification, x86-64 lowering, and native execution.
The [language optional-value contract](../language/OPTIONAL_VALUES.md) defines
source meaning, the [status matrix](../language/STATUS.md) defines compiler
availability, and the [implemented grammar](../language/GRAMMAR.md) remains
authoritative for source currently accepted by the compiler.

This document freezes phase ownership, target-independent invariants, the
initial x86-64 representation and internal ABI direction, failure lowering,
and test obligations for explicit optionals. AST and resolved IR contain all
supported source-shaped forms and flat identities. Primitive and exact-class
optionals continue through explicit typed HIR and MIR places, calls, and
lifecycle operations; the verifier proves their storage, aggregate-boundary,
conditional ownership, and failure-edge invariants, and the x86-64 backend
executes them. Checked class payload views, optional shared owners, and
optional-container aliases still stop at `TYP035`.

## Phase ownership

Each phase owns one stable responsibility:

| Phase | Optional-value responsibility |
|---|---|
| Lexing and parsing | Preserve `?`, postfix `!`, reserved `none`, contextual `some`, `shared?`, presence tests, precedence, trivia, and recovery spans. |
| Resolution | Resolve inline optional payloads and optional shared targets without deciding layout or ownership operations. |
| Type checking and HIR | Decide optional compatibility, injection, overload selection, lifecycle operation, payload value/place category, checked-view extent, access, and shared-anchor requirement. |
| MIR lowering | Make optional storage state, conditional lifecycle, failure edges, presence guards, shared ownership, temporaries, and cleanup executable and explicit. |
| MIR verification | Prove storage, payload, owner, guard, anchor, transition, failure, and CFG invariants independently of source shape. |
| Backend | Realize only verified layouts, state transitions, calls, traps, and ownership operations for the selected target. |
| C runtime | Remain unaware of optional tags, guards, payload layout, unwrap, and conditional ownership. |

No backend may infer optional source semantics from a type use or insert
unverified conditional cleanup. No earlier target-independent phase may encode
x86-64 byte offsets, tag values, registers, or runtime symbols.

## Type identities

The initial resolved, HIR, and MIR models require two non-recursive optional
families:

- inline optional over a primitive or exact class payload; and
- optional shared owner over a class, interface, or `Obj` shared target.

Resolution retains compact copyable target enums rather than making every
existing type recursively heap allocated merely to represent one optional
layer. Inline optional payloads and optional shared targets are distinct flat
families; repeated phase-local boolean flags are not used.

The type model must preserve these distinctions:

```text
T           ordinary inline payload
T?          inline optional payload
shared T    ordinary non-null shared owner
shared? T   optional shared owner
```

`shared T?`, `shared? T?`, nested optionality, optional aliases, and invalid
payload families reach one focused diagnostic boundary and never become an
executable HIR type.

## Source-shaped IR

AST and resolved IR preserve:

- the `?` span on an inline type;
- the `shared` and owner-optional `?` spans separately;
- `none` as a distinct source expression;
- `is some` and `is none` as presence tests distinct from object type tests;
- postfix unwrap as a distinct postfix operation; and
- enough operator and operand spans for deterministic recovery and diagnostics.

Canonical dumps use `T?` and `shared? T`, independent of source trivia.
Reserved box spellings remain visible to diagnostics but do not acquire an
executable type identity.

## Typed HIR

HIR records semantic operations selected by type checking rather than asking
MIR or the backend to rediscover them from expected types. By responsibility,
it must distinguish:

- empty optional construction with its exact destination type;
- present injection from a primitive, inline object source, or shared owner;
- optional copy construction, assignment, and destruction;
- non-failing presence tests;
- checked primitive value extraction;
- checked inline payload places with exact root, access, target, projections,
  and immediate consumer;
- optional shared-owner copy, adopt/move, release, and secured unwrap;
- direct fresh construction into a new inline payload destination;
- stable, copied, or adopted shared anchors covering an optional container;
  and
- source evaluation and full-expression temporary order.

Flow-sensitive knowledge may classify a check as statically successful for
optimization, but HIR source legality never depends on that classification.
Every unwrap remains a checked semantic operation.

The implemented inline subset uses explicit HIR nodes for absent and present
initialization, exact optional copy and assignment, field places,
arguments/results, produced calls, presence tests, conditional class lifecycle,
and checked primitive unwrap. Guarded class payload places and shared owners
remain later subsets of this same model.

## MIR optional storage model

Every source-visible optional local, field, parameter, result, and owning
temporary lowers to initialized optional storage. The wrapper lifetime and the
conditional payload lifetime are distinct:

```text
wrapper storage: uninitialized -> initialized -> ended
payload state:                 absent <-> present
```

The wrapper must be initialized exactly once before use. Dynamic presence may
differ across CFG predecessors, but every predecessor must agree that the same
compatible wrapper storage is initialized and owned. Presence remains runtime
state rather than a verifier guess.

MIR represents, by semantic operation rather than necessarily by these Rust
names:

- initialize absent;
- initialize present from a primitive value;
- construct or copy-construct an inline payload;
- copy or assign an optional;
- test presence;
- branch on checked unwrap success/failure;
- begin and end a checked inline payload view;
- conditionally destroy an inline payload;
- copy, adopt/move, and conditionally release an optional shared owner;
- secure an unwrapped ordinary shared owner;
- reject a presence-changing transition while guarded; and
- terminate for absent access, guard overflow, or guarded mutation.

The backend executes those operations. It does not synthesize them from loads,
stores, or cleanup lists.

Primitive local MIR currently represents absent/present initialization,
optional copy and assignment, presence tests, and unwrap as a success/failure
terminator. Its failure successor is an explicit empty block ending in
`OptionalAccessFailure`. Definite-initialization verification intersects
initialized wrapper storage at CFG joins and deliberately does not treat
dynamic presence as a static fact.

Exact-class optional MIR additionally records conditional initialization,
publication after destination-directed construction, copy construction,
assignment, and cleanup. Optional payload projections designate only reserved
exact-class bytes and are valid construction/lifecycle destinations; ordinary
source access to those bytes remains unavailable until checked views land.

## Lifecycle state machine

Optional initialization publishes presence only after the complete payload
initialization succeeds. Optional assignment evaluates and secures its source
before changing the destination.

The executable transition matrix is:

| Destination | Source | Required MIR effect |
|---|---|---|
| absent | absent | Preserve absence |
| absent | present | Initialize or copy-construct payload, then publish presence |
| present | absent | Verify unguarded, destroy/release payload, then publish absence |
| present | present | Verify legal access and perform payload assignment |

For a direct non-optional source, the source is present. A named shared source
is secured by owner copy; a produced owner is adopted or moved according to
its existing provenance. Direct or allocation-alias self-assignment remains
safe because incoming ownership is secured before the old optional owner is
conditionally released.

Fresh ungrouped exact construction into newly initialized `T?` uses the
optional payload as the final construction destination. Other object sources
retain the ordinary copy/materialization semantics selected by HIR.

Construction, payload assignment, and destruction may execute user code. MIR
must keep the optional in an internal transition-safe state or guard its live
payload so re-entrant source operations cannot end or overwrite the payload
being consumed. Exact state encoding is backend private; no transient state is
a source-visible third optional value.

## Checked payload views

An inline unwrap used as a place lowers to a checked payload carrier plus an
active presence guard. The guard begins at source evaluation and ends after
the complete immediate consumer.

For a call:

1. evaluate the receiver or argument source;
2. establish any required shared owner anchor for the optional container;
3. check presence and begin the payload guard;
4. evaluate later arguments left to right;
5. execute the call while the container anchor and payload guard remain live;
6. secure the result;
7. end checked views; and
8. release anchors and other temporaries in reverse completion order.

A stable inline container needs no owner anchor. An optional field reached
through stable shared owner storage uses that owner. A replaceable shared field
copies an owner into `SharedAnchor`; a produced owner is adopted into its
existing full-expression owner. The shared anchor covers storage lifetime,
while the presence guard covers conditional payload lifetime.

Primitive extraction copies before later effects and needs no continuing
guard. Optional shared-owner extraction secures one ordinary non-null owner and
likewise needs no continuing optional guard.

## Presence-guard state

The target-independent model tracks:

- whether optional storage is initialized;
- whether a checked payload view is active;
- the view's compatible optional root and payload type;
- guard nesting/count balance;
- any owner anchor covering the container storage; and
- the normal point where the view ends.

The source-visible optional states remain absent and present. The backend state
may additionally represent one or more active guards and an internal lifecycle
transition.

Beginning a checked view:

1. verifies that the optional is present;
2. verifies that the guard count can be increased;
3. records one additional active view; and
4. yields the payload place only on success.

Clearing, replacing, or destroying the container requires no active view.
Failure occurs before any tag, payload, owner, or lifecycle change. Ordinary
mutation within the still-present payload is unaffected.

No separate source-visible lock is introduced. On x86-64 the state word may
encode presence and the active guard count together. Exact numeric encodings
and any internal transition marker remain backend private.

## MIR verification

The verifier rejects a program unless all of the following hold:

- every optional storage entry has one legal optional type and is initialized
  before any test, unwrap, copy, assignment, or cleanup;
- absent storage has no live payload or strong-owner account;
- present inline storage has one compatible live payload;
- present optional shared storage accounts for exactly one ordinary strong
  owner;
- absent payload bytes are never addressed through a `T` place;
- initialization publishes presence only after complete payload construction;
- optional assignment secures its source before changing its destination;
- each normal optional lifetime end conditionally cleans exactly one present
  payload or owner;
- every checked payload carrier has one compatible live optional root;
- begin/end guard operations are balanced and ordered around the complete
  immediate consumer;
- a checked view ends before its container anchor or owning temporary ends;
- clearing, replacement, destruction, and lifecycle transitions prove no
  active guard or branch to the matching failure;
- normal exits contain no active optional guard;
- optional failure edges terminate with their exact non-returning reason;
- zero optional-owner representation never reaches an ordinary shared
  retain, release, dereference, metadata, cast, or finalization operation;
- CFG joins agree on initialized storage, live owning temporaries, active
  guards, anchors, and lifecycle transitions; and
- optional values are absent from external signatures.

Malformed public or test-mutated MIR must fail verification before target
layout or instruction selection.

## Target-independent failures

MIR uses distinct termination reasons for:

- absent optional access;
- presence-guard count overflow; and
- clearing, replacing, or destroying a guarded optional.

Every reason is non-returning. A success and failure edge must differ, and the
failure block may not continue into ordinary cleanup or payload use.

Future recoverable exceptions must add explicit exceptional edges that end
active guards and clean initialized optional temporaries. They cannot reinterpret
the current non-returning reasons as catchable exceptions.

## Initial x86-64 inline layout

Inline primitive and class optionals use one eight-byte, eight-aligned state
word followed by the payload at its required alignment:

```text
state offset   = 0
payload offset = align_up(8, align_of(T))
alignment      = max(8, align_of(T))
size           = align_up(payload offset + size_of(T), alignment)
```

The current target supports payload alignment no greater than eight, but the
formula remains explicit so a later wider-alignment type cannot be silently
mislaid out.

The payload reserves the ordinary complete representation of `T`. Absence
does not initialize those bytes as a `T`; loads, projections, copying,
assignment, and cleanup branch on verified state before addressing them as a
payload.

For the implemented primitive-value subset, zero means absent and one means
present. The reserved eight-byte word leaves the guarded and transition states
backend-private for later inline-class stages; their exact future encoding may
change without changing source behavior or the C runtime ABI.

Fields use the same layout recursively. An optional exact class therefore
contributes the complete payload layout to containment-cycle detection even
when its runtime state may be absent.

## Initial x86-64 optional shared-owner layout

`shared? T` is one eight-byte, eight-aligned integer-class word:

- zero represents absence of the optional owner; and
- non-zero is the canonical header handle of an ordinary `shared T`.

Zero is never published as or converted into a plain `shared T`. Every owner
copy, release, dereference, metadata load, cast, and finalization branches on
presence or consumes a previously secured non-zero owner.

The shared allocation header, payload offset, dynamic descriptor, finalizer,
strong count, allocator/deallocator calls, and plain `shared T` representation
remain unchanged.

## Initial internal calling convention

Inline `T?` values use one uniform owning-aggregate convention:

- the caller prepares complete optional argument storage and transfers it to
  the callee through one pointer-sized internal argument;
- the callee owns the initialized parameter storage and conditionally destroys
  its payload during normal parameter cleanup; and
- an inline optional result uses caller-provided destination storage, completed
  by the callee before normal return and owned by the caller afterward.

This convention applies to primitive and exact-class inline optionals. It
prioritizes one lifecycle-correct boundary over special scalar cases; a future
optimization may specialize an ABI only with coordinated caller/callee and
documentation changes.

`shared? T` follows the existing direct shared-owner convention: one
integer-class argument word and one direct result word in `rax`. A present
owner is copied or transferred under the existing shared call rules; absence
transfers zero without retain or release.

These are compiler-private conventions. External declarations reject all
optional parameters and results.

## Backend lowering

The x86-64 backend:

- computes optional layout through one shared layout owner rather than
  duplicating offsets across instruction lowering;
- executes verified state transitions and conditional lifecycle operations;
- preserves state and payload homes across calls according to the frame model;
- implements checked unwrap and guarded mutation as explicit branches;
- lowers every optional failure termination to the existing non-returning
  `ud2` boundary;
- conditionally emits existing copy, assignment, destruction, retain, release,
  anchor, cast, and finalization sequences; and
- never calls a C helper for optional tags, guards, or unwrap.

Backend legality rejects an optional operation whose MIR type, layout, source,
destination, failure edge, guard, or ownership effect is inconsistent.

## C runtime ABI

The optional profile adds no C runtime symbol and requires no runtime ABI
version bump. Optional state, guard counts, conditional ownership, checked
access, and trap lowering remain compiler-owned.

The current allocator and deallocator continue to receive only the same valid
nonzero sizes and exact non-null allocation bases required by the
[runtime ABI](RUNTIME_ABI.md). The optional shared-owner zero niche is never
passed to either function or to generated plain-owner operations.

If implementation evidence requires a runtime helper, the compiler, public
header, runtime implementation, version marker, C harnesses, link-mismatch
tests, and documentation must change together. An undocumented helper is not
permitted.

## Dumps and diagnostics

Syntax, resolved, HIR, and MIR dumps use semantic wording and deterministic
identity order. They distinguish inline optional storage, optional shared
owners, checked payload views, presence guards, optional temporaries, anchors,
and failure reasons without exposing source-inaccessible machine offsets.

Diagnostics should identify:

- invalid payload and target families;
- reserved shared-box spellings;
- missing expected type for `none`;
- implicit unwrap and direct member/dereference attempts;
- incompatible injection, assignment, argument, and return types;
- initializer-overload ambiguity;
- external optional signatures;
- recursive inline containment through `T?`; and
- invalid checked-view access or lifetime.

Exact diagnostic codes and wording remain compiler behavior rather than
portable language semantics.

## Test obligations

Focused implementation tests must cover:

- tokens, parsing, recovery, nesting limits, spans, and deterministic dumps;
- every primitive and exact-class inline optional layout;
- optional shared class/interface/`Obj` owners and the zero niche;
- absence, presence, injection, copy, assignment, cleanup, and self-assignment;
- fields, parameters, results, temporaries, overloads, overrides, and
  interfaces;
- direct payload construction and side-effect-visible lifecycle order;
- checked value extraction and every checked object-place consumer;
- nested/overlapping guards, later-argument and re-entrant invalidation,
  shared-root anchors, and normal guard cleanup;
- each non-returning optional failure;
- mutated-MIR verifier rejection;
- register, stack, hidden-destination, recursion, and interface-dispatch ABI
  pressure;
- source-to-native positive, compile-failure, and runtime-failure goldens;
- malformed-source deterministic robustness; and
- documentation, MSRV, and complete repository gates.

Tests must continue to prove that plain `T` and `shared T` never acquire absent
or zero states and that no optional feature changes the C runtime ABI
accidentally.

## Exclusions

This compiler contract does not design generalized shared boxes, nested
optionals, optional arrays, optional function values, first-class references,
optional casts, equality or operator lifting, chaining/coalescing/propagation,
recoverable failures, concurrency or atomic guards, external optional ABI, or
dynamic-type-preserving cloning.

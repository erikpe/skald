# Copy-Capability Materialization Design Proposal

Status: frozen accepted design. Accepted on 2026-09-14; implementation is
tracked by the
[Copy-Capability Materialization Roadmap](COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md).

This proposal addresses
[A17](CODEBASE_CLEANUP_AUDIT.md#a17--reduce-copy-capability-fixed-point-reconstruction).
It defines how type checking should construct concrete HIR copy and array
lifecycle plans from the phase-neutral lifecycle availability authority added
by A08. The change is internal: source semantics, diagnostics, resolved IR,
typed HIR, MIR, dumps, and native behavior remain unchanged.

The current type checker computes complete HIR class-copy plans, clones them
into provisional capability snapshots, and rebuilds a complete HIR array table
on every convergence round. Resolution separately uses a compact
phase-neutral solver for the same availability question. The selected design
makes that neutral result authoritative for availability and turns type
checking into a one-pass materializer of phase-owned HIR plans.

## Intended outcome

- Compute class and array copy-construction and assignment availability once
  through `ResolvedLifecycleCapabilities` for each type-check request.
- Retain the neutral service's deterministic failure paths for capability
  diagnostics.
- Construct each concrete HIR class capability once from resolved declarations
  and immutable neutral facts.
- Construct the final `HirArrayTypeTable` once in canonical array-identity
  order.
- Move that final table into `HirProgram` after type checking rather than
  cloning it for publication.
- Remove provisional `CopyCapabilities` snapshots, per-round capability-set
  clones, provisional HIR array tables, and the second availability solver in
  type checking.
- Keep concrete selected operations, ordered base and field plans, final-field
  permissions, array element operations, and HIR types owned by type checking.
- Preserve the existing public HIR, MIR, diagnostic, dump, and driver surfaces.
- Establish a measured stop condition so the migration is retained only when
  it reduces reconstruction and semantic duplication without weakening phase
  ownership.

## Current architecture and evidence

The phase-neutral
[`ResolvedLifecycleCapabilities`](../../crates/skald-compiler/src/type_capabilities/lifecycle.rs)
computes four availability families over a `ResolvedCapabilityView`:

- class copy construction;
- class copy assignment;
- array copy construction; and
- array copy assignment.

It also retains deterministic outer-to-inner class failure paths. Generic
class and interface requirement validation request these facts lazily through
`GenericCapabilityQuery`. This service depends only on resolved declarations,
canonical type tables, and stable identities. It contains no HIR, MIR, pass,
backend, or diagnostic-rendering type.

The type checker independently computes the same availability while building
concrete HIR plans in
[`CopyCapabilities::compute`](../../crates/skald-compiler/src/typeck/capabilities.rs).
Its current sequence is:

1. recursively construct provisional class copy-constructor plans;
2. construct a provisional assignment set for array analysis;
3. clone both sets into a temporary `CopyCapabilities` value;
4. rebuild every HIR array lifecycle plan;
5. invalidate class constructors whose array fields are unavailable and
   repeat until stable;
6. reconstruct assignments using the stable constructors;
7. repeat cloning, array-plan construction, and invalidation for assignments;
8. rebuild the complete HIR array table once more for publication.

The loop exists because a synthesized class operation can depend on an array
field whose element eventually depends on a class operation. The current
implementation initially records array fields optimistically and later
invalidates the enclosing class. Correct convergence matters, but constructing
phase-owned plans is unnecessary while only availability is being discovered.

The A08 neutral solver already separates those concerns. It converges over
compact booleans and failure paths, then exposes immutable queries. The
[capability tests](../../crates/skald-compiler/src/typeck/tests/capabilities.rs)
compare every neutral class and array availability result with the selected
HIR plans, including recursive optional and array dependencies. This parity
test establishes equivalence today, but both sides still implement the
availability semantics independently.

Concrete HIR construction contains information that the neutral service must
not absorb:

- selected user or synthesized copy-operation identities;
- ordered base-copy plans;
- ordered `HirSynthesizedFieldCopy` operations;
- assignment permission for direct final fields;
- HIR primitive and shared-target types;
- typed array default, copy, assignment, and destruction operations; and
- canonical HIR table representation.

These are executable typed plans for lower phases. Availability and its causal
path are phase-neutral semantic facts; the plan that realizes an available
operation belongs to HIR.

## Scope and invariants

- `ResolvedLifecycleCapabilities` remains owned by the neutral
  `type_capabilities` module and consumes only `ResolvedCapabilityView`.
- Type checking computes a fresh neutral result from the final selected
  `ResolvedProgram`. The first implementation does not reuse resolver-local
  query caches across the publication boundary.
- Resolution does not depend on HIR or type checking, and `ResolvedProgram`
  does not gain cached HIR plans or request-mutable analysis state.
- Neutral facts decide whether a class or array operation is available and
  retain the failure path when it is not.
- Type checking decides how an available operation becomes a concrete HIR
  plan. It cannot override neutral unavailability.
- Class construction and assignment remain distinct. Assignment of an
  optional class payload continues to require both its constructor and
  assignment operations.
- Synthesized plans preserve direct-base-first and declaration-ordered field
  behavior. Direct final fields remain writable only through synthesized copy
  assignment.
- Canonical nested array identities continue to precede containing array
  identities. Materialization must not replace identity order with traversal
  or hash-map order.
- Default construction and destruction remain independent from copy and
  assignment availability.
- Recursive or unavailable class, optional, and array dependencies retain the
  same outer-to-inner diagnostic path.
- Source diagnostics, HIR and MIR dumps, generated assembly, and native
  behavior remain byte-for-byte stable for deterministic inputs.
- Failure to materialize a plan that neutral facts marked available is a
  compiler consistency defect. It must not be converted into a source
  diagnostic or silently downgraded to unavailable.

## Non-goals

- No change to source-visible copy, assignment, optional, array, inheritance,
  or final-field semantics.
- No phase-neutral HIR operation enum or generic lifecycle-plan IR.
- No HIR type, selected operation, expression, statement, or declaration in
  `type_capabilities`.
- No lifecycle cache stored in `ResolvedProgram`, global state, thread-local
  state, or process-wide memoization.
- No reuse of capability results across different candidate publications or
  compilation requests.
- No dependency-driven worklist, incremental invalidation graph, arbitrary
  iteration cap, or general semantic-query framework.
- No change to canonical identity allocation or table ordering.
- No removal of MIR verification or change to later lifecycle verification.
- No requirement for a measurable wall-time improvement when the affected
  workloads are too small; the structural allocation and ownership results
  remain independently measurable.

## Selected design

### Neutral lifecycle facts are the availability authority

At the start of copy-capability construction, type checking computes one
`ResolvedLifecycleCapabilities` from the selected `ResolvedProgram`. That
value is immutable for the remainder of HIR construction.

The neutral result continues to expose focused queries rather than its vectors:

```text
constructor(class) -> available
assignment(class) -> available
array_copy(array) -> available
array_assignment(array) -> available
constructor_failure(class) -> optional path
assignment_failure(class) -> optional path
```

Names may be clarified during implementation, but callers cannot mutate facts
or index raw storage. The service remains capable of running against resolver
candidate views, as required by generic publication validation.

Type checking computes a new result rather than attempting to recover a cache
from resolution. Resolver validation can operate on candidate products that
are later rejected or replaced, while type checking receives only the final
published `ResolvedProgram`. Crossing that boundary would require cache
identity and invalidation rules unrelated to A17.

### HIR materialization consumes facts without rediscovering them

Replace the recursive availability-and-plan `CapabilitySet::compute` with a
HIR materializer. It receives the resolved program and the immutable neutral
facts and creates constructor and assignment plans.

For each class operation:

- a neutral unavailable fact produces `HirCopyCapability::Unavailable`;
- a resolved user operation produces `HirUserCopy`, including its selected
  base operation;
- a resolved synthesized operation produces `HirSynthesizedCopy`, including
  its base, declaration-ordered fields, and assignment-only final fields; and
- every required nested operation is looked up from already completed or
  recursively materialized HIR plans, with an explicit consistency assertion
  against the neutral fact.

The materializer may use a small `Unvisited`, `Visiting`, `Complete` state table
to preserve identity-indexed recursive construction. An available synthesized
cycle indicates disagreement with the neutral authority and is a compiler
defect. The state table is construction control, not another availability
solver: it never changes a neutral result or computes failure paths.

Constructors are materialized before assignments. Assignment construction may
borrow completed constructor plans for optional class payloads and base or
field selection. No capability set is cloned to create a provisional view.

### Arrays are materialized once after class plans

After both class plan tables exist, type checking walks the canonical resolved
array table exactly once. Existing element-plan helpers continue to own the
conversion from a resolved element kind to `HirArrayDefaultElement`,
`HirArrayCopyElement`, `HirArrayAssignElement`, and
`HirArrayDestroyElement`.

The neutral array facts decide whether copy and assignment operations exist.
When an operation is available, the helper selects its concrete class,
optional, shared, or nested-array HIR payload from completed class plans and
earlier array entries. When it is unavailable, the corresponding HIR lifecycle
slot is `None` without constructing and discarding a provisional operation.

Default and destruction plans are still produced for every valid array type,
regardless of its copy or assignment capability. The resulting
`HirArrayTypeTable` is the only HIR array table built by the type-check request.

### CopyCapabilities retains the products needed by type checking

`CopyCapabilities` remains the type-checker's private facade. Its durable
shape is conceptually:

```text
CopyCapabilities
├── neutral lifecycle facts
├── HIR constructor plans
├── HIR assignment plans
└── final HIR array type table
```

Ordinary expression, statement, class, optional, and array checking continue
to ask it for concrete HIR operations. Failure diagnostics delegate to the
neutral result's paths. Keeping the neutral value avoids cloning failure paths
into a second owner and makes the authority visible at the point of use.

The final `HirProgram` continues to own its array table. After all consumers
finish checking, the private facade is consumed and moves that table into the
program. This removes the current final table clone without exposing mutation
or changing the HIR surface.

### Consistency is checked at the construction boundary

The current parity test compares two independently computed answers. After the
migration, tests must instead verify the stronger construction invariant:

```text
neutral class operation available
    iff the corresponding HIR capability has a selected operation

neutral array operation available
    iff the corresponding HIR lifecycle slot is populated
```

Materialization also checks that every available synthesized dependency has a
concrete selected operation and that every unavailable class uses the neutral
failure path. These checks are exhaustive over identity-indexed tables.

The old solver must be deleted after equivalence is established. It must not
remain as a dormant production fallback or permanent test oracle. Focused
fixtures with explicit expected facts and HIR plans preserve independent
regression value without maintaining two implementations of the algorithm.

## Measurement and stop conditions

### Baseline measurement

Before changing ownership, add a temporary test-only computation report around
the current `CopyCapabilities::compute`. It records deterministic counts for:

- constructor convergence rounds;
- assignment convergence rounds;
- cloned constructor and assignment capability records;
- provisional HIR array-table builds and entries;
- final HIR array-table builds and entries;
- compact neutral availability rounds and array-entry evaluations;
- final array-table publication clones and cloned entries; and
- class constructor and assignment plan constructions.

Exercise at least:

- no classes or arrays;
- direct user and synthesized class operations;
- inheritance and nested inline class fields;
- arrays of classes and nested arrays;
- optional class and optional array payloads;
- a class-to-array-to-class invalidation chain requiring convergence;
- recursive unavailable dependencies; and
- separate constructor and assignment failure paths.

Record the baseline and post-change structural counts in a focused development
measurement document. Temporary probes should be removed when durable tests
can prove the final construction counts directly. Overall compiler time and
peak RSS may be sampled through the existing cleanup measurement procedure,
but noisy wall-time results do not override the ownership and allocation
evidence.

### Go condition

Retain the migration only when all of these hold:

1. Type checking uses one immutable neutral lifecycle result as its sole
   availability authority.
2. No `CapabilitySet` or equivalent HIR plan table is cloned to form a
   provisional capability view.
3. No provisional `HirArrayTypeTable` is built during convergence.
4. Each concrete class operation is materialized at most once per operation
   family, and the final HIR array table is built once and moved into
   `HirProgram`.
5. The neutral module remains free of HIR, MIR, diagnostics, passes, and
   backend types.
6. Resolution and type checking do not acquire a reverse dependency or a
   shared mutable cache.
7. Exact capability availability, failure paths, selected operations, plan
   order, dumps, diagnostics, assembly, and native results remain unchanged.
8. The resulting implementation is smaller or materially easier to reason
   about than the two current solvers and provisional reconstruction loops.

### No-go condition

Revert the materialization migration and retain only useful measurement or
regression coverage if any of these occur:

- neutral facts cannot construct the existing HIR plans without duplicating a
  second availability decision;
- HIR types or operation-selection policy must move into the neutral module;
- reuse requires storing candidate-specific caches in `ResolvedProgram` or
  threading them through publication;
- available plan construction needs a general dependency graph or comparable
  machinery whose complexity exceeds the removed loops;
- source diagnostics or deterministic phase products change; or
- representative repeated measurements show a material compiler-time or
  memory regression that cannot be removed without weakening ownership.

If the full design reaches no-go, a later narrowly scoped change may replace
owned provisional `CopyCapabilities` snapshots with borrowed views. That
fallback must be assessed separately because it removes cloning but leaves the
duplicate availability solver and repeated HIR array construction intact.

## Validation strategy

### Neutral facts and HIR construction

- Cover every resolved user, synthesized, and unavailable class operation.
- Cover base classes, direct fields, nested fields, shared fields, primitive
  optionals, class optionals, nested optionals, arrays, and optional arrays.
- Check constructor and assignment independently, including assignment's
  constructor prerequisite for optional class payloads.
- Check direct final-field order and assignment-only permissions.
- Check recursive containment and class-array invalidation without an
  iteration cap.
- Assert exact neutral failure paths and exact HIR selected-operation payloads.
- Assert exhaustive table identity order and one final array materialization.

### Observable behavior

- Preserve existing copy-capability and generic-requirement diagnostics,
  including primary cause and outer-to-inner field/base paths.
- Preserve resolved, HIR, MIR, and phase-determinism dumps.
- Preserve copy construction, assignment, cleanup, optional, array,
  inheritance, shared-owner, and final-field native behavior.
- Preserve malformed or partial resolved-product behavior supported by the
  current public type-checking boundary.
- Keep the existing phase-dependency guard green and add a focused neutral
  module dependency check only if the implementation introduces imports that
  the current guard cannot constrain.

### Repository gates

- Focused `type_capabilities` and type-check capability tests.
- Generic class and interface requirement tests.
- Array, optional, inheritance, copy, and final-field type-check and MIR tests.
- Relevant process-determinism and golden suites.
- `make check`.
- `make msrv-check` because Rust module interfaces and supported syntax change.
- `git diff --check` and documentation validation.

## Risks and controls

| Risk | Control |
| --- | --- |
| Neutral and HIR availability silently diverge | Make neutral facts authoritative, assert every available dependency during materialization, and exhaustively compare final populated slots with neutral tables. |
| Failure diagnostics change when type checking stops owning failure paths | Retain neutral outer-to-inner paths in `CopyCapabilities` and freeze exact nested class, base, optional, and array diagnostics. |
| One-pass construction changes base or field order | Traverse direct base first and fields in declaration order; compare exact HIR dumps and synthesized field vectors. |
| Assignment accidentally stops requiring construction for optional payload replacement | Materialize constructors first and test constructor-available/assignment-unavailable and the inverse malformed combinations. |
| Resolver candidate facts leak into final HIR | Compute fresh from the selected `ResolvedProgram`; do not cross the publication boundary with a cache. |
| Removing the independent solver loses regression detection | Replace implementation parity with explicit expected-fact fixtures and exhaustive materialization consistency checks. |
| Refactoring hides work behind a generic query framework | Keep a concrete lifecycle result and a private HIR materializer with identity-indexed tables. |
| Performance measurements are dominated by unrelated compiler work | Use deterministic construction counters as primary evidence and wall time/RSS only as supporting observations. |

## Alternatives considered

### Keep both solvers and optimize only cloning

A borrowed provisional view could remove `CapabilitySet` clones with a small
diff. It would still rebuild HIR array plans during convergence and preserve
two implementations of lifecycle availability. It is a valid fallback after a
no-go outcome, but it does not use the ownership boundary established by A08.

### Put concrete copy plans in the neutral service

This would give resolution a dependency on HIR operation identities and plan
types, recreating the phase violation removed by A08. Rejected.

### Store lifecycle facts in ResolvedProgram

This could avoid recomputation when resolution already requested lifecycle
facts, but candidate publication, rejected materializations, and partial error
products would need explicit cache identity and invalidation. The expected
saving does not justify changing the resolved product. Rejected for A17.

### Let type checking remain the authority and adapt resolution to it

Resolution would again depend on a later phase or on an HIR-shaped service.
Rejected.

### Introduce a general dependency graph or incremental worklist

The compact neutral fixed point already terminates without an arbitrary cap,
and no measurement shows its boolean reconstruction is material. A general
graph would increase code and invalidation complexity before it has a second
consumer. Rejected unless later measurements justify a separate proposal.

### Keep the old solver as a permanent oracle

This would preserve the maintenance burden and allow the implementations to
drift. Exact fixtures and boundary consistency checks provide independent
regression evidence without shipping dormant production logic. Rejected.

## Roadmap boundary

The accepted
[implementation roadmap](COPY_CAPABILITY_MATERIALIZATION_ROADMAP.md) divides
delivery into four reviewable stages:

1. Capture deterministic reconstruction counts and freeze exact capability,
   failure-path, and HIR-plan behavior on adversarial dependency shapes.
2. Expose the minimal immutable neutral queries required by type checking and
   establish one request-local authority without changing HIR construction.
3. Replace the type-check availability solver and provisional array rebuilds
   with one-pass concrete HIR materialization, then apply the go/no-go check.
4. Remove temporary instrumentation and obsolete solver code, update living
   architecture and testing documentation, and run full repository acceptance.

Each stage must leave tests green. The migration stage must be reverted if it
fails the no-go criteria; measurement and independently useful regression
coverage may remain. The roadmap must not absorb A15 optional-place
representation work, A23 effect/alias analysis, or broader optimizer changes.

## Decision summary

| Question | Selected decision |
| --- | --- |
| What owns lifecycle availability? | One immutable `ResolvedLifecycleCapabilities` result in the phase-neutral `type_capabilities` service. |
| What owns concrete copy and array plans? | Type checking and typed HIR. |
| Does type checking reuse resolver-local caches? | No; it computes facts from the final selected resolved program. |
| How is convergence handled? | The existing compact neutral fixed point; HIR construction does not participate in convergence. |
| How are class plans built? | Once per operation family through an identity-indexed materializer guided by neutral facts. |
| How are array plans built? | Once in canonical array identity order after class plans are complete. |
| Where do failure paths live? | In the neutral result, borrowed by type-check diagnostics. |
| What happens to the old HIR availability solver? | Delete it after equivalence and the go condition are proven. |
| What evidence decides retention? | Deterministic reconstruction counts, exact semantic equivalence, phase-boundary checks, and repository gates. |
| What happens on no-go? | Revert the authority/materialization migration; retain useful measurements and regressions, then separately assess a borrow-only clone reduction. |

## Promotion decision

Review accepted all of the following on 2026-09-14:

- neutral availability is authoritative while concrete HIR plans remain
  type-check owned;
- resolver-local results are not cached through `ResolvedProgram`;
- the roadmap begins with structural measurement and exact regression
  fixtures;
- the materializer uses neutral facts without recomputing availability;
- the old solver is removed rather than retained as a fallback;
- go/no-go instructions explicitly cover ownership, construction counts,
  observable equivalence, and complexity; and
- larger optional representation, dependency-graph, and optimization work
  remains outside A17.

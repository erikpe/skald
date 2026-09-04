# Local Final-MIR Simplification Design Proposal

Status: frozen implemented design. LFS1 through LFS14 were confirmed together
and delivered by the completed
[implementation roadmap](LOCAL_FINAL_MIR_SIMPLIFICATION_ROADMAP.md) on
2026-09-02. The living
[compiler phase](../compiler/PHASES_AND_IR.md#local-final-mir-simplification),
[driver](../compiler/DRIVER_AND_ARTIFACTS.md#local-final-mir-simplification-selection),
and
[reporting](../compiler/REPORTING.md#local-final-mir-simplification-observation)
documentation own the current contract. Follow-up findings belong in the
[discoveries record](LOCAL_FINAL_MIR_SIMPLIFICATION_DISCOVERIES.md).

This proposal defines Skald's first broader layer of local target-independent
final-MIR optimization. It adds conservative primitive constant folding,
primitive algebraic simplification with atomic value forwarding, and narrowly
bounded CFG cleanup to the implemented selectable pipeline. The existing
dead-pure-definition elimination pass cleans up newly dead scalar work, and
whole-world reachability remains the final target-independent retention pass.

The purpose is larger than the individual rewrites. The work should establish
reusable owners for exact primitive evaluation, value-use eligibility, and
proof-aware local CFG reachability without introducing a speculative analysis
manager or a second optimization IR.

The proposal follows the implemented foundations recorded by the
[optimization architecture discoveries](../roadmaps/OPTIMIZATION_ARCHITECTURE_DISCOVERIES.md):
the static-lifecycle certificate, dense callable-local MIR rewriting,
selectable final-MIR pipeline, target-independent whole-world reachability,
and reachability-gated static lifecycle are already in place.

## Intended outcome

The design should provide:

- separately selectable primitive constant-folding, algebraic-simplification,
  and conservative CFG-cleanup passes;
- exact compiler-owned primitive evaluation that cannot inherit Rust debug
  overflow, host floating-point, or backend instruction behavior;
- linear block-local scalar facts keyed by existing `ValueId` identities;
- algebraic identities whose applicability and failure behavior are explicit;
- atomic same-block value forwarding without adding a synthetic copy rvalue;
- an exhaustive use-site query that prevents forwarding through proof-bearing
  or checked-protocol roles;
- a reusable callable-local executable reachability query with explicit
  entry, lifecycle, and proof roots;
- constant ordinary-branch folding and safe unreachable-block deletion;
- deterministic dense recompaction through the existing MIR rewrite facade;
- an explicit default schedule that deliberately repeats cleanup passes;
- pass-local measurements and verified inspection products; and
- optimization-off parity plus native semantic-equivalence coverage.

This is a local simplification layer. It should make common MIR smaller and
create evidence for later alias, effect, proof-normalization, SSA, target-LIR,
and register-allocation decisions without pre-committing Skald to them.

## Current architecture and evidence

### Final MIR is already a safe transformation boundary

Final MIR is target-independent, executable in shape, and accepted by a
backend only through `VerifiedFinalMirProgram`. The selectable pipeline:

1. centrally verifies raw final MIR;
2. resolves one deterministic profile into pass occurrences;
3. gives each occurrence read-only verified input and a narrow rewrite
   capability;
4. atomically commits dense MIR changes through sparse callable edits; and
5. immediately re-verifies every changed product before another pass or
   backend can observe it.

The registry, stable pass names, `none` and `default` profiles, repeated
occurrences, exclusions, `--list-mir-passes`, measurements, and verified
inspection checkpoints are implemented. The current production default runs
`dead-pure-definition-elimination` followed by
`whole-world-reachability`.

### MIR scalar values make local analysis deliberately simple

`ValueId` denotes a dense callable-local transient value with one declaration
and exactly one definition. Verification requires every use to occur after
its definition in the same basic block. State crossing control-flow edges is
spilled to `StorageId`; MIR is not SSA and has no phi or block-argument form.

The relevant pure scalar rvalues are explicit constants, unary operations,
binary operations, primitive comparisons, and primitive casts. Checked
integer division, remainder, shifts, and floating-to-integer conversion have
separate proof-bearing CFG protocols. Loads, path-condition reads, callable
addresses, type tests, optional presence, and array length are semantically
different producers and must not be treated as primitive constants merely
because their result type is scalar.

This representation permits deterministic instruction-order analysis inside
one block without dominance, SSA, or a persistent dataflow framework. It does
not support unrestricted propagation through mutable storage.

### MIR has no copy rvalue

There is no `MirRvalueKind::Use` or other transient-to-transient copy node.
Lowering directly names an existing `ValueId` when no operation is required.
The rewrite facade already supports type-checked value-use replacement, after
which a pass may explicitly delete the obsolete definition and declaration.

Consequently, a standalone copy-propagation pass would initially have no
input. Adding a copy node solely so algebraic simplification could produce it
would enlarge verification and backend contracts, and disabling the consumer
pass would leave artificial copies in final MIR. This proposal instead makes
value forwarding one atomic operation used by algebraic simplification. A
future real copy producer may justify a separate registered pass.

### Primitive semantics are already selected before the backend

MIR operations encode type, width, signedness, comparison flavor, and checked
failure structure. The living backend contract requires wrapping `i64`,
`u64`, and `u8` arithmetic, canonical `u8` and `bool` results, signed floor
division, divisor-sign remainder, checked shift counts, and IEEE binary64
behavior. The optimizer must consume this selected semantic vocabulary; it
must not reconstruct meaning from source syntax or copy host-language
arithmetic casually.

### CFG and proof metadata remain coupled

Ordinary boolean control flow uses `MirTerminator::Branch`. Checked casts,
integer divisors, shifts, optionals, arrays, and ownership protocols use
dedicated terminators. `MirPathCondition` and `MirLogicalExpression` records
name exact blocks, storage, and values. Static-initializer publication also
names initialization and shutdown blocks outside the ordinary body entry.

The dense rewrite facade can redirect edges, replace terminators, delete
blocks and records, and compact all local identities. It intentionally does
not decide which proof relationships a transformation may discard. Central
verification rejects invalid output, but eligibility must remain an explicit
pass rule rather than trial-and-error against the verifier.

### Whole-world and single-threaded assumptions

Every Skald compilation is permanently closed-world, and every resulting
program is single threaded. Those guarantees make final whole-program
reachability and static activation especially valuable, and they remove
concurrent memory-order concerns from local rewrites.

They do not make a mutable load constant, prove two aliases independent, or
make destruction unobservable. This proposal therefore neither relies on nor
weakens alias, mutation, ownership, allocation, cleanup, or failure behavior.

## Comparison with Niflheim

Niflheim's backend optimizer has separate dead-definition, trivial-copy,
constant-folding, algebraic-simplification, and CFG-simplification passes. Its
default order deliberately repeats copy and dead-definition cleanup around
the productive passes. Its CFG cleanup folds constant and same-target
branches, forwards empty blocks, and removes unreachable blocks to a fixed
point.

The useful precedents are:

- keep transformations small and independently selectable;
- centralize exact pass order rather than relying on module discovery;
- repeat cleanup where another pass exposes work;
- separate constant evaluation from algebraic identities; and
- validate the suite using production transformations, not synthetic hooks.

Skald should not copy Niflheim's exact pass boundaries. Niflheim has an
explicit copy form and different backend IR invariants. Skald has block-local
transient values, a final-MIR seal, exact proof metadata, static publication
attachments, and deterministic sparse-to-dense commit. Empty-block forwarding
would alter predecessor topology that Skald metadata currently records
exactly. This proposal adopts Niflheim's modular scheduling pattern while
using boundaries native to Skald MIR.

## Constraints and non-goals

- The language contract does not change.
- Evaluation order and exactly-once evaluation remain unchanged.
- Checked failure, panic reason, and failure span remain unchanged.
- Allocation, ownership, cleanup, destruction, and static startup/shutdown
  behavior remain unchanged.
- Mutable storage and shared-pointee access retain their current permissive
  aliasing behavior.
- `none` remains the exact verification-only reference profile.
- Every pass remains target independent and consumes only verified final MIR.
- No source diagnostic is created, removed, or reordered by optimization.
- No load forwarding, store forwarding, dead-store elimination, storage copy
  propagation, alias analysis, escape analysis, or effect inference is added.
- No call folding, inlining, devirtualization, specialization, global value
  numbering, common-subexpression elimination, loop optimization, SSA, block
  parameters, or optimization IR is added.
- No checked integer-division, remainder, shift, checked-cast, optional, array,
  shared-ownership, or allocation protocol is rewritten.
- No floating arithmetic, floating comparison, or numeric conversion involving
  `f64` is folded initially. Raw-bit-preserving literals remain supported MIR,
  but host-independent IEEE evaluation is deferred.
- No proof record is deleted or normalized.
- No empty-block forwarding, jump threading, block merging, critical-edge
  splitting, or general CFG canonicalization is added.
- No storage declaration is removed by CFG cleanup.
- No pass-preservation declarations or cross-pass analysis cache is added.
- No arbitrary user-selected pass order, numeric `-O` level, dynamic pass
  plugin, or target-specific registry is introduced.

## Design principles

### Exact transformations before broad transformations

Every initial rule has a closed input family and a direct semantic argument.
Ambiguous host behavior, proof-heavy operations, storage epochs, and ownership
operations are conservative barriers.

### Preserve identities when replacement is representable

Constant folding replaces an rvalue while retaining its assignment, result
`ValueId`, type, and source span. Algebraic rules that produce a literal do the
same. An identity rule may forward uses and delete the result only when the
use-site contract proves that the deletion cannot invalidate metadata.

### Facts are pass-local and cheap to rebuild

The useful analyses are linear in one callable and small compared with MIR
verification. Each occurrence builds immutable facts from its verified input
and discards them after the pass. A changed occurrence invalidates all facts
through the existing seal boundary.

### Verification is the trust boundary, not the selection algorithm

The central verifier independently checks every changed product. Passes still
must select candidates through explicit conservative rules and return a
structured failure if a supposedly valid atomic rewrite cannot commit.

### Proof-bearing structure is retained until normalization has an owner

Blocks or values named by non-executable proof and lifecycle metadata are
protected. Conservatively retaining them is preferable to silently teaching
the first CFG pass a partial proof-rewrite protocol.

## Frozen pass suite

### Stable registrations

The production registry should add these entries after the existing internal
identities 0 and 1:

| Internal identity | Stable name | Responsibility |
|---|---|---|
| 2 | `primitive-constant-folding` | Replace eligible primitive operations whose operands are block-local constants |
| 3 | `primitive-algebraic-simplification` | Apply exact primitive identities, using guarded atomic value forwarding where appropriate |
| 4 | `conservative-cfg-cleanup` | Fold eligible ordinary branches and remove unprotected unreachable blocks |

Numeric identities remain compiler-private. Stable names are the selection,
listing, reporting, and exclusion contract. Each pass is independently
selectable through the already implemented profile/exclusion machinery.

There is no umbrella executable pass and no registered copy-propagation pass.
“Local final-MIR simplification” names this coordinated design and default
schedule, not a hidden nested pipeline.

### Primitive constant folding

The pass scans each block in instruction order and maintains a map from an
already defined `ValueId` to an exact primitive constant. Because MIR values
cannot cross blocks, the map is discarded at every block boundary. The pass
does not inspect or model `StorageId` contents.

An eligible assignment retains its result declaration, `ValueId`, `MirType`,
instruction position, and span. Only its rvalue kind becomes the matching
constant. The new constant is immediately available to later instructions in
the same scan, so a single occurrence reaches a local forward fixed point for
straight-line constant chains.

The initial evaluator supports:

- `ConstantI64`, `ConstantU64`, `ConstantU8`, and `ConstantBool` facts;
- wrapping integer addition, subtraction, and multiplication;
- integer bitwise and, or, xor, and complement;
- wrapping `i64` negation;
- boolean logical not;
- integer comparisons with the encoded signedness and width;
- boolean equality and inequality;
- identity casts;
- integer-to-integer bit conversions with exact width canonicalization;
- integer-to-boolean zero testing; and
- boolean-to-integer canonical conversion.

The evaluator does not initially fold:

- any operation involving `ConstantF64Bits`, except preserving an existing
  literal unchanged;
- integer division or remainder;
- shifts;
- checked floating-to-integer conversion;
- integer-to-floating conversion or other numeric conversion involving
  `f64`;
- loads, path conditions, callable addresses, type tests, optional presence,
  optional-box presence, or array length; or
- any instruction or terminator with ownership, cleanup, allocation, call,
  I/O, or failure semantics.

Division and shifts remain excluded even for apparently valid constants. Their
MIR rvalues participate in exact checked diamonds, and replacing only the
success operation would break that protocol while removing the entire diamond
belongs to a later proof-aware design.

The implementation must use explicit wrapping and width-conversion helpers.
Ordinary Rust arithmetic whose debug/release behavior differs is forbidden.
Unsupported operation/type pairs return “not foldable”; they do not panic or
silently guess semantics.

### Primitive algebraic simplification

This pass uses the same block-local constant facts but requires no attempt to
evaluate an operation when all operands are constant. It owns a small reviewed
catalog of identities. The initial catalog is:

| Family | Rules |
|---|---|
| Wrapping add | `x + 0 -> x`, `0 + x -> x` |
| Wrapping subtract | `x - 0 -> x`, `x - x -> 0` |
| Wrapping multiply | `x * 1 -> x`, `1 * x -> x`, `x * 0 -> 0`, `0 * x -> 0` |
| Integer bitwise and | `x & all_ones -> x`, `all_ones & x -> x`, `x & 0 -> 0`, `0 & x -> 0`, `x & x -> x` |
| Integer bitwise or | `x | 0 -> x`, `0 | x -> x`, `x | all_ones -> all_ones`, `all_ones | x -> all_ones`, `x | x -> x` |
| Integer bitwise xor | `x ^ 0 -> x`, `0 ^ x -> x`, `x ^ x -> 0` |
| Integer/bool comparison | `x == x -> true`, `x != x -> false` |
| Unary involution | `!!x -> x`, `~~x -> x`, wrapping `-(-x) -> x` |

“Integer” includes the exact encoded `i64`, `u64`, or `u8` width. The pass
constructs zero, one, and all-ones using that width and canonicalizes `u8`.
Comparison self-identities apply only to integer and `bool` operands, never
to `f64`.

All operand-producing instructions have already executed before the
assignment being simplified. A rewrite does not move, duplicate, or suppress
those producers. Rules such as `x * 0 -> 0` therefore retain potentially
observable producers in place. The later dead-pure pass may delete a now-dead
producer only when its independent non-failing purity classifier permits it.

Rules producing constants retain the result identity and replace its rvalue.
Rules producing an existing operand use guarded atomic value forwarding:

1. prove source and result types equal;
2. prove the source is defined earlier in the same block;
3. classify every result use as forwarding-safe;
4. replace result uses with the source through the exhaustive identity mapper;
5. delete the obsolete assignment and result declaration in the same callable
   transaction; and
6. let dense commit and central verification validate the complete product.

The pass processes a deterministic snapshot of candidates. After a structural
deletion it rebuilds position-keyed facts before selecting another candidate.
It must not retain instruction indices across a rewrite.

### Forwarding-safe use sites

A reusable use-site analysis should enumerate, not merely count, every use of
one transient value. It should be built on the existing immutable exhaustive
identity observer and return deterministic sites classified by semantic role.

Initial forwarding is allowed only for ordinary executable value operands:

- unary, binary, and primitive-comparison operands;
- non-checked primitive-cast operands;
- ordinary scalar stores;
- ordinary call target, receiver, and scalar argument operands where the
  existing mapper identifies a value use;
- ordinary return values; and
- ordinary boolean branch conditions.

Forwarding is rejected if any use belongs to:

- path-condition or logical-expression metadata;
- a divisor, shift, primitive-cast-range, checked-cast, optional, array, or
  other dedicated checked terminator;
- a proof-coupled success rvalue;
- callable or static-publication attachment metadata;
- ownership, cleanup, allocation, I/O, or lifecycle protocol state; or
- an unknown future role.

The exhaustive classifier must use closed matches so a new MIR variant cannot
silently become forwarding-safe. The general rewrite mapper remains broader;
the eligibility query is deliberately narrower.

### Conservative CFG cleanup

This pass has two initial transformations:

1. rewrite an eligible ordinary `Branch` with a block-local constant boolean
   condition into `Goto` to the selected successor; and
2. rewrite an eligible ordinary `Branch` whose two targets are identical into
   `Goto` to that target.

The replacement retains the original terminator span. Dedicated checked and
multiway terminators are never rewritten.

After branch folding, the pass computes executable block reachability. The
root set is explicit and conservative:

- the callable body entry;
- every block named by callable-level lifecycle or publication attachments,
  including static initialization and shutdown entry/exit roles; and
- every block named by `MirPathCondition`, `MirLogicalExpression`, or other
  non-executable proof metadata known to the exhaustive local-identity model.

All roots are traversed through ordinary executable successor edges. A block
not in this closure is removable only if no retained non-executable attachment
references it. Treating proof-named blocks as roots deliberately retains
otherwise unreachable proof regions until a future metadata-normalization
design can remove their records as a coherent unit.

The pass removes an eligible unreachable block together with every transient
value whose sole definition is owned by an instruction in that block. It does
not remove storage declarations, path conditions, logical records, optional
guards, or callable attachments. The existing definition/use census and
exhaustive identity traversal should supply the ownership facts; the pass
must not duplicate a partial list of value-producing instruction variants.

Branch folding is rejected when the branch block itself is named by proof or
lifecycle metadata. This keeps exact split/merge/publication shapes intact.
Other folded branches may make protected regions unreachable, but those
regions remain rooted and retained.

Empty-block forwarding is intentionally absent. Even an instruction-empty
`Goto` block may be a named predecessor, lifetime boundary, publication
endpoint, or cleanup join. Proving general predecessor redirection safe is
part of future proof-provenance normalization.

## Shared support ownership

The three passes should remain focused modules behind
`passes::pipeline::optimizations`. Reusable implementation support should be
private to the target-independent optimization layer:

- a primitive constant model and evaluator;
- a block-local scalar fact builder;
- an exhaustive value-use-site classifier;
- a proof/lifecycle block-root collector; and
- a deterministic executable block-reachability query.

These are not public language semantics and not a general analysis manager.
The MIR model remains the semantic vocabulary, the verifier remains the
correctness authority, and the rewrite facade remains the only structural
mutation owner.

The primitive evaluator should expose pure functions over MIR operation enums
and typed constants. It must not depend on a target backend, CLI policy,
reporting, filesystem state, or optimization schedule.

The CFG root collector should share exhaustive identity observation with MIR
rewriting. Static publication attachments live outside `MirBody`, so the
callable rewrite coordinator must expose their block roles through one narrow
read-only query rather than letting the pass reach into definition variants.

## Default schedule and composition

The proposed `default` profile is:

```text
dead-pure-definition-elimination
primitive-constant-folding
primitive-algebraic-simplification
primitive-constant-folding
dead-pure-definition-elimination
conservative-cfg-cleanup
dead-pure-definition-elimination
whole-world-reachability
```

The ordering has explicit reasons:

- the first dead-pure occurrence reduces work inherited from lowering;
- constant folding exposes algebraic identities;
- algebraic forwarding may expose new all-constant consumers;
- the second constant-folding occurrence consumes those opportunities;
- dead-pure removal cleans scalar producers made unused by both passes;
- CFG cleanup consumes constant branch conditions;
- the final dead-pure occurrence removes obsolete branch-condition chains;
  and
- whole-world reachability runs last because removed CFG regions may remove
  the final calls, callable-address formations, or other executable
  dependencies retaining a definition.

There is no generic pipeline fixed-point driver. Constant folding reaches a
forward fixed point within each block, dead-pure elimination already reaches
its conservative local fixed point, and the schedule expresses deliberate
cross-pass repetition. Stable-name exclusion removes every occurrence of the
named pass under the existing policy.

The `none` profile remains empty. Disabling all five registered pass names
from `default` must resolve to the same product and execution count as `none`.

## Analysis lifetime, rewriting, and verification

Every occurrence begins with a verified final-MIR seal and its current
whole-world facts. Local simplification facts borrow that sealed product and
live for one occurrence only.

An occurrence should first inspect dense MIR without consuming the seal. If no
candidate exists, it returns unchanged with processed-callable measurements
and triggers no rewrite or redundant verification. If a candidate exists, it
consumes the existing pipeline capability, rewrites through the atomic
coordinator, and returns the structured commit result. Any change invalidates
the final-MIR seal and every fact derived from that exact program, including
whole-world reachability. The immutable preliminary-MIR static activation and
baseline lifecycle authority are not recomputed. Central verification instead
rechecks the transformed realization against that authority and rebuilds the
final seal and its derived facts.

No pass constructs a seal, emits a source diagnostic, repairs invalid input,
logs, writes a dump, or catches verifier failures as “not applicable.” A
rewrite or output-verification failure is attributed to the exact pass
occurrence by the existing pipeline error boundary.

## Determinism and observation

Candidate discovery follows executable-definition order, block order,
instruction order, and value index. Hash iteration must not affect candidate
selection, measurements, or output. If sets or maps are required for
membership, output is normalized before observation.

Pass-owned measurements should include:

| Pass | Measurements |
|---|---|
| Primitive constant folding | processed/changed callables; folded unary, binary, comparison, and cast assignments |
| Primitive algebraic simplification | processed/changed callables; constant-result rewrites; forwarded uses; removed assignments and value declarations; rejected protected-use candidates |
| Conservative CFG cleanup | processed/changed callables; folded constant branches; folded same-target branches; removed blocks; removed value declarations; retained protected unreachable blocks |

The existing structural rewrite counts remain authoritative for total entity
changes. Pass counters explain why changes occurred and must not double as a
second commit accounting system.

Verified before/after/final MIR checkpoints remain the detailed inspection
mechanism. No complete MIR or CFG dump is embedded in ordinary reports. A
small focused local-facts or CFG-root dump may be added for compiler tests only
if unit assertions cannot adequately explain deterministic eligibility.

## Verification and test strategy

### Primitive evaluator tests

- every supported operation and type pair;
- `i64::MIN`, signed extrema, unsigned extrema, and `u8` canonicalization;
- wrapping overflow and wrapping negation;
- signed and unsigned comparison boundaries;
- boolean canonicalization and conversions;
- explicit rejection of division, remainder, shifts, checked conversions,
  floating operations, loads, and semantic queries;
- build-profile-independent results; and
- exhaustive `u8` operation inputs where practical.

### Constant-folding tests

- forward chains within one block;
- no fact leakage across blocks;
- unchanged result identities, types, positions, and spans;
- deterministic per-kind measurements;
- no rewrite and no extra verification when no candidate exists;
- no changes to checked-operation diamonds; and
- focused malformed-rewrite attribution.

### Algebraic and forwarding tests

- every catalog rule for each supported width;
- self-identity and unary-involution rules;
- `f64`, load, checked, proof, ownership, and unknown-role exclusions;
- same-block definition-order and exact-type requirements;
- multiple ordinary uses rewritten exhaustively;
- metadata-referenced results retained;
- operand producers preserved until dead-pure independently removes them;
- dense value recompaction after deletion; and
- deterministic recomputation after instruction positions change.

### CFG tests

- constant true and false ordinary branches;
- same-target ordinary branches;
- no rewrite of any dedicated checked terminator;
- body entry retention;
- static initialization and shutdown attachment retention;
- path-condition and logical-expression block retention;
- unreachable loops and disconnected regions;
- all value-producing instruction forms inside a removed block;
- storage declarations retained;
- source spans preserved on rewritten terminators;
- deterministic block/value recompaction; and
- unchanged central proof, lifetime, ownership, and static-lifecycle
  verification.

### Pipeline and native tests

- registry listing and lexical descriptor order;
- exact default schedule and repeated occurrence numbering;
- disabling each new pass removes all of its occurrences;
- `none` and all-disabled exact final-MIR parity;
- deterministic optimized MIR and measurements across repeated and independent
  processes;
- golden MIR changes for representative arithmetic and CFG programs;
- native stdout, stderr, exit-status, panic, runtime-trace, ownership,
  destruction, optional, shared, array, function-value, and static-lifecycle
  equivalence; and
- whole-world pruning made newly effective by CFG removal.

The implementation roadmap should require focused Rust tests while each owner
lands, then the root quality gate, golden and native suites, documentation-link
checks, independent-process determinism, and the supported MSRV gate when Rust
code changes.

## Decision register

| Decision | Question | Frozen decision | Status |
|---|---|---|---|
| [LFS1](#lfs1--add-three-independent-production-passes) | What is registered? | Constant folding, algebraic simplification, and conservative CFG cleanup as separate passes | **Confirmed** |
| [LFS2](#lfs2--centralize-exact-target-independent-primitive-evaluation) | Who owns constant semantics? | One optimizer-private evaluator over typed MIR constants and operations | **Confirmed** |
| [LFS3](#lfs3--keep-scalar-facts-block-local-and-pass-local) | How broad is analysis? | Linear instruction-order facts, reset at blocks and discarded after each occurrence | **Confirmed** |
| [LFS4](#lfs4--start-with-a-closed-integer-and-boolean-folding-set) | Which operations fold? | Total wrapping integer and boolean operations/casts only; checked and floating families excluded | **Confirmed** |
| [LFS5](#lfs5--use-an-explicit-algebraic-identity-catalog) | Which identities apply? | One reviewed integer/bool table with width-specific constants and no floating identities | **Confirmed** |
| [LFS6](#lfs6--forward-values-atomically-without-adding-a-copy-rvalue) | How is `x op identity` represented? | Substitute safe uses and delete the obsolete assignment/value in one transaction | **Confirmed** |
| [LFS7](#lfs7--classify-every-use-before-forwarding) | What blocks forwarding? | Any proof, checked-protocol, lifecycle, ownership, or unknown use role | **Confirmed** |
| [LFS8](#lfs8--limit-cfg-rewriting-to-ordinary-branch-folding) | Which edges change? | Constant and same-target `Branch` become `Goto`; dedicated terminators never change | **Confirmed** |
| [LFS9](#lfs9--treat-proof-and-lifecycle-blocks-as-reachability-roots) | What CFG is retained? | Entry plus every attachment- or metadata-named block and its executable closure | **Confirmed** |
| [LFS10](#lfs10--remove-only-unprotected-unreachable-blocks-and-their-values) | What is deleted? | Blocks and block-defined transient values; retain storage and proof metadata | **Confirmed** |
| [LFS11](#lfs11--express-composition-through-an-explicit-repeated-schedule) | How do passes cooperate? | Deliberately repeat folding and dead-pure cleanup; keep whole-world reachability last | **Confirmed** |
| [LFS12](#lfs12--retain-the-existing-seal-and-invalidation-contract) | How are changes trusted? | Read verified input, atomic rewrite, immediate central reverification after every change | **Confirmed** |
| [LFS13](#lfs13--observe-reasons-without-duplicating-commit-accounting) | How is behavior measured? | Deterministic pass counters plus existing structural counts and verified checkpoints | **Confirmed** |
| [LFS14](#lfs14--make-no-language-or-target-contract-change) | What semantic assumptions change? | None; whole-world and single-threaded guarantees do not relax alias, failure, or lifecycle rules | **Confirmed** |

## LFS1 — Add three independent production passes

Register `primitive-constant-folding`,
`primitive-algebraic-simplification`, and `conservative-cfg-cleanup` as
separate selectable final-MIR transformations. The design name is not a hidden
fourth pass.

**Rationale:** the passes have different semantic barriers, measurements, and
future growth. Independent selection makes regressions bisectable and preserves
the framework's modular contract.

## LFS2 — Centralize exact target-independent primitive evaluation

Add one optimizer-private typed constant and evaluator owner shared by folding
and algebraic facts. Use explicit wrapping and width conversion, with a closed
unsupported result.

**Rationale:** duplicating arithmetic in passes invites semantic drift; using
Rust or a backend as the implicit evaluator risks build- or target-dependent
results.

## LFS3 — Keep scalar facts block-local and pass-local

Build facts in instruction order, clear them between blocks, and discard them
at occurrence end. Do not infer storage contents or cache across a changed
seal.

**Rationale:** this exactly matches MIR's transient-value rules and avoids
premature dominance, alias, SSA, and invalidation infrastructure.

## LFS4 — Start with a closed integer and boolean folding set

Fold the exact supported integer and boolean families described above. Exclude
checked protocols and floating evaluation until they receive dedicated
semantic and proof handling.

**Rationale:** the initial set is useful, total, and straightforward to test at
all boundaries. The exclusions avoid coupling this roadmap to proof
normalization or host-independent IEEE implementation.

## LFS5 — Use an explicit algebraic identity catalog

Keep the initial identities in one reviewed table, parameterized by encoded
integer width, and reject floating identities.

**Rationale:** an open-ended simplifier is difficult to audit. A closed catalog
makes exact behavior, tests, and future expansion reviewable.

## LFS6 — Forward values atomically without adding a copy rvalue

When an algebraic identity's result is an existing operand, replace eligible
uses and delete the obsolete definition/declaration in one rewrite
transaction. Do not introduce `MirRvalueKind::Copy` merely as an intermediate.

**Rationale:** current MIR already represents a copied transient by naming the
same value. A new copy node would burden verification and backends and would
remain when pass selection disables its consumer.

## LFS7 — Classify every use before forwarding

Add an exhaustive deterministic use-site query. Forward only when every use
is an explicitly allowed ordinary executable role; reject proof, checked,
lifecycle, ownership, and unknown roles.

**Rationale:** type equality and same-block dominance prove scalar availability
but not preservation of exact proof metadata. Eligibility must encode that
distinction before mutation.

## LFS8 — Limit CFG rewriting to ordinary branch folding

Rewrite constant and same-target ordinary boolean branches to `Goto`. Preserve
span and leave every dedicated checked or multiway terminator untouched.

**Rationale:** this produces real CFG simplification without weakening the
structural failure contracts already verified by dedicated terminators.

## LFS9 — Treat proof and lifecycle blocks as reachability roots

Compute local reachability from body entry plus all blocks named by callable
attachments and proof metadata. Share exhaustive reference knowledge rather
than maintaining an ad hoc list in the pass.

**Rationale:** static shutdown is a second executable region, while path and
logical records name exact shapes. Rooting those regions is a safe,
maintainable boundary before proof normalization exists.

## LFS10 — Remove only unprotected unreachable blocks and their values

Delete unreachable unprotected blocks and values defined inside them. Retain
storage declarations and all proof records. Perform deletion and dense
compaction through the existing atomic rewrite owner.

**Rationale:** values require exactly one retained definition; unused storage
does not justify adding storage-lifetime optimization. This granularity gives
useful cleanup without broadening semantic scope.

## LFS11 — Express composition through an explicit repeated schedule

Use the default schedule specified above, including two constant-folding and
three dead-pure occurrences, with whole-world reachability last. Reuse the
existing rule that disabling a stable name removes every occurrence.

**Rationale:** transformations expose one another's work. An explicit schedule
is deterministic, observable, and easier to reason about than a generic
pipeline fixed point.

## LFS12 — Retain the existing seal and invalidation contract

Preserve unchanged seals when no candidate exists. Any change atomically
commits dense MIR, invalidates all facts, and is immediately reverified by the
central final-MIR verifier.

**Rationale:** local simplicity is not a reason to add a second trust path.
The implemented seal boundary already gives exact failure attribution and
prevents malformed intermediate products from leaking.

## LFS13 — Observe reasons without duplicating commit accounting

Return deterministic pass-specific reasons and rely on existing rewrite
statistics for total structural changes. Use verified checkpoints for detailed
MIR inspection.

**Rationale:** reason counts explain effectiveness; commit counts remain the
single authority for entity changes.

## LFS14 — Make no language or target contract change

Preserve all existing source and runtime semantics. Treat closed-world and
single-threaded execution as context, not permission to assume immutable
memory or unobservable destruction.

**Rationale:** every proposed rule is valid without changing alias, lifecycle,
failure, or allocation contracts and remains target independent.

## Risks and mitigations

| Risk | Mitigation |
|---|---|
| Compiler evaluation differs from target semantics | One typed evaluator, explicit wrapping, boundary tests, and no initial floating folding |
| Algebraic simplification suppresses an observable producer | Never move/delete producers in the simplifier; let dead-pure apply its independent whitelist |
| Value forwarding invalidates proof metadata | Exhaustive use-site classification with unknown roles rejected |
| CFG pruning removes static shutdown or proof regions | Root every callable attachment and metadata-named block before closure |
| Removing a block leaves a declared value without a definition | Use exhaustive definition census and delete block-owned values atomically |
| Pass-local facts use stale instruction positions | Rebuild facts after structural deletion and never persist them across a commit |
| Repeated passes make selection confusing | Existing occurrence identities and all-occurrence stable-name exclusion remain authoritative |
| Conservative roots retain too much CFG | Measure protected unreachable blocks; normalize proof metadata only under a later design |
| A new MIR variant is optimized accidentally | Closed exhaustive matches in evaluator, use-site classifier, and CFG root collector |
| Whole-world reachability uses facts from pre-simplified MIR | Existing changed-pass invalidation and immediate verification recompute the seal-scoped product |

## Alternatives considered

### Add one monolithic `simplify` pass

This would hide which semantic boundary changed the program, prevent focused
selection, and make measurements less useful. The existing pipeline already
supports a deliberate multi-pass schedule.

### Add a copy rvalue and a standalone copy pass

This would manufacture an IR form absent from lowering, require verifier and
backend support, and leave artificial copies when selection disables the
consumer. Atomic forwarding expresses current algebraic identities directly.

### Propagate scalar loads within a block

Even a nearby load can observe mutation through direct or shared aliases.
Single-threaded execution removes data races, not sequential mutation. Load
and store reasoning should wait for storage epochs and alias/effect analysis.

### Fold checked diamonds when their operands are constant

This could remove failure edges and is potentially valuable, but the exact
divisor, shift, cast, cleanup, and lifecycle records would need coherent
rewriting. That is a later proof-aware optimization, not local rvalue folding.

### Fold floating operations using Rust `f64`

Most common results would agree, but NaN payload choice, host evaluation, and
future target support need an explicit deterministic contract. Integer and
boolean folding provides a strong first layer without settling that boundary.

### Forward empty blocks immediately

Niflheim benefits from this, but Skald records exact predecessors and joins in
proof and lifecycle metadata. A conservative branch fold plus unreachable
prune exercises CFG mutation while leaving topology normalization for a
dedicated design.

### Let failed output verification reject unsafe candidates

Verification remains mandatory, but using it as the applicability algorithm
would turn expected conservative barriers into internal compiler failures and
make pass behavior dependent on verifier diagnostics. Candidate selection
must be explicit.

### Build a global analysis manager now

The proposed facts are small and linear to rebuild. A cache protocol would add
invalidations and preservation declarations before measurements show a need.

## Effort and recommended delivery order

Overall effort is **medium to large**. Primitive evaluation is modest; the
largest correctness work is exhaustive forwarding eligibility and
proof/lifecycle-aware block retention.

| Delivery slice | Relative effort | Primary result |
|---|---|---|
| Typed primitive constant model and evaluator | Medium | One exact target-independent semantic owner |
| Block-local facts and constant-folding pass | Medium | First productive scalar rewrite |
| Use-site classification and atomic forwarding | Medium to large | Safe algebraic identity elimination without a copy node |
| Algebraic catalog and pass | Medium | Reviewed integer/bool simplification |
| CFG root and reachability support | Medium to large | Reusable proof-aware local control-flow facts |
| Branch folding and unreachable-region deletion | Large | First safe structural CFG cleanup |
| Registry/default schedule/report integration | Medium | Selectable repeated production suite |
| Parity, native, determinism, and documentation hardening | Medium to large | Default activation with evidence |

The implementation roadmap should preserve this dependency order:

1. freeze operation sets, stable names, measurements, and default schedule;
2. implement and exhaustively test the exact primitive evaluator;
3. add immutable block-local facts and primitive constant folding;
4. add exhaustive use-site roles and atomic value forwarding;
5. implement the algebraic identity catalog;
6. add callable attachment/proof roots and local CFG reachability;
7. implement branch folding and unreachable block/value deletion;
8. register the passes and activate the repeated default schedule; and
9. prove selection, optimization-off parity, native equivalence,
   determinism, and full repository gates.

Proof-record deletion, checked-diamond folding, floating evaluation, storage
propagation, empty-block forwarding, and any larger maintainability findings
should be recorded in the roadmap's discoveries file rather than absorbed
into implementation scope.

Potential optimization follow-ups beyond this design are inventoried in the
living
[optimization candidate catalog](../roadmaps/OPTIMIZATION_CANDIDATE_CATALOG.md). When one
is promoted into a confirmed design, implementation roadmap, active delivery,
or completed implementation, its catalog status should advance while the
authoritative detail moves to the corresponding design, roadmap, or living
compiler contract.

## Freeze and promotion

LFS1 through LFS14 are frozen as one bundle. The operation set, forwarding
eligibility, CFG roots, deletion granularity, and default schedule jointly
define the safety and composition boundary; freezing only the pass names would
leave the important decisions implicit.

The durable direction is promoted into the compiler phase, driver, and
reporting contracts linked from this proposal's status. The implementation
roadmap and discoveries record divide reviewed delivery from follow-up work.
The optimization candidate catalog now marks the three planned passes as
**Proposed** and should advance them to **In progress** and **Implemented** as
delivery proceeds.

This frozen record should move to `docs/archive/` only after the complete
implementation roadmap is delivered.
